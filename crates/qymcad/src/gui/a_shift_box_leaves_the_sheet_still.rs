//! SHIFT WITH THE LEFT BUTTON DRAWS THE BOX AND THE SHEET STAYS PUT, through the window with the QymCad layout: a left
//! drag with Shift on empty sheet takes into the selection what the box holds, and the view of the sheet has not moved;
//! Shift with the middle or the right button still moves the sheet.
//!
//! Reported behaviour (#64): Shift with a left drag drew the selection box and moved the sheet under the pointer at
//! once - the layout's "any button with Shift moves the view" took the left button too.
#[cfg(test)]
mod tests {
    use super::super::hand::{Drag2d, Hand};
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;

    /// Two lines drawn apart, the select tool taken, with the QymCad layout.
    fn two_lines() -> App {
        let mut app = App::default();
        assert_eq!(app.set.mouse_nav, qymcad_ui_state::MouseNav::QymCad, "setup: the QymCad layout is the default");
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        Hand::new(&mut app).sk_tool(qymcad_ui_state::SketchShape::Line).click2d(0.0, 0.0).click2d(20.0, 0.0).key(egui::Key::Escape).click2d(0.0, 10.0).click2d(20.0, 10.0).key(egui::Key::Escape);
        app
    }

    fn shift_drag(button: egui::PointerButton) -> Drag2d {
        Drag2d { from: (-5.0, -5.0), to: (25.0, 15.0), button, modifiers: egui::Modifiers::SHIFT }
    }

    #[test]
    fn shift_with_a_left_drag_draws_the_box_and_the_sheet_stays_put() {
        let mut app = two_lines();
        let mut hand = Hand::new(&mut app);
        hand.sk_select();
        let before = hand.app.viewing.view.center;
        hand.drag2d_held(shift_drag(egui::PointerButton::Primary));
        let moved = app.viewing.view.center - before;
        assert!(moved.length() < 1e-6, "the sheet moved under a Shift and left drag by {moved:?}");
        let lines = app.tools.sel_sk.items.iter().filter(|it| it.kind == qymcad_ui_state::ItemKind::Curve).count();
        assert_eq!(lines, 2, "the box did not take the two lines it held: {:?}", app.tools.sel_sk.items);
    }

    #[test]
    fn shift_with_the_middle_or_the_right_button_moves_the_sheet() {
        for button in [egui::PointerButton::Middle, egui::PointerButton::Secondary] {
            let mut app = two_lines();
            let mut hand = Hand::new(&mut app);
            hand.sk_select();
            let before = hand.app.viewing.view.center;
            hand.drag2d_held(shift_drag(button));
            assert!((app.viewing.view.center - before).length() > 1.0, "Shift with the {button:?} button did not move the sheet");
        }
    }
}
