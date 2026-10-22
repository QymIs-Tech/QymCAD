//! ESCAPE WITH THE DRIVER LIST OPEN CLOSES THE LIST, NOT THE COMMAND.
//!
//! Reported behaviour: typing in a parameter field brings up the list; pressing Escape does not make the
//! list go away but cancels the operation instead.
//!
//! The field's own contract already says the right thing and is checked in `expr_field_behaviour.rs`: with
//! the list open Escape closes the list. That check draws THE FIELD ALONE, and there it holds. In the whole
//! frame it does not: `handle_key_commands` runs before anything is drawn, sees `wants_keyboard_input` and
//! surrenders the focus — so the field never gets the key, and the next Escape reaches `on_escape` and takes
//! the command down.
//!
//! That is why this check drives THE WHOLE WINDOW, frame by frame: keys first, drawing second. A field drawn on its
//! own cannot see this at all.
//!
//! MEASURED: on the feature command's popup the ladder ALREADY holds — both checks below were green before
//! anything was changed, so the reported field is a different one. They stay as a guard on the frame order
//! (the field alone cannot prove it) and to narrow the search: this path is not the one to look at.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::App;

    /// The scene: a part with a sketch, an open extrude, and a name the list can offer.
    fn extruding_with_a_driver() -> App {
        let mut app = super::super::screen_keys::tests::plate();
        let part = app.project.components.iter().rev().find(|c| c.parent.is_some()).map(|c| c.id).expect("the part");
        app.enter_component(part);
        app.chosen.sel = super::super::Sel::Sketch(0);
        app.viewing.mode_3d = true;
        app.project.parameters.push(qymcad_core::model::Param { name: "width".into(), expr: "50".into(), value: 50.0 });
        app.start_feat_cmd(qymcad_ui_state::PartTool::Extrude);
        assert!(app.tools.armed.commanding(), "setup: the extrude did not open — there is nothing to check");
        app
    }

    /// Whether `what` is written anywhere on the window the last frame drew.
    fn shows(hand: &Hand, what: &str) -> bool {
        hand.words_drawn().iter().any(|(t, _)| t.contains(what))
    }

    /// ESCAPE TAKES THE LIST DOWN AND LEAVES THE COMMAND ALONE.
    #[test]
    fn escape_closes_the_list_and_keeps_the_command() {
        let mut app = extruding_with_a_driver();
        let mut hand = Hand::as_it_stands(&mut app);
        hand.frame(Vec::new());
        hand.type_text("wid").frame(Vec::new()).frame(Vec::new());
        assert!(shows(&hand, "width"), "setup: the list must be open before Escape, drawn: {:?}", hand.words_drawn());

        hand.key(egui::Key::Escape);
        assert!(hand.app.tools.armed.commanding(), "Escape with the list open cancelled the whole operation instead of closing the list");

        hand.frame(Vec::new());
        assert!(!shows(&hand, "width"), "the list stayed open after Escape: {:?}", hand.words_drawn());
    }

    /// THE SECOND ESCAPE STILL CANCELS THE COMMAND — the ladder is not lost.
    #[test]
    fn a_second_escape_still_cancels_the_command() {
        let mut app = extruding_with_a_driver();
        let mut hand = Hand::as_it_stands(&mut app);
        hand.frame(Vec::new());
        hand.type_text("wid").frame(Vec::new()).frame(Vec::new());

        hand.key(egui::Key::Escape); // the list
        hand.key(egui::Key::Escape); // out of the field
        hand.key(egui::Key::Escape); // the command
        assert!(!hand.app.tools.armed.commanding(), "with the list closed and the field left, Escape no longer cancels the command");
    }
}
