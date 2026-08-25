//! OPENING A FILE IS NOT OBLIGED TO BUILD EVERYTHING AGAIN.
//!
//! Reported behaviour: make a cut in one part and sit waiting a couple of minutes for the rebuild.
//! Measurement found the culprit: the edit itself in a built project costs a second (2 nodes out of
//! 28), whereas the FIRST operation after opening a file rebuilds the whole timeline — 13.4 s on the
//! reported file. The cause is not the scheduler: the bundle held meshes and faces, that is the
//! geometry of DISPLAY, while nobody had a live body, and `ensure_brep` honestly marked the whole
//! document dirty.
//!
//! Now the live body travels into the file next to the mesh. Checked here is that the save-and-open
//! round trip puts it back and there is nothing left to rebuild.
#[cfg(test)]
mod tests {
    use super::super::App;

    /// A part with a body, saved into a file of its own. Returns the path.
    fn saved_part(dir: &std::path::Path) -> (App, String) {
        let mut app = App::default();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        app.regenerate_now();
        assert!(!app.live.shapes.is_empty(), "setup: a built part must have a live body");
        let path = dir.join("brep-keep.qcad").to_string_lossy().into_owned();
        app.set_project_path(path.clone());
        app.save_project_for_test();
        app.wait_bg_for_test();
        (app, path)
    }

    /// THE LIVE BODY LIES IN THE FILE AND IS RAISED ON OPENING.
    #[test]
    fn opening_a_file_brings_the_live_body_back() {
        let dir = std::env::temp_dir().join(format!("qym-brep-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("the temporary directory");
        let (app, path) = saved_part(&dir);
        let want: Vec<_> = {
            let mut v: Vec<_> = app.live.shapes.keys().copied().collect();
            v.sort_unstable();
            v
        };

        let (project, breps) = qymcad_io::load_project_with_brep(&path).expect("the file reads");
        assert!(!breps.is_empty(), "the bundle holds NO live bodies — opening will pay for a full rebuild again");
        let shapes: Vec<_> = breps.into_iter().filter_map(|(id, b)| qymcad_kernel::Shape::from_brep_bytes(&b).map(|s| (id, s))).collect();
        let got: Vec<_> = {
            let mut v: Vec<_> = shapes.iter().map(|(id, _)| *id).collect();
            v.sort_unstable();
            v
        };
        assert_eq!(want, got, "the bodies raised are not the ones that were saved");

        let mut fresh = App::default();
        fresh.finish_project_load(path.clone(), project, shapes);
        assert!(!fresh.live.shapes.is_empty(), "after opening the cache of live bodies is empty");

        // THE POINT: there is nothing to rebuild. `ensure_brep` marks dirty exactly those nodes whose
        // bodies have no live B-rep — so not one of them must be left.
        fresh.ensure_brep_for_test();
        let dirty: Vec<String> = fresh.project.timeline.iter().filter(|n| n.dirty).map(|n| n.name.clone()).collect();
        assert!(dirty.is_empty(), "opening demands a rebuild of nodes {dirty:?} again — the live body from the file was not picked up");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// THE BLOB OF A LIVE BODY IS NOT RECOMPUTED ON EVERY SAVE.
///
/// Measured on a real file: serialising all the bodies costs about 0.6 s. Without a cache every
/// autosave (once every three minutes) would pay it right on the UI thread — that is, it would bring
/// back the very freeze this work is getting away from. The cache is dropped for exactly those bodies
/// that were rebuilt.
#[cfg(test)]
mod blob_cache {
    use super::super::App;

    #[test]
    fn a_rebuilt_body_loses_its_cached_blob_and_an_untouched_one_keeps_it() {
        let mut app = App::default();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        super::super::joint_flow::tests::add_part_at(&mut app, 100.0);
        app.regenerate_now();
        let bodies: Vec<_> = app.live.shapes.keys().copied().collect();
        assert_eq!(bodies.len(), 2, "setup: two bodies, and it came out {}", bodies.len());

        // the first save fills the cache
        let dir = std::env::temp_dir().join(format!("qym-blob-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("the temporary directory");
        app.set_project_path(dir.join("blobs.qcad").to_string_lossy().into_owned());
        app.save_project_for_test();
        app.wait_bg_for_test();
        assert_eq!(app.live.blobs.len(), 2, "after the write the blobs must lie in the cache");

        // rebuild ONE body
        let victim = app.project.timeline.iter().find(|n| n.kind.body() == Some(bodies[0])).map(|n| n.id).expect("the node of the body");
        app.project.mark_node_dirty(victim);
        app.regenerate_now();
        assert!(!app.live.blobs.contains_key(&bodies[0]), "a rebuilt body must lose its stale blob");
        assert!(app.live.blobs.contains_key(&bodies[1]), "an untouched body is not obliged to recompute its blob on every save");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
