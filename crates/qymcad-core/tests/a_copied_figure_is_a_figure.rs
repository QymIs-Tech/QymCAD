//! A FIGURE COPIED IS A FIGURE: Copy, Mirror, a mirror about a point, a pattern and a paste from the clipboard of a
//! rectangle, a polygon or a slot lay rectangles, polygons and slots - each with a record of its own on its own curves, a
//! rectangle with its centre and its diagonals, each held by as many constraints of its own as the figure copied (a
//! rectangle whose turn an angle dimension took, by its parallel and square sides), which the copy satisfies where it
//! stands: a solve after the copy moves no point of it; and with its size - the radius of a polygon on the copy's circle.
//!
//! Reported behaviour: the copies were plain curves - a mirrored rectangle fell apart under a dragged corner, a copied
//! polygon had no circle of its own to hold its corners; a paste from the clipboard laid plain curves still.
use qymcad_core::feature::Purpose;
use qymcad_core::geom::Point2;
use qymcad_core::model::{Constraint, PatternKind, Project};

/// A SHAPE TO COPY: its name and how it is drawn into a sketch, answering the curves a person takes it by.
struct Shape {
    name: &'static str,
    draw: fn(&mut Project, usize) -> Vec<u64>,
}

const SHAPES: [Shape; 5] = [
    // turned 30 deg, so a turn of its sides that the copy gets wrong shows as a move under the solve
    Shape { name: "turned rectangle", draw: |p, si| p.add_rect3_entity(si, Point2::new(10.0, 5.0), Point2::new(18.66, 10.0), Point2::new(15.66, 15.2), Purpose::Real) },
    // its turn taken by an angle dimension to a fixed line: held by parallel and square sides, not by turns
    Shape {
        name: "rectangle turned by an angle",
        draw: |p, si| {
            let sides = p.add_rect_entity(si, 10.0, 5.0, 20.0, 12.0, Purpose::Real);
            let l = p.add_line_entity(si, 10.0, -10.0, 20.0, -14.0, Purpose::Real);
            let ends = p.sketches[si].entities.iter().find(|e| e.id == l).and_then(|e| e.kind.as_line()).expect("the line");
            let r = p.sketches[si].rects()[0].clone();
            let dim = Constraint::AngleLines { a: ends.a, b: ends.b, c: r.corners[0], d: r.corners[1], deg: 20.0, expr: String::new(), driven: false, off: 0.0, at: None };
            p.give_rect_turn_to(si, &dim);
            p.sketches[si].constraints.extend([Constraint::Fixed { p: ends.a }, Constraint::Fixed { p: ends.b }, dim]);
            p.solve_sketch(si);
            sides
        },
    },
    Shape { name: "rectangle from its centre", draw: |p, si| p.add_rect_from_centre(si, Point2::new(14.0, 8.0), Point2::new(20.0, 12.0), Purpose::Real) },
    Shape {
        name: "polygon",
        // its size given by the radius of its circle, as the field of the polygon gives it
        draw: |p, si| {
            p.add_polygon_entity(si, Point2::new(14.0, 8.0), Point2::new(20.0, 10.0), 6, Purpose::Real);
            let centre = p.sketches[si].entities.iter().find_map(|e| e.kind.as_circle()).map(|c| c.center).expect("the circle of the polygon");
            p.ensure_diameter(si, centre, false);
            p.sketches[si].entities.iter().map(|e| e.id).collect()
        },
    },
    Shape {
        name: "slot",
        draw: |p, si| {
            p.add_slot_entity(si, Point2::new(10.0, 8.0), Point2::new(20.0, 12.0), 3.0, Purpose::Real);
            p.sketches[si].entities.iter().map(|e| e.id).collect()
        },
    },
];

/// AN EDITING TOOL that lays copies: its name and what it does to the curves taken.
struct Tool {
    name: &'static str,
    apply: fn(&mut Project, usize, &[u64]),
}

