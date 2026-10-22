//! A CORNER IS A POINT AND TWO CURVES: two circles crossing, cut where they cross, are four arcs every two of which share
//! both their ends - a corner at each of the two crossings. Every corner at the crossing cut is rounded R2 and chamfered
//! 2, each on a copy of the drawing, where it was named: the arc or the cut line stands where the preview of that corner
//! drew it. A circle crossed by an arc is asked the same.
//!
//! Reported behaviour: the fillet tool at the crossing of two circles drew a strange preview and refused the value as one
//! that does not fit the sides of the corner.
use qymcad_core::feature::{Purpose, Winding};
use qymcad_core::geom::Point2;
use qymcad_core::model::curve::Curve;
use qymcad_core::model::{ChamferLegs, CornerAt, CornerCut, FilletSize, Id, Project};

/// THE CURVE THAT CROSSES THE CIRCLE about (20, 20) of radius 10.
#[derive(Clone, Copy, Debug)]
enum Other {
    /// The circle about (32, 20) of radius 10.
    Circle,
    /// The left half of that circle, from (32, 30) round to (32, 10).
    Arc,
}

/// A DRAWING CUT AT A CROSSING, and the point of the cut.
struct Cut {
    p: Project,
    at: Id,
}

/// The drawing cut at the crossing near (26, 28).
fn cut(other: Other) -> Cut {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    p.add_circle_entity(si, 20.0, 20.0, 10.0, Purpose::Real);
    match other {
        Other::Circle => {
            p.add_circle_entity(si, 32.0, 20.0, 10.0, Purpose::Real);
        }
        Other::Arc => p.add_arc_entity(si, Point2::new(32.0, 20.0), Point2::new(32.0, 30.0), Point2::new(32.0, 10.0), Winding::Ccw, Purpose::Real),
    }
    let crossing = p.crossing_near(si, Point2::new(26.0, 28.5), 2.0).expect("the crossing");
    let at = p.cut_at_crossing(si, crossing).expect("the point of the cut");
    Cut { p, at }
}

/// The centre a curve of the sketch runs round.
fn centre_of(p: &Project, eid: Id) -> Option<Id> {
    p.sketches[0].entity(eid).and_then(|e| e.kind.as_arc()).map(|a| a.center)
}

/// Whether the drawing holds a curve of the kind `cut` lays - an arc for a fillet, a line for a chamfer - running between
/// `ends`.
fn laid(p: &Project, ends: [[f64; 2]; 2], cut: CornerCut) -> bool {
    let s = &p.sketches[0];
    let at = |q: [f64; 2]| Point2::new(q[0], q[1]);
    s.entities.iter().filter(|e| if matches!(cut, CornerCut::Fillet(_)) { e.kind.as_arc().is_some() } else { e.kind.as_line().is_some() }).filter_map(|e| Curve::of(s, e)).any(|c| {
        let [a, b] = c.ends();
        (a.dist(at(ends[0])) < 1e-6 && b.dist(at(ends[1])) < 1e-6) || (a.dist(at(ends[1])) < 1e-6 && b.dist(at(ends[0])) < 1e-6)
    })
}

