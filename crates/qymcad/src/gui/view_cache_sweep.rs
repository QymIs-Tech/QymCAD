//! THE PICTURE MUST CHANGE WHEN WHAT IS DRAWN ON IT CHANGES.
//!
//! The viewport is cached: the raster (and the GPU buffer) are rebuilt only if the KEY changed. The
//! key is a contract — "here is what the picture depends on". Everything that affects the picture and
//! did not make it into the key produces one and the same class of complaint: the model is intact and
//! the screen shows something else.
//!
//! Reported behaviour on a freshly opened project: enter the edit of a fillet, leave with Esc — the
//! fillet is gone and the chamfer under it too, and the features are not failing, they are simply not
//! drawn. The model was intact the whole time. Editing a modifier HIDES its result and the whole
//! descendant chain (the state BEFORE the feature is shown) — and that was not part of the key: on
//! the way in the raster was redrawn by a heavy rebuild that happened to flip `geom_rev`, and on the
//! way out there was nothing left to change the key, so the frame from edit mode stayed on screen.
//!
//! So the guard checks not "is such-and-such a field in the key" but a PROPERTY: if the set of drawn
//! bodies changed, the key must change. Both paths, the CPU raster and the GPU scene.
#[cfg(test)]
mod tests {
    use super::super::{App, Sel};

    fn rect() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0))
    }

    /// What is actually drawn right now — the list of body ids.
    fn drawn(app: &App) -> Vec<u64> {
        app.visible_mesh_items().iter().filter_map(|(mi, ..)| app.project.mesh_id(*mi)).collect()
    }

    /// A part: a plate with a fillet on it. Returns (application, id of the fillet node).
    fn plate_with_fillet() -> (App, u64) {
        let mut app = App::default();
        let part = app.project.add_part("part");
        app.enter_component(part);
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, 0.0, 0.0, 60.0, 40.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        app.sel = Sel::Sketch(si);
        app.start_feat_cmd(1);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 12.0;
            p.txt = "12".into();
        }
        app.apply_feat_cmd();
        app.rebuild_if_dirty();

        let body = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the body");
        app.select_body(body);
        app.start_feat_cmd(4); // fillet
        app.refresh_edges();
        let edge = app.edges.ids.iter().copied().find(|e| *e != 0).expect("an edge");
        app.gsel.edges.insert(edge);
        app.apply_feat_cmd();
        app.rebuild_if_dirty();

        let fillet = app
            .project
            .timeline
            .iter()
            .find(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::Fillet { .. }))
            .map(|n| n.id)
            .expect("the fillet in the timeline");
        (app, fillet)
    }

    /// ENTERING AN EDIT AND LEAVING IT CHANGE THE CACHE KEY — BOTH DRAWING PATHS.
    #[test]
    fn entering_and_leaving_a_feature_edit_invalidates_the_viewport() {
        let (mut app, fillet) = plate_with_fillet();
        app.mode_3d = true;
        app.cam.init = true;
        app.cam.scale = 8.0;
        let r = rect();

        let before = (drawn(&app), app.view_key_pub(r, 1.0), app.gpu_scene_key_pub());

        app.start_feat_cmd_edit(fillet);
        let editing = (drawn(&app), app.view_key_pub(r, 1.0), app.gpu_scene_key_pub());
        assert_ne!(before.0, editing.0, "setup: an edit must change the set of drawn bodies");
        assert_ne!(before.1, editing.1, "the set of drawn bodies changed — the RASTER key must change, otherwise the previous frame stays on screen");
        assert_ne!(before.2, editing.2, "the same for the GPU scene");

        app.cancel_all_tools(); // Esc
        let after = (drawn(&app), app.view_key_pub(r, 1.0), app.gpu_scene_key_pub());
        assert_eq!(before.0, after.0, "setup: leaving the edit brings back the previous set of bodies");
        assert_ne!(editing.1, after.1, "the edit was left — the RASTER key must change, otherwise the fillet never gets drawn");
        assert_ne!(editing.2, after.2, "the same for the GPU scene");
        assert_eq!(before.1, after.1, "and come back to the previous one: the picture is the same as it was before the edit");
        assert_eq!(before.2, after.2, "the same for the GPU scene");
    }
}
