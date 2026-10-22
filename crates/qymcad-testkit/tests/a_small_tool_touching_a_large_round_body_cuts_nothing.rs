//! A SMALL TOOL THAT ONLY TOUCHES A LARGE ROUND BODY CUTS NOTHING, AND IS SAID SO. A cut is told from a touch by the
//! volumes of the body before and after it, read to a percent first: on a ball of 4.2 million mm^3 a percent is 42
//! thousand, and a square of 2 by 2 set on its pole takes nothing of it - the volumes to a percent cannot say so, and
//! the exact ones decide. Reported behaviour: a touch on a large body of spline faces was taken for a cut on the volumes
//! to a percent, the error of the body outweighing the tool.
use qymcad_core::errors::CoreError;
use qymcad_core::feature::{BasePlane, Extent, Purpose, Reach, SketchPlane};
use qymcad_core::model::{CombineSpan, Project};

#[test]
fn a_square_set_on_the_pole_of_a_ball_cuts_nothing() {
    let mut p = Project::default();
    p.new_document();
    let ball = p.add_sphere(100.0);
    // a square of 2 by 2 on the plane touching the ball at its pole, cut upwards - away from the ball
    let plane = p.add_offset_plane(BasePlane::XY, 100.0);
    let si = p.new_sketch("Square");
    let sid = p.sketches[si].id;
    p.sketches[si].plane = SketchPlane::Datum(plane);
    p.add_sketch_node(sid, "Square");
    p.add_rect_entity(si, -1.0, -1.0, 1.0, 1.0, Purpose::Real);
    p.regen_sketch(si);
    let c: Vec<u64> = p.sketches[si].contour_ids.iter().copied().filter(|c| p.contour_profile_xy(*c).is_some()).collect();
    assert!(!c.is_empty(), "setup: the square has no contour");
    let cut = p.add_combine_multi_op(ball, sid, c, CombineSpan { height: 5.0, down: 0.0, extent: Extent { reach: Reach::Forward, ..Default::default() }, fill: &[] }, 0);
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    let v = shapes.get(&cut).map(|s| s.volume()).unwrap_or(0.0);
    assert!(
        report.errors.iter().any(|(id, e)| *id == cut && matches!(e, CoreError::CutRemovedNothing)),
        "a square touching the pole of the ball was taken for a cut ({v:.1} mm^3 left of 4188790.2): {:?}",
        report.errors
    );
}
