//! A WALL OF A CURVE BROKEN IN TWO KEEPS ITS NAME, of an arc and of a spline as of a line: a closed profile - a slot, and
//! three lines closed by a spline - is extruded, the arc or the spline is broken at its middle, and the wall swept from
//! the piece the curve kept its id on is the wall it was, named by the bridge after that curve.
use qymcad_core::feature::{Ends, Purpose, Reach};
use qymcad_core::geom::Point2;
use qymcad_core::model::curve::Curve;
use qymcad_core::model::{Id, Project};
use qymcad_core::names::Role;

/// THE CURVE BROKEN.
#[derive(Clone, Copy, Debug)]
enum Kind {
    Arc,
    Spline,
}

/// THE PROFILE EXTRUDED, the block, and the curve to break with the place it is clicked.
struct Block {
    p: Project,
    si: usize,
    sketch: Id,
    block: Id,
    curve: Id,
    click: Point2,
}

fn block(kind: Kind) -> Block {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("s");
    let sketch = p.sketches[si].id;
    p.add_sketch_node(sketch, "s");
    match kind {
        // a slot from (0, 0) to (20, 0), 5 across its round ends
        Kind::Arc => p.add_slot_entity(si, Point2::new(0.0, 0.0), Point2::new(20.0, 0.0), 5.0, Purpose::Real),
        // three sides of a rectangle 20 by 10, its top a spline bowed up through (10, 14)
        Kind::Spline => {
            p.add_line_entity(si, 0.0, 10.0, 0.0, 0.0, Purpose::Real);
            p.add_line_entity(si, 0.0, 0.0, 20.0, 0.0, Purpose::Real);
            p.add_line_entity(si, 20.0, 0.0, 20.0, 10.0, Purpose::Real);
            p.add_spline(si, vec![Point2::new(20.0, 10.0), Point2::new(10.0, 14.0), Point2::new(0.0, 10.0)], Ends::Open, Purpose::Real);
            p.merge_close_points(si, 1e-9);
        }
    }
    p.regen_sketch(si);
    let contour = p.sketches[si].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).expect("a closed contour");
    let block = p.add_extrude_multi(sketch, vec![contour], 5.0, Reach::Forward, 0.0, vec![]);
    let s = &p.sketches[si];
    let (curve, click) = match kind {
        // the round end at x = 20
        Kind::Arc => (s.entities.iter().find(|e| e.kind.as_arc().is_some() && Curve::of(s, e).is_some_and(|c| c.at(0.5).x > 20.0)).map(|e| e.id).expect("the right end"), Point2::new(25.0, 0.0)),
        Kind::Spline => (s.entities.iter().find(|e| e.kind.as_spline().is_some()).map(|e| e.id).expect("the spline"), Point2::new(10.0, 14.0)),
    };
    Block { p, si, sketch, block, curve, click }
}

#[test]
fn a_wall_of_a_curve_broken_in_two_keeps_its_name() {
    let mut failures = Vec::new();
    for kind in [Kind::Arc, Kind::Spline] {
        let Block { mut p, si, sketch, block, curve, click } = block(kind);
        let (report, _) = qymcad_testkit::regenerate(&mut p);
        if !report.errors.is_empty() {
            failures.push(format!("{kind:?}: the block does not build: {:?}", report.errors));
            continue;
        }
        // the walls swept from the curve, as the bridge named them
        let walls = |p: &Project| p.regen_faces[&block].iter().map(|f| f.id).filter(|d| p.names.get(*d).is_some_and(|n| n.role == Role::Wall && n.src == curve)).collect::<Vec<u32>>();
        let was = walls(&p);
        if was.is_empty() {
            failures.push(format!("{kind:?}: no wall is named after the curve {curve}"));
            continue;
        }
        if !p.break_curve(si, curve, click.x, click.y) {
            failures.push(format!("{kind:?}: the curve did not break"));
            continue;
        }
        p.mark_sketch_dirty(sketch);
        let (report, _) = qymcad_testkit::regenerate(&mut p);
        if !report.errors.is_empty() {
            failures.push(format!("{kind:?}: the block with a broken curve does not build: {:?}", report.errors));
            continue;
        }
        let now = walls(&p);
        if now.is_empty() || !now.iter().all(|d| was.contains(d)) {
            failures.push(format!(
                "{kind:?}: the walls of the curve were {}, are {}",
                was.iter().map(|d| p.names.describe(*d)).collect::<Vec<_>>().join(", "),
                now.iter().map(|d| p.names.describe(*d)).collect::<Vec<_>>().join(", ")
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
