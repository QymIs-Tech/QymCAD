//! A BODY OF A LARGE SPLINE BUILDS AND ROUNDS LIGHTLY: a closed spline of 40 nodes some 500 mm across, waved in and out,
//! and its offset 5 inwards; the ring between them extruded 10 is a wall of a face for the spline, a face for each piece
//! of the offset and the two ends, and an edge of its top along the spline rounds with a radius of 1. The faces and the
//! rounding hold at every level; the time of each, at the level of the probes of time (`QYMCAD_TIER=time`), within its
//! budget: on a debug build the extrusion measured 0.25-0.30 s, the rounding 1.2-1.5 s, the work of the program on its
//! own threads (`Doer::Process`).
//!
//! Reported behaviour: such a ring extruded in 3 s into 303 faces, a face for each span of the curves, and a rounding of
//! its outer edge ran 11.6 s along 288 joints of them, then was refused.
use qymcad_core::feature::{Ends, Purpose, Reach};
use qymcad_core::geom::Point2;
use qymcad_core::model::curve::Curve;
use qymcad_core::model::{Id, Project};
use qymcad_core::time_budget::{cost_of, over, tell, within, Doer};
use std::time::Duration;

/// A LOOP OF THE SKETCH AND ITS AREA, for the one that holds the others.
struct Loop {
    id: Id,
    area: f64,
}

/// THE OUTER LOOP OF THE SKETCH `s`: the one of the largest area, which holds the others.
fn outer_loop(p: &Project, s: &qymcad_core::model::Sketch) -> Option<Id> {
    let loops = s.contour_ids.iter().copied().filter_map(|id| p.contour_index(id).map(|i| Loop { id, area: p.contours[i].area().abs() }));
    loops.max_by(|a, b| a.area.total_cmp(&b.area)).map(|l| l.id)
}

/// The budget of the extrusion of the ring.
const EXTRUDED_IN: Duration = Duration::from_millis(1_000);
/// The budget of the rounding of its edge.
const ROUNDED_IN: Duration = Duration::from_millis(3_000);

/// THE NODES OF THE SPLINE: 40 places round a circle of 200, waved by 40 five times and by 10 nine times; its bend
/// inwards is as tight as 13.7 mm, so an offset 5 inwards keeps every piece.
fn nodes() -> Vec<Point2> {
    (0..40)
        .map(|k| {
            let t = std::f64::consts::TAU * k as f64 / 40.0;
            let r = 200.0 + 40.0 * (5.0 * t).sin() + 10.0 * (9.0 * t).cos();
            Point2::new(r * t.cos(), r * t.sin())
        })
        .collect()
}

