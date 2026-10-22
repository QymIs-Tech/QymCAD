//! A COPY GOES TO THE CLIPBOARD: Ctrl+C (or Edit -> Copy) on a selection asks for a base point, and the click on it
//! fills the clipboard and nothing more - no ghost follows the pointer, the sketch is unchanged. Ctrl+V (or Edit ->
//! Paste) brings the ghost held by the base point, a click places it; again and again, in this sketch or another. Ctrl+X
//! takes the selection out of the sketch into the clipboard. A rectangle pasted is a rectangle, with a centre of its own.
//! The Copy tool of the panel still places at once (its own probes: `the_move_tool_is_clicked`).
//!
//! Reported behaviour: "a copy meant for the clipboard - to be pasted later, into this sketch or another one - becomes a
//! move-copy on the spot"; a rectangle pasted was four plain lines.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::App;
    use crate::gui::import_door::tests::running;
    use qymcad_core::feature::SketchPlane;

    /// HOW A COMMAND OF THE CLIPBOARD IS GIVEN.
    #[derive(Clone, Copy, Debug)]
    enum By {
        Keys,
        Menu,
    }

    /// A sketch open for editing with a rectangle drawn from (0, 0) to (20, 10) and selected whole with Ctrl+A.
    fn a_rectangle_selected() -> (App, egui::Context) {
        let (mut app, ctx) = running();
        let si = app.create_sketch_on(SketchPlane::default());
        app.enter_sketch_edit(si);
        let mut hand = Hand::new(&mut app);
        hand.sk_tool(qymcad_ui_state::SketchShape::Rect).click2d(0.0, 0.0).click2d(20.0, 10.0).key(egui::Key::Escape).key(egui::Key::Escape);
        hand.sk_select();
        hand.ctrl(egui::Key::A);
        assert_eq!(hand.app.tools.sel_sk.items.iter().filter(|it| it.kind == qymcad_ui_state::ItemKind::Curve).count(), 4, "GUARD: the four sides selected");
        (app, ctx)
    }

    /// The words of the Edit menu are written after their icon: the item is the one whose words after the icon are its
    /// own - "Undo Insert" ends with the words of Paste too.
    fn menu(hand: &mut Hand, item: &str) {
        let (edit, item) = (crate::i18n::tr("menu-edit"), crate::i18n::tr(item));
        assert!(hand.press_word(&edit, egui::pos2(0.0, 0.0)), "no Edit menu");
        hand.frame(Vec::new());
        let words = |t: &str| t.chars().skip_while(|c| ('\u{e000}'..='\u{f8ff}').contains(c)).collect::<String>().trim().to_string();
        let at = hand.words_drawn().iter().find(|(t, _)| words(t) == item).map(|(_, r)| r.center());
        hand.click_screen(at.unwrap_or_else(|| panic!("no {item:?} in the Edit menu")));
    }

    fn copy(hand: &mut Hand, by: By) {
        match by {
            By::Keys => {
                hand.copy();
            }
            By::Menu => menu(hand, "menu-copy"),
        }
    }

    fn cut(hand: &mut Hand, by: By) {
        match by {
            By::Keys => {
                hand.cut();
            }
            By::Menu => menu(hand, "menu-cut"),
        }
    }

    fn paste(hand: &mut Hand, by: By) {
        match by {
            By::Keys => {
                hand.paste();
            }
            By::Menu => menu(hand, "win-insert"),
        }
    }

    fn lines(hand: &Hand, si: usize) -> usize {
        hand.app.project.sketches[si].entities.len()
    }

    #[test]
    fn a_copy_fills_the_clipboard_and_a_paste_places_it_again_and_again() {
        let mut failures = Vec::new();
        for by in [By::Keys, By::Menu] {
            let (mut app, _ctx) = a_rectangle_selected();
            let mut hand = Hand::new(&mut app);
            copy(&mut hand, by);
            hand.click2d(0.0, 0.0); // the base point: the lower left corner
            hand.mouse2d(40.0, 40.0);
            if hand.app.side.clip.geom.as_ref().is_none_or(|c| c.entities.len() != 4) || hand.app.side.clip.geom_place || lines(&hand, 0) != 4 {
                failures.push(format!(
                    "{by:?}: the base point clicked - the clipboard holds {:?} curves, a ghost follows the pointer {}, the sketch has {} curves",
                    hand.app.side.clip.geom.as_ref().map(|c| c.entities.len()),
                    hand.app.side.clip.geom_place,
                    lines(&hand, 0)
                ));
                continue;
            }
            for (k, at) in [(40.0, 0.0), (80.0, 0.0)].into_iter().enumerate() {
                paste(&mut hand, by);
                if !hand.app.side.clip.geom_place {
                    failures.push(format!("{by:?}: paste {} brought no ghost", k + 1));
                    break;
                }
                hand.click2d(at.0, at.1);
                let held = hand.app.project.sketches[0].points.iter().any(|q| (q.x - at.0).abs() < 1e-6 && (q.y - at.1).abs() < 1e-6);
                let rects = hand.app.project.sketches[0].rects().len();
                if lines(&hand, 0) != 4 * (k + 2) || !held || rects != k + 2 {
                    failures.push(format!("{by:?}: paste {} at {at:?} - the sketch has {} curves and {rects} rectangles, a corner on the click {held}", k + 1, lines(&hand, 0)));
                }
            }
        }
        assert!(failures.is_empty(), "a copy to the clipboard:\n{}", failures.join("\n"));
    }

    #[test]
    fn a_copy_is_pasted_into_another_sketch() {
        let (mut app, _ctx) = a_rectangle_selected();
        let mut hand = Hand::new(&mut app);
        hand.copy();
        hand.click2d(0.0, 0.0);
        assert!(hand.press_word(&crate::i18n::tr("wb-finish"), egui::pos2(0.0, 0.0)), "the sketch could not be left");
        let other = hand.app.create_sketch_on(SketchPlane::default());
        hand.app.enter_sketch_edit(other);
        hand.sk_select();
        hand.paste();
        hand.click2d(30.0, 30.0);
        assert_eq!(lines(&hand, other), 4, "the copy pasted into another sketch: {} curves there", lines(&hand, other));
        let s = &hand.app.project.sketches[other];
        let centre_held = s.rects().first().is_some_and(|r| s.points.iter().any(|q| q.id == r.centre && (q.x - 40.0).abs() < 1e-6 && (q.y - 35.0).abs() < 1e-6));
        assert!(centre_held, "the copy pasted into another sketch is no rectangle with its centre at (40, 35): {:?}", s.rects());
        assert_eq!(lines(&hand, 0), 4, "the first sketch changed");
    }

    #[test]
    fn a_cut_takes_the_selection_out_and_a_paste_brings_it_back() {
        let mut failures = Vec::new();
        for by in [By::Keys, By::Menu] {
            let (mut app, _ctx) = a_rectangle_selected();
            let mut hand = Hand::new(&mut app);
            cut(&mut hand, by);
            hand.click2d(0.0, 0.0);
            if lines(&hand, 0) != 0 || hand.app.side.clip.geom_place {
                failures.push(format!("{by:?}: cut - the sketch keeps {} curves, a ghost follows the pointer {}", lines(&hand, 0), hand.app.side.clip.geom_place));
                continue;
            }
            paste(&mut hand, by);
            hand.click2d(10.0, 10.0);
            if lines(&hand, 0) != 4 || hand.app.project.sketches[0].rects().len() != 1 {
                failures.push(format!("{by:?}: the cut pasted back - {} curves, {} rectangles", lines(&hand, 0), hand.app.project.sketches[0].rects().len()));
            }
        }
        assert!(failures.is_empty(), "a cut to the clipboard:\n{}", failures.join("\n"));
    }
}
