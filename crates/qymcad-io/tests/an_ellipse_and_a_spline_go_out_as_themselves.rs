//! AN ELLIPSE AND A SPLINE GO OUT AND COME BACK AS THEMSELVES: written to DXF they are an ELLIPSE and a SPLINE of the file,
//! and read back they are the curves they were, to 0.01 mm - not runs of segments; laid into a sketch, the ellipse is an
//! ellipse and the spline one spline of its nodes. Written to SVG, an ellipse is an elliptical arc of the path.
//!
//! Reported behaviour: DXF wrote an ellipse and a spline as 48 LINEs each and read them back as segments, and SVG wrote an
//! ellipse as a polyline.
use qymcad_core::feature::SketchPlane;
use qymcad_core::geom::{Point2, ProfEdge};
use qymcad_core::model::curve::Curve;
use qymcad_core::model::Project;
use qymcad_io::{export_dxf, export_svg, import_dxf};

fn tmp(name: &str) -> String {
    std::env::temp_dir().join(name).to_string_lossy().into_owned()
}

fn p2(at: [f64; 2]) -> Point2 {
    Point2::new(at[0], at[1])
}

/// A whole ellipse 12 by 5 turned by 30 deg, an arc of an ellipse whose minor axis runs the other way round and whose
/// way is clockwise, and a spline through three nodes.
fn curves() -> Vec<ProfEdge> {
    let t = 30f64.to_radians();
    let spline = Curve::spline(vec![p2([0.0, 30.0]), p2([12.0, 30.0]), p2([24.0, 33.0])], &[], qymcad_core::feature::Ends::Open);
    let mut out = vec![
        ProfEdge::Oval { centre: p2([10.0, 5.0]), major: p2([12.0 * t.cos(), 12.0 * t.sin()]), minor: p2([-5.0 * t.sin(), 5.0 * t.cos()]), from: 0.0, sweep: std::f64::consts::TAU },
        ProfEdge::Oval { centre: p2([40.0, 0.0]), major: p2([3.0, 0.0]), minor: p2([0.0, -7.0]), from: 0.4, sweep: -2.0 },
    ];
    out.extend(spline.to_edges());
    out
}

/// How far the curve `got` strays from `want` at most, over 200 places of each looked up on the other.
fn apart([want, got]: [&Curve; 2]) -> f64 {
    let one = (0..=200).map(|k| got.nearest(want.at(k as f64 / 200.0)).off).fold(0.0, f64::max);
    let other = (0..=200).map(|k| want.nearest(got.at(k as f64 / 200.0)).off).fold(0.0, f64::max);
    one.max(other)
}

#[test]
fn an_ellipse_and_a_spline_written_to_dxf_come_back_as_themselves() {
    let path = tmp("qym_ellipse_spline.dxf");
    export_dxf(&curves(), &path).expect("the DXF is written");
    let text = std::fs::read_to_string(&path).expect("the DXF is read");
    let entity = |name: &str| text.lines().collect::<Vec<_>>().windows(2).filter(|w| w[0].trim() == "0" && w[1].trim() == name).count();
    let mut failures = Vec::new();
    if (entity("ELLIPSE"), entity("SPLINE"), entity("LINE")) != (2, 1, 0) {
        failures.push(format!("the file holds {} ELLIPSE, {} SPLINE and {} LINE, not 2, 1 and 0", entity("ELLIPSE"), entity("SPLINE"), entity("LINE")));
    }
    let back = import_dxf(&path).expect("the DXF is read back").curves;
    let ovals: Vec<&ProfEdge> = back.iter().filter(|e| matches!(e, ProfEdge::Oval { .. })).collect();
    let spans: Vec<&ProfEdge> = back.iter().filter(|e| matches!(e, ProfEdge::Spline(_))).collect();
    if ovals.len() != 2 || spans.len() != 1 || back.len() != 3 {
        failures.push(format!("read back: {} curves, {} ellipses, {} splines", back.len(), ovals.len(), spans.len()));
    } else {
        for (want, got) in curves().iter().zip(ovals.into_iter().chain(spans)) {
            let off = apart([&Curve::of_edge(want), &Curve::of_edge(got)]);
            if off > 0.01 {
                failures.push(format!("{want:?} came back {off:.4} off: {got:?}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_dxf_ellipse_and_spline_laid_into_a_sketch_are_an_ellipse_and_one_spline() {
    let path = tmp("qym_ellipse_spline_sketch.dxf");
    export_dxf(&curves(), &path).expect("the DXF is written");
    let back = import_dxf(&path).expect("the DXF is read back").curves;
    let mut p = Project::default();
    p.new_document();
    let si = p.import_sketch("drawing", back, None, SketchPlane::default());
    let s = &p.sketches[si];
    let ellipses = s.entities.iter().filter(|e| e.kind.as_ellipse().is_some()).count();
    let splines: Vec<usize> = s.entities.iter().filter_map(|e| e.kind.as_spline().map(|sp| sp.nodes.len())).collect();
    assert!(ellipses == 2 && splines == [3] && s.entities.len() == 3, "the sketch holds {} curves: {ellipses} ellipses, splines of {splines:?} nodes", s.entities.len());
}

#[test]
fn an_ellipse_written_to_svg_is_an_elliptical_arc() {
    let path = tmp("qym_ellipse.svg");
    export_svg(&curves()[..2], &path).expect("the SVG is written");
    let text = std::fs::read_to_string(&path).expect("the SVG is read");
    let paths: Vec<&str> = text.lines().filter(|l| l.contains("<path")).collect();
    assert!(paths.len() == 2 && paths.iter().all(|l| l.contains(" A ") && !l.contains(" L ")), "the ellipses went out as {paths:?}");
}
