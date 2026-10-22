//! AN OFFSET OF A LARGE SPLINE IS LIGHT AND TRUE: a closed spline of 39 nodes some 650 mm across, its bends as tight as
//! 10 mm, offset 3 either way - laid in well under a second, a few hundred nodes in all, every place of every piece the
//! distance off the spline within the tolerance of an offset.
//!
//! Reported behaviour: a side of such an offset picked, the program hung for minutes, and the copy came out a spline of
//! 7 184 nodes, every frame after it slow.
use qymcad_core::feature::Ends;
use qymcad_core::geom::Point2;
use qymcad_core::model::curve::{Curve, OFFSET_TOLERANCE};
use qymcad_core::time_budget::{cost_of, Doer};

const NODES: [[f64; 2]; 39] = [
    [-62.0, 202.0],
    [-125.0, 161.0],
    [-159.0, 115.0],
    [-85.0, 98.0],
    [-163.0, 58.0],
    [-256.0, 59.0],
    [-228.0, -7.0],
    [-139.0, 39.0],
    [-136.0, -32.0],
    [-167.0, -95.0],
    [-83.0, -127.0],
    [-66.0, -64.0],
    [-26.0, -117.0],
    [88.0, -88.0],
    [48.0, -52.0],
    [131.0, -41.0],
    [158.0, -85.0],
    [229.0, -24.0],
    [132.0, 18.0],
    [181.0, 55.0],
    [112.0, 147.0],
    [62.0, 105.0],
    [60.0, 164.0],
    [155.0, 186.0],
    [83.0, 231.0],
    [27.0, 182.0],
    [21.0, 238.0],
    [-58.0, 240.0],
    [-174.0, 267.0],
    [-299.0, 247.0],
    [-422.0, 161.0],
    [-378.0, 85.0],
    [-304.0, 161.0],
    [-284.0, 212.0],
    [-197.0, 177.0],
    [-277.0, 112.0],
    [-196.0, 123.0],
    [-169.0, 191.0],
    [-98.0, 215.0],
];

#[test]
fn an_offset_of_a_large_spline_is_laid_light_and_true() {
    let spline = Curve::spline(NODES.iter().map(|q| Point2::new(q[0], q[1])).collect(), &[], Ends::Closed);
    let mut failures = Vec::new();
    for d in [3.0, -3.0] {
        let laid = cost_of(Doer::Thread, || spline.offset(d));
        let (pieces, took) = (laid.out, laid.cost);
        let nodes: usize = pieces.iter().filter_map(|p| p.handles()).map(|h| h.len()).sum();
        // a place of a piece off the distance by more than the tolerance, the worst of 2 000 along each
        let worst = pieces.iter().flat_map(|p| (0..=2000).map(move |k| p.at(k as f64 / 2000.0))).map(|q| (spline.nearest(q).off - 3.0).abs()).fold(0.0, f64::max);
        if pieces.is_empty() || nodes > 1000 || took.work > std::time::Duration::from_secs(1) || worst > OFFSET_TOLERANCE * 1.1 {
            failures.push(format!("offset by {d}: {} pieces of {nodes} nodes in all, laid in {took}, standing up to {worst:.4} off the distance", pieces.len()));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
