//! A TEXT IS WRITTEN IN A FONT, AND IT KEEPS THAT FONT.
//!
//! The glyphs of a text are baked once and stored in the document; the font itself was kept nowhere. So any
//! re-bake - and every edit of the string or the height is one - used whatever font happened to be loaded at
//! that moment. Open a document again (nothing is loaded, the system default is taken), change the height of
//! your own label, and it is silently redrawn IN SOMEBODY ELSE'S TYPEFACE.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;

    const OURS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/fonts/LiberationSans-Bold.ttf");

    /// How wide the string comes out for its height - the one number that says which typeface it is in.
    fn shape_of(font: &[u8]) -> Option<f64> {
        let cs = qymcad_core::text::text_outline_contours(font, 0, "Ag", 20.0, 0.0, 0.0);
        let mut bb: Option<(f64, f64, f64, f64)> = None;
        for c in &cs {
            for p in &c.points {
                bb = Some(match bb {
                    None => (p.x, p.y, p.x, p.y),
                    Some((a, b, cc, d)) => (a.min(p.x), b.min(p.y), cc.max(p.x), d.max(p.y)),
                });
            }
        }
        bb.and_then(|(x0, y0, x1, y1)| ((y1 - y0) > 1e-9).then_some((x1 - x0) / (y1 - y0)))
    }

    /// A font of the system that writes the string NOTICEABLY differently - what an editing session after a
    /// reopen would fall back to. Looked for where the program itself looks, so every system has its own folders.
    fn another_font(unlike: f64) -> Option<(String, f64)> {
        let mut stack = qymcad_ui_state::font_directories();
        let mut seen = 0;
        while let Some(dir) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&dir) else { continue };
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
                if ext != "ttf" && ext != "otf" {
                    continue;
                }
                seen += 1;
                if seen > 400 {
                    return None;
                }
                if let Some(w) = std::fs::read(&p).ok().and_then(|b| shape_of(&b)) {
                    if (w - unlike).abs() / unlike > 0.05 {
                        return Some((p.to_string_lossy().into_owned(), w));
                    }
                }
            }
        }
        None
    }

    /// A family of this machine that a click on the list can take, other than `unlike`: the first row shown for
    /// its name is that family, and the face can write its own name - a row that cannot answers to no click.
    /// Taken from what is installed rather than named, because no one font lies on every system.
    fn a_family_to_click(unlike: &str) -> Option<String> {
        let installed = qymcad_ui_state::installed_fonts();
        installed.iter().filter(|f| f.family != unlike).find_map(|f| {
            let first = qymcad_ui_state::fonts_matching(&installed, &f.family).into_iter().next()?;
            let bytes = std::fs::read(&first.path).ok()?;
            let writes = qymcad_core::text::can_write(&bytes, first.index, &qymcad_ui_state::font_row_text(&first));
            (first.family == f.family && writes).then(|| f.family.clone())
        })
    }

    /// The width of the label on the sketch, over its height.
    fn drawn_shape(app: &App, si: usize) -> f64 {
        let (x0, y0, x1, y1) = app.project.sketch_text_bbox(si, 0).expect("the label is there");
        (x1 - x0) / (y1 - y0)
    }

    #[test]
    fn editing_a_label_keeps_the_font_it_was_written_in() {
        let ours = std::fs::read(OURS).expect("the font shipped with the repository");
        let want = shape_of(&ours).expect("setup: our font writes nothing");
        let Some((other, other_shape)) = another_font(want) else {
            panic!("setup: no second font was found in the system to tell one typeface from another");
        };

        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        Hand::new(&mut app).pick_font_file(OURS).sk_text("Ag", 20.0).click2d(0.0, 0.0);
        assert_eq!(app.project.sketches[si].texts.len(), 1, "setup: the label was not placed");
        let drawn = drawn_shape(&app, si);
        assert!((drawn - want).abs() < 0.05, "setup: the label was written in something else ({drawn:.3} against {want:.3})");

        // THE DOCUMENT IS REOPENED, in effect: nothing of the person's choice is loaded any more and the font
        // in hand is the one the system offers.
        Hand::new(&mut app).pick_font_file(&other);
        let (x0, y0, x1, y1) = app.project.sketch_text_bbox(si, 0).expect("the label is there");
        let middle = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        Hand::canvas(&mut app).sk_edit_text(middle, "Ag", 40.0);

        assert!((app.project.sketches[si].texts[0].height - 40.0).abs() < 1e-9, "setup: the height was not applied, the popup did not take");
        let now = drawn_shape(&app, si);
        assert!(
            (now - want).abs() < 0.05,
            "the label was re-baked in another typeface: it now writes at {now:.3} where its own font writes at {want:.3} (the font in hand writes at {other_shape:.3}, {other})"
        );
    }

    /// A LABEL WHOSE FONT IS GONE IS LEFT ALONE, and the person is told why.
    ///
    /// Two ways to get there: a document written before the font was recorded at all, and a document carried
    /// to a machine where that font is not installed. Both end the same way - the drawing is intact, because
    /// the glyphs are in the file, and the edit is refused instead of being answered with another typeface.
    #[test]
    fn a_label_whose_font_is_missing_is_not_re_baked_in_another_one() {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        Hand::new(&mut app).pick_font_file(OURS).sk_text("Ag", 20.0).click2d(0.0, 0.0);
        assert_eq!(app.project.sketches[si].texts.len(), 1, "setup: the label was not placed");

        // the state of a file carried to a machine without that font
        app.project.sketches[si].texts[0].font.path = "/nowhere/at/all.ttf".into();
        let before = app.project.sketches[si].texts[0].glyphs.clone();
        let height_before = app.project.sketches[si].texts[0].height;

        let (x0, y0, x1, y1) = app.project.sketch_text_bbox(si, 0).expect("the label is there");
        Hand::canvas(&mut app).sk_edit_text(((x0 + x1) / 2.0, (y0 + y1) / 2.0), "Ag", 40.0);

        let t = &app.project.sketches[si].texts[0];
        assert_eq!(t.height, height_before, "the height was applied while the font was not there to bake with");
        assert_eq!(t.glyphs.len(), before.len(), "the drawing was re-baked without its own font");
        assert!(!app.status.trim().is_empty(), "the edit was refused and nothing was said about it");
    }


    /// THE LIST OF FONTS IS A LIST: it opens, it narrows to what is typed, and a click on a row takes that
    /// face - no file paths, no knowing where fonts live.
    #[test]
    fn a_font_is_taken_from_the_list_by_a_click() {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);

        let Some(family) = a_family_to_click("") else {
            panic!("setup: this machine has no font a click on the list could take");
        };

        let took = Hand::new(&mut app).pick_font_from_list(&family);
        assert!(took, "a click on the first row of the list chose nothing; the list showed {} faces", qymcad_ui_state::fonts_matching(&qymcad_ui_state::installed_fonts(), &family).len());
        assert_eq!(app.tool_prefs.font.family, family, "the face taken is not the one the row showed");
        assert!(!app.font_cache.picker.open, "the window stayed open after a face was chosen");
    }


    /// THE INTERFACE SAYS WHICH FONT IT WILL WRITE WITH, in both places that ask the question.
    ///
    /// The button of the bar used to read "Font..." whatever was chosen - a person pressed it to find out
    /// what they were about to write with. The editor of a label said nothing at all about the label's own
    /// font, and that one is re-baked in it.
    #[test]
    fn the_font_is_named_where_a_person_asks_which_font() {
        use qymcad_core::model::FontRef;
        let named = FontRef { family: "Liberation Sans".into(), path: "/a.ttf".into(), index: 0 };
        assert_eq!(qymcad_ui_state::font_label(&named, "opt-font"), "Liberation Sans", "a chosen font is named by its family");
        let unknown = FontRef::default();
        let said = qymcad_ui_state::font_label(&unknown, "opt-font");
        assert_eq!(said, qymcad_i18n::tr("opt-font"), "with nothing chosen the button keeps its own word");
        assert_ne!(said, "opt-font", "the code of the word leaked into the interface instead of the word");

        // AND BOTH PLACES ASK THROUGH IT. Read out of the source: a copy of "family, or else a word" in one
        // of them drifts from the other, and both answer the same question.
        let bar = include_str!("../../../qymcad-part/src/lib.rs");
        let state = include_str!("../../../qymcad-ui-state/src/lib.rs");
        assert!(bar.contains("font_label(&bc.tool_prefs.font"), "the bar names the font of the tool some other way");
        assert!(state.contains("font_label(&of_label"), "the editor of a label names its font some other way");
    }


    /// A FONT WITHOUT THE LETTERS SAYS SO WHEN THE LABEL IS PLACED, and not "the text is empty".
    ///
    /// The list marks a face that cannot write its own name, and that is not the same question: a font with
    /// Latin and no Cyrillic looks perfectly good there, and a label typed in Russian comes out with nothing
    /// in it. The place to answer is the click that places the label, where the string is known.
    #[test]
    fn a_font_without_the_letters_says_so_when_the_label_is_placed() {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        // the font of this repository holds Latin and Cyrillic and no CJK at all
        Hand::new(&mut app).pick_font_file(OURS).sk_text("日本語", 20.0).click2d(0.0, 0.0);

        assert!(app.project.sketches[si].texts.is_empty(), "a label with nothing in it was placed all the same");
        let said = app.status.clone();
        assert_eq!(
            said,
            qymcad_i18n::tr1("sk-text-no-letters", "name", "Liberation Sans"),
            "the program blamed the string instead of naming the font that cannot write it: {said:?}"
        );
    }


    /// A NOTE ASKS ITS LETTERS OF THE FONT IT IS DRAWN IN: the interface's. A letter no font of the window has (a
    /// Tangut sign) is not taken into the note, and the status line says why. Reported behaviour: the letters of a
    /// note were asked of the tool's font, while the note is drawn in the interface's - a box on the sheet.
    #[test]
    fn a_note_takes_only_the_letters_the_interface_can_draw() {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        let mut hand = Hand::new(&mut app);
        hand.sk_tool(11).frame(Vec::new());
        assert!(hand.press_word(&qymcad_i18n::tr("opt-note"), egui::pos2(700.0, 0.0)), "the bar of the text tool has no note box");
        hand.sk_text("A\u{17000}", 20.0).frame(Vec::new());
        drop(hand);
        assert_eq!(app.tool_prefs.text, "A", "the note kept a letter no font of the window can draw");
        assert_eq!(app.status, qymcad_i18n::tr("sk-note-no-letters"), "nothing says why the letter went");
    }

    /// THE FONT OF A LABEL IS CHANGED WHERE THE LABEL IS EDITED, and the label changes at once.
    ///
    /// Reported behaviour, with a screenshot of the editor: "the font cannot be changed while editing". The
    /// popup named the font and nothing more - to write the same words in another face a person had to delete
    /// the label and place it again.
    #[test]
    fn the_font_of_a_label_is_changed_from_its_editor() {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        Hand::new(&mut app).pick_font_file(OURS).sk_text("Ag", 20.0).click2d(0.0, 0.0);
        assert_eq!(app.project.sketches[si].texts.len(), 1, "setup: the label was not placed");
        let was_font = app.project.sketches[si].texts[0].font.family.clone();
        // THE OUTLINES THEMSELVES, not their proportion: two different faces can write "Ag" at almost the
        // same width for its height - measured, 1.369 against 1.379 - and a check on the proportion alone
        // would call that "nothing changed".
        let was_glyphs = app.project.sketches[si].texts[0].glyphs.clone();
        let (x0, y0, x1, y1) = app.project.sketch_text_bbox(si, 0).expect("the label is there");
        let middle = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        let Some(wanted_font) = a_family_to_click(&was_font) else {
            panic!("setup: this machine has no second font a click on the list could take");
        };
        let took = Hand::canvas(&mut app).sk_change_label_font(middle, &wanted_font);

        assert!(took, "the list gave nothing back: the editor's font button leads nowhere");
        let now = &app.project.sketches[si].texts[0];
        assert_ne!(now.font.family, was_font, "the label kept the font it was written in: {:?}", now.font);
        assert_eq!(now.font.family, wanted_font, "some other face was taken: {:?}", now.font);
        assert_ne!(now.glyphs, was_glyphs, "the outlines are the same as before: the label was not re-baked in the new face");
    }


    /// EDITING A LABEL MEANS THE TEXT TOOL IS IN HAND, and the bar shows what is being edited.
    ///
    /// Reported behaviour, with a screenshot: "while editing, the tool stays Select". The bar is where the
    /// string, the height and the font are named, and a person editing a label was looking at the options of
    /// the arrow.
    #[test]
    fn editing_a_label_takes_up_the_text_tool() {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        Hand::new(&mut app).pick_font_file(OURS).sk_text("Ag", 20.0).click2d(0.0, 0.0);
        // put the tool down, as a person does before picking things with the arrow
        Hand::new(&mut app).sk_tool(0);
        assert_eq!(app.tools.armed.draw_kind(), 0, "setup: a tool is still in hand");
        app.tool_prefs.text = "something else".into();
        app.tool_prefs.text_h = 3.0;

        let (x0, y0, x1, y1) = app.project.sketch_text_bbox(si, 0).expect("the label is there");
        let opened = Hand::canvas(&mut app).sk_open_text_edit(((x0 + x1) / 2.0, (y0 + y1) / 2.0));

        assert!(opened, "the double click found no label to edit");
        assert_eq!(app.tools.armed.draw_kind(), 11, "editing a label left the arrow in hand instead of the text tool");
        assert_eq!(app.tool_prefs.text, "Ag", "the bar shows some other string than the label being edited");
        assert!((app.tool_prefs.text_h - 20.0).abs() < 1e-9, "the bar shows some other height: {}", app.tool_prefs.text_h);
        assert_eq!(app.tool_prefs.font.family, "Liberation Sans", "the bar shows some other font: {:?}", app.tool_prefs.font);
    }


    /// A DOUBLE CLICK ON A LABEL OPENS IT WITH THE TEXT TOOL STILL IN HAND, and puts no other label on it.
    ///
    /// The tool stays in hand after a label is placed, so the next thing a person does to fix a typo is
    /// double-click the label - and in the window the two clicks of that double click placed two more labels on
    /// top of it before the editor opened: measured, three labels where one was.
    #[test]
    fn a_double_click_on_a_label_with_the_text_tool_in_hand_places_nothing() {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        Hand::new(&mut app).pick_font_file(OURS).sk_text("Ag", 20.0).click2d(0.0, 0.0);
        assert_eq!(app.project.sketches[si].texts.len(), 1, "setup: the label was not placed");
        assert_eq!(app.tools.armed.draw_kind(), 11, "setup: the text tool is not in hand after placing");

        let (x0, y0, x1, y1) = app.project.sketch_text_bbox(si, 0).expect("the label is there");
        let opened = Hand::canvas(&mut app).sk_open_text_edit(((x0 + x1) / 2.0, (y0 + y1) / 2.0));

        let n = app.project.sketches[si].texts.len();
        assert_eq!(n, 1, "a double click on the label placed {} more labels on top of it", n - 1);
        assert!(opened, "the double click opened no editor");
    }

    /// AND THE TOOL IS PUT DOWN WHEN THE EDIT IS APPLIED.
    ///
    /// Reported behaviour: "after the tick or Enter the text tool stays in hand with the parameters of the
    /// edit, and clicking the sketch spams more of them". An edit is finished business: what was being
    /// edited is edited, and nothing is waiting to be placed.
    #[test]
    fn applying_an_edit_puts_the_text_tool_down() {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        Hand::new(&mut app).pick_font_file(OURS).sk_text("Ag", 20.0).click2d(0.0, 0.0);
        assert_eq!(app.project.sketches[si].texts.len(), 1, "setup: the label was not placed");

        let (x0, y0, x1, y1) = app.project.sketch_text_bbox(si, 0).expect("the label is there");
        let middle = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        Hand::canvas(&mut app).sk_edit_text(middle, "Ag", 40.0);

        assert_eq!(app.tools.armed.draw_kind(), 0, "the text tool stayed in hand after the edit was applied");

        // and a click on the canvas places nothing: there is nothing in hand
        Hand::canvas(&mut app).click2d(60.0, -40.0);
        assert_eq!(app.project.sketches[si].texts.len(), 1, "a click after the edit placed another label");
    }

}
