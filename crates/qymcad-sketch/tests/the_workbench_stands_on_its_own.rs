//! THE SKETCH WORKBENCH, CHECKED WITHOUT THE APPLICATION.
//!
//! The sketcher is the most entangled of the three - its context asks for 45 records - and until now that
//! entanglement was also what kept it unverifiable on its own: only `App` owned the records the borrows
//! point at, so reaching a rule about what is in hand meant building a window and running frames.
//!
//! `Bench` owns those records and ties the same `SketchCtx`. Nothing here is a mock: the document, the
//! tool and the selection are the real ones.

use qymcad_core::geom::Point2;
use qymcad_ui_state::{Armed, Bench};

/// GOING BACK TO SELECTION EMPTIES THE HAND, AND EMPTIES IT WHOLE.
///
/// `sketch_select_mode` is "the single transition from a mode back to selection", and the rule it stands
/// for is that letting go means nothing is left over: not the tool, and not what the tool had half
/// collected. A half-drawn line surviving the exit is how a click lands on a shape nobody is drawing any
/// more.
///
/// The three records checked here are picked because they are filled by three DIFFERENT tools - the
/// drawing tools, the dimension tool, and the ruler - so a transition that only remembers its own tool
/// fails this.
#[test]
fn going_back_to_selection_empties_the_hand() {
    let mut b = Bench::default();
    b.armed = Armed::Draw(1); // a line tool in hand
    b.tool.pts.push(Point2::new(10.0, 20.0)); // and one point of it already clicked
    b.dim.pick.push(7); // a dimension half-pointed at
    b.measure.pts.push(Point2::new(0.0, 0.0)); // and the ruler holding a point of its own

    qymcad_sketch::sketch_select_mode(&mut b.sketch_ctx());

    assert_eq!(b.armed, Armed::None, "letting go means nothing is in hand");
    assert!(b.tool.pts.is_empty(), "the half-drawn line must go with the tool that was drawing it");
    assert!(b.dim.pick.is_empty(), "a dimension half-pointed at must not survive the exit");
    assert!(b.measure.pts.is_empty(), "the ruler's points belong to the ruler, not to the next tool");
    assert!(!b.status.is_empty(), "the transition must say what the person is now doing");
}

/// AND IT IS SAFE TO SAY IT TWICE.
///
/// Buttons that mean "back to selection" exist in more than one bar, and a second press must not be a
/// different event from the first. Checked because the exit clears a dozen records and a `clear` written
/// as a swap rather than as a reset would show it here.
#[test]
fn a_second_return_to_selection_changes_nothing() {
    let mut b = Bench::default();
    b.armed = Armed::Draw(1);
    qymcad_sketch::sketch_select_mode(&mut b.sketch_ctx());
    let once = b.status.clone();

    qymcad_sketch::sketch_select_mode(&mut b.sketch_ctx());

    assert_eq!(b.armed, Armed::None, "an empty hand stays empty");
    assert_eq!(b.status, once, "the same transition must say the same thing");
}
