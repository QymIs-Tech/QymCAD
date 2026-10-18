//! THE CAPS OF REGIONS MERGED IN ONE EXTRUSION ARE NAMED BY THE PROFILES THEY ARE DRAWN FROM. Contours picked together
//! that share a side merge into one face before the extrusion, and the shared side is gone from it. The caps of that
//! face are named after the lowest of the contours the person picked for it - the recipe - not after a wall the merge
//! took away, not after the order the table met the names in, and not after a side of the sketch an edit cut into more
//! pieces. Reported behaviour: three of the four corner pads of a sample took their caps as seams of a neighbour, and a
//! parameter that moved none of them renamed the caps of the fourth.
use qymcad_core::feature::{Purpose, Reach};
use qymcad_core::model::{Id, Project};
use qymcad_core::names::Role;

/// PADS AS THE SAMPLE DRAWS THEM: at each corner of a rectangle from (6, 6) to (43, 61) a circle R5 over the corner and
/// a hole R1.5 at its centre; the two pieces of each circle - inside the rectangle and outside it - extruded 6 in one
/// node. Each pair merges into one disc with a hole, wherever the rectangle's sides run inside the circle.
fn pads(p: &mut Project) -> Pads {
    let si = p.new_sketch("s");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "s");
    let centres = [(5.0, 5.0), (44.0, 5.0), (44.0, 62.0), (5.0, 62.0)];
    for (x, y) in centres {
        p.add_circle_entity(si, x, y, 5.0, Purpose::Real);
    }
    p.add_rect_entity(si, 6.0, 6.0, 43.0, 61.0, Purpose::Real);
    for (x, y) in centres {
        p.add_circle_entity(si, x, y, 1.5, Purpose::Real);
    }
    p.regen_sketch(si);
    let near = |q: &[f64], c: (f64, f64)| ((q[0] - c.0).powi(2) + (q[1] - c.1).powi(2)).sqrt();
    let piece = |xy: &[f64]| centres.iter().any(|c| xy.chunks(2).all(|q| near(q, *c) < 5.0 + 1e-6) && xy.chunks(2).any(|q| near(q, *c) > 2.0));
    let picked: Vec<Id> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some_and(|xy| piece(&xy))).collect();
    assert_eq!(picked.len(), 8, "the circles are not cut in two pieces each by the rectangle");
    let body = p.add_extrude_multi(sid, picked, 6.0, Reach::Forward, 0.0, vec![]);
    Pads { sketch: sid, body }
}

/// The sketch the pads are drawn in and the body they make.
struct Pads {
    sketch: Id,
    body: Id,
}

/// The corner of the rectangle at (6, 6) moved to (`to`, `to`), as a drag in the sketch moves it.
fn move_corner(p: &mut Project, sid: Id, to: f64) {
    let si = p.sketch_index(sid).expect("the sketch");
    for q in &mut p.sketches[si].points {
        if (q.x - 6.0).abs() < 1e-9 && (q.y - 6.0).abs() < 1e-9 {
            q.x = to;
            q.y = to;
        }
    }
    for q in &mut p.sketches[si].points {
        if (q.x - 6.0).abs() < 1e-9 && (q.y - 61.0).abs() < 1e-9 {
            q.x = to;
        }
        if (q.x - 43.0).abs() < 1e-9 && (q.y - 6.0).abs() < 1e-9 {
            q.y = to;
        }
    }
    p.regen_sketch(si);
    p.mark_sketch_dirty(sid);
}

/// The names of the faces of a body square to the sketch: its caps.
fn caps(p: &Project, body: Id) -> Vec<String> {
    let mut v: Vec<String> = p.regen_faces[&body].iter().filter(|f| f.normal[2].abs() > 0.99).map(|f| p.names.describe(f.id)).collect();
    v.sort();
    v
}

