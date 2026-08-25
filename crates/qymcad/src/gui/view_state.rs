//! THE VIEW BELONGS TO THE PERSON, NOT TO THE TOOL.
//!
//! Reported behaviour: leaving a tool with Esc resets the viewport into some half-sketcher or flat
//! view. The tools that need a flat view (picking a contour for a sweep, picking an axis of
//! revolution) switch it on the way in and ask for a REFIT on the way out — that is, the camera a
//! person set up by hand is thrown away. Cancelling an action must not change the point of view.
#[cfg(test)]
mod tests {
    use super::super::{App, ContourSlot};

    /// Esc out of the half-sketcher must bring back the view as it was BEFORE entering.
    #[test]
    fn escaping_a_flat_pick_restores_the_camera_it_found() {
        let mut app = App::default();
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, 0.0, 0.0, 20.0, 20.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        app.rebuild_if_dirty();

        // the person turned and zoomed the view by hand
        app.mode_3d = true;
        app.cam.yaw = 1.234;
        app.cam.pitch = 0.321;
        app.cam.scale = 7.5;
        app.cam.target = [11.0, 22.0, 33.0];
        app.cam.init = true;
        app.view.initialized = true;
        let want = (app.mode_3d, app.cam.yaw, app.cam.pitch, app.cam.scale, app.cam.target, app.view.initialized);

        // the tool leads into the flat half-sketcher — that is fine, it needs a flat view
        let sid = app.project.sketches[si].id;
        app.begin_contour_pick(ContourSlot::SweepProfile, sid);
        assert!(!app.mode_3d, "setup: the half-sketcher must be flat");

        // Esc cancels the ACTION; the point of view must come back to the person's own
        app.on_escape();
        let got = (app.mode_3d, app.cam.yaw, app.cam.pitch, app.cam.scale, app.cam.target, app.view.initialized);
        assert_eq!(
            got, want,
            "Esc must bring back the view as the person left it: it was {want:?}, it became {got:?} — \
             cancelling an action has no right to change the camera and demand a refit"
        );
    }

    /// Esc OUT OF ANY COMMAND leaves the view alone — the reported scenario exactly.
    ///
    /// Open the edit of a finished operation with a double click, press Esc — and the viewport is
    /// broken into some two-dimensional projection. Pick a new operation — the same thing. And so with
    /// everything: chamfer, fillet.
    #[test]
    fn escaping_any_command_leaves_the_view_alone() {
        let mut app = App::default();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        app.rebuild_if_dirty();
        let body = app.project.mesh_id(0).expect("the body");
        if let Some(owner) = app.project.body_owner(body) {
            app.enter_component(owner);
        }
        let si = app.project.sketches.iter().position(|s| !s.entities.is_empty()).expect("a sketch");
        let mi = app.project.mesh_index(body).expect("the mesh");

        // the person looks at the model in 3D from an angle of their own
        let setup = |app: &mut App| {
            app.mode_3d = true;
            app.cam.yaw = 0.9;
            app.cam.pitch = 0.4;
            app.cam.scale = 6.0;
            app.cam.init = true;
            app.view.initialized = true;
        };

        // NEW commands: extrude, cut (the same one with a different operation), fillet, chamfer
        for (kind, sel) in [(1u8, true), (4, false), (5, false)] {
            setup(&mut app);
            app.sel = if sel { super::super::Sel::Sketch(si) } else { super::super::Sel::Mesh(mi) };
            app.start_feat_cmd(kind);
            app.on_escape();
            assert!(app.mode_3d, "command {kind}: Esc threw the view into a flat one");
            assert!(app.view.initialized, "command {kind}: Esc demanded a refit — the person's camera will be gone");
        }

        // EDITING an existing feature with a double click
        let fid = app.project.timeline.iter().rev().find_map(|n| n.kind.body().map(|_| n.id)).expect("the feature is there");
        setup(&mut app);
        app.start_feat_cmd_edit(fid);
        app.on_escape();
        assert!(app.mode_3d, "editing a feature: Esc threw the view into a flat one");
        assert!(app.view.initialized, "editing a feature: Esc demanded a refit — the person's camera will be gone");
    }

    /// APPLYING a command leaves the view alone too.
    ///
    /// Esc was checked and Enter was not, and that is exactly the half that made this item need
    /// closing twice. Finishing a command went by the same path, with a refit.
    #[test]
    fn applying_a_command_leaves_the_view_alone() {
        let mut app = App::default();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        app.rebuild_if_dirty();
        let body = app.project.mesh_id(0).expect("the body");
        if let Some(owner) = app.project.body_owner(body) {
            app.enter_component(owner);
        }
        let mi = app.project.mesh_index(body).expect("the mesh");

        app.mode_3d = true;
        app.cam.yaw = 0.77;
        app.cam.scale = 5.5;
        app.cam.init = true;
        app.view.initialized = true;
        let want = (app.mode_3d, app.cam.yaw, app.cam.scale, app.view.initialized);

        app.sel = super::super::Sel::Mesh(mi);
        app.start_feat_cmd(4); // fillet
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "radius") {
            p.val = 2.0;
            p.txt = "2".into();
        }
        app.gsel.edges = app.body_edges_cached(body).map(|e| e.1.iter().copied().filter(|&i| i != 0).collect()).unwrap_or_default();
        app.apply_feat_cmd();
        app.rebuild_if_dirty();

        let got = (app.mode_3d, app.cam.yaw, app.cam.scale, app.view.initialized);
        assert_eq!(got, want, "Enter must leave the view as it is: it was {want:?}, it became {got:?}");
    }

}
