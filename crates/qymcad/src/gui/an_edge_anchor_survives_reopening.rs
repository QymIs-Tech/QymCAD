//! AN EDGE ANCHOR STAYS ALIVE AFTER THE FILE IS OPENED.
//!
//! A carriage is put on a rail with an EDGE anchor: then the travel runs ALONG the rail rather than
//! along the normal of a face. On the reported machine that ended like this: the hand takes the tool,
//! switches the anchor to "edge", clicks the edge of the rail and the edge of the carriage — the joint
//! is created and IMMEDIATELY declared faulty ("the anchor is lost"), its travel 0.000 mm, no axis of
//! travel at all.
//!
//! The cause is TWO DIFFERENT SOURCES OF EDGES. The click lands on an edge because the picking takes
//! them from the LIVE B-rep (`body_edges_cached` -> `shape.edges_with_ids()`). But `connector_frame`
//! resolves the anchor through the `regen_edges` OF THE MODEL, and that is filled only by the post
//! pass of a rebuild. Opening a file does not rebuild: a bundle stores meshes and faces, not edges.
//! Measured on the reported machine: 138 bodies, faces on all 138, EDGES ON TWO, a live B-rep on all
//! 138.
//!
//! Checked here is the whole path as a person walks it: build, save, open again, place a joint on an
//! edge — and the joint must be sound.
#[cfg(test)]
mod tests {
    use super::super::App;
    use qymcad_core::feature::JointKind;
    use qymcad_core::model::Id;

