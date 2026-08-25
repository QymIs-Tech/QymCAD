//! DELETE A FEATURE FROM THE TREE AND THE PART STAYS ON SCREEN.
//!
//! Reported behaviour: deleting a push-face feature makes the whole body vanish from the viewport
//! until Edit -> Rebuild everything is done — and that was true of ALL the modifying features, not of
//! one alone.
//!
//! The cause was not in the geometry but in the derived caches. `geom_rev`, the key the caches of
//! consumed bodies, bounding boxes, normals and edges live by, ticked ONLY inside a rebuild that
//! actually happened. Deleting a leaf modifier leaves not one dirty node: there is nothing to compute,
//! the scheduler honestly does nothing, and the counter stands still. Meanwhile the cache of consumed
//! bodies is left over from the previous frame, where the source body was still consumed by the
//! deleted feature — and so it was hidden as a "step of history". NOTHING was left on screen: the
//! result deleted, the source hidden.
//!
//! Checked on each of the new features: one cause — but let the regression be caught by any of
//! them.
#[cfg(test)]
mod tests {
    use super::super::App;
    use qymcad_core::feature::FaceKey;

    /// A part with a cube; returns (mesh index, body id).
    fn part_with_cube(app: &mut App) -> (usize, u64) {
        super::super::joint_flow::tests::add_part_at(app, 0.0);
        app.rebuild_if_dirty();
        let body = app.project.mesh_id(0).expect("the body");
        if let Some(owner) = app.project.body_owner(body) {
            app.enter_component(owner);
        }
        let mi = app.project.mesh_index(body).expect("the mesh");
        (mi, body)
    }