#[test]
fn every_corner_at_a_crossing_of_round_curves_is_cut_where_it_was_named() {
    let mut failures = Vec::new();
    for other in [Other::Circle, Other::Arc] {
        let Cut { p, at } = cut(other);
        let edges = p.vertex_edges(0, at);
        // a corner is two curves of different circles; two halves of one circle go on smoothly
        let corners: Vec<CornerAt> =
            edges.iter().enumerate().flat_map(|(i, &a)| edges[i + 1..].iter().map(move |&b| CornerAt::of(at, [a, b]))).filter(|c| centre_of(&p, c.pair.0) != centre_of(&p, c.pair.1)).collect();
        if corners.len() != 4 {
            failures.push(format!("{other:?}: {} corners at the crossing, want 4", corners.len()));
        }
        for corner in corners {
            let pair = corner.pair;
            for cut in [CornerCut::Fillet(FilletSize::radius(2.0)), CornerCut::Chamfer(ChamferLegs::equal(2.0))] {
                let mut q = p.clone();
                let Some(shown) = q.corner_blend(0, at, pair, cut) else {
                    failures.push(format!("{other:?} {pair:?} {cut:?}: no preview"));
                    continue;
                };
                if q.cut_corner_set(0, &[corner], cut, None) != 1 {
                    failures.push(format!("{other:?} {pair:?} {cut:?}: not cut"));
                    continue;
                }
                if !laid(&q, shown.ends, cut) {
                    failures.push(format!("{other:?} {pair:?} {cut:?}: nothing laid between the ends the preview drew, {:?}", shown.ends));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// THE CURVES OF A CROSSING, for the preview before the cut.
#[derive(Clone, Copy, Debug)]
enum Pair {
    /// The circle about (20, 20) and the curve crossing it.
    RoundAnd(Other),
    /// A line from (10, 34) to (40, 22) across the circle about (20, 20).
    LineAndCircle,
    /// Two lines crossing at (26, 28).
    Lines,
}

/// The drawing of `pair`, uncut.
fn drawn(pair: Pair) -> Project {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    match pair {
        Pair::RoundAnd(other) => {
            p.add_circle_entity(si, 20.0, 20.0, 10.0, Purpose::Real);
            match other {
                Other::Circle => {
                    p.add_circle_entity(si, 32.0, 20.0, 10.0, Purpose::Real);
                }
                Other::Arc => p.add_arc_entity(si, Point2::new(32.0, 20.0), Point2::new(32.0, 30.0), Point2::new(32.0, 10.0), Winding::Ccw, Purpose::Real),
            }
        }
        Pair::LineAndCircle => {
            p.add_circle_entity(si, 20.0, 20.0, 10.0, Purpose::Real);
            p.add_line_entity(si, 10.0, 34.0, 40.0, 22.0, Purpose::Real);
        }
        Pair::Lines => {
            p.add_line_entity(si, 10.0, 28.0, 40.0, 28.0, Purpose::Real);
            p.add_line_entity(si, 20.0, 14.0, 32.0, 42.0, Purpose::Real);
        }
    }
    p
}

#[test]
fn the_preview_of_a_corner_at_a_crossing_is_the_corner_cut_there() {
    let mut failures = Vec::new();
    for pair in [Pair::RoundAnd(Other::Circle), Pair::RoundAnd(Other::Arc), Pair::LineAndCircle, Pair::Lines] {
        let p = drawn(pair);
        let Some(crossing) = p.crossing_near(0, Point2::new(26.0, 29.0), 3.0) else {
            failures.push(format!("{pair:?}: no crossing near (26, 29)"));
            continue;
        };
        // the pointer 1.5 mm off the crossing in each quarter
        for turn in [1.0_f64, 3.0, 5.0, 7.0] {
            let a = turn * std::f64::consts::FRAC_PI_4;
            let toward = Point2::new(crossing.at.x + 1.5 * a.cos(), crossing.at.y + 1.5 * a.sin());
            for cut in [CornerCut::Fillet(FilletSize::radius(1.0)), CornerCut::Chamfer(ChamferLegs::equal(1.0))] {
                let Some(shown) = p.crossing_blend(0, crossing, toward, cut) else {
                    failures.push(format!("{pair:?} toward {toward:?} {cut:?}: no preview before the cut"));
                    continue;
                };
                let mut q = p.clone();
                let Some(at) = q.cut_at_crossing(0, crossing) else {
                    failures.push(format!("{pair:?}: the crossing was not cut"));
                    continue;
                };
                let Some(sides) = q.vertex_pair(0, at, Some((toward.x, toward.y))) else {
                    failures.push(format!("{pair:?} toward {toward:?}: no corner after the cut"));
                    continue;
                };
                if q.cut_corner_set(0, &[CornerAt::of(at, [sides.0, sides.1])], cut, None) != 1 || !laid(&q, shown.ends, cut) {
                    failures.push(format!("{pair:?} toward {toward:?} {cut:?}: the corner cut is not the one the preview drew, {:?}", shown.ends));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
