//! AN OFFSET COPY STAYS AN OFFSET, on the drawing of a report: a closed spline with a notch, offset inwards - the copy in
//! pieces where the loops of the true offset are cut away. A node of the source dragged: every place along every copy
//! stands the distance off the source, no piece reaching to a point of another. A spline across a copy and a fillet at
//! the crossing: done in a moment, the copy cut as any spline is.
//!
//! Reported behaviour: a node of the source dragged, the copies came apart - a piece ran to a corner of another; a
//! fillet where another spline crossed the copy hung the program.
use qymcad_core::feature::{Ends, Purpose};
use qymcad_core::geom::Point2;
use qymcad_core::model::curve::Curve;
use qymcad_core::model::{CornerAt, CornerCut, FilletSize, Id, Project};
use qymcad_core::time_budget::{cost_of, over, Doer};

/// The blob of the report: seven nodes, a notch between the second and the fourth.
const BLOB: [[f64; 2]; 7] = [[-22.0, 6.0], [-14.0, 14.0], [-3.0, 7.0], [7.0, 25.0], [21.0, 9.0], [10.0, -9.0], [-17.0, -14.0]];

/// A BLOB AND ITS OFFSET: the drawing, its sketch and the blob.
struct Drawn {
    p: Project,
    si: usize,
    blob: Id,
}

/// The blob drawn and offset by `d` (inwards for a negative one).
fn drawn(d: f64) -> Drawn {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    let blob = p.add_spline(si, BLOB.iter().map(|q| Point2::new(q[0], q[1])).collect(), Ends::Closed, Purpose::Real).expect("the blob");
    p.solve_sketch(si);
    assert!(p.offset_entities(si, &[blob], d) > 0, "setup: the blob is not offset");
    Drawn { p, si, blob }
}

/// How far from `d` the copies stand off the blob, the worst of 400 places along each.
fn worst_off(p: &Project, si: usize, blob: Id, d: f64) -> f64 {
    let s = &p.sketches[si];
    let source = s.entity(blob).and_then(|e| Curve::of(s, e)).expect("the blob");
    s.entities
        .iter()
        .filter(|e| e.id != blob)
        .filter_map(|e| Curve::of(s, e))
        .flat_map(|c| (0..=400).map(move |k| c.at(k as f64 / 400.0)).collect::<Vec<_>>())
        .map(|q| (source.nearest(q).off - d.abs()).abs())
        .fold(0.0, f64::max)
}

