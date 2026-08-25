//! A JOINT RAISED TO THE ROOT IS DRAGGED BY THE PART ITSELF — LIKE EVERY OTHER ONE.
//!
//! Reported behaviour: tick "drive from the root" on a joint inside a subassembly and yes, it becomes
//! visible from the root and can be driven by the gizmo handle — but dragging that same joint by the
//! selected part itself, the way it works in the main assembly, is impossible, which seems illogical.
//!
//! And it really is illogical. "Drive from the root" means exactly one thing: the joint IS IN EFFECT
//! here. Then all the ways of driving it must be available, not half of them: the gizmo handle and a
//! hand on the part are two entrances into one and the same action, and nobody chooses between them
//! by where the joint happens to live.
//!
//! The cause was in the question the hand was asking. It asked "which joint drives THE COMPONENT OF
//! THIS CONTEXT" — that is, the subassembly as a whole — while a raised joint drives a part INSIDE
//! it. The answer honestly came back empty, and the hand silently let go.
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

    fn origin_of(app: &App, comp: Id) -> [f64; 3] {
        qymcad_core::feature::apply12(&app.project.world_transform(comp), [0.0, 0.0, 0.0])
    }

    /// A SUBASSEMBLY WITH A SLIDER INSIDE IT, RAISED TO THE ROOT. Returns (body of the driven part,
    /// its component).
    ///
    /// Built the same way a person builds it: two parts are put INTO A SUBASSEMBLY, a joint between
    /// them, and the joint is ticked "drive from the root". Then it is looked at FROM THE ROOT.
    fn a_subassembly_with_a_global_slider(app: &mut App) -> (Id, Id) {
        let before: Vec<Id> = app.project.bodies.iter().map(|b| b.id).collect();
        let root = app.project.root;
        super::super::joint_flow::tests::add_part_at(app, 0.0);
        super::super::joint_flow::tests::add_part_at(app, 60.0);
        while app.current_ctx_id_for_test() != root {
            app.exit_context_for_test(); // back to the root by the same path a person takes
        }
        app.rebuild_if_dirty();
        app.refresh_edges();
        let mine: Vec<Id> = app.project.bodies.iter().map(|b| b.id).filter(|b| !before.contains(b)).collect();
        assert_eq!(mine.len(), 2, "setup: there should be two bodies of our own, and there are {}", mine.len());
        let comps: Vec<Id> = mine.iter().map(|b| app.project.body_owner(*b).expect("the owner of the body")).collect();
        // THE PARTS ARE MOVED INTO THE SUBASSEMBLY by the same method dragging in the tree uses
        app.project.set_active_component(Some(root));
        let sub = app.project.add_assembly("node");
        for c in &comps {
            assert!(app.project.reparent_component(*c, sub), "setup: the part must move into the subassembly");
            assert_eq!(app.project.components.iter().find(|x| x.id == *c).and_then(|x| x.parent), Some(sub), "setup: the part must lie INSIDE THE SUBASSEMBLY");
        }
        app.project.set_grounded(comps[0], true);
        let ca = app.project.add_connector(comps[0], AnchorRef::BasePlane(BasePlane::YZ)); // normal X
        let cb = app.project.add_connector(comps[1], AnchorRef::BasePlane(BasePlane::YZ));
        let jid = app.project.add_joint(ca, cb, JointKind::Slider);
        app.project.joints.iter_mut().find(|x| x.id == jid).unwrap().global = true; // the "drive from the root" tick

        // LOOK FROM THE ROOT — where the complaint comes from
        app.project.set_active_component(Some(root));
        app.rebuild_if_dirty();
        app.refresh_edges();
        let mut hand = Hand::new(app);
        hand.look_at([30.0, 10.0, 5.0], 5.0);
        app.workbench = super::super::Workbench::Assembly;
        assert_eq!(app.current_ctx_id_for_test(), root, "setup: it must be looked at FROM THE ROOT");
        let j = app.project.joints.iter().find(|x| x.id == jid).expect("the joint is there").clone();
        assert!(app.project.joint_in_context(&j, root), "setup: a raised joint must be visible in the root");
        (mine[1], comps[1])
    }

    /// THE PART IS GRABBED FROM THE ROOT TOO — AND IT MOVES.
    #[test]
    fn a_part_driven_by_a_global_mate_can_be_grabbed_from_the_root() {
        let mut app = App::default();
        let (body, comp) = a_subassembly_with_a_global_slider(&mut app);
        app.project.solve_joints();
        let was = origin_of(&app, comp);

        let basis = app.cam.basis();
        let at = app.project3(aim(&app, body), viewport(), &basis).0;
        let by = egui::vec2(50.0, 0.0);
        assert!(
            app.joint_grab_part_at_for_test(viewport(), at, by, &basis),
            "the joint is raised to the root and the part cannot be grabbed from there: the hand refused silently"
        );
        for k in 1..=4 {
            let step = by * (k as f32 / 4.0);
            app.joint_giz_drag_to_for_test(at + step, by / 4.0, viewport(), &basis);
        }
        app.joint_giz_end_for_test();

        let now = origin_of(&app, comp);
        let moved = ((now[0] - was[0]).powi(2) + (now[1] - was[1]).powi(2) + (now[2] - was[2]).powi(2)).sqrt();
        assert!(moved > 1.0, "the part was grabbed and led — and it moved only {moved:.3} mm ({was:?} -> {now:?})");
    }

    /// AND WITHOUT THE TICK IT IS NOT GRABBED, AND THAT IS A RULE TOO.
    ///
    /// The other side of it: a joint not raised to the root IS NOT in effect there — neither by handle
    /// nor by hand. Otherwise the tick would mean nothing, and a person would lose track of what is
    /// driven where.
    #[test]
    fn without_the_checkbox_the_part_is_not_grabbed_from_the_root() {
        let mut app = App::default();
        let (body, _) = a_subassembly_with_a_global_slider(&mut app);
        let jid = app.project.joints.last().map(|j| j.id).expect("the joint");
        app.project.joints.iter_mut().find(|x| x.id == jid).unwrap().global = false; // the tick was cleared
        app.project.solve_joints();

        let basis = app.cam.basis();
        let at = app.project3(aim(&app, body), viewport(), &basis).0;
        let by = egui::vec2(50.0, 0.0);
        assert!(
            !app.joint_grab_part_at_for_test(viewport(), at, by, &basis),
            "the tick is cleared — the joint is not in effect in the root, and the hand must not take hold of it"
        );
    }
}
