//! THE DOORS OF A TOOL - what Enter, Esc and Cancel do - gone through by the frame alike in every tool, a refusal to
//! apply said by the frame where the tool speaks.
//!
//! Each tool matched the answer of its frame by itself before, and said a refusal to apply on the status line in the
//! tone it chose.
#[cfg(test)]
mod tests {
    use crate::gui::hand::Hand;
    use crate::gui::App;
    use qymcad_ui_state::{Notice, Tone};

    /// A part with its block, entered, the view in space.
    fn a_part(app: &mut App) {
        crate::gui::joint_flow::tests::add_part_at(app, 0.0);
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        let body = app.project.mesh_id(0).expect("the body");
        if let Some(owner) = app.project.body_owner(body) {
            app.enter_component(owner);
        }
        app.viewing.mode_3d = true;
    }

    /// AN AXIS BY HAND WITH NO DIRECTION IS NOT MADE: Enter is refused in words where the tool speaks, in the tone of a
    /// refusal, and the status line does not say it a second time; the tool stays in hand, the document as it was.
    #[test]
    fn a_refusal_to_apply_is_said_where_the_tool_speaks() {
        let mut app = App::default();
        a_part(&mut app);
        let mut hand = Hand::new(&mut app);
        assert!(hand.press_hint(&crate::i18n::tr("g-datum-axis-hint")), "the button of a datum axis");
        assert!(hand.press_word(&crate::i18n::tr("cmd-manual"), egui::pos2(640.0, 24.0)), "the mode by hand of a datum axis");
        hand.set("dx", 0.0).set("dy", 0.0).set("dz", 0.0).frame(Vec::new());
        let axes = hand.app.project.datum_axes.len();
        hand.key(egui::Key::Enter);
        let why = crate::i18n::tr("msg-axis-not-set");
        let says = hand.tool_says();
        assert!(says.contains(&Notice::refusal(why.clone())), "the refusal is not said where the tool speaks: {says:?}");
        assert_ne!(hand.app.status.words, why, "the status line says the refusal a second time");
        assert_eq!(hand.app.project.datum_axes.len(), axes, "an axis with no direction was made");
        assert!(hand.app.tools.armed.commanding(), "the tool was put down by a refusal");
    }

    /// THE MEASURE STEPS BACK ON Esc: the first lets go of what was clicked and the tool stays, the second puts it down.
    #[test]
    fn esc_steps_back_before_it_puts_the_tool_down() {
        let mut app = App::default();
        a_part(&mut app);
        let mut hand = Hand::new(&mut app);
        assert!(hand.press_hint(&crate::i18n::tr("tb-measure3d-hint")), "the button of the measure");
        hand.click([20.0, 15.0, 10.0]).frame(Vec::new());
        assert!(!hand.app.side.m3.picks.is_empty(), "setup: the click on the top of the block was not taken by the measure");
        hand.key(egui::Key::Escape);
        assert!(hand.app.side.m3.on && hand.app.side.m3.picks.is_empty(), "the first Esc did not let go of the click alone");
        hand.key(egui::Key::Escape);
        assert!(!hand.app.side.m3.on, "the second Esc did not put the measure down");
    }

    /// THE RELATION: Cancel puts it down, Esc puts it down, Enter with both mates taken makes it.
    #[test]
    fn a_relation_is_put_down_by_cancel_and_esc_and_made_by_enter() {
        let mut app = App::default();
        let ([ja, jb], _) = crate::gui::a_relation_is_made_by_hand::tests::two_hinges(&mut app);
        let row = |app: &App, j| crate::i18n::name(&app.project.joints.iter().find(|x| x.id == j).expect("the joint").name);
        let (row_a, row_b) = (row(&app, ja), row(&app, jb));
        let mut hand = Hand::new(&mut app);
        let right = egui::pos2(1300.0, 300.0);
        let take = |hand: &mut Hand| assert!(hand.press_hint(&crate::i18n::tr("j-relation-tip")), "the button of the relation");
        take(&mut hand);
        assert!(hand.press_word(&crate::i18n::tr("cmd-cancel-esc"), egui::pos2(1200.0, 24.0)), "the relation ends with no Cancel");
        assert!(hand.app.side.joint.relation_pick.is_none(), "Cancel did not put the relation down");
        take(&mut hand);
        hand.key(egui::Key::Escape);
        assert!(hand.app.side.joint.relation_pick.is_none(), "Esc did not put the relation down");
        assert!(hand.press_word(&row_a, right), "the row of the first joint is not in the list");
        assert!(hand.press_word_holding(&row_b, right, egui::Modifiers::COMMAND), "the row of the second joint is not in the list");
        take(&mut hand);
        hand.key(egui::Key::Enter);
        assert_eq!(hand.app.project.relations.len(), 1, "Enter made no relation of the two joints taken");
        let said: Vec<Notice> = hand.tool_says().into_iter().filter(|n| n.tone == Tone::Refusal).collect();
        assert!(said.is_empty(), "a relation made is refused: {said:?}");
    }

    /// THE EDIT OF A MATE STEPS BACK ON Esc: the axis being picked is let go and the edit stays; the next Esc leaves it.
    #[test]
    fn esc_in_the_edit_of_a_mate_lets_go_of_the_axis_first() {
        let mut app = App::default();
        crate::gui::tests::the_box_of_a_joint_is_laid_in_sections::tests::a_hinge_edited(&mut app);
        let mut hand = Hand::new(&mut app);
        hand.frame(Vec::new()).frame(Vec::new());
        let area = hand.tool_box_id().and_then(|id| hand.area_rect(id)).expect("the box of the mate is drawn");
        assert!(hand.press_word(&crate::i18n::tr("j-axis-pick"), area.center()), "the box of the mate has no button to pick an axis");
        assert!(hand.app.side.joint.axis_pick.is_some(), "setup: picking the axis did not begin");
        hand.key(egui::Key::Escape);
        assert!(hand.app.side.joint.axis_pick.is_none() && hand.app.side.joint.edit.is_some(), "the first Esc did not let go of the axis alone");
        hand.key(egui::Key::Escape);
        assert!(hand.app.side.joint.edit.is_none(), "the second Esc did not leave the edit");
    }
}
