//! A GROUP IS ASSEMBLED BY HAND.
//!
//! In the model the group appeared before the interface did, and that is exactly the state called
//! "visible progress": the ability exists and there is nothing to reach it with. Checked here is the
//! whole path — take the tool, click the parts, confirm, see the line in the panel, delete it.
//!
//! The expected behaviour: pick the group tool and select the parts; new ones can be added later by
//! editing the group.
#[cfg(test)]
mod tests {
    use super::super::App;
    use qymcad_core::model::Id;

    fn viewport() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0))
    }

    /// An assembly of three parts in the root. Returns their bodies.
    fn three_parts(app: &mut App) -> Vec<Id> {
        for x in [0.0, 60.0, 120.0] {
            super::super::joint_flow::tests::add_part_at(app, x);
        }
        for _ in 0..4 {
            if app.current_ctx_id_for_test() == app.project.root {
                break;
            }
            app.exit_context_for_test();
        }
        app.rebuild_if_dirty_for_test();
        (0..3).filter_map(|i| app.project.mesh_id(i)).collect()
    }

    /// The words drawn by the mates panel.
    fn panel_text(app: &mut App) -> Vec<String> {
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let mut texts = Vec::new();
        for _ in 0..2 {
            let out = ctx.run_ui(egui::RawInput { screen_rect: Some(viewport()), ..Default::default() }, |c| {
                egui::CentralPanel::default().show(c, |ui| app.joints_panel_for_test(ui));
            });
            texts.clear();
            for cs in &out.shapes {
                super::super::screen_keys::tests::collect_text(&cs.shape, &mut texts);
            }
        }
        texts
    }

    /// TOOL TAKEN, PARTS CLICKED, CONFIRMED — THE GROUP EXISTS AND IS VISIBLE.
    #[test]
    fn a_person_can_pick_parts_and_make_a_group_of_them() {
        let mut app = App::default();
        let bodies = three_parts(&mut app);
        assert!(app.project.mate_constraints.is_empty(), "setup: there are no groups yet");

        app.start_group_pick_for_test();
        assert!(app.group_pick_active_for_test(), "the group tool was not taken up");

        // Click two of the three parts — as the mouse does on the bodies.
        app.group_pick_click_for_test(bodies[0]);
        app.group_pick_click_for_test(bodies[1]);
        assert_eq!(app.group_pick_members_for_test().len(), 2, "two parts were clicked, and the set holds {:?}", app.group_pick_members_for_test());

        // Once more on the same part — the selection comes off: the set is edited, not accumulated.
        app.group_pick_click_for_test(bodies[1]);
        assert_eq!(app.group_pick_members_for_test().len(), 1, "a second click on a part must take it off the set");
        app.group_pick_click_for_test(bodies[1]);

        app.group_pick_confirm_for_test();
        assert_eq!(app.project.mate_constraints.len(), 1, "the group was not created");
        assert_eq!(app.project.mate_constraints[0].members.len(), 2, "the wrong parts ended up in the group: {:?}", app.project.mate_constraints[0].members);
        assert!(!app.group_pick_active_for_test(), "the tool was not released after confirmation");

        // AND IT IS VISIBLE IN THE PANEL — otherwise it cannot be found or deleted.
        let texts = panel_text(&mut app);
        let want = crate::i18n::name(&app.project.mate_constraints[0].name);
        assert!(texts.iter().any(|t| t.contains(&want)), "the group is not in the mates panel: expected \"{want}\", in the frame {texts:?}");
    }

    /// A GROUP OF FEWER THAN TWO PARTS CANNOT BE MADE: there is nothing to fasten, and staying
    /// silent about it is not allowed.
    #[test]
    fn a_group_of_one_is_refused_out_loud() {
        let mut app = App::default();
        let bodies = three_parts(&mut app);

        app.start_group_pick_for_test();
        app.group_pick_click_for_test(bodies[0]);
        app.group_pick_confirm_for_test();

        assert!(app.project.mate_constraints.is_empty(), "a group of one part was assembled — there is nothing in it to fasten");
        assert!(app.group_pick_active_for_test(), "the tool was dropped instead of letting the second part be clicked");
        assert_eq!(app.status, crate::i18n::tr("j-group-need-two"), "the person was not told what is missing: {}", app.status);
    }

    /// A GROUP CAN BE DELETED FROM THE PANEL — otherwise it is forever.
    #[test]
    fn a_group_can_be_deleted_from_the_panel() {
        let mut app = App::default();
        let bodies = three_parts(&mut app);
        let owners: Vec<Id> = bodies.iter().filter_map(|&b| app.project.body_owner(b)).collect();
        app.project.add_group(&owners[..2]);
        assert_eq!(app.project.mate_constraints.len(), 1, "setup: the group is there");

        let at = super::super::a_broken_joint_says_so::tests::button_pos_in(&mut app, super::super::ph::X).expect("the delete cross in the panel");
        super::super::a_broken_joint_says_so::tests::click_panel_at(&mut app, at);
        assert!(app.project.mate_constraints.is_empty(), "the group was not deleted by the cross: {:?}", app.project.mate_constraints);
    }
}
