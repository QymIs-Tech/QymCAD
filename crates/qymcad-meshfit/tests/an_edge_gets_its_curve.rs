//! The curve each edge runs along: a straight line or a circle where the two surfaces meet in one, and the edge's own
//! points put on both surfaces where no such curve is known. The meshes come from our kernel, so every curve is known
//! before the mesh is read.

use qymcad_kernel::Shape;
use qymcad_meshfit::{boundaries, curves, prepare, regions, weld_tolerance, Boundaries, Curve, Edge, Region, Surface, Tolerance};
use qymcad_kernel::specimen::Specimen as _;

fn curves_of(shape: &Shape, deflection: f64) -> (Boundaries, Vec<Curve>) {
    let qymcad_core::geom::Built { mesh, .. } = qymcad_kernel::specimen::mesh(shape, deflection).into_iter().next().expect("a body");
    let p = prepare(&mesh, weld_tolerance(&mesh));
    let tol = Tolerance::for_mesh(&p);
    let found = regions(&p, &tol);
    let b = boundaries(&p, &found);
    let c = curves(&b, &found, &tol);
    (b, c)
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// How far `q` lies from the line through `point` along the unit `dir`.
fn off_line(q: [f64; 3], point: [f64; 3], dir: [f64; 3]) -> f64 {
    let d = [q[0] - point[0], q[1] - point[1], q[2] - point[2]];
    let t = dot(d, dir);
    (dot(d, d) - t * t).max(0.0).sqrt()
}

#[test]
fn a_cube_edge_runs_along_a_straight_line() {
    let cube = qymcad_kernel::specimen::extrude(&[0.0, 0.0, 100.0, 0.0, 100.0, 100.0, 0.0, 100.0], 100.0).expect("a cube");
    let (b, found) = curves_of(&cube, 0.1);
    assert_eq!(found.len(), 12, "curves of the cube");
    for (e, c) in found.iter().enumerate() {
        let Curve::Line { point, dir } = *c else { panic!("edge {e} of the cube runs along {c:?}") };
        assert!(dir.iter().any(|d| (d.abs() - 1.0).abs() < 1e-9), "edge {e} runs along {dir:?}, off every axis");
        for v in b.edges[e].ends.expect("a cube's edge has ends") {
            let off = off_line(b.vertices[v], point, dir);
            assert!(off < 1e-6, "the corner {:?} lies {off} off the line of edge {e}", b.vertices[v]);
        }
    }
}

#[test]
fn a_washer_edge_is_a_circle() {
    let washer = qymcad_kernel::specimen::revolve(&[0.0, 5.0, 4.0, 5.0, 4.0, 10.0, 0.0, 10.0], 0, 360.0).expect("a washer");
    let (_, found) = curves_of(&washer, 0.01);
    assert_eq!(found.len(), 4, "curves of the washer");
    let mut radii = Vec::new();
    for c in &found {
        let Curve::Circle { center, axis, radius } = *c else { panic!("a washer's edge runs along {c:?}") };
        assert!((axis[0].abs() - 1.0).abs() < 1e-6, "a circle about {axis:?}, not about X");
        assert!((center[0].abs() < 1e-6 || (center[0] - 4.0).abs() < 1e-6) && center[1].hypot(center[2]) < 0.01, "a circle about {center:?}");
        radii.push(radius);
    }
    radii.sort_by(f64::total_cmp);
    assert!(radii.iter().zip([5.0, 5.0, 10.0, 10.0]).all(|(r, want)| (r - want).abs() < 0.01), "radii {radii:?}, not 5, 5, 10, 10");
}

/// Where a rounding leaves a flat side, the two meet tangent along a straight line; where it meets a corner's sphere,
/// along a circle of the rounding's radius.
#[test]
fn a_rounded_bar_edge_is_a_tangent_line_or_a_circle() {
    let bar = qymcad_kernel::specimen::extrude(&[0.0, 0.0, 40.0, 0.0, 40.0, 20.0, 0.0, 20.0], 10.0).expect("a bar").rounded(2.0).expect("filleted");
    for deflection in [0.1, 0.02] {
        let (b, found) = curves_of(&bar, deflection);
        let lines: Vec<usize> = (0..found.len()).filter(|&e| matches!(found[e], Curve::Line { .. })).collect();
        let circles: Vec<f64> = found.iter().filter_map(|c| if let Curve::Circle { radius, .. } = *c { Some(radius) } else { None }).collect();
        assert_eq!((lines.len(), circles.len()), (24, 24), "lines and circles of the rounded bar at {deflection}: {:?}", found.iter().filter(|c| matches!(c, Curve::Points(_))).count());
        assert!(circles.iter().all(|r| (r - 2.0).abs() < 0.01), "circles of radii {circles:?}, not 2");
        // a rounding touches a corner's sphere along its great circle: the circle's centre is the sphere's
        let centres: Vec<[f64; 3]> = found.iter().filter_map(|c| if let Curve::Circle { center, .. } = *c { Some(center) } else { None }).collect();
        for c in &centres {
            let off = [2.0, 38.0]
                .iter()
                .flat_map(|&x| [2.0, 18.0].map(|y| (x, y)))
                .flat_map(|(x, y)| [2.0, 8.0].map(|z| [x, y, z]))
                .map(|s| ((c[0] - s[0]).powi(2) + (c[1] - s[1]).powi(2) + (c[2] - s[2]).powi(2)).sqrt())
                .fold(f64::MAX, f64::min);
            assert!(off < 0.01, "a circle about {c:?} stands {off} mm off every corner sphere's centre at {deflection}");
        }
        for e in lines {
            let Curve::Line { point, dir } = found[e] else { unreachable!() };
            for v in b.edges[e].ends.expect("an edge of the bar has ends") {
                let off = off_line(b.vertices[v], point, dir);
                assert!(off < 0.01, "the corner {:?} lies {off} off the line of edge {e}", b.vertices[v]);
            }
        }
    }
}

/// A SPHERE SET OFF A CYLINDER'S AXIS MEETS IT IN NO CIRCLE, and the edge keeps its points. A sphere of 10.5 on a
/// cylinder of 10, its centre 0.1 mm off the axis - near enough to be offered a circle - meets the wall along a curve
/// whose height swings by 0.63 mm round the axis: no circle fits it, and one taken unchecked would bend the edge. The
/// edge is laid on the true curve by hand: a mesh of such a knob from the kernel splits its wall into strips first.
#[test]
fn a_sphere_off_the_axis_meets_a_cylinder_in_no_circle() {
    let wall = Surface::Cylinder { point: [0.0, 0.0, 0.0], axis: [0.0, 0.0, 1.0], radius: 10.0 };
    let knob = Surface::Sphere { center: [0.1, 0.0, 30.0], radius: 10.5 };
    // on the wall x = 10 cos f, y = 10 sin f; on the sphere (x - 0.1)^2 + y^2 + (z - 30)^2 = 10.5^2
    let points: Vec<[f64; 3]> = (0..120)
        .map(|k| {
            let f = k as f64 * std::f64::consts::TAU / 120.0;
            [10.0 * f.cos(), 10.0 * f.sin(), 30.0 - (10.24 + 2.0 * f.cos()).sqrt()]
        })
        .collect();
    let swing = points.iter().map(|q| q[2]).fold(f64::MIN, f64::max) - points.iter().map(|q| q[2]).fold(f64::MAX, f64::min);
    let found = [Region { tris: Vec::new(), surface: Some(wall) }, Region { tris: Vec::new(), surface: Some(knob) }];
    let b = Boundaries { edges: vec![Edge { faces: [0, 1], ends: None, points }], loops: vec![Vec::new(), Vec::new()], ..Default::default() };
    let tol = Tolerance { distance: 0.0005, angle_deg: 10.0, sharp_deg: 30.0 };
    let c = curves(&b, &found, &tol);
    assert!(matches!(c[0], Curve::Points(_)), "the wall meets the sphere, swinging {swing:.2} mm, along {:?}", c[0]);
}

/// AN ARC ITS POINTS CANNOT SHOW IS NOT TAKEN. A plane meets a sphere in a circle of 2.56, and an edge between them runs
/// from one point of that circle to another 160 deg round it - with nothing between. Any circle through two points fits
/// them; the edge is the straight side of a triangle, as it was on a gear of `cube_gears`, where the arc taken for it
/// made its face 3 400 times its triangle.
#[test]
fn an_arc_its_points_cannot_show_is_not_taken() {
    let flat = Surface::Plane { point: [0.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] };
    let crumb = Surface::Sphere { center: [0.0, 0.0, -1.5], radius: (2.56f64 * 2.56 + 1.5 * 1.5).sqrt() };
    let at = |deg: f64| [2.56 * deg.to_radians().cos(), 2.56 * deg.to_radians().sin(), 0.0];
    let (a, z) = (at(10.0), at(170.0));
    let found = [Region { tris: Vec::new(), surface: Some(flat) }, Region { tris: Vec::new(), surface: Some(crumb) }];
    let b = Boundaries { vertices: vec![a, z], corners: vec![0, 1], edges: vec![Edge { faces: [0, 1], ends: Some([0, 1]), points: vec![a, z] }], loops: vec![Vec::new(), Vec::new()] };
    let tol = Tolerance { distance: 0.00027, angle_deg: 10.0, sharp_deg: 30.0 };
    let c = curves(&b, &found, &tol);
    assert!(!matches!(c[0], Curve::Circle { .. }), "two points 160 deg apart were taken for an arc: {:?}", c[0]);
}
