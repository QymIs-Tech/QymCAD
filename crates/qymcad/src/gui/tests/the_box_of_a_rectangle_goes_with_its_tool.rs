//! THE BOX OF THE SIZES OF A RECTANGLE GOES WITH ITS TOOL: drawn from two corners, the rectangle shows its width and
//! height in a box beside it; once the tool is put down the box is gone, and nothing typed later reaches the rectangle.
//!
//! Reported behaviour (issue #271): the W / H box stayed on the screen after the Select tool was taken, with the first
//! corner and the size as drawn; a dimension laid on the rectangle afterwards was thrown away the moment a value was typed
//! into the stale box, which rebuilt the rectangle from its old size.

use super::super::hand::Hand;
use super::super::{App, Sel};
use qymcad_core::feature::SketchPlane;

/// The words of the box of sizes, as the person sees them.
fn width_word() -> String {
    crate::i18n::tr("sk-width-short")
}

#[test]
fn the_box_of_sizes_is_gone_once_the_select_tool_is_taken() {
    let mut app = App::default();
    let si = app.create_sketch_on(SketchPlane::default());
    app.chosen.sel = Sel::Sketch(si);
    let mut hand = Hand::new(&mut app);
    hand.sk_tool(qymcad_ui_state::SketchShape::Rect).click2d(0.0, 0.0).click2d(79.0, 78.0);
    // a box shows from its second frame: egui lays a new area out unseen first
    assert!(hand.frame(Vec::new()).shows(&width_word()), "setup: the box of sizes of the rectangle just drawn is not on the screen");
    hand.sk_select().frame(Vec::new());
    assert!(!hand.shows(&width_word()), "the box of sizes of the rectangle stays on the screen after the Select tool was taken");
}
