//! ESCAPE WITH THE DRIVER LIST OPEN CLOSES THE LIST, NOT THE COMMAND.
//!
//! Reported behaviour: typing in a parameter field brings up the list; pressing Escape does not make the
//! list go away but cancels the operation instead.
//!
//! The field's own contract already says the right thing and is checked in `expr_field_behaviour.rs`: with
//! the list open Escape closes the list. That check draws THE FIELD ALONE, and there it holds. In the whole
//! frame it does not: `handle_key_commands` runs before anything is drawn, sees `wants_keyboard_input` and
//! surrenders the focus — so the field never gets the key, and the next Escape reaches `on_escape` and takes
//! the command down.
//!
//! That is why this check drives A WHOLE FRAME in the production order: keys first, drawing second. A field
//! drawn on its own cannot see this at all.
//!
//! MEASURED: on the feature command's popup the ladder ALREADY holds — both checks below were green before
//! anything was changed, so the reported field is a different one. They stay as a guard on the frame order
//! (the field alone cannot prove it) and to narrow the search: this path is not the one to look at.
#[cfg(test)]
mod tests {
    use super::super::App;

    /// The scene: a part with a sketch, an open extrude, and a name the list can offer.
    fn extruding_with_a_driver() -> App {
        let mut app = super::super::screen_keys::tests::plate();
        let part = app.project.components.iter().rev().find(|c| c.parent.is_some()).map(|c| c.id).expect("the part");
        app.enter_component_for_test(part);
        app.sel = super::super::Sel::Sketch(0);
        app.mode_3d = true;
        app.project.parameters.push(qymcad_core::model::Param { name: "width".into(), expr: "50".into(), value: 50.0 });
        app.start_feat_cmd(1);
        assert!(app.cmd.active(), "setup: the extrude did not open — there is nothing to check");
        app
    }

    /// THE DESK: whole frames in the production order — the keys are handled first, the popup is drawn
    /// after. Reversing them would check a program that does not exist.
    struct Desk {
        ctx: egui::Context,
        rect: egui::Rect,
        events: Vec<egui::Event>,
        drawn: Vec<String>,
    }

    impl Desk {
        fn new() -> Self {
            let ctx = egui::Context::default();
            super::super::install_fonts(&ctx);
            Self { ctx, rect: egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, 900.0)), events: Vec::new(), drawn: Vec::new() }
        }

        fn type_text(&mut self, s: &str) -> &mut Self {
            for c in s.chars() {
                self.events.push(egui::Event::Text(c.to_string()));
            }
            self
        }

        fn key(&mut self, key: egui::Key) -> &mut Self {
            for pressed in [true, false] {
                self.events.push(egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers: Default::default() });
            }
            self
        }

        fn frame(&mut self, app: &mut App) -> &mut Self {
            let input = egui::RawInput { screen_rect: Some(self.rect), events: std::mem::take(&mut self.events), ..Default::default() };
            let rect = self.rect;
            let out = self.ctx.run_ui(input, |ui| {
            // The frame hands in the root `Ui` now; the context comes from it.
            let ctx = &ui.ctx().clone();
                app.handle_key_commands(ctx);
                app.feat_cmd_popup(ctx, rect);
            });
            self.drawn.clear();
            for cs in &out.shapes {
                super::super::screen_keys::tests::collect_text(&cs.shape, &mut self.drawn);
            }
            self
        }

        /// egui areas settle on the second pass — a list that has just opened is not drawn on the first.
        fn settle(&mut self, app: &mut App) -> &mut Self {
            self.frame(app);
            self.frame(app)
        }

        fn shows(&self, what: &str) -> bool {
            self.drawn.iter().any(|t| t.contains(what))
        }
    }

    /// ESCAPE TAKES THE LIST DOWN AND LEAVES THE COMMAND ALONE.
    #[test]
    fn escape_closes_the_list_and_keeps_the_command() {
        let mut app = extruding_with_a_driver();
        let mut d = Desk::new();
        d.settle(&mut app);
        d.type_text("wid").settle(&mut app);
        assert!(d.shows("width"), "setup: the list must be open before Escape, drawn: {:?}", d.drawn);

        d.key(egui::Key::Escape).frame(&mut app);
        assert!(app.cmd.active(), "Escape with the list open cancelled the whole operation instead of closing the list");

        d.frame(&mut app);
        assert!(!d.shows("width"), "the list stayed open after Escape: {:?}", d.drawn);
    }

    /// THE SECOND ESCAPE STILL CANCELS THE COMMAND — the ladder is not lost.
    #[test]
    fn a_second_escape_still_cancels_the_command() {
        let mut app = extruding_with_a_driver();
        let mut d = Desk::new();
        d.settle(&mut app);
        d.type_text("wid").settle(&mut app);

        d.key(egui::Key::Escape).frame(&mut app); // the list
        d.key(egui::Key::Escape).frame(&mut app); // out of the field
        d.key(egui::Key::Escape).frame(&mut app); // the command
        assert!(!app.cmd.active(), "with the list closed and the field left, Escape no longer cancels the command");
    }
}
