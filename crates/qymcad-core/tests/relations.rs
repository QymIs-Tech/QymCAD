//! Relations between mates.
//!
//! The lower half of this file exercises the solver primitive itself: a single equation tying two already
//! measured degrees of freedom together. All four kinds of relation — gear, rack and pinion, screw and linear —
//! are that same equation with a different meaning attached to the coefficient, which is why it is worth
//! checking once and thoroughly.
//!
//! The upper half covers the document level: the relation object inside a project, its creation, the phase it
//! captures, and its effect through `solve_joints`.

use nalgebra::{Isometry3, Translation3, UnitQuaternion, Vector3};
use qymcad_core::asm::decompose::solve_assembly;
use qymcad_core::asm::frame::Anchor;
use qymcad_core::asm::joint::{Joint, JointKind};
use qymcad_core::asm::problem::{ratio_period, Body, Constraint, Problem, SlotMeasure};

fn at(x: f64, y: f64, z: f64) -> Isometry3<f64> {
    Isometry3::from_parts(Translation3::new(x, y, z), UnitQuaternion::identity())
}

/// Rotation of a body about the world Z axis, in degrees.
fn spin_deg(pose: &Isometry3<f64>) -> f64 {
    let x = pose.rotation * Vector3::x();
    x.y.atan2(x.x).to_degrees()
}

/// Two wheels on a shared housing. The first is driven by a given angle, the second is free.
///
/// Bodies: 0 is the grounded housing, 1 the driving wheel, 2 the driven one. The wheel axes are 50 mm apart and
/// both point along Z. Returns the problem and both measurements, which are what a relation ties together.
fn two_wheels(drive_deg: f64) -> (Problem, SlotMeasure, SlotMeasure) {
    let hub_a = Anchor::from_axes(0, Vector3::zeros(), Vector3::z(), Vector3::x()).unwrap();
    let rim_a = Anchor::from_axes(1, Vector3::zeros(), Vector3::z(), Vector3::x()).unwrap();
    let hub_b = Anchor::from_axes(0, Vector3::new(50.0, 0.0, 0.0), Vector3::z(), Vector3::x()).unwrap();
    let rim_b = Anchor::from_axes(2, Vector3::zeros(), Vector3::z(), Vector3::x()).unwrap();
    let ja = Joint::new(hub_a, rim_a, JointKind::Revolute).with_angle(drive_deg);
    let jb = Joint::new(hub_b, rim_b, JointKind::Revolute);
    let bodies = vec![Body::grounded(at(0.0, 0.0, 0.0)), Body::new(at(0.0, 0.0, 0.0)), Body::new(at(50.0, 0.0, 0.0))];
    let p = qymcad_core::asm::joint::problem_from(bodies, &[ja, jb]);
    (p, SlotMeasure::around(hub_a, rim_a, 2), SlotMeasure::around(hub_b, rim_b, 2))
}

/// Guard: without a relation the driven wheel does not move at all.
///
/// Checking that the second wheel turned is only meaningful once it is known that it stands still on its own.
/// Otherwise the test would go green for any reason at all — the initial placement, a pull, a stray solver
/// step.
#[test]
fn without_a_relation_the_second_wheel_does_not_move_at_all() {
    let (p, _, _) = two_wheels(30.0);
    let (poses, rep) = solve_assembly(&p);
    assert!(rep.converged, "two loops have to converge: {:.3e}", rep.residual);
    assert!((spin_deg(&poses[1]) - 30.0).abs() < 1e-6, "the driving wheel has to turn by the requested 30°, but turned by {:.6}", spin_deg(&poses[1]));
    assert!(spin_deg(&poses[2]).abs() < 1e-6, "without a relation the driven wheel has to stand still, but it turned by {:.6}°", spin_deg(&poses[2]));
    assert_eq!(rep.dof, 1, "exactly one degree of freedom has to remain: the angle of the second wheel");
}

/// A gear relation: the driving wheel turns 30° and the driven one 60° the other way.
#[test]
fn a_gear_relation_turns_the_second_wheel_by_the_ratio() {
    let (mut p, ma, mb) = two_wheels(30.0);
    // θ_b = −2·θ_a is written as θ_a = −0.5·θ_b
    p.add(Constraint::slot_ratio(ma, mb, -0.5, 0.0));
    let (poses, rep) = solve_assembly(&p);
    assert!(rep.converged, "a gear pair has to converge: {:.3e}", rep.residual);
    assert!((spin_deg(&poses[1]) - 30.0).abs() < 1e-6, "the driving wheel has to stay at the requested 30°: {:.6}", spin_deg(&poses[1]));
    assert!(
        (spin_deg(&poses[2]) + 60.0).abs() < 1e-4,
        "with a ratio of 2 and the driver at 30° the driven wheel has to sit at −60°, but sits at {:.6}°",
        spin_deg(&poses[2])
    );
    assert_eq!(rep.dof, 0, "the relation has to consume the last degree of freedom: the driven angle is no longer arbitrary");
    // the body did not travel: a relation holds the rotation, not the position
    assert!(
        (poses[2].translation.vector - Vector3::new(50.0, 0.0, 0.0)).norm() < 1e-6,
        "the driven wheel has to stay on its own axis, but travelled to {:?}",
        poses[2].translation.vector
    );
}

