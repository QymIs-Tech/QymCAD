//! PARSING THE WORD BEING TYPED AND SUBSTITUTING A NAME.
//!
//! What is left here are the two pure functions the autocompletion stands on: which word a person is
//! typing right now, and what to replace it with when one is chosen from the list. The list itself
//! lives in `expr_field.rs`, and it is checked by gesture in `expr_field_behaviour.rs`.
//!
//! The former checks of this file drew the OLD list (`driver_popup`), which has been thrown out
//! whole: it lived in the same layer as the tool popups and therefore went behind them, did not
//! listen to the keyboard and climbed onto the screen at a single click in the field. Everything they
//! checked has been rechecked on the new list.
//!
//! One check moved here and stayed: the tool bar of a PART offers drivers too. The requirement was
//! stated plainly: features must have all of this, not sketches alone.
#[cfg(test)]
mod tests {
    use super::super::{current_token, insert_driver, App};
    use qymcad_core::geom::Point2;
    use qymcad_core::model::{Constraint, Id, Project};

    /// A part with a sketch and a named driving dimension.
    fn part_with_driver(p: &mut Project, part: &str, sketch: &str, driver: &str, len: f64) {
        let comp = p.add_component(part);
        p.set_active_component(Some(comp));
        let sid = p.add_line_sketch(
            sketch,
            vec![Point2::new(0.0, 0.0), Point2::new(len, 0.0), Point2::new(len, 10.0), Point2::new(0.0, 10.0)],
            true,
        );
        let si = p.sketch_index(sid).unwrap();
        p.add_sketch_node(sid, sketch);
        let pts: Vec<Id> = p.sketches[si].points.iter().map(|q| q.id).collect();
        p.sketches[si].constraints.push(Constraint::Distance {
            a: pts[0],
            b: pts[1],
            d: len,
            off: 0.0,
            expr: String::new(),
            driven: false,
            axis: 0,
        });
        assert!(p.add_named_dim(driver.into(), sid, vec![pts[0], pts[1]]));
    }

    /// The word under a caret standing at the end of the text — the commonest case.
    fn token_at_end(text: &str) -> &str {
        current_token(text, text.len()).2
    }

    /// THE WORD BEING TYPED, NOT THE WHOLE LINE. An expression lives in the field; the suggestion
    /// must go by one fragment, otherwise after `w*2+` the list empties at exactly the moment a
    /// person started a new name.
    #[test]
    fn the_token_is_the_word_being_typed() {
        assert_eq!(token_at_end("len"), "len");
        assert_eq!(token_at_end("w*2+le"), "le");
        assert_eq!(token_at_end("w*2+"), "", "after an operator the word has not started yet");
        assert_eq!(token_at_end("(w+h)/dl"), "dl");
        assert_eq!(token_at_end(""), "");
    }

    /// THE WORD IS LOOKED FOR ON BOTH SIDES OF THE CARET.
    ///
    /// Reported behaviour: choosing a variable from the list while standing in the middle of the text
    /// puts it anywhere at all. Searching from the END of the whole line answered about the wrong
    /// word: with the caret after `le` in `10+le*2` the search ran on `2`.
    #[test]
    fn the_token_is_the_word_under_the_caret() {
        assert_eq!(current_token("10+le*2", 5).2, "le", "the caret stands right after `le`");
        assert_eq!(current_token("10+len*2", 5).2, "len", "the caret is INSIDE the word — the whole of it is taken");
        assert_eq!(current_token("10+len*2", 3).2, "len", "the caret is at the start of the word");
        assert_eq!(current_token("10+len*2", 0).2, "10", "at the very beginning the word is the first one");
        assert_eq!(current_token("10+*2", 3).2, "", "the caret stands on an operator: no word");
    }

