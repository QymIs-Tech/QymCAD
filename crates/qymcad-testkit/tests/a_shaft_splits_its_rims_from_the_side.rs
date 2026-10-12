//! A CYLINDER SEEN FROM THE SIDE SHOWS ITS SILHOUETTE: hidden-line projection of a shaft r5 x 20 mm
//! along -Y.
//!
//! The rims split where they turn away: each rim circle gives a seen front half and a hidden back half.
//! The silhouette sides have no topology edge to sample an exact curve from, so they come tessellated
//! from the shape's own mesh; rim halves stay exact. What the sheet needs is all there: 4 rim halves
//! and both sides drawn unbroken from rim to rim.
use qymcad_core::model::Project;

#[test]
fn a_shaft_seen_from_the_side_splits_its_rims() {
    let mut p = Project::default();
    p.new_document();
    let shaft = p.add_cylinder(5.0, 20.0);
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "the shaft did not build: {:?}", report.errors);
    let shape = shapes.get(&shaft).expect("the shaft shape");
    let polys = shape.hlr([0.0, -1.0, 0.0]).expect("the projection runs on a shaft");
    // Rim pieces run at a constant z (top or bottom); silhouette sides run along z.
    let is_rim = |q: &qymcad_kernel::HlrPoly| q.points.iter().all(|p| (p[2] - q.points[0][2]).abs() < 1e-9);
    let (rims, sides): (Vec<_>, Vec<_>) = polys.iter().partition(|q| is_rim(q));
    assert_eq!(rims.len(), 4, "top and bottom rims in front and back halves: {polys:?}");
    assert_eq!(rims.iter().filter(|q| q.visible).count(), 2, "front halves seen");
    assert_eq!(rims.iter().filter(|q| !q.visible).count(), 2, "back halves hidden");
    // The seam of the wall draws exact where it stands on the silhouette; the mesh silhouettes below
    // draw over it invisibly. Tangent interior edges draw too, as thin lines - the usual convention.
    let seam_len: f64 =
        sides.iter().filter(|q| q.visible).map(|q| q.points.windows(2).map(|w| ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2) + (w[1][2] - w[0][2]).powi(2)).sqrt()).sum::<f64>()).sum();
    assert!((seam_len - 20.0).abs() < 1e-6, "the seam side draws exact: {seam_len}");

    // Both silhouette sides come from the mesh, unbroken from rim to rim: at every height a segment
    // stands at |x| = 5. A gap would read as a broken outline on the sheet.
    let mesh = &p.bodies.iter().find(|b| b.id == shaft).expect("the body").mesh;
    let silhs = qymcad_core::geom::silhouette_segments(mesh, [0.0, 1.0, 0.0]);
    let total: f64 = silhs.iter().map(|s| ((s.b[0] - s.a[0]).powi(2) + (s.b[1] - s.a[1]).powi(2) + (s.b[2] - s.a[2]).powi(2)).sqrt()).sum();
    assert!((total - 40.0).abs() < 1.0, "two sides of 20: {total}");
    for zt in [1.0, 5.0, 10.0, 15.0, 19.0] {
        for side in [-5.0, 5.0] {
            let ok = silhs.iter().any(|s| {
                let near = |q: [f64; 3]| (q[0] - side).abs() < 0.3 && (q[2] - zt).abs() < 1.5;
                near(s.a) && near(s.b) || (s.a[0] - side).abs() < 0.3 && (s.b[0] - side).abs() < 0.3 && (s.a[2] - zt) * (s.b[2] - zt) <= 0.0
            });
            assert!(ok, "no silhouette at x {side}, z {zt}");
        }
    }
}

#[test]
fn a_shaft_seen_end_on_shows_two_circles() {
    let mut p = Project::default();
    p.new_document();
    let shaft = p.add_cylinder(5.0, 20.0);
    let (report, shapes) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "the shaft did not build: {:?}", report.errors);
    let shape = shapes.get(&shaft).expect("the shaft shape");
    let polys = shape.hlr([0.0, 0.0, -1.0]).expect("the projection runs end on");
    assert_eq!(polys.len(), 2, "the near rim seen, the far rim hidden: {polys:?}");
    let (seen, hid): (Vec<_>, Vec<_>) = polys.iter().partition(|q| q.visible);
    assert_eq!((seen.len(), hid.len()), (1, 1), "one rim each way");
    let rim = std::f64::consts::TAU * 5.0;
    let len = |qs: &[&qymcad_kernel::HlrPoly]| {
        qs.iter().map(|q| q.points.windows(2).map(|w| ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2) + (w[1][2] - w[0][2]).powi(2)).sqrt()).sum::<f64>()).sum::<f64>()
    };
    assert!((len(&seen) - rim).abs() / rim < 0.01, "a full rim within a per cent: {}", len(&seen));
    assert!((len(&hid) - rim).abs() / rim < 0.01, "the far rim whole: {}", len(&hid));
}
