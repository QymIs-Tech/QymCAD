//! "HOLD IT AS IT STANDS" — A JOINT DECLARES THE CURRENT ARRANGEMENT ITS OWN (as-built).
//!
//! An ordinary joint ALIGNS its anchors, and that is right when a part is being placed by the joint.
//! But an assembly arranged by hand or brought in by import must not be aligned: the parts already
//! stand where they should, and the joint is needed only so that they do not drift apart from now on.
//! Without this the very first joint collapses an imported assembly, and the offsets have to be found
//! by hand.
//!
//! An ability with no button does not exist for a person, so the whole path is checked: joint popup
//! -> button -> the part stayed where it was.
#[cfg(test)]
mod tests {
    use super::super::App;
    use qymcad_core::feature::apply12;

    fn viewport() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0))
    }

    /// A RIGID JOINT HAS A TWIST FIELD IN ITS POPUP.
    ///
    /// An ability with no field does not exist for a person: the kernel can turn a fastened part
    /// around the axis of the junction, but while the field was shown only for the rotating kinds
    /// there was nothing to reach it with.
    #[test]
    fn a_fastened_joint_shows_its_twist_field() {
        let mut app = App::default();
        let ([_, _], [wheel_a, wheel_b]) = super::super::a_relation_is_made_by_hand::tests::two_hinges(&mut app);
        // a RIGID joint of our own between the same parts
        let ca = app.project.add_connector(wheel_a, qymcad_core::feature::AnchorRef::Origin);
        let cb = app.project.add_connector(wheel_b, qymcad_core::feature::AnchorRef::Origin);
        let rigid = app.project.add_joint(ca, cb, qymcad_core::feature::JointKind::Rigid);

        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        app.workbench = super::super::Workbench::Assembly;
        app.mode_3d = true;
        app.joint.edit = Some(rigid);
        let mut texts = Vec::new();
        // an egui popup settles on the SECOND frame — draw it twice
        for _ in 0..2 {
            let out = ctx.run_ui(egui::RawInput { screen_rect: Some(viewport()), ..Default::default() }, |c| {
                app.joint_popup_for_test(c, viewport());
            });
            texts.clear();
            for cs in &out.shapes {
                super::super::screen_keys::tests::collect_text(&cs.shape, &mut texts);
            }
        }
        // the twist field in the popup is captioned by the key of the slot itself, not by the
        // angle caption from the creation bar
        let want = crate::i18n::tr("j-angle-lower");
        assert!(
            texts.iter().any(|t| t.contains(&want)),
            "the rigid joint has no twist field \"{want}\": drawn {texts:?}"
        );
        // AND THE JOINT NAME IS A WORD, NOT A CATALOGUE CODE. This is where that surfaced: the popup
        // read "joint-kind-rigid 3", because the name was assembled as "key space number" while the
        // name translator only parses "key#argument".
        assert!(
            !texts.iter().any(|t| t.contains("joint-kind-")),
            "the popup shows a CODE instead of the joint name: {texts:?}"
        );
    }

    /// "HOLD IT AS IT STANDS" IS IN THE JOINT POPUP AND WORKS FROM THERE (as-built).
    ///
    /// An ability with no button does not exist for a person. Checked here is exactly the path a
    /// person will take: joint popup -> button -> the part stayed where it was.
    #[test]
    fn a_person_can_declare_the_current_arrangement_from_the_joint_popup() {
        let mut app = App::default();
        let ([ja, _], [wheel_a, _]) = super::super::a_relation_is_made_by_hand::tests::two_hinges(&mut app);
        // move the wheel aside: an ordinary hinge would pull it back to the axis of the housing
        if let Some(i) = app.project.component_index(wheel_a) {
            app.project.components[i].transform[3] = 40.0;
        }

        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        app.workbench = super::super::Workbench::Assembly;
        app.mode_3d = true;
        app.joint.edit = Some(ja);
        let mut texts = Vec::new();
        // an egui popup settles on the SECOND frame — draw it twice
        for _ in 0..2 {
            let out = ctx.run_ui(egui::RawInput { screen_rect: Some(viewport()), ..Default::default() }, |c| {
                app.joint_popup_for_test(c, viewport());
            });
            texts.clear();
            for cs in &out.shapes {
                super::super::screen_keys::tests::collect_text(&cs.shape, &mut texts);
            }
        }
        let want = crate::i18n::tr("j-as-built");
        assert!(texts.iter().any(|t| t.contains(&want)), "the joint popup has no \"{want}\" button: drawn {texts:?}");

        // press it by the same path a person presses it
        assert!(app.project.set_joint_as_built(ja), "declaring the arrangement as built must go through");
        app.project.solve_joints();
        let o = apply12(&app.project.world_transform(wheel_a), [0.0, 0.0, 0.0]);
        assert!((o[0] - 40.0).abs() < 1e-3, "after the declaration the part must stay at 40 mm, and it ended up at {o:?}");
    }

}
