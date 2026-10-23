//! A SPLINE IS ONE B-SPLINE: the curve of a spline of the sketch, laid as a cubic B-spline (`bspline::BSpline`), runs
//! where the spline runs at every place, keeps one knot a node where it bends smoothly through it and two where a turned
//! handle holds the node; a knot inserted leaves the curve where it was, a piece cut out of it runs where the curve runs
//! between the ends of the piece, and the curve fitted to its own places on its own knots is the curve again - an open
//! one through its ends, a closed one round its start.
use qymcad_core::feature::Ends;
use qymcad_core::geom::Point2;
use qymcad_core::geom::bspline::{BSpline, Sample};
use qymcad_core::model::curve::Curve;
use qymcad_core::model::Handle;

/// A SPLINE TO LAY: its name, its nodes, its ends, the node whose handle is turned if any.
struct Case {
    name: &'static str,
    nodes: &'static [[f64; 2]],
    ends: Ends,
    turned: Option<usize>,
}

const CASES: [Case; 3] = [
    Case { name: "an open spline of six nodes", nodes: &[[0.0, 0.0], [10.0, 6.0], [22.0, -3.0], [31.0, 8.0], [40.0, 2.0], [52.0, 10.0]], ends: Ends::Open, turned: None },
    Case { name: "a closed spline of five nodes", nodes: &[[0.0, 0.0], [14.0, -4.0], [24.0, 6.0], [12.0, 17.0], [-3.0, 10.0]], ends: Ends::Closed, turned: None },
    Case { name: "an open spline with the handle of a node turned", nodes: &[[0.0, 0.0], [10.0, 6.0], [22.0, -3.0], [31.0, 8.0], [40.0, 2.0]], ends: Ends::Open, turned: Some(2) },
];

/// The farthest the B-spline stands from the curve, over 2 000 places, and over each run of the parameter of `b` that
/// `map` takes a place of the curve to.
fn apart(c: &Curve, b: &BSpline, map: &dyn Fn(f64) -> Option<f64>) -> f64 {
    (0..=2000)
        .filter_map(|k| {
            let t = k as f64 / 2000.0;
            map(t).map(|u| c.at(t).dist(b.at(u)))
        })
        .fold(0.0, f64::max)
}

#[test]
fn a_spline_is_one_b_spline() {
    let mut failures = Vec::new();
    for Case { name, nodes, ends, turned } in CASES {
        let pts: Vec<Point2> = nodes.iter().map(|q| Point2::new(q[0], q[1])).collect();
        let mut handles = vec![Handle::default(); pts.len()];
        if let Some(k) = turned {
            handles[k] = Handle { arrive: Some([12.0, 9.0]), leave: Some([12.0, 9.0]) };
        }
        let c = Curve::spline(pts.clone(), &handles, ends);
        let Some(b) = c.b_spline().cloned() else {
            failures.push(format!("{name}: no B-spline"));
            continue;
        };
        let whole = apart(&c, &b, &|t| Some(t));
        if whole > 1e-9 {
            failures.push(format!("{name}: the B-spline stands {whole:.2e} off the curve"));
        }
        // one knot a node inside it, two at a node a handle holds
        let inner = if ends == Ends::Closed { pts.len() - 1 } else { pts.len() - 2 };
        let want = inner + usize::from(turned.is_some());
        let have = b.knots.len() - 8;
        if have != want {
            failures.push(format!("{name}: {have} inner knots, not {want}: {:?}", b.knots));
        }
        let mut inserted = b.clone();
        inserted.insert(0.37, 2);
        let moved = apart(&c, &inserted, &|t| Some(t));
        if moved > 1e-9 {
            failures.push(format!("{name}: a knot inserted twice moved the curve by {moved:.2e}"));
        }
        // fitted to 8 places a span of itself, on its own knots
        if turned.is_none() {
            let mut grid = vec![0.0];
            grid.extend(b.inner_knots());
            grid.push(1.0);
            let mut samples: Vec<Sample> = grid.windows(2).flat_map(|w| (0..8).map(move |j| w[0] + (w[1] - w[0]) * j as f64 / 8.0)).map(|u| Sample { u, at: b.at(u) }).collect();
            if ends == Ends::Open {
                samples.push(Sample { u: 1.0, at: b.at(1.0) });
            }
            match BSpline::fit(&samples, &b.inner_knots(), ends) {
                Some(fit) => {
                    let off = apart(&c, &fit, &|t| Some(t));
                    if off > 1e-7 {
                        failures.push(format!("{name}: fitted to its own places it stands {off:.2e} off itself"));
                    }
                }
                None => failures.push(format!("{name}: no fit to its own places")),
            }
        }
        let (from, to) = (0.21, 0.83);
        let piece = b.piece([from, to]);
        let off = apart(&c, &piece, &|t| (from..=to).contains(&t).then(|| (t - from) / (to - from)));
        if off > 1e-9 {
            failures.push(format!("{name}: the piece from {from} to {to} stands {off:.2e} off the curve"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A PIECE OF A LOOP FROM ITS VERY END ROUND ITS START is the run past the start, with nothing before it: the share of
/// the joint is 0. Reported behaviour: the knot of the joint stood seven times at 0, taking it away subtracted past zero,
/// and a point clicked on a closed spline closed the program.
#[test]
fn a_piece_of_a_loop_from_its_end_round_its_start_stands() {
    let nodes: Vec<Point2> = CASES[1].nodes.iter().map(|p| Point2::new(p[0], p[1])).collect();
    let c = Curve::spline(nodes, &[], Ends::Closed);
    let piece = c.piece([1.0, 1.25]);
    let off = (0..=200).map(|k| k as f64 / 200.0).map(|t| piece.at(t).dist(c.at(t * 0.25))).fold(0.0, f64::max);
    assert!(off < 1e-6, "the piece from 1 to 1.25 stands {off:.2e} off the quarter after the start");
}