/// The gear ratio acts as a number, not as a sign: different numbers give different answers.
///
/// This guards against a fit: if the relation merely turned the driven wheel along with the driver, one
/// coefficient would pass and the rest would not.
#[test]
fn the_gear_ratio_is_obeyed_for_several_different_numbers() {
    let mut bad = Vec::new();
    // the constraint reads θ_driver = k·θ_driven, so the driven wheel lands at 20°/k
    for (ratio, want) in [(1.0, 20.0), (0.5, 40.0), (0.25, 80.0), (-1.0, -20.0)] {
        let (mut p, ma, mb) = two_wheels(20.0);
        p.add(Constraint::slot_ratio(ma, mb, ratio, 0.0));
        let (poses, rep) = solve_assembly(&p);
        let got = spin_deg(&poses[2]);
        if !rep.converged {
            bad.push(format!("k={ratio}: did not converge, residual {:.3e}", rep.residual));
        } else if (got - want).abs() > 1e-4 {
            bad.push(format!("k={ratio}: expected {want}°, got {got:.6}°"));
        }
    }
    assert!(bad.is_empty(), "the gear ratio is not honoured:\n  {}", bad.join("\n  "));
}

/// Passing half a turn does not break a gear relation.
///
/// With the driver at 170° and a ratio of 2 the driven wheel needs 340°, while the angle measurement reports
/// −20°: the same position under a different number. Without a period on the residual the solver would see a
/// 180° miss and chase a solution that does not exist. The period is derived in `ratio_period`, and here it is
/// verified by the body ending up where it should.
#[test]
fn passing_half_a_turn_does_not_break_a_gear_relation() {
    let (mut p, ma, mb) = two_wheels(170.0);
    p.add(Constraint::slot_ratio(ma, mb, 0.5, 0.0));
    let (poses, rep) = solve_assembly(&p);
    assert!(rep.converged, "the relation has to converge past half a turn as well: {:.3e}", rep.residual);
    // 340° and −20° are the same wheel position
    let got = spin_deg(&poses[2]);
    assert!((got + 20.0).abs() < 1e-3, "the driven wheel has to sit at 340°, that is −20°, but sits at {got:.6}°");
}

/// The driven wheel travels smoothly across the seam of the measurement.
///
/// The driver is walked through the point where the measured angle of the driven wheel jumps from +180° to
/// −180°, and the actual rotation of the body is what gets checked. A jump would show on screen as the gear
/// being yanked, which is exactly what the period exists to prevent.
#[test]
fn the_driven_wheel_moves_smoothly_across_the_measurement_seam() {
    let mut prev: Option<f64> = None;
    let mut jumps = Vec::new();
    let mut steps = 0;
    for i in 0..=20 {
        let drive = 80.0 + i as f64 * 1.0; // the driven wheel sweeps 160°..200°, crossing the seam at 180°
        let (mut p, ma, mb) = two_wheels(drive);
        p.add(Constraint::slot_ratio(ma, mb, 0.5, 0.0));
        let (poses, rep) = solve_assembly(&p);
        assert!(rep.converged, "step {drive}°: has to converge, residual {:.3e}", rep.residual);
        let got = spin_deg(&poses[2]);
        if let Some(p0) = prev {
            // rotation of the body between steps, wrapped to (−180,180]; otherwise the seam of the
            // measurement itself would pass for a jump of the body
            let d = (got - p0 + 540.0) % 360.0 - 180.0;
            if (d - 2.0).abs() > 0.01 {
                jumps.push(format!("at {drive}° the driven wheel stepped by {d:.4}° instead of 2°"));
            }
            steps += 1;
        }
        prev = Some(got);
    }
    assert_eq!(steps, 20, "guard: there have to be twenty comparison steps, there were {steps}");
    assert!(jumps.is_empty(), "the travel of the driven wheel is discontinuous:\n  {}", jumps.join("\n  "));
}

