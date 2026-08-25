//! Bridge: document to solver problem and component poses back again.
//!
//! Here and only here do the two models meet: the document one (components, connectors with
//! `AnchorRef`, mate kinds from `feature`) and the solver one (bodies, anchor frames, primitives). The
//! bridge exists so that the solver knows nothing about the document and the document nothing about
//! numerical methods.
//!
//! The bridge decides nothing by itself. Decisions taken in passing — pre-aligning bodies by
//! heuristics, choosing a mating side, skipping constraints with broken references — are invisible and
//! therefore unfixable. Here a translation is only a translation.

use nalgebra::{Isometry3, Matrix3, Rotation3, Translation3, UnitQuaternion, Vector3};

use super::decompose::{solve_assembly, AssemblyReport};
use super::frame::Anchor;
use super::joint::{Joint as AsmJoint, JointKind as AsmKind};
use super::problem::{Body, Problem};
use crate::feature::Side;
use crate::model::{Id, Project};

/// Convert a 3x4 document matrix into a solver pose.
///
/// A document matrix may be slightly non-orthogonal from accumulated edits, so the rotation is
/// extracted through the nearest orthogonal matrix rather than taken as is: otherwise the quaternion
/// comes out unnormalised and the solver works on distorted geometry.
pub fn pose_from12(m: &[f64; 12]) -> Isometry3<f64> {
    let r = Matrix3::new(m[0], m[1], m[2], m[4], m[5], m[6], m[8], m[9], m[10]);
    let rot = Rotation3::from_matrix_eps(&r, 1e-12, 100, Rotation3::identity());
    Isometry3::from_parts(Translation3::new(m[3], m[7], m[11]), UnitQuaternion::from_rotation_matrix(&rot))
}

/// The reverse conversion: a solver pose into a 3x4 document matrix.
pub fn pose_to12(t: &Isometry3<f64>) -> [f64; 12] {
    let r = t.rotation.to_rotation_matrix();
    let m = r.matrix();
    let p = t.translation.vector;
    [m[(0, 0)], m[(0, 1)], m[(0, 2)], p.x, m[(1, 0)], m[(1, 1)], m[(1, 2)], p.y, m[(2, 0)], m[(2, 1)], m[(2, 2)], p.z]
}

/// Document mate kind to solver kind. One to one: there is a single set of kinds.
pub fn kind_of(k: crate::feature::JointKind) -> AsmKind {
    use crate::feature::JointKind as K;
    match k {
        K::Rigid => AsmKind::Rigid,
        K::Revolute => AsmKind::Revolute,
        K::Slider => AsmKind::Slider,
        K::Cylindrical => AsmKind::Cylindrical,
        K::Planar => AsmKind::Planar,
        K::Ball => AsmKind::Ball,
        K::PinSlot => AsmKind::PinSlot,
        K::Parallel => AsmKind::Parallel,
    }
}

/// Build a solver problem from a document.
///
/// Returns the problem, the components in body order, and the mates that entered the problem in
/// constraint order, so that a report can be mapped back to mate numbers.
pub fn problem_of(project: &Project) -> Option<(Problem, Vec<Id>, Vec<Id>)> {
    problem_of_with_pins(project, &std::collections::HashMap::new())
}

