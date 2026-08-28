//! FOCUS IN A FIELD DOES NOT TAKE THE KEYBOARD AWAY FROM THE COMMAND.
//!
//! It used to be: `handle_tool_hotkeys` began with an unconditional
//! `if ctx.egui_wants_keyboard_input() { return }`. One line put out ALL 23 tool keys in ALL commands the
//! moment the cursor landed in any input field. The most visible case was reported: in an extrude `U`
//! ("re-pick the contour") could not be pressed until the focus was knocked off with the mouse. And
//! the other half is right there: a second Enter did not apply the command — it never reached it
//! either, and one had to aim at the tick.
//!
//! The rule is now this:
//!
//! * **a bare letter** — when there is no focus in a field (in a field it must be typed: expressions
//!   hold both `w` and `len`);
//! * **Alt plus a letter** — always, including from a field: `egui` does not type it;
//! * **Enter in a field** — accept the value and RELEASE the focus, so that the next Enter applies the
//!   command;
//! * **Esc** — in two steps: first leave the field, and only then cancel the command.
#[cfg(test)]
mod tests {
    use super::super::App;
    use egui::{Event, Key, Modifiers};

    /// A frame with a key pressed: returns the application after the handling.
    fn press(app: &mut App, key: Key, modifiers: Modifiers, focus_field: bool) {
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, 900.0));
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let mut input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
        // the first frame lays out and, if needed, lets the field take the focus
        let _ = ctx.run_ui(input.clone(), |c| {
            frame(app, c, focus_field);
        });
        // THE MODIFIERS GO BOTH INTO THE EVENT AND INTO THE INPUT STATE. `i.modifiers` is read from
        // the state rather than from the event: without this line Alt is "held" only inside the event
        // and the check never sees it.
        input.modifiers = modifiers;
        input.events.push(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers });
        let _ = ctx.run_ui(input, |c| {
            frame(app, c, focus_field);
        });
    }

    /// A frame of the program plus, on demand, an input field that takes the focus.
    ///
    /// The field is REAL rather than a flag: what has to be checked is exactly what `egui` does —
    /// whether it swallows the key. A fake "as if there were focus" would check somebody's own
    /// invention.
    fn frame(app: &mut App, ctx: &egui::Context, focus_field: bool) {
        if focus_field {
            egui::Area::new(egui::Id::new("probe_field")).show(ctx, |ui| {
                let mut buf = String::from("10");
                let r = ui.text_edit_singleline(&mut buf);
                r.request_focus();
            });
        }
        app.handle_key_commands(ctx);
        app.handle_tool_hotkeys(ctx);
    }

    /// The scene: a part with a body and an open extrude command.
    fn extruding() -> App {
        let mut app = super::super::screen_keys::tests::plate();
        // INTO THE PART: outside it the workbench is the Assembly, and `U` there means "subassembly"
        // rather than "re-pick the contour". The scene must be the one a person presses this key in.
        let part = app.project.components.iter().rev().find(|c| c.parent.is_some()).map(|c| c.id).expect("the part");
        app.enter_component_for_test(part);
        // AN EXTRUDE NEEDS A SKETCH rather than a body: with a body selected the command simply does
        // not open (`kind` stays 0), and the test would be checking emptiness.
        app.sel = super::super::Sel::Sketch(0);
        app.mode_3d = true; // "re-pick the contour" exists exactly in the 3D step of the command
        app.start_feat_cmd(1);
        assert!(app.cmd.active() && app.cmd.sketch.is_some(), "the scene did not open the extrude — there is nothing to check");
        app
    }

    /// A BARE LETTER WORKS WHEN NOTHING IS FOCUSED.
    #[test]
    fn a_bare_letter_works_when_nothing_is_focused() {
        let mut app = extruding();
        assert!(!app.contour_repick_active_for_test(), "the half-sketcher must not be open in advance");
        press(&mut app, Key::U, Modifiers::NONE, false);
        assert!(app.contour_repick_active_for_test(), "a bare U with the focus free did not open the contour re-pick");
    }

    /// INSIDE A FIELD A BARE LETTER DOES NOT RUN A COMMAND — it is typed.
    ///
    /// The half without which "let the letter through past the focus" would break typing: `w` and
    /// `len` in a formula are an everyday thing.
    #[test]
    fn inside_a_field_a_bare_letter_is_typed_not_executed() {
        let mut app = extruding();
        press(&mut app, Key::U, Modifiers::NONE, true);
        assert!(!app.contour_repick_active_for_test(), "a letter from a field ran a command — `len` can no longer be written into a formula");
    }

    /// ALT PLUS A LETTER WORKS FROM A FIELD TOO. Exactly the reported case.
    #[test]
    fn alt_letter_works_even_while_typing() {
        let mut app = extruding();
        press(&mut app, Key::U, Modifiers::ALT, true);
        assert!(app.contour_repick_active_for_test(), "Alt+U from a field did not open the contour re-pick — the hand still reaches for the mouse");
    }

    /// ESC FROM A FIELD DOES NOT CANCEL THE COMMAND, AND A SECOND ONE DOES.
    ///
    /// Esc in a field used to take down the whole command along with the picked geometry: a person
    /// meant to erase a half-typed number and lost the selection.
    #[test]
    fn escape_leaves_the_field_first_and_cancels_second() {
        let mut app = extruding();
        press(&mut app, Key::Escape, Modifiers::NONE, true);
        assert!(app.cmd.active(), "the first Esc from a field cancelled the whole command");
        press(&mut app, Key::Escape, Modifiers::NONE, false);
        assert!(!app.cmd.active(), "the second Esc, already without focus, did not cancel the command");
    }

    /// THE HINT CHANGES ALONG WITH THE RULE.
    ///
    /// Without it "with focus, use Alt" would stay a secret: a person presses `U` in a field, gets
    /// nothing, and does not try a second time. A mechanism nobody was told about is as good as
    /// switched off.
    #[test]
    fn the_hint_says_alt_while_typing() {
        let app = extruding();
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        // with no focus it is a bare letter
        let _ = ctx.run_ui(egui::RawInput::default(), |_| {});
        let free = app.hotkey_hint(&ctx, "part.contour-reselect");
        assert_eq!(free, "U", "with no focus the hint should be a bare letter rather than \"{free}\"");
        // with focus it is Alt plus a letter
        let ctx2 = egui::Context::default();
        super::super::install_fonts(&ctx2);
        for _ in 0..2 {
            let _ = ctx2.run_ui(egui::RawInput::default(), |c| {
                egui::Area::new(egui::Id::new("f")).show(c, |ui| {
                    let mut s = String::new();
                    ui.text_edit_singleline(&mut s).request_focus();
                });
            });
        }
        let typing = app.hotkey_hint(&ctx2, "part.contour-reselect");
        assert_eq!(typing, "Alt+U", "with focus in a field the hint should call for Alt rather than \"{typing}\"");
    }

    /// AND THE RULE IS WRITTEN IN ONE PLACE rather than smeared across the handlers.
    #[test]
    fn the_rule_lives_in_one_place() {
        let src = include_str!("input.rs");
        assert!(src.contains("if typing { i.modifiers.alt"), "the \"with focus, use Alt\" rule is gone from the common place");
        assert!(!src.contains("if ctx.egui_wants_keyboard_input() {\n            return;\n        }\n        use egui::Key;"), "the unconditional muting of every key on focus has come back");
    }
}
