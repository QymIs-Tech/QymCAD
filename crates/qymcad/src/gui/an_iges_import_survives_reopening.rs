//! AN IMPORTED IGES PART GETS ITS LIVE BODY BACK on the program's own opening path.
//!
//! The same fault as in `qymcad-testkit`'s copy of this path, which is why both are checked: the embedded source
//! was read as STEP whatever it was, and an IGES part opened with a mesh and no live body.
#[cfg(test)]
mod tests {
    use qymcad_core::feature::PLACE_IDENTITY;
    use qymcad_core::model::Project;

    #[test]
    fn an_iges_part_gets_its_live_body_back() {
        let dir = std::path::PathBuf::from(format!("{}/../../target/iges-reopen-app", env!("CARGO_MANIFEST_DIR")));
        std::fs::create_dir_all(&dir).expect("a folder for the check");
        let path = dir.join("cube.igs");
        let cube = qymcad_kernel::specimen::extrude(&[0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0], 10.0).expect("a cube");
        qymcad_kernel::write_iges(&[(&cube, PLACE_IDENTITY)], &path.to_string_lossy(), qymcad_kernel::LengthUnit::Millimetre).expect("written");
        let mut p = Project::default();
        p.new_document();
        let source = p.add_source("cube.igs", std::fs::read(&path).expect("reads"));
        let qymcad_core::geom::Built { mesh, .. } = qymcad_kernel::import_iges(&path.to_string_lossy(), 0.5).expect("imports").remove(0);
        let body = p.add_mesh(mesh);
        p.import_tree_as_parts(vec![qymcad_core::model::ImportNode { name: "cube".into(), body: Some(body), ..Default::default() }], source, "cube");
        let raised = crate::gui::restore_import_shapes_for(&p);
        assert!(raised.iter().any(|(id, s)| *id == body && (s.volume() - 1000.0).abs() < 5.0), "the IGES part came back with no live body ({} raised)", raised.len());
    }
}
