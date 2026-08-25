//! THE DIMENSION POPUP OF A SKETCH UNDER A PERSON'S FINGERS.
//!
//! This is the very place that was reported: every letter typed into the driver field rebuilt the
//! whole project. And so it was: the name was written into the model on every keystroke, and
//! `mark_param_dependents_dirty()` was called after it, marking EVERY node with dimensions dirty.
//!
//! What is measured is the consequences rather than the appearance: the document key (was the model
//! touched or not) and the number of undo steps.
#[cfg(test)]
mod tests {
    use super::super::{App, InlineEdit, Sel};
    use qymcad_core::geom::Point2;
    use qymcad_core::model::{Constraint, Id};

    /// A sketch with a rectangle and one distance dimension. Returns (si, ci).
    fn sketch_with_dim(app: &mut App) -> (usize, usize) {
        let sid = app.project.add_line_sketch(
            "Profile",
            vec![Point2::new(0.0, 0.0), Point2::new(40.0, 0.0), Point2::new(40.0, 20.0), Point2::new(0.0, 20.0)],
            true,
        );
        let si = app.project.sketch_index(sid).unwrap();
        app.project.add_sketch_node(sid, "Profile");
        let pts: Vec<Id> = app.project.sketches[si].points.iter().map(|q| q.id).collect();
        app.project.sketches[si].constraints.push(Constraint::Distance {
            a: pts[0],
            b: pts[1],
            d: 40.0,
            off: 0.0,
            expr: String::new(),
            driven: false,
            axis: 0,
        });
        let ci = app.project.sketches[si].constraints.len() - 1;
        app.sel = Sel::Sketch(si);
        app.inline = InlineEdit::Dim(ci);
        (si, ci)
    }

    struct Popup {
        ctx: egui::Context,
        rect: egui::Rect,
        events: Vec<egui::Event>,
        drawn: Vec<String>,
        name_rect: egui::Rect,
    }

