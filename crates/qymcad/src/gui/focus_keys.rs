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
        app.enter_component(part);
        // AN EXTRUDE NEEDS A SKETCH rather than a body: with a body selected the command simply does
        // not open (`kind` stays 0), and the test would be checking emptiness.
        app.chosen.sel = super::super::Sel::Sketch(0);
        app.viewing.mode_3d = true; // "re-pick the contour" exists exactly in the 3D step of the command
        app.start_feat_cmd(1);
        assert!(app.tools.armed.commanding() && app.tools.cmd.sketch.is_some(), "the scene did not open the extrude — there is nothing to check");
        app
    }

    /// A BARE LETTER WORKS WHEN NOTHING IS FOCUSED.
    #[test]
    fn a_bare_letter_works_when_nothing_is_focused() {
        let mut app = extruding();
        assert!(!(matches!(app.tools.armed.cmd_kind(), 1 | 3) && app.tools.cmd.sketch.is_some() && !app.viewing.mode_3d), "the half-sketcher must not be open in advance");
        press(&mut app, Key::U, Modifiers::NONE, false);
        assert!((matches!(app.tools.armed.cmd_kind(), 1 | 3) && app.tools.cmd.sketch.is_some() && !app.viewing.mode_3d), "a bare U with the focus free did not open the contour re-pick");
    }

    /// INSIDE A FIELD A BARE LETTER DOES NOT RUN A COMMAND — it is typed.
    ///
    /// The half without which "let the letter through past the focus" would break typing: `w` and
    /// `len` in a formula are an everyday thing.
    #[test]
    fn inside_a_field_a_bare_letter_is_typed_not_executed() {
        let mut app = extruding();
        press(&mut app, Key::U, Modifiers::NONE, true);
        assert!(
            !(matches!(app.tools.armed.cmd_kind(), 1 | 3) && app.tools.cmd.sketch.is_some() && !app.viewing.mode_3d),
            "a letter from a field ran a command — `len` can no longer be written into a formula"
        );
    }

    /// ALT PLUS A LETTER WORKS FROM A FIELD TOO. Exactly the reported case.
    #[test]
    fn alt_letter_works_even_while_typing() {
        let mut app = extruding();
        press(&mut app, Key::U, Modifiers::ALT, true);
        assert!(
            (matches!(app.tools.armed.cmd_kind(), 1 | 3) && app.tools.cmd.sketch.is_some() && !app.viewing.mode_3d),
            "Alt+U from a field did not open the contour re-pick — the hand still reaches for the mouse"
        );
    }

    /// ESC FROM A FIELD DOES NOT CANCEL THE COMMAND, AND A SECOND ONE DOES.
    ///
    /// Esc in a field used to take down the whole command along with the picked geometry: a person
    /// meant to erase a half-typed number and lost the selection.
    #[test]
    fn escape_leaves_the_field_first_and_cancels_second() {
        let mut app = extruding();
        press(&mut app, Key::Escape, Modifiers::NONE, true);
        assert!(app.tools.armed.commanding(), "the first Esc from a field cancelled the whole command");
        press(&mut app, Key::Escape, Modifiers::NONE, false);
        assert!(!app.tools.armed.commanding(), "the second Esc, already without focus, did not cancel the command");
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
        let free = qymcad_ui_state::hotkey_hint(&app.draw_ctx(), &ctx, "part.contour-reselect");
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
        let typing = qymcad_ui_state::hotkey_hint(&app.draw_ctx(), &ctx2, "part.contour-reselect");
        // written the way this system writes keys: `Alt+U`, and `⌥U` on a Mac
        let alt = qymcad_ui_state::key_label("Alt+U");
        assert_eq!(typing, alt, "with focus in a field the hint should call for Alt ({alt}) rather than \"{typing}\"");
    }

    /// AND THE RULE IS WRITTEN IN ONE PLACE rather than smeared across the handlers.
    #[test]
    fn the_rule_lives_in_one_place() {
        // the rule moved out of the god object into `pressed_chord`, and the handler must still go through it
        let rule = include_str!("../../../qymcad-ui-state/src/lib.rs");
        assert!(crate::gui::render_source::has(rule, "if typing { !chord.altgr && alt != chord_key }"), "the \"with focus, use Alt\" rule is gone from the common place");
        let seq = include_str!("../../../qymcad-ui-state/src/key_seq.rs");
        assert!(seq.contains("let press = crate::pressed_chord(ctx);"), "the key sequences no longer read the press through the common rule");
        let src = include_str!("input.rs");
        assert!(src.contains("qymcad_ui_state::hotkey_presses(ctx"), "the tool keys no longer read the press through the common rule");
        assert!(
            !crate::gui::render_source::has(src, "if ctx.egui_wants_keyboard_input() {\n            return;\n        }\n        use egui::Key;"),
            "the unconditional muting of every key on focus has come back"
        );
    }

    /// Is the contour re-pick open - the half-sketcher of the extrude command, in 2D?
    fn repicking(app: &App) -> bool {
        matches!(app.tools.armed.cmd_kind(), 1 | 3) && app.tools.cmd.sketch.is_some() && !app.viewing.mode_3d
    }

    /// A KEY OUTSIDE THE FACTORY LETTERS IS HEARD once something is bound to it.
    ///
    /// The handler listed twenty-three letters by hand. A tool rebound to W was saved and shown in the
    /// reference, and W never reached the table: the rebinding worked everywhere except at the keyboard.
    #[test]
    fn a_key_rebound_outside_the_factory_letters_is_heard() {
        let mut app = extruding();
        app.set.hotkeys.insert("part.contour-reselect".into(), "W".into());
        press(&mut app, Key::W, Modifiers::NONE, false);
        assert!(repicking(&app), "the re-pick was moved to W and W does nothing");
    }

    /// A CTRL CHORD IS HEARD, AND FROM A FIELD AS WELL: Ctrl types nothing, so it needs no Alt.
    #[test]
    fn a_ctrl_chord_is_heard_with_and_without_focus() {
        for focus in [false, true] {
            let mut app = extruding();
            app.set.hotkeys.insert("part.contour-reselect".into(), "Ctrl+J".into());
            press(&mut app, Key::J, Modifiers::NONE, focus);
            assert!(!repicking(&app), "the binding is Ctrl+J and a bare J ran it (focus: {focus})");
            press(&mut app, Key::J, Modifiers::COMMAND, focus);
            assert!(repicking(&app), "Ctrl+J is bound and does nothing (focus: {focus})");
        }
    }

    /// SHIFT IS PART OF THE KEY: Shift+W runs what is on Shift+W, and a bare W does not.
    #[test]
    fn shift_is_part_of_the_key() {
        let mut app = extruding();
        app.set.hotkeys.insert("part.contour-reselect".into(), "Shift+W".into());
        press(&mut app, Key::W, Modifiers::NONE, false);
        assert!(!repicking(&app), "the binding is Shift+W and a bare W ran it");
        press(&mut app, Key::W, Modifiers::SHIFT, false);
        assert!(repicking(&app), "Shift+W is bound and does nothing");
    }

    /// A MODIFIER NOBODY ASKED FOR STOPS A BARE KEY: Ctrl+U is not U.
    #[test]
    fn a_bare_key_does_not_fire_under_ctrl() {
        let mut app = extruding();
        press(&mut app, Key::U, Modifiers::COMMAND, false);
        assert!(!repicking(&app), "Ctrl+U ran what is bound to a bare U");
    }

    /// THE MAC'S CONTROL KEY IS A MODIFIER OF ITS OWN, not Cmd and not nothing. `ctrl` without `command` is exactly
    /// how egui reports it there: a Control+J press runs what is bound to Control+J - on a Mac, the only system with
    /// that key - and neither the Cmd+J binding nor the bare J.
    #[test]
    fn the_macs_control_key_is_its_own_modifier() {
        let mac_control = Modifiers { ctrl: true, ..Modifiers::NONE };
        let mut app = extruding();
        app.set.hotkeys.insert("part.contour-reselect".into(), "Ctrl+J".into());
        press(&mut app, Key::J, mac_control, false);
        assert!(!repicking(&app), "the Mac's Control+J ran what is bound to Cmd+J");
        let mut app = extruding();
        press(&mut app, Key::U, mac_control, false);
        assert!(!repicking(&app), "the Mac's Control+U ran what is bound to a bare U");
        let mut app = extruding();
        app.set.hotkeys.insert("part.contour-reselect".into(), "Control+J".into());
        press(&mut app, Key::J, mac_control, false);
        assert_eq!(repicking(&app), cfg!(target_os = "macos"), "Control+J runs its binding on a Mac and nowhere else");
    }

    /// AN ACTION LEFT WITHOUT A KEY is not run by its factory key either.
    #[test]
    fn an_unbound_action_is_not_run_by_its_factory_key() {
        let mut app = extruding();
        app.set.hotkeys.insert("part.contour-reselect".into(), String::new());
        press(&mut app, Key::U, Modifiers::NONE, false);
        assert!(!repicking(&app), "the re-pick was left without a key and U still runs it");
    }

    /// THE HINT OF A CTRL CHORD needs no Alt from a field.
    #[test]
    fn the_hint_of_a_ctrl_chord_has_no_alt() {
        let mut app = extruding();
        app.set.hotkeys.insert("part.contour-reselect".into(), "Ctrl+J".into());
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        for _ in 0..2 {
            let _ = ctx.run_ui(egui::RawInput::default(), |c| {
                egui::Area::new(egui::Id::new("f")).show(c, |ui| {
                    let mut s = String::new();
                    ui.text_edit_singleline(&mut s).request_focus();
                });
            });
        }
        assert_eq!(qymcad_ui_state::hotkey_hint(&app.draw_ctx(), &ctx, "part.contour-reselect"), qymcad_ui_state::key_label("Ctrl+J"), "a Ctrl chord works from a field as it is");
    }

    /// A BINDING THIS SYSTEM KEEPS DOES NOT RUN, though a profile from another system brought it: Cmd+W, free on a
    /// Mac, is the word eraser of a field on Linux and Windows.
    #[test]
    fn a_binding_this_system_keeps_does_not_run() {
        if qymcad_ui_state::platform_keys::Os::current() == qymcad_ui_state::platform_keys::Os::Mac {
            return; // there Cmd+W is a lawful binding - checked by the rules of each system in `hotkeys_rebind`
        }
        let mut app = extruding();
        app.set.hotkeys.insert("part.contour-reselect".into(), "Ctrl+W".into());
        press(&mut app, Key::W, Modifiers::COMMAND, false);
        assert!(!repicking(&app), "Ctrl+W, carried from a Mac, ran a tool where it erases a word");
    }

    /// KEY SEQUENCES THROUGH WHOLE FRAMES of one context, on a clock of sixty frames a second. The wait for the next
    /// chord is time, and a context made afresh for every press would forget the frames before it.
    struct Keys {
        ctx: egui::Context,
        time: f64,
        /// A real text field holding the keyboard, and what has been typed into it; `None` - no field.
        field: Option<String>,
        /// The status line of the last frame, as drawn.
        status: Vec<String>,
    }

    impl Keys {
        fn new(field: bool) -> Self {
            let ctx = egui::Context::default();
            super::super::install_fonts(&ctx);
            let mut k = Keys { ctx, time: 0.0, field: field.then(String::new), status: Vec::new() };
            k.idle_frames(&mut App::default(), 2); // the field takes the focus
            k
        }

        /// One frame in the order of the live one: the keys are heard before anything is drawn, then the field,
        /// then the status line.
        fn frame(&mut self, app: &mut App, modifiers: Modifiers, events: Vec<Event>) {
            self.time += 1.0 / 60.0;
            let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, 900.0));
            let input = egui::RawInput { screen_rect: Some(screen), time: Some(self.time), modifiers, events, ..Default::default() };
            let field = &mut self.field;
            let out = self.ctx.run_ui(input, |ui| {
                let c = ui.ctx().clone();
                app.handle_tool_hotkeys(&c);
                app.handle_key_commands(&c);
                if let Some(text) = field.as_mut() {
                    egui::Area::new(egui::Id::new("probe_field")).show(&c, |ui| ui.text_edit_singleline(text).request_focus());
                }
                egui::Area::new(egui::Id::new("probe_status")).fixed_pos(egui::pos2(0.0, 800.0)).show(&c, |ui| {
                    crate::gui::panels_bars::status_bar(
                        &mut qymcad_ui_state::StatusCtx {
                            cache: &app.cache,
                            cursor: None,
                            project: &app.project,
                            scheme: &app.scheme,
                            set: &mut app.set,
                            sketch_ses: &app.sketch_ses,
                            status: &app.status,
                            keys: &app.hotkeys.wait,
                            win: &mut app.win,
                        },
                        ui,
                    )
                });
            });
            let mut texts = Vec::new();
            for cs in out.shapes {
                walk(&cs.shape, &mut texts);
            }
            self.status = texts;
        }

        /// A key pressed and let go, with the text it types where it types one, as the window sends them.
        fn press(&mut self, app: &mut App, key: Key, modifiers: Modifiers) {
            let mut down = vec![Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }];
            if !modifiers.command && !modifiers.ctrl && !modifiers.alt {
                down.push(Event::Text(key.name().to_lowercase()));
            }
            self.frame(app, modifiers, down);
            self.frame(app, Modifiers::NONE, vec![Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers }]);
        }

        fn idle_frames(&mut self, app: &mut App, n: usize) {
            for _ in 0..n {
                self.frame(app, Modifiers::NONE, Vec::new());
            }
        }

        /// Frames with no input for `secs` of the clock.
        fn idle(&mut self, app: &mut App, secs: f64) {
            self.idle_frames(app, (secs * 60.0).ceil() as usize);
        }

        fn shows(&self, text: &str) -> bool {
            self.status.iter().any(|t| t == text)
        }
    }

    fn walk(s: &egui::epaint::Shape, out: &mut Vec<String>) {
        match s {
            egui::epaint::Shape::Text(t) => out.push(t.galley.text().to_string()),
            egui::epaint::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }

    /// G, G RUNS ITS ACTION, AND G ALONE DOES NOT. The status line shows the G pressed while the next one is waited
    /// for, and clears when the wait runs out.
    #[test]
    fn a_sequence_runs_on_its_second_press_and_shows_the_first() {
        let mut app = extruding();
        app.set.hotkeys.insert("part.contour-reselect".into(), "G, G".into());
        let mut keys = Keys::new(false);
        let waiting = crate::i18n::tr1("hotkeys-seq-waiting", "keys", &qymcad_ui_state::key_label("G"));
        keys.press(&mut app, Key::G, Modifiers::NONE);
        assert!(!repicking(&app), "G alone ran what is bound to G, G");
        assert!(keys.shows(&waiting), "the status line does not show the G waiting for its second press: {:?}", keys.status);
        keys.press(&mut app, Key::G, Modifiers::NONE);
        assert!(repicking(&app), "G, G is bound and the second G did nothing");
        assert!(!keys.shows(&waiting), "the sequence ran and the status line still waits");

        let mut app = extruding();
        app.set.hotkeys.insert("part.contour-reselect".into(), "G, G".into());
        keys.press(&mut app, Key::G, Modifiers::NONE);
        keys.idle(&mut app, 0.5);
        assert!(!keys.shows(&waiting), "the wait ran out and the status line still shows it: {:?}", keys.status);
        keys.press(&mut app, Key::G, Modifiers::NONE);
        assert!(!repicking(&app), "a G after the wait ran out finished the sequence");
    }

    /// A FIRST PRESS THAT IS A BINDING OF ITS OWN runs when no second press comes in time, and at once when no longer
    /// binding starts with it. The wait is the setting's.
    #[test]
    fn a_bound_first_press_runs_when_the_wait_runs_out() {
        let mut app = extruding();
        app.set.hotkeys.insert("part.contour-reselect".into(), "G".into());
        app.set.hotkeys.insert("part.hole".into(), "G, G".into());
        app.set.key_wait_ms = 500;
        let mut keys = Keys::new(false);
        keys.press(&mut app, Key::G, Modifiers::NONE);
        keys.idle(&mut app, 0.4);
        assert!(!repicking(&app), "G ran before the wait of 500 ms was up");
        keys.idle(&mut app, 0.2);
        assert!(repicking(&app), "the wait ran out and G did not run");

        let mut app = extruding();
        app.set.hotkeys.insert("part.contour-reselect".into(), "G".into());
        keys.press(&mut app, Key::G, Modifiers::NONE);
        assert!(repicking(&app), "G starts no longer binding and still waited");
    }

    /// ESC ENDS THE WAIT AND DOES NOTHING ELSE: the command in hand stays. A second Esc walks the ladder as ever.
    #[test]
    fn escape_ends_the_wait_and_keeps_the_command() {
        let mut app = extruding();
        app.set.hotkeys.insert("part.contour-reselect".into(), "G, G".into());
        app.set.hotkeys.insert("part.hole".into(), "G".into());
        let mut keys = Keys::new(false);
        keys.press(&mut app, Key::G, Modifiers::NONE);
        keys.press(&mut app, Key::Escape, Modifiers::NONE);
        assert!(app.tools.armed.commanding(), "the Esc that ended the wait cancelled the command too");
        keys.idle(&mut app, 0.5);
        assert!(!repicking(&app) && app.tools.armed.cmd_kind() == 1, "the G dropped by Esc ran after all");
        keys.press(&mut app, Key::Escape, Modifiers::NONE);
        assert!(!app.tools.armed.commanding(), "with nothing waiting, Esc no longer cancels the command");
    }

    /// FROM A FIELD ALT GOES ON THE FIRST PRESS ONLY: Alt+G, then a bare G runs the binding, and that G is not typed.
    /// A bare G first is typed and starts nothing.
    #[test]
    fn from_a_field_alt_goes_on_the_first_press_only() {
        let mut app = extruding();
        app.set.hotkeys.insert("part.contour-reselect".into(), "G, G".into());
        let mut keys = Keys::new(true);
        keys.press(&mut app, Key::G, Modifiers::NONE);
        keys.press(&mut app, Key::G, Modifiers::NONE);
        assert!(!repicking(&app), "bare letters typed into a field ran a sequence");
        assert_eq!(keys.field.as_deref(), Some("gg"), "the letters were not typed into the field");

        keys.press(&mut app, Key::G, Modifiers::ALT);
        keys.press(&mut app, Key::G, Modifiers::NONE);
        assert!(repicking(&app), "Alt+G, G from a field did nothing");
        assert_eq!(keys.field.as_deref(), Some("gg"), "the second G of the sequence was typed into the field as well");
    }

    /// A CTRL FIRST PRESS needs no Alt from a field, and the bare press after it is taken from the field; a press
    /// that continues nothing is typed as ever.
    #[test]
    fn from_a_field_a_ctrl_sequence_takes_its_second_press() {
        let mut app = extruding();
        app.set.hotkeys.insert("part.contour-reselect".into(), "Ctrl+T, F".into());
        let mut keys = Keys::new(true);
        keys.press(&mut app, Key::T, Modifiers::COMMAND);
        keys.press(&mut app, Key::Q, Modifiers::NONE);
        assert!(!repicking(&app));
        assert_eq!(keys.field.as_deref(), Some("q"), "a letter that continues nothing was kept from the field");
        keys.press(&mut app, Key::T, Modifiers::COMMAND);
        keys.press(&mut app, Key::F, Modifiers::NONE);
        assert!(repicking(&app), "Ctrl+T, F from a field did nothing");
        assert_eq!(keys.field.as_deref(), Some("q"), "the F of the sequence was typed into the field");
    }

    /// ALTGR IS A MODIFIER OF ITS OWN: AltGr+W runs what is bound to it, and neither the bare W nor Alt+W does.
    #[test]
    fn altgr_is_a_modifier_of_its_own() {
        let mut app = extruding();
        app.set.hotkeys.insert("part.contour-reselect".into(), "AltGr+W".into());
        let mut keys = Keys::new(false);
        keys.press(&mut app, Key::W, Modifiers::NONE);
        keys.press(&mut app, Key::W, Modifiers::ALT);
        assert!(!repicking(&app), "W or the left Alt+W ran what is bound to AltGr+W");
        let alt_right = |pressed| Event::Key { key: Key::AltRight, physical_key: None, pressed, repeat: false, modifiers: Modifiers::ALT };
        keys.frame(&mut app, Modifiers::ALT, vec![alt_right(true)]);
        keys.press(&mut app, Key::W, Modifiers::ALT);
        keys.frame(&mut app, Modifiers::NONE, vec![alt_right(false)]);
        assert!(repicking(&app), "AltGr+W is bound and does nothing");
    }
}
