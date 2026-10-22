//! A CONSTRAINT HOLDS TO A CURVE OF ANY KIND: a point put on a spline stands on it, and two curves made to touch -
//! a line, a circle, an ellipse, a spline, with one another - meet on themselves where they run the same way, once the
//! sketch is solved.
//!
//! Reported behaviour: a point could not be put on a spline, and a tangency took a line and a circle only.
use qymcad_core::feature::{Ends, Purpose};
use qymcad_core::geom::Point2;
use qymcad_core::model::curve::Curve;
use qymcad_core::model::{curve_ref_of, Constraint, Project};

fn p2(at: [f64; 2]) -> Point2 {
    Point2::new(at[0], at[1])
}

/// A CURVE LAID INTO A SKETCH by a case.
type Lay = fn(&mut Project, usize);

fn line_left(p: &mut Project, si: usize) {
    p.add_line_entity(si, 0.0, -10.0, 4.0, 10.0, Purpose::Real);
}
fn circle_left(p: &mut Project, si: usize) {
    p.add_circle_entity(si, 0.0, 0.0, 5.0, Purpose::Real);
}
fn ellipse_right(p: &mut Project, si: usize) {
    p.add_ellipse_entity(si, p2([18.0, 0.0]), 6.0, 3.0, 0.0, Purpose::Real);
}
fn ellipse_left(p: &mut Project, si: usize) {
    p.add_ellipse_entity(si, p2([0.0, 0.0]), 6.0, 3.0, 0.0, Purpose::Real);
}
fn spline_right(p: &mut Project, si: usize) {
    p.add_spline(si, vec![p2([14.0, -8.0]), p2([17.0, -2.0]), p2([16.0, 4.0]), p2([19.0, 9.0])], Ends::Open, Purpose::Real);
}
/// The shapes of the window's cells: an ellipse 8 by 4 about (28, 30), splines about (28, 30) and (53, 30), side by side.
fn wide_ellipse(p: &mut Project, si: usize) {
    p.add_ellipse_entity(si, p2([28.0, 30.0]), 8.0, 4.0, 0.0, Purpose::Real);
}
fn wavy_left(p: &mut Project, si: usize) {
    p.add_spline(si, vec![p2([20.0, 30.0]), p2([25.0, 35.0]), p2([31.0, 27.0]), p2([36.0, 32.0])], Ends::Open, Purpose::Real);
}
fn wavy_right(p: &mut Project, si: usize) {
    p.add_spline(si, vec![p2([46.0, 28.0]), p2([51.0, 34.0]), p2([56.0, 26.0]), p2([61.0, 31.0])], Ends::Open, Purpose::Real);
}
fn spline_left(p: &mut Project, si: usize) {
    p.add_spline(si, vec![p2([-2.0, -8.0]), p2([1.0, -2.0]), p2([0.0, 4.0]), p2([3.0, 9.0])], Ends::Open, Purpose::Real);
}

/// How far apart two curves stand where they come nearest, and the angle between their ways there in degrees: the place
/// of `b` nearest `a` of 2 000 along it, and the place of `a` nearest that.
fn touch([a, b]: [&Curve; 2]) -> [f64; 2] {
    let tb = (0..=2000).map(|k| k as f64 / 2000.0).min_by(|x, y| a.nearest(b.at(*x)).off.total_cmp(&a.nearest(b.at(*y)).off)).unwrap_or(0.0);
    let at = a.nearest(b.at(tb));
    let (ua, ub) = (a.tangent(at.t), b.tangent(tb));
    [at.off, (ua.x * ub.x + ua.y * ub.y).abs().min(1.0).acos().to_degrees()]
}

#[test]
fn a_point_put_on_a_spline_stands_on_it() {
    let mut p = Project::default();
    let si = p.new_sketch("on");
    line_left(&mut p, si);
    spline_right(&mut p, si);
    let s = &p.sketches[si];
    let end = s.points.iter().find(|q| (q.x - 4.0).abs() < 1e-9 && (q.y - 10.0).abs() < 1e-9).map(|q| q.id).expect("the end of the line");
    let on = curve_ref_of(s.entities.last().expect("the spline"));
    p.sketches[si].constraints.push(Constraint::PointOnCurve { p: end, on });
    let resid = p.solve_sketch(si);
    let s = &p.sketches[si];
    let spline = Curve::of(s, s.entities.last().expect("the spline")).expect("its curve");
    let q = s.points.iter().find(|q| q.id == end).map(|q| Point2::new(q.x, q.y)).expect("the end");
    let off = spline.nearest(q).off;
    assert!(off < 1e-6, "the end of the line stands {off:.6} off the spline; residual {resid:.2e}");
}

/// A PAIR OF CURVES MADE TO TOUCH.
struct Case {
    name: &'static str,
    first: Lay,
    second: Lay,
}

