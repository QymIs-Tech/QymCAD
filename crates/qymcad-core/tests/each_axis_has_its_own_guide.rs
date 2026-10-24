//! An axis of a sketch is named, not numbered: `SketchAxis` says which slot of `Sketch::axis_pts` holds its guide and
//! where that guide stands, and both the creation of an axis and the repair of a moved frame read it from there.
use qymcad_core::model::{Project, SketchAxis};

#[test]
fn each_axis_is_created_on_its_own_guide_and_slot() {
    let mut p = Project::default();
    let si = p.new_sketch("s");
    for axis in SketchAxis::ALL {
        let (anchor, guide) = p.ensure_axis(si, axis);
        let s = &p.sketches[si];
        assert_eq!(anchor, s.frame, "the {axis:?} axis does not stand on the anchor of the frame");
        assert_eq!(s.axis_guide(axis), guide, "the guide of {axis:?} is not kept in its own slot");
        let at = s.points.iter().find(|q| q.id == guide).expect("the guide point exists");
        assert_eq!((at.x, at.y), (axis.guide().x, axis.guide().y), "the guide of {axis:?} stands off its axis");
    }
    let s = &p.sketches[si];
    assert_ne!(s.axis_guide(SketchAxis::X), s.axis_guide(SketchAxis::Y), "the two axes share one guide point");
}

#[test]
fn a_slot_names_one_axis_and_nothing_past_the_last() {
    for axis in SketchAxis::ALL {
        assert_eq!(SketchAxis::from_slot(axis.slot()), Some(axis), "slot {} does not name {axis:?} back", axis.slot());
    }
    assert_eq!(SketchAxis::from_slot(SketchAxis::ALL.len()), None, "a slot past the last axis named an axis");
}

#[test]
fn a_moved_guide_is_put_back_on_its_axis() {
    let mut p = Project::default();
    let si = p.new_sketch("s");
    for axis in SketchAxis::ALL {
        let (_, guide) = p.ensure_axis(si, axis);
        let at = p.sketches[si].points.iter_mut().find(|q| q.id == guide).expect("the guide point exists");
        (at.x, at.y) = (7.0, -3.0);
    }

    assert!(p.sketches[si].pin_frame(), "the frame was moved and the repair saw nothing to move");
    let s = &p.sketches[si];
    for axis in SketchAxis::ALL {
        let at = s.points.iter().find(|q| q.id == s.axis_guide(axis)).expect("the guide point exists");
        assert_eq!((at.x, at.y), (axis.guide().x, axis.guide().y), "the guide of {axis:?} was not put back");
    }
}
