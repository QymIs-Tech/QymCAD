//! AN OFFSET OF A SPLINE THAT TURNS BACK ON ITSELF IS LIGHT: a spline from (20, 0) to (10, 10) and back to (20, 0) runs
//! the same way twice, stopping dead at its middle, and its offset turns half a turn there. Offset 3 either way, it is laid
//! in well under a second.
//!
//! Reported behaviour: every corner of such a spline rounded, the frame took 15 s - the fit of the offset cut its spans at
//! the turn again and again, the error standing at a jump no cubic follows.
use qymcad_core::feature::Ends;
use qymcad_core::geom::Point2;
use qymcad_core::model::curve::Curve;
use qymcad_core::time_budget::{cost_of, Doer};

#[test]
fn an_offset_of_a_spline_turning_back_is_laid_light() {
    let spline = Curve::spline(vec![Point2::new(20.0, 0.0), Point2::new(10.0, 10.0), Point2::new(20.0, 0.0)], &[], Ends::Open);
    let mut failures = Vec::new();
    for d in [3.0, -3.0] {
        let laid = cost_of(Doer::Thread, || spline.offset(d));
        let (pieces, took) = (laid.out, laid.cost);
        if pieces.is_empty() || took.work > std::time::Duration::from_secs(1) {
            failures.push(format!("offset by {d}: {} pieces, laid in {took}", pieces.len()));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
