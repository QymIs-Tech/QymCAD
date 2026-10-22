//! A CURVE OF AN SVG COMES IN AS A SPLINE: the Bezier curves of a path are the splines they are, those going on smoothly
//! one into the next one spline - a circle, four Beziers in the file, one closed spline of four nodes, of the area of the
//! circle - and a corner between two curves stays a corner, two splines meeting there.
//!
//! Reported behaviour: a curve of an SVG came in as 12 or 16 segments a Bezier, a drawing of curves as thousands of lines
//! a person could not drag as curves, and an extrusion of it a face a segment.
use qymcad_core::feature::{BasePlane, SketchPlane};
use qymcad_core::model::Project;
use qymcad_io::import_svg;

/// The kinds of the curves of a sketch laid from the SVG `svg`: lines, and the nodes of each spline; the areas of its
/// closed regions.
struct Laid {
    lines: usize,
    splines: Vec<usize>,
    areas: Vec<f64>,
}

/// AN SVG TO READ: its text, and the name of the file it is written to.
struct Sheet {
    svg: &'static str,
    name: &'static str,
}

fn laid(sheet: Sheet) -> Laid {
    let Sheet { svg, name } = sheet;
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, svg).expect("the SVG is written");
    let sk = import_svg(path.to_str().expect("a path")).expect("the SVG is read");
    let mut p = Project::default();
    p.new_document();
    let si = p.import_sketch(name, sk.curves, None, SketchPlane::World(BasePlane::XY));
    let s = &p.sketches[si];
    Laid {
        lines: s.entities.iter().filter(|e| e.kind.as_line().is_some()).count(),
        splines: s.entities.iter().filter_map(|e| e.kind.as_spline().map(|sp| sp.nodes.len())).collect(),
        areas: s.contour_ids.iter().filter_map(|c| p.contour_index(*c)).map(|i| p.contours[i].area()).collect(),
    }
}

#[test]
fn a_circle_of_an_svg_is_one_closed_spline() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="100mm" viewBox="0 0 100 100"><circle cx="50" cy="50" r="15" fill="none" stroke="black"/></svg>"#;
    let got = laid(Sheet { svg, name: "qymcad_svg_circle.svg" });
    let want = std::f64::consts::PI * 225.0;
    assert!(
        got.lines == 0 && got.splines == [4] && got.areas.iter().any(|a| (a - want).abs() < 0.5),
        "a circle of r 15 came in as {} lines and splines of {:?} nodes, regions of {:?} mm^2, not one spline of 4 nodes of {want:.1}",
        got.lines,
        got.splines,
        got.areas
    );
}

#[test]
fn a_corner_between_two_curves_of_an_svg_stays_a_corner() {
    // two arches meeting at (40, 50) at a corner, closed by a line along the bottom
    let svg =
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="100mm" viewBox="0 0 100 100"><path d="M 0 50 C 0 20 40 20 40 50 C 40 20 80 20 80 50 Z" fill="none" stroke="black"/></svg>"#;
    let got = laid(Sheet { svg, name: "qymcad_svg_corner.svg" });
    assert!(got.lines == 1 && got.splines.len() == 2, "two arches meeting at a corner came in as {} lines and splines of {:?} nodes, not a line and two splines", got.lines, got.splines);
}
