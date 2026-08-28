//! A FEATURE PARAMETER IS NAMED AS A DRIVER BY A PERSON'S OWN HANDS.
//!
//! The requirement was stated plainly: features must have all of this, not sketches alone.
//!
//! The kernel had been able to do it for a long time, but IN THE INTERFACE there was NOWHERE to name
//! the height of an extrude: the feature properties had no numeric fields at all, and the command
//! opened only for editing a value. That was found when the check went by gesture — as it does
//! here.
#[cfg(test)]
mod tests {
    use super::super::App;
    use qymcad_core::geom::Point2;
    use qymcad_core::model::{DimTarget, Id};

    /// A part with an extrude. Returns (index of the node in the timeline, its id).
    fn part_with_extrude(app: &mut App) -> (usize, Id) {
        let comp = app.project.add_part("Housing");
        app.enter_component(comp);
        let sid = app.project.add_line_sketch(
            "Profile",
            vec![Point2::new(0.0, 0.0), Point2::new(40.0, 0.0), Point2::new(40.0, 20.0), Point2::new(0.0, 20.0)],
            true,
        );
        app.project.add_sketch_node(sid, "Profile");
        let node = app.project.add_extrude_on(sid, 0, 25.0, qymcad_core::feature::Reach::Forward, 0.0);
        let ti = app.project.timeline.iter().position(|n| n.id == node).expect("the node in the timeline");
        (ti, node)
    }

    /// The bench with the feature properties.
    struct Props {
        ctx: egui::Context,
        screen: egui::Rect,
        events: Vec<egui::Event>,
        drawn: Vec<String>,
    }

