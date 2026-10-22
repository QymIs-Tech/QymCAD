//! THE BOX OF A TOOL STANDS BY WHAT IT SIZES, clear of it and of every handle and gizmo, and stands where a person put it.
//!
//! Reported behaviour: the box of a mate stood over the mate it sizes when the view moved, and the gizmo of a datum stood
//! under its box.
#[cfg(test)]
mod tests {
    use crate::gui::hand::{Hand, ViewDrag};
    use crate::gui::App;

    /// Where the middle of the anchors of joint `jid` stands on the screen in the view the window laid out.
    fn the_mate_on_screen(hand: &mut Hand, jid: qymcad_core::model::Id) -> egui::Pos2 {
        hand.frame(Vec::new());
        let app = &*hand.app;
        let j = app.project.joints.iter().find(|j| j.id == jid).expect("the hinge").clone();
        let basis = app.viewing.cam.basis();
        let dc = qymcad_ui_state::DrawCtx { cam: &app.viewing.cam, set: &app.set, scheme: &app.scheme, project: &app.project, active_path: &app.active_path };
        let (a, b) = qymcad_ui_state::joint_endpoints(&dc, &j, app.viewing.view_rect, &basis).expect("the anchors of the hinge are on screen");
        egui::pos2((a.x + b.x) / 2.0, (a.y + b.y) / 2.0)
    }

    /// THE BOX OF A MATE DOES NOT LIE OVER IT WHEN THE MATE COMES TO THE EDGE OF THE VIEW: wheeled in by the left of the
    /// view, the hinge goes to its right edge, and the box goes to its left, wholly in the view. Measured before: the box
    /// pressed against the edge stood over the hinge, 304 px moved against its 165, and ran 22 px out of the view.
    #[test]
    fn the_box_of_a_mate_does_not_lie_over_it_at_the_edge_of_the_view() {
        let mut app = App::default();
        let jid = crate::gui::tests::the_box_of_a_joint_is_laid_in_sections::tests::a_hinge_edited(&mut app);
        let mut hand = Hand::new(&mut app);
        hand.frame(Vec::new()).frame(Vec::new());
        let view = hand.app.viewing.view_rect;
        hand.wheel3d(view.left_center() + egui::vec2(80.0, 0.0), 3);
        hand.frame(Vec::new()).frame(Vec::new());
        let mate = the_mate_on_screen(&mut hand, jid);
        let the_box = hand.area_rect(egui::Id::new("joint_edit_popup")).expect("the box of the mate is drawn");
        assert!(view.contains(mate), "the hinge went out of the view: {mate:?} of {view:?}");
        assert!(!the_box.expand(4.0).contains(mate), "the box of the mate lies over it: the mate at {mate:?}, the box {the_box:?}");
        assert!(view.contains_rect(the_box), "the box of the mate runs out of the view: {the_box:?} of {view:?}");
    }

