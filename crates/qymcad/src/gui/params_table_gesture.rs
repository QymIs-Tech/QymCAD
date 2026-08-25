//! THE PARAMETERS TABLE UNDER SOMEBODY'S FINGERS.
//!
//! What is measured is not "it looks right" but THE CONSEQUENCES: whether the document was touched (the
//! key of the document), how many steps of undo piled up, whether the references followed a renamed
//! name. The report was about exactly those consequences — a rebuild of the whole project on every
//! letter typed into a driver name — and consequences cannot be seen "by eye" in a check, only
//! measured.
#[cfg(test)]
mod tests {
    use super::super::App;
    use qymcad_core::model::Param;

    /// A desk with the parameters table open.
    struct Table {
        ctx: egui::Context,
        screen: egui::Rect,
        events: Vec<egui::Event>,
        /// The rectangles of the name fields by row number — FROM THE FRAME rather than by eye.
        name_rects: Vec<egui::Rect>,
        drawn: Vec<String>,
    }

    impl Table {
        fn new() -> Self {
            let ctx = egui::Context::default();
            super::super::install_fonts(&ctx);
            Self {
                ctx,
                screen: egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0)),
                events: Vec::new(),
                name_rects: Vec::new(),
                drawn: Vec::new(),
            }
        }

        fn frame(&mut self, app: &mut App) -> &mut Self {
            let input = egui::RawInput { screen_rect: Some(self.screen), events: std::mem::take(&mut self.events), ..Default::default() };
            // THE REAL WINDOW IS DRAWN, NOT THE ROWS OF THE TABLE ALONE.
            //
            // The first edition called `params_rows_ui` directly — and missed EVERYTHING the window does
            // after an edit: the recomputation of the parameters and the rebuild of the bodies. The check
            // for the reported case (300 in the table while the part stayed as it was) would have been
            // green with the program broken.
            app.win.params = true;
            let out = self.ctx.run(input, |ctx| {
                app.params_window(ctx);
            });
            // The name fields stand in the first column; egui remembers their rectangles under our own
            // ids.
            self.name_rects.clear();
            for i in 0..app.project.parameters.len() {
                let id = egui::Id::new(("par_name", i));
                if let Some(r) = self.ctx.memory(|m| m.area_rect(id)).or_else(|| self.ctx.read_response(id).map(|r| r.rect)) {
                    self.name_rects.push(r);
                }
            }
            self.drawn.clear();
            for cs in &out.shapes {
                super::super::screen_keys::tests::collect_text(&cs.shape, &mut self.drawn);
            }
            self
        }

        fn settle(&mut self, app: &mut App) -> &mut Self {
            self.frame(app);
            self.frame(app)
        }

        fn hover(&mut self, at: egui::Pos2) -> &mut Self {
            self.events.push(egui::Event::PointerMoved(at));
            self
        }

        fn click(&mut self, at: egui::Pos2) -> &mut Self {
            self.events.push(egui::Event::PointerMoved(at));
            for pressed in [true, false] {
                self.events.push(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() });
            }
            self
        }

        /// Select everything in the field — with the same Ctrl+A a person uses.
        fn select_all(&mut self) -> &mut Self {
            let modifiers = egui::Modifiers::COMMAND;
            for pressed in [true, false] {
                self.events.push(egui::Event::Key { key: egui::Key::A, physical_key: None, pressed, repeat: false, modifiers });
            }
            self
        }

        fn key(&mut self, key: egui::Key) -> &mut Self {
            for pressed in [true, false] {
                self.events.push(egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers: Default::default() });
            }
            self
        }

        fn type_text(&mut self, s: &str) -> &mut Self {
            for c in s.chars() {
                self.events.push(egui::Event::Text(c.to_string()));
            }
            self
        }

        /// Step into the name field of row `row`: bring the cursor, click, select what was there.
        fn into_name(&mut self, app: &mut App, row: usize) -> &mut Self {
            self.settle(app);
            let at = self.name_rects.get(row).copied().unwrap_or_else(|| panic!("the frame holds no name field for row {row}")).center();
            self.hover(at).frame(app);
            self.click(at).frame(app);
            self.select_all().frame(app)
        }

        /// Where the path of driver number `k` is drawn — a coordinate FROM THE FRAME.
        ///
        /// The first edition aimed "roughly there" (ten points above the value field) and missed: the
        /// click went into emptiness while the check declared the jump broken. Now the window puts the
        /// rectangles of its captions into the frame the way the tree puts its rows.
        fn path_pos(&self, app: &App, k: usize) -> Option<egui::Pos2> {
            app.tree.drv_path_rects.iter().find(|(i, _)| *i == k).map(|(_, r)| r.center())
        }

        /// Where the value field of driver number `k` stands — a coordinate FROM THE FRAME.
        fn driver_value_pos(&self, k: usize) -> Option<egui::Pos2> {
            self.ctx.read_response(egui::Id::new(("drv_val", k))).map(|r| r.rect.center())
        }

        /// THE SAME COORDINATE, BUT ONCE THE WINDOW HAS SETTLED INTO PLACE.
        ///
        /// A measurement: the parameters window lays itself out over the first few frames and the field
        /// travels; a click on a stale rectangle lands in emptiness and the check declares the edit
        /// broken. That very rake was already stepped on with the dimension popup.
        fn stable_driver_value_pos(&mut self, app: &mut App, k: usize) -> Option<egui::Pos2> {
            let mut last: Option<egui::Pos2> = None;
            let mut still = 0;
            for _ in 0..20 {
                self.frame(app);
                let now = self.driver_value_pos(k);
                still = if now == last && now.is_some() { still + 1 } else { 0 };
                last = now;
                if still >= 2 {
                    break;
                }
            }
            last
        }

        fn shows(&self, what: &str) -> bool {
            self.drawn.iter().any(|t| t.contains(what))
        }
    }

    fn app_with_params() -> App {
        let mut app = App::default();
        app.project.new_document();
        app.project.parameters.push(Param { name: "w".into(), expr: "50".into(), value: 50.0 });
        app.project.parameters.push(Param { name: "h".into(), expr: "w*2+5".into(), value: 105.0 });
        app
    }

    /// TYPING A NAME DOES NOT TOUCH THE DOCUMENT.
    ///
    /// This is exactly what was reported: a rebuild of the whole project on every letter typed into a
    /// driver name. The key of the document is computed over the whole model: if it did not change, the
    /// model was not touched once.
    #[test]
    fn typing_a_name_does_not_touch_the_document() {
        let mut app = app_with_params();
        let key_before = app.doc_key_for_test();
        let undo_before = app.undo_len_for_test();

        let mut t = Table::new();
        t.into_name(&mut app, 0);
        for c in "shirina".chars() {
            t.type_text(&c.to_string()).frame(&mut app);
            assert_eq!(app.doc_key_for_test(), key_before, "the document changed on the letter \"{c}\" — the edit goes into the model under the fingers");
        }
        assert_eq!(app.undo_len_for_test(), undo_before, "typing bred steps of undo");
        assert_eq!(app.project.parameters[0].name, "w", "the name in the model changed before the commit");
        assert_eq!(app.project.parameters[1].expr, "w*2+5", "the formula broke before the name was even finished");
    }

    /// ENTER COMMITS ONCE, AND THE REFERENCES FOLLOW THE NAME.
    #[test]
    fn enter_renames_and_carries_the_formulas() {
        let mut app = app_with_params();
        let undo_before = app.undo_len_for_test();

        let mut t = Table::new();
        t.into_name(&mut app, 0);
        t.type_text("shirina").frame(&mut app);
        t.key(egui::Key::Enter).frame(&mut app);
        t.frame(&mut app);

        assert_eq!(app.project.parameters[0].name, "shirina", "the name was not renamed by Enter");
        assert_eq!(app.project.parameters[1].expr, "shirina*2+5", "the formula stayed on a name that has vanished");
        assert_eq!(app.undo_len_for_test(), undo_before + 1, "a rename must be ONE step of undo");
        assert_eq!(app.project.eval_expr("shirina*2+5").unwrap(), 105.0, "the value changed because of the rename");
    }

    /// A TAKEN NAME: THE COMMIT DOES NOT GO THROUGH, WHAT WAS TYPED IS INTACT, THE REASON IS WRITTEN.
    ///
    /// Reported: typing `len` had the `n` deleted automatically, with a request to block the commit on
    /// Enter instead and write below in yellow that a driver of that name already exists.
    #[test]
    fn a_taken_name_is_refused_without_eating_letters() {
        let mut app = app_with_params();
        let mut t = Table::new();
        t.into_name(&mut app, 0);
        t.type_text("h").frame(&mut app); // the name of the second parameter
        t.key(egui::Key::Enter).frame(&mut app);
        t.frame(&mut app);

        assert_eq!(app.project.parameters[0].name, "w", "a taken name was applied after all — there are two \"h\" in the project");
        // THE CATALOGUE IS ASKED RATHER THAN A LITERAL IN THE CODE: a check written against words of one
        // language fails whenever the run uses another, and not because the program stayed silent.
        let said = crate::i18n::tr2("par-name-taken", "name", "h", "where", &crate::i18n::tr("par-owner-project"));
        assert!(t.shows(&said), "the program did not say why it refused: \"{said}\" was expected, and the frame holds {:?}", t.drawn);
        assert!(t.shows("h"), "the name typed vanished from the field: {:?}", t.drawn);
    }

    /// A NAME UNUSABLE IN A FORMULA IS REFUSED WITH AN EXPLANATION AS WELL.
    #[test]
    fn a_name_that_cannot_be_a_formula_is_refused() {
        let mut app = app_with_params();
        let mut t = Table::new();
        t.into_name(&mut app, 0);
        t.type_text("2w").frame(&mut app);
        t.key(egui::Key::Enter).frame(&mut app);
        t.frame(&mut app);

        assert_eq!(app.project.parameters[0].name, "w", "the name \"2w\" was applied — inside a formula it falls apart");
        let said = crate::i18n::tr1("par-name-bad", "name", "2w");
        assert!(t.shows(&said), "the program did not explain the refusal: \"{said}\" was expected, and the frame holds {:?}", t.drawn);
    }

    /// THE SEARCH LEAVES ONLY WHAT MATCHES.
    ///
    /// With a hundred and fifty names the list cannot be sorted out by eye. The search goes by name and
    /// by path alike — one remembers either what one called it or where it lies.
    #[test]
    fn the_search_narrows_the_table() {
        let mut app = app_with_params();
        app.project.parameters.push(Param { name: "shirina".into(), expr: "20".into(), value: 20.0 });

        let mut t = Table::new();
        t.settle(&mut app);
        assert!(t.shows("shirina") && t.shows("w"), "setup: both rows must be in the table");

        app.par_search_for_test("shir");
        t.settle(&mut app);
        assert!(t.shows("shirina"), "the search lost a row that matches: {:?}", t.drawn);
        assert!(!t.drawn.iter().any(|x| x == "h"), "the search left a row that does not match: {:?}", t.drawn);
    }

    /// THE VALUE OF A DIMENSION DRIVER IS EDITED FROM THE TABLE, AND THE SKETCH IS RECOMPUTED ONCE.
    ///
    /// The drivers used to hang in a look-but-do-not-touch list of their own: a name, a path, a number,
    /// and that was all.
    #[test]
    fn a_driver_value_is_edited_from_the_table() {
        use qymcad_core::geom::Point2;
        use qymcad_core::model::{Constraint, Id};
        let mut app = App::default();
        app.project.new_document();
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
        assert!(app.project.add_named_dim("len".into(), sid, vec![pts[0], pts[1]]));

        let mut t = Table::new();
        let at = t.stable_driver_value_pos(&mut app, 0).expect("the value field of the driver is in the frame");
        let undo_before = app.undo_len_for_test();
        t.hover(at).frame(&mut app);
        t.click(at).frame(&mut app);
        t.select_all().frame(&mut app);
        t.type_text("55").frame(&mut app);
        assert_eq!(app.project.param_map().get("len"), Some(&40.0), "the value went into the model BEFORE the commit");

        t.key(egui::Key::Enter).frame(&mut app);
        t.settle(&mut app);
        assert_eq!(app.project.param_map().get("len"), Some(&55.0), "the value of a driver is not editable from the table");
        assert_eq!(app.undo_len_for_test(), undo_before + 1, "editing a value must be ONE step of undo");
        // AND THE GEOMETRY FOLLOWED THE DIMENSION: the sketch was recomputed rather than a number merely
        // rewritten.
        let w = app.project.sketches[si].points[1].x - app.project.sketches[si].points[0].x;
        assert!((w.abs() - 55.0).abs() < 1e-6, "the sketch did not recompute for the new dimension: the width is {w}");
    }

    /// A CLICK ON THE PATH LEADS TO THAT VERY SKETCH.
    ///
    /// It was asked that it be plain which sketch, body or assembly they come from. The path answers the
    /// question, and a click on it carries the answer home: one arrives there instead of searching by
    /// hand.
    #[test]
    fn clicking_the_path_goes_to_the_sketch() {
        use qymcad_core::geom::Point2;
        use qymcad_core::model::{Constraint, Id};
        let mut app = App::default();
        app.project.new_document();
        let comp = app.project.add_part("Body");
        app.enter_component(comp);
        let sid = app.project.add_line_sketch(
            "Profile",
            vec![Point2::new(0.0, 0.0), Point2::new(40.0, 0.0), Point2::new(40.0, 20.0), Point2::new(0.0, 20.0)],
            true,
        );
        let si = app.project.sketch_index(sid).unwrap();
        app.project.add_sketch_node(sid, "Profile");
        let pts: Vec<Id> = app.project.sketches[si].points.iter().take(2).map(|q| q.id).collect();
        app.project.sketches[si].constraints.push(Constraint::Distance {
            a: pts[0],
            b: pts[1],
            d: 40.0,
            off: 0.0,
            expr: String::new(),
            driven: false,
            axis: 0,
        });
        assert!(app.project.add_named_dim("len".into(), sid, pts));
        // We leave the part so that the jump is visible: it must also ENTER it.
        app.exit_context_for_test();
        app.sel = super::super::Sel::None;

        let mut t = Table::new();
        t.settle(&mut app);
        let at = t.path_pos(&app, 0).expect("the path of the driver is in the frame");
        let undo_before = app.undo_len_for_test();
        t.hover(at).frame(&mut app);
        t.click(at).frame(&mut app);

        assert!(app.sel == super::super::Sel::Sketch(si), "the click on the path did not lead to the sketch");
        assert_eq!(app.undo_len_for_test(), undo_before, "a jump is not an edit of the document, there must be no step of undo");
    }

    /// EDITING A DRIVER IN THE TABLE REBUILDS THE BODY, NOT ONLY THE SKETCH.
    ///
    /// Reported from a run by hand: `S_diametr_out` was changed from 90 to 300, the green circle took the
    /// larger diameter, and the part behind it never rebuilt.
    ///
    /// The cause lay in the edit that introduced this: the sketch was recomputed while the bodies
    /// standing on it were not marked for a rebuild. The general recomputation of the parameters did not
    /// save it either — that one touches only sketches with EXPRESSIONS, and a dimension driver is an
    /// ordinary NUMBER. What came out was a silent answer: 300 in the table, 90 on screen.
    ///
    /// The check measures THE SIZE OF THE BODY — the very thing a person looks at.
    #[test]
    fn editing_a_driver_rebuilds_the_body_not_only_the_sketch() {
        use qymcad_core::model::{Constraint, Id};
        let mut app = App::default();
        app.project.new_document();
        let comp = app.project.add_part("Ring");
        app.enter_component(comp);
        let si = app.project.new_sketch("Sketch 1");
        let sid = app.project.sketches[si].id;
        app.project.add_sketch_node(sid, "Sketch 1");
        let ent = app.project.add_circle_entity(si, 0.0, 0.0, 45.0, qymcad_core::feature::Purpose::Real);
        // THE DIMENSION OF A CIRCLE HOLDS ON TO ITS CENTRE and not to the entity: the radius in the
        // solver is a variable keyed by the centre (`RadiusVar { center }`). The first edition of the
        // check put the id of the ENTITY here — the constraint did not find its variable, the sketch
        // silently did not recompute, and the check blamed the rebuild of the bodies for it.
        let c = match app.project.sketches[si].entities.iter().find(|e| e.id == ent).map(|e| e.kind) {
            Some(qymcad_core::model::EntityKind::Circle { center, .. }) => center,
            _ => panic!("the circle was not created"),
        };
        app.project.sketches[si].constraints.push(Constraint::Diameter { c, d: 90.0, off: 0.0, expr: String::new(), driven: false, diam: true });
        app.project.regen_sketch(si);
        let prof = app.project.contour_id(0).unwrap_or(0);
        app.project.add_extrude_on(sid, prof, 10.0, qymcad_core::feature::Reach::Forward, 0.0);
        app.rebuild_if_dirty_for_test();
        app.drain_bg_for_test();
        app.rebuild_if_dirty_for_test();

        let width = |app: &App| {
            app.project
                .bodies
                .iter()
                .find_map(|b| b.mesh.bounds())
                .map(|bb| bb.max.x - bb.min.x)
                .unwrap_or(0.0)
        };
        let before = width(&app);
        assert!(before > 80.0 && before < 100.0, "setup: the body must be 90 mm across, and its width is {before:.1}");

        let refs: Vec<Id> = vec![c];
        assert!(app.project.add_named_dim("S_diametr_out".into(), sid, refs));

        let mut t = Table::new();
        let at = t.stable_driver_value_pos(&mut app, 0).expect("the value field of the driver is in the frame");
        t.hover(at).frame(&mut app);
        t.click(at).frame(&mut app);
        t.select_all().frame(&mut app);
        t.type_text("300").frame(&mut app);
        t.key(egui::Key::Enter).frame(&mut app);
        t.settle(&mut app);
        app.rebuild_if_dirty_for_test();
        app.drain_bg_for_test();
        app.rebuild_if_dirty_for_test();

        let after = width(&app);
        assert!(
            after > 290.0,
            "the table says 300 and the body stayed {after:.1} wide — the part did not rebuild after the driver"
        );
    }

    /// UNNAMED FEATURE NUMBERS STAY OUT OF THE TABLE.
    ///
    /// They were all shown at first, after the fashion of grown-up CAD. It was asked that the empty
    /// records, where no driver name was given, be taken out of the global parameters. The list of
    /// parameters is what somebody NAMED; rows with an empty name turn it into a dump.
    #[test]
    fn unnamed_feature_numbers_stay_out_of_the_table() {
        let mut app = App::default();
        app.project.new_document();
        let comp = app.project.add_part("Body");
        app.enter_component(comp);
        let sid = app.project.add_line_sketch(
            "Profile",
            vec![
                qymcad_core::geom::Point2::new(0.0, 0.0),
                qymcad_core::geom::Point2::new(40.0, 0.0),
                qymcad_core::geom::Point2::new(40.0, 20.0),
                qymcad_core::geom::Point2::new(0.0, 20.0),
            ],
            true,
        );
        app.project.add_sketch_node(sid, "Profile");
        let node = app.project.add_extrude_on(sid, 0, 25.0, qymcad_core::feature::Reach::Forward, 0.0);

        let mut t = Table::new();
        t.settle(&mut app);
        assert!(!t.shows("height"), "an unnamed feature number got into the parameters table: {:?}", t.drawn);

        // AND A NAMED ONE DOES GET IN: the list shows exactly what somebody named.
        assert!(app.project.add_named_feat_dim("vysota".into(), node, "height"));
        t.settle(&mut app);
        assert!(t.shows("vysota"), "a named feature number vanished from the table: {:?}", t.drawn);
        assert!(t.shows("Body"), "a named feature number has no path shown: {:?}", t.drawn);
    }
}