/// A linear relation: two sliders, the second travelling twice as far as the first.
#[test]
fn a_linear_relation_makes_one_slider_travel_twice_the_other() {
    let rail_a = Anchor::from_axes(0, Vector3::zeros(), Vector3::z(), Vector3::x()).unwrap();
    let car_a = Anchor::from_axes(1, Vector3::zeros(), Vector3::z(), Vector3::x()).unwrap();
    let rail_b = Anchor::from_axes(0, Vector3::new(0.0, 40.0, 0.0), Vector3::z(), Vector3::x()).unwrap();
    let car_b = Anchor::from_axes(2, Vector3::zeros(), Vector3::z(), Vector3::x()).unwrap();
    let ja = Joint::new(rail_a, car_a, JointKind::Slider).with_offset(20.0);
    let jb = Joint::new(rail_b, car_b, JointKind::Slider);
    let bodies = vec![Body::grounded(at(0.0, 0.0, 0.0)), Body::new(at(0.0, 0.0, 0.0)), Body::new(at(0.0, 40.0, 0.0))];
    let mut p = qymcad_core::asm::joint::problem_from(bodies, &[ja, jb]);
    // travel of b = 2 · travel of a, i.e. travel of a = 0.5 · travel of b
    p.add(Constraint::slot_ratio(SlotMeasure::along(rail_a, car_a, 2), SlotMeasure::along(rail_b, car_b, 2), 0.5, 0.0));
    let (poses, rep) = solve_assembly(&p);
    assert!(rep.converged, "a linear relation has to converge: {:.3e}", rep.residual);
    assert!((poses[1].translation.vector.z - 20.0).abs() < 1e-6, "the first slider has to travel the requested 20 mm, but travelled {:.6}", poses[1].translation.vector.z);
    assert!(
        (poses[2].translation.vector.z - 40.0).abs() < 1e-4,
        "the second has to travel twice as far, 40 mm, but travelled {:.6}",
        poses[2].translation.vector.z
    );
    assert_eq!(rep.dof, 0, "the relation has to consume the freedom of the second slider");
}

/// Rack and pinion: one turn of the pinion moves the rack by the given distance per revolution.
///
/// That number is exactly the coefficient of the constraint expressed in radians: travel = (pitch/2π)·angle.
/// Here the pitch is 60 mm, so a quarter turn gives 15 mm.
#[test]
fn a_rack_and_pinion_relation_moves_the_rack_by_the_distance_per_revolution() {
    let hub = Anchor::from_axes(0, Vector3::zeros(), Vector3::z(), Vector3::x()).unwrap();
    let rim = Anchor::from_axes(1, Vector3::zeros(), Vector3::z(), Vector3::x()).unwrap();
    // the rack travels along world X, so the primary axis of the rail points along X
    let rail = Anchor::from_axes(0, Vector3::new(0.0, 30.0, 0.0), Vector3::x(), Vector3::z()).unwrap();
    let rack = Anchor::from_axes(2, Vector3::zeros(), Vector3::x(), Vector3::z()).unwrap();
    let pinion = Joint::new(hub, rim, JointKind::Revolute).with_angle(90.0);
    let slide = Joint::new(rail, rack, JointKind::Slider);
    let bodies = vec![Body::grounded(at(0.0, 0.0, 0.0)), Body::new(at(0.0, 0.0, 0.0)), Body::new(at(0.0, 30.0, 0.0))];
    let mut p = qymcad_core::asm::joint::problem_from(bodies, &[pinion, slide]);
    let per_turn = 60.0;
    // rack travel = (pitch/2π)·pinion angle
    let m_rack = SlotMeasure::along(rail, rack, 2);
    let m_pinion = SlotMeasure::around(hub, rim, 2);
    p.add(Constraint::slot_ratio(m_rack, m_pinion, per_turn / std::f64::consts::TAU, 0.0));
    let (poses, rep) = solve_assembly(&p);
    assert!(rep.converged, "rack and pinion has to converge: {:.3e}", rep.residual);
    let x = poses[2].translation.vector.x;
    assert!((x - 15.0).abs() < 1e-4, "a quarter turn at 60 mm per revolution has to move the rack by 15 mm, but it moved {x:.6}");
}

