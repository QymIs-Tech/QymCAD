//! A BOX SHOWS FOUR EDGES AND HIDES FOUR: hidden-line projection of a 10 mm cube seen along -Z.
//!
//! The outlines a drawing sheet shows come from exact HLR, not from the tessellation: a dimension off
//! these lines meets the body. Seen face-on, the 4 side edges stand along the view and project to points,
//! so they are no lines at all: the front square is seen and the back square is hidden. Their projection
//! spans 10 by 10, 40 mm of seen outline and 40 mm of hidden.
use qymcad_core::model::Project;

#[test]
fn a_cube_seen_face_on_shows_four_edges_and_hides_four() {
    let mut p = Project::default();
    p.new_document();
    let block = p.add_box(10.0, 10.0, 10.0);
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "the block did not build: {:?}", report.errors);
    let shape = shapes.get(&block).expect("the block shape");
    let polys = shape.hlr([0.0, 0.0, -1.0]).expect("the projection runs on a box");
    assert_eq!(polys.len(), 8, "front and back squares only: the side edges stand along the view: {polys:?}");
    let (seen, hid): (Vec<_>, Vec<_>) = polys.iter().partition(|q| q.visible);
    assert_eq!((seen.len(), hid.len()), (4, 4), "the front square seen, the back square hidden");

    // The projection on the sheet plane spans the 10 mm of the cube both ways.
    let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for q in &polys {
        for pt in &q.points {
            x0 = x0.min(pt[0]);
            x1 = x1.max(pt[0]);
            y0 = y0.min(pt[1]);
            y1 = y1.max(pt[1]);
        }
    }
    assert!((x1 - x0 - 10.0).abs() < 1e-6 && (y1 - y0 - 10.0).abs() < 1e-6, "10 by 10 on the sheet: x {x0}..{x1}, y {y0}..{y1}");
    // Straight edges come out exact whatever the sampling: 4 of 10 mm seen, 4 hidden.
    let len = |qs: &[&qymcad_kernel::HlrPoly]| {
        qs.iter().map(|q| q.points.windows(2).map(|w| ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2) + (w[1][2] - w[0][2]).powi(2)).sqrt()).sum::<f64>()).sum::<f64>()
    };
    assert!((len(&seen) - 40.0).abs() < 1e-6, "40 mm of seen outline, got {}", len(&seen));
    assert!((len(&hid) - 40.0).abs() < 1e-6, "40 mm of hidden outline, got {}", len(&hid));
}

#[test]
fn no_direction_no_outlines() {
    let mut p = Project::default();
    p.new_document();
    let block = p.add_box(10.0, 10.0, 10.0);
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "the block did not build: {:?}", report.errors);
    let shape = shapes.get(&block).expect("the block shape");
    assert!(shape.hlr([0.0, 0.0, 0.0]).is_none(), "a zero direction looks from nowhere");
    assert!(shape.hlr([f64::NAN, 0.0, 0.0]).is_none(), "a direction that is not a number is refused");
}
