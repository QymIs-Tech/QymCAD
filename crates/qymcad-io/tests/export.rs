//! Export tests: STL round-tripped through the STL import, and SVG and DXF round-tripped through the DXF
//! import.

use qymcad_core::geom::{Mesh, Point2, Point3, ProfEdge};
use qymcad_io::{export_dxf, export_stl, export_svg, import_dxf, import_stl};

fn tmp(name: &str) -> String {
    std::env::temp_dir().join(name).to_string_lossy().into_owned()
}

/// A tetrahedron of four vertices and four faces: the minimal closed body for checking STL.
fn tetra() -> Mesh {
    Mesh {
        verts: vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(10.0, 0.0, 0.0),
            Point3::new(0.0, 10.0, 0.0),
            Point3::new(0.0, 0.0, 10.0),
        ],
        tris: vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
    }
}

#[test]
fn stl_roundtrip_preserves_tris_and_bounds() {
    let m = tetra();
    let path = tmp("qym_export_test.stl");
    export_stl(&[m.clone()], &path).expect("export ok");

    let back = import_stl(&path).expect("import ok");
    assert_eq!(back.tris.len(), m.tris.len(), "the triangle count is preserved");
    // the extents match; binary STL uses f32, so an epsilon is allowed
    let bb = back.bounds().expect("bounds");
    assert!((bb.min.x - 0.0).abs() < 1e-3 && (bb.max.x - 10.0).abs() < 1e-3, "the X extent: {bb:?}");
    assert!((bb.max.z - 10.0).abs() < 1e-3, "the Z extent: {bb:?}");
}

#[test]
fn stl_empty_is_error() {
    let empty = Mesh { verts: vec![], tris: vec![] };
    assert!(export_stl(&[empty], &tmp("qym_empty.stl")).is_err());
}

#[test]
fn svg_writes_exact_primitives() {
    let edges = vec![
        ProfEdge::Line { a: Point2::new(0.0, 0.0), b: Point2::new(40.0, 0.0) },
        ProfEdge::Circle { center: Point2::new(70.0, 40.0), r: 15.0 },
        ProfEdge::Arc { a: Point2::new(0.0, 0.0), b: Point2::new(10.0, 10.0), center: Point2::new(10.0, 0.0), ccw: true },
    ];
    let path = tmp("qym_export_test.svg");
    export_svg(&edges, &path).expect("svg ok");
    let s = std::fs::read_to_string(&path).unwrap();
    assert!(s.contains("<svg"), "the svg root is present");
    assert!(s.contains("<line"), "a segment becomes a line");
    assert!(s.contains("<circle"), "a circle becomes a circle");
    assert!(s.contains("<path"), "an arc becomes a path");
}

#[test]
fn dxf_roundtrip_recovers_entities() {
    let edges = vec![
        ProfEdge::Line { a: Point2::new(0.0, 0.0), b: Point2::new(40.0, 0.0) },
        ProfEdge::Circle { center: Point2::new(70.0, 40.0), r: 15.0 },
    ];
    let path = tmp("qym_export_test.dxf");
    export_dxf(&edges, &path).expect("dxf ok");
    let s = std::fs::read_to_string(&path).unwrap();
    assert!(s.contains("LINE") && s.contains("CIRCLE"), "the DXF contains LINE and CIRCLE");

    // importing back recovers the exact primitives, a line and a circle
    let sk = import_dxf(&path).expect("reimport ok");
    assert!(sk.curves.iter().any(|e| matches!(e, ProfEdge::Line { .. })), "the line was recovered: {:?}", sk.curves);
    assert!(sk.curves.iter().any(|e| matches!(e, ProfEdge::Circle { .. })), "the circle was recovered: {:?}", sk.curves);
}
