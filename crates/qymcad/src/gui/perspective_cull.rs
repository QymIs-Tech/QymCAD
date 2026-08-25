//! A BODY DOES NOT FALL APART IN PERSPECTIVE. Written from two reported screenshots.
//!
//! Reported behaviour: changing the perspective angle breaks the bodies — holes appear in them, thin
//! faces fall apart, gaps show through — and a ring in the screenshot broke into separate ribbons.
//! Both on the CPU and on the GPU.
//!
//! The two paths had one thing in common: back-face culling was computed ORTHOGONALLY,
//! `dot(n, fwd) >= 0`, under a perspective projection. In an orthographic view the ray of sight is one
//! for the whole frame, and `fwd` is that ray. In perspective the ray is different at every point, and
//! near the edges of the frame it diverges from `fwd` the more, the wider the field of view. The error
//! went BOTH ways: visible faces were culled (gaps in the body) and invisible ones were kept (the ring
//! in ribbons).
//!
//! The defect had been there before — perspective has been in the program for a long time. It is just
//! that with a hard-coded angle of 35.5 degrees the divergence was small, and it only became visible
//! once the angle became a setting.
#[cfg(test)]
mod tests {
    use super::super::App;

    /// A cylinder is where this shows: its side surface runs all the way round, and the faces near
    /// the edge of the frame look at a large angle to `fwd`. On a cube standing face-on to the camera
    /// the error would hardly appear at all.
    fn cylinder_app() -> App {
        let mut app = App::default();
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_circle_entity(si, 0.0, 0.0, 30.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        app.sel = super::super::Sel::Sketch(si);
        app.start_feat_cmd(1);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 80.0;
            p.txt = "80".into();
        }
        app.apply_feat_cmd();
        app.rebuild_if_dirty();
        app.mode_3d = true;
        app.cam.init = true;
        app
    }

    /// The triangles about which THE OLD rule and the eye say DIFFERENT things.
    ///
    /// "Sees" here is not a restatement of the new formula but a definition: a face is turned towards
    /// the eye if the eye lies on its front side. The old rule asked not about the eye but about the
    /// general direction of the camera, and so in perspective it answered wrongly. Those very
    /// triangles were the holes.
    fn wrongly_culled(app: &App, inv_d: f64) -> usize {
        use super::super::{v_cross, v_dot, v_norm, v_sub};
        let (_, _, fwd) = app.cam.basis();
        let d_eye = 1.0 / inv_d;
        let eye = [app.cam.target[0] - fwd[0] * d_eye, app.cam.target[1] - fwd[1] * d_eye, app.cam.target[2] - fwd[2] * d_eye];
        let mut n = 0;
        for b in &app.project.bodies {
            for tri in &b.mesh.tris {
                let p = |i: usize| {
                    let v = b.mesh.verts[tri[i] as usize];
                    [v.x, v.y, v.z]
                };
                let (a, b2, c) = (p(0), p(1), p(2));
                let nrm = v_norm(v_cross(v_sub(b2, a), v_sub(c, a)));
                let eye_sees = v_dot(nrm, v_sub(a, eye)) < 0.0; // the definition: the eye is on the front side
                let old_kept = v_dot(nrm, fwd) < 0.0; // how it used to be culled
                if eye_sees != old_kept {
                    n += 1; // a divergence EITHER way: both a lost face and a superfluous kept one
                }
            }
        }
        n
    }