/// The same, with pins: degrees that hit their limit and are held as driven for this pass. See
/// `solve_project` for why pins exist and why there are several passes.
pub fn problem_of_with_pins(project: &Project, pins: &std::collections::HashMap<(Id, usize), f64>) -> Option<(Problem, Vec<Id>, Vec<Id>)> {
    if project.joints.is_empty() && project.mate_constraints.iter().all(|g| g.members.len() < 2 && g.anchors.len() < 3 && g.faces.len() < 2) {
        return None;
    }
    // Components taking part in mates.
    let mut comps: Vec<Id> = Vec::new();
    let mut index: std::collections::HashMap<Id, usize> = std::collections::HashMap::new();
    let push = |c: Id, comps: &mut Vec<Id>, index: &mut std::collections::HashMap<Id, usize>| {
        if !index.contains_key(&c) {
            index.insert(c, comps.len());
            comps.push(c);
        }
    };
    // A faulty mate does not enter the problem — any fault, not only a lost connector.
    //
    // Filtering on a single fault kind, "the mate has no connector", lets a mate whose anchor sits on a
    // moving part reach the solver as healthy while the list already calls it broken, and that mate
    // tears the assembly apart. Naming a fault and still obeying it is the worst combination: the
    // explanation and the breakage are on screen at the same time.
    //
    // Measured on a real machine document: with such a mate every recompute moved a body about 60 mm,
    // endlessly (71.3, 68.1, 64.1, 61.0); without it the document settles in one solve and then stands
    // still, 0.0000 mm over five consecutive solves.
    //
    // Only permanent breakage counts. "Anchor not found" does not mean a broken document but geometry
    // that has not been raised yet: live B-rep for imports is restored on demand, and immediately after
    // opening no body has any edges (measured: 0 edges, live B-rep on 0 of 138 bodies). Dropping those
    // mates would mean solving the assembly differently before and after background work arrives, and
    // bodies settle in the wrong place and stay there.
    let broken: Vec<Id> = project.joint_faults().into_iter().filter(|(_, why)| *why != "j-fault-anchor-lost").map(|(id, _)| id).collect();
    let mut usable: Vec<(&crate::feature::Joint, Id, Id)> = Vec::new();
    for j in &project.joints {
        if broken.contains(&j.id) {
            continue; // Nothing to hold with; the report and the mate list say so.
        }
        let (Some(oa), Some(ob)) = (project.connector(j.a).map(|c| c.owner), project.connector(j.b).map(|c| c.owner)) else {
            continue; // The connector is lost, so the mate does not act and the report shows it.
        };
        push(oa, &mut comps, &mut index);
        push(ob, &mut comps, &mut index);
        usable.push((j, oa, ob));
    }
    // Constraints take part in the problem too. A group names components directly, having no connectors
    // at all, while a width constraint names the owners of its anchors.
    for g in &project.mate_constraints {
        for &m in &g.members {
            push(m, &mut comps, &mut index);
        }
        for &c in &g.anchors {
            if let Some(o) = project.connector(c).map(|x| x.owner) {
                push(o, &mut comps, &mut index);
            }
        }
        for (o, _) in &g.faces {
            push(*o, &mut comps, &mut index);
        }
    }
    if comps.is_empty() {
        return None;
    }

    let bodies: Vec<Body> = comps
        .iter()
        .map(|&c| {
            let pose = pose_from12(&project.world_transform(c));
            if project.is_grounded(c) {
                Body::grounded(pose)
            } else {
                Body::new(pose)
            }
        })
        .collect();

    let mut problem = Problem::new(bodies);
    let mut joint_of_constraint: Vec<Id> = Vec::new();
    // Anchors of each mate, used by relations between mates.
    let mut anchors_of_joint: std::collections::HashMap<Id, (Anchor, Anchor, AsmKind)> = std::collections::HashMap::new();
    for (j, oa, ob) in usable {
        // The frame depends on the mate kind: for a slider on a flat face the travel axis lies in the
        // plane, not along the normal (see `connector_frame_for_kind`). Otherwise a slider placed on the
        // face of a rail carries the body away from the rail.
        //
        // One entry point for anchor frames: the solver, the displayed value and the "hold as built"
        // declaration must all see the same axes. A separate computation here drifts apart from the
        // displayed value.
        let Some((fa, fb)) = joint_local_frames(project, j) else { continue };
        // A document connector frame is expressed in the local space of its owner, which is exactly what
        // an anchor needs. Removing a world transform from it would apply the correction twice and move
        // the body twice; this is covered by document tests.
        //
        // Roll counts as defined only when the secondary axis came from part geometry. An anchor on a
        // face without reachable geometry (body not built, synthetic key) takes its axis from world
        // coordinates; pinning roll to that axis settles the body into an arbitrary rotation.
        let a = Anchor { body: index[&oa], local: pose_from12(&fa), roll_known: roll_is_geometric(project, j.a) };
        let b = Anchor { body: index[&ob], local: pose_from12(&fb), roll_known: roll_is_geometric(project, j.b) };

        let mut aj = AsmJoint::new(a, b, kind_of(j.kind));
        // The mating side is picked as the nearer of the two, and the stored flag flips it.
        //
        // Aligning axes admits two sides, along and against. Choosing between them must not be
        // arbitrary, or the body appears to flip on its own; obeying the stored side blindly is just as
        // wrong, because it is set automatically and a mistake turns the body by 180 degrees for no
        // visible reason. The rule is the usual one: never move a body more than necessary, so take the
        // side closer to where it stands now.
        let side = joint_side_now(project, j);
        aj = aj.with_side(side);
        // Joint values drive a body only where they carry meaning.
        //
        // A value counts as specified only when it was set explicitly. Using `!= 0.0` as that marker
        // makes zero indistinguishable from unset: an angle of 0 degrees cannot be requested, and any
        // reading the solver wrote back turns into a requirement on the next solve.
        //
        // While the pointer leads a body, specified values follow it instead of holding it. Dragging is
        // not an argument with a number: the number has to follow, and after the drag it is stored at
        // the value actually reached (`write_back_free_values`). Measured on a machine document: three
        // sliders, all with specified values (-280, 7, 100), leave the problem with zero degrees of
        // freedom, so pulling a body moves nothing at all.
        //
        // Limits still apply: a pin (`pins`) is not a requested value but a boundary the body is not
        // allowed past, by pointer or by number.
        let hand_leads = project.drag_pull.is_some();
        let specified = |slot: usize| if hand_leads { None } else { j.driven(slot) };
        if let Some(v) = specified(0).or_else(|| pins.get(&(j.id, 0)).copied()) {
            aj = aj.with_angle(v);
        }
        if let Some(v) = specified(1).or_else(|| pins.get(&(j.id, 1)).copied()) {
            aj = aj.with_offset(v);
        }
        if let Some(v) = specified(2).or_else(|| pins.get(&(j.id, 2)).copied()) {
            aj = aj.with_offset2(v);
        }
        for c in aj.primitives() {
            problem.add(c);
            joint_of_constraint.push(j.id);
        }
        // A relation measures in the same frame the mate holds in.
        //
        // Storing the original anchor here, without the flip, disagrees with the mate itself, which is
        // built on the flipped one (the mating side is picked as the nearer, see `joint_flip_now`).
        // Measured: a linear relation with ratio 2 moved the driven slot by -20 along its own arrow for
        // 10 mm of driver travel, so the mechanism ran backwards while the numbers agreed.
        let mut a_eff = a;
        a_eff.local *= side_turn(side);
        anchors_of_joint.insert(j.id, (a_eff, b, kind_of(j.kind)));
    }
    // A pointer drag enters the same problem as the mates.
    //
    // The goal travels together with the mates, so every free degree is resolved in one pass and the
    // mechanism moves as a chain. Solving in two steps, mates first and drag second, costs exactly one
    // frame of lag.
    //
    // The goal contributes no equations (`rows()` = 0); the null-space step in `iterate.rs` does its
    // work. It is added here so that splitting the problem into independent parts carries the goal into
    // the part that holds the mates of the grabbed body.
    if let Some((comp, local, to)) = project.drag_pull {
        if let Some(&bi) = index.get(&comp) {
            let anchor = Anchor {
                body: bi,
                local: Isometry3::from_parts(Translation3::new(local[0], local[1], local[2]), UnitQuaternion::identity()),
                roll_known: false,
            };
            problem.add(super::problem::Constraint::Pull { a: anchor, to: Vector3::new(to[0], to[1], to[2]) });
            joint_of_constraint.push(0); // The goal belongs to no mate, so the report has nothing to attribute it to.
        }
    }
    // Relations between mates: one equation per pair of degrees.
    //
    // The anchors are the same ones the solver builds the mates from, without the specified value baked
    // in — that value belongs to the mate, not to the measurement. A mate that never entered the problem
    // carries no relation: there is nothing to measure, and `joint_faults` says so instead of a made-up
    // number.
    for r in &project.relations {
        let (Some(&(aa, ab, ka)), Some(&(ba, bb, kb))) = (anchors_of_joint.get(&r.a), anchors_of_joint.get(&r.b)) else { continue };
        let (Some((m_a, sa, rot_a)), Some((m_b, sb, rot_b))) = (slot_measure(ka, r.slot_a, aa, ab), slot_measure(kb, r.slot_b, ba, bb)) else { continue };
        let (Some(k), true) = (relation_coefficient(r, rot_a, rot_b), rot_a == r.kind.slots_are_rotations().0 && rot_b == r.kind.slots_are_rotations().1) else {
            continue; // The slot is not of the kind this relation requires; `relation_faults` reports it.
        };
        // Signs fold into the coefficient instead of correcting the measurement.
        //
        // The requirement is `s_b*m_b = k*s_a*m_a + phase`, where `s` is the sign of the measurement
        // (negative for a pin-slot, see `slot_measure`). Multiplying both sides by `s_b` gives exactly
        // what the primitive supports: `m_b = (k*s_a*s_b)*m_a + phase*s_b`. The phase is taken in the
        // same units `relation_phase` computes it in, via `measured_slot`, so it already carries its
        // sign.
        problem.add(super::problem::Constraint::slot_ratio(m_b, m_a, k * sa * sb, r.phase * sb));
        joint_of_constraint.push(r.id);
    }
    // A group fixes bodies relative to each other where they stand now.
    //
    // Each anchor is the body's own origin, and the mutual placement is read from the document at the
    // moment the problem is assembled. No frozen transform is stored alongside: it would drift away from
    // the component placements silently.
    for g in &project.mate_constraints {
        // Cylinder against plane: the shaft lies on the plane.
        //
        // Two conditions at once, both expressed with existing primitives: the axis is parallel to the
        // plane (`Angle` of 90 degrees between normal and axis, otherwise the cylinder cuts through the
        // plane) and the axis-to-plane distance equals the radius (`OnPlane` offset by the radius).
        //
        // This is the one mate that uses no connectors at all: the anchors are built directly from the
        // selected surfaces.
        if g.kind == crate::feature::ConstraintKind::Tangent {
            let [Some((oa, ra)), Some((ob, rb))] = [0usize, 1].map(|k| g.faces.get(k).cloned()) else { continue };
            let (Some(&ba), Some(&bb)) = (index.get(&oa), index.get(&ob)) else { continue };
            // Which of the two surfaces is the cylinder and which is the plane is asked of the geometry.
            let cyl_of = |owner: Id, r: &crate::feature::AnchorRef| match r {
                crate::feature::AnchorRef::FaceCenter(body, key) => project.face_cylinder(*body, key).map(|c| (owner, c)),
                _ => None,
            };
            let plane_of = |owner: Id, r: &crate::feature::AnchorRef| match r {
                crate::feature::AnchorRef::FaceCenter(body, key) if project.face_cylinder(*body, key).is_none() => Some((owner, project.resolve_face(*body, key))),
                _ => None,
            };
            // Cylinder against cylinder: they touch when the axis distance equals the sum of the radii
            // (outside) or their difference (one inside the other).
            //
            // The side is picked as the nearer one, like the mating side of a joint: both are valid, and
            // dragging a shaft into a bushing when it lies beside it would move the body somewhere it was
            // never asked to go. Axis direction follows the same rule.
            if let (Some((_, (o1, ax1, r1))), Some((_, (o2, ax2, r2)))) = (cyl_of(oa, &ra), cyl_of(ob, &rb)) {
                let v = |p: [f64; 3]| Vector3::new(p[0], p[1], p[2]);
                let (w1, w2) = (pose_from12(&project.world_transform(oa)), pose_from12(&project.world_transform(ob)));
                // A point must be translated, not only rotated: `Isometry3 * Vector3` in nalgebra rotates
                // the vector and adds no translation. With that mistake the current axis distance comes out
                // as zero and the side is always chosen as "inside".
                let (wo1, wo2) = (w1 * nalgebra::Point3::from(v(o1)), w2 * nalgebra::Point3::from(v(o2)));
                let (wa1, wa2) = (w1.rotation * v(ax1), w2.rotation * v(ax2));
                let d = wo2 - wo1;
                let now = (d - wa1 * wa1.dot(&d)).norm();
                let (outside, inside) = (r1 + r2, (r1 - r2).abs());
                let dist = if (now - inside).abs() < (now - outside).abs() { inside } else { outside };
                // Axes are held with `AxisAligned`, not with `Angle` at zero. The derivative of a dot
                // product vanishes at the solution itself (r = cos t - 1 ~ -t^2/2), leaving the solver
                // nothing to straighten a tilt with: measured, shafts settled at a 24 degree tilt while the
                // residual stayed near zero. The full axis difference does not degenerate that way, as the
                // primitive itself documents.
                let mut la = pose_from12(&crate::feature::PlaneFrame::from_origin_normal(o1, ax1, 0.0).matrix12());
                if wa1.dot(&wa2) < 0.0 {
                    la *= UnitQuaternion::from_axis_angle(&Vector3::x_axis(), std::f64::consts::PI);
                }
                let f1 = Anchor { body: ba, local: la, roll_known: false };
                let f2 = Anchor { body: bb, local: pose_from12(&crate::feature::PlaneFrame::from_origin_normal(o2, ax2, 0.0).matrix12()), roll_known: false };
                problem.add(super::problem::Constraint::AxisAligned { a: f1, b: f2 });
                joint_of_constraint.push(g.id);
                problem.add(super::problem::Constraint::AxisDistance { a: f1, b: f2, dist });
                joint_of_constraint.push(g.id);
                continue;
            }
            // Sphere against plane: the centre stands exactly one radius above the plane.
            //
            // The side is taken from the current placement: the ball sits either above or below, and
            // moving it to the other side of the plane was never requested.
            let sph_of = |owner: Id, r: &crate::feature::AnchorRef| match r {
                crate::feature::AnchorRef::FaceCenter(body, key) => project.face_sphere(*body, key).map(|c| (owner, c)),
                _ => None,
            };
            // Sphere against sphere: they touch when the centre distance equals the sum of the radii
            // (outside) or their difference (one inside the other). The side is the nearer one, as for
            // cylinders.
            if let (Some((_, (c1, r1))), Some((_, (c2, r2)))) = (sph_of(oa, &ra), sph_of(ob, &rb)) {
                let v = |p: [f64; 3]| Vector3::new(p[0], p[1], p[2]);
                let (w1, w2) = (pose_from12(&project.world_transform(oa)), pose_from12(&project.world_transform(ob)));
                let (wc1, wc2) = (w1 * nalgebra::Point3::from(v(c1)), w2 * nalgebra::Point3::from(v(c2)));
                let now = (wc2 - wc1).norm();
                let (outside, inside) = (r1 + r2, (r1 - r2).abs());
                let dist = if (now - inside).abs() < (now - outside).abs() { inside } else { outside };
                let f1 = Anchor { body: ba, local: pose_from12(&crate::feature::PlaneFrame::from_origin_normal(c1, [0.0, 0.0, 1.0], 0.0).matrix12()), roll_known: false };
                let f2 = Anchor { body: bb, local: pose_from12(&crate::feature::PlaneFrame::from_origin_normal(c2, [0.0, 0.0, 1.0], 0.0).matrix12()), roll_known: false };
                problem.add(super::problem::Constraint::PointDistance { a: f1, b: f2, dist });
                joint_of_constraint.push(g.id);
                continue;
            }
            let ball = sph_of(oa, &ra).zip(plane_of(ob, &rb)).map(|(s, p)| (s, p, true)).or_else(|| sph_of(ob, &rb).zip(plane_of(oa, &ra)).map(|(s, p)| (s, p, false)));
            if let Some(((_, (ctr, radius)), (_, (po, pn)), sphere_is_a)) = ball {
                let (sb, pb_) = if sphere_is_a { (ba, bb) } else { (bb, ba) };
                let (so, po_) = if sphere_is_a { (oa, ob) } else { (ob, oa) };
                let v = |p: [f64; 3]| Vector3::new(p[0], p[1], p[2]);
                let wc = pose_from12(&project.world_transform(so)) * nalgebra::Point3::from(v(ctr));
                let wp = pose_from12(&project.world_transform(po_));
                let (wo, wn) = (wp * nalgebra::Point3::from(v(po)), wp.rotation * v(pn));
                let side = if wn.dot(&(wc - wo)) < 0.0 { -radius } else { radius };
                let pl = Anchor { body: pb_, local: pose_from12(&crate::feature::PlaneFrame::from_origin_normal(po, pn, 0.0).matrix12()), roll_known: false };
                let ce = Anchor { body: sb, local: pose_from12(&crate::feature::PlaneFrame::from_origin_normal(ctr, [0.0, 0.0, 1.0], 0.0).matrix12()), roll_known: false };
                problem.add(super::problem::Constraint::OnPlane { a: pl, b: ce, offset: side });
                joint_of_constraint.push(g.id);
                continue;
            }
            let pair = cyl_of(oa, &ra).zip(plane_of(ob, &rb)).or_else(|| cyl_of(ob, &rb).zip(plane_of(oa, &ra)));
            let Some(((cyl_owner, (co, cax, radius)), (pl_owner, (po, pn)))) = pair else {
                continue; // The surface pair is not cylinder-and-plane; `constraint_faults` reports it.
            };
            let (cyl_body, pl_body) = if cyl_owner == oa { (ba, bb) } else { (bb, ba) };
            let _ = (cyl_owner, pl_owner);
            let cyl = Anchor { body: cyl_body, local: pose_from12(&crate::feature::PlaneFrame::from_origin_normal(co, cax, 0.0).matrix12()), roll_known: false };
            let pl = Anchor { body: pl_body, local: pose_from12(&crate::feature::PlaneFrame::from_origin_normal(po, pn, 0.0).matrix12()), roll_known: false };
            problem.add(super::problem::Constraint::Angle { a: pl, b: cyl, deg: 90.0 });
            joint_of_constraint.push(g.id);
            problem.add(super::problem::Constraint::OnPlane { a: pl, b: cyl, offset: radius });
            joint_of_constraint.push(g.id);
            continue;
        }
        // Width: the tab stands midway between the walls, at equal distance from both.
        //
        // Written as a single condition: the tab origin lies in the mid-plane of the walls. The mid-plane
        // is the anchor of the first wall shifted by half the span; both walls share one normal, or
        // "midway" means nothing, so the normal is taken from the first.
        if g.kind == crate::feature::ConstraintKind::Width {
            let [Some(w1), Some(w2), Some(tab)] = [0usize, 1, 2].map(|k| g.anchors.get(k).copied()) else { continue };
            let frames = [w1, w2, tab].map(|c| project.connector(c).and_then(|x| project.connector_frame(x)).map(|f| (project.connector(c).map(|x| x.owner), f)));
            let [Some((Some(o1), f1)), Some((Some(o2), f2)), Some((Some(ot), ft))] = frames else { continue };
            let (Some(&b1), Some(&b2), Some(&bt)) = (index.get(&o1), index.get(&o2), index.get(&ot)) else { continue };
            // World origins of the walls, to take half the span along the shared normal.
            let (p1, p2) = (pose_from12(&project.world_transform(o1)) * pose_from12(&f1.matrix12()), pose_from12(&project.world_transform(o2)) * pose_from12(&f2.matrix12()));
            let n = p1.rotation * Vector3::z();
            let half = n.dot(&(p2.translation.vector - p1.translation.vector)) * 0.5;
            let mid = Anchor { body: b1, local: pose_from12(&f1.matrix12()) * Translation3::new(0.0, 0.0, half), roll_known: true };
            let tab_a = Anchor { body: bt, local: pose_from12(&ft.matrix12()), roll_known: true };
            let _ = b2;
            problem.add(super::problem::Constraint::OnPlane { a: mid, b: tab_a, offset: 0.0 });
            joint_of_constraint.push(g.id);
            continue;
        }
        let members: Vec<Id> = g.members.iter().copied().filter(|m| index.contains_key(m)).collect();
        let Some(&first) = members.first() else { continue };
        if members.len() < 2 {
            continue;
        }
        let w0 = pose_from12(&project.world_transform(first));
        for &m in members.iter().skip(1) {
            let rel = pose_from12(&project.world_transform(m)).inverse() * w0;
            let a = Anchor { body: index[&first], local: Isometry3::identity(), roll_known: true };
            let b = Anchor { body: index[&m], local: rel, roll_known: true };
            for c in AsmJoint::new(a, b, AsmKind::Rigid).primitives() {
                problem.add(c);
                joint_of_constraint.push(g.id);
            }
        }
    }
    if problem.constraints.is_empty() {
        return None;
    }
    Some((problem, comps, joint_of_constraint))
}

