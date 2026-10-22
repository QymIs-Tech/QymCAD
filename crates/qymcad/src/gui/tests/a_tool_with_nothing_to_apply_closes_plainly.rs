//! A TOOL WITH NOTHING TO APPLY ENDS WITH "CLOSE (Esc)", A PLAIN BUTTON.
//!
//! Reported behaviour: the measure, the section once on and grounding ended with a button of their own words - "Cancel",
//! "Off", "Done (Esc)" - in red, the tone of a refusal, though putting such a tool down refuses nothing.
#[cfg(test)]
mod tests {
    use crate::gui::hand::Hand;
    use crate::gui::App;

    /// THE MEASURE ENDS WITH "CLOSE (Esc)" IN THE COLOUR OF A PLAIN BUTTON, and the button puts it down.
    #[test]
    fn the_measure_ends_with_a_plain_close() {
        let mut app = App::default();
        crate::gui::joint_flow::tests::add_part_at(&mut app, 0.0);
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        let body = app.project.mesh_id(0).expect("the body");
        if let Some(owner) = app.project.body_owner(body) {
            app.enter_component(owner);
        }
        app.viewing.mode_3d = true;
        let mut hand = Hand::new(&mut app);
        assert!(hand.press_hint(&crate::i18n::tr("tb-measure3d-hint")), "the button of the measure");
        assert!(hand.app.side.m3.on, "the measure is not taken");
        let close = crate::i18n::tr("cmd-close-esc");
        let painted = hand.painted_words();
        let word = painted.iter().find(|w| w.text.contains(&close)).unwrap_or_else(|| panic!("the bar of the measure does not end with {close:?}"));
        let refuse = hand.app.scheme.pal.refuse();
        assert_ne!(word.colour, refuse, "{close:?} is painted in the tone of a refusal");
        let at = word.rect.center();
        assert!(hand.press_word(&close, at), "{close:?} is not pressed");
        hand.frame(Vec::new());
        assert!(!hand.app.side.m3.on, "{close:?} did not put the measure down");
    }
}
