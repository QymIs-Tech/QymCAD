use qymcad_core::feature::{apply12, AnchorRef, BasePlane, FaceKey, JointKind};
use qymcad_core::model::{Id, Project};

struct Lever {
    project: Project,
    part: Id,
}

fn hinged_lever(hinge_x: f64) -> Lever {
    let mut p = Project::default();
    p.new_document();
    let root = p.root;
    p.set_active_component(Some(root));
    let ground = p.add_part("G");
    let part = p.add_part("L");
    p.set_grounded(ground, true);
    p.set_active_component(Some(part));
    let body = p.add_extrude(2, 5.0);
    let on_ground = p.add_connector(ground, AnchorRef::Origin);
    let key = FaceKey { index: 0, centroid: [hinge_x, 0.0, 0.0], normal: [0.0, 0.0, 1.0], id: 0 };
    let on_lever = p.add_connector(part, AnchorRef::FaceCenter(body, key));
    p.add_joint(on_ground, on_lever, JointKind::Revolute);
    p.solve_joints();
    Lever { project: p, part }
}

fn world(p: &Project, comp: Id, local: [f64; 3]) -> [f64; 3] {
    apply12(&p.world_transform(comp), local)
}

#[derive(Clone, Copy)]
struct Gap {
    from: [f64; 3],
    to: [f64; 3],
}

