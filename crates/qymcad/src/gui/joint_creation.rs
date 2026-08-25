//! A MATE IS CREATED ONLY BY A COMMAND.
//!
//! The properties panel held two A/B dropdowns, a choice of kind and a "create by origins" button. The
//! joint appeared instantly: with no geometry picked by a click, no preview, no Esc. That went round
//! the rule the whole rest of the interface is built on, and taught people to walk the wrong way —
//! while in an assembly PICKING THE GEOMETRY is the substance of the work.
//!
//! The "by origins" way is useful (parts without convenient faces, a quick rough assembly), so it was
//! not thrown out but became a FOURTH KIND OF ANCHOR inside the command itself — next to face, edge
//! and vertex. The button in the properties stayed, but it LAUNCHES the command rather than creating
//! an object: those are different things.
#[cfg(test)]
mod tests {
    use super::super::App;
    use qymcad_core::feature::AnchorRef;

    /// An assembly of two parts.
    fn two_parts() -> (App, Vec<qymcad_core::model::Id>) {
        let mut app = App::default();
        for (name, at) in [("base", 0.0), ("post", 60.0)] {
            let cid = app.project.add_component(name);
            app.enter_component_for_test(cid);
            let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
            app.project.add_rect_entity(si, 0.0, 0.0, 30.0, 20.0, qymcad_core::feature::Purpose::Real);
            app.project.regen_sketch(si);
            app.finish_sketch_edit();
            app.sel = super::super::Sel::Sketch(si);
            app.start_feat_cmd(1);
            if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
                p.val = 10.0;
                p.txt = "10".into();
            }
            app.apply_feat_cmd();
            app.rebuild_if_dirty();
            app.project.set_component_transform(cid, [1.0, 0.0, 0.0, at, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
            app.exit_context();
        }
        app.rebuild_if_dirty();
        let parts: Vec<_> = app.project.components.iter().filter(|c| c.name == "base" || c.name == "post").map(|c| c.id).collect();
        (app, parts)
    }

    /// THE PROPERTIES PANEL NO LONGER CREATES MATES.
    ///
    /// A guard over the source, and this is the case where it fits better than a behavioural one: what
    /// is checked is not the result but THE PATH — that the panel holds no call bypassing the
    /// command.
    #[test]
    fn the_properties_panel_does_not_create_joints() {
        let src = crate::gui::panels_source::PANELS;
        let panel = &src[src.find("fn joints_panel").expect("the mates panel")..];
        let panel = &panel[..panel.find("\n    pub(super) fn ").unwrap_or(panel.len())];
        assert!(!panel.contains("add_joint("), "the properties panel creates a mate bypassing the command again");
        assert!(!panel.contains("add_connector("), "the properties panel creates connectors on its own again");
        assert!(panel.contains("start_joint_pick()"), "a mate can no longer be started from the properties panel — the way was lost rather than moved");
    }

    /// "BY ORIGINS" IS NOT LOST: it became a kind of anchor inside the command.
    ///
    /// The half of the guard without which "removed the button" would look like "done": the way must
    /// stay available, otherwise it is not a move but a loss.
    #[test]
    fn joining_by_origins_is_still_possible_through_the_command() {
        let (mut app, parts) = two_parts();
        assert_eq!(parts.len(), 2, "there should be two parts in the scene");
        app.start_joint_pick();
        app.set_joint_anchor_mode_for_test(3); // "by origins"
        let bodies: Vec<_> = (0..app.project.bodies.len()).filter_map(|mi| app.project.mesh_id(mi)).collect();
        assert!(bodies.len() >= 2, "there should be two bodies");
        app.joint_pick_origin_click_for_test(bodies[0]);
        app.joint_pick_origin_click_for_test(bodies[1]);
        assert_eq!(app.project.joints.len(), 1, "a mate by origins was not created by the command");
        let j = &app.project.joints[0];
        for c in [j.a, j.b] {
            let anchor = app.project.connectors.iter().find(|x| x.id == c).map(|x| x.anchor.clone());
            assert_eq!(anchor, Some(AnchorRef::Origin), "the anchor should be the origin of the part");
        }
    }

    /// AND IN THAT MODE A CLICK ON A FACE TAKES THE PART, NOT THE FACE.
    ///
    /// Otherwise the mode would exist only in words: a person clicks what they see, and that is a
    /// face.
    #[test]
    fn in_origin_mode_a_face_click_anchors_the_part() {
        let (mut app, _) = two_parts();
        app.start_joint_pick();
        app.set_joint_anchor_mode_for_test(3);
        let body = app.project.mesh_id(0).expect("the body");
        let key = qymcad_core::feature::FaceKey { index: 0, centroid: [1.0, 1.0, 10.0], normal: [0.0, 0.0, 1.0], id: 1 };
        app.joint_pick_face_click_for_test(body, key);
        let first = app.joint_pick_first_anchor_for_test();
        assert_eq!(first, Some(AnchorRef::Origin), "a click on a face in \"by origins\" mode took the face rather than the part");
    }

    /// THE KIND OF MATE IS STILL CHOSEN — in the command bar rather than in the right panel.
    #[test]
    fn the_kind_is_chosen_in_the_command_bar() {
        let mut app = App::default();
        app.start_joint_pick();
        let bar = include_str!("joints.rs");
        let head = &bar[bar.find("fn joint_tool_bar").expect("the command bar")..];
        for k in ["Rigid", "Revolute", "Slider", "Cylindrical", "Planar", "Ball", "PinSlot"] {
            assert!(head.contains(k), "the kind \"{k}\" is gone from the command bar");
        }
        // "BY ORIGINS" BECAME A TOGGLE rather than one of four mutually exclusive modes: face, edge
        // and vertex were taken out of the bar — the kind of anchor is inferred under the cursor.
        assert!(head.contains("by_origin"), "the \"by origins\" mode did not appear in the command bar");
        for gone in ["anchor_mode, 0u8", "anchor_mode, 1u8", "anchor_mode, 2u8"] {
            assert!(!head.contains(gone), "the anchor-kind switch \"{gone}\" was supposed to go: the kind is inferred under the cursor");
        }
        assert!(app.joint_pick_active_for_test(), "the command did not start");
    }
}