#[test]
fn a_ring_of_a_large_spline_extrudes_and_rounds_lightly() {
    let mut p = Project::default();
    p.new_document();
    let c = p.add_part("A");
    p.set_active_component(Some(c));
    let si = p.new_sketch("s");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "s");
    let spline = p.add_spline(si, nodes(), Ends::Closed, Purpose::Real).expect("setup: the spline");
    assert!(p.offset_entities(si, &[spline], -5.0) > 0, "setup: the spline is not offset");
    p.solve_sketch(si);
    p.regen_sketch(si);
    let s = &p.sketches[si];
    let source = Curve::of(s, s.entity(spline).expect("the spline")).expect("its curve");
    let pieces = s.entities.iter().filter(|e| e.id != spline && e.kind.as_spline().is_some()).count();
    // the ring is the region of the loop that holds the other
    let outer = outer_loop(&p, s);
    let body: Id = p.add_extrude_on(sid, outer.expect("setup: the loops"), 10.0, Reach::Forward, 0.0);
    p.finish_base_body(body, 1);
    let mut failures = Vec::new();

    let extruded = cost_of(Doer::Process, || qymcad_testkit::regenerate(&mut p));
    tell("the ring extruded", extruded.cost);
    assert!(extruded.out.0.errors.is_empty(), "the ring did not extrude: {:?}", extruded.out.0.errors);
    if !within(extruded.cost.work, EXTRUDED_IN) {
        failures.push(over("the ring extruded", extruded.cost, EXTRUDED_IN));
    }
    let faces = p.regen_faces.get(&body).map_or(0, Vec::len);
    if faces != 1 + pieces + 2 {
        failures.push(format!("the ring extruded is {faces} faces, not the spline, the {pieces} pieces of its offset and two ends"));
    }

    // the edges of the top along the spline: the middles of their polylines lie on it, within the sag of a polyline
    let rim: Vec<u32> = p.regen_edges.get(&body).into_iter().flatten().filter(|e| e.mid[2] > 9.9 && source.nearest(Point2::new(e.mid[0], e.mid[1])).off < 0.05).map(|e| e.id).collect();
    if rim.is_empty() {
        failures.push("no edge of the top lies along the spline".to_string());
    } else {
        p.add_fillet(body, 1.0, rim);
        let rounded = cost_of(Doer::Process, || qymcad_testkit::regenerate(&mut p));
        tell("the edge rounded", rounded.cost);
        if !rounded.out.0.errors.is_empty() {
            failures.push(format!("the edge along the spline did not round: {:?}", rounded.out.0.errors));
        }
        if !within(rounded.cost.work, ROUNDED_IN) {
            failures.push(over("the edge rounded", rounded.cost, ROUNDED_IN));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// THE BODY OF THE RING, ROUNDED, IS CHECKED SOUND ONCE: asked a second time whether it is a sound solid, it answers from
/// what it knows, with a tenth of the work of the first time at most. The check ran on every result of a blend three
/// times over - in the operation, in the wrap of its result, in the door - 100 ms a time on a body of 80 faces.
#[test]
fn the_ring_rounded_is_checked_sound_once() {
    let mut p = Project::default();
    p.new_document();
    let c = p.add_part("A");
    p.set_active_component(Some(c));
    let si = p.new_sketch("s");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "s");
    let spline = p.add_spline(si, nodes(), Ends::Closed, Purpose::Real).expect("setup: the spline");
    assert!(p.offset_entities(si, &[spline], -5.0) > 0, "setup: the spline is not offset");
    p.solve_sketch(si);
    p.regen_sketch(si);
    let s = &p.sketches[si];
    let source = Curve::of(s, s.entity(spline).expect("the spline")).expect("its curve");
    let outer = outer_loop(&p, s);
    let body: Id = p.add_extrude_on(sid, outer.expect("setup: the loops"), 10.0, Reach::Forward, 0.0);
    p.finish_base_body(body, 1);
    let _ = qymcad_testkit::regenerate(&mut p);
    let rim: Vec<u32> = p.regen_edges.get(&body).into_iter().flatten().filter(|e| e.mid[2] > 9.9 && source.nearest(Point2::new(e.mid[0], e.mid[1])).off < 0.05).map(|e| e.id).collect();
    assert!(!rim.is_empty(), "setup: no edge of the top lies along the spline");
    let rounded = p.add_fillet(body, 1.0, rim);
    let (rep, shapes) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "setup: the edge along the spline did not round: {:?}", rep.errors);
    // the body as it comes back from its bytes, as a trial build reads it: nothing known of it yet
    let bytes = shapes.get(&rounded).and_then(|s| s.to_brep_bytes()).expect("setup: the rounded ring has no bytes");
    let fresh = qymcad_kernel::specimen::read(&bytes).expect("setup: the rounded ring does not read back");
    let first = cost_of(Doer::Thread, || fresh.is_valid());
    let second = cost_of(Doer::Thread, || fresh.is_valid());
    tell("the rounded ring checked", first.cost);
    tell("the rounded ring checked again", second.cost);
    assert!(first.out && second.out, "the rounded ring is not a sound solid");
    assert!(second.cost.work * 10 <= first.cost.work, "the body checked sound was checked again: {:?} the first time, {:?} the second", first.cost.work, second.cost.work);
}