fn distance(gap: Gap) -> f64 {
    let (a, b) = (gap.from, gap.to);
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

#[derive(Clone, Copy)]
struct Lead {
    grab: [f64; 3],
    cursor: [f64; 3],
}

fn pull(p: &mut Project, comp: Id, lead: Lead) {
    p.drag_pull = Some((comp, lead.grab, lead.cursor));
    p.solve_joints();
    p.drag_pull = None;
}

#[test]
fn a_point_off_the_axis_turns_a_part_whose_origin_is_on_the_axis() {
    let mut l = hinged_lever(0.0);
    let tip = [30.0, 0.0, 0.0];
    let target = [30.0 * 40f64.to_radians().cos(), 30.0 * 40f64.to_radians().sin(), 0.0];
    pull(&mut l.project, l.part, Lead { grab: tip, cursor: target });
    let miss = distance(Gap { from: world(&l.project, l.part, tip), to: target });
    assert!(miss < 0.5, "the tip was led towards a point 40 deg round the hinge and is still {miss:.3} mm from it");
    let pivot = world(&l.project, l.part, [0.0, 0.0, 0.0]);
    assert!(distance(Gap { from: pivot, to: [0.0; 3] }) < 1e-6, "the hinge came apart: the pivot stands at {pivot:?}");
}

#[test]
fn a_point_off_the_axis_follows_the_cursor_on_either_side() {
    for sign in [1.0, -1.0] {
        let mut l = hinged_lever(0.0);
        let tip = [30.0, 0.0, 0.0];
        let a = sign * 25f64.to_radians();
        let target = [30.0 * a.cos(), 30.0 * a.sin(), 0.0];
        pull(&mut l.project, l.part, Lead { grab: tip, cursor: target });
        let miss = distance(Gap { from: world(&l.project, l.part, tip), to: target });
        assert!(miss < 0.5, "cursor {} deg round the hinge: the tip is {miss:.3} mm from it", sign * 25.0);
    }
}

#[test]
fn the_pivot_itself_asked_to_leave_the_axis_stays_and_the_hinge_holds() {
    let mut l = hinged_lever(0.0);
    pull(&mut l.project, l.part, Lead { grab: [0.0; 3], cursor: [0.0, 20.0, 0.0] });
    let pivot = world(&l.project, l.part, [0.0, 0.0, 0.0]);
    assert!(distance(Gap { from: pivot, to: [0.0; 3] }) < 1e-6, "the pivot was torn off its hinge: {pivot:?}");
    assert!(!l.project.mates_conflict, "asking the pivot to leave the axis made the hinge conflict");
}

#[test]
fn the_origin_off_the_axis_is_led_as_before() {
    let mut l = hinged_lever(-30.0);
    let from = world(&l.project, l.part, [0.0, 0.0, 0.0]);
    pull(&mut l.project, l.part, Lead { grab: [0.0; 3], cursor: [from[0], from[1] + 20.0, 0.0] });
    let after = world(&l.project, l.part, [0.0, 0.0, 0.0]);
    assert!(after[1] - from[1] > 10.0, "the origin 30 mm off the hinge was asked 20 mm up and went {:.3} mm", after[1] - from[1]);
}

struct Crank {
    project: Project,
    crank: Id,
    rod: Id,
    slider: Id,
}

const CRANK_ARM: f64 = 30.0;
const ROD: f64 = 100.0;

fn slider_crank() -> Crank {
    let mut p = Project::default();
    p.new_document();
    let root = p.root;
    p.set_active_component(Some(root));
    let ids: Vec<Id> = (0..4).map(|i| p.add_part(format!("P{i}"))).collect();
    p.set_grounded(ids[0], true);
    let mut bodies = Vec::new();
    for (k, c) in ids.iter().enumerate() {
        p.set_active_component(Some(*c));
        bodies.push(p.add_extrude(k as Id + 1, 5.0));
    }
    let up = [0.0, 0.0, 1.0];
    let face = |centroid: [f64; 3]| FaceKey { index: 0, centroid, normal: up, id: 0 };
    let ground_origin = p.add_connector(ids[0], AnchorRef::Origin);
    let crank_origin = p.add_connector(ids[1], AnchorRef::Origin);
    p.add_joint(ground_origin, crank_origin, JointKind::Revolute);
    let crank_pin = p.add_connector(ids[1], AnchorRef::FaceCenter(bodies[1], face([CRANK_ARM, 0.0, 0.0])));
    let rod_origin = p.add_connector(ids[2], AnchorRef::Origin);
    p.add_joint(crank_pin, rod_origin, JointKind::Revolute);
    let rod_pin = p.add_connector(ids[2], AnchorRef::FaceCenter(bodies[2], face([ROD, 0.0, 0.0])));
    let slider_origin = p.add_connector(ids[3], AnchorRef::Origin);
    p.add_joint(rod_pin, slider_origin, JointKind::Revolute);
    let line_on_ground = p.add_connector(ids[0], AnchorRef::FaceCenter(bodies[0], face([0.0; 3])));
    let line_on_slider = p.add_connector(ids[3], AnchorRef::Origin);
    for c in [line_on_ground, line_on_slider] {
        p.connectors.iter_mut().find(|k| k.id == c).unwrap().axis_ref = Some(AnchorRef::BasePlane(BasePlane::YZ));
    }
    p.add_joint(line_on_ground, line_on_slider, JointKind::Slider);
    p.solve_joints();
    Crank { project: p, crank: ids[1], rod: ids[2], slider: ids[3] }
}

fn closure_gap(c: &Crank) -> f64 {
    let p = &c.project;
    let crank_pin = world(p, c.crank, [CRANK_ARM, 0.0, 0.0]);
    let rod_pin = world(p, c.rod, [ROD, 0.0, 0.0]);
    let slider = world(p, c.slider, [0.0; 3]);
    distance(Gap { from: crank_pin, to: world(p, c.rod, [0.0; 3]) }).max(distance(Gap { from: rod_pin, to: slider })).max(slider[1].abs()).max(slider[2].abs())
}

#[test]
fn a_crank_pin_led_round_its_circle_takes_the_whole_mechanism_with_it() {
    let mut c = slider_crank();
    let mut worst_miss = 0.0f64;
    let mut worst_gap = 0.0f64;
    let mut cursor = 5.0f64;
    while cursor <= 725.0 {
        let t = cursor.to_radians();
        let target = [CRANK_ARM * t.cos(), CRANK_ARM * t.sin(), 0.0];
        pull(&mut c.project, c.crank, Lead { grab: [CRANK_ARM, 0.0, 0.0], cursor: target });
        worst_miss = worst_miss.max(distance(Gap { from: world(&c.project, c.crank, [CRANK_ARM, 0.0, 0.0]), to: target }));
        worst_gap = worst_gap.max(closure_gap(&c));
        cursor += 5.0;
    }
    assert!(worst_miss < 1.0, "two turns of the cursor round the crank circle: the pin fell as far as {worst_miss:.3} mm behind it");
    assert!(worst_gap < 1e-6, "the loop came apart while being dragged: {worst_gap:.3e} mm");
    assert!(!c.project.mates_conflict, "the drag left the document in conflict");
}

#[test]
fn a_crank_pin_asked_off_its_circle_goes_as_near_as_the_hinge_lets_it() {
    let mut c = slider_crank();
    let mut worst_gap = 0.0f64;
    let mut cursor = 5.0f64;
    while cursor <= 365.0 {
        let t = cursor.to_radians();
        let target = [60.0 * t.cos(), 60.0 * t.sin(), 0.0];
        pull(&mut c.project, c.crank, Lead { grab: [CRANK_ARM, 0.0, 0.0], cursor: target });
        worst_gap = worst_gap.max(closure_gap(&c));
        let pin = world(&c.project, c.crank, [CRANK_ARM, 0.0, 0.0]);
        let radius = (pin[0].powi(2) + pin[1].powi(2)).sqrt();
        assert!((radius - CRANK_ARM).abs() < 1e-6, "the pin left its circle: radius {radius:.6}");
        let gap = (pin[1].atan2(pin[0]).to_degrees() - cursor + 540.0).rem_euclid(360.0) - 180.0;
        assert!(gap.abs() < 3.0, "the cursor stands at {cursor} deg and the pin {gap:.2} deg off it");
        cursor += 5.0;
    }
    assert!(worst_gap < 1e-6, "the loop came apart: {worst_gap:.3e} mm");
}
