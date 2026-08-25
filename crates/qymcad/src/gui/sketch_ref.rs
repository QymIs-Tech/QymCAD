//! A SKETCH ON A FACE SHOWS THE CURRENT BODY, NOT A HISTORY STEP.
//!
//! Reported behaviour: extrude a square, make a hole, take the chamfers off — and the sketcher shows
//! them layered on top of each other, both the original square and the chamfered body. The cause is
//! that `SketchPlane::Face(body, _)` holds the body id AS OF THE MOMENT the sketch was created, while
//! every following operation creates a new body and consumes the previous one. The reference stays on
//! the consumed one, and geometry that no longer exists in the model travels to the screen.
#[cfg(test)]
mod tests {
    use super::super::{App, Sel};

    /// The outline of the face under the sketch must come from the LIVE body.
    ///
    /// The check is built to tell the bodies apart by shape: the top face of the original box is four
    /// straight edges, and after rounding the vertical edges there are more of them (straight ones
    /// plus arcs). If the projection takes the consumed body, there will be exactly four polylines.
    #[test]
    fn the_sketch_outline_comes_from_the_live_body_not_a_history_step() {
        let mut app = App::default();
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, 0.0, 0.0, 40.0, 40.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        app.sel = Sel::Sketch(si);
        app.start_feat_cmd(1);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 10.0;
            p.txt = "10".into();
        }
        app.apply_feat_cmd();
        app.rebuild_if_dirty();

        // a sketch on the TOP face of the box (normal +Z)
        let body1 = app.project.mesh_id(0).expect("the body is there");
        let mi = app.project.mesh_index(body1).expect("the mesh is there");
        let (fi, face) = app.project.bodies[mi]
            .faces
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.normal[2].partial_cmp(&b.1.normal[2]).unwrap())
            .map(|(i, f)| (i, f.clone()))
            .expect("the faces are there");
        let key = qymcad_core::feature::FaceKey {
            index: fi as u32,
            centroid: [face.centroid.x, face.centroid.y, face.centroid.z],
            normal: face.normal,
            id: face.id,
        };
        let sk = app.create_sketch_on(qymcad_core::feature::SketchPlane::Face(body1, key));
        app.finish_sketch_edit();
        app.rebuild_if_dirty();
        // THE BODIES ARE TOLD APART BY THE SPAN OF THE OUTLINE rather than by the number of
        // polylines: the top face of the rounded box is also four straight edges, only SMALLER
        // (40 - 2*r), while the arcs belong to the fillet faces by then. The polyline count is the
        // same here and cannot serve as a discriminator.
        let span = |app: &App, sk: usize| -> f64 {
            let polys = app.sketch_ref_edges_2d(sk);
            let (mut lo, mut hi) = (f64::MAX, f64::MIN);
            for p in &polys {
                for v in p {
                    lo = lo.min(v.x);
                    hi = hi.max(v.x);
                }
            }
            hi - lo
        };
        let before = span(&app, sk);
        assert!((before - 40.0).abs() < 1e-6, "setup: the outline of the original face is 40 mm, and it came out {before:.3}");

        // NOW CHANGE THE BODY: rounding all the edges -> a new body, the old one consumed.
        // IMPORTANT: the operation must run in the CONTEXT of the owning part, otherwise isolation
        // rejects the input as a cross-component reference and the body is not built at all. That is
        // a rule of the application, not an obstacle to the test.
        if let Some(owner) = app.project.body_owner(body1) {
            app.enter_component(owner);
        }
        app.sel = Sel::Mesh(mi);
        app.start_feat_cmd(4); // fillet
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "radius") {
            p.val = 4.0;
            p.txt = "4".into();
        }
        app.gsel.edges = app.body_edges_cached(body1).map(|e| e.1.iter().copied().filter(|&i| i != 0).collect()).unwrap_or_default();
        app.apply_feat_cmd();
        app.rebuild_if_dirty();

        let live = app.project.mesh_id(app.project.bodies.len() - 1).expect("the new body");
        assert_ne!(live, body1, "setup: the fillet must create a NEW body");
        assert!(app.project.consumed_bodies().contains(&body1), "setup: the previous body must be consumed");

        let after = span(&app, sk);
        assert!(
            (after - 32.0).abs() < 1e-3,
            "the sketch must show the outline of the LIVE body: after a fillet of r=4 the top face became 32 mm, \
             and the projection gives {after:.3} mm — so on screen is the geometry of a history step that no longer exists in the model"
        );
    }
}