/// Whether the secondary axis of an anchor comes from part geometry rather than from world axes.
///
/// The question is the same for every joint kind: does the anchor have a second axis of its own.
///
/// It used to carry an exception for rigid and slider joints, and that exception was a patch. Edges and
/// vertices supplied no secondary axis at all — it was derived from world Z — so roll was honestly
/// unknown and a rigid mate on a pair of edges left the body free to spin about an edge. Holding roll
/// anyway removed the degree of freedom but derived the rotation from how the body happened to lie in
/// the world instead of from its own shape. The cause is gone: an edge now carries the normal of its
/// adjacent face (`MeshEdge::ref_dir`) and a vertex carries the axis of its edge.
fn roll_is_geometric(project: &Project, conn: Id) -> bool {
    let Some(c) = project.connector(conn) else { return false };
    match &c.anchor {
        // Face: usable when its principal direction or cylinder axis could be taken.
        crate::feature::AnchorRef::FaceCenter(b, k) => project.face_principal_dir(*b, k).is_some() || project.face_axis(*b, k).is_some(),
        // Component base plane: its axes are defined by the component itself, which is its geometry.
        crate::feature::AnchorRef::BasePlane(_) | crate::feature::AnchorRef::Origin => true,
        // Edge and vertex: a second axis exists exactly when the edge found an adjacent face.
        crate::feature::AnchorRef::EdgeMid(b, e) | crate::feature::AnchorRef::Vertex(b, e, _) => project.edge_ref_dir(*b, *e).is_some(),
    }
}

