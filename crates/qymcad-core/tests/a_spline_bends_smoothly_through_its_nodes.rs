//! A SPLINE BENDS SMOOTHLY THROUGH ITS NODES: the curve through the nodes of a spline turns at each node as it turns on
//! either side of it - its curvature runs on (C2), as a spline through points does in the sketchers a person comes from;
//! an open one runs straight out of its free ends. The curvature is read as the turn of the way per length, whatever
//! share of the parameter of the curve each span takes. A node whose handle a person turned is held by it, the bend changing
//! there alone.
//!
//! Reported behaviour: the sketcher made a spline smooth to the tangent only; its curvature jumped at every node, and a
//! fillet along the side of a body swept from it was refused at any radius.
use qymcad_core::feature::{Ends, Purpose};
use qymcad_core::geom::Point2;
use qymcad_core::model::curve::Curve;
use qymcad_core::model::{Id, Project};

/// The curve of the spline `eid` of sketch `si`.
fn curve(p: &Project, si: usize, eid: Id) -> Curve {
    let s = &p.sketches[si];
    s.entity(eid).and_then(|e| Curve::of(s, e)).expect("the spline")
}

/// WHICH SIDE OF A PLACE a curvature is read on.
#[derive(Clone, Copy)]
enum Side {
    Before,
    After,
}

/// THE CURVATURE OF CURVE `c` AT `t`, read on the side `side`: the turn of its way per length, signed.
fn bend(c: &Curve, t: f64, side: Side) -> f64 {
    // read a little way in, the change of the way taken across that place
    let h = match side {
        Side::Before => -1e-6,
        Side::After => 1e-6,
    };
    let (v, a) = (c.velocity(t + 2.0 * h), (c.velocity(t + 3.0 * h) - c.velocity(t + h)) * (0.5 / h));
    (v.x * a.y - v.y * a.x) / v.x.hypot(v.y).powi(3)
}

/// How far the curvature before the place `t` stands from the curvature after it.
fn jump(c: &Curve, t: f64) -> f64 {
    (bend(c, t, Side::After) - bend(c, t, Side::Before)).abs()
}

/// A SPLINE TO DRAW: its name, its nodes, its ends, the node whose handle is turned if any.
struct Case {
    name: &'static str,
    nodes: &'static [[f64; 2]],
    ends: Ends,
    turned: Option<usize>,
}

const CASES: [Case; 3] = [
    Case { name: "an open spline of six nodes", nodes: &[[0.0, 0.0], [10.0, 6.0], [22.0, -3.0], [31.0, 8.0], [40.0, 2.0], [52.0, 10.0]], ends: Ends::Open, turned: None },
    Case { name: "a closed spline of five nodes", nodes: &[[0.0, 0.0], [14.0, -4.0], [24.0, 6.0], [12.0, 17.0], [-3.0, 10.0]], ends: Ends::Closed, turned: None },
    Case { name: "a closed spline with the handle of a node turned", nodes: &[[0.0, 0.0], [14.0, -4.0], [24.0, 6.0], [12.0, 17.0], [-3.0, 10.0]], ends: Ends::Closed, turned: Some(2) },
];

#[test]
fn a_spline_bends_smoothly_through_its_nodes() {
    let mut failures = Vec::new();
    for Case { name, nodes, ends, turned } in CASES {
        let mut p = Project::default();
        p.new_document();
        let si = p.new_sketch(name);
        let eid = p.add_spline(si, nodes.iter().map(|q| Point2::new(q[0], q[1])).collect(), ends, Purpose::Real).expect("the spline");
        if let Some(k) = turned {
            let at = Point2::new(nodes[k][0], nodes[k][1]);
            assert!(p.set_spline_handle(0, eid, k, at + Point2::new(-3.0, 8.0)), "setup: the handle is not turned");
        }
        let c = curve(&p, si, eid);
        // the nodes inside the curve stand at its knots, node k at the k-th
        let knots = c.b_spline().map(|b| b.inner_knots()).unwrap_or_default();
        if knots.len() + 2 != nodes.len() + usize::from(ends == Ends::Closed) {
            failures.push(format!("{name}: {} knots inside the curve for {} nodes", knots.len(), nodes.len()));
        }
        for (i, t) in knots.iter().enumerate() {
            let k = i + 1;
            let j = jump(&c, *t);
            if Some(k) != turned && j > 1e-4 {
                failures.push(format!("{name}: the curvature jumps by {j:.5} at node {k}"));
            }
        }
        if ends == Ends::Open {
            // straight out of its free ends: no curvature there
            // its start read after it, its end before it
            for side in [Side::After, Side::Before] {
                let t = match side {
                    Side::After => 0.0,
                    Side::Before => 1.0,
                };
                let k = bend(&c, t, side).abs();
                if k > 1e-4 {
                    failures.push(format!("{name}: its curvature is {k:.5} at its end {t}"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