/// A screw relation lives inside a single mate: the angle and the travel of one cylindrical joint.
///
/// It is the only relation that needs one mate rather than two. Here that is expressed by both measurements
/// being built on the very same pair of anchors, so no special case appears in the solver.
#[test]
fn a_screw_relation_ties_the_angle_and_the_travel_of_one_and_the_same_mate() {
    let nut = Anchor::from_axes(0, Vector3::zeros(), Vector3::z(), Vector3::x()).unwrap();
    let bolt = Anchor::from_axes(1, Vector3::zeros(), Vector3::z(), Vector3::x()).unwrap();
    // A quarter turn, not a half. At exactly 180° the problem is genuinely two-valued: the travel is
    // determined only up to the pitch — that is the period of a screw constraint, since a thread repeats every
    // revolution — and −1.25 mm is exactly as far from the current position as +1.25. Demanding one definite
    // answer at that point would be asking the solver to call a coin toss.
    let j = Joint::new(nut, bolt, JointKind::Cylindrical).with_angle(90.0);
    let bodies = vec![Body::grounded(at(0.0, 0.0, 0.0)), Body::new(at(0.0, 0.0, 0.0))];
    let mut p = qymcad_core::asm::joint::problem_from(bodies, &[j]);
    let pitch = 2.5; // mm per revolution
    p.add(Constraint::slot_ratio(SlotMeasure::along(nut, bolt, 2), SlotMeasure::around(nut, bolt, 2), pitch / std::f64::consts::TAU, 0.0));
    let (poses, rep) = solve_assembly(&p);
    assert!(rep.converged, "a screw constraint has to converge: {:.3e}", rep.residual);
    assert!((spin_deg(&poses[1]) - 90.0).abs() < 1e-6, "the bolt has to turn by the requested 90°, but turned by {:.6}", spin_deg(&poses[1]));
    assert!(
        (poses[1].translation.vector.z - pitch / 4.0).abs() < 1e-4,
        "a quarter turn at a pitch of 2.5 mm has to move the bolt by 0.625 mm, but it moved {:.6}",
        poses[1].translation.vector.z
    );
    assert_eq!(rep.dof, 0, "a cylindrical joint has two degrees of freedom: the angle is driven and the screw relation takes the travel");
}

/// The period of the residual is computed, not assigned.
#[test]
fn the_period_of_a_relation_follows_from_what_it_relates() {
    let turn = std::f64::consts::TAU;
    let near = |a: f64, b: f64| (a - b).abs() < 1e-12;
    assert!(near(ratio_period(false, false, 2.0), 0.0), "two travels do not repeat, so there is no period");
    assert!(near(ratio_period(true, false, 2.0), turn), "only the first measurement is angular, so the period is one revolution");
    assert!(near(ratio_period(false, true, 3.0), 3.0 * turn), "only the second is angular, so the period is the distance per revolution");
    assert!(near(ratio_period(true, true, 2.0), turn), "a ratio of 2 repeats the configuration once per revolution of the driver");
    assert!(near(ratio_period(true, true, 0.5), turn / 2.0), "a ratio of 1:2 repeats twice as often, otherwise the relation breaks at half a turn");
    assert!(near(ratio_period(true, true, 1.5), turn / 2.0), "3/2 has a denominator of two");
    assert!(near(ratio_period(true, true, 17.0 / 53.0), turn / 53.0), "17/53 has a denominator of fifty-three");
    // an irrational number has no period at all, and inventing one is not allowed
    assert!(near(ratio_period(true, true, std::f64::consts::SQRT_2), 0.0), "with an irrational number the configurations never repeat, so there is no period");
}

/// A relation keeps a part together: two wheels on a shared housing are solved as one problem.
///
/// The split into independent parts follows the bodies of a constraint, and a relation has up to four of them.
/// Splitting it by the first pair alone would move the second loop into a separate part where nothing knows
/// about the relation.
#[test]
fn a_relation_keeps_both_mates_in_one_solved_part() {
    let (mut p, ma, mb) = two_wheels(30.0);
    p.add(Constraint::slot_ratio(ma, mb, -0.5, 0.0));
    let (_, rep) = solve_assembly(&p);
    assert_eq!(rep.parts, 1, "mates tied by a relation have to be solved together, but split into {} parts", rep.parts);
}

/// A broken reference inside a relation is caught before solving, rather than turning into an out-of-bounds
/// access.
#[test]
fn a_broken_reference_inside_a_relation_is_detected_before_solving() {
    let a = Anchor::origin(0);
    let ghost = Anchor::origin(7);
    let mut p = Problem::new(vec![Body::grounded(Isometry3::identity()), Body::new(at(10.0, 0.0, 0.0))]);
    p.add(Constraint::slot_ratio(SlotMeasure::along(a, Anchor::origin(1), 2), SlotMeasure::along(a, ghost, 2), 1.0, 0.0));
    assert!(!p.references_are_valid(), "a reference to a non-existent body inside a relation has to be caught");
}

// ─── DOCUMENT LEVEL ───────────────────────────────────────────────────────────────────────────
//
// The solver primitive is checked above. What follows is what the user sees: the relation object inside a
// project, its creation, the phase it captures and its effect through `solve_joints`.

// the document-level joint kind, not to be confused with the identically named solver kind used above
use qymcad_core::feature::{apply12, AnchorRef, JointKind as DocJointKind, RelationKind};
use qymcad_core::model::{Id, Project};

fn tr(x: f64, y: f64, z: f64) -> [f64; 12] {
    [1.0, 0.0, 0.0, x, 0.0, 1.0, 0.0, y, 0.0, 0.0, 1.0, z]
}

fn set_transform(p: &mut Project, comp: Id, m: [f64; 12]) {
    let i = p.component_index(comp).unwrap();
    p.components[i].transform = m;
}

