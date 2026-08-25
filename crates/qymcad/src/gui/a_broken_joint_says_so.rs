//! A FAULTY JOINT SAYS SO IN THE LIST.
//!
//! Found on a real document: it holds FIVE joints and TWO connectors. Four rigid mates refer to
//! connectors that are not in the document. The solver skipped such joints silently, the report came
//! back empty, and the list showed them EXACTLY THE SAME as a healthy one. Five joints are visible, the
//! part does not move — and there is not one explanation. That is precisely the reported behaviour:
//! the joints do not work as intended and nothing travels.
//!
//! There was a warning — but only in the EDIT bar of a joint: to see it one already had to suspect
//! something and open the joint. The list is what people look at.
#[cfg(test)]
pub(in crate::gui) mod tests {
    use super::super::{ph, App};
    use qymcad_core::feature::{AnchorRef, JointKind};
    use qymcad_core::model::Id;

    fn viewport() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0))
    }

    /// An assembly of two parts with a joint between their origins. Returns the id of the joint and
    /// the id of connector B.
    fn assembly_with_a_joint(app: &mut App) -> (Id, Id) {
        super::super::joint_flow::tests::add_part_at(app, 0.0);
        super::super::joint_flow::tests::add_part_at(app, 60.0);
        for _ in 0..4 {
            if app.current_ctx_id_for_test() == app.project.root {
                break;
            }
            app.exit_context_for_test();
        }
        app.rebuild_if_dirty_for_test();
        let a = app.project.mesh_id(0).and_then(|b| app.project.body_owner(b)).expect("part A");
        let b = app.project.mesh_id(1).and_then(|b| app.project.body_owner(b)).expect("part B");
        app.project.set_grounded(a, true);
        let ca = app.project.add_connector(a, AnchorRef::Origin);
        let cb = app.project.add_connector(b, AnchorRef::Origin);
        let j = app.project.add_joint(ca, cb, JointKind::Rigid);
        (j, cb)
    }

    /// One frame of the mates panel — what words are drawn in it.
    fn panel_text(app: &mut App) -> Vec<String> {
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let mut texts = Vec::new();
        // TWO FRAMES: egui areas and tooltips fall into place on the second pass.
        for _ in 0..2 {
            let out = ctx.run(egui::RawInput { screen_rect: Some(viewport()), ..Default::default() }, |c| {
                egui::CentralPanel::default().show(c, |ui| app.joints_panel_for_test(ui));
            });
            texts.clear();
            for cs in &out.shapes {
                super::super::screen_keys::tests::collect_text(&cs.shape, &mut texts);
            }
        }
        texts
    }

    /// A JOINT WHOSE CONNECTOR IS GONE IS MARKED IN THE LIST, AND WHAT IS WRONG IS SAID.
    #[test]
    fn a_joint_whose_connector_is_gone_is_marked_in_the_list() {
        let mut app = App::default();
        let (_j, cb) = assembly_with_a_joint(&mut app);

        // SETUP: a healthy joint is marked with nothing — otherwise the mark means nothing.
        let healthy = panel_text(&mut app);
        let lost = crate::i18n::tr("j-fault-connector-lost");
        assert!(!healthy.iter().any(|t| t.contains(&lost)), "a healthy joint is marked as faulty: {healthy:?}");

        // The state of that document: the connector is gone, the joint has stayed.
        app.project.connectors.retain(|c| c.id != cb);
        let texts = panel_text(&mut app);
        assert!(
            texts.iter().any(|t| t.contains(&lost)),
            "a joint with no connector looks healthy in the list: \"{lost}\" was expected, and the frame holds {texts:?}"
        );
    }

    /// A JOINT WITH AN UNRESOLVABLE ANCHOR IS MARKED WITH ITS OWN REASON, NOT SOMEBODY ELSE'S.
    ///
    /// The two troubles differ: a connector is gone for good (curable only by re-choosing the anchors),
    /// while an anchor does not resolve until the geometry is raised (it cures itself). One word for
    /// both would lie.
    #[test]
    fn an_unresolvable_anchor_is_told_apart_from_a_lost_connector() {
        let mut app = App::default();
        let (_j, cb) = assembly_with_a_joint(&mut app);
        // an anchor on an edge of a body that does not exist — the geometry does not resolve
        if let Some(c) = app.project.connectors.iter_mut().find(|c| c.id == cb) {
            c.anchor = AnchorRef::EdgeMid(999_999, 1);
        }

        let texts = panel_text(&mut app);
        let anchor = crate::i18n::tr("j-fault-anchor-lost");
        let lost = crate::i18n::tr("j-fault-connector-lost");
        assert!(texts.iter().any(|t| t.contains(&anchor)), "a joint with an unresolvable anchor is not marked: {texts:?}");
        assert!(!texts.iter().any(|t| t.contains(&lost)), "an unresolvable anchor was called a lost connector: {texts:?}");
    }

    /// DELETING A JOINT IN ONE ASSEMBLY DOES NOT CRIPPLE JOINTS IN OTHERS.
    ///
    /// This is HOW a document arrives at "five joints on two connectors". The panel shows the joints of
    /// the CURRENT assembly only — and it cleaned up orphaned connectors by that same filtered list.
    /// Delete a joint in the root and the connectors of the joints of EVERY subassembly went with it;
    /// the joints themselves stayed and never worked again, silently.
    ///
    /// The check presses that very button in the frame, at a coordinate TAKEN FROM THE FRAME: the
    /// trouble lived in the handler of the button, and calling the method directly would have gone
    /// right past it.
    #[test]
    fn deleting_a_joint_here_does_not_break_a_joint_over_there() {
        let mut app = App::default();
        let (near, _cb) = assembly_with_a_joint(&mut app);

        // The second joint is IN A SUBASSEMBLY, that is, in ANOTHER context: it is not visible in the
        // list of the root.
        let sub = app.project.add_assembly("Subassembly");
        app.project.set_active_component(Some(sub));
        let p1 = app.project.add_part("P1");
        let p2 = app.project.add_part("P2");
        app.project.set_active_component(Some(app.project.root));
        let (cd, ce) = (app.project.add_connector(p1, AnchorRef::Origin), app.project.add_connector(p2, AnchorRef::Origin));
        let far = app.project.add_joint(cd, ce, JointKind::Slider);
        let ctx = app.current_ctx_id_for_test();
        assert!(
            !app.project.joints.iter().filter(|j| app.project.joint_in_context(j, ctx)).any(|j| j.id == far),
            "setup: the joint of the subassembly must not be visible in the list of the root — otherwise the trouble does not reproduce"
        );

        // THE DELETE CROSS IS PRESSED the way a person does it: the icon is found by eye and clicked.
        let at = button_pos(&mut app, ph::X).expect("the delete cross of the joint is in the frame");
        click_at(&mut app, at);
        assert!(!app.project.joints.iter().any(|j| j.id == near), "the click on the cross did not delete the joint — the check aims past it");

        assert!(app.project.joints.iter().any(|j| j.id == far), "deleting a joint in the root wiped out the joint of the subassembly");
        assert!(
            app.project.connector(cd).is_some() && app.project.connector(ce).is_some(),
            "the connectors of a joint in ANOTHER assembly were wiped out — it stayed in the document dead and silent"
        );
        assert!(app.project.joint_faults().is_empty(), "a faulty joint appeared after the deletion: {:?}", app.project.joint_faults());
    }

    /// The screen point of the widget with the caption or icon `needle` — a coordinate FROM THE FRAME.
    pub(in crate::gui) fn button_pos_in(app: &mut App, needle: &str) -> Option<egui::Pos2> {
        button_pos(app, needle)
    }

    /// A click on the mates panel at a point — for the neighbouring checks.
    pub(in crate::gui) fn click_panel_at(app: &mut App, at: egui::Pos2) {
        click_at(app, at)
    }

    fn button_pos(app: &mut App, needle: &str) -> Option<egui::Pos2> {
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let mut found = None;
        // TWO FRAMES: egui areas fall into place on the second pass.
        for _ in 0..2 {
            let out = ctx.run(egui::RawInput { screen_rect: Some(viewport()), ..Default::default() }, |c| {
                egui::CentralPanel::default().show(c, |ui| app.joints_panel_for_test(ui));
            });
            found = None;
            for cs in &out.shapes {
                super::super::screen_keys::tests::text_pos(&cs.shape, needle, &mut found);
            }
        }
        found
    }

    /// A mouse click at a point: hover, press, release — each in a frame of its own, as with a person.
    fn click_at(app: &mut App, at: egui::Pos2) {
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let mut frame = |events: Vec<egui::Event>| {
            let _ = ctx.run(egui::RawInput { screen_rect: Some(viewport()), events, ..Default::default() }, |c| {
                egui::CentralPanel::default().show(c, |ui| app.joints_panel_for_test(ui));
            });
        };
        let btn = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        frame(vec![egui::Event::PointerMoved(at)]);
        frame(vec![egui::Event::PointerMoved(at), btn(true)]);
        frame(vec![btn(false)]);
    }

    /// NO SOLUTION MEANS THE PART HOLDS STILL, AND THAT IS WRITTEN DOWN.
    ///
    /// The caption in the panel had long promised that the parts are left in place and have not flown
    /// apart, while all that time the solver recorded A COMPROMISE — a position no joint required. The
    /// promise in the interface and the behaviour of the kernel diverged silently; the check brings them
    /// together.
    #[test]
    fn with_no_solution_the_part_holds_still_and_the_panel_says_why() {
        let mut app = App::default();
        let (_j, _cb) = assembly_with_a_joint(&mut app);
        // A second rigid joint of the same part — to ANOTHER grounded anchor standing elsewhere.
        let a = app.project.mesh_id(0).and_then(|b| app.project.body_owner(b)).expect("part A");
        let b = app.project.mesh_id(1).and_then(|b| app.project.body_owner(b)).expect("part B");
        let third = app.project.add_part("P3");
        app.project.move_component(third, [300.0, 0.0, 0.0]);
        app.project.set_grounded(third, true);
        let _ = a;
        let (c3, cb2) = (app.project.add_connector(third, AnchorRef::Origin), app.project.add_connector(b, AnchorRef::Origin));
        app.project.add_joint(c3, cb2, JointKind::Rigid);

        let before = app.project.world_transform(b);
        app.project.solve_joints();
        let after = app.project.world_transform(b);
        let moved = ((after[3] - before[3]).powi(2) + (after[7] - before[7]).powi(2) + (after[11] - before[11]).powi(2)).sqrt();
        assert!(moved < 1e-9, "there is no solution, yet the part travelled {moved:.3} mm — into a compromise nobody asked for");

        let texts = panel_text(&mut app);
        let said = crate::i18n::tr("jp-conflict");
        assert!(texts.iter().any(|t| t.contains(&said)), "not a word about unsolvable joints in the panel: {texts:?}");
    }

    /// A BROKEN JOINT CAN BE MENDED BY RE-CHOOSING THE ANCHOR.
    ///
    /// A joint now outlives its geometry and turns red, so the work done is intact. But what really
    /// keeps it intact is the ability to MEND it: to show the joint another face in place of the one
    /// that vanished. Otherwise "preserved" turns into "left dead for ever", which differs from
    /// deletion by little more than a line in the list.
    #[test]
    fn a_broken_joint_comes_back_when_you_point_at_live_geometry() {
        use qymcad_core::feature::FaceKey;

        let mut app = App::default();
        // THREE PARTS, AND THAT IS NOT excess: the joint runs BETWEEN DIFFERENT parts, so mending it
        // by re-choosing onto the very part that carries the second anchor is not allowed — and rightly
        // not allowed.
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        super::super::joint_flow::tests::add_part_at(&mut app, 60.0);
        super::super::joint_flow::tests::add_part_at(&mut app, 120.0);
        for _ in 0..4 {
            if app.current_ctx_id_for_test() == app.project.root {
                break;
            }
            app.exit_context_for_test();
        }
        app.rebuild_if_dirty_for_test();
        let (ba, bb) = (app.project.mesh_id(0).expect("body A"), app.project.mesh_id(1).expect("body B"));
        let bc = app.project.mesh_id(2).expect("body C");
        let (a, b) = (app.project.body_owner(ba).expect("part A"), app.project.body_owner(bb).expect("part B"));
        app.project.set_grounded(a, true);
        let key = |app: &App, body: Id| {
            let f = app.project.regen_faces.get(&body).and_then(|fs| fs.first().cloned()).expect("a face");
            FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id }
        };
        let ka = key(&app, ba);
        let ca = app.project.add_connector(a, AnchorRef::FaceCenter(ba, ka));
        let cb = app.project.add_connector(b, AnchorRef::Origin);
        let jid = app.project.add_joint(ca, cb, JointKind::Rigid);
        assert!(app.project.joint_faults().is_empty(), "setup: the joint is healthy");

        // THE BODY OF THE ANCHOR VANISHES — the joint stays and turns red.
        app.project.timeline.retain(|n| n.kind.body() != Some(ba));
        app.project.drop_connectors_of_dead_bodies(&[ba]);
        assert!(!app.project.joint_faults().is_empty(), "setup: the joint must turn red");

        // THE ANCHOR IS CHANGED: "change the anchor" for side A, then a click on a live face.
        app.joint.edit = Some(jid);
        app.joint.edit_repick = Some((jid, false));
        app.joint.anchor_mode = 0;
        let kc = key(&app, bc);
        app.joint_edit_repick_apply_for_test(bc, AnchorRef::FaceCenter(bc, kc));

        assert!(app.project.joint_faults().is_empty(), "the joint did not come back to life after the anchor was re-chosen: {:?}", app.project.joint_faults());
        assert!(app.joint.edit_repick.is_none(), "the change-the-anchor mode did not close — a person will not realise they have mended it");
    }
}
