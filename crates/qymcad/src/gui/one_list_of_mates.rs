//! EVERYTHING THAT HOLDS THE PARTS TOGETHER IS SEEN AS ONE LIST.
//!
//! Joints, constraints (group, width, tangent) and relations used to be drawn by THREE SEPARATE
//! LOOPS: each with its own row, its own delete button and its own idea of whether the element is
//! sound. Three ideas about one and the same thing drift apart silently — and a person sees what is
//! not there.
//!
//! The check is live and judges BY THE FRAME: the panel is really drawn, and all three kinds are
//! looked for in its words. The timeline core is checked separately
//! (`qymcad-core/tests/one_mate_timeline.rs`) — checked here is that the panel draws that timeline
//! rather than working it out again in its own way.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::App;
    use qymcad_core::feature::{JointKind, RelationKind};
    use qymcad_core::model::Id;

    fn viewport() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0))
    }

    /// The words DRAWN by the mates panel.
    fn panel_words(app: &mut App) -> Vec<String> {
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        app.workbench = super::super::Workbench::Assembly;
        app.mode_3d = true;
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

    /// A point ON THE BODY that can be clicked: the centre of the topmost face.
    fn aim(app: &App, body: Id) -> [f64; 3] {
        let wt = app.project.body_display_transform(body, app.current_ctx_id_for_test());
        let f = app
            .project
            .regen_faces
            .get(&body)
            .and_then(|fs| fs.iter().max_by(|a, b| a.centroid.z.total_cmp(&b.centroid.z)))
            .expect("the body has faces");
        qymcad_core::feature::apply12(&wt, [f.centroid.x, f.centroid.y, f.centroid.z])
    }

    /// AN ASSEMBLY HOLDING ALL THREE KINDS: two joints, a group and a relation between the joints.
    fn all_three_kinds(app: &mut App) -> (Id, Id, Id) {
        let before: Vec<Id> = app.project.bodies.iter().map(|b| b.id).collect();
        for x in [0.0, 60.0, 200.0, 260.0] {
            super::super::joint_flow::tests::add_part_at(app, x);
        }
        let root = app.project.root;
        app.enter_component(root);
        app.rebuild_if_dirty();
        app.refresh_edges();
        let mine: Vec<Id> = app.project.bodies.iter().map(|b| b.id).filter(|b| !before.contains(b)).collect();
        assert_eq!(mine.len(), 4, "setup: there should be four bodies of our own, and there are {}", mine.len());
        for (k, b) in mine.iter().enumerate() {
            if let Some(o) = app.project.body_owner(*b) {
                if let Some(i) = app.project.component_index(o) {
                    let x = [0.0, 60.0, 200.0, 260.0][k];
                    app.project.components[i].transform = [1.0, 0.0, 0.0, x, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];
                }
                if k == 0 {
                    app.project.set_grounded(o, true);
                }
            }
        }
        app.rebuild_if_dirty();
        app.refresh_edges();

        // TWO REVOLUTE JOINTS — by hand, two clicks each.
        let (pa, pb) = (aim(app, mine[0]), aim(app, mine[1]));
        let mut hand = Hand::new(app);
        hand.look_at([30.0, 10.0, 5.0], 6.0).mate(JointKind::Revolute).anchor(3).click(pa).click(pb);
        let first = app.project.joints.last().map(|j| j.id).expect("the first joint");
        let (pc, pd) = (aim(app, mine[2]), aim(app, mine[3]));
        let mut hand = Hand::new(app);
        hand.look_at([230.0, 10.0, 5.0], 6.0).mate(JointKind::Revolute).anchor(3).click(pc).click(pd);
        let second = app.project.joints.last().map(|j| j.id).expect("the second joint");
        assert_ne!(first, second, "setup: there should be two joints");

        // A GROUP — a constraint that does not reduce to a pair of anchors.
        let members: Vec<Id> = mine.iter().filter_map(|b| app.project.body_owner(*b)).collect();
        let group = app.project.add_group(&members[..2]);

        // A RELATION — by the same tool a person uses.
        app.start_relation_pick_for_test();
        app.relation_pick_set_for_test(RelationKind::Gear, 2.0);
        app.relation_pick_click_for_test(first);
        app.relation_pick_click_for_test(second);
        app.relation_pick_confirm_for_test();
        app.rebuild_if_dirty();
        let relation = app.project.relations.last().map(|r| r.id).expect("the relation was created");
        (first, group, relation)
    }

    #[test]
    fn mates_constraints_and_relations_are_drawn_in_one_list() {
        let mut app = App::default();
        let (joint, group, relation) = all_three_kinds(&mut app);

        // TRAP GUARD: all three really are in the document, otherwise there is nothing to draw.
        let line = app.project.mate_timeline(app.current_ctx_id_for_test());
        for (what, id) in [("joint", joint), ("group", group), ("relation", relation)] {
            assert!(line.iter().any(|e| e.id == id), "setup: {what} {id} is not in the timeline — there is nothing to check");
        }

        let words = panel_words(&mut app);
        assert!(words.len() > 5, "GUARD: the panel drew suspiciously few lines ({})", words.len());
        // EVERY KIND IS NAMED BY ITS OWN WORD — and all three are in one list.
        for key in [JointKind::Revolute.label(), qymcad_core::feature::ConstraintKind::Group.label(), RelationKind::Gear.label()] {
            let want = crate::i18n::tr(key);
            assert!(
                words.iter().any(|t| t.contains(&want)),
                "the kind \"{want}\" is not drawn in the mates list — so it lives somewhere apart, or does not live at all.\ndrawn: {words:?}"
            );
        }
    }

    /// A FAULT IS NAMED IN THE SAME ROW, WHATEVER KIND THE ELEMENT IS.
    ///
    /// Before, only a joint could say what was wrong with it: a relation had a check of its own, and a
    /// constraint had none at all. The state is now one for the whole timeline — and so are the
    /// words.
    #[test]
    fn a_broken_relation_says_so_in_the_same_list() {
        let mut app = App::default();
        let (_, _, relation) = all_three_kinds(&mut app);
        // REMOVE THE SECOND JOINT — the relation has nothing left to rest on.
        let second = app.project.relations.iter().find(|r| r.id == relation).map(|r| r.b).expect("the second degree");
        app.project.joints.retain(|j| j.id != second);
        app.rebuild_if_dirty();

        let words = panel_words(&mut app);
        let want = crate::i18n::tr("r-fault-mate-lost");
        assert!(
            words.iter().any(|t| t.contains(&want)),
            "the relation points at a removed joint and the list stays silent: drawn {words:?}"
        );
    }
}