/// Rotation of a component about the world Z axis, taken from its matrix, in degrees.
fn part_spin_deg(p: &Project, comp: Id) -> f64 {
    let m = p.world_transform(comp);
    let o = apply12(&m, [0.0, 0.0, 0.0]);
    let x = apply12(&m, [1.0, 0.0, 0.0]);
    (x[1] - o[1]).atan2(x[0] - o[0]).to_degrees()
}

/// A document with two loops: two grounded housings and a wheel on each.
///
/// Returns the project, both wheels and both mates.
fn doc_two_wheels() -> (Project, [Id; 2], [Id; 2]) {
    let mut p = Project::default();
    p.new_document();
    let root = p.root;
    p.set_active_component(Some(root));
    let hub_a = p.add_part("Housing A");
    let wheel_a = p.add_part("Wheel A");
    let hub_b = p.add_part("Housing B");
    let wheel_b = p.add_part("Wheel B");
    for c in [hub_b, wheel_b] {
        set_transform(&mut p, c, tr(50.0, 0.0, 0.0));
    }
    p.set_grounded(hub_a, true);
    p.set_grounded(hub_b, true);
    let (ca, cb) = (p.add_connector(hub_a, AnchorRef::Origin), p.add_connector(wheel_a, AnchorRef::Origin));
    let ja = p.add_joint(ca, cb, DocJointKind::Revolute);
    let (cc, cd) = (p.add_connector(hub_b, AnchorRef::Origin), p.add_connector(wheel_b, AnchorRef::Origin));
    let jb = p.add_joint(cc, cd, DocJointKind::Revolute);
    (p, [wheel_a, wheel_b], [ja, jb])
}

/// Drive the angle of a mate, slot 0, and solve.
fn drive_angle(p: &mut Project, joint: Id, deg: f64) {
    if let Some(j) = p.joints.iter_mut().find(|j| j.id == joint) {
        j.drive[0] = Some(deg);
    }
    p.solve_joints();
}

/// Document-level guard: without a relation the second wheel stands still.
#[test]
fn in_a_document_the_second_wheel_stays_put_until_a_relation_is_added() {
    let (mut p, [_, wheel_b], [ja, _]) = doc_two_wheels();
    drive_angle(&mut p, ja, 30.0);
    assert!(part_spin_deg(&p, wheel_b).abs() < 1e-6, "without a relation the second wheel has to stand still, but turned by {:.6}°", part_spin_deg(&p, wheel_b));
}

/// A gear relation in a document drives the second wheel.
#[test]
fn a_gear_relation_in_a_document_drives_the_second_wheel() {
    let (mut p, [wheel_a, wheel_b], [ja, jb]) = doc_two_wheels();
    let rid = p.add_relation(RelationKind::Gear, ja, 0, jb, 0, 2.0);
    assert!(p.relation_faults().is_empty(), "a freshly created relation has to be sound: {:?}", p.relation_faults());
    drive_angle(&mut p, ja, 30.0);
    assert!((part_spin_deg(&p, wheel_a) - 30.0).abs() < 1e-4, "the driving wheel has to land at 30°, but landed at {:.6}", part_spin_deg(&p, wheel_a));
    assert!(
        (part_spin_deg(&p, wheel_b) - 60.0).abs() < 1e-3,
        "with a ratio of 2 the driven wheel has to turn by 60°, but turned by {:.6}°",
        part_spin_deg(&p, wheel_b)
    );
    // and deleting the relation gives the freedom back
    p.delete_relation(rid);
    drive_angle(&mut p, ja, 45.0);
    assert!(
        (part_spin_deg(&p, wheel_b) - 60.0).abs() < 1e-3,
        "after the relation is deleted the driven wheel has to stay where it was, at 60°, but ended up at {:.6}°",
        part_spin_deg(&p, wheel_b)
    );
}

/// The reverse flag changes the direction, not the magnitude.
#[test]
fn the_reverse_flag_turns_the_second_wheel_the_other_way() {
    let (mut p, [_, wheel_b], [ja, jb]) = doc_two_wheels();
    let rid = p.add_relation(RelationKind::Gear, ja, 0, jb, 0, 2.0);
    p.relations.iter_mut().find(|r| r.id == rid).expect("relation").reversed = true;
    drive_angle(&mut p, ja, 30.0);
    assert!(
        (part_spin_deg(&p, wheel_b) + 60.0).abs() < 1e-3,
        "with the reverse flag the driven wheel has to go to −60°, but went to {:.6}°",
        part_spin_deg(&p, wheel_b)
    );
}

