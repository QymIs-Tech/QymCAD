//! LEAVING A SKETCH IS ONE GESTURE, AND IT IS THE SAME ONE AS FOR A PART.
//!
//! Reported behaviour: "why can we leave the sketch with ESC? Leaving the sketch only by CTRL+Enter, as
//! for a part and a subassembly."
//!
//! Esc is the key that PUTS DOWN: the drawing under way, then the tool, then the selection. Every step of
//! that ladder gives something back. Closing the sketch is not giving something back - it is finishing a
//! context, the same act as leaving a part, and a part is left by Ctrl+Enter. One key doing both means
//! that a person tapping Esc to get rid of a tool loses the sketch on the tap after the last one, and the
//! only way to notice is that the toolbar has changed.
//!
//! So the ladder ends where it runs out of things to put down, and finishing a context stays with the
//! gesture that finishes a context.
#[cfg(test)]
mod tests {
    use super::super::App;
    use egui::{Event, Key, Modifiers};

    /// A part with a sketch OPEN FOR EDITING, and nothing in hand.
    ///
    /// Nothing armed and nothing selected on purpose: those are the rungs of the ladder above, and with
    /// any of them filled Esc has something to put down and never reaches the last rung at all.
    fn a_sketch_under_edit() -> App {
        let mut app = App::default();
        app.set.gpu_viewport = false;
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, 0.0, 0.0, 40.0, 30.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.tools.armed = qymcad_ui_state::Armed::None;
        app.tools.sel_sk.clear();
        assert!(app.sketch_ses.editing.is_some(), "GUARD: the scene must start with the sketch open, or there is nothing to close");
        app
    }

    /// A frame with a key pressed, as the program reads it.
    fn press(app: &mut App, key: Key, modifiers: Modifiers) {
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, 900.0));
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let mut input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
        let _ = ctx.run_ui(input.clone(), |c| app.handle_key_commands(c)); // the layout frame
        input.modifiers = modifiers;
        input.events.push(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers });
        let _ = ctx.run_ui(input, |c| app.handle_key_commands(c));
    }

    /// ESC DOES NOT CLOSE THE SKETCH, however many times it is pressed.
    ///
    /// Three presses rather than one: the complaint is about the tap AFTER the ladder has run out, and a
    /// single press would pass even if the last rung merely moved one place up.
    #[test]
    fn escape_does_not_close_the_sketch() {
        let mut app = a_sketch_under_edit();
        for n in 1..=3 {
            press(&mut app, Key::Escape, Modifiers::NONE);
            assert!(
                app.sketch_ses.editing.is_some(),
                "Esc number {n} closed the sketch: Esc puts a tool down, it does not finish a context - that is Ctrl+Enter"
            );
        }
    }

    /// AND CTRL+ENTER DOES CLOSE IT.
    ///
    /// Without this the check above would be green for a program that cannot leave a sketch at all.
    #[test]
    fn ctrl_enter_closes_the_sketch() {
        let mut app = a_sketch_under_edit();
        press(&mut app, Key::Enter, Modifiers::COMMAND);
        assert!(app.sketch_ses.editing.is_none(), "Ctrl+Enter must finish the sketch, the same as it finishes a part");
    }
}
