//! An axis picked in a sketch is a `SketchItem` of the kind `ItemKind::Axis`, and the item reads back as the same axis.
//! An item of any other kind names no axis, whatever its id: a curve with the id 0 or 1 is not the X or Y axis.
use qymcad_core::model::SketchAxis;
use qymcad_ui_state::{ItemKind, SketchItem};

#[test]
fn an_axis_item_reads_back_as_its_axis() {
    for axis in SketchAxis::ALL {
        assert_eq!(SketchItem::axis(axis).kind, ItemKind::Axis, "the item of {axis:?} is not of the axis kind");
        assert_eq!(SketchItem::axis(axis).sketch_axis(), Some(axis), "the item of {axis:?} reads back as another axis");
    }
}

#[test]
fn an_item_of_another_kind_names_no_axis() {
    for id in [0, 1] {
        assert_eq!(SketchItem::curve(id).sketch_axis(), None, "the curve {id} was read as an axis");
        assert_eq!(SketchItem::point(id).sketch_axis(), None, "the point {id} was read as an axis");
        assert_eq!(SketchItem::text(id).sketch_axis(), None, "the text {id} was read as an axis");
    }
    let past_the_last = SketchItem { kind: ItemKind::Axis, id: SketchAxis::ALL.len() as u64 };
    assert_eq!(past_the_last.sketch_axis(), None, "an axis past the last was read out of the selection");
}