/// Creating a relation moves nothing.
///
/// Gears standing in arbitrary positions must not jump when a gear relation is added: the relation holds the
/// motion, not the absolute readings. Capturing the phase is what makes that true.
#[test]
fn creating_a_relation_does_not_move_anything() {
    let (mut p, [_, wheel_b], [ja, jb]) = doc_two_wheels();
    // the driven wheel is pre-turned to 17° and the driver to 5°, which a ratio of 2 does not relate
    drive_angle(&mut p, ja, 5.0);
    if let Some(j) = p.joints.iter_mut().find(|j| j.id == jb) {
        j.drive[0] = Some(17.0);
    }
    p.solve_joints();
    let before = part_spin_deg(&p, wheel_b);
    assert!((before - 17.0).abs() < 1e-3, "setup: the driven wheel has to sit at 17°, but sits at {before:.6}");
    // release the driven wheel and add the relation: the body must not stir
    if let Some(j) = p.joints.iter_mut().find(|j| j.id == jb) {
        j.drive[0] = None;
    }
    p.add_relation(RelationKind::Gear, ja, 0, jb, 0, 2.0);
    p.solve_joints();
    let after = part_spin_deg(&p, wheel_b);
    assert!((after - before).abs() < 1e-3, "creating the relation turned the body from {before:.4}° to {after:.4}°, so the phase was not captured");
    // from then on the motion is tied: another 10° on the driver gives 20° on the driven wheel
    drive_angle(&mut p, ja, 15.0);
    let moved = part_spin_deg(&p, wheel_b);
    assert!((moved - (before + 20.0)).abs() < 1e-2, "the driver travelled 10°, so the driven wheel has to travel 20°, reaching {:.4}°, but ended at {moved:.4}°", before + 20.0);
}

/// A relation that cannot work names the reason instead of staying silent.
#[test]
fn a_relation_that_cannot_work_says_why() {
    let (mut p, _, [ja, jb]) = doc_two_wheels();
    let mut named: Vec<&str> = Vec::new();

    // the mate does not exist at all
    let r1 = p.add_relation(RelationKind::Gear, ja, 0, 99_999 as Id, 0, 2.0);
    named.extend(p.relation_faults().iter().filter(|(id, _)| *id == r1).map(|(_, w)| *w));
    p.delete_relation(r1);

    // a revolute joint has no travel slot, and a linear relation asks for exactly that
    let r2 = p.add_relation(RelationKind::Linear, ja, 1, jb, 1, 2.0);
    named.extend(p.relation_faults().iter().filter(|(id, _)| *id == r2).map(|(_, w)| *w));
    p.delete_relation(r2);

    // a gear relation on one and the same mate
    let r3 = p.add_relation(RelationKind::Gear, ja, 0, ja, 0, 2.0);
    named.extend(p.relation_faults().iter().filter(|(id, _)| *id == r3).map(|(_, w)| *w));
    p.delete_relation(r3);

    // a screw relation across two mates
    let r4 = p.add_relation(RelationKind::Screw, ja, 0, jb, 0, 2.0);
    named.extend(p.relation_faults().iter().filter(|(id, _)| *id == r4).map(|(_, w)| *w));

    assert_eq!(named.len(), 4, "guard: four faults were staged, {} were named: {named:?}", named.len());
    assert!(named.contains(&"r-fault-mate-lost"), "a lost mate has to be named: {named:?}");
    assert!(named.contains(&"r-fault-slot-lost"), "a non-existent degree of freedom has to be named: {named:?}");
    assert!(named.contains(&"r-fault-same-mate"), "a gear relation on a single mate has to be named: {named:?}");
    assert!(named.contains(&"r-fault-two-mates"), "a screw relation across two mates has to be named: {named:?}");
}

/// A relation survives a save. A field missing from the file schema is lost silently.
#[test]
fn a_relation_survives_a_round_trip_through_the_file() {
    let (mut p, _, [ja, jb]) = doc_two_wheels();
    let rid = p.add_relation(RelationKind::Gear, ja, 0, jb, 0, 2.5);
    p.relations.iter_mut().find(|r| r.id == rid).expect("relation").reversed = true;
    let text = qymcad_core::model::to_ron(&p).expect("the document is written");
    let back = qymcad_core::model::from_ron(&text).expect("the document is read back");
    let r = back.relations.iter().find(|r| r.id == rid).expect("the relation has to survive the save");
    assert_eq!(r.kind, RelationKind::Gear, "the kind has to be preserved");
    assert!((r.value - 2.5).abs() < 1e-12, "the value has to be preserved: {}", r.value);
    assert!(r.reversed, "the reverse flag has to be preserved");
    assert_eq!((r.a, r.slot_a, r.b, r.slot_b), (ja, 0, jb, 0), "what the relation points at has to be preserved");
}

