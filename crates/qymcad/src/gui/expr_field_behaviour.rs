//! THE EXPRESSION FIELD AND THE LIST OF DRIVERS — CHECKED BY GESTURE.
//!
//! The checks drive a REAL mouse and keyboard over the frame: they type letters as `Event::Text`, press
//! the arrows and Enter, and read what is drawn and in what order it is drawn.
//!
//! Why exactly this way. The former checks called the handler directly and so saw neither the layer nor
//! the keyboard — while both were spotted at a glance from outside: the drop-down list was BEHIND the
//! input popup of the tool. A check saying "there is a call in the code" has already missed click
//! handlers that had been cut out; it proves nothing any more.
#[cfg(test)]
mod tests {
    use super::super::App;
    use qymcad_core::geom::Point2;
    use qymcad_core::model::{Constraint, Id, Param, Project};

    /// A part with a sketch and a named driver dimension.
    fn part_with_driver(p: &mut Project, part: &str, sketch: &str, driver: &str, len: f64) {
        let comp = p.add_component(part);
        p.set_active_component(Some(comp));
        let sid = p.add_line_sketch(sketch, vec![Point2::new(0.0, 0.0), Point2::new(len, 0.0), Point2::new(len, 10.0), Point2::new(0.0, 10.0)], true);
        let si = p.sketch_index(sid).unwrap();
        p.add_sketch_node(sid, sketch);
        let pts: Vec<Id> = p.sketches[si].points.iter().map(|q| q.id).collect();
        p.sketches[si].constraints.push(Constraint::Distance { a: pts[0], b: pts[1], d: len, off: 0.0, expr: String::new(), driven: false, axis: 0 });
        assert!(p.add_named_dim(driver.into(), sid, vec![pts[0], pts[1]]));
    }

    /// THE DESK A PERSON SITS AT: a real egui context, one field in the frame, a queue of events.
    struct Desk {
        ctx: egui::Context,
        screen: egui::Rect,
        events: Vec<egui::Event>,
        /// The text of the last frame IN THE ORDER IT WAS DRAWN: it shows not only WHAT is drawn but
        /// also WHAT IS ON TOP OF WHAT.
        drawn: Vec<String>,
        /// What the field reported on the last frame.
        committed: bool,
        cancelled: bool,
        text: String,
        /// WHERE THE FIELD ENDED UP IN THE FRAME. Aiming by eye is not allowed: inside a popup the
        /// field stands under the heading, and "roughly there" misses — and then the check is green or
        /// red by accident rather than on the merits.
        field_rect: egui::Rect,
    }

