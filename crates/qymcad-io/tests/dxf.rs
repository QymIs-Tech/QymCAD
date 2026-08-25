//! An integration test of the DXF import.

use qymcad_io::import_dxf;
use std::io::Write;

/// The fixture: a closed 10×10 polyline square, a circle of r = 5, and two lines forming an open chain from
/// (0,0) through (5,0) to (5,5).
const FIXTURE: &str = "\
0
SECTION
2
ENTITIES
0
LWPOLYLINE
8
0
90
4
70
1
10
0.0
20
0.0
10
10.0
20
0.0
10
10.0
20
10.0
10
0.0
20
10.0
0
CIRCLE
8
0
10
20.0
20
20.0
40
5.0
0
LINE
8
0
10
0.0
20
0.0
11
5.0
21
0.0
0
LINE
8
0
10
5.0
20
0.0
11
5.0
21
5.0
0
ENDSEC
0
EOF
";

fn write_fixture() -> std::path::PathBuf {
    let path = std::env::temp_dir().join("qymcad_dxf_test.dxf");
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(FIXTURE.as_bytes()).unwrap();
    path
}

#[test]
fn imports_exact_curves_square_circle_lines() {
    use qymcad_core::geom::ProfEdge;
    let path = write_fixture();
    let sketch = import_dxf(path.to_str().unwrap()).expect("import ok");

    // Exact curves: the polyline square gives four segments and is closed, the circle gives one primitive, and
    // the two lines give two segments. Six segments plus one circle, rather than a tessellation into hundreds
    // of segments.
    let circles: Vec<f64> = sketch.curves.iter().filter_map(|e| if let ProfEdge::Circle { r, .. } = e { Some(*r) } else { None }).collect();
    assert_eq!(circles.len(), 1, "the circle stayed a circle primitive");
    assert!((circles[0] - 5.0).abs() < 1e-9, "the radius of the circle is 5");

    let lines = sketch.curves.iter().filter(|e| matches!(e, ProfEdge::Line { .. })).count();
    assert_eq!(lines, 6, "the square gives four and the two separate lines give two, so six segments");

    assert_eq!(sketch.curves.len(), 7, "seven exact curves in total, with no tessellation of arcs or circles");
}
