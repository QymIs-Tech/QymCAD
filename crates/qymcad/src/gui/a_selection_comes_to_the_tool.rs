//! WHAT IS SELECTED BEFORE A TOOL IS TAKEN COMES TO THE TOOL, for the three tools that take two things: a boolean of
//! two bodies, a relation of two joints, and the mirrored copy of a part with the face it is mirrored about.
//!
//! Reported behaviour: the boolean took one body selected and waited for the second, as Shift with nothing in hand
//! kept only the body clicked last; the list of joints held one joint at a time; a face under the gizmo of the part
//! picked opened the numeric move of the part instead of being picked. Each is walked through the window: the
//! clicks and the keys in whole frames, the button of the tool found by its hint.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::{App, Sel};
    use qymcad_core::model::Id;

    /// A part of one cube 20 x 20 x 10 cut in two at the height of 5, the part entered: the lower piece and the upper.
    fn two_pieces(app: &mut App) -> [Id; 2] {
        super::super::joint_flow::tests::add_part_at(app, 0.0);
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        let body = app.project.mesh_id(0).expect("the cube");
        app.chosen.sel = Sel::Mesh(app.project.mesh_index(body).expect("the mesh of the cube"));
        app.start_feat_cmd(qymcad_ui_state::PartTool::SplitBody);
        app.params.split.plane = Some(qymcad_core::feature::SketchPlane::default());
        if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "offset") {
            (p.val, p.txt) = (5.0, "5".into());
        }
        app.apply_feat_cmd();
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        let pieces = app.project.timeline.iter().find(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::SplitBody { .. })).map(|n| n.kind.bodies()).unwrap_or_default();
        assert_eq!(pieces.len(), 2, "setup: the cube is cut in two; the status line says {}", app.status);
        let low = |b: &Id| app.project.mesh_index(*b).and_then(|mi| app.project.bodies[mi].mesh.bounds()).map_or(f64::MAX, |bb| bb.min.z);
        let (lower, upper) = if low(&pieces[0]) < low(&pieces[1]) { (pieces[0], pieces[1]) } else { (pieces[1], pieces[0]) };
        app.chosen.sel = Sel::None;
        [lower, upper]
    }

    /// TWO BODIES PICKED WITH NOTHING IN HAND - the second with Shift - ARE THE BOOLEAN'S A AND B, and Enter makes it.
    #[test]
    fn a_boolean_takes_both_bodies_picked_before_it() {
        let mut app = App::default();
        let [lower, upper] = two_pieces(&mut app);
        let mut hand = Hand::new(&mut app);
        hand.look_at([10.0, 10.0, 5.0], 8.0);
        hand.press3d([10.0, 0.0, 2.5], egui::Modifiers::NONE).press3d([10.0, 10.0, 10.0], egui::Modifiers::SHIFT);
        assert_eq!(hand.app.chosen.tree_sel.multi, vec![lower, upper], "Shift with nothing in hand does not hold the second body with the first");
        assert!(hand.press_hint(&crate::i18n::tr("tb-bool-bodies-hint")), "the button of the boolean is not found");
        assert_eq!(hand.app.params.boolean.pick.map(|(a, _)| a), Some(lower), "the boolean did not take body A picked before it; the status line says {}", hand.app.status);
        assert_eq!(hand.app.params.boolean.b, Some(upper), "the boolean did not take body B held with A");
        hand.key(egui::Key::Enter);
        let made = hand.app.project.timeline.iter().any(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::BodyBoolean { .. }));
        assert!(made, "Enter made no boolean of the two bodies; the status line says {}", hand.app.status);
    }

    /// TWO JOINTS PICKED IN THE LIST WITH NOTHING IN HAND - the second with Ctrl - ARE THE RELATION'S TWO MATES.
    #[test]
    fn a_relation_takes_both_joints_picked_in_the_list_before_it() {
        let mut app = App::default();
        let ([ja, jb], _) = super::super::a_relation_is_made_by_hand::tests::two_hinges(&mut app);
        // a row is written as its icon and the name of its joint, the icon a glyph of a font of its own
        let row = |app: &App, j: Id| crate::i18n::name(&app.project.joints.iter().find(|x| x.id == j).expect("the joint").name);
        let (row_a, row_b) = (row(&app, ja), row(&app, jb));
        let mut hand = Hand::new(&mut app);
        let right = egui::pos2(1300.0, 300.0);
        assert!(hand.press_word(&row_a, right), "the row of the first joint is not in the list: {row_a}");
        assert!(hand.press_word_holding(&row_b, right, egui::Modifiers::COMMAND), "the row of the second joint is not in the list: {row_b}");
        assert_eq!(hand.app.chosen.tree_sel.multi, vec![ja, jb], "Ctrl in the list does not hold the second joint with the first");
        assert!(hand.press_hint(&crate::i18n::tr("j-relation-tip")), "the button of the relation is not found");
        let picks: Vec<Id> = hand.app.side.joint.relation_pick.as_ref().map(|p| p.picks.iter().map(|(j, _)| *j).collect()).unwrap_or_default();
        assert_eq!(picks, vec![ja, jb], "the relation did not take the joints picked before it; the status line says {}", hand.app.status);
        hand.key(egui::Key::Enter);
        assert_eq!(hand.app.project.relations.len(), 1, "Enter made no relation of the two joints; the status line says {}", hand.app.status);
    }

    /// THE PART PICKED AND A FACE UNDER ITS GIZMO PICKED WITH SHIFT ARE THE MIRRORED COPY'S PART AND PLANE.
    #[test]
    fn a_mirrored_copy_takes_the_face_picked_under_the_gizmo_of_the_part() {
        let mut app = App::default();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        app.exit_context();
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        let parts = app.project.components.len();
        let mut hand = Hand::new(&mut app);
        hand.look_at([10.0, 10.0, 5.0], 8.0);
        hand.press3d([10.0, 10.0, 10.0], egui::Modifiers::NONE);
        assert!(matches!(hand.app.chosen.sel, Sel::Component(_)), "setup: a click on the part picks it");
        // a place of the front face the gizmo of the part stands over
        hand.frame(Vec::new());
        let (rect, basis) = (hand.app.viewing.view_rect, hand.app.viewing.cam.basis());
        let under = (1..20)
            .flat_map(|x| (1..10).map(move |z| [f64::from(x), 0.0, f64::from(z)]))
            .find(|w| {
                let at = qymcad_ui_state::Screen { cam: &hand.app.viewing.cam, set: &hand.app.set, rect, basis: &basis }.at(*w).0;
                crate::gui::gizmo_math::gizmo_click_hit(&hand.app.painting(), rect, at, &basis).is_some()
            })
            .expect("setup: the gizmo of the part stands over its front face");
        hand.press3d(under, egui::Modifiers::SHIFT);
        assert!(hand.app.dragged.body_giz.num.is_none(), "Shift and a click on the face under the gizmo opened the numeric move of the part");
        assert!(hand.press_hint(&crate::i18n::tr("tb-mirror-part-hint")), "the button of the mirrored copy is not found");
        assert!(hand.app.params.mirror.at.is_some(), "the mirrored copy did not take the face picked before it; the status line says {}", hand.app.status);
        hand.key(egui::Key::Enter);
        assert!(hand.app.project.components.len() > parts, "Enter made no mirrored copy; the status line says {}", hand.app.status);
    }
}
