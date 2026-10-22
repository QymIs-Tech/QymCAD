//! THE KEY OF THE TOOL IN HAND PRESSED AGAIN PUTS IT DOWN, in every workbench, as its button pressed again does; while a
//! field holds the keyboard the letter is typed into the field and takes no tool.
//!
//! Reported behaviour: in a sketch only the keys of the dimension, the corner, the trim and the mirror put their tool down;
//! a drawing tool pressed again was taken anew, and the key of a command of a part opened the command again over
//! itself.

use super::super::hand::Hand;
use super::super::{App, Sel};
use qymcad_core::feature::SketchPlane;

/// L takes the line in a sketch; L again puts it down.
#[test]
fn the_key_of_a_drawing_tool_pressed_again_puts_it_down() {
    let mut app = App::default();
    let si = app.create_sketch_on(SketchPlane::default());
    app.chosen.sel = Sel::Sketch(si);
    let mut hand = Hand::canvas(&mut app);
    hand.key(egui::Key::L);
    assert_eq!(hand.app.tools.armed.drawing(), Some(qymcad_ui_state::SketchShape::Line), "setup: L did not take the line");
    hand.key(egui::Key::L);
    assert!(qymcad_ui_state::in_select_mode(&hand.app.tools.armed), "L pressed again left {:?} in hand, not the arrow", hand.app.tools.armed);
}

/// E takes the extrusion of a part, waiting for the sketch it extrudes; E again puts it down.
#[test]
fn the_key_of_a_command_of_a_part_pressed_again_puts_it_down() {
    let mut app = App::default();
    let part = app.project.add_part(app.project.free_part_name());
    app.enter_component(part); // the keys of a part are heard in a part open
    let mut hand = Hand::new(&mut app);
    hand.key(egui::Key::E);
    assert!(
        qymcad_ui_state::catalogue::pressed(qymcad_ui_state::catalogue::Launch::Feat(qymcad_ui_state::PartTool::Extrude), &hand.app.bar_ctx(&mut Vec::new())),
        "setup: E did not take the extrusion"
    );
    hand.key(egui::Key::E);
    assert!(
        !qymcad_ui_state::catalogue::pressed(qymcad_ui_state::catalogue::Launch::Feat(qymcad_ui_state::PartTool::Extrude), &hand.app.bar_ctx(&mut Vec::new())),
        "E pressed again left the extrusion in hand"
    );
}

/// THE LETTER GOES TO THE FIELD THAT HOLDS THE KEYBOARD: L typed into the box of the sizes of a rectangle just drawn
/// takes no line, and the rectangle tool stays in hand.
#[test]
fn a_letter_typed_into_a_field_takes_no_tool() {
    let mut app = App::default();
    let si = app.create_sketch_on(SketchPlane::default());
    app.chosen.sel = Sel::Sketch(si);
    let mut hand = Hand::new(&mut app);
    hand.sk_tool(qymcad_ui_state::SketchShape::Rect).click2d(0.0, 0.0).click2d(79.0, 78.0);
    hand.frame(Vec::new()); // the box is laid unseen in its first frame and its field takes the keyboard in the next
    let press = |pressed| egui::Event::Key { key: egui::Key::L, physical_key: None, pressed, repeat: false, modifiers: Default::default() };
    hand.frame(vec![press(true), egui::Event::Text("l".into())]).frame(vec![press(false)]);
    assert_eq!(hand.app.tools.armed.drawing(), Some(qymcad_ui_state::SketchShape::Rect), "L typed into the field of the box took {:?} instead of being typed", hand.app.tools.armed);
    assert!(hand.app.tools.place.fields.iter().any(|p| p.txt.contains('l')), "the letter typed did not reach the field of the box");
}