    fn viewport() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0))
    }

    /// Two parts written into a bundle and opened AGAIN — as a person does the next day.
    fn two_parts_reopened(app: &mut App) -> Vec<Id> {
        let mut maker = App::default();
        super::super::joint_flow::tests::add_part_at(&mut maker, 0.0);
        super::super::joint_flow::tests::add_part_at(&mut maker, 40.0);
        for _ in 0..4 {
            if qymcad_ui_state::current_ctx_id(&maker.active_path, &maker.project) == maker.project.root {
                break;
            }
            maker.exit_context();
        }
        qymcad_ui_state::rebuild_if_dirty(&mut maker.rebuild_ctx());
        let path = std::env::temp_dir().join("qym-edge-anchor-reopen.qcad").to_string_lossy().into_owned();
        qymcad_io::save_project(&maker.project, &path).expect("the document was written");

        let project = qymcad_io::load_project(&path).expect("the document opened");
        app.finish_project_load(path, project, Vec::new());
        app.viewing.mode_3d = true;
        crate::gui::io_jobs::ensure_brep(&mut app.rebuild_ctx());
        app.drain_bg_for_test();
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        crate::gui::commands::refresh_edges(&mut app.part_ctx());
        // THAT IS EXACTLY WHAT AN OPENED DOCUMENT LOOKS LIKE: `regen_edges` is a DERIVED field, it is
        // not written into the bundle (see doc_file.rs) and is filled only by the post pass of a
        // rebuild. It is cleared here so that the check stands in exactly the state a person opens a
        // file in.
        app.project.regen_edges.clear();
        app.project.components.iter().filter(|c| c.parent == Some(app.project.root)).map(|c| c.id).collect()
    }

    /// The midpoint of the longest edge of the part — what a person actually aims at.
    fn an_edge_of(app: &App, comp: Id) -> [f64; 3] {
        let mut best: Option<(f64, [f64; 3])> = None;
        for b in app.project.component_bodies(comp) {
            let Some(cached) = crate::gui::pick::body_edges_cached(&app.cache, &app.live, &app.regen, b) else { continue };
            let wt = app.project.body_world_transform(b);
            for (poly, id) in cached.polys.iter().zip(cached.ids.iter().copied()) {
                if id == 0 || poly.len() < 2 {
                    continue;
                }
                let p = |q: &[f32; 3]| qymcad_core::feature::apply12(&wt, [q[0] as f64, q[1] as f64, q[2] as f64]);
                let (u, v) = (p(&poly[0]), p(&poly[poly.len() - 1]));
                let d = [v[0] - u[0], v[1] - u[1], v[2] - u[2]];
                let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                if best.is_none_or(|(bl, _)| len > bl) {
                    best = Some((len, [(u[0] + v[0]) / 2.0, (u[1] + v[1]) / 2.0, (u[2] + v[2]) / 2.0]));
                }
            }
        }
        best.expect("the part has edges in the live B-rep").1
    }

    #[test]
    fn a_mate_made_on_an_edge_is_not_born_broken() {
        let mut app = App::default();
        let all = two_parts_reopened(&mut app);
        // A CLEAN DOCUMENT MAY HOLD A STOCK PART — take the two LAST parts, our own.
        assert!(all.len() >= 2, "setup: there should be two parts of our own, and the root holds {}", all.len());
        let parts: Vec<Id> = all[all.len() - 2..].to_vec();

        // TRAP GUARD: an opened document has fewer edge sets IN THE MODEL than bodies — otherwise
        // there is nothing to check and a green answer means nothing.
        assert!(
            app.project.regen_edges.len() < app.project.bodies.len(),
            "GUARD: there is no trap — every body has edges in the model ({} of {}), the check is meaningless",
            app.project.regen_edges.len(),
            app.project.bodies.len()
        );

        app.workbench = super::super::Workbench::Assembly;
        app.side.joint.new_kind = JointKind::Slider;
        app.arm_joint_pick_for_test();
        qymcad_assembly::set_joint_anchor_mode_for_test(&mut app.joint_ctx(), 1); // "anchor: edge" — the switch in the assembling bar
        app.viewing.cam.target = [30.0, 10.0, 5.0];
        app.viewing.cam.scale = 9.0;
        app.viewing.cam.init = true;
        let basis = app.viewing.cam.basis();
        for c in &parts {
            let at = qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect: viewport(), basis: &basis }.at(an_edge_of(&app, *c)).0;
            crate::gui::commands::refresh_edges(&mut app.part_ctx());
            app.viewport_3d_click_at(at, viewport(), &basis);
        }

        let made = app.project.joints.last().map(|j| j.id).expect("a joint on edges must be created");
        let faults = app.project.joint_faults();
        assert!(
            !faults.iter().any(|(id, _)| *id == made),
            "a joint on an edge was born FAULTY: {faults:?}; status: {}",
            app.status
        );
        assert!(
            app.project.joint_slot_axis(made, 1, qymcad_ui_state::current_ctx_id(&app.active_path, &app.project)).is_some(),
            "a sound joint must have an axis of travel, and there is none"
        );

        // AND IT SURVIVES CLOSING. That is exactly where the first fix was caught out: joints on
        // edges worked in the session they were made in and died on the next opening — "the anchor is
        // lost", travel 0.000. The fix was half a fix: the two sources of edges were reconciled at the
        // moment an anchor is PLACED and not at the moment a document is OPENED.
        let path = std::env::temp_dir().join("qym-edge-anchor-roundtrip.qcad").to_string_lossy().into_owned();
        qymcad_io::save_project(&app.project, &path).expect("the document was written");
        let project = qymcad_io::load_project(&path).expect("the document opened");
        let mut again = App::default();
        again.finish_project_load(path, project, Vec::new());
        again.viewing.mode_3d = true;
        crate::gui::io_jobs::ensure_brep(&mut again.rebuild_ctx());
        again.drain_bg_for_test();
        qymcad_ui_state::rebuild_if_dirty(&mut again.rebuild_ctx());
        let faults = again.project.joint_faults();
        assert!(
            !faults.iter().any(|(id, _)| *id == made),
            "a joint on an edge DID NOT SURVIVE closing the document: {faults:?}"
        );
        // and it must MOVE rather than merely count as sound
        let owner = again.project.connector(again.project.joints.iter().find(|x| x.id == made).map(|x| x.b).expect("the joint")).map(|c| c.owner).expect("the owner of the driven part");
        let was = again.project.world_transform(owner);
        if let Some(j) = again.project.joints.iter_mut().find(|x| x.id == made) {
            j.drive[1] = Some(j.offset + 10.0);
        }
        again.project.solve_joints();
        let now = again.project.world_transform(owner);
        let d = was.iter().zip(now.iter()).map(|(x, y)| (x - y).abs()).fold(0.0f64, f64::max);
        assert!(d > 1e-6, "after reopening the joint does not move the part at all: shift {d:.6}");
    }
}
