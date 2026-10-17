//! A DRAG REMEMBERS WHAT IT DRAGS, AND MAKES EVERY FRAME AS A FRAME MADE ALONE: frames of a drag made through what the
//! drag keeps from one frame to the next (`solve_sketch_drag_fast`) leave the points, the radii, the reference
//! dimensions and the loops of the sketch bit for bit as frames made alone (`solve_sketch_drag_frame_alone`) - on every
//! kind of sketch, on rectangles drawn by the tool with a reference dimension, dragged by a corner and by the centre,
//! when the dragged point changes, and when a dimension is changed between two frames.
mod sketch_kinds;

use qymcad_core::model::{Constraint, Project};
use sketch_kinds::{build, Kind, KINDS};

/// All a frame may change, as text: the points and circles bit for bit, the values of the dimensions, the loops.
fn state(p: &Project) -> String {
    let s = &p.sketches[0];
    let points: Vec<String> = s.points.iter().map(|q| format!("{}:{:x}:{:x}", q.id, q.x.to_bits(), q.y.to_bits())).collect();
    let circles: Vec<String> = s
        .entities
        .iter()
        .filter_map(|e| match e.kind {
            qymcad_core::model::EntityKind::Circle { center, r } => Some(format!("{center}:{:x}", r.to_bits())),
            _ => None,
        })
        .collect();
    let dims: Vec<String> = s.constraints.iter().map(|c| format!("{:?}", c.dim_value().map(f64::to_bits))).collect();
    let mut loops: Vec<String> = s
        .contour_ids
        .iter()
        .filter_map(|cid| {
            let c = &p.contours[p.contour_index(*cid)?];
            Some(format!("{cid} in {:?} {} {:?} {:?}", p.contours.parent_of(*cid), c.closed, c.edge_src, c.points.iter().map(|q| (q.x.to_bits(), q.y.to_bits())).collect::<Vec<_>>()))
        })
        .collect();
    loops.sort();
    format!("{points:?}\n{circles:?}\n{dims:?}\n{loops:?}\nleft {}", s.left_unsolved)
}

/// Drag along `path`, each place `(point, x, y)` a frame, both ways, comparing after each frame.
fn both_ways(p: &Project, path: &[(u64, f64, f64)], what: &str, failures: &mut Vec<String>) {
    let (mut kept, mut alone) = (p.clone(), p.clone());
    for (k, &(id, x, y)) in path.iter().enumerate() {
        let _ = kept.solve_sketch_drag_fast(0, Some((id, x, y)));
        let _ = alone.solve_sketch_drag_frame_alone(0, Some((id, x, y)));
        if state(&kept) != state(&alone) {
            failures.push(format!("{what}, frame {k}: the frame made through the session differs from the frame made alone"));
            return;
        }
    }
}