#[test]
fn the_copies_of_a_dragged_source_stay_its_offset() {
    let mut failures = Vec::new();
    for d in [2.0, -2.0] {
        let Drawn { mut p, si, blob } = drawn(d);
        let before = worst_off(&p, si, blob, d);
        // the node at the top of the notch dragged 2 down and let go
        let node = p.sketches[si].entity(blob).and_then(|e| e.kind.as_spline()).map(|sp| sp.nodes[3]).expect("the node");
        let q = p.sketches[si].point(node).map(|q| Point2::new(q.x, q.y)).expect("its place");
        p.solve_sketch_drag(si, Some((node, q.x, q.y - 2.0)));
        p.solve_sketch(si);
        let after = worst_off(&p, si, blob, d);
        // a spline through points of the true offset keeps near it - 0.09 next to the corner a loop was cut away at, as
        // laid; a piece run off to another stands millimetres off
        if before > 0.15 || after > 0.15 {
            failures.push(format!("offset by {d}: the copies stand up to {before:.3} off the distance as laid, {after:.3} after a node of the source was dragged"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_fillet_where_a_spline_crosses_a_copy_is_done_in_a_moment() {
    let Drawn { mut p, si, blob } = drawn(3.0);
    // a spline from outside across the copy and the blob
    let across = p.add_spline(si, vec![Point2::new(-40.0, 0.0), Point2::new(-30.0, 2.0), Point2::new(-15.0, -1.0)], Ends::Open, Purpose::Real).expect("the spline across");
    let copy = p.sketches[si].entities.iter().find(|e| e.id != blob && e.id != across && e.kind.as_spline().is_some()).map(|e| e.id).expect("the copy");
    let done = cost_of(Doer::Thread, || {
        let crossing = p.crossing_of(si, [copy, across], Point2::new(-27.0, 1.0)).expect("the spline crosses the copy");
        let point = p.cut_at_crossing(si, crossing).expect("the crossing cut");
        // the corner of the piece of the spline outside and the piece of the copy going up
        let s = &p.sketches[si];
        let at = |e: &qymcad_core::model::SketchEntity| Curve::of(s, e).map(|c| c.at(0.5));
        let ends_there: Vec<&qymcad_core::model::SketchEntity> = s.entities.iter().filter(|e| qymcad_core::model::entity_ends(e).is_some_and(|ends| ends.contains(&point))).collect();
        let outside = ends_there.iter().filter(|e| at(e).is_some_and(|m| m.x < -28.0)).map(|e| e.id).next();
        let up = ends_there.iter().filter(|e| at(e).is_some_and(|m| m.y > 1.0 && m.x > -28.0)).map(|e| e.id).next();
        let (Some(outside), Some(up)) = (outside, up) else { panic!("no corner of the crossing: the curves ending there are {:?}", ends_there.iter().map(|e| e.id).collect::<Vec<_>>()) };
        p.cut_corner_set(si, &[CornerAt::of(point, [outside, up])], CornerCut::Fillet(FilletSize::radius(2.0)), None)
    });
    let budget = std::time::Duration::from_secs(2);
    assert!(done.cost.work < budget, "{}", over("the fillet at the crossing of a spline and a copy", done.cost, budget));
    let made = done.out;
    assert_eq!(made, 1, "the fillet at the crossing of a spline and a copy was not laid");
}

/// The closed spline of a second report, twelve nodes some 110 mm across, its offset 3 inwards in three pieces.
const BIRD: [[f64; 2]; 12] = [
    [75.8, -17.8],
    [57.0, -22.8],
    [23.6, -29.1],
    [15.0, -52.7],
    [46.1, -59.2],
    [43.4, -77.3],
    [68.0, -91.5],
    [93.9, -88.9],
    [88.8, -60.5],
    [123.7, -51.4],
    [106.9, -24.2],
    [82.3, -34.6],
];

/// A NODE OF THE SOURCE LED IN FRAMES WHILE THE OFFSET CHANGES ITS PIECES: dragged, the inward offset of the spline of the
/// report parts in four pieces and in five at the folds the drag brings in. Every frame, every place along every copy
/// stands the distance off the source, and the offset keeps its dimension.
///
/// Reported behaviour: a closed spline offset inwards, a node of the source dragged - the copy fell apart, straight
/// spans running across the drawing.
#[test]
fn the_copies_hold_while_a_drag_changes_the_pieces_of_the_offset() {
    let mut failures = Vec::new();
    for node_k in 2..=5 {
        for way in [[0.0, 1.0], [1.0, 0.0], [0.0, -1.0], [-1.0, 0.0]] {
            let mut p = Project::default();
            p.new_document();
            let si = p.new_sketch("S");
            let bird = p.add_spline(si, BIRD.iter().map(|q| Point2::new(q[0], q[1])).collect(), Ends::Closed, Purpose::Real).expect("the spline");
            p.solve_sketch(si);
            assert!(p.offset_entities(si, &[bird], -3.0) > 0, "setup: the spline is not offset");
            let node = p.sketches[si].entity(bird).and_then(|e| e.kind.as_spline()).map(|sp| sp.nodes[node_k]).expect("the node");
            let q = p.sketches[si].point(node).map(|q| Point2::new(q.x, q.y)).expect("its place");
            for frame in 1..=20 {
                let t = frame as f64 * 0.3;
                p.solve_sketch_drag(si, Some((node, q.x + way[0] * t, q.y + way[1] * t)));
                let off = worst_off(&p, si, bird, 3.0);
                let s = &p.sketches[si];
                let held = s
                    .offsets
                    .first()
                    .and_then(|o| o.copies.first())
                    .is_some_and(|first| s.constraints.iter().any(|c| matches!(c, qymcad_core::model::Constraint::CurveDistance { to, .. } if to.curve_id() == Some(*first))));
                if off > 0.15 || !held {
                    failures.push(format!(
                        "node {node_k} led {way:?}, frame {frame}: the copies stand up to {off:.3} off the distance, the dimension of the offset {}",
                        if held { "kept" } else { "lost" }
                    ));
                    break;
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