/// Solve the assembly of a document and write the resulting placements back.
pub fn solve_project(project: &mut Project) -> Option<AssemblyReport> {
    // The mating side is decided once, before solving, and stored in the mate itself. After that only an
    // explicit flag changes it: a guess recomputed on every solve rocks the body between two solutions
    // (measured on a machine document: 300 mm back and forth).
    let undecided: Vec<(Id, Side)> = project
        .joints
        .iter()
        .filter(|j| !j.flip_decided)
        .map(|j| (j.id, joint_side_now(project, j)))
        .collect();
    for (id, side) in undecided {
        if let Some(j) = project.joints.iter_mut().find(|x| x.id == id) {
            (j.flip, j.roll_flip) = (side.flip, side.roll_flip);
            j.flip_decided = true;
        }
    }
    // Limits on free degrees are handled by an active set.
    //
    // A limit is an inequality while the solver works with equalities. The standard treatment: solve
    // without limits, see who left the allowed range, pin the offenders at the boundary as specified
    // values and solve again. Once nobody leaves the range, that solution is the answer.
    //
    // Without this, limits do nothing for free degrees and only clamp an explicitly specified value: a
    // slider with a range of 0..50 mm can be dragged to 300, so the interface offers a limit the mate
    // never enforces.
    let mut pins: std::collections::HashMap<(Id, usize), f64> = std::collections::HashMap::new();
    let (mut problem, mut comps, mut joint_of) = problem_of(project)?;
    let (mut poses, mut report) = solve_assembly(&problem);
    for _ in 0..4 {
        let index: std::collections::HashMap<Id, usize> = comps.iter().enumerate().map(|(i, &c)| (c, i)).collect();
        let mut added = false;
        for j in &project.joints {
            let Some((wa, wb)) = joint_frames(project, j, &index, &poses) else { continue };
            let kind = kind_of(j.kind);
            for slot in 0..3 {
                if j.driven(slot).is_some() || pins.contains_key(&(j.id, slot)) {
                    continue; // Already held, so there is nothing to pin again.
                }
                let Some(v) = measured_slot(kind, slot, &wa, &wb) else { continue };
                let bound = match (j.limit_min[slot], j.limit_max[slot]) {
                    (Some(lo), _) if v < lo - 1e-9 => Some(lo),
                    (_, Some(hi)) if v > hi + 1e-9 => Some(hi),
                    _ => None,
                };
                if let Some(bound) = bound {
                    pins.insert((j.id, slot), bound);
                    added = true;
                }
            }
        }
        if !added {
            break;
        }
        let Some(next) = problem_of_with_pins(project, &pins) else { break };
        (problem, comps, joint_of) = next;
        let solved = solve_assembly(&problem);
        poses = solved.0;
        report = solved.1;
    }
    let _ = &problem;

    // Translate constraint indices into document mate ids: the interface speaks in mates, not in the
    // primitives a mate decomposed into.
    report.redundant = dedup(report.redundant.iter().filter_map(|&i| joint_of.get(i).copied()).collect());
    report.violated = report.violated.iter().filter_map(|&(i, v)| joint_of.get(i).map(|&jid| (jid as usize, v))).collect();

    // A part that did not converge is not moved.
    //
    // Contradictory mates have no solution: the minimum of the residual sum is not zero but a compromise
    // that satisfies no requirement. Writing it back moves bodies where nobody asked them to go, which
    // reads as the assembly blowing apart. Measured with two rigid mates to different grounded anchors:
    // the body travelled to exactly the midpoint between them.
    //
    // The rule is that without a solution a body stays where it was placed and the mates are flagged;
    // resolving the contradiction is a decision for the author, not for the solver. Free-degree readings
    // are not written either — they would describe a placement the body does not have.
    //
    // The refusal is per part, not per document. Refusing wholesale lets one faulty mate freeze
    // mechanisms that have nothing to do with it: measured on a scenario document, a slider travelled
    // 0.000 mm instead of 15.000 and a wheel 0.000 degrees instead of 40.000 while an unrelated mate was
    // violated elsewhere. Bodies of the non-converged parts are listed in `report.stuck` and stay put
    // below; the rest are placed.
    if !report.converged && report.stuck.len() >= comps.len() {
        return Some(report); // Nothing converged, so there is nothing to move.
    }

    // The value of a free joint reflects reality. A hinge can be asked for its angle and a slider for its
    // travel; a value that was never specified has to show where the body ended up, or the mate lies
    // about its own state.
    write_back_free_values(project, &comps, &poses);

    // Parents are written before children, or a nested body travels twice with its subassembly.
    //
    // A component placement is stored relative to its parent while the solver works in world space, and
    // the conversion uses the parent's current placement. Writing a child first converts it against the
    // old parent, and then the parent moves and drags the child along. The order of the component list is
    // arbitrary (built from first appearance in the mates), so happening to get it right is no defence:
    // measured 105 mm instead of 65.
    let mut order: Vec<usize> = (0..comps.len()).collect();
    order.sort_by_key(|&i| project.component_depth(comps[i]));
    for i in order {
        let c = comps[i];
        if project.is_grounded(c) || report.stuck.contains(&i) {
            continue; // A grounded component never moves; one in a non-converged part waits for the conflict to be resolved.
        }
        let parent = project.components.iter().find(|x| x.id == c).and_then(|x| x.parent);
        let wp = parent.map(|p| pose_from12(&project.world_transform(p))).unwrap_or_else(Isometry3::identity);
        project.set_component_transform(c, pose_to12(&(wp.inverse() * poses[i])));
    }
    Some(report)
}

