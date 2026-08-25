//! A test of the STL import.

use qymcad_io::import_stl;
use std::io::Write;

/// An ASCII STL: a 10×10 square at z = 2, of two faces.
const FIXTURE: &str = "\
solid s
facet normal 0 0 1
 outer loop
  vertex 0 0 2
  vertex 10 0 2
  vertex 10 10 2
 endloop
endfacet
facet normal 0 0 1
 outer loop
  vertex 0 0 2
  vertex 10 10 2
  vertex 0 10 2
 endloop
endfacet
endsolid s
";

#[test]
fn imports_ascii_stl_square() {
    let path = std::env::temp_dir().join("qymcad_test_square.stl");
    std::fs::File::create(&path).unwrap().write_all(FIXTURE.as_bytes()).unwrap();

    let mesh = import_stl(path.to_str().unwrap()).expect("import ok");
    assert_eq!(mesh.tris.len(), 2, "two faces");
    assert!(mesh.verts.len() >= 3);

    let b = mesh.bounds().expect("bounds");
    assert!((b.min.z - 2.0).abs() < 1e-6 && (b.max.z - 2.0).abs() < 1e-6, "the plane z = 2");
    assert!((b.max.x - 10.0).abs() < 1e-6 && (b.max.y - 10.0).abs() < 1e-6);
}
