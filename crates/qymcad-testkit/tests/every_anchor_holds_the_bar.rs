//! ACCEPTANCE MATRIX ON LIVE GEOMETRY: EVERY JOINT KIND ON EACH OF THE SEVEN ANCHORS.
//!
//! The matrix in the kernel stands on INVENTED geometry: `MeshEdge` is assembled by hand and a face
//! is given by a synthetic key. That catches the mathematics but not what comes out of the OCCT
//! kernel — and that is exactly where round edges, cylinder axes and neighbouring face normals come
//! from. Of the seven anchors, three were covered there.
//!
//! Here the part is real: a 40x40x20 plate with a through d20 hole. It carries ALL seven anchors — a
//! planar face, a cylindrical face, a straight edge, a round edge, a vertex, the part origin and a
//! base plane.
//!
//! What is asked is not "it did not crash" but, point by point: the anchor RESOLVES (otherwise the
//! joint drops out of the computation silently), there are EXACTLY as many degrees of freedom as the
//! kind promises, a commanded value is met EXACTLY, and the part travels WHERE THE GIZMO POINTS.
use qymcad_core::feature::{apply12, AnchorRef, FaceKey, JointKind};
use qymcad_core::model::{Id, Project};

/// The part: a 40x40x20 plate with a through d20 hole in the middle. Returns (component, body).
fn plate_with_a_hole(p: &mut Project, name: &str, at: [f64; 3]) -> (Id, Id) {
    let c = p.add_part(name);
    p.set_active_component(Some(c));
    let si = p.new_sketch(name);
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, name);
    p.add_rect_entity(si, 0.0, 0.0, 40.0, 40.0, qymcad_core::feature::Purpose::Real);
    p.add_circle_entity(si, 20.0, 20.0, 10.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    let body = p.add_extrude(sid, 20.0);
    p.finish_base_body(body, 1);
    p.move_component(c, at);
    (c, body)
}

/// All seven anchors of the part, found IN LIVE GEOMETRY (not invented).
fn seven_anchors(p: &Project, body: Id) -> Vec<(&'static str, AnchorRef)> {
    let faces = p.regen_faces.get(&body).cloned().unwrap_or_default();
    let edges = p.regen_edges.get(&body).cloned().unwrap_or_default();

    // planar face: the top of the plate (normal +Z, the largest of the horizontal ones by area)
    let flat = faces
        .iter()
        .filter(|f| f.normal[2] > 0.99)
        .max_by(|a, b| a.area.partial_cmp(&b.area).unwrap())
        .map(|f| FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id })
        .expect("the plate has a top face");
    // cylindrical face: the bore wall — the one where the kernel finds an AXIS
    let cyl = faces
        .iter()
        .find(|f| {
            let k = FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id };
            p.face_axis(body, &k).is_some()
        })
        .map(|f| FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id })
        .expect("the plate with a hole has a cylindrical face");
    // straight edge — a vertical edge of the plate; round edge — the rim of the bore
    let straight = edges.iter().find(|e| !e.is_circular() && e.dir[2].abs() > 0.99).expect("a vertical edge of the plate").id;
    let round = edges.iter().find(|e| e.is_circular()).expect("the rim of the bore").id;

    vec![
        ("part origin", AnchorRef::Origin),
        ("base plane", AnchorRef::BasePlane(qymcad_core::feature::BasePlane::XY)),
        ("planar face", AnchorRef::FaceCenter(body, flat)),
        ("cylindrical face", AnchorRef::FaceCenter(body, cyl)),
        ("straight edge", AnchorRef::EdgeMid(body, straight)),
        ("round edge", AnchorRef::EdgeMid(body, round)),
        ("vertex", AnchorRef::Vertex(body, straight, false)),
    ]
}

fn norm(v: [f64; 3]) -> f64 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

