//! THE GEOMETRY TOLERANCE TRAVELS WITH THE DOCUMENT.
//!
//! The tessellation deflection used to be a constant in `qymcad-kernel`. Both the picture on screen and
//! what goes into an STL depend on it — so make it a setting of THE PROGRAM and one and the same file
//! would give two people a DIFFERENT export. Each of them would be certain the program was lying, and
//! there would be nothing to find it with: both files are identical, it is the machines that differ.
//!
//! So the tolerance is a property of THE DOCUMENT. Jewellery needs one, a machine frame another, and
//! it is decided by whoever drew it, not by whoever opened it.
#[cfg(test)]
mod tests {
    use super::super::App;
    use qymcad_core::model::GeomQuality;

    fn plate_with(q: GeomQuality) -> App {
        let mut app = App::default();
        app.project.geom_quality = q;
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        // A CIRCLE rather than a rectangle: the tessellation of flat faces does not depend on the
        // deflection at all, and a test on them would pass with the setting entirely ignored.
        app.project.add_circle_entity(si, 0.0, 0.0, 20.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        app.sel = super::super::Sel::Sketch(si);
        app.start_feat_cmd(1);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 10.0;
            p.txt = "10".into();
        }
        app.apply_feat_cmd();
        app.rebuild_if_dirty();
        app
    }

    fn tris(app: &App) -> usize {
        app.project.bodies.iter().map(|b| b.mesh.tris.len()).sum()
    }

    /// THE POINT: the tolerance of the document really does reach the tessellation.
    ///
    /// Checked BY THE NUMBER OF TRIANGLES rather than by the field having been stored: a setting that
    /// was recorded and never applied is the worst kind of working code — it looks like it works and
    /// does nothing.
    #[test]
    fn the_document_accuracy_reaches_the_tessellation() {
        let draft = tris(&plate_with(GeomQuality::Draft));
        let normal = tris(&plate_with(GeomQuality::Normal));
        let fine = tris(&plate_with(GeomQuality::Fine));
        assert!(draft > 0 && normal > 0 && fine > 0, "the cylinder did not build: {draft}/{normal}/{fine}");
        assert!(draft < normal, "the draft setting must give FEWER triangles: {draft} against {normal}");
        assert!(fine > normal, "the fine setting must give MORE triangles: {fine} against {normal}");
    }

    /// ONE FILE MEANS ONE GEOMETRY, on whosever machine it is opened.
    ///
    /// Exactly the trouble for which the tolerance moved into the document: the program settings of
    /// two people always differ, and had the tolerance lived there the export would drift silently.
    #[test]
    fn the_same_file_gives_the_same_geometry_on_any_machine() {
        let dir = std::env::temp_dir().join("qym_geom_quality_test");
        std::fs::create_dir_all(&dir).expect("the directory for the check");
        let path = dir.join("quality.qcad").to_string_lossy().into_owned();
        let _ = std::fs::remove_file(&path);

        let mut author = plate_with(GeomQuality::Fine);
        author.set_project_path(path.clone());
        author.save_project_for_test();
        author.wait_bg_for_test();
        let want = tris(&author);

        // "another machine": the program settings differ in everything unrelated to the geometry
        let mut other = App::default();
        other.set.language = "en".into();
        other.set.scheme = "light".into();
        other.set.viewcube_size = 0;
        other.open_for_test(path.clone());
        other.drain_busy_for_test();
        assert_eq!(other.project.geom_quality, GeomQuality::Fine, "the tolerance did not come with the file — so it stayed a property of the machine");

        // rebuild ENTIRELY, as "rebuild everything" does: the geometry from the bundle is no hint here
        for n in other.project.timeline.iter_mut() {
            if n.kind.body().is_some() {
                n.dirty = true;
            }
        }
        other.rebuild_if_dirty_for_test();
        other.drain_busy_for_test();
        assert_eq!(tris(&other), want, "the same file gave DIFFERENT geometry: {} against {want}", tris(&other));
        let _ = std::fs::remove_file(&path);
    }

    /// "NORMAL" IS WHAT THE PROGRAM LIVED BY BEFORE THE SETTING EXISTED.
    ///
    /// The appearance of a choice has no right to silently change the look of projects already drawn:
    /// a person opens an old file and must see it as it was, not "slightly different".
    #[test]
    fn normal_is_exactly_the_old_behaviour() {
        assert!((GeomQuality::Normal.deflection_k() - 0.0015).abs() < 1e-12, "the factory deflection fraction changed — old projects will start to look different");
        assert_eq!(GeomQuality::default(), GeomQuality::Normal, "a document with no tolerance stated must read as the normal one");
    }

    /// A FILE WITHOUT THE TOLERANCE (saved before the setting existed) reads as the normal one rather
    /// than crashing.
    ///
    /// A REAL document is built and exactly one line is cut out of it: a minimal RON typed by hand
    /// would check somebody's idea of the format rather than the format.
    #[test]
    fn a_file_without_the_field_still_opens() {
        let mut p = qymcad_core::model::Project::default();
        p.geom_quality = GeomQuality::Fine;
        let text = qymcad_core::model::to_ron(&p).expect("the document writes");
        assert!(text.contains("geom_quality"), "setup: the tolerance field must be in the file");
        let older: String = text.lines().filter(|l| !l.contains("geom_quality")).collect::<Vec<_>>().join("\n");
        let back = qymcad_core::model::from_ron(&older).expect("a document WITHOUT the tolerance field must read");
        assert_eq!(back.geom_quality, GeomQuality::Normal, "a missing tolerance must take the factory value");
    }

    /// AND THE DOCUMENT PROPERTIES TRAVEL WITH THE FILE TOO.
    ///
    /// Found along the way and it turned out worse: `meta` (author, title, version, comment) was NOT
    /// in the file schema at all. The field was added to the model, and the format is a separate type;
    /// a person filled the properties in, saved, and lost them silently. The neighbouring tests
    /// checked the creation date and whether it leaked into the program settings, but not whether it
    /// survives the file itself.
    #[test]
    fn the_document_properties_survive_the_file() {
        let mut p = qymcad_core::model::Project::default();
        p.meta.title = "Bracket".into();
        p.meta.author = "Denis".into();
        p.meta.version = "rev. B".into();
        p.meta.comment = "a check".into();
        let back = qymcad_core::model::from_ron(&qymcad_core::model::to_ron(&p).expect("it writes")).expect("it reads");
        assert_eq!(back.meta, p.meta, "the document properties did not survive the file — they get filled in and lost silently");
    }
}
