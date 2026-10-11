//! A DXF SPLINE WHOSE NUMBERS MAKE NO CURVE IS LEFT OUT AND NAMED, not a reason for the program to end.
//!
//! The range of a spline is two of its knots, and the reader clamped every parameter into it: `clamp` panics when the
//! low end is above the high one or either is not a number - from the UI thread, ending the program. A knot, a weight
//! or a point that is not finite let NaN corners into the sketch. Such a spline is now drawn through its fit points
//! when it has them (they lie on the curve) and named as drawn so, and otherwise left out and named as invalid - each
//! spline once, under what happened to it.
use dxf::entities::{Entity, EntityType, Spline};
use dxf::{Drawing, Point};
use qymcad_io::import_dxf;

/// A folder for the files, under `target`.
fn written(name: &str) -> String {
    let dir = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/dxf-spline-probe"));
    std::fs::create_dir_all(&dir).expect("a folder for the check");
    dir.join(name).to_string_lossy().into_owned()
}

/// The knots of a Bezier: one span from 0 to 1.
const BEZIER: [f64; 8] = [0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];

/// A drawing of one cubic spline with the control points of a Bezier from (0, 0) to (40, 0), its knots `knots`, and
/// its fit points `fit`; saved as R2013 (splines came with R13).
fn drawing_of(name: &str, knots: Vec<f64>, fit: Vec<Point>) -> String {
    drawing_with(name, knots, fit, |_| {})
}

/// The same, with `change` made to the spline before it is saved.
fn drawing_with(name: &str, knots: Vec<f64>, fit: Vec<Point>, change: impl FnOnce(&mut Spline)) -> String {
    let mut d = Drawing::new();
    d.header.version = dxf::enums::AcadVersion::R2013;
    let mut sp = Spline { degree_of_curve: 3, knot_values: knots, ..Default::default() };
    sp.control_points = vec![Point::new(0.0, 0.0, 0.0), Point::new(10.0, 20.0, 0.0), Point::new(30.0, -20.0, 0.0), Point::new(40.0, 0.0, 0.0)];
    sp.fit_points = fit;
    change(&mut sp);
    d.add_entity(Entity::new(EntityType::Spline(sp)));
    let p = written(name);
    d.save_file(&p).expect("written");
    p
}

/// What the import names: the kinds not read, left out as invalid, and drawn another way.
#[derive(Debug, PartialEq)]
struct Named {
    not_read: Vec<(String, usize)>,
    invalid: Vec<(String, usize)>,
    redrawn: Vec<(String, usize)>,
}

fn named(s: &qymcad_io::ImportedSketch) -> Named {
    Named { not_read: s.skipped.clone(), invalid: s.invalid.clone(), redrawn: s.redrawn.clone() }
}

fn none() -> Vec<(String, usize)> {
    Vec::new()
}

/// One spline.
fn spline() -> Vec<(String, usize)> {
    vec![("SPLINE".to_string(), 1)]
}

/// Every corner of every curve read.
fn corners(s: &qymcad_io::ImportedSketch) -> Vec<(f64, f64)> {
    s.curves
        .iter()
        .flat_map(|c| match c {
            qymcad_core::geom::ProfEdge::Line { a, b } => vec![(a.x, a.y), (b.x, b.y)],
            _ => Vec::new(),
        })
        .collect()
}

#[test]
fn knots_out_of_order_leave_the_spline_out() {
    let p = drawing_of("backwards.dxf", vec![1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0], Vec::new());
    let got = import_dxf(&p).expect("the drawing is read");
    assert!(got.curves.is_empty(), "a spline with its range backwards drew {} curves", got.curves.len());
    assert_eq!(named(&got), Named { not_read: none(), invalid: spline(), redrawn: none() }, "the spline left out is not named once, as invalid");
}

#[test]
fn a_knot_that_is_not_a_number_leaves_the_spline_out() {
    let p = drawing_of("nan-knot.dxf", vec![0.0, 0.0, 0.0, 0.0, f64::NAN, 1.0, 1.0, 1.0], Vec::new());
    let got = import_dxf(&p).expect("the drawing is read");
    assert!(corners(&got).iter().all(|(x, y)| x.is_finite() && y.is_finite()), "a NaN knot let corners that are not numbers into the sketch");
    assert_eq!(named(&got), Named { not_read: none(), invalid: spline(), redrawn: none() }, "the spline left out is not named once, as invalid");
}