    /// A BOX MOVED BY A PERSON STANDS WHERE IT WAS PUT: dragged by a caption, it goes with the pointer, stays there while
    /// the view moves, and stands by its mate again once the edit is left and the mate is opened anew.
    #[test]
    fn a_box_moved_by_hand_stands_where_it_was_put_until_its_tool_goes() {
        let mut app = App::default();
        let jid = crate::gui::tests::the_box_of_a_joint_is_laid_in_sections::tests::a_hinge_edited(&mut app);
        let mut hand = Hand::new(&mut app);
        hand.frame(Vec::new()).frame(Vec::new());
        let id = egui::Id::new("joint_edit_popup");
        let by_rule = hand.area_rect(id).expect("the box of the mate is drawn");
        let caption = hand.words_drawn().iter().find(|(t, r)| *t == crate::i18n::tr("box-turn") && by_rule.contains_rect(*r)).map(|(_, r)| *r).expect("the caption of the turn is in the box");
        let pull = egui::vec2(-200.0, 100.0);
        hand.drag_screen(caption.center(), caption.center() + pull);
        hand.frame(Vec::new());
        let put = hand.area_rect(id).expect("the box of the mate is drawn");
        assert!((put.min - by_rule.min - pull).length() < 6.0, "the box did not go with the pointer: from {:?} by {pull:?}, it stands at {:?}", by_rule.min, put.min);
        let from = hand.app.viewing.view_rect.left_center() + egui::vec2(60.0, 120.0);
        hand.drag_view(ViewDrag { button: egui::PointerButton::Secondary, modifiers: egui::Modifiers::SHIFT, from, to: from + egui::vec2(160.0, -60.0) });
        hand.frame(Vec::new());
        let after_view = hand.area_rect(id).expect("the box of the mate is drawn");
        assert!((after_view.min - put.min).length() < 1.0, "the box put by hand moved with the view: from {:?} to {:?}", put.min, after_view.min);
        // left by Cancel and opened anew, it stands by its mate by the rule again
        assert!(hand.press_word(&crate::i18n::tr("cmd-cancel-esc"), after_view.center()), "no Cancel in the box");
        hand.frame(Vec::new());
        assert!(hand.app.side.joint.edit.is_none(), "Cancel did not leave the edit of the mate");
        crate::gui::enter_joint_edit(&mut hand.app.side.joint, &mut hand.app.chosen.sel, &mut hand.app.status, jid);
        hand.frame(Vec::new()).frame(Vec::new());
        let reopened = hand.area_rect(id).expect("the box of the mate is drawn");
        let mate = the_mate_on_screen(&mut hand, jid);
        assert!(reopened.min.distance(put.min) > 40.0, "the box opened anew stands where it was put: {reopened:?}, put at {:?}", put.min);
        assert!(reopened.distance_to_pos(mate) < 200.0, "the box opened anew stands away from its mate: {reopened:?}, the mate at {mate:?}");
    }

    /// What box `id` covers of what the last frame kept in sight - the handles and gizmos it drew.
    fn covered(hand: &Hand, id: egui::Id) -> Vec<egui::Rect> {
        let the_box = hand.area_rect(id).expect("the box is drawn");
        hand.kept_in_sight().into_iter().filter(|r| r.intersects(the_box)).collect()
    }

    /// NO BOX COVERS A HANDLE OR A GIZMO: the box of a datum point stands clear of its arrows and rings, the box of a
    /// datum plane clear of the arrow of its offset and the rings of its tilts, the box of a mate clear of the gizmo of
    /// its freedoms. Reported behaviour: the gizmo of a datum stood under its box.
    #[test]
    fn no_box_covers_a_handle_or_a_gizmo() {
        let mut wrong = Vec::new();
        for kind in [qymcad_ui_state::PartTool::DatumPoint, qymcad_ui_state::PartTool::DatumPlane] {
            let mut app = App::default();
            crate::gui::joint_flow::tests::add_part_at(&mut app, 0.0);
            qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
            let body = app.project.mesh_id(0).expect("the body");
            if let Some(owner) = app.project.body_owner(body) {
                app.enter_component(owner);
            }
            let side = app.project.regen_faces[&body].iter().filter(|f| f.normal[1] < -0.9).max_by(|a, b| a.area.total_cmp(&b.area)).cloned().expect("the side face");
            let mut hand = Hand::new(&mut app);
            hand.look_at([10.0, 10.0, 5.0], 8.0).tool(kind);
            if kind == qymcad_ui_state::PartTool::DatumPlane {
                hand.click([side.centroid.x, side.centroid.y, side.centroid.z]);
            }
            hand.frame(Vec::new()).frame(Vec::new()).frame(Vec::new());
            let id = egui::Id::new("feat_cmd_popup");
            if hand.kept_in_sight().is_empty() {
                wrong.push(format!("tool {kind:?}: GUARD - it drew no handles to keep clear of"));
            }
            let over = covered(&hand, id);
            if !over.is_empty() {
                wrong.push(format!("tool {kind:?}: the box {:?} covers {over:?}", hand.area_rect(id)));
            }
        }
        let mut app = App::default();
        crate::gui::tests::the_box_of_a_joint_is_laid_in_sections::tests::a_hinge_edited(&mut app);
        let mut hand = Hand::new(&mut app);
        hand.frame(Vec::new()).frame(Vec::new()).frame(Vec::new());
        let id = egui::Id::new("joint_edit_popup");
        if hand.kept_in_sight().is_empty() {
            wrong.push("the mate: GUARD - no gizmo of its freedoms drawn".to_string());
        }
        let over = covered(&hand, id);
        if !over.is_empty() {
            wrong.push(format!("the mate: the box {:?} covers {over:?}", hand.area_rect(id)));
        }
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    }
}