#[test]
fn the_caps_of_pads_of_two_pieces_are_caps_and_keep_their_names_when_the_corner_moves() {
    let mut fails = Vec::new();
    let mut p = Project::default();
    p.new_document();
    let Pads { sketch: sid, body } = pads(&mut p);
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "the pads do not extrude: {:?}", report.errors);
    let before = caps(&p, body);
    if before.len() != 8 {
        fails.push(format!("the pads have {} caps, not 8: {before:?}", before.len()));
    }
    for f in p.regen_faces[&body].iter().filter(|f| f.normal[2].abs() > 0.99) {
        if !matches!(p.names.get(f.id).map(|n| n.role), Some(Role::CapStart | Role::CapEnd)) {
            fails.push(format!("a cap of a pad is named {} - not a cap", p.names.describe(f.id)));
        }
    }
    move_corner(&mut p, sid, 6.6);
    let (report, _) = qymcad_testkit::regenerate_dirty_with_shapes(&mut p, shapes);
    assert!(report.errors.is_empty(), "the pads do not extrude with the corner moved: {:?}", report.errors);
    let after = caps(&p, body);
    if after != before {
        fails.push(format!("the corner of the rectangle moved under a pad, which moved no cap, and the caps were renamed: {before:?} -> {after:?}"));
    }
    assert!(fails.is_empty(), "{}", fails.join("\n"));
}

/// THE CORNER PADS OF THE PRIVATE SAMPLE vacuumCleaner (node 175: four pads, each two pieces of a circle cut by a
/// rectangle, the arcs of each circle in four pieces) take caps, and the parameter `ram_w`, which moves the rectangle
/// and none of the pads, renames none of them.
#[test]
fn the_corner_pads_of_the_vacuum_cleaner_keep_their_caps() {
    let Some(path) = sample("vacuumCleaner.qcad") else { return };
    let mut p = qymcad_io::load_project(&path).expect("the sample opens");
    let shapes = qymcad_testkit::restore_import_shapes(&p);
    let (_, shapes) = qymcad_testkit::regenerate_with_shapes(&mut p, shapes);
    let mut fails = Vec::new();
    let before = caps(&p, 175);
    for f in p.regen_faces[&175].iter().filter(|f| f.normal[2].abs() > 0.99) {
        if !matches!(p.names.get(f.id).map(|n| n.role), Some(Role::CapStart | Role::CapEnd)) {
            fails.push(format!("a cap of a pad is named {} - not a cap", p.names.describe(f.id)));
        }
    }
    let i = p.parameters.iter().position(|x| x.name == "ram_w").expect("the parameter ram_w");
    p.parameters[i].expr = "6.6".into();
    p.eval_parameters();
    for si in 0..p.sketches.len() {
        if p.sketches[si].constraints.iter().any(|c| c.expr().is_some()) {
            p.solve_sketch(si);
            let sid = p.sketches[si].id;
            p.mark_sketch_dirty(sid);
        }
    }
    p.mark_param_dependents_dirty_for("ram_w");
    let _ = qymcad_testkit::regenerate_dirty_with_shapes(&mut p, shapes);
    let after = caps(&p, 175);
    if after != before {
        fails.push(format!("ram_w 6 -> 6.6 moved no pad and renamed their caps: {before:?} -> {after:?}"));
    }
    assert!(fails.is_empty(), "{}", fails.join("\n"));
}

/// A file of the private samples, when this tree has it.
fn sample(name: &str) -> Option<String> {
    let path = format!("{}/../../samples/{name}", env!("CARGO_MANIFEST_DIR"));
    if std::path::Path::new(&path).exists() {
        Some(path)
    } else {
        eprintln!("PASSED OVER: the private sample {name} is not in this tree - the check on it runs only where the samples are");
        None
    }
}

/// THE CAP OF THE CUT OF THE PRIVATE SAMPLE vacuumCleaner (node 176) keeps its name when the side of the rectangle its
/// lowest wall lies on is cut in more pieces: the dimension [80, 81] of sketch 3 from 49 to 53.9 brings the corner pads
/// across that side, which then runs in three pieces, and moves no cap of the cut.
#[test]
fn a_cap_keeps_its_name_when_the_side_it_is_named_after_is_cut_in_more_pieces() {
    use qymcad_core::model::DimTarget;
    let Some(path) = sample("vacuumCleaner.qcad") else { return };
    let mut p = qymcad_io::load_project(&path).expect("the sample opens");
    let shapes = qymcad_testkit::restore_import_shapes(&p);
    let (_, shapes) = qymcad_testkit::regenerate_with_shapes(&mut p, shapes);
    let before = caps(&p, 176);
    p.set_dim_target_value(&DimTarget::Sketch { sketch: 3, refs: vec![80, 81] }, 53.9);
    let si = p.sketch_index(3).expect("sketch 3");
    p.solve_sketch(si);
    p.mark_sketch_dirty(3);
    let _ = qymcad_testkit::regenerate_dirty_with_shapes(&mut p, shapes);
    let after = caps(&p, 176);
    assert_eq!(after, before, "the caps of the cut were renamed though none of them moved");
}
