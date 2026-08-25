//! PULL THE HANDLE THAT WAY AND THE PART GOES THAT WAY.
//!
//! Reported behaviour: the directions of a slider are simply wrong, whatever is chosen. The kernel
//! already checks that the ARROW matches the travel; checked here is what the kernel cannot see —
//! that DRAGGING WITH THE MOUSE along the drawn arrow moves the part the same way.
//!
//! The anchors deliberately face EACH OTHER: then the solver turns the first anchor around by itself
//! (the mating side is chosen by proximity), and a half-turn about X flips the axis of travel. That
//! is exactly where the arrow used to part ways with the motion.
#[cfg(test)]
mod tests {
    use super::super::App;
    use qymcad_core::feature::{apply12, AnchorRef, JointKind};
    use qymcad_core::model::Id;

    fn viewport() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0))
    }

    /// A slider between anchors facing EACH OTHER. Returns the joint and the driven part.
    fn slider_with_opposed_anchors(app: &mut App) -> (Id, Id) {
        let before: Vec<Id> = app.project.components.iter().filter(|c| c.parent == Some(app.project.root)).map(|c| c.id).collect();
        for x in [0.0, 40.0] {
            super::super::joint_flow::tests::add_part_at(app, x);
        }
        for _ in 0..4 {
            if app.current_ctx_id_for_test() == app.project.root {
                break;
            }
            app.exit_context_for_test();
        }
        app.rebuild_if_dirty_for_test();
        let mine: Vec<Id> = app
            .project
            .components
            .iter()
            .filter(|c| c.parent == Some(app.project.root) && !before.contains(&c.id))
            .map(|c| c.id)
            .collect();
        assert_eq!(mine.len(), 2, "setup: there should be two parts of our own, and there are {}", mine.len());
        let (a, b) = (mine[0], mine[1]);
        app.project.set_grounded(a, true);
        // the second part is given a half-turn about X — its anchor faces the first one
        if let Some(i) = app.project.component_index(b) {
            app.project.components[i].transform = [1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 40.0];
        }
        let ca = app.project.add_connector(a, AnchorRef::Origin);
        let cb = app.project.add_connector(b, AnchorRef::Origin);
        let jid = app.project.add_joint(ca, cb, JointKind::Slider);
        (jid, b)
    }

    /// A DRAG STOPS AT THE LIMIT INSTEAD OF FLYING THROUGH IT.
    ///
    /// Checked by measurement: drag far past the limit and see where the part is.
    ///
    /// THE OUTCOME IS MEASURED, NOT THE MECHANISM, and that was found out by damaging it: remove the
    /// clamp inside the drag itself (`clamp_slot` in `apply_joint_giz`) and the check does NOT go red,
    /// because the limit is also held by the joint (`Joint::clamp_free`). Two guards on one rule is
    /// not a defect, but knowing that the check cannot tell them apart is essential: it says "the part
    /// stopped", not "the drag stopped it".
    #[test]
    fn dragging_past_the_limit_stops_at_the_mark() {
        let mut app = App::default();
        let (jid, part) = slider_with_opposed_anchors(&mut app);
        if let Some(j) = app.project.joints.iter_mut().find(|x| x.id == jid) {
            j.drive[1] = Some(0.0);
            j.limit_min[1] = Some(0.0);
            j.limit_max[1] = Some(8.0);
        }
        app.project.solve_joints();

        let ctx = app.current_ctx_id_for_test();
        let dir = app.project.joint_slot_axis(jid, 1, ctx).expect("the axis of travel");
        let m = app.project.joint_frame(jid, ctx).expect("the frame of the joint");
        let o = [m[3], m[7], m[11]];
        let rect = viewport();
        let basis = app.cam.basis();
        let l = 60.0 / app.cam.scale as f64;
        let s0 = app.project3(o, rect, &basis).0;
        let s1 = app.project3([o[0] + dir[0] * l, o[1] + dir[1] * l, o[2] + dir[2] * l], rect, &basis).0;
        let along = (s1 - s0).normalized();

        let before = apply12(&app.project.world_transform(part), [0.0, 0.0, 0.0]);
        app.joint_giz_begin(jid, 1, false);
        // drag FAR past the limit — a good hundred millimetres
        for _ in 0..8 {
            app.joint_giz_drag_to(s1, along * 60.0, rect, &basis);
        }
        app.project.solve_joints();
        let after = apply12(&app.project.world_transform(part), [0.0, 0.0, 0.0]);
        let d = [after[0] - before[0], after[1] - before[1], after[2] - before[2]];
        let travelled = d[0] * dir[0] + d[1] * dir[1] + d[2] * dir[2];
        assert!(
            (travelled - 8.0).abs() < 1e-3,
            "a limit of 8 mm: the drag must stop the part at the mark, and it travelled {travelled:.4} mm"
        );
    }

    /// PULL THE HANDLE ALONG THE DRAWN ARROW — THE PART GOES ALONG THAT SAME ARROW.
    #[test]
    fn dragging_the_handle_along_the_arrow_moves_the_part_along_the_arrow() {
        let mut app = App::default();
        let (jid, part) = slider_with_opposed_anchors(&mut app);
        app.project.solve_joints();

        let ctx = app.current_ctx_id_for_test();
        let dir = app.project.joint_slot_axis(jid, 1, ctx).expect("the axis of travel");
        let m = app.project.joint_frame(jid, ctx).expect("the frame of the joint");
        let o = [m[3], m[7], m[11]];

        // THE SCREEN DIRECTION OF THE ARROW — the one a person actually drags along.
        let rect = viewport();
        let basis = app.cam.basis();
        let l = 60.0 / app.cam.scale as f64;
        let s0 = app.project3(o, rect, &basis).0;
        let s1 = app.project3([o[0] + dir[0] * l, o[1] + dir[1] * l, o[2] + dir[2] * l], rect, &basis).0;
        let along = (s1 - s0).normalized();
        assert!(along.length() > 0.5, "setup: the arrow must be visible on screen, and its projection is degenerate");

        let before = apply12(&app.project.world_transform(part), [0.0, 0.0, 0.0]);
        app.joint_giz_begin(jid, 1, false);
        app.joint_giz_drag_to(s1, along * 40.0, rect, &basis);
        app.project.solve_joints();
        let after = apply12(&app.project.world_transform(part), [0.0, 0.0, 0.0]);

        let d = [after[0] - before[0], after[1] - before[1], after[2] - before[2]];
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        assert!(len > 1.0, "the part did not move at all: it travelled {len:.4} mm");
        let dot = (d[0] * dir[0] + d[1] * dir[1] + d[2] * dir[2]) / len;
        assert!(
            dot > 0.999,
            "the drag went ALONG the arrow and the part moved the other way: match {dot:.4} (travel {:?}, arrow {dir:?})",
            [d[0] / len, d[1] / len, d[2] / len]
        );
    }
}
