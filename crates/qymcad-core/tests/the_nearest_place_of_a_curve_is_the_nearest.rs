//! THE NEAREST PLACE OF A CURVE IS THE NEAREST: for points all round a closed spline with a notch - the blob of a report
//! with its top node dragged down - the place `Curve::nearest` answers stands no farther than the nearest of 20 000
//! places along it.
//!
//! Reported behaviour: a node of a spline dragged, its offset came out 40 mm off - the offset was right, and the nearest
//! place of the spline to a point of it was taken at the far side of the spline.
use qymcad_core::feature::Ends;
use qymcad_core::geom::Point2;
use qymcad_core::model::curve::Curve;

#[test]
fn the_nearest_place_of_a_spline_is_no_farther_than_a_dense_search_finds() {
    let nodes = [[-22.0, 6.0], [-14.0, 14.0], [-3.0, 7.0], [7.0, 23.0], [21.0, 9.0], [10.0, -9.0], [-17.0, -14.0]].map(|q| Point2::new(q[0], q[1]));
    let blob = Curve::spline(nodes.to_vec(), &[], Ends::Closed);
    let dense: Vec<Point2> = (0..20_000).map(|k| blob.at(k as f64 / 20_000.0)).collect();
    let mut worst: Option<String> = None;
    let mut misses = 0;
    // a grid all round it, and the point of its offset by 2 inwards that was answered 42 mm off
    let grid = (0..=60).flat_map(|i| (0..=60).map(move |j| Point2::new(-30.0 + i as f64, -20.0 + 0.75 * j as f64)));
    for q in grid.chain([Point2::new(7.039731355623767, 20.901495777491668)]) {
        {
            let found = blob.nearest(q).off;
            let best = dense.iter().map(|d| d.dist(q)).fold(f64::MAX, f64::min);
            if found > best + 1e-3 {
                misses += 1;
                worst.get_or_insert_with(|| format!("at {q:?} the nearest place answered stands {found:.3} off, a dense search finds {best:.3}"));
            }
        }
    }
    assert!(misses == 0, "{misses} of 3 722 points: {}", worst.unwrap_or_default());
}

/// At a bend of a large spline as tight as the spacing of the places the search starts from, the distance to a point
/// 3 off it dips twice between two of those places; the nearer dip is the nearest place.
#[test]
fn the_nearest_place_at_a_tight_bend_of_a_large_spline_is_the_nearer_dip() {
    let nodes = [
        [-62.0, 202.0],
        [-125.0, 161.0],
        [-159.0, 115.0],
        [-85.0, 98.0],
        [-163.0, 58.0],
        [-256.0, 59.0],
        [-228.0, -7.0],
        [-139.0, 39.0],
        [-136.0, -32.0],
        [-167.0, -95.0],
        [-83.0, -127.0],
        [-66.0, -64.0],
        [-26.0, -117.0],
        [88.0, -88.0],
        [48.0, -52.0],
        [131.0, -41.0],
        [158.0, -85.0],
        [229.0, -24.0],
        [132.0, 18.0],
        [181.0, 55.0],
        [112.0, 147.0],
        [62.0, 105.0],
        [60.0, 164.0],
        [155.0, 186.0],
        [83.0, 231.0],
        [27.0, 182.0],
        [21.0, 238.0],
        [-58.0, 240.0],
        [-174.0, 267.0],
        [-299.0, 247.0],
        [-422.0, 161.0],
        [-378.0, 85.0],
        [-304.0, 161.0],
        [-284.0, 212.0],
        [-197.0, 177.0],
        [-277.0, 112.0],
        [-196.0, 123.0],
        [-169.0, 191.0],
        [-98.0, 215.0],
    ]
    .map(|q| Point2::new(q[0], q[1]));
    let spline = Curve::spline(nodes.to_vec(), &[], Ends::Closed);
    let q = Point2::new(-273.4159394123811, 112.59601740508121);
    let best = (0..200_000).map(|k| spline.at(k as f64 / 200_000.0).dist(q)).fold(f64::MAX, f64::min);
    let found = spline.nearest(q).off;
    assert!(found < best + 1e-3, "the nearest place answered stands {found:.4} off, a dense search finds {best:.4}");
}
