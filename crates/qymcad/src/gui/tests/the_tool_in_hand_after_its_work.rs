//! WHAT IS IN HAND AFTER A TOOL HAS DONE ITS WORK: a command that does one thing leaves the selection in hand once it is
//! applied; a tool that draws or sizes one thing after another stays for the next, and Esc with nothing in progress puts
//! it down. The frame decides it in one place, by what the tool says of itself (`AfterApply`).
//!
//! Reported behaviour: one Esc in the box of the sizes of a rectangle just drawn put the rectangle tool down with the box,
//! and the next rectangle had to be taken again.

use super::super::hand::Hand;
use super::super::{App, Sel};
use qymcad_core::feature::SketchPlane;

/// A COMMAND OF A PART APPLIED BY ENTER leaves the selection in hand: the box made, no command left open.
#[test]
fn a_command_of_a_part_applied_by_enter_leaves_the_selection() {
    let mut app = App::default();
    let part = app.project.add_part(app.project.free_part_name());
    app.enter_component(part); // the keys of a part are heard in a part open
    let mut hand = Hand::new(&mut app);
    hand.key(egui::Key::B);
    assert_eq!(hand.app.tools.armed.part_tool(), Some(qymcad_ui_state::PartTool::Box), "setup: the key of the box did not take the box");
    hand.key(egui::Key::Enter);
    assert_eq!(hand.app.project.bodies.len(), 1, "setup: Enter did not make the box");
    assert!(qymcad_ui_state::in_select_mode(&hand.app.tools.armed), "the box made by Enter left {:?} in hand, not the selection", hand.app.tools.armed);
}

/// A TURN OF A SKETCH APPLIED BY ENTER leaves the arrow in hand: the turn does one thing.
#[test]
fn a_turn_of_a_sketch_applied_by_enter_leaves_the_arrow() {
    let mut app = App::default();
    let si = app.create_sketch_on(SketchPlane::default());
    app.chosen.sel = Sel::Sketch(si);
    let mut hand = Hand::new(&mut app);
    hand.sk_tool(qymcad_ui_state::SketchShape::Line).click2d(0.0, 0.0).click2d(40.0, 0.0).key(egui::Key::Escape);
    assert!(!hand.app.project.sketches[si].entities.is_empty(), "setup: the line was not drawn");
    hand.sk_rotate((20.0, 0.0), (0.0, 0.0), 90.0);
    assert!(qymcad_ui_state::in_select_mode(&hand.app.tools.armed), "the turn made by Enter left {:?} in hand, not the arrow", hand.app.tools.armed);
}

/// A RECTANGLE DRAWN STAYS IN HAND: Esc in the box of its sizes takes the box away and leaves the rectangle tool for the
/// next one; the next Esc, with nothing in progress, puts it down.
#[test]
fn a_shape_tool_stays_after_the_box_and_esc_puts_it_down() {
    let mut app = App::default();
    let si = app.create_sketch_on(SketchPlane::default());
    app.chosen.sel = Sel::Sketch(si);
    let mut hand = Hand::new(&mut app);
    hand.sk_tool(qymcad_ui_state::SketchShape::Rect).click2d(0.0, 0.0).click2d(79.0, 78.0);
    let width = crate::i18n::tr("sk-width");
    assert!(hand.frame(Vec::new()).shows(&width), "setup: the box of sizes of the rectangle just drawn is not on the screen");
    hand.key(egui::Key::Escape).frame(Vec::new());
    assert!(!hand.shows(&width), "Esc left the box of sizes of the rectangle on the screen");
    assert_eq!(hand.app.tools.armed.drawing(), Some(qymcad_ui_state::SketchShape::Rect), "Esc in the box of sizes put the rectangle tool down with the box: {:?} in hand", hand.app.tools.armed);
    hand.key(egui::Key::Escape);
    assert!(qymcad_ui_state::in_select_mode(&hand.app.tools.armed), "Esc with nothing in progress left {:?} in hand, not the arrow", hand.app.tools.armed);
}