    impl Desk {
        fn new() -> Self {
            let ctx = egui::Context::default();
            super::super::install_fonts(&ctx);
            Self {
                ctx,
                screen: egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, 900.0)),
                events: Vec::new(),
                drawn: Vec::new(),
                committed: false,
                cancelled: false,
                text: String::new(),
                field_rect: egui::Rect::NOTHING,
            }
        }

        fn key(&mut self, key: egui::Key) -> &mut Self {
            for pressed in [true, false] {
                self.events.push(egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers: Default::default() });
            }
            self
        }

        fn ctrl_space(&mut self) -> &mut Self {
            let modifiers = egui::Modifiers::COMMAND;
            self.events.push(egui::Event::Key { key: egui::Key::Space, physical_key: None, pressed: true, repeat: false, modifiers });
            self.events.push(egui::Event::Key { key: egui::Key::Space, physical_key: None, pressed: false, repeat: false, modifiers });
            self
        }

        /// Type text — letter by letter, as a person does.
        fn type_text(&mut self, s: &str) -> &mut Self {
            for c in s.chars() {
                self.events.push(egui::Event::Text(c.to_string()));
            }
            self
        }

        /// BRING THE CURSOR OVER. A measurement: on the frame where the pointer first appears over the
        /// field, the hover has not reached it yet (`hovered=false`), and a click on the same frame is
        /// wasted. A person moves the mouse and then presses — the check does the same.
        fn hover(&mut self, at: egui::Pos2) -> &mut Self {
            self.events.push(egui::Event::PointerMoved(at));
            self
        }

        fn click(&mut self, at: egui::Pos2) -> &mut Self {
            self.events.push(egui::Event::PointerMoved(at));
            self.events.push(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() });
            self.events.push(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() });
            self
        }

        /// One frame: the field is drawn and the events collected go into it.
        fn frame(&mut self, app: &mut App, model: &str) -> &mut Self {
            let input = egui::RawInput { screen_rect: Some(self.screen), events: std::mem::take(&mut self.events), ..Default::default() };
            let id = egui::Id::new("field_under_test");
            let (mut committed, mut cancelled, mut text) = (false, false, String::new());
            let mut rect = egui::Rect::NOTHING;
            let out = self.ctx.run_ui(input, |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    let o = super::super::expr_field::expr_field(ui, &app.project, id, model, 160.0, "");
                    committed = o.committed;
                    cancelled = o.cancelled;
                    text = o.text;
                    rect = o.resp.rect;
                });
            });
            self.committed = committed;
            self.cancelled = cancelled;
            self.text = text;
            self.field_rect = rect;
            self.drawn.clear();
            for cs in &out.shapes {
                super::super::screen_keys::tests::collect_text(&cs.shape, &mut self.drawn);
            }
            self
        }

        /// LET THE FRAME SETTLE. A measurement: on the frame where the list OPENED it is not yet drawn
        /// — an egui area takes a frame to fall into place, and the text of the list is absent from the
        /// first pass:
        ///
        /// ```text
        /// the frame of the typing ["len"]
        /// the next one            ["len", "lena", "5.000", "len", "Body.Profile", "20.000", ...]
        /// ```
        ///
        /// In the program this goes unnoticed (the frames run continuously), but the check must look
        /// where a person looks — at a settled frame rather than an intermediate one.
        fn settle(&mut self, app: &mut App, model: &str) -> &mut Self {
            self.frame(app, model);
            self.frame(app, model)
        }

        /// THE SAME FRAME, BUT THE FIELD STANDS INSIDE THE POPUP OF A TOOL — as in the sketcher and at
        /// the features.
        ///
        /// Dimension popups live in `egui::Order::Foreground`; the list must end up HIGHER, otherwise it
        /// goes behind them — exactly what was pointed at: the drop-down list behind the input popup of
        /// the tool.
        fn frame_in_popup(&mut self, app: &mut App, model: &str) -> &mut Self {
            let input = egui::RawInput { screen_rect: Some(self.screen), events: std::mem::take(&mut self.events), ..Default::default() };
            let id = egui::Id::new("field_under_test");
            let mut rect = egui::Rect::NOTHING;
            let out = self.ctx.run_ui(input, |ui| {
            // The frame hands in the root `Ui` now; the context comes from it.
            let ctx = &ui.ctx().clone();
                egui::CentralPanel::default().show(ui, |ui| {
                    ui.label("SCENE");
                });
                egui::Area::new(egui::Id::new("tool_popup")).order(egui::Order::Foreground).fixed_pos(egui::pos2(20.0, 20.0)).show(ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.label("TOOL-POPUP");
                        rect = super::super::expr_field::expr_field(ui, &app.project, id, model, 160.0, "").resp.rect;
                        ui.label("POPUP-BOTTOM");
                    });
                });
            });
            self.field_rect = rect;
            self.drawn.clear();
            for cs in &out.shapes {
                super::super::screen_keys::tests::collect_text(&cs.shape, &mut self.drawn);
            }
            self
        }

        /// The ordinal of a piece of text in the frame. A frame is drawn bottom upwards, so a larger
        /// ordinal means "drawn ON TOP".
        fn drawn_at(&self, what: &str) -> Option<usize> {
            self.drawn.iter().position(|t| t.contains(what))
        }

        /// Where to click to hit the field: the middle of the rectangle the field took up in the last
        /// frame.
        fn field_pos(&self) -> egui::Pos2 {
            self.field_rect.center()
        }

        fn shows(&self, what: &str) -> bool {
            self.drawn.iter().any(|t| t.contains(what))
        }
    }

    fn app_with_drivers() -> App {
        let mut app = App::default();
        app.project.new_document();
        part_with_driver(&mut app.project, "Body", "Profile", "len", 20.0);
        part_with_driver(&mut app.project, "Lid", "Outline", "len_lid", 70.0);
        app.project.parameters.push(Param { name: "lena".into(), expr: "5".into(), value: 5.0 });
        app
    }

    /// TYPING COMMITS NOTHING.
    ///
    /// While a person types, the caller receives not one "committed" — so nobody touches the model and
    /// there is nothing to rebuild. That is the main law, checked by gesture.
    #[test]
    fn typing_commits_nothing() {
        let mut app = app_with_drivers();
        let mut d = Desk::new();
        d.frame(&mut app, "10");
        let at = d.field_pos();
        d.click(at).frame(&mut app, "10");

        let mut commits = 0;
        for c in "dlina".chars() {
            d.type_text(&c.to_string()).frame(&mut app, "10");
            if d.committed {
                commits += 1;
            }
        }
        assert_eq!(commits, 0, "typing five letters gave {commits} commits — the model is being edited under the fingers");
    }

    /// ENTER COMMITS — ONCE.
    #[test]
    fn enter_commits_once_and_gives_the_typed_text() {
        let mut app = app_with_drivers();
        let mut d = Desk::new();
        d.frame(&mut app, "10");
        let at = d.field_pos();
        d.click(at).frame(&mut app, "10");
        // What is typed is "1025" — there are no drivers under such a name, the list does not open, and
        // Enter belongs to the field. The first edition of the check pressed Escape here "just in case"
        // and cancelled the edit itself, then wondered why Enter committed nothing.
        d.type_text("25").frame(&mut app, "10");

        d.key(egui::Key::Enter).frame(&mut app, "10");
        assert!(d.committed, "Enter must commit the edit");
        assert_eq!(d.text.trim(), "1025", "the wrong text was committed: {:?}", d.text);

        d.frame(&mut app, "1025");
        assert!(!d.committed, "the commit repeated on the next frame — the edit would go into the model twice");
    }

    /// ESCAPE RESTORES WHAT WAS THERE AND COMMITS NOTHING.
    #[test]
    fn escape_restores_the_model_text() {
        let mut app = app_with_drivers();
        let mut d = Desk::new();
        d.frame(&mut app, "10");
        let at = d.field_pos();
        d.click(at).frame(&mut app, "10");
        // There is no list here (there are no matches under "1099"), so Escape cancels the edit at once.
        d.type_text("99").frame(&mut app, "10");
        d.key(egui::Key::Escape).frame(&mut app, "10");

        assert!(d.cancelled, "Escape must cancel the edit");
        assert!(!d.committed, "Escape committed the edit — the model changed in spite of the cancellation");
        assert_eq!(d.text, "10", "the field did not go back to what is written in the model: {:?}", d.text);

        d.frame(&mut app, "10");
        assert_eq!(d.text, "10", "after the cancellation what was typed stayed in the field");
    }

    /// THE LIST DOES NOT CLIMB UP FROM FOCUS ALONE, BUT IT OPENS ON Ctrl+Space.
    #[test]
    fn the_list_does_not_appear_on_a_bare_click() {
        let mut app = app_with_drivers();
        let mut d = Desk::new();
        d.frame(&mut app, "");
        let at = d.field_pos();
        d.click(at).frame(&mut app, "");
        assert!(!d.shows("Body"), "the list opened from a single click in the field: {:?}", d.drawn);

        d.ctrl_space().settle(&mut app, "");
        assert!(d.shows("Body"), "Ctrl+Space did not open the list: {:?}", d.drawn);
    }

    /// TYPING OPENS THE LIST, AND THE PATH IS VISIBLE IN IT.
    #[test]
    fn typing_opens_the_list_with_paths_and_values() {
        let mut app = app_with_drivers();
        let mut d = Desk::new();
        d.frame(&mut app, "");
        let at = d.field_pos();
        d.click(at).frame(&mut app, "");
        d.type_text("len").settle(&mut app, "");

        assert!(d.shows("len"), "the name is not in the list: {:?}", d.drawn);
        assert!(d.shows("Body.Profile"), "the path to the part and the sketch is not in the list: {:?}", d.drawn);
        assert!(d.shows("20.000"), "the value of the driver is not in the list: {:?}", d.drawn);
    }

    /// THE ARROWS AND ENTER INSERT WHAT IS SELECTED.
    ///
    /// The list used to have no keyboard at all: choosing was possible only with the mouse. In grown-up
    /// CAD such a list walks with the arrows and the hands never leave the keyboard.
    #[test]
    fn arrows_and_enter_insert_the_selected_driver() {
        let mut app = app_with_drivers();
        let mut d = Desk::new();
        d.frame(&mut app, "");
        let at = d.field_pos();
        d.click(at).frame(&mut app, "");
        d.type_text("len").settle(&mut app, "");

        // The order of the list: a match from the start of the name outranks the rest, then by document.
        let order: Vec<String> = app.project.drivers_matching("len").into_iter().map(|x| x.name).collect();
        assert!(order.len() >= 2, "checking the arrows needs at least two matches: {order:?}");

        d.key(egui::Key::ArrowDown).frame(&mut app, "");
        d.key(egui::Key::Enter).frame(&mut app, "");
        assert_eq!(d.text, order[1], "the arrow plus Enter inserted something other than the SECOND row of the list: {:?} with the order {order:?}", d.text);
        assert!(!d.committed, "an insertion from the list must not commit the edit — the formula is still being written");
    }

    /// THE LIST FOLLOWS THE WORD UNDER THE CARET, NOT THE TAIL OF THE STRING.
    ///
    /// Reported behaviour: choosing a variable from the list while standing in the middle of the text
    /// puts it anywhere at all. The first half of that is that the list does not even offer the right
    /// thing: the word is looked for from the END of the whole string, so with the caret after `le` in
    /// `10+le*2` the search runs on `2` and nothing matches.
    #[test]
    fn the_list_follows_the_word_under_the_caret() {
        let mut app = app_with_drivers();
        let mut d = Desk::new();
        d.frame(&mut app, "");
        let at = d.field_pos();
        d.click(at).frame(&mut app, "");
        d.type_text("10+le*2").settle(&mut app, "");

        // BACK TO THE MIDDLE: two steps left put the caret right after `le`.
        d.key(egui::Key::ArrowLeft).key(egui::Key::ArrowLeft).frame(&mut app, "");
        d.ctrl_space().settle(&mut app, "");

        assert!(d.shows("Body.Profile"), "the list does not offer the word under the caret (`le`): {:?}", d.drawn);
    }

    /// AN INSERTION IN THE MIDDLE KEEPS THE TAIL.
    ///
    /// Reported behaviour: the variable is inserted anywhere at all. What was typed after the word
    /// disappeared: the name was glued to the head of the string and everything past the word was
    /// thrown away, so `10+le*2` turned into `10+len` and the `*2` was lost.
    #[test]
    fn inserting_in_the_middle_keeps_the_tail() {
        let mut app = app_with_drivers();
        let mut d = Desk::new();
        d.frame(&mut app, "");
        let at = d.field_pos();
        d.click(at).frame(&mut app, "");
        d.type_text("10+le*2").settle(&mut app, "");
        d.key(egui::Key::ArrowLeft).key(egui::Key::ArrowLeft).frame(&mut app, "");
        d.ctrl_space().settle(&mut app, "");

        let order: Vec<String> = app.project.drivers_matching("le").into_iter().map(|x| x.name).collect();
        assert!(!order.is_empty(), "setup: `le` must match something: {order:?}");

        d.key(egui::Key::Enter).frame(&mut app, "");
        assert_eq!(d.text, format!("10+{}*2", order[0]), "the insertion lost what was typed after the word: {:?}", d.text);
    }

    /// ESCAPE CLOSES THE LIST, NOT THE EDIT.
    #[test]
    fn escape_closes_the_list_first() {
        let mut app = app_with_drivers();
        let mut d = Desk::new();
        d.frame(&mut app, "");
        let at = d.field_pos();
        d.click(at).frame(&mut app, "");
        d.type_text("len").settle(&mut app, "");
        assert!(d.shows("Body.Profile"), "the list must be open before Escape: {:?}", d.drawn);

        d.key(egui::Key::Escape).frame(&mut app, "");
        assert!(!d.shows("Body.Profile"), "Escape did not close the list: {:?}", d.drawn);
        assert!(!d.cancelled, "the first Escape cancelled THE EDIT while it should have closed only the list");
        assert_eq!(d.text, "len", "what was typed vanished along with the list: {:?}", d.text);
    }

    /// A CLICK ON A ROW INSERTS THE NAME AND COMMITS NOTHING.
    #[test]
    fn clicking_a_row_inserts_the_name() {
        let mut app = app_with_drivers();
        let mut d = Desk::new();
        d.frame(&mut app, "");
        let at = d.field_pos();
        d.click(at).frame(&mut app, "");
        d.type_text("lena").settle(&mut app, "");
        assert!(d.shows("lena"), "the list must offer \"lena\": {:?}", d.drawn);

        // The first row of the list is right under the field.
        let row = egui::pos2(30.0, 45.0);
        d.click(row).frame(&mut app, "");
        assert_eq!(d.text, "lena", "the click on the row did not substitute the name: {:?}", d.text);
        assert!(!d.committed, "the click on the row committed the edit");
    }

    /// THE LIST IS DRAWN ON TOP OF THE POPUP OF A TOOL.
    ///
    /// Reported behaviour: the drop-down list is behind the input popup of the tool. And so it was: the
    /// list was drawn in `Order::Foreground` — the same place as the dimension popups — and within one
    /// layer the area being interacted with comes to the top, that is, the popup. What is checked is not
    /// "the layer is right" but THE ORDER OF DRAWING in the frame: whoever is later is on top.
    #[test]
    fn the_list_is_drawn_above_a_tool_popup() {
        let mut app = app_with_drivers();
        let mut d = Desk::new();
        d.frame_in_popup(&mut app, "");
        d.frame_in_popup(&mut app, "");
        // The field stands inside the popup, under its heading — the aim is taken from its rectangle in
        // the frame.
        let at = d.field_pos();
        d.hover(at).frame_in_popup(&mut app, "");
        d.click(at).frame_in_popup(&mut app, "");
        d.type_text("len").frame_in_popup(&mut app, "");
        d.frame_in_popup(&mut app, "");

        let popup_bottom = d.drawn_at("POPUP-BOTTOM").unwrap_or_else(|| panic!("the popup is not drawn: {:?}", d.drawn));
        let list_row = d.drawn_at("Body.Profile").unwrap_or_else(|| panic!("the list did not open inside the popup: {:?}", d.drawn));
        assert!(
            list_row > popup_bottom,
            "the list is drawn UNDER the popup (the row of the list is no. {list_row}, the bottom of the popup no. {popup_bottom}) — it will go behind it again: {:?}",
            d.drawn
        );
    }

    /// NOTHING MATCHES, NOTHING IS SHOWN.
    #[test]
    fn nothing_matching_shows_no_list() {
        let mut app = app_with_drivers();
        let mut d = Desk::new();
        d.frame(&mut app, "");
        let at = d.field_pos();
        d.click(at).frame(&mut app, "");
        d.type_text("zzz").settle(&mut app, "");
        assert!(!d.shows("Body"), "the list showed something that does not match: {:?}", d.drawn);
    }
}
