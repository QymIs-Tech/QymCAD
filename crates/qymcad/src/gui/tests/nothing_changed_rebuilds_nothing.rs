//! WHAT CHANGED NOTHING THE BODIES ARE BUILT FROM REBUILDS NOTHING, AND A SKETCH IS REBUILT WHEN IT CLOSES.
//!
//! Reported behaviour, on the filter of the examples: a sketch opened and closed rebuilt the cuts on it and a rounding of
//! 36 edges; "Add a parameter" - an empty row, no value changed - rebuilt every sketch with a formula and what stands on
//! them, under the dimmed screen; and geometry was rebuilt while the sketch was being drawn in rather than once it was
//! closed.
//!
//! Each step is made by hand in real frames, and every frame after it is read for a rebuild in flight.
#[cfg(test)]
mod tests {
    use crate::gui::hand::Hand;
    use crate::gui::App;

    /// The filter of the examples, opened as the window opens it, the tube stepped into.
    fn the_tube(app: &mut App) -> Hand<'_> {
        crate::gui::io_jobs::spawn_project_load(&mut app.regen, format!("{}/../../examples/Filter-v2.qcad", env!("CARGO_MANIFEST_DIR")));
        app.drain_busy_for_test();
        app.wait_bg();
        let mut h = Hand::new(app);
        assert!(h.double_click_word("Tube"), "the tree shows no part Tube");
        h
    }

    /// Open the seventh sketch of the tube - the one its cuts stand on - by a double click on its row of the tree.
    fn open_the_sketch_of_the_cuts(h: &mut Hand) {
        let row = h.app.project.sketches.iter().map(|s| crate::i18n::name(&s.name)).find(|n| n.ends_with(" 7")).expect("the seventh sketch of the filter");
        assert!(h.double_click_word(&row), "the tree shows no sketch {row:?}");
        assert!(h.app.sketch_ses.editing.is_some(), "setup: a double click on the sketch {row:?} does not open it");
    }

    /// The labels of the rebuilds that were in flight over `frames` frames, the frames apart by 10 ms.
    fn rebuilds_over(h: &mut Hand, frames: usize) -> Vec<String> {
        let mut seen = Vec::new();
        for _ in 0..frames {
            h.frame(Vec::new());
            if let Some(b) = h.app.regen.busy.as_ref().filter(|b| b.kind == crate::gui::BgKind::Regen) {
                seen.push(b.label.clone());
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        seen.dedup();
        seen
    }

    /// Press "Finish" of the sketch in hand.
    fn finish(h: &mut Hand) {
        let word = crate::i18n::tr("wb-finish");
        let at = h.written_at(&word).unwrap_or_else(|| panic!("no {word:?} on screen")).center();
        h.click_screen(at);
        assert!(h.app.sketch_ses.editing.is_none(), "setup: {word:?} did not close the sketch");
    }

    #[test]
    fn a_sketch_opened_and_closed_untouched_rebuilds_nothing() {
        let mut app = App::default();
        let mut h = the_tube(&mut app);
        open_the_sketch_of_the_cuts(&mut h);
        let open = rebuilds_over(&mut h, 20);
        finish(&mut h);
        let closed = rebuilds_over(&mut h, 30);
        assert!(open.is_empty() && closed.is_empty(), "a sketch opened and closed with nothing touched rebuilt: open {open:?}, closed {closed:?}");
    }

    /// A point put in the sketch: nothing is rebuilt while it is open, the bodies on it are rebuilt once it closes.
    #[test]
    fn a_sketch_changed_is_rebuilt_when_it_closes_and_not_before() {
        let mut app = App::default();
        let mut h = the_tube(&mut app);
        open_the_sketch_of_the_cuts(&mut h);
        let sid = h.app.sketch_ses.editing.expect("setup: the sketch is open");
        let points = |h: &Hand| h.app.project.sketches.iter().find(|s| s.id == sid).map(|s| s.points.len()).unwrap_or(0);
        let before = points(&h);
        h.sk_tool(5);
        h.mouse2d(3.0, 4.0);
        let open = rebuilds_over(&mut h, 30);
        assert!(points(&h) > before, "setup: the point was not put in the sketch");
        assert!(open.is_empty(), "a point put in the open sketch rebuilt the bodies while it was still open: {open:?}");
        h.key(egui::Key::Escape);
        finish(&mut h);
        let closed = rebuilds_over(&mut h, 5);
        assert!(!closed.is_empty() || h.app.regen.wanted || h.app.regen.busy.is_some(), "the sketch changed and closed, and the bodies on it were not rebuilt");
        h.close_window();
    }

    #[test]
    fn adding_an_empty_parameter_rebuilds_nothing() {
        let mut app = App::default();
        let mut h = the_tube(&mut app);
        let params = h.words_drawn().iter().map(|w| w.0.clone()).find(|w| *w == crate::i18n::tr("wb-params")).expect("the parameters button");
        assert!(h.press_word(&params, egui::pos2(0.0, 0.0)), "the parameters button");
        for _ in 0..3 {
            h.frame(Vec::new()); // the window of the parameters lays itself out
        }
        let add = h.words_drawn().iter().map(|w| w.0.clone()).find(|w| w.contains(&crate::i18n::tr("win-add-param"))).expect("the button that adds a parameter");
        let rows = h.app.project.parameters.len();
        assert!(h.press_word(&add, egui::pos2(0.0, 0.0)), "the button that adds a parameter");
        assert_eq!(h.app.project.parameters.len(), rows + 1, "setup: the parameter was not added");
        let seen = rebuilds_over(&mut h, 30);
        assert!(seen.is_empty(), "an empty parameter added - no value changed - rebuilt: {seen:?}");
    }
}
