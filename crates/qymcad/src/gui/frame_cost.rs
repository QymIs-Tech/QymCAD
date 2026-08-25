//! THE COST OF A FRAME: WHAT IS DONE ON EVERY DRAW DOES NOT GO INTO THE KERNEL.
//!
//! Reported behaviour: picking an anchor — choose anything other than a face and the whole CAD hangs.
//! The cause turned out to be not the logic but the cost: picking an edge or a vertex pulled the edges
//! of ALL the bodies out of the live B-rep on EVERY movement of the mouse. On two little cubes that
//! goes unnoticed; on an assembly of a thousand components the application stops responding. Here it
//! is measured by time rather than by eye.
//!
//! The first measurement blamed the wrong thing: the bodies in it were not separate parts. A part is
//! ONE body, and every operation consumes the previous one, so "120 bodies" turned out to be one part
//! with 120 steps of history — and that uncovered the real cause. The consumed bodies counted as
//! visible: they are out of sight only because the final body covers them, yet they were walked over
//! on a par with the live ones. Hence TWO checks here: the cost grows neither with the length of the
//! history of a part nor with the number of parts away from the cursor.
#[cfg(test)]
mod tests {
    use super::super::{App, Sel};

    use super::super::joint_flow::tests::add_part_at;


    fn per_frame_ms(app: &App, rect: egui::Rect, pos: egui::Pos2) -> f64 {
        let _ = app.pick_edge_any(rect, pos); // the first pick fills the cache; every frame runs the ones after it
        let t = std::time::Instant::now();
        for _ in 0..30 {
            let _ = app.pick_edge_any(rect, pos);
        }
        t.elapsed().as_secs_f64() * 1000.0 / 30.0
    }

    /// An assembly of many PARTS: picking an edge must fit into a frame, not into seconds.
    #[test]
    fn picking_an_edge_stays_within_a_frame_budget() {
        let mut app = App::default();
        for i in 0..40 {
            add_part_at(&mut app, i as f64 * 30.0);
        }
        let root = app.project.root;
        app.enter_component(root);
        app.rebuild_if_dirty();
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
        let pos = egui::pos2(400.0, 300.0);

        // THE MEASUREMENT MUST MEASURE SOMETHING: an empty loop "fits into a frame" and quietly
        // declares the picking sound.
        assert_eq!(app.shown_bodies().len(), 40, "all 40 parts must be visible in the root assembly");
        assert!(app.pick_edge_any(rect, pos).is_some(), "there must be an edge under the cursor — otherwise the measurement measures emptiness");
        let per_frame = per_frame_ms(&app, rect, pos);
        eprintln!("[edge pick] {} parts, {per_frame:.2} ms per frame", app.project.bodies.len());
        assert!(per_frame < 16.0, "picking an edge must fit into a frame (16 ms) and it takes {per_frame:.2} ms — that is the hang");

        // SCALE is what all this was for: three times as many parts FURTHER from the cursor. They are
        // not under the cursor, so they must not cost anything. If the time grows with the number of
        // parts, the application will hang again on a real assembly (1182 components).
        for i in 0..80 {
            add_part_at(&mut app, 5000.0 + i as f64 * 30.0);
        }
        app.enter_component(app.project.root);
        app.rebuild_if_dirty();
        assert_eq!(app.shown_bodies().len(), 120, "after the addition all 120 parts must be visible");
        assert!(app.pick_edge_any(rect, pos).is_some(), "the edge under the cursor has not gone anywhere");
        let per_frame2 = per_frame_ms(&app, rect, pos);
        eprintln!("[edge pick] {} parts, {per_frame2:.2} ms per frame", app.project.bodies.len());
        assert!(
            per_frame2 < per_frame * 2.0 + 2.0,
            "three times as many parts AWAY from the cursor must not treble the cost of a pick: it was {per_frame:.2} ms, it became {per_frame2:.2} ms"
        );
    }

    /// THE HISTORY OF A PART MUST COST NOTHING WHEN PICKING.
    ///
    /// Twenty operations on a part are twenty consumed bodies of the same shape. If they get into the
    /// walk, a pick grows dearer along with the history: the longer you work, the worse it hangs.
    #[test]
    fn consumed_history_bodies_are_not_picked() {
        let mut app = App::default();
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, 0.0, 0.0, 20.0, 20.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        for _ in 0..12 {
            app.sel = Sel::Sketch(si);
            app.start_feat_cmd(1);
            if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
                p.val = 10.0;
                p.txt = "10".into();
            }
            app.apply_feat_cmd();
        }
        app.rebuild_if_dirty();
        let shown = app.shown_bodies().len();
        assert!(app.project.bodies.len() > shown, "the timeline must accumulate consumed bodies — otherwise the check proves nothing");
        assert_eq!(shown, 1, "a part is one body: only the last one must stay visible, not the whole history ({} bodies in the timeline)", app.project.bodies.len());
    }

    /// THE DRAWING CODE DOES NOT GO INTO THE KERNEL FOR EDGES.
    ///
    /// The highlight of a joint anchor and the projection of the face edges under a sketch are drawn
    /// EVERY FRAME — including while the mouse stands still. A direct call to `edges_with_ids()` from
    /// there means a full extraction of the edges from the B-rep every frame; on a real part that is
    /// what produced "placed a mate, waited, and the CAD stopped responding". There is one door to the
    /// edges — `body_edges_cached` — and it caches by the geometry revision.
    #[test]
    fn drawing_code_takes_edges_only_from_the_cache() {
        for (name, src) in [("render.rs", crate::gui::render_source::RENDER), ("sketching.rs", include_str!("sketching.rs"))] {
            assert!(
                !src.contains("edges_with_ids()"),
                "{name} draws every frame and must take the edges through body_edges_cached rather than pulling the kernel directly"
            );
        }
    }

    /// The projection of the face edges under a sketch is computed on every frame of an edit — it
    /// must be cheap.
    #[test]
    fn sketch_face_edges_are_cheap_per_frame() {
        let mut app = App::default();
        add_part_at(&mut app, 0.0);
        app.rebuild_if_dirty();
        // a sketch ON A FACE of a built body — the very case where the face edges are projected every frame
        let body = app.project.mesh_id(0).expect("the body is built");
        let mi = app.project.mesh_index(body).expect("the mesh index");
        let (fi, face) = app.project.bodies[mi].faces.iter().enumerate().next().map(|(i, f)| (i, f.clone())).expect("the body has faces");
        let key = qymcad_core::feature::FaceKey {
            index: fi as u32,
            centroid: [face.centroid.x, face.centroid.y, face.centroid.z],
            normal: face.normal,
            id: face.id,
        };
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::Face(body, key));
        app.finish_sketch_edit();
        let warm = app.sketch_ref_edges_2d(si); // warm up the cache
        assert!(!warm.is_empty(), "the face edges must project — otherwise the measurement measures an empty result");
        let t = std::time::Instant::now();
        for _ in 0..60 {
            let _ = app.sketch_ref_edges_2d(si);
        }
        let per_frame = t.elapsed().as_secs_f64() * 1000.0 / 60.0;
        eprintln!("[face edges under a sketch] {per_frame:.3} ms per frame");
        assert!(per_frame < 2.0, "the projection of the face edges runs every frame of a sketch edit: {per_frame:.2} ms — no kernel work may be put here");
    }
}
