//! A CIRCLE TOUCHING TWO CURVES HAS ITS RADIUS: two lines crossing, a line and a circle, two circles apart, a spline and a
//! line - a circle of radius 3 asked for near a point between the two is laid touching both, of radius 3, its centre on
//! the side of the point, and it stays so once the sketch is solved.
use qymcad_core::feature::{Ends, Purpose};
use qymcad_core::geom::Point2;
use qymcad_core::model::curve::Curve;
use qymcad_core::model::{Id, Project, Touching};

/// THE TWO CURVES A CIRCLE TOUCHES.
#[derive(Clone, Copy, Debug)]
enum Two {
    Lines,
    LineAndCircle,
    Circles,
    SplineAndLine,
}

/// A SKETCH WITH TWO CURVES, by their ids.
struct Drawn {
    p: Project,
    curves: [Id; 2],
}

fn drawn(two: Two) -> Drawn {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    let curves = match two {
        Two::Lines => [p.add_line_entity(si, 0.0, 0.0, 40.0, 0.0, Purpose::Real), p.add_line_entity(si, 0.0, 0.0, 0.0, 40.0, Purpose::Real)],
        Two::LineAndCircle => [p.add_line_entity(si, 0.0, 0.0, 40.0, 0.0, Purpose::Real), p.add_circle_entity(si, 20.0, 12.0, 8.0, Purpose::Real)],
        Two::Circles => [p.add_circle_entity(si, 0.0, 0.0, 8.0, Purpose::Real), p.add_circle_entity(si, 20.0, 0.0, 8.0, Purpose::Real)],
        Two::SplineAndLine => {
            let spline = p.add_spline(si, vec![Point2::new(0.0, 5.0), Point2::new(15.0, 8.0), Point2::new(30.0, 5.0)], Ends::Open, Purpose::Real).expect("the spline");
            [spline, p.add_line_entity(si, 0.0, 0.0, 40.0, 0.0, Purpose::Real)]
        }
    };
    Drawn { p, curves }
}

/// Where the centre is asked for: between the two.
fn near(two: Two) -> Point2 {
    match two {
        Two::Lines => Point2::new(4.0, 4.0),
        Two::LineAndCircle => Point2::new(20.0, 3.0),
        Two::Circles => Point2::new(10.0, 4.0),
        Two::SplineAndLine => Point2::new(15.0, 4.0),
    }
}

#[test]
fn a_circle_touching_two_curves_is_laid_with_its_radius() {
    let mut failures = Vec::new();
    for two in [Two::Lines, Two::LineAndCircle, Two::Circles, Two::SplineAndLine] {
        let Drawn { mut p, curves } = drawn(two);
        let ask = Touching { curves, r: 3.0, near: near(two) };
        let Some(eid) = p.add_circle_touching(0, ask, Purpose::Real) else {
            failures.push(format!("{two:?}: no circle of radius 3 touches both"));
            continue;
        };
        let s = &p.sketches[0];
        let Some(Curve::Round { centre, r, .. }) = s.entity(eid).and_then(|e| Curve::of(s, e)) else {
            failures.push(format!("{two:?}: what was laid is no circle"));
            continue;
        };
        let off: Vec<f64> = curves.iter().filter_map(|&id| s.entity(id).and_then(|e| Curve::of(s, e))).map(|c| c.nearest(centre).off).collect();
        if (r - 3.0).abs() > 1e-6 || off.len() != 2 || off.iter().any(|d| (d - 3.0).abs() > 1e-4) || centre.y <= 0.0 {
            failures.push(format!("{two:?}: radius {r}, the centre {centre:?} stands {off:?} from the two, want 3 from each, above the line as {:?} is", ask.near));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
