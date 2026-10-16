//! A BODY BROUGHT IN FROM A FILE KEEPS THE NAMES OF ITS FACES across saving and opening the project. On opening, its
//! live body is raised again from the file it came in from, without a rebuild (1170 imported solids took 31 s to
//! rebuild), and a body read from a file holds positional numbers - while the document lists the names the faces were
//! given when the body came in. A fillet on an edge of it, laid before saving, found nothing after opening.
#[cfg(test)]
mod tests {
    use qymcad_core::feature::PLACE_IDENTITY;
    use qymcad_core::model::Project;

    /// The face ids a live body holds, sorted.
    fn held(s: &qymcad_kernel::Shape) -> Vec<u32> {
        let mut ids: Vec<u32> = qymcad_kernel::specimen::face_places(s).iter().map(|f| f.id).collect();
        ids.sort_unstable();
        ids
    }

    #[test]
    fn an_imported_body_takes_back_its_names_on_opening() {
        let dir = std::path::PathBuf::from(format!("{}/../../target/import-names-app", env!("CARGO_MANIFEST_DIR")));
        std::fs::create_dir_all(&dir).expect("a folder for the check");
        let step = dir.join("cube.step");
        let cube = qymcad_kernel::specimen::extrude(&[0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0], 10.0).expect("a cube");
        qymcad_kernel::write_step(&[(&cube, PLACE_IDENTITY)], &step.to_string_lossy()).expect("written");
        // brought in as the program brings a file in: its meshes laid as parts, its live body raised from the file and
        // the import rebuilt on it
        let mut p = Project::default();
        p.new_document();
        let source = p.add_source("cube.step", std::fs::read(&step).expect("reads"));
        let qymcad_core::geom::Built { mesh, .. } = qymcad_kernel::import_step(&step.to_string_lossy(), 0.5).expect("imports").remove(0);
        let body = p.add_mesh(mesh);
        p.import_tree_as_parts(vec![qymcad_core::model::ImportNode { name: "cube".into(), body: Some(body), ..Default::default() }], source, "cube").expect("came in");
        let raised = crate::gui::restore_import_shapes_for(&p);
        let kernel = qymcad_kernel::OcctKernel { shapes: std::cell::RefCell::new(raised.into_iter().collect()), ..Default::default() };
        {
            let _gate = qymcad_kernel::kernel_gate();
            p.regenerate(&kernel);
        }
        let mut listed: Vec<u32> = p.regen_faces.get(&body).map(|f| f.iter().map(|x| x.id).collect()).unwrap_or_default();
        listed.sort_unstable();
        assert_eq!(listed.len(), 6, "the cube came in with {} faces", listed.len());
        assert!(listed.iter().all(|d| qymcad_core::names::NameTable::is_named(*d)), "the faces of the body brought in are not named: {listed:x?}");
        // saved and opened: the body raised again holds the names the document lists
        let path = dir.join("cube.qcad").to_string_lossy().into_owned();
        qymcad_io::save_project(&p, &path).expect("saved");
        let q = qymcad_io::load_project(&path).expect("opened");
        let again = crate::gui::restore_import_shapes_for(&q);
        let shape = again.iter().find(|(id, _)| *id == body).map(|(_, s)| s).expect("the body raised again");
        assert_eq!(held(shape), listed, "the body raised on opening holds other ids than the names the document lists");
    }
}