/// Measured value of a slot: where the body stands along this degree of freedom.
///
/// The axis and the kind of motion come from one table (`slot_axis`), the same one a specified value
/// uses to pin the degree. Measuring against the main axis only leaves planar, ball and pin-slot joints
/// showing nothing: the interface displays a permanent zero whatever the body does.
pub fn measured_slot(kind: AsmKind, slot: usize, wa: &Isometry3<f64>, wb: &Isometry3<f64>) -> Option<f64> {
    let (axis, is_rotation) = super::joint::slot_axis(kind, slot)?;
    let dir = |q: &nalgebra::UnitQuaternion<f64>| match axis {
        0 => q * Vector3::x(),
        1 => q * Vector3::y(),
        _ => q * Vector3::z(),
    };
    // For a pin-slot the travel direction belongs to the second anchor: the slot is its feature, so
    // travel is measured along its axis. Measuring along the first anchor makes the mate misreport its own
    // state — the body slid seven along the slot while the field shows zero.
    let along_b = matches!(kind, AsmKind::PinSlot) && slot == 1;
    let n = dir(if along_b { &wb.rotation } else { &wa.rotation });
    if !is_rotation {
        return Some(n.dot(&(wb.translation.vector - wa.translation.vector)));
    }
    // Angle about axis `n`: take any anchor axis perpendicular to it and measure the signed rotation.
    let perp = |q: &nalgebra::UnitQuaternion<f64>| match axis {
        0 => q * Vector3::y(),
        1 => q * Vector3::z(),
        _ => q * Vector3::x(),
    };
    let (pa, pb) = (perp(&wa.rotation), perp(&wb.rotation));
    let s = n.dot(&pa.cross(&pb));
    Some(s.atan2(pa.dot(&pb)).to_degrees())
}

