//! GRAB A PART AND THE PART ITSELF IS PULLED, WHILE THE MECHANISM FOLLOWS AS A CHAIN.
//!
//! THE REQUIREMENT: grab a subassembly anywhere in the scene, and every joint and every degree of
//! freedom there is must follow AS A CHAIN rather than one at a time.
//!
//! WHAT USED TO BE DONE INSTEAD, and where the whole class of complaints came from: the hand picked
//! ONE joint, took ONE degree of it (the one whose axis is closest to the mouse direction) and pulled
//! that SCALAR through `drive`. In a real document a click on the spindle takes the joint that
//! carries the WHOLE NODE — the node moves, the spindle inside it stands still, and a person reads
//! that as "it does not drag".
//!
//! WHAT IS DONE NOW: the hand does not pick a joint at all; it sets a GOAL, "this point of the part
//! is under the cursor", and the solver walks the part step by step through the null space of the
//! joints (`pull_towards_cursor` in `asm/iterate.rs`). The freedoms there belong to that whole part
//! of the document at once, so the chain follows by itself.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::App;
    use qymcad_core::feature::{AnchorRef, BasePlane, JointKind};
    use qymcad_core::model::Id;

    fn viewport() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0))
    }

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

    fn local_of(app: &App, comp: Id) -> [f64; 3] {
        let m = app.project.component_transform(comp);
        [m[3], m[7], m[11]]
    }

    /// A CHAIN OF TWO LINKS ON DIFFERENT AXES: A is grounded, A-B along X, B-C along Z.
    ///
    /// Grabbing C, a person drags it diagonally across the screen. One degree cannot do that: both
    /// the travel of B along X and the travel of C along Z are needed. So the check measures exactly
    /// the thing that is missing.
    fn a_two_link_chain(app: &mut App) -> (Id, Id, Id) {
        let before: Vec<Id> = app.project.bodies.iter().map(|b| b.id).collect();
        for k in 0..3 {
            super::super::joint_flow::tests::add_part_at(app, k as f64 * 60.0);
        }
        let root = app.project.root;
        app.enter_component(root);
        app.rebuild_if_dirty();
        app.refresh_edges();
        let mine: Vec<Id> = app.project.bodies.iter().map(|b| b.id).filter(|b| !before.contains(b)).collect();
        assert_eq!(mine.len(), 3, "setup: there should be three bodies of our own");
        let comps: Vec<Id> = mine.iter().map(|b| app.project.body_owner(*b).expect("the owner")).collect();
        app.project.set_grounded(comps[0], true);

        let ja = app.project.add_connector(comps[0], AnchorRef::BasePlane(BasePlane::YZ)); // normal X
        let jb = app.project.add_connector(comps[1], AnchorRef::BasePlane(BasePlane::YZ));
        app.project.add_joint(ja, jb, JointKind::Slider);
        let kb = app.project.add_connector(comps[1], AnchorRef::BasePlane(BasePlane::XY)); // normal Z
        let kc = app.project.add_connector(comps[2], AnchorRef::BasePlane(BasePlane::XY));
        app.project.add_joint(kb, kc, JointKind::Slider);

        app.rebuild_if_dirty();
        app.refresh_edges();
        let mut hand = Hand::new(app);
        hand.look_at([60.0, 10.0, 5.0], 4.0);
        app.workbench = super::super::Workbench::Assembly;
        (mine[2], comps[1], comps[2])
    }

    /// DRAG THE LAST PART DIAGONALLY — BOTH DEGREES OF THE CHAIN MOVE.
    #[test]
    fn grabbing_a_part_moves_every_free_degree_of_the_chain() {
        let mut app = App::default();
        let (body_c, comp_b, comp_c) = a_two_link_chain(&mut app);
        app.project.solve_joints();
        let (was_b, was_c) = (local_of(&app, comp_b), local_of(&app, comp_c));

        let basis = app.cam.basis();
        let at = app.project3(aim(&app, body_c), viewport(), &basis).0;
        let by = egui::vec2(40.0, -40.0); // diagonally: no single axis of the chain leads that way
        assert!(app.joint_grab_part_at_for_test(viewport(), at, by, &basis), "it must be possible to grab the part");
        // THE FRAME PASS MUST LEARN THAT THE HAND IS BUSY — otherwise the drag never reaches
        // `joint_giz_drag_to` and the whole pull is dead while the check stays green. That is exactly
        // what happened: the check called the drag directly, while the live application only asked
        // about the gizmo handle, and the reported behaviour was that a part cannot be moved with the
        // mouse anywhere, only the handles work.
        assert!(app.joint_drag_active(), "the grab happened and the frame does not know about it: the drag will go to the camera, not the part");
        for k in 1..=8 {
            app.joint_giz_drag_to_for_test(at + by * (k as f32 / 8.0), by / 8.0, viewport(), &basis);
        }
        app.joint_giz_end_for_test();

        let moved = |a: [f64; 3], b: [f64; 3]| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
        let (db, dc) = (moved(was_b, local_of(&app, comp_b)), moved(was_c, local_of(&app, comp_c)));
        assert!(db > 1.0, "the middle link did not move: {db:.3} mm — so one degree was pulled, not the chain");
        assert!(dc > 1.0, "the grabbed part did not move along ITS OWN degree: {dc:.3} mm");
    }
}