    impl Props {
        fn new() -> Self {
            let ctx = egui::Context::default();
            super::super::install_fonts(&ctx);
            Self { ctx, screen: egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 900.0)), events: Vec::new(), drawn: Vec::new() }
        }

        fn frame(&mut self, app: &mut App, ti: usize) -> &mut Self {
            let input = egui::RawInput { screen_rect: Some(self.screen), events: std::mem::take(&mut self.events), ..Default::default() };
            let out = self.ctx.run_ui(input, |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    app.feature_props(ui, ti);
                });
            });
            self.drawn.clear();
            for cs in &out.shapes {
                super::super::screen_keys::tests::collect_text(&cs.shape, &mut self.drawn);
            }
            self
        }

        fn settle(&mut self, app: &mut App, ti: usize) -> &mut Self {
            self.frame(app, ti);
            self.frame(app, ti)
        }

        fn field(&self, node: Id, key: &str) -> Option<egui::Rect> {
            self.ctx.read_response(egui::Id::new(("featdrv", node, key))).map(|r| r.rect)
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

        /// Get into the driver name field of parameter `key`: bring the cursor over, click.
        fn into_driver_field(&mut self, app: &mut App, ti: usize, node: Id, key: &str) -> &mut Self {
            self.settle(app, ti);
            let at = self.field(node, key).unwrap_or_else(|| panic!("the feature properties have no driver name field for \"{key}\"")).center();
            self.events.push(egui::Event::PointerMoved(at));
            self.frame(app, ti);
            self.events.push(egui::Event::PointerMoved(at));
            for pressed in [true, false] {
                self.events.push(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() });
            }
            self.frame(app, ti)
        }

        fn shows(&self, what: &str) -> bool {
            self.drawn.iter().any(|t| t.contains(what))
        }
    }

    /// EVERY NUMBER OF A FEATURE HAS A FIELD OF ITS OWN IN THE PROPERTIES.
    ///
    /// The list is taken from the same table the rebuild applies the parameters by — otherwise it
    /// would be possible to name as a driver something the build knows nothing about, and the other
    /// way round.
    #[test]
    fn feature_properties_show_every_number() {
        let mut app = App::default();
        app.project.new_document();
        let (ti, node) = part_with_extrude(&mut app);

        let mut p = Props::new();
        p.settle(&mut app, ti);
        assert!(p.field(node, "height").is_some(), "the height of the extrude has no driver name field: {:?}", p.drawn);
        assert!(p.field(node, "down").is_some(), "the second direction has no field: {:?}", p.drawn);
    }

    /// TYPING A NAME DOES NOT TOUCH THE DOCUMENT; ENTER IS ONE UNDO STEP.
    #[test]
    fn naming_a_feature_parameter_commits_once() {
        let mut app = App::default();
        app.project.new_document();
        let (ti, node) = part_with_extrude(&mut app);

        let mut p = Props::new();
        p.into_driver_field(&mut app, ti, node, "height");
        let key_before = app.doc_key_for_test();
        let undo_before = app.undo_len_for_test();

        for c in "vysota".chars() {
            p.type_text(&c.to_string()).frame(&mut app, ti);
            assert_eq!(app.doc_key_for_test(), key_before, "the document changed on the letter \"{c}\" — the edit goes into the model under the fingers");
        }
        assert!(app.project.named_dims.is_empty(), "the name was written before it was committed: {:?}", app.project.named_dims);

        p.key(egui::Key::Enter).frame(&mut app, ti);
        p.settle(&mut app, ti);

        assert_eq!(app.project.name_of_target(&DimTarget::Feature { node, key: "height".into() }), "vysota", "the height of the extrude did not get a name on Enter");
        assert_eq!(app.undo_len_for_test(), undo_before + 1, "the name must be ONE undo step");
        assert_eq!(app.project.param_map().get("vysota"), Some(&25.0), "a named feature parameter is not visible in the formulas");
    }

    /// A TAKEN NAME IS REFUSED, THE LETTERS ARE INTACT, THE OWNER IS NAMED — as in a sketch.
    ///
    /// Different behaviour in two similar places is a defect, even if both of them "work".
    #[test]
    fn a_taken_name_is_refused_in_feature_properties_too() {
        let mut app = App::default();
        app.project.new_document();
        app.project.parameters.push(qymcad_core::model::Param { name: "w".into(), expr: "50".into(), value: 50.0 });
        let (ti, node) = part_with_extrude(&mut app);

        let mut p = Props::new();
        p.into_driver_field(&mut app, ti, node, "height");
        p.type_text("w").frame(&mut app, ti);
        p.key(egui::Key::Enter).frame(&mut app, ti);
        p.settle(&mut app, ti);

        assert!(app.project.named_dims.is_empty(), "a taken name was applied after all: {:?}", app.project.named_dims);
        let said = crate::i18n::tr2("par-name-taken", "name", "w", "where", &crate::i18n::tr("par-owner-project"));
        assert!(p.shows(&said), "it is not said what the name is taken by: expected \"{said}\", in the frame {:?}", p.drawn);
        assert!(p.shows("w"), "the typed name vanished from the field: {:?}", p.drawn);
    }

    /// TYPING AN EXPRESSION ON A FEATURE TOUCHES NOTHING; COMMITTING IS ONE STEP.
    ///
    /// The neighbouring check guards the NAME field, and this one the EXPRESSION field: a different
    /// field, a different path into the model, and the law "editing text is not editing the model"
    /// must hold in both.
    #[test]
    fn typing_a_feature_expression_touches_nothing_until_enter() {
        let mut app = App::default();
        app.project.new_document();
        app.project.parameters.push(qymcad_core::model::Param { name: "w".into(), expr: "50".into(), value: 50.0 });
        let (ti, node) = part_with_extrude(&mut app);

        let mut p = Props::new();
        p.settle(&mut app, ti);
        let at = p
            .ctx
            .read_response(egui::Id::new(("featdim", node, "height")))
            .unwrap_or_else(|| panic!("the feature properties have no expression field for the height: {:?}", p.drawn))
            .rect
            .center();
        p.events.push(egui::Event::PointerMoved(at));
        p.frame(&mut app, ti);
        p.events.push(egui::Event::PointerMoved(at));
        for pressed in [true, false] {
            p.events.push(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() });
        }
        p.frame(&mut app, ti);

        let key_before = app.doc_key_for_test();
        let undo_before = app.undo_len_for_test();
        for c in "w/2".chars() {
            p.type_text(&c.to_string()).frame(&mut app, ti);
            assert_eq!(app.doc_key_for_test(), key_before, "the document changed on the character \"{c}\" — the expression goes into the model under the fingers");
        }
        assert!(app.project.feat_dim(node, "height").is_none_or(|e| e.is_empty()), "the expression was written before it was committed: {:?}", app.project.feat_dim(node, "height"));

        // ESCAPE IS NEITHER NEEDED NOR HARMLESS HERE. By the end of the typing the word is "2", there
        // are no drivers for it and the list is already closed; and with the list closed Escape
        // cancels THE EDIT. The first edition pressed it just in case and cancelled the very thing it
        // was checking.
        p.key(egui::Key::Enter).frame(&mut app, ti);
        p.settle(&mut app, ti);

        assert_eq!(app.project.feat_dim(node, "height"), Some("w/2"), "the expression was not committed on Enter");
        assert_eq!(app.undo_len_for_test(), undo_before + 1, "editing an expression must be ONE undo step");
    }
}
