//! AN IMPORTED IGES PART GETS ITS LIVE BODY BACK when the document is opened from its embedded source.
//!
//! The live bodies of imports are raised again from the original file kept inside the document. That was
//! written for STEP and read every source as STEP whatever its extension - an IGES part came back with its mesh
//! and no live body, so nothing exact could be built on it and it fell out of a STEP export.
use qymcad_core::feature::PLACE_IDENTITY;
use qymcad_core::model::Project;
use qymcad_kernel::{import_iges, write_iges, LengthUnit};

#[test]
fn an_iges_part_gets_its_live_body_back() {
    let dir = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/iges-reopen"));
    std::fs::create_dir_all(&dir).expect("a folder for the check");
    let path = dir.join("cube.igs");
    let cube = qymcad_kernel::specimen::extrude(&[0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0], 10.0).expect("a cube");
    write_iges(&[(&cube, PLACE_IDENTITY)], &path.to_string_lossy(), LengthUnit::Millimetre).expect("the IGES is written");

    let mut p = Project::default();
    p.new_document();
    let source = p.add_source("cube.igs", std::fs::read(&path).expect("the file reads"));
    let qymcad_core::geom::Built { mesh, .. } = import_iges(&path.to_string_lossy(), 0.5).expect("the IGES imports").remove(0);
    let body = p.add_mesh(mesh);
    p.import_tree_as_parts(vec![qymcad_core::model::ImportNode { name: "cube".into(), body: Some(body), ..Default::default() }], source, "cube");

    let shapes = qymcad_testkit::restore_import_shapes(&p);
    let live = shapes.get(&body).unwrap_or_else(|| panic!("the IGES part came back with no live body ({} raised)", shapes.len()));
    assert!((live.volume() - 1000.0).abs() < 5.0, "the live body raised is not the cube: {} mm^3", live.volume());
}