#[test]
fn every_frame_through_the_session_is_a_frame_made_alone() {
    let mut failures = Vec::new();
    for kind in KINDS {
        let mut p = build(kind, 30);
        p.solve_sketch(0);
        let q = p.sketches[0].points[3];
        let path: Vec<(u64, f64, f64)> = (1..=6).map(|k| (q.id, q.x + k as f64 * 2.5, q.y - k as f64 * 1.5)).collect();
        both_ways(&p, &path, &format!("{kind:?}"), &mut failures);
        // the dragged point changes in the middle of the drag
        let r = p.sketches[0].points[p.sketches[0].points.len() - 2];
        let switched: Vec<(u64, f64, f64)> = path.iter().take(3).copied().chain((1..=3).map(|k| (r.id, r.x + k as f64, r.y + k as f64))).collect();
        both_ways(&p, &switched, &format!("{kind:?}, the dragged point changed"), &mut failures);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn rectangles_drawn_by_the_tool_drag_through_the_session_as_alone() {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    for k in 0..12 {
        let (x, y) = ((k % 4) as f64 * 30.0, (k / 4) as f64 * 30.0);
        p.add_rect_entity(si, x, y, x + 20.0, y + 12.0, qymcad_core::feature::Purpose::Real);
    }
    p.add_rect_from_centre(si, qymcad_core::geom::Point2::new(200.0, 0.0), qymcad_core::geom::Point2::new(210.0, 6.0), qymcad_core::feature::Purpose::Real);
    // a reference dimension across the first rectangle
    let (a, b) = (p.sketches[si].rects[0].corners[0], p.sketches[si].rects[0].corners[2]);
    p.sketches[si].constraints.push(Constraint::Distance { dimension: Default::default(), a, b, d: 1.0, off: 2.0, expr: String::new(), driven: true, axis: 0, at: None });
    p.solve_sketch(si);
    let mut failures = Vec::new();
    let corner = p.sketches[si].rects[0].corners[2];
    let at = |id: u64| p.sketches[si].points.iter().find(|q| q.id == id).map(|q| (q.x, q.y)).expect("a point");
    let (cx, cy) = at(corner);
    both_ways(&p, &(1..=5).map(|k| (corner, cx + k as f64, cy + k as f64 * 0.5)).collect::<Vec<_>>(), "a corner of a rectangle", &mut failures);
    let centre = p.sketches[si].rects[12].centre;
    let (mx, my) = at(centre);
    both_ways(&p, &(1..=5).map(|k| (centre, mx - k as f64, my + k as f64)).collect::<Vec<_>>(), "a rectangle by its centre", &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_dimension_changed_between_two_frames_is_taken_by_the_next() {
    let mut p = build(Kind::Rectangles, 6);
    p.solve_sketch(0);
    let q = p.sketches[0].points[1];
    let (mut kept, mut alone) = (p.clone(), p.clone());
    for proj in [&mut kept, &mut alone] {
        let _ = proj.solve_sketch_drag_fast(0, Some((q.id, q.x + 1.0, q.y)));
    }
    // the width of the first rectangle changed between the frames, as a person types it without letting go
    for proj in [&mut kept, &mut alone] {
        if let Some(Constraint::Distance { d, .. }) = proj.sketches[0].constraints.iter_mut().find(|c| matches!(c, Constraint::Distance { .. })) {
            *d = 15.0;
        }
    }
    let _ = kept.solve_sketch_drag_fast(0, Some((q.id, q.x + 2.0, q.y)));
    let _ = alone.solve_sketch_drag_frame_alone(0, Some((q.id, q.x + 2.0, q.y)));
    assert_eq!(state(&kept), state(&alone), "the frame after a dimension changed kept what the drag found before it");
}

#[test]
fn loops_inside_loops_keep_their_nesting_through_the_session() {
    // a frame 100 x 60 with rectangles inside it, and rectangles beside it
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    p.add_rect_entity(si, 0.0, 0.0, 100.0, 60.0, qymcad_core::feature::Purpose::Real);
    for k in 0..4 {
        let x = 10.0 + k as f64 * 22.0;
        p.add_rect_entity(si, x, 20.0, x + 12.0, 32.0, qymcad_core::feature::Purpose::Real);
        p.add_rect_entity(si, 130.0 + k as f64 * 20.0, 20.0, 142.0 + k as f64 * 20.0, 32.0, qymcad_core::feature::Purpose::Real);
    }
    p.solve_sketch(si);
    let mut failures = Vec::new();
    let at = |p: &Project, id: u64| p.sketches[si].points.iter().find(|q| q.id == id).map(|q| (q.x, q.y)).expect("a point");
    // a rectangle inside led out of the frame by its centre and back
    let inner = p.sketches[si].rects[1].corners[0];
    let (x, y) = at(&p, inner);
    let out: Vec<(u64, f64, f64)> = [0.0, 20.0, 45.0, 70.0, 45.0, 0.0].iter().map(|dy| (inner, x, y + dy)).collect();
    both_ways(&p, &out, "a rectangle inside led out and back", &mut failures);
    // a corner of the frame drawn over the rectangles beside it
    let corner = p.sketches[si].rects[0].corners[2];
    let (cx, cy) = at(&p, corner);
    let over: Vec<(u64, f64, f64)> = [10.0, 40.0, 80.0, 40.0].iter().map(|dx| (corner, cx + dx, cy)).collect();
    both_ways(&p, &over, "the frame drawn over the rectangles beside it", &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