/// A pin-slot travel can be tied by a relation too, rather than being skipped silently.
///
/// The travel of a pin-slot is measured along the axis of the second anchor, which owns the slot, while the
/// relation measurement takes its axis from the first. Until those agreed, the bridge dropped such
/// constraints: the relation sat in the list and did nothing at all, which is worse than a refusal — the
/// relation is visible and there is no explanation for the body not moving.
#[test]
fn a_pin_slot_travel_can_be_tied_by_a_relation_too() {
    let mut p = Project::default();
    p.new_document();
    let root = p.root;
    p.set_active_component(Some(root));
    let before: Vec<Id> = p.components.iter().filter(|c| c.parent == Some(root)).map(|c| c.id).collect();
    let (plate, pin, rail, car) = (p.add_part("Plate"), p.add_part("Pin"), p.add_part("Rail"), p.add_part("Carriage"));
    assert_eq!(before.len() + 4, p.components.iter().filter(|c| c.parent == Some(root)).count(), "setup: exactly four parts were added");
    p.set_grounded(plate, true);
    p.set_grounded(rail, true);
    let (c1, c2) = (p.add_connector(plate, AnchorRef::Origin), p.add_connector(pin, AnchorRef::Origin));
    let slot_joint = p.add_joint(c1, c2, DocJointKind::PinSlot);
    let (c3, c4) = (p.add_connector(rail, AnchorRef::Origin), p.add_connector(car, AnchorRef::Origin));
    let slide = p.add_joint(c3, c4, DocJointKind::Slider);

    // Guard: without a relation the pin stands still however far the carriage is driven.
    if let Some(j) = p.joints.iter_mut().find(|j| j.id == slide) {
        j.drive[1] = Some(10.0);
    }
    p.solve_joints();
    let idle = apply12(&p.world_transform(pin), [0.0, 0.0, 0.0]);
    assert!(idle[0].abs() < 1e-6, "setup: without a relation the pin has to stand still, but it is at {idle:?}");

    // Return the carriage to zero before creating the relation. A relation captures the phase and holds
    // whatever arrangement it finds: created now, it would declare "carriage at ten, pin at zero" correct and
    // the pin would legitimately not move. That is not a flaw but the very behaviour the phase exists for;
    // what has to be checked here is the travel, so both start from a common zero.
    if let Some(j) = p.joints.iter_mut().find(|j| j.id == slide) {
        j.drive[1] = Some(0.0);
    }
    p.solve_joints();

    // tie them: the pin travels along the slot twice as far as the carriage
    p.add_relation(RelationKind::Linear, slide, 1, slot_joint, 1, 2.0);
    if let Some(j) = p.joints.iter_mut().find(|j| j.id == slide) {
        j.drive[1] = Some(10.0);
    }
    assert!(p.relation_faults().is_empty(), "the relation has to be sound: {:?}", p.relation_faults());
    p.solve_joints();
    let moved = apply12(&p.world_transform(pin), [0.0, 0.0, 0.0]);
    assert!(
        (moved[0] - 20.0).abs() < 1e-3,
        "the carriage travelled 10 mm, so the pin has to travel 20 along the slot, but it is at {moved:?}"
    );
}

/// A relation drives the second mate the right way when an anchor is turned around.
///
/// A relation measures degrees of freedom with the same measurement that produces the reading, and the solver
/// may flip the first anchor on its own, since the mating side is chosen by proximity. Let the orientation of
/// the measurement diverge from that of the joint and the driven body travels in exactly the opposite
/// direction: the numbers agree while the mechanism runs backwards.
#[test]
fn a_relation_drives_the_second_mate_the_right_way_on_a_turned_anchor() {
    let mut p = Project::default();
    p.new_document();
    let root = p.root;
    p.set_active_component(Some(root));
    let (rail1, car1, rail2, car2) = (p.add_part("Rail 1"), p.add_part("Carriage 1"), p.add_part("Rail 2"), p.add_part("Carriage 2"));
    p.set_grounded(rail1, true);
    p.set_grounded(rail2, true);
    // the second carriage is turned 180° about X, so its anchor faces its own rail head-on
    for (c, m) in [(rail2, tr(60.0, 0.0, 0.0)), (car2, [1.0, 0.0, 0.0, 60.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0])] {
        let i = p.component_index(c).unwrap();
        p.components[i].transform = m;
    }
    let (c1, c2) = (p.add_connector(rail1, AnchorRef::Origin), p.add_connector(car1, AnchorRef::Origin));
    let ja = p.add_joint(c1, c2, DocJointKind::Slider);
    let (c3, c4) = (p.add_connector(rail2, AnchorRef::Origin), p.add_connector(car2, AnchorRef::Origin));
    let jb = p.add_joint(c3, c4, DocJointKind::Slider);

    // bring both to zero before creating the relation, since it captures the phase
    for j in [ja, jb] {
        if let Some(x) = p.joints.iter_mut().find(|x| x.id == j) {
            x.drive[1] = Some(0.0);
        }
    }
    p.solve_joints();
    // And release the driven degree of freedom: a driven slot is not free, and the relation would have
    // nothing to drive. Omitting this makes the check fail with a reading of 0.0000, an error in the setup
    // rather than in the code under test.
    if let Some(x) = p.joints.iter_mut().find(|x| x.id == jb) {
        x.drive[1] = None;
    }
    let dir_b = p.joint_slot_axis(jb, 1, root).expect("travel axis of the second mate");
    let base = apply12(&p.world_transform(car2), [0.0, 0.0, 0.0]);

    p.add_relation(RelationKind::Linear, ja, 1, jb, 1, 2.0);
    assert!(p.relation_faults().is_empty(), "the relation has to be sound: {:?}", p.relation_faults());
    if let Some(x) = p.joints.iter_mut().find(|x| x.id == ja) {
        x.drive[1] = Some(10.0);
    }
    p.solve_joints();

    let now = apply12(&p.world_transform(car2), [0.0, 0.0, 0.0]);
    let d = [now[0] - base[0], now[1] - base[1], now[2] - base[2]];
    let along = d[0] * dir_b[0] + d[1] * dir_b[1] + d[2] * dir_b[2];
    assert!(
        (along - 20.0).abs() < 1e-2,
        "the first carriage travelled 10 mm, so the second has to travel 20 along its own arrow, but travelled {along:.4}"
    );
}

