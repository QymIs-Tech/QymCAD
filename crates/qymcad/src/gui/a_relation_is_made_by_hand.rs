//! A RELATION BETWEEN MATES IS CREATED BY A PERSON.
//!
//! In the model the relation appeared before the interface did — that very "visible progress" state:
//! the ability exists and there is nothing to reach it with. Checked here is the whole path: take the
//! tool, click two mates, set the number, confirm, see the row in the panel, delete it.
//!
//! A RELATION IS POINTED AT BY MATES RATHER THAN BY GEOMETRY, and that is not a departure from the
//! single-command contract but its fulfilment: a relation has no subject in the viewport. Grown-up CAD
//! takes the mates for a relation from a list in exactly the same way.
#[cfg(test)]
pub(in crate::gui) mod tests {
    use super::super::App;
    use qymcad_core::feature::{AnchorRef, JointKind, RelationKind};
    use qymcad_core::model::Id;

    fn viewport() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0))
    }

    /// An assembly of two housings and two wheels: the two hinges that are to be tied by a relation.
    ///
    /// Returns both mates and both wheels.
    pub(in crate::gui) fn two_hinges(app: &mut App) -> ([Id; 2], [Id; 2]) {
        // take OUR OWN four parts: a clean document may already hold a stock part, and "the first
        // four children of the root" would point at the wrong thing
        let before: Vec<Id> = app.project.components.iter().filter(|c| c.parent == Some(app.project.root)).map(|c| c.id).collect();
        for x in [0.0, 30.0, 60.0, 90.0] {
            super::super::joint_flow::tests::add_part_at(app, x);
        }
        for _ in 0..4 {
            if app.current_ctx_id_for_test() == app.project.root {
                break;
            }
            app.exit_context_for_test();
        }
        app.rebuild_if_dirty_for_test();
        let parts: Vec<Id> = app
            .project
            .components
            .iter()
            .filter(|c| c.parent == Some(app.project.root) && !before.contains(&c.id))
            .map(|c| c.id)
            .collect();
        assert_eq!(parts.len(), 4, "setup: there should be four parts of our own, and there are {}", parts.len());
        let (hub_a, wheel_a, hub_b, wheel_b) = (parts[0], parts[1], parts[2], parts[3]);
        app.project.set_grounded(hub_a, true);
        app.project.set_grounded(hub_b, true);
        let ca = app.project.add_connector(hub_a, AnchorRef::Origin);
        let cb = app.project.add_connector(wheel_a, AnchorRef::Origin);
        let ja = app.project.add_joint(ca, cb, JointKind::Revolute);
        let cc = app.project.add_connector(hub_b, AnchorRef::Origin);
        let cd = app.project.add_connector(wheel_b, AnchorRef::Origin);
        let jb = app.project.add_joint(cc, cd, JointKind::Revolute);
        ([ja, jb], [wheel_a, wheel_b])
    }

    /// The words drawn by the mates panel.
    fn panel_text(app: &mut App) -> Vec<String> {
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let mut texts = Vec::new();
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

    /// TOOL TAKEN, TWO JOINTS CLICKED, CONFIRMED — THE RELATION EXISTS, IS VISIBLE AND WORKS.
    #[test]
    fn a_person_can_pick_two_mates_and_tie_them_with_a_gear_relation() {
        let mut app = App::default();
        let ([ja, jb], [_, wheel_b]) = two_hinges(&mut app);
        assert!(app.project.relations.is_empty(), "setup: there are no relations yet");

        app.start_relation_pick_for_test();
        assert!(app.relation_pick_active_for_test(), "the relation tool was not taken up");
        app.relation_pick_set_for_test(RelationKind::Gear, 2.0);

        app.relation_pick_click_for_test(ja);
        assert_eq!(app.relation_pick_count_for_test(), 1, "after the first click the selection should hold one degree");
        // A SECOND CLICK ON THE SAME JOINT WILL NOT DO: a gear needs TWO different mates.
        app.relation_pick_click_for_test(ja);
        assert_eq!(app.relation_pick_count_for_test(), 1, "the same joint was taken twice — the selection should have refused");
        app.relation_pick_click_for_test(jb);
        assert_eq!(app.relation_pick_count_for_test(), 2, "after the second click the selection should hold two degrees");

        app.relation_pick_confirm_for_test();
        assert!(!app.relation_pick_active_for_test(), "the tool was not released after confirmation");
        assert_eq!(app.project.relations.len(), 1, "the relation was not created");
        let r = app.project.relations[0].clone();
        assert_eq!(r.kind, RelationKind::Gear, "the kind of relation must be the one that was chosen");
        assert!((r.value - 2.0).abs() < 1e-12, "the number must be the one that was set: {}", r.value);
        assert!(app.project.relation_faults().is_empty(), "a fresh relation must be sound: {:?}", app.project.relation_faults());

        // AND IT WORKS: the driving wheel at 30 deg means the driven one at 60.
        if let Some(j) = app.project.joints.iter_mut().find(|j| j.id == ja) {
            j.drive[0] = Some(30.0);
        }
        app.project.solve_joints();
        let m = app.project.world_transform(wheel_b);
        let o = qymcad_core::feature::apply12(&m, [0.0, 0.0, 0.0]);
        let x = qymcad_core::feature::apply12(&m, [1.0, 0.0, 0.0]);
        let spin = (x[1] - o[1]).atan2(x[0] - o[0]).to_degrees();
        assert!((spin - 60.0).abs() < 1e-2, "with a ratio of 2 the driven wheel must stand at 60 deg, and it stands at {spin:.4} deg");
    }

    /// THE RELATION IS VISIBLE IN THE PANEL AND CAN BE DELETED FROM THERE.
    ///
    /// Without a row in the list it can neither be found nor removed, and a person is left with a part
    /// that moves "by itself".
    #[test]
    fn the_relation_shows_up_in_the_panel_and_can_be_deleted() {
        let mut app = App::default();
        let ([ja, jb], _) = two_hinges(&mut app);
        app.project.add_relation(RelationKind::Gear, ja, 0, jb, 0, 2.0);

        let texts = panel_text(&mut app);
        let want = crate::i18n::tr("relation-kind-gear");
        assert!(
            texts.iter().any(|t| t.contains(&want)),
            "the panel does not show the relation: \"{want}\" was looked for, and what is drawn is {texts:?}"
        );

        app.project.delete_relation(app.project.relations[0].id);
        let texts = panel_text(&mut app);
        assert!(!texts.iter().any(|t| t.contains(&want)), "a deleted relation stayed in the panel: {texts:?}");
    }

    /// A BROKEN RELATION SAYS SO IN THE PANEL RATHER THAN STAYING SILENT.
    #[test]
    fn a_broken_relation_says_so_in_the_panel() {
        let mut app = App::default();
        let ([ja, jb], _) = two_hinges(&mut app);
        app.project.add_relation(RelationKind::Gear, ja, 0, jb, 0, 2.0);
        // the joint the relation looked at is deleted — the relation must turn red
        app.project.delete_joint(jb);
        let want = crate::i18n::tr("r-fault-mate-lost");
        let texts = panel_text(&mut app);
        assert!(
            texts.iter().any(|t| t.contains(&want)),
            "the panel says nothing about a broken relation: \"{want}\" was looked for, and what is drawn is {texts:?}"
        );
    }

    /// CHANGING THE KIND DROPS THE SELECTION.
    ///
    /// The degrees that were pointed at were of the right sort for the PREVIOUS kind. Keeping them
    /// would mean building the relation on the wrong degrees — and a person would learn of it only
    /// from an assembly that had come apart.
    #[test]
    fn changing_the_relation_kind_drops_what_was_already_picked() {
        let mut app = App::default();
        let ([ja, _], _) = two_hinges(&mut app);
        app.start_relation_pick_for_test();
        app.relation_pick_set_for_test(RelationKind::Gear, 2.0);
        app.relation_pick_click_for_test(ja);
        assert_eq!(app.relation_pick_count_for_test(), 1, "setup: one degree is taken");
        app.relation_pick_set_for_test(RelationKind::Linear, 2.0);
        assert_eq!(app.relation_pick_count_for_test(), 0, "changing the kind must drop the selection");
    }

    /// A JOINT WITHOUT THE NEEDED DEGREE IS REFUSED IN WORDS.
    ///
    /// A linear relation asks for TRAVEL, and a hinge has none. Swallowing such a click silently would
    /// leave a person guessing why the selection does not grow.
    #[test]
    fn a_mate_without_the_needed_degree_is_refused_with_words() {
        let mut app = App::default();
        let ([ja, _], _) = two_hinges(&mut app);
        app.start_relation_pick_for_test();
        app.relation_pick_set_for_test(RelationKind::Linear, 2.0);
        app.relation_pick_click_for_test(ja);
        assert_eq!(app.relation_pick_count_for_test(), 0, "a hinge has no travel — there was nothing to take");
        let want = crate::i18n::tr("j-relation-need-travel");
        assert_eq!(app.status, want, "the refusal must be named in words, and the status line holds: {}", app.status);
    }
}
