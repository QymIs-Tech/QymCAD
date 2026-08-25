//! A test of the SVG import.

use qymcad_io::import_svg;
use std::io::Write;

const FIXTURE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="80">
  <rect x="10" y="10" width="40" height="30"/>
  <circle cx="70" cy="40" r="15"/>
</svg>"#;

#[test]
fn imports_rect_and_circle() {
    use qymcad_core::feature::{BasePlane, SketchPlane};
    use qymcad_core::model::Project;
    let path = std::env::temp_dir().join("qymcad_test.svg");
    std::fs::File::create(&path).unwrap().write_all(FIXTURE.as_bytes()).unwrap();

    let sk = import_svg(path.to_str().unwrap()).expect("import ok");
    // SVG lowers into Beziers and then segments; `usvg` keeps no circle or arc primitives
    assert!(!sk.curves.is_empty(), "there are curves");

    // assemble into a sketch and check the closed contours by area: a rectangle of about 1200 and a circle of
    // about 707
    let mut p = Project::default();
    p.new_document();
    let si = p.import_sketch("t.svg", sk.curves, None, SketchPlane::World(BasePlane::XY));
    let areas: Vec<f64> = p.sketches[si].contour_ids.iter().filter_map(|cid| p.contour_index(*cid)).map(|ci| p.contours[ci].area()).collect();

    assert!(areas.iter().any(|a| (a - 1200.0).abs() < 5.0), "a rectangle of about 1200: {areas:?}");
    assert!(areas.iter().any(|a| (a - std::f64::consts::PI * 225.0).abs() < 30.0), "a circle of about 707: {areas:?}");
}