#[test]
fn every_kind_on_every_one_of_the_seven_anchors() {
    // The kinds and what they promise: (kind, degrees of freedom, does slot 1 travel)
    let kinds = [
        (JointKind::Rigid, 0u8, false),
        (JointKind::Revolute, 1, false),
        (JointKind::Slider, 1, true),
        (JointKind::Cylindrical, 2, true),
        (JointKind::Ball, 3, false),
        (JointKind::Planar, 3, true),
        (JointKind::PinSlot, 2, true),
        // PARALLEL holds direction only — four degrees of freedom.
        (JointKind::Parallel, 4, false),
    ];

    let mut fails: Vec<String> = Vec::new();
    // Anchor names are taken once — the part itself is rebuilt for every pair so that a previous
    // solution does not influence the next one.
    let names: Vec<&'static str> = {
        let mut p = Project::default();
        p.new_document();
        let (_, body) = plate_with_a_hole(&mut p, "A", [0.0; 3]);
        let (r, _) = qymcad_testkit::regenerate(&mut p);
        assert!(r.errors.is_empty(), "the plate with a hole did not build: {:?}", r.errors);
        seven_anchors(&p, body).into_iter().map(|(n, _)| n).collect()
    };
    // GUARD ON THE SIZE OF THE MATRIX. The matrix is declared as "seven anchors x eight kinds", and
    // the number of pairs checked must equal that: the anchor list is built from live geometry, and
    // if that geometry fails to come up the matrix shrinks silently while the run stays green.
    assert_eq!(names.len(), 7, "the matrix must hold seven anchors, and {} were found: {names:?}", names.len());
    assert_eq!(kinds.len(), 8, "the matrix must hold eight joint kinds, and {} are listed", kinds.len());

    for anchor_name in names {
        for (kind, dof, moves) in kinds {
            let mut p = Project::default();
            p.new_document();
            let (ca_owner, body_a) = plate_with_a_hole(&mut p, "A", [0.0, 0.0, 0.0]);
            let (cb_owner, body_b) = plate_with_a_hole(&mut p, "B", [100.0, 0.0, 0.0]);
            let (r, _) = qymcad_testkit::regenerate(&mut p);
            if !r.errors.is_empty() {
                fails.push(format!("{kind:?}/{anchor_name}: the parts did not build: {:?}", r.errors));
                continue;
            }
            p.set_grounded(ca_owner, true);
            let pick = |p: &Project, body: Id| {
                seven_anchors(p, body).into_iter().find(|(n, _)| *n == anchor_name).map(|(_, a)| a).expect("anchor by name")
            };
            let (aa, ab) = (pick(&p, body_a), pick(&p, body_b));
            let ca = p.add_connector(ca_owner, aa);
            let cb = p.add_connector(cb_owner, ab);
            let j = p.add_joint(ca, cb, kind);

            // 1. THE ANCHOR RESOLVES — otherwise the joint drops out of the computation silently.
            if !p.joint_faults().is_empty() {
                fails.push(format!("{kind:?}/{anchor_name}: the joint is faulty right after creation: {:?}", p.joint_faults()));
                continue;
            }

            // 2. EXACTLY AS MANY DEGREES OF FREEDOM AS THE KIND PROMISES.
            let got = p.component_dof(cb_owner);
            if got != dof {
                fails.push(format!("{kind:?}/{anchor_name}: {got} degrees of freedom instead of {dof}"));
            }

            // 3. THE JOINT DOES NOT JUMP: solve a second time and the part does not stir.
            //
            // The part MUST move when the joint is created — otherwise the joint means nothing. But a
            // second solve under the same conditions must move nothing: if it does, the solution is
            // undefined and the part creeps on every rebuild of the document. That is what shows up
            // as an assembly drifting apart by itself.
            p.solve_joints();
            let settled = apply12(&p.world_transform(cb_owner), [0.0, 0.0, 0.0]);
            p.solve_joints();
            let again = apply12(&p.world_transform(cb_owner), [0.0, 0.0, 0.0]);
            let creep = norm([again[0] - settled[0], again[1] - settled[1], again[2] - settled[2]]);
            if creep > 1e-6 {
                fails.push(format!("{kind:?}/{anchor_name}: a repeat solve moved the part {creep:.4} mm — the assembly creeps by itself"));
            }

            // 4. RE-CREATION GIVES THE SAME RESULT. The reported behaviour was that joints do not
            // move even after being deleted and placed again. Delete the joint, push the part away,
            // place the same joint on the same anchors — the part must come back to the same place.
            //
            // EVERY DEGREE IS PINNED TO ZERO, and without that the check demanded the impossible: a
            // joint with a free degree leaves the position along it UNDEFINED, and minimal
            // displacement legitimately leaves the part where it was pushed. The first version failed
            // 37 cases — and failed them for nothing. Only a fully determined position can be
            // compared; then a divergence means exactly what is being checked: the anchors were
            // derived DIFFERENTLY from last time.
            let pin_all = |p: &mut Project, jid: Id| {
                for slot in 0..3usize {
                    if qymcad_core::asm::joint::slot_axis(qymcad_core::asm::bridge::kind_of(kind), slot).is_some() {
                        p.joints.iter_mut().find(|x| x.id == jid).unwrap().drive[slot] = Some(0.0);
                    }
                }
            };
            pin_all(&mut p, j);
            p.solve_joints();
            let pinned = apply12(&p.world_transform(cb_owner), [0.0, 0.0, 0.0]);
            let turn = {
                let m = p.world_transform(cb_owner);
                let o = apply12(&m, [0.0, 0.0, 0.0]);
                let x = apply12(&m, [1.0, 0.0, 0.0]);
                [x[0] - o[0], x[1] - o[1], x[2] - o[2]]
            };

            p.delete_joint(j);
            p.move_component(cb_owner, [70.0, -30.0, 45.0]);
            let (aa2, ab2) = (pick(&p, body_a), pick(&p, body_b));
            let ca2 = p.add_connector(ca_owner, aa2);
            let cb2 = p.add_connector(cb_owner, ab2);
            let j = p.add_joint(ca2, cb2, kind);
            if p.joints.len() != 1 {
                fails.push(format!("{kind:?}/{anchor_name}: {} joints after re-creation instead of one — the check measures the wrong thing", p.joints.len()));
                continue;
            }
            let displaced = apply12(&p.world_transform(cb_owner), [0.0, 0.0, 0.0]);
            pin_all(&mut p, j);
            p.solve_joints();
            let remade = apply12(&p.world_transform(cb_owner), [0.0, 0.0, 0.0]);

            // ASK OF A JOINT ONLY WHAT IT DEFINES. Parallel holds DIRECTION only: where the part
            // stands is its own business, and demanding it return to a place is demanding what was
            // never promised. The other kinds do define position, so their origin is fair game.
            let holds_position = !matches!(kind, JointKind::Parallel);
            if holds_position {
                // THE JOINT REALLY DID PULL THE PART BACK. Without this guard the "landed in the same
                // place" check would also pass when the re-created joint does not act at all.
                if norm([remade[0] - displaced[0], remade[1] - displaced[1], remade[2] - displaced[2]]) < 1.0 {
                    fails.push(format!("{kind:?}/{anchor_name}: the re-created joint did not move the displaced part — there is nothing to check re-creation with"));
                    continue;
                }
                let drift = norm([remade[0] - pinned[0], remade[1] - pinned[1], remade[2] - pinned[2]]);
                if drift > 1e-6 {
                    fails.push(format!("{kind:?}/{anchor_name}: the same joint, placed again, took the part {drift:.4} mm away"));
                }
            }
            // ORIENTATION IS ASKED OF EVERY KIND: any joint defines it, parallel included.
            let dir = |m: &[f64; 12]| {
                let o = apply12(m, [0.0, 0.0, 0.0]);
                let x = apply12(m, [1.0, 0.0, 0.0]);
                [x[0] - o[0], x[1] - o[1], x[2] - o[2]]
            };
            let now = dir(&p.world_transform(cb_owner));
            let want = turn;
            let dot = now[0] * want[0] + now[1] * want[1] + now[2] * want[2];
            if dot < 0.999 {
                fails.push(format!("{kind:?}/{anchor_name}: the same joint, placed again, oriented the part differently: was {want:?}, now {now:?}"));
            }

            if !moves {
                continue;
            }
            // A COMMAND IS AN ABSOLUTE VALUE, NOT "TRAVEL BY". The first version of this check
            // measured the distance from the free solution to a commanded ten and failed at 90 mm —
            // exactly the distance between the parts. It failed fairly: the distance really is 90, it
            // is simply not what the command promises. Measure the travel between TWO commands: zero
            // and ten.
            //
            // THE OTHER DEGREES ARE PINNED. What is measured is the position of the PART origin, and
            // that origin also moves under rotation about the joint axis: on cylindrical and planar
            // joints "travel" mixed with rotation gave anything but ten. Only the degree under test
            // is left free.
            for other in [0usize, 2] {
                if qymcad_core::asm::joint::slot_axis(qymcad_core::asm::bridge::kind_of(kind), other).is_some() {
                    p.joints.iter_mut().find(|x| x.id == j).unwrap().drive[other] = Some(0.0);
                }
            }
            p.joints.iter_mut().find(|x| x.id == j).unwrap().drive[1] = Some(0.0);
            p.solve_joints();
            let at0 = apply12(&p.world_transform(cb_owner), [0.0, 0.0, 0.0]);
            // where the interface says the part will go
            let arrow = p.joint_slot_axis(j, 1, p.root);
            p.joints.iter_mut().find(|x| x.id == j).unwrap().drive[1] = Some(10.0);
            p.solve_joints();
            let at1 = apply12(&p.world_transform(cb_owner), [0.0, 0.0, 0.0]);
            let d = [at1[0] - at0[0], at1[1] - at0[1], at1[2] - at0[2]];
            let len = norm(d);

            // 5. COMMANDED 10 — TRAVEL EXACTLY 10.
            if (len - 10.0).abs() > 1e-6 {
                fails.push(format!("{kind:?}/{anchor_name}: commanded 10, travelled {len:.3}"));
                continue;
            }
            // 6. THE PART TRAVELS WHERE THE GIZMO POINTS.
            if let Some(ax) = arrow {
                let dot = (d[0] * ax[0] + d[1] * ax[1] + d[2] * ax[2]) / len;
                if dot < 0.999 {
                    fails.push(format!("{kind:?}/{anchor_name}: the gizmo arrow points at {ax:?} and the part went to {:?}", [d[0] / len, d[1] / len, d[2] / len]));
                }
            } else {
                fails.push(format!("{kind:?}/{anchor_name}: a movable joint cannot be asked for its travel direction — there is nothing to draw a gizmo from"));
            }

            // 7. THE LIMIT HOLDS. Range 0..5 with a command of 20: the part must stop at five rather
            // than travel twenty. A limit offered in the interface but not held in the computation is
            // worse than none — the user believes the travel is bounded when it is not.
            {
                let jj = p.joints.iter_mut().find(|x| x.id == j).unwrap();
                jj.limit_min[1] = Some(0.0);
                jj.limit_max[1] = Some(5.0);
                jj.drive[1] = Some(20.0);
            }
            p.solve_joints();
            let at2 = apply12(&p.world_transform(cb_owner), [0.0, 0.0, 0.0]);
            let went = norm([at2[0] - at0[0], at2[1] - at0[1], at2[2] - at0[2]]);
            if (went - 5.0).abs() > 1e-6 {
                fails.push(format!("{kind:?}/{anchor_name}: limit 0..5, commanded 20 — the part travelled {went:.3} instead of 5"));
            }
        }
    }

    assert!(fails.is_empty(), "acceptance matrix: {} divergences\n{}", fails.len(), fails.join("\n"));
}