/// Slot measurement of a joint for use by a relation: `(measure, sign, is_rotation)`.
///
/// The sign exists because of the pin-slot. Its travel is measured along the axis of the second anchor,
/// since the slot belongs to that anchor, while `SlotMeasure` always takes the axis from the first.
/// Moving the measurement onto the second anchor is only possible by swapping the anchors, and that
/// flips the difference of origins, producing exactly the negated value. The sign is returned to the
/// caller rather than hidden inside the measurement, where the next joint kind would forget it.
fn slot_measure(kind: AsmKind, slot: usize, a: Anchor, b: Anchor) -> Option<(super::problem::SlotMeasure, f64, bool)> {
    let (axis, rotation) = super::joint::slot_axis(kind, slot)?;
    if matches!(kind, AsmKind::PinSlot) && slot == 1 {
        return Some((super::problem::SlotMeasure { a: b, b: a, axis, rotation }, -1.0, rotation));
    }
    Some((super::problem::SlotMeasure { a, b, axis, rotation }, 1.0, rotation))
}

/// Relation coefficient in solver units: how much faster the second degree runs than the first.
///
/// The number stored in the document means different things for different relation kinds, and it is
/// converted here rather than in the solver:
///
/// * both degrees of the same kind (gear, linear) — the number is dimensionless and passes through;
/// * first a rotation, second a travel (rack, screw) — the number is travel per revolution, that is,
///   millimetres per 2*pi radians.
///
/// The reverse order (travel first, rotation second) does not occur: the relation kind states which slot
/// is which (`slots_are_rotations`), and `relation_faults` rejects a disagreeing selection.
pub fn relation_coefficient(r: &crate::feature::MateRelation, rot_a: bool, rot_b: bool) -> Option<f64> {
    let v = if r.reversed { -r.value } else { r.value };
    match (rot_a, rot_b) {
        (true, true) | (false, false) => Some(v),
        (true, false) => Some(v / std::f64::consts::TAU),
        (false, true) => None,
    }
}