    /// THE SUBSTITUTION REPLACES THE WORD UNDER THE CARET AND KEEPS THE TAIL.
    #[test]
    fn picking_replaces_the_word_and_keeps_the_tail() {
        let at_end = |t: &str, n: &str| insert_driver(t, n, t.len()).0;
        assert_eq!(at_end("w*2+le", "len"), "w*2+len");
        assert_eq!(at_end("", "len"), "len");
        assert_eq!(at_end("w*2+", "len"), "w*2+len");
        assert_eq!(at_end("len", "shirina"), "shirina");

        // What was typed after the word survives: it used to be thrown away whole.
        assert_eq!(insert_driver("10+le*2", "len", 5).0, "10+len*2");
        assert_eq!(insert_driver("10+le*2+w", "len", 5).0, "10+len*2+w");
    }

    /// THE CARET LANDS BEHIND THE INSERTED NAME so that typing carries on from there.
    #[test]
    fn the_caret_lands_behind_the_inserted_name() {
        let (text, at) = insert_driver("10+le*2", "len", 5);
        assert_eq!(&text[..at], "10+len", "the caret is not right after the name: {text:?} at {at}");
        assert_eq!(&text[at..], "*2");
    }

    /// A MULTIBYTE NAME IS NOT CUT IN THE MIDDLE OF A LETTER. Names come in other alphabets, and
    /// `i + 1` over bytes leads straight to a panic — that is exactly why this check stands here. The
    /// Cyrillic below is the test data itself: with an ASCII name the check would prove nothing.
    #[test]
    fn a_cyrillic_name_does_not_panic() {
        assert_eq!(token_at_end("ширина"), "ширина");
        assert_eq!(token_at_end("2*ши"), "ши");
        assert_eq!(insert_driver("2*ши", "ширина", "2*ши".len()).0, "2*ширина");

        // In the middle, with the tail also in Cyrillic: every boundary lands between letters.
        let text = "2*ши+длина";
        let caret = text.find('+').expect("the operator");
        assert_eq!(insert_driver(text, "ширина", caret).0, "2*ширина+длина");
    }

    /// THE FIELDS OF THE PART TOOLS OFFER DRIVERS TOO.
    ///
    /// The expression field in the tool bars is one for all of them (`num_or_expr`, see
    /// `expr_fields.rs`), so the check goes through it: wired in one place, it works in the extrude,
    /// the fillet, the chamfer, the shell, the hole, the draft and the pattern.
    #[test]
    fn the_tool_bar_field_offers_drivers_too() {
        let mut app = App::default();
        app.project.new_document();
        part_with_driver(&mut app.project, "Housing", "Profile", "len", 20.0);
        app.bar_exprs.insert("t_h", "le".into());

        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, 900.0));
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let mut texts = Vec::new();
        let mut field = egui::Rect::NOTHING;
        // FOCUS IS GIVEN BY A CLICK ON A COORDINATE FROM THE FRAME, not by a guessed id: the first
        // edition called `request_focus` on `ui.next_auto_id()`, the id did not match the real field,
        // and the check reported "nothing was offered" while the window was sound. And the cursor is
        // brought over FIRST: hovering reaches the field a frame later, and a click in the same frame
        // is wasted.
        for pass in 0..6 {
            let mut input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
            let at = field.center();
            if pass == 2 {
                input.events.push(egui::Event::PointerMoved(at));
            }
            if pass == 3 {
                input.events.push(egui::Event::PointerMoved(at));
                for pressed in [true, false] {
                    input.events.push(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() });
                }
            }
            if pass == 4 {
                input.events.push(egui::Event::Text("n".into()));
            }
            let out = ctx.run_ui(input, |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    app.num_or_expr(ui, "t_h", 10.0, 0.0, 100.0, false, "mm");
                });
            });
            if let Some(r) = ctx.read_response(egui::Id::new(("bar_expr", "t_h"))) {
                field = r.rect;
            }
            texts.clear();
            for cs in &out.shapes {
                super::super::screen_keys::tests::collect_text(&cs.shape, &mut texts);
            }
        }
        assert!(
            texts.iter().any(|t| t.contains("Housing") && t.contains("Profile")),
            "the field of a part tool did not offer a driver with its path: {texts:?}"
        );
    }
}