/// A gear relation turns the driven wheel the right way on a turned anchor.
///
/// The linear relation is covered above, where this was in fact broken. The gear case was not, and turning an
/// anchor flips the axis of rotation and with it the sign of the angle.
#[test]
fn a_gear_relation_turns_the_driven_wheel_the_right_way_on_a_turned_anchor() {
    let mut p = Project::default();
    p.new_document();
    let root = p.root;
    p.set_active_component(Some(root));
    let (hub1, wheel1, hub2, wheel2) = (p.add_part("Housing 1"), p.add_part("Wheel 1"), p.add_part("Housing 2"), p.add_part("Wheel 2"));
    p.set_grounded(hub1, true);
    p.set_grounded(hub2, true);
    // the second wheel is turned 180° about X, so its anchor faces its own housing head-on
    for (c, m) in [(hub2, tr(60.0, 0.0, 0.0)), (wheel2, [1.0, 0.0, 0.0, 60.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0])] {
        let i = p.component_index(c).unwrap();
        p.components[i].transform = m;
    }
    let (c1, c2) = (p.add_connector(hub1, AnchorRef::Origin), p.add_connector(wheel1, AnchorRef::Origin));
    let ja = p.add_joint(c1, c2, DocJointKind::Revolute);
    let (c3, c4) = (p.add_connector(hub2, AnchorRef::Origin), p.add_connector(wheel2, AnchorRef::Origin));
    let jb = p.add_joint(c3, c4, DocJointKind::Revolute);

    // bring both to zero before creating the relation, which captures the phase, and release the driven one
    for j in [ja, jb] {
        if let Some(x) = p.joints.iter_mut().find(|x| x.id == j) {
            x.drive[0] = Some(0.0);
        }
    }
    p.solve_joints();
    if let Some(x) = p.joints.iter_mut().find(|x| x.id == jb) {
        x.drive[0] = None;
    }

    let axis = p.joint_slot_axis(jb, 0, root).expect("rotation axis of the driven wheel");
    let zero = p.joint_zero_dir(jb, root).expect("angle zero of the driven wheel");
    let spin = |p: &Project| {
        let m = p.world_transform(wheel2);
        let o = apply12(&m, [0.0, 0.0, 0.0]);
        let px = apply12(&m, [1.0, 0.0, 0.0]);
        let v = [px[0] - o[0], px[1] - o[1], px[2] - o[2]];
        let cr = [zero[1] * v[2] - zero[2] * v[1], zero[2] * v[0] - zero[0] * v[2], zero[0] * v[1] - zero[1] * v[0]];
        let s = axis[0] * cr[0] + axis[1] * cr[1] + axis[2] * cr[2];
        let c = zero[0] * v[0] + zero[1] * v[1] + zero[2] * v[2];
        s.atan2(c).to_degrees()
    };
    let before = spin(&p);

    p.add_relation(RelationKind::Gear, ja, 0, jb, 0, 2.0);
    assert!(p.relation_faults().is_empty(), "the relation has to be sound: {:?}", p.relation_faults());
    if let Some(x) = p.joints.iter_mut().find(|x| x.id == ja) {
        x.drive[0] = Some(20.0);
    }
    p.solve_joints();

    let d = (spin(&p) - before + 540.0) % 360.0 - 180.0;
    assert!(
        (d - 40.0).abs() < 1e-2,
        "the driving wheel travelled 20°, so the driven one has to travel 40° in the direction of its own gizmo, but travelled {d:.4}°"
    );
}