/// Local frames of both joint anchors, built for the joint kind — the same frames the solver uses.
///
/// The kind matters: the bare connector frame (`connector_matrix`) disagrees with the solver frame for a
/// slider on a flat face, where the main axis of the face is its normal while the body travels along the
/// face (`connector_frame_for_kind`). With the bare frame the mate holds travel along the rail while the
/// field displays travel across it, and the limit, computed from the same reading, constrains something
/// other than what moves.
pub fn joint_local_frames(project: &Project, j: &crate::feature::Joint) -> Option<([f64; 12], [f64; 12])> {
    let frame_of = |cid: Id| project.connector(cid).and_then(|c| project.connector_frame_for_kind(c, j.kind)).map(|f| f.matrix12());
    let (fa, fb) = (frame_of(j.a)?, frame_of(j.b)?);
    // "Hold as built" is baked into the first anchor, so the mate treats the declared placement as its
    // zero: it holds that placement and counts its free degrees from it. Doing it here rather than in the
    // solver keeps the displayed value, the limit and the relation on the same zero.
    match j.as_built {
        None => Some((fa, fb)),
        Some(rel) => Some((pose_to12(&(pose_from12(&fa) * pose_from12(&rel))), fb)),
    }
}

/// The mating side the solver will actually apply.
///
/// There is exactly one answer and it lives in the mate: `flip` is the side and `flip_decided` says it
/// has been chosen. When it has not, the nearer side to the current placement is chosen: main axes of
/// the anchors facing each other mean the first has to be turned so the body stays where it stands.
///
/// The field must not double as a manual toggle. Reading it as `(za.zb < 0) != j.flip` gives it two
/// meanings at once; that is harmless while the side is chosen exactly once on a fresh mate (where the
/// field is still `false`), but every re-evaluation — a changed face, swapped roles, a changed kind —
/// then reads the previous decision as a request to flip and answers inside out, tumbling the body by
/// 180 degrees. Measured on that path: exactly 180.000 degrees.
///
/// An explicit flip has its own entry point, `Project::flip_joint_side`: it stores the opposite of the
/// current side and marks it decided so the automatic choice does not immediately undo it.
///
/// It became a separate entry point because the measurement did not know about the flip. Measured: a
/// cylindrical mate with a specified angle of 30 degrees displayed -30, because the solver turned the
/// anchor while the reading was computed on the unturned one. The limit and the relation use that same
/// reading.
pub fn joint_flip_now(project: &Project, j: &crate::feature::Joint) -> bool {
    joint_side_now(project, j).flip
}

/// Second half of the side: whether the secondary axis of the first anchor is turned.
pub fn joint_roll_flip_now(project: &Project, j: &crate::feature::Joint) -> bool {
    joint_side_now(project, j).roll_flip
}

/// The mating side in full: both halves at once, named.
///
/// Alignment ambiguity is not one bit but four placements. Two coordinate systems can be aligned without
/// breaking right-handedness in four ways: as they are, and by three 180-degree turns about the anchor
/// axes. All four are equally legal, so picking whichever falls out first puts the body anywhere.
///
/// The nearest to the current placement wins — the usual rule of never moving a body more than
/// necessary. The measure of "nearest" is exact here: a 180-degree turn about an axis has a diagonal
/// matrix of plus and minus ones, so the trace of `q^T * M` is just the diagonal of `M = R_a^T * R_b`
/// with the matching signs, and the largest trace means the smallest rotation.
///
/// Comparing main axes only (`za.zb < 0`) ignores the secondary axis entirely, which is what tumbles a
/// body by 180 degrees about the travel axis when a face is replaced.
pub fn joint_side_now(project: &Project, j: &crate::feature::Joint) -> Side {
    // Decided once, never guessed again. Choosing the nearer side relies on the current placement, and
    // the mate is what changes that placement: recomputing the side on every solve feeds the decision its
    // own consequence.
    if j.flip_decided {
        return j.side();
    }
    let bare = |cid: Id| -> Option<Isometry3<f64>> {
        let c = project.connector(cid)?;
        let f = project.connector_frame_for_kind(c, j.kind)?;
        Some(pose_from12(&project.world_transform(c.owner)) * pose_from12(&f.matrix12()))
    };
    let (Some(wa), Some(wb)) = (bare(j.a), bare(j.b)) else { return j.side() };
    let m = wa.rotation.to_rotation_matrix().transpose() * wb.rotation.to_rotation_matrix();
    let d = [m[(0, 0)], m[(1, 1)], m[(2, 2)]];
    // (main reversed, secondary reversed) -> diagonal signs of the turn
    let variants = [
        (Side { flip: false, roll_flip: false }, [1.0, 1.0, 1.0]),   // as is
        (Side { flip: true, roll_flip: false }, [1.0, -1.0, -1.0]),  // 180 degrees about X: main axis reversed
        (Side { flip: false, roll_flip: true }, [-1.0, -1.0, 1.0]),  // 180 degrees about Z: secondary axis reversed
        (Side { flip: true, roll_flip: true }, [-1.0, 1.0, -1.0]),   // both at once = 180 degrees about Y
    ];
    // On a tie, the variant with fewer turns wins. Ties are common rather than rare: when the anchors
    // need a 90-degree turn, "as is" and "secondary axis reversed" are equally far from the target and the
    // score cannot separate them. Resolving a tie by list order is a defect — `max_by` returns the last of
    // the equals, so a rigid mate on a pair of edges chose a turn and left adjacent faces of the bodies
    // pointing apart (measured: face B faced -Y instead of +Y). A turn has to be earned by a strict win:
    // the secondary axis comes from part geometry (the face adjacent to an edge, the principal direction
    // of a face), and reversing it for nothing is the same as moving a body for nothing.
    //
    // The variants below are ordered by number of turns (0, 1, 1, 2) and are replaced only on a strict
    // win, so the first of any equals survives — the quietest one.
    let mut best = (j.side(), f64::NEG_INFINITY);
    for (side, s) in variants {
        let score = s[0] * d[0] + s[1] * d[1] + s[2] * d[2];
        if score > best.1 + 1e-9 {
            best = (side, score);
        }
    }
    best.0
}