    /// The key of the top face of the body.
    fn top_face(app: &App, mi: usize) -> FaceKey {
        app.project.bodies[mi]
            .faces
            .iter()
            .filter(|f| f.normal[2] > 0.9)
            .max_by(|a, b| a.centroid.z.partial_cmp(&b.centroid.z).unwrap())
            .map(|f| FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id })
            .expect("the top face is there")
    }

    /// How many bodies are actually shown in the viewport.
    fn shown(app: &App) -> usize {
        (0..app.project.bodies.len()).filter(|&i| app.body_shown(i)).count()
    }

    /// Delete a timeline node by its Id — by exactly the path the feature tree uses.
    fn delete_node(app: &mut App, nid: u64) {
        let ti = app.project.timeline.iter().position(|n| n.id == nid).expect("the feature node in the timeline");
        app.delete_feature(ti);
    }

    /// PUSH FACE: deleted, and the source body stays on screen.
    #[test]
    fn deleting_push_face_leaves_the_source_body_on_screen() {
        let mut app = App::default();
        let (mi, body) = part_with_cube(&mut app);
        let face = top_face(&app, mi);
        let nb = app.project.add_push_face(body, face, 5.0);
        app.rebuild_if_dirty();
        assert_eq!(shown(&app), 1, "setup: exactly the result of the feature is visible");

        delete_node(&mut app, nb);
        assert_eq!(shown(&app), 1, "after the feature is deleted the source body must stay on screen rather than emptiness");
        assert!(app.project.mesh_index(body).is_some_and(|i| app.body_shown(i)), "what is visible must be the source body itself");
    }

    /// SPLIT BODY: the cut is deleted, the whole body comes back and the pieces go.
    #[test]
    fn deleting_split_body_brings_the_whole_body_back() {
        let mut app = App::default();
        let (mi, body) = part_with_cube(&mut app);
        let face = top_face(&app, mi);
        let h = app.project.bodies[mi].mesh.bounds().map(|b| b.max.z - b.min.z).expect("the height");
        app.start_feat_cmd(27);
        app.mode_3d = true;
        app.split.plane = Some(qymcad_core::feature::SketchPlane::Face(body, face));
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "offset") {
            p.val = -h * 0.5;
            p.txt = format!("{:.4}", -h * 0.5);
        }
        app.apply_feat_cmd();
        app.rebuild_if_dirty();
        let nid = app
            .project
            .timeline
            .iter()
            .find(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::SplitBody { .. }))
            .map(|n| n.id)
            .expect("the split feature");
        let pieces: Vec<u64> = app.project.timeline.iter().find(|n| n.id == nid).map(|n| n.kind.bodies()).expect("the pieces");
        assert_eq!(shown(&app), 2, "setup: both pieces are visible");

        delete_node(&mut app, nid);
        assert_eq!(shown(&app), 1, "after the split is deleted the WHOLE body must stay on screen");
        assert!(app.project.mesh_index(body).is_some_and(|i| app.body_shown(i)), "what is visible must be the source body");
        // the pieces went entirely rather than staying as ghosts with no recipe
        for p in pieces {
            assert!(app.project.mesh_index(p).is_none(), "piece {p} must disappear along with the split");
        }
    }

    /// REMOVE FACE: the feature is deleted and the source body is on screen.
    ///
    /// The healing on a body from the pipeline of the application does not always succeed (a known
    /// limitation), so what is checked is what DELETING A NODE is actually required to do: the screen
    /// does not stay empty.
    #[test]
    fn deleting_remove_face_leaves_the_source_body_on_screen() {
        let mut app = App::default();
        let (mi, body) = part_with_cube(&mut app);
        let face = top_face(&app, mi);
        let nb = app.project.add_remove_face(body, vec![face]);
        app.rebuild_if_dirty();

        delete_node(&mut app, nb);
        assert_eq!(shown(&app), 1, "after the feature is deleted the source body must stay on screen");
        assert!(app.project.mesh_index(body).is_some_and(|i| app.body_shown(i)), "what is visible must be the source body");
    }

    /// A CHANGE OF TOPOLOGY ALWAYS MOVES `geom_rev` — even when there is nothing to rebuild.
    ///
    /// That is the root of it: the derived caches (consumed bodies, bounding boxes, normals, edges)
    /// live by that counter, and it ticked only inside a rebuild that actually happened. Without this
    /// check the regression comes back quietly — the body simply stops being drawn.
    #[test]
    fn a_topology_change_always_invalidates_derived_caches() {
        let mut app = App::default();
        let (mi, body) = part_with_cube(&mut app);
        let face = top_face(&app, mi);
        let nb = app.project.add_push_face(body, face, 5.0);
        app.rebuild_if_dirty();
        assert!(app.project.timeline.iter().all(|n| !n.dirty), "setup: after a rebuild there are no dirty nodes");

        let rev = app.regen.geom_rev;
        delete_node(&mut app, nb);
        assert!(app.regen.geom_rev != rev, "deleting a node must invalidate the derived caches even if there was nothing to rebuild");
    }

    /// DELETING A DATUM GOES THROUGH THE EDIT BOUNDARY.
    ///
    /// The log showed "the document was changed outside App::edit" exactly on a split: the cutting
    /// plane was deleted, then the feature itself. Deleting a feature and deleting a sketch had a
    /// boundary, and deleting a datum did not: the change went past it, and the undo step came out as
    /// a nameless edit picked up after the fact.
    #[test]
    fn deleting_a_datum_plane_goes_through_the_edit_boundary() {
        use super::super::Sel;
        let mut app = App::default();
        let (mi, body) = part_with_cube(&mut app);
        let face = top_face(&app, mi);
        let pid = app.project.add_plane_from_face(body, face, -5.0);
        app.rebuild_if_dirty();
        app.edits.committed_key = app.doc_key(); // the point of reckoning: the document is "handed in"
        let undo_before = app.edits.undo.len();

        let i = app.project.planes.iter().position(|p| p.id == pid).expect("the plane in the list");
        app.sel = Sel::Plane(i);
        app.execute_delete(Sel::Plane(i));

        assert!(!app.doc_changed_outside_edit(), "deleting a datum must go through App::edit rather than past it");
        assert_eq!(app.edits.undo.len(), undo_before + 1, "EXACTLY one undo step must appear");
        assert_eq!(app.edits.undo.last().map(|s| s.name.clone()), Some(crate::i18n::tr("status-plane-delete")), "the step must be NAMED rather than picked up after the fact");
    }

}
