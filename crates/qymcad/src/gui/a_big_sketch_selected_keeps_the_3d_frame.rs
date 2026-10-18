//! A BIG SKETCH SELECTED KEEPS THE 3D FRAME: a grid of two patterns of 150 lines, 22 201 cells, drawn by hand, the sketch
//! left with Ctrl+Enter - the view back in 3D with the sketch selected - and a frame of that view in the time of a frame.
//!
//! Reported behaviour: with a sketch of 26 335 loops selected every frame of the 3D view took 4.3 s. The loops drawn
//! were each looked for along the list of the loops of the sketch selected.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::Sel;
    use crate::gui::import_door::tests::running;
    use qymcad_core::feature::SketchPlane;
    use std::time::Duration;

    #[test]
    fn a_frame_of_the_3d_view_with_a_big_sketch_selected_is_quick() {
        let n: u32 = 150;
        let len = 20.0 * n as f64;
        let (mut app, _ctx) = running();
        let si = app.create_sketch_on(SketchPlane::default());
        let mut hand = Hand::new(&mut app);
        hand.app.enter_sketch_edit(si);
        // a rebuild under way refuses input: the hand waits for it to end, as a person waits for the spinner to go
        hand.frame(Vec::new());
        crate::gui::session::waiting::until("the rebuild before the sketch is drawn in", || {
            if hand.app.regen.busy.is_none() {
                return true;
            }
            hand.frame(Vec::new());
            false
        });
        hand.sk_tool(1).click2d(0.0, -10.0).click2d(len, -10.0).key(egui::Key::Escape).key(egui::Key::Escape);
        hand.sk_tool(1).click2d(-10.0, 0.0).click2d(-10.0, len).key(egui::Key::Escape).key(egui::Key::Escape);
        for (middle, step) in [((len / 2.0, -10.0), ["0", "20"]), ((-10.0, len / 2.0), ["20", "0"])] {
            hand.sk_tool(0);
            hand.click2d(middle.0, middle.1);
            assert!(hand.press_hint(&crate::i18n::tr("tb-lin-array-hint")), "the linear pattern");
            assert!(hand.type_after_word(&crate::i18n::tr("opt-count"), &n.to_string()), "the count of the pattern");
            assert!(hand.type_after_word(&crate::i18n::tr("opt-step-x"), step[0]), "the step X");
            assert!(hand.type_after_word(&crate::i18n::tr("opt-step-y"), step[1]), "the step Y");
            hand.key(egui::Key::Enter).key(egui::Key::Escape);
        }
        let loops = hand.app.project.sketches[si].contour_ids.len();
        assert!(loops >= ((n - 1) * (n - 1)) as usize, "the grid has {loops} loops, fewer than its cells");
        hand.key(egui::Key::Escape).ctrl(egui::Key::Enter);
        assert!(hand.app.sketch_ses.editing.is_none() && hand.app.viewing.mode_3d, "Ctrl+Enter did not leave the sketch for the 3D view");
        assert!(matches!(hand.app.chosen.sel, Sel::Sketch(s) if s == si), "the sketch left is not selected");
        hand.frame(Vec::new());
        // the work of the frame, not the share of the machine it was given (`time_budget::Cost`)
        let frame = qymcad_core::time_budget::cost_of(qymcad_core::time_budget::Doer::Thread, || hand.frame(Vec::new()).worst_frame()).cost;
        eprintln!("a frame of the 3D view with a sketch of {loops} loops selected: {frame}");
        // measured: 4.3 s a frame on 26 335 loops in a test build, the loops looked for along a list
        let budget = if cfg!(debug_assertions) { Duration::from_millis(800) } else { Duration::from_millis(150) };
        assert!(frame.work < budget, "{}", qymcad_core::time_budget::over(&format!("a frame of the 3D view with a sketch of {loops} loops selected"), frame, budget));
    }
}