/// Turn of the first anchor for the chosen side — one implementation for the whole tree.
///
/// Six places need it (the solver problem, the displayed reading, the mate frame, the degree axis, the
/// "hold as built" declaration and the relation), and a private copy in each is free to drift; an earlier
/// copy turned about X only.
pub fn side_turn(side: Side) -> UnitQuaternion<f64> {
    let Side { flip, roll_flip } = side;
    let mut q = UnitQuaternion::identity();
    if flip {
        q *= UnitQuaternion::from_axis_angle(&Vector3::x_axis(), std::f64::consts::PI);
    }
    if roll_flip {
        q *= UnitQuaternion::from_axis_angle(&Vector3::z_axis(), std::f64::consts::PI);
    }
    q
}

/// The same turn as a 3x4 matrix, for callers that do not work in quaternions.
pub fn side_turn12(side: Side) -> [f64; 12] {
    let Side { flip, roll_flip } = side;
    let (x, y, z) = (if roll_flip { -1.0 } else { 1.0 }, if flip != roll_flip { -1.0 } else { 1.0 }, if flip { -1.0 } else { 1.0 });
    [x, 0.0, 0.0, 0.0, 0.0, y, 0.0, 0.0, 0.0, 0.0, z, 0.0]
}

/// World frames of both joint anchors as the document has them right now.
pub fn joint_frames_now(project: &Project, j: &crate::feature::Joint) -> Option<(Isometry3<f64>, Isometry3<f64>)> {
    joint_frames_now_of(project, j)
}

/// The same, for any joint description, not necessarily one stored in the document.
///
/// Needed by the "hold as built" declaration: the placement has to be read from frames without the
/// already-baked declaration, or it would compose with itself and the body would drift on every repeated
/// declaration.
pub fn joint_frames_now_of(project: &Project, j: &crate::feature::Joint) -> Option<(Isometry3<f64>, Isometry3<f64>)> {
    let (ca, cb) = (project.connector(j.a)?, project.connector(j.b)?);
    let (fa, fb) = joint_local_frames(project, j)?;
    let side = joint_side_now(project, j);
    let mut wa = pose_from12(&project.world_transform(ca.owner)) * pose_from12(&fa);
    wa *= side_turn(side);
    let wb = pose_from12(&project.world_transform(cb.owner)) * pose_from12(&fb);
    Some((wa, wb))
}

/// World frames of both joint anchors for a given set of component placements.
fn joint_frames(project: &Project, j: &crate::feature::Joint, index: &std::collections::HashMap<Id, usize>, poses: &[Isometry3<f64>]) -> Option<(Isometry3<f64>, Isometry3<f64>)> {
    let (ca, cb) = (project.connector(j.a)?, project.connector(j.b)?);
    let (ia, ib) = (index.get(&ca.owner)?, index.get(&cb.owner)?);
    let (fa, fb) = joint_local_frames(project, j)?;
    // The anchor turn is the solver's turn; anything else reports the value with the opposite sign.
    let side = joint_side_now(project, j);
    let mut wa = poses[*ia] * pose_from12(&fa);
    wa *= side_turn(side);
    Some((wa, poses[*ib] * pose_from12(&fb)))
}

/// Write the actual values back into the joints: where the body ended up along each of its degrees.
///
/// Every slot is written, specified ones included: the reading is a fact, and for a satisfied
/// requirement it coincides with the requested value. Writing only into the unspecified slot freezes the
/// value after the first write, so the mate displays a long-stale number and then starts demanding it.
fn write_back_free_values(project: &mut Project, comps: &[Id], poses: &[Isometry3<f64>]) {
    let index: std::collections::HashMap<Id, usize> = comps.iter().enumerate().map(|(i, &c)| (c, i)).collect();
    let mut updates: Vec<(Id, [Option<f64>; 3])> = Vec::new();
    for j in &project.joints {
        let Some((wa, wb)) = joint_frames(project, j, &index, poses) else { continue };
        let kind = kind_of(j.kind);
        let vals = [0usize, 1, 2].map(|slot| measured_slot(kind, slot, &wa, &wb));
        if vals.iter().any(|v| v.is_some()) {
            updates.push((j.id, vals));
        }
    }
    // While the pointer leads, a specified number follows the body instead of holding it.
    //
    // The requirement is released for the duration of the drag (see `problem_of_with_pins`), and leaving
    // the old number in the document would lie twice over: the panel shows 100, the body stands at 137,
    // and the next solve without the pointer drags it back to 100. The value reached is written instead.
    let hand_leads = project.drag_pull.is_some();
    for (jid, vals) in updates {
        if let Some(j) = project.joints.iter_mut().find(|x| x.id == jid) {
            if let Some(v) = vals[0] {
                j.angle = v;
            }
            if let Some(v) = vals[1] {
                j.offset = v;
            }
            if let Some(v) = vals[2] {
                j.offset2 = v;
            }
            if hand_leads {
                for slot in 0..3 {
                    if j.drive[slot].is_some() {
                        if let Some(v) = vals[slot] {
                            j.drive[slot] = Some(v);
                        }
                    }
                }
            }
        }
    }
}

fn dedup(mut v: Vec<Id>) -> Vec<usize> {
    v.sort_unstable();
    v.dedup();
    v.into_iter().map(|x| x as usize).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix_round_trip_is_exact() {
        let t = Isometry3::from_parts(
            Translation3::new(3.0, -7.0, 11.0),
            UnitQuaternion::from_axis_angle(&nalgebra::Unit::new_normalize(Vector3::new(1.0, 2.0, -3.0)), 0.9),
        );
        let back = pose_from12(&pose_to12(&t));
        assert!((back.translation.vector - t.translation.vector).norm() < 1e-12, "translation must survive the conversion");
        assert!(back.rotation.angle_to(&t.rotation) < 1e-12, "rotation must survive the conversion");
    }

    #[test]
    fn a_slightly_non_orthogonal_matrix_is_repaired_not_trusted() {
        // A document may accumulate non-orthogonality; taking such a matrix as is means solving against
        // distorted geometry.
        let m = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1e-6, 0.0, 0.0, -1e-6, 1.0, 0.0];
        let t = pose_from12(&m);
        let r = t.rotation.to_rotation_matrix();
        let err = (r.matrix() * r.matrix().transpose() - Matrix3::identity()).norm();
        assert!(err < 1e-12, "rotation must be exactly orthogonal after conversion: {err:.3e}");
    }
}
