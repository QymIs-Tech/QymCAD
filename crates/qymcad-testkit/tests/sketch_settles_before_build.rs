//! Reported behaviour: a cut placed right along a face left no gap where one was expected, and after
//! a rebuild the wall appeared on the other side instead.
//!
//! The file held an UNDER-SOLVED sketch: the slot was collinear with a construction line dimensioned
//! at exactly 130 from the axis, while the coordinates read -129.99900 and -129.99930 (the ends of
//! the "vertical" line had drifted apart by 0.3 micrometres). The body was built from exactly those
//! points, and a FILM a fraction of a micron thick was left in the wall: no gap. Adjust the width and
//! the film jumped to the other side. One full solve is enough to put the points at -129.999999908.
//!
//! The invariant: THE TIMELINE SETTLES ITS SKETCHES BEFORE BUILDING. That is what is checked here —
//! on the body, not on the points.
use qymcad_core::model::{Constraint, Project};

/// A 20 mm plate with a slot collinear with the dimension reference line. The sketch points are
/// deliberately knocked off by a micron (exactly as in the real file) — the built body must be what
/// the CONSTRAINTS REQUIRE.
#[test]
fn body_is_built_from_the_solved_sketch_not_the_stale_points() {
    let mut p = Project::default();
    p.new_document();

    // the base: a 40x40x20 plate
    let base = p.add_sketch("base", vec![], None);
    p.add_sketch_node(base, "Base sketch");
    let bi = p.sketch_index(base).unwrap();
    p.add_rect_entity(bi, -20.0, -20.0, 20.0, 20.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(bi);
    let base_body = p.add_extrude_multi(base, Vec::new(), 20.0, qymcad_core::feature::Reach::Forward, 0.0, Vec::new());

    // the slot: a rectangle whose left side is COLLINEAR with a construction vertical dimensioned from the axis
    let cut = p.add_sketch("slot", vec![], None);
    p.add_sketch_node(cut, "Slot sketch");
    let ci = p.sketch_index(cut).unwrap();
    let aux = p.add_line_entity(ci, -20.0, -15.0, -20.0, 15.0, qymcad_core::feature::Purpose::Construction);
    p.add_rect_entity(ci, -20.0, -5.0, -15.0, 5.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(ci);

    let (a0, a1) = match p.sketches[ci].entities.iter().find(|e| e.id == aux).unwrap().kind {
        qymcad_core::model::EntityKind::Line { a, b } => (a, b),
        _ => unreachable!(),
    };
    let left: Vec<u64> = p.sketches[ci].points.iter().filter(|q| (q.x + 20.0).abs() < 1e-9 && q.y.abs() < 10.0).map(|q| q.id).collect();
    assert_eq!(left.len(), 2, "the left side of the slot is two points");
    let axis_o = p.sketches[ci].points.iter().find(|q| q.x == 0.0 && q.y == 0.0).map(|q| q.id);
    let s = &mut p.sketches[ci];
    s.constraints.push(Constraint::Vertical { a: a0, b: a1 });
    s.constraints.push(Constraint::Collinear { a: a0, b: a1, c: left[0], d: left[1] });
    if let Some(o) = axis_o {
        s.constraints.push(Constraint::Fixed { p: o });
    }
    for pt in [a0, a1] {
        s.constraints.push(Constraint::Fixed { p: pt });
    }
    p.solve_sketch(ci);

    // KNOCK the points off by a micron — exactly the state the real file was in
    for id in [left[0], left[1]] {
        if let Some(q) = p.sketches[ci].points.iter_mut().find(|q| q.id == id) {
            q.x += 0.0009;
        }
    }
    let stale: Vec<f64> = p.sketches[ci].points.iter().filter(|q| left.contains(&q.id)).map(|q| q.x).collect();
    assert!(stale.iter().all(|x| (*x + 20.0).abs() > 1e-4), "the points really are knocked off: {stale:?}");

    let cut_prof = p.sketches[ci].contour_ids.clone();
    p.add_combine_multi_op(base_body, cut, cut_prof, 25.0, 0, qymcad_core::feature::Extent::default(), 0.0, Vec::new());
    for n in p.timeline.iter_mut() {
        n.dirty = true;
    }
    let (report, _shapes) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "the timeline built: {:?}", report.errors);

    // 1) the sketch settled — the points returned onto the constraint
    let now: Vec<f64> = p.sketches[ci].points.iter().filter(|q| left.contains(&q.id)).map(|q| q.x).collect();
    for x in &now {
        assert!((x + 20.0).abs() < 1e-6, "the slot landed on the reference line: x={x:.9}, expected -20");
    }

    // 2) and the main point — there is NO film in the body: the slot is open through the wall
    let last = p.timeline.iter().filter_map(|n| n.kind.body()).next_back().unwrap();
    let mi = p.mesh_index(last).expect("the mesh");
    let mesh = &p.bodies[mi].mesh;
    let (z, y) = (10.0_f64, 0.0_f64);
    let mut xs: Vec<f64> = Vec::new();
    for ti in 0..mesh.tris.len() {
        let t = mesh.triangle(ti);
        let (a, b, c) = (t[0], t[1], t[2]);
        let d = (b.y - a.y) * (c.z - a.z) - (b.z - a.z) * (c.y - a.y);
        if d.abs() < 1e-12 {
            continue;
        }
        let u = ((y - a.y) * (c.z - a.z) - (z - a.z) * (c.y - a.y)) / d;
        let v = ((b.y - a.y) * (z - a.z) - (b.z - a.z) * (y - a.y)) / d;
        if u >= 0.0 && v >= 0.0 && u + v <= 1.0 {
            xs.push(a.x + u * (b.x - a.x) + v * (c.x - a.x));
        }
    }
    let film: Vec<f64> = xs.iter().filter(|x| (**x + 20.0).abs() < 0.5).cloned().collect();
    assert!(film.len() <= 1, "a film was left at the outer wall: material boundaries {film:?} (an open slot was expected)");
}