#[test]
fn every_pair_made_to_touch_meets_where_it_runs_the_same_way() {
    let cases = [
        Case { name: "a line and an ellipse", first: line_left, second: ellipse_right },
        Case { name: "a line and a spline", first: line_left, second: spline_right },
        Case { name: "a circle and an ellipse", first: circle_left, second: ellipse_right },
        Case { name: "an ellipse and a spline", first: ellipse_left, second: spline_right },
        Case { name: "two splines", first: spline_left, second: spline_right },
        Case { name: "an ellipse and a spline far apart", first: wide_ellipse, second: wavy_right },
        Case { name: "two splines far apart", first: wavy_left, second: wavy_right },
    ];
    let mut failures = Vec::new();
    for Case { name, first, second } in cases {
        let mut p = Project::default();
        let si = p.new_sketch(name);
        first(&mut p, si);
        second(&mut p, si);
        let s = &p.sketches[si];
        let n = s.entities.len();
        let made = p.tangency(si, [s.entities[0].id, s.entities[n - 1].id]);
        p.sketches[si].constraints.extend(made);
        let resid = p.solve_sketch(si);
        let s = &p.sketches[si];
        let (a, b) = (Curve::of(s, &s.entities[0]).expect("first"), Curve::of(s, &s.entities[n - 1]).expect("second"));
        let [apart, turn] = touch([&a, &b]);
        if apart > 0.02 || turn > 1.0 {
            failures.push(format!("{name}: {apart:.4} apart, ways {turn:.2} deg apart; residual {resid:.2e}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_touching_holds_while_a_curve_is_dragged() {
    let mut p = Project::default();
    let si = p.new_sketch("drag");
    p.add_circle_entity(si, 30.0, 30.0, 5.0, Purpose::Real);
    p.add_ellipse_entity(si, p2([54.0, 32.0]), 8.0, 4.0, 0.0, Purpose::Real);
    let s = &p.sketches[si];
    let ids = [s.entities[0].id, s.entities[1].id];
    let made = p.tangency(si, ids);
    p.sketches[si].constraints.extend(made);
    p.solve_sketch(si);
    let centre = |p: &Project| {
        let s = &p.sketches[si];
        match s.entities[1].kind.as_ellipse() {
            Some(qymcad_core::model::EllipseParts { c, .. }) => s.points.iter().find(|q| q.id == c).map(|q| (q.id, Point2::new(q.x, q.y))),
            _ => None,
        }
        .expect("the centre of the ellipse")
    };
    let (c, at) = centre(&p);
    // dragged 1.5 mm right and 1 up in six frames, as the window drags - each frame on the budget of a frame - and solved
    // whole when let go
    for k in 1..=6 {
        let f = k as f64 / 6.0;
        p.solve_sketch_drag_fast(si, Some((c, at.x + 1.5 * f, at.y + f)));
    }
    p.solve_sketch(si);
    let s = &p.sketches[si];
    let (a, b) = (Curve::of(s, &s.entities[0]).expect("circle"), Curve::of(s, &s.entities[1]).expect("ellipse"));
    let [apart, turn] = touch([&a, &b]);
    assert!(apart < 0.02 && turn < 1.0, "after the drag: {apart:.4} apart, ways {turn:.2} deg apart");
}

/// The least distance between two curves, read at 2 000 places along each.
fn least_apart([a, b]: [&Curve; 2]) -> f64 {
    (0..=2000).map(|k| a.nearest(b.at(k as f64 / 2000.0)).off).chain((0..=2000).map(|k| b.nearest(a.at(k as f64 / 2000.0)).off)).fold(f64::MAX, f64::min)
}

#[test]
fn a_distance_to_a_curve_of_any_kind_holds_its_value() {
    // a point, a circle, an arc, an ellipse, a spline: the least distance set to 4 from the curve laid first
    type Pair = [Lay; 2];
    fn point_left(p: &mut Project, si: usize) {
        p.sketch_point_at(si, 28.0, 30.0, 1e-9);
    }
    fn circle_at_left(p: &mut Project, si: usize) {
        p.add_circle_entity(si, 28.0, 30.0, 5.0, Purpose::Real);
    }
    fn spline_at_right(p: &mut Project, si: usize) {
        p.add_spline(si, vec![p2([44.0, 29.0]), p2([48.0, 34.0]), p2([52.0, 27.0]), p2([56.0, 32.0])], Ends::Open, Purpose::Real);
    }
    fn ellipse_at_right(p: &mut Project, si: usize) {
        p.add_ellipse_entity(si, p2([50.0, 30.0]), 7.0, 3.5, 0.0, Purpose::Real);
    }
    let cases: [Pair; 3] = [[circle_at_left, spline_at_right], [circle_at_left, ellipse_at_right], [point_left, spline_at_right]];
    let mut failures = Vec::new();
    for [first, second] in cases {
        let mut p = Project::default();
        let si = p.new_sketch("distance");
        first(&mut p, si);
        second(&mut p, si);
        let s = &p.sketches[si];
        let n = s.entities.len();
        let to = qymcad_core::model::curve_ref_drawn(&s.entities[n - 1]);
        let from = if n == 2 {
            qymcad_core::model::curve_ref_drawn(&s.entities[0])
        } else {
            qymcad_core::model::CurveRef::Point { p: s.points.iter().find(|q| (q.x - 28.0).abs() < 1e-9 && (q.y - 30.0).abs() < 1e-9).map(|q| q.id).expect("the point") }
        };
        p.sketches[si].constraints.push(Constraint::CurveDistance { from: from.clone(), to, d: 4.0, off: 0.0, expr: String::new(), driven: false, at: None });
        let resid = p.solve_sketch(si);
        let s = &p.sketches[si];
        let b = Curve::of(s, &s.entities[n - 1]).expect("second");
        let got = if n == 2 {
            least_apart([&Curve::of(s, &s.entities[0]).expect("first"), &b])
        } else {
            let CurveRefPoint(q) = point_of(s, &from);
            b.nearest(q).off
        };
        if (got - 4.0).abs() > 1e-3 {
            failures.push(format!("{:?}: the least distance is {got:.4}, not 4; residual {resid:.2e}", s.entities.iter().map(|e| Curve::of(s, e).map(|c| c.closed())).collect::<Vec<_>>()));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A point of the sketch where it stands.
struct CurveRefPoint(Point2);

fn point_of(s: &qymcad_core::model::Sketch, r: &qymcad_core::model::CurveRef) -> CurveRefPoint {
    let qymcad_core::model::CurveRef::Point { p } = r else { panic!("not a point") };
    let q = s.points.iter().find(|q| q.id == *p).expect("the point");
    CurveRefPoint(Point2::new(q.x, q.y))
}