    impl Popup {
        fn new() -> Self {
            let ctx = egui::Context::default();
            super::super::install_fonts(&ctx);
            Self {
                ctx,
                rect: egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0)),
                events: Vec::new(),
                drawn: Vec::new(),
                name_rect: egui::Rect::NOTHING,
            }
        }

        fn frame(&mut self, app: &mut App, si: usize, ci: usize) -> &mut Self {
            let input = egui::RawInput { screen_rect: Some(self.rect), events: std::mem::take(&mut self.events), ..Default::default() };
            let rect = self.rect;
            let out = self.ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |_ui| {});
                // THE PRODUCTION ORDER: the frame's keys are handled BEFORE anything is drawn (`update` calls
                // `handle_key_commands` and only then paints). Drawing the popup alone checks a program that
                // does not exist - and it was exactly this omission that made an Escape check green while a
                // person saw the operation cancelled.
                app.handle_key_commands(ctx);
                app.dim_editor(ctx, rect);
            });
            if let Some(r) = self.ctx.read_response(egui::Id::new(("dimdrv", si, ci))) {
                self.name_rect = r.rect;
            }
            self.drawn.clear();
            for cs in &out.shapes {
                super::super::screen_keys::tests::collect_text(&cs.shape, &mut self.drawn);
            }
            self
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

        /// Get into the driver name field: wait for the popup to settle into place, bring the cursor
        /// over, click.
        ///
        /// WAITING IS ESSENTIAL. Measured: the popup is anchored to the dimension ON SCREEN, and for
        /// the first few frames the view is still being fitted to the sketch — the field moves from
        /// [350,340] to [588,404]. A click on the stale rectangle landed in empty space, and that was
        /// an error of aim in the check, not a broken popup.
        fn into_driver_field(&mut self, app: &mut App, si: usize, ci: usize) -> &mut Self {
            let mut still = 0;
            for _ in 0..20 {
                let was = self.name_rect;
                self.frame(app, si, ci);
                still = if self.name_rect == was { still + 1 } else { 0 };
                if still >= 2 {
                    break;
                }
            }
            let at = self.name_rect.center();
            assert!(self.name_rect.is_positive(), "the frame has no driver name field — the dimension popup did not draw it");
            self.events.push(egui::Event::PointerMoved(at));
            self.frame(app, si, ci);
            self.events.push(egui::Event::PointerMoved(at));
            for pressed in [true, false] {
                self.events.push(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() });
            }
            self.frame(app, si, ci)
        }

        fn shows(&self, what: &str) -> bool {
            self.drawn.iter().any(|t| t.contains(what))
        }

        /// THE SAME WAY IN, BUT INTO THE VALUE FIELD — the one holding the expression, where the list of
        /// drivers lives. Waiting for the popup to settle matters just as much: it is anchored to the
        /// dimension on screen and moves while the view is being fitted.
        fn into_value_field(&mut self, app: &mut App, si: usize, ci: usize) -> &mut Self {
            let id = egui::Id::new(("dimval", si, ci));
            let mut rect = egui::Rect::NOTHING;
            let mut still = 0;
            for _ in 0..20 {
                let was = rect;
                self.frame(app, si, ci);
                rect = self.ctx.read_response(id).map(|r| r.rect).unwrap_or(egui::Rect::NOTHING);
                still = if rect == was { still + 1 } else { 0 };
                if still >= 2 {
                    break;
                }
            }
            assert!(rect.is_positive(), "the frame has no value field — the dimension popup did not draw it");
            let at = rect.center();
            self.events.push(egui::Event::PointerMoved(at));
            self.frame(app, si, ci);
            self.events.push(egui::Event::PointerMoved(at));
            for pressed in [true, false] {
                self.events.push(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() });
            }
            self.frame(app, si, ci)
        }
    }

    /// ESCAPE WITH THE DRIVER LIST OPEN CLOSES THE LIST, NOT THE WHOLE POPUP.
    ///
    /// Reported behaviour: typing in a parameter field brings up the list; pressing Escape does not
    /// make the list go away but cancels the operation instead.
    ///
    /// The field's own ladder is right and is checked in `expr_field_behaviour.rs` — but the popup
    /// reads `key_pressed(Escape)` on its own, without asking whether anybody was using the key, and
    /// clears itself. Two readers of one key, and the louder one wins.
    #[test]
    fn escape_closes_the_list_and_leaves_the_popup_open() {
        let mut app = App::default();
        app.project.parameters.push(qymcad_core::model::Param { name: "width".into(), expr: "50".into(), value: 50.0 });
        let (si, ci) = sketch_with_dim(&mut app);
        let mut p = Popup::new();
        p.into_value_field(&mut app, si, ci);
        p.type_text("wid").frame(&mut app, si, ci);
        p.frame(&mut app, si, ci);
        assert!(p.shows("width"), "setup: the list must be open before Escape, drawn: {:?}", p.drawn);

        p.key(egui::Key::Escape).frame(&mut app, si, ci);
        assert!(
            matches!(app.inline, InlineEdit::Dim(_)),
            "Escape with the list open closed the whole dimension popup instead of the list — the edit is lost"
        );

        p.frame(&mut app, si, ci);
        assert!(!p.shows("width"), "the list stayed open after Escape: {:?}", p.drawn);
    }

    /// ONCE THE LIST IS DOWN, ESCAPE CLOSES THE POPUP — the ladder is not lost.
    #[test]
    fn escape_after_the_list_still_closes_the_popup() {
        let mut app = App::default();
        app.project.parameters.push(qymcad_core::model::Param { name: "width".into(), expr: "50".into(), value: 50.0 });
        let (si, ci) = sketch_with_dim(&mut app);
        let mut p = Popup::new();
        p.into_value_field(&mut app, si, ci);
        p.type_text("wid").frame(&mut app, si, ci);
        p.frame(&mut app, si, ci);

        p.key(egui::Key::Escape).frame(&mut app, si, ci); // the list
        p.key(egui::Key::Escape).frame(&mut app, si, ci); // out of the field
        p.key(egui::Key::Escape).frame(&mut app, si, ci); // the popup
        assert!(matches!(app.inline, InlineEdit::None), "with the list closed Escape no longer closes the popup");
    }

    /// TYPING A DRIVER NAME DOES NOT TOUCH THE DOCUMENT.
    ///
    /// Exactly the reported complaint. The document key is computed over the whole model: it did not
    /// change, so not one letter travelled into the model and there was nothing to rebuild.
    #[test]
    fn typing_a_driver_name_does_not_rebuild_anything() {
        let mut app = App::default();
        app.project.new_document();
        let (si, ci) = sketch_with_dim(&mut app);

        let mut p = Popup::new();
        p.into_driver_field(&mut app, si, ci);
        let key_before = app.doc_key_for_test();
        let undo_before = app.undo_len_for_test();

        for c in "dlina".chars() {
            p.type_text(&c.to_string()).frame(&mut app, si, ci);
            assert_eq!(app.doc_key_for_test(), key_before, "the document changed on the letter \"{c}\" — the edit goes into the model under the fingers");
        }
        assert_eq!(app.undo_len_for_test(), undo_before, "typing bred undo steps");
        assert!(app.project.named_dims.is_empty(), "the name was written into the model before it was committed: {:?}", app.project.named_dims);
    }

    /// ENTER GIVES THE NAME — once, and as one undo step.
    #[test]
    fn enter_names_the_dimension_once() {
        let mut app = App::default();
        app.project.new_document();
        let (si, ci) = sketch_with_dim(&mut app);

        let mut p = Popup::new();
        p.into_driver_field(&mut app, si, ci);
        let undo_before = app.undo_len_for_test();
        p.type_text("dlina").frame(&mut app, si, ci);
        p.key(egui::Key::Enter).frame(&mut app, si, ci);

        assert_eq!(app.project.named_dims.len(), 1, "the dimension did not get a name on Enter: {:?}", app.project.named_dims);
        assert_eq!(app.project.named_dims[0].name, "dlina");
        assert_eq!(app.undo_len_for_test(), undo_before + 1, "a driver name must be ONE undo step");
        assert_eq!(app.project.param_map().get("dlina"), Some(&40.0), "the driver is not visible in the formulas");
    }

    /// A TAKEN NAME: NOT APPLIED, THE LETTERS INTACT, THE OWNER NAMED.
    ///
    /// Reported behaviour: enter `len` as a driver in one sketch, go to another sketch, start typing
    /// `len` — and the `n` is deleted automatically. What if `lena` or `length` was what was wanted?
    #[test]
    fn a_taken_name_is_refused_and_says_whose_it_is() {
        let mut app = App::default();
        app.project.new_document();
        // The first sketch already owns the name "len".
        let comp = app.project.add_component("Housing");
        app.project.set_active_component(Some(comp));
        let (si0, _) = sketch_with_dim(&mut app);
        let sid0 = app.project.sketches[si0].id;
        let pts: Vec<Id> = app.project.sketches[si0].points.iter().take(2).map(|q| q.id).collect();
        assert!(app.project.add_named_dim("len".into(), sid0, pts));

        // The second sketch — that is where the typing happens.
        let (si, ci) = sketch_with_dim(&mut app);
        let mut p = Popup::new();
        p.into_driver_field(&mut app, si, ci);
        p.type_text("len").frame(&mut app, si, ci);

        // THE LETTERS ARE INTACT while the name is being typed.
        assert!(p.shows("len"), "the typed name vanished from the field: {:?}", p.drawn);
        assert!(p.shows("Housing.Profile"), "it is not said whose name this is: {:?}", p.drawn);

        p.key(egui::Key::Enter).frame(&mut app, si, ci);
        assert_eq!(app.project.named_dims.len(), 1, "a taken name was applied after all — the project holds two \"len\": {:?}", app.project.named_dims);
    }
}