const TOOLS: [Tool; 5] = [
    Tool {
        name: "Copy",
        apply: |p, si, eids| {
            p.copy_entities(si, eids, 30.0, 0.0);
        },
    },
    Tool { name: "Mirror", apply: |p, si, eids| p.mirror_entities(si, eids, 0.0, 0.0, 0.0, 1.0) },
    Tool { name: "Mirror about a point", apply: |p, si, eids| p.mirror_about_point(si, eids, 0.0, 0.0) },
    Tool {
        name: "Circular pattern",
        apply: |p, si, eids| {
            p.add_pattern(si, eids, PatternKind::Circular { cx: 0.0, cy: 0.0, count: 3, total_deg: 360.0 });
        },
    },
    Tool {
        name: "Paste",
        apply: |p, si, eids| {
            let clip = p.copy_sketch_geometry(si, eids, 0.0, 0.0);
            p.paste_sketch_geometry(si, &clip, 30.0, 40.0);
        },
    },
];

/// The records of the sketch: the rectangles, then the figures, each by the ids of its curves.
fn records(p: &Project, si: usize) -> Vec<Vec<u64>> {
    let s = &p.sketches[si];
    let rects = s.rects();
    rects.iter().map(|r| r.sides.iter().copied().chain(r.diagonals.into_iter().flatten()).collect()).chain(s.shapes_not_rects().map(|f| f.curves.clone())).collect()
}

fn check(shape: &Shape, tool: &Tool) -> Result<(), String> {
    let mut p = Project::default();
    let si = p.new_sketch("figures");
    let eids = (shape.draw)(&mut p, si);
    let before = records(&p, si);
    let held = p.sketches[si].held_by_shapes().len();
    let sized = p.sketches[si].constraints.iter().filter(|c| matches!(c, Constraint::Diameter { .. })).count();
    let old: Vec<u64> = p.sketches[si].entities.iter().map(|e| e.id).collect();
    (tool.apply)(&mut p, si, &eids);
    let copies = if tool.name == "Circular pattern" { 2 } else { 1 };
    let after = records(&p, si);
    if after.len() != before.len() * (1 + copies) {
        return Err(format!("{} records, not {}", after.len(), before.len() * (1 + copies)));
    }
    for r in &after[before.len()..] {
        if r.len() != before[0].len() || r.iter().any(|id| old.contains(id)) {
            return Err(format!("a record of a copy holds {r:?}, the source {:?}", before[0]));
        }
        if r.iter().any(|id| p.sketches[si].entity(*id).is_none()) {
            return Err(format!("a record of a copy names a curve the sketch has not: {r:?}"));
        }
    }
    // the size of a figure goes with each copy of it: a radius of a polygon laid on the copy's own circle
    let sizes = |p: &Project| p.sketches[si].constraints.iter().filter(|c| matches!(c, Constraint::Diameter { .. })).count();
    if sizes(&p) != sized * (1 + copies) {
        return Err(format!("{} radii of figures, not {}: a copy lost the size of the figure", sizes(&p), sized * (1 + copies)));
    }
    let held_after = p.sketches[si].held_by_shapes().len();
    if held_after != held * (1 + copies) {
        return Err(format!("{held_after} constraints the figures hold themselves by, not {}: a copy is not held as the figure is", held * (1 + copies)));
    }
    let centres: Vec<u64> = p.sketches[si].rects().iter().map(|r| r.centre).collect();
    if centres.iter().enumerate().any(|(k, c)| centres[..k].contains(c)) {
        return Err(format!("two rectangles share a centre: {centres:?}"));
    }
    let at: Vec<[f64; 2]> = p.sketches[si].points.iter().map(|q| [q.x, q.y]).collect();
    p.solve_sketch(si);
    let moved = p.sketches[si].points.iter().zip(&at).map(|(q, [x, y])| (q.x - x).hypot(q.y - y)).fold(0.0, f64::max);
    if moved > 1e-6 {
        return Err(format!("the solve after it moved a point by {moved:.6}: the copy does not stand as its own constraints hold it"));
    }
    Ok(())
}

#[test]
fn every_figure_copied_by_every_tool_is_a_figure() {
    let failures: Vec<String> = SHAPES.iter().flat_map(|s| TOOLS.iter().map(move |t| (s, t))).filter_map(|(s, t)| check(s, t).err().map(|why| format!("{} of a {}: {why}", t.name, s.name))).collect();
    assert!(failures.is_empty(), "{} of {} failed:\n  {}", failures.len(), SHAPES.len() * TOOLS.len(), failures.join("\n  "));
}