#[test]
fn bad_knots_with_fit_points_run_through_the_fit_points() {
    let fit = vec![Point::new(0.0, 0.0, 0.0), Point::new(20.0, 0.0, 0.0), Point::new(40.0, 0.0, 0.0)];
    let p = drawing_of("backwards-fit.dxf", vec![1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0], fit);
    let got = import_dxf(&p).expect("the drawing is read");
    let c = corners(&got);
    for (x, y) in [(0.0, 0.0), (20.0, 0.0), (40.0, 0.0)] {
        assert!(c.iter().any(|q| (q.0 - x).abs() < 1e-9 && (q.1 - y).abs() < 1e-9), "the spline does not run through its fit point ({x}, {y})");
    }
    assert_eq!(named(&got), Named { not_read: none(), invalid: none(), redrawn: spline() }, "a spline drawn through its fit points is not named once, as redrawn");
}

/// A SOUND SPLINE IS NAMED NOWHERE, nor is one given by its fit points alone.
#[test]
fn sound_splines_are_not_named_invalid() {
    let got = import_dxf(&drawing_of("sound.dxf", BEZIER.to_vec(), Vec::new())).expect("the drawing is read");
    assert!(!got.curves.is_empty(), "a sound spline drew nothing");
    assert_eq!(named(&got), Named { not_read: none(), invalid: none(), redrawn: none() }, "what is named of a sound spline");
    let fit = vec![Point::new(0.0, 0.0, 0.0), Point::new(20.0, 0.0, 0.0), Point::new(40.0, 0.0, 0.0)];
    let got = import_dxf(&drawing_with("fit-only.dxf", Vec::new(), fit, |sp| sp.control_points.clear())).expect("the drawing is read");
    assert!(!got.curves.is_empty(), "a spline of fit points alone drew nothing");
    assert_eq!(named(&got), Named { not_read: none(), invalid: none(), redrawn: none() }, "what is named of a spline of fit points alone");
}

/// The spline of the drawing at `p` is left out and named once, as invalid, and no corner that is not a number comes in.
fn left_out(p: &str) {
    let got = import_dxf(p).expect("the drawing is read");
    assert!(corners(&got).iter().all(|(x, y)| x.is_finite() && y.is_finite()), "corners that are not numbers came into the sketch");
    assert_eq!(named(&got), Named { not_read: none(), invalid: spline(), redrawn: none() }, "the spline left out is not named once, as invalid");
}

/// Every knot the same: the range is empty, and the curve would be one point - nothing drawn and nothing said.
#[test]
fn knots_with_no_range_leave_the_spline_out() {
    left_out(&drawing_of("no-range.dxf", vec![0.5; 8], Vec::new()));
}

/// Every knot a number, but the range from -1e308 to 1e308 is longer than any number: its samples would not be.
#[test]
fn a_range_too_long_for_a_number_leaves_the_spline_out() {
    left_out(&drawing_of("huge-range.dxf", vec![-1e308, -1e308, -1e308, -1e308, 1e308, 1e308, 1e308, 1e308], Vec::new()));
}

/// The range (knots 3 and 4) is in order, but a knot before it goes back: the knots are no B-spline's.
#[test]
fn knots_that_go_back_before_the_range_leave_the_spline_out() {
    left_out(&drawing_of("back-inside.dxf", vec![0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 1.0, 1.0], Vec::new()));
}

#[test]
fn an_infinite_knot_leaves_the_spline_out() {
    let mut knots = BEZIER.to_vec();
    knots[0] = f64::NEG_INFINITY;
    left_out(&drawing_of("infinite-knot.dxf", knots, Vec::new()));
}

#[test]
fn a_weight_of_zero_leaves_the_spline_out() {
    left_out(&drawing_with("zero-weight.dxf", BEZIER.to_vec(), Vec::new(), |sp| sp.weight_values = vec![0.0, 1.0, 1.0, 1.0]));
}

#[test]
fn a_control_point_that_is_not_a_number_leaves_the_spline_out() {
    left_out(&drawing_with("nan-point.dxf", BEZIER.to_vec(), Vec::new(), |sp| sp.control_points[1].x = f64::NAN));
}

#[test]
fn bad_knots_and_a_fit_point_that_is_not_a_number_leave_the_spline_out() {
    let fit = vec![Point::new(0.0, 0.0, 0.0), Point::new(f64::NAN, 0.0, 0.0), Point::new(40.0, 0.0, 0.0)];
    left_out(&drawing_of("nan-fit.dxf", vec![1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0], fit));
}
