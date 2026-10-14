//! An axis picked in a sketch is a `(kind, id)` entry of the selection of kind `SELECTION_AXIS`, and the entry reads
//! back as the same axis. An entry of any other kind names no axis, whatever its id: an entity with the id 0 or 1 is not
//! the X or Y axis.
use qymcad_core::model::SketchAxis;
use qymcad_ui_state::{axis_of_selection_entry, selection_entry_for_axis, SELECTION_AXIS, SEL_TEXT};

#[test]
fn an_axis_entry_reads_back_as_its_axis() {
    for axis in SketchAxis::ALL {
        assert_eq!(selection_entry_for_axis(axis).0, SELECTION_AXIS, "the entry of {axis:?} is not of the axis kind");
        assert_eq!(axis_of_selection_entry(selection_entry_for_axis(axis)), Some(axis), "the entry of {axis:?} reads back as another axis");
    }
}

#[test]
fn an_entry_of_another_kind_names_no_axis() {
    for id in [0, 1] {
        assert_eq!(axis_of_selection_entry((1, id)), None, "the entity {id} was read as an axis");
        assert_eq!(axis_of_selection_entry((0, id)), None, "the point {id} was read as an axis");
        assert_eq!(axis_of_selection_entry((SEL_TEXT, id)), None, "the text {id} was read as an axis");
    }
    assert_eq!(axis_of_selection_entry((SELECTION_AXIS, SketchAxis::ALL.len() as u64)), None, "an axis past the last was read out of the selection");
}
