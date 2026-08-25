//! CLICK A PART AND ITS FACE IS TAKEN, NOT A "MISS".
//!
//! Reported behaviour: proper mates could not be assembled at all. Measured on the reported machine:
//! the cursor stands on the gantry, the program's own picking finds a body there ("Rama 19",
//! 18 faces, a live B-rep, the body visible) — and it says "Miss, click a FACE of the part". The
//! joint is not created at all.
//!
//! The cause is the same for every assembly tool: picking an anchor needs a FACE, while the picker
//! returns the nearer of two things — a face of the part OR a datum plane. A datum plane is an
//! invisible square at the origin the size of a third of the scene; in a large assembly it stands
//! between the camera and the parts and takes the click for itself. The tool sees "not a face" and
//! declares a miss.
//!
//! Checked here is what a person sees: a click on a part takes ITS face. So the check does not go
//! green for nothing, it has a TRAP GUARD: first make sure the plane under this cursor really does
//! intercept the picking — otherwise there would be nothing to check.
#[cfg(test)]
mod tests {
    use super::super::App;
    use qymcad_core::feature::{apply12, JointKind, SketchPlane};

    fn viewport() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0))
    }

    /// An assembly arranged as in the reported case: the part stands FAR from the origin, and the
    /// datum plane ends up between the camera and the part.
    fn a_part_far_from_the_origin(app: &mut App) -> (u64, [f64; 3]) {
        // TWO PARTS: one at the origin, the second carried far away. That makes the scene large —
        // and the datum plane grows with the scene (a third of its extent) and covers half the
        // machine.
        super::super::joint_flow::tests::add_part_at(app, 0.0);
        super::super::joint_flow::tests::add_part_at(app, 40.0);
        for _ in 0..4 {
            if app.current_ctx_id_for_test() == app.project.root {
                break;
            }
            app.exit_context_for_test();
        }
        let comp = *app.project.components.iter().filter(|c| c.parent == Some(app.project.root)).map(|c| c.id).collect::<Vec<_>>().last().expect("the part");
        // CARRY IT PAST THE ORIGIN ALONG THE CAMERA RAY: then the ray from the eye to the part
        // passes through the square of the datum plane — exactly the arrangement of the reported
        // machine.
        app.mode_3d = true;
        let v = app.cam.basis().2;
        let d = 800.0;
        if let Some(i) = app.project.component_index(comp) {
            app.project.components[i].transform = [1.0, 0.0, 0.0, v[0] * d - 10.0, 0.0, 1.0, 0.0, v[1] * d - 10.0, 0.0, 0.0, 1.0, v[2] * d - 5.0];
        }
        app.rebuild_if_dirty();
        app.refresh_edges();

        // THE TOP FACE OF THIS PART — that is what we aim at, as a person aims at a rail.
        let body = app.project.component_bodies(comp).first().copied().expect("the body of the part");
        let wt = app.project.body_world_transform(body);
        let faces = app.project.regen_faces.get(&body).expect("the faces of the body");
        let top = faces
            .iter()
            .map(|f| (apply12(&wt, [f.centroid.x, f.centroid.y, f.centroid.z]), qymcad_core::feature::apply12_dir(&wt, f.normal)))
            .filter(|(_, n)| n[2] > 0.9)
            .max_by(|a, b| a.0[2].total_cmp(&b.0[2]))
            .expect("the top face")
            .0;
        (body, top)
    }

    /// Aim the camera at the whole scene — as the program does when opening a file.
    fn look_at_everything(app: &mut App) {
        app.mode_3d = true;
        let v = app.cam.basis().2;
        app.cam.target = [v[0] * 400.0, v[1] * 400.0, v[2] * 400.0];
        app.cam.scale = 0.35;
        app.cam.init = true;
    }

    #[test]
    fn clicking_a_part_gives_its_face_not_a_miss() {
        let mut app = App::default();
        let (body, top) = a_part_far_from_the_origin(&mut app);
        look_at_everything(&mut app);
        let basis = app.cam.basis();
        let at = app.project3(top, viewport(), &basis).0;

        // TRAP GUARD: there is a body under this cursor, and the picker returns the datum plane.
        let hit_body = app.pick_body_at(viewport(), at).and_then(|mi| app.project.mesh_id(mi));
        assert_eq!(hit_body, Some(body), "setup: there must be a part under the cursor, and there is {hit_body:?}");
        assert!(
            matches!(app.pick_sketch_plane_at(viewport(), at), Some(SketchPlane::World(_))),
            "GUARD: there is no trap — the datum plane does not intercept this click, so there is nothing to check"
        );

        // AND NOW BY HAND: take the joint and click the part.
        app.workbench = super::super::Workbench::Assembly;
        app.joint.new_kind = JointKind::Slider;
        app.arm_joint_pick_for_test();
        app.viewport_3d_click_at(at, viewport(), &basis);

        let picked = app.joint_pick_first_anchor_for_test();
        assert!(
            matches!(picked, Some(qymcad_core::feature::AnchorRef::FaceCenter(b, _)) if b == body),
            "a part was clicked — ITS face must be taken, and what was taken is {picked:?}; status: {}",
            app.status
        );
    }
}