    /// THE POINT: the old rule diverges from "does the eye see it", and the more so the wider the
    /// field of view. The divergence is counted BOTH ways — that is exactly the reported picture: a
    /// lost face gives a gap in the body, a superfluous kept one gives a ring broken into ribbons. The
    /// new rule IS "does the eye see it", so what is checked is not the rule itself (that would be
    /// circular) but that the ray is computed FROM THE EYE.
    #[test]
    fn the_old_rule_disagrees_with_the_eye_and_worse_the_wider_the_angle() {
        let mut app = cylinder_app();
        app.set.cam_perspective = true;
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1200.0, 800.0));

        let mut lost = Vec::new();
        for fov in [20.0, 35.5, 60.0, 80.0] {
            app.set.persp_fov_deg = fov;
            let inv_d = app.persp_inv_d_for_test(rect.height() * 0.5);
            assert!(inv_d > 0.0, "setup: the perspective is on");
            lost.push((fov, wrongly_culled(&app, inv_d)));
        }
        let widest = lost.last().expect("the list is not empty").1;
        assert!(widest > 0, "at 80 deg the old rule diverged from the eye nowhere — then the reported screenshot has some other explanation");
        assert!(lost[0].1 < widest, "the divergence must grow with the field of view: {lost:?}");

        // and the new rule does not lose them BY CONSTRUCTION: it is "does the eye see it"
        for (fov, _) in &lost {
            app.set.persp_fov_deg = *fov;
            let inv_d = app.persp_inv_d_for_test(rect.height() * 0.5);
            let (_, _, fwd) = app.cam.basis();
            let d_eye = 1.0 / inv_d;
            let eye = [app.cam.target[0] - fwd[0] * d_eye, app.cam.target[1] - fwd[1] * d_eye, app.cam.target[2] - fwd[2] * d_eye];
            let probe = [30.0, 0.0, 40.0];
            let want = super::super::v_norm(super::super::v_sub(probe, eye));
            let got = app.view_dir_at_for_test(probe, fwd, inv_d);
            for k in 0..3 {
                assert!((got[k] - want[k]).abs() < 1e-9, "angle {fov} deg: the ray of sight was computed not from the eye: {got:?} against {want:?}");
            }
        }
    }

    /// AND THE ORTHOGRAPHIC VIEW IS UNTOUCHED: there the ray is one per frame, and it is still `fwd`.
    #[test]
    fn the_orthographic_view_is_untouched() {
        let app = cylinder_app();
        let (_, _, fwd) = app.cam.basis();
        let p = [12.0, -7.0, 33.0];
        assert_eq!(app.view_dir_at_for_test(p, fwd, 0.0), fwd, "in an orthographic view the direction of sight must stay as it was");
    }

    /// THE RAY REALLY DOES DIVERGE FROM `fwd` — otherwise the check above would mean nothing.
    #[test]
    fn off_centre_the_ray_really_differs_from_the_camera_axis() {
        let app = cylinder_app();
        let (_, _, fwd) = app.cam.basis();
        let inv_d = 1.0 / 150.0; // the eye is close, so the angle is wide
        let far_off = [90.0, 0.0, 0.0]; // a point off to the side of the centre of the frame
        let v = app.view_dir_at_for_test(far_off, fwd, inv_d);
        let same = super::super::v_dot(v, fwd);
        assert!(same < 0.98, "the ray to a point near the edge of the frame almost coincided with the camera axis ({same:.3}) — the check proves nothing");
    }

    /// AND BOTH RENDERERS ASK ONE FUNCTION. Should one of them go back to `dot(n, fwd)`, the picture
    /// will diverge between the CPU and the GPU again — and only in perspective, and only near the
    /// edges.
    #[test]
    fn both_renderers_cull_by_the_eye_ray() {
        let render = crate::gui::render_source::RENDER;
        let gpu = include_str!("../viewport_gpu.rs");
        assert_eq!(render.matches("view_dir_at(").count(), 2, "the raster must cull by the ray from the eye in both places (bodies and face fill)");
        // TOGETHER WITH THE CONDITION, not only the line that computes it: the first edition of the
        // guard checked for the presence of `let eye = ...` and passed calmly when the branch was
        // stubbed out with `if (false)`. The guard must see that the computation is SWITCHED ON in
        // perspective, not merely present in the file.
        let want = "if (inv_d > 0.0) {\n        let eye = cam.tgt.xyz - cam.fwd.xyz * (1.0 / inv_d);\n        view = normalize(in.wpos - eye);";
        assert!(gpu.contains(want), "the shader stopped culling by the ray from the eye in perspective — the picture will diverge from the raster again");
        assert!(gpu.contains("if (dot(in.nrm, view) >= 0.0) { discard; }"), "the shader culls by the general camera direction again");
    }
}
