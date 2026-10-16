//! A region's surface, to the number: radii and positions to 0.01 mm, axes to a tenth of a degree. The meshes come
//! from our kernel, which stands every primitive on the Z axis at the origin - a cylinder and a cone with their base
//! at z = 0 - so every number is known before the mesh is read.

use qymcad_kernel::Shape;
use qymcad_meshfit::{prepare, regions, weld_tolerance, Region, Surface, Tolerance};
use qymcad_kernel::specimen::Specimen as _;

fn regions_of(shape: &Shape, deflection: f64) -> Vec<Region> {
    let qymcad_core::geom::Built { mesh, .. } = qymcad_kernel::specimen::mesh(shape, deflection).into_iter().next().expect("a body");
    let p = prepare(&mesh, weld_tolerance(&mesh));
    regions(&p, &Tolerance::for_mesh(&p))
}

/// The angle between an axis and Z, either way round, in degrees.
fn off_z(axis: [f64; 3]) -> f64 {
    axis[2].abs().clamp(0.0, 1.0).acos().to_degrees()
}

/// Where the line through `point` along `axis` (near Z) crosses z = 0: how far from the origin.
fn off_origin(point: [f64; 3], axis: [f64; 3]) -> f64 {
    let t = -point[2] / axis[2];
    (point[0] + t * axis[0]).hypot(point[1] + t * axis[1])
}

fn surfaces(found: &[Region]) -> Vec<Surface> {
    found.iter().filter_map(|r| r.surface.clone()).collect()
}

#[test]
fn a_cylinder_gets_its_radius_and_its_axis() {
    let found = surfaces(&regions_of(&qymcad_kernel::specimen::cylinder(10.0, 30.0).expect("a cylinder"), 0.05));
    let cyl: Vec<([f64; 3], [f64; 3], f64)> = found
        .iter()
        .filter_map(|s| match *s {
            Surface::Cylinder { point, axis, radius } => Some((point, axis, radius)),
            _ => None,
        })
        .collect();
    assert_eq!(cyl.len(), 1, "surfaces {found:?}");
    let (point, axis, radius) = cyl[0];
    assert!((radius - 10.0).abs() < 0.01, "radius {radius}, not 10");
    assert!(off_z(axis) < 0.1, "the axis leans {} deg off Z", off_z(axis));
    assert!(off_origin(point, axis) < 0.01, "the axis passes {} mm off the origin", off_origin(point, axis));
}

#[test]
fn a_sphere_gets_its_centre_and_radius() {
    let found = surfaces(&regions_of(&qymcad_kernel::specimen::sphere(15.0).expect("a sphere"), 0.05));
    let Some(Surface::Sphere { center, radius }) = found.first().cloned() else { panic!("no sphere: {found:?}") };
    assert!((radius - 15.0).abs() < 0.01, "radius {radius}, not 15");
    assert!(center.iter().all(|c| c.abs() < 0.01), "centre {center:?}, not the origin");
}

/// Every rounding of the bar has radius 2, every corner sphere radius 2, every plane faces along an axis.
#[test]
fn a_rounded_bar_gets_every_radius() {
    let bar = qymcad_kernel::specimen::extrude(&[0.0, 0.0, 40.0, 0.0, 40.0, 20.0, 0.0, 20.0], 10.0).expect("a bar").rounded(2.0).expect("filleted");
    for s in surfaces(&regions_of(&bar, 0.1)) {
        match s {
            Surface::Cylinder { radius, .. } | Surface::Sphere { radius, .. } => assert!((radius - 2.0).abs() < 0.01, "a rounding of radius {radius}, not 2"),
            Surface::Plane { normal, .. } => assert!(normal.iter().any(|n| n.abs() > 0.1f64.to_radians().cos()), "a plane facing {normal:?}, off every axis"),
            other => panic!("the bar has no {other:?}"),
        }
    }
}

/// A cone from radius 10 at z = 0 to radius 4 at z = 20: its half-angle is atan(6 / 20), its apex at z = 100 / 3.
#[test]
fn a_cone_gets_its_apex_and_its_angle() {
    let found = surfaces(&regions_of(&qymcad_kernel::specimen::cone(10.0, 4.0, 20.0).expect("a cone"), 0.05));
    let cones: Vec<([f64; 3], [f64; 3], f64)> = found
        .iter()
        .filter_map(|s| match *s {
            Surface::Cone { apex, axis, half_angle } => Some((apex, axis, half_angle)),
            _ => None,
        })
        .collect();
    assert_eq!(cones.len(), 1, "surfaces {found:?}");
    let (apex, axis, half) = cones[0];
    let want = (6.0f64 / 20.0).atan();
    assert!((half - want).abs().to_degrees() < 0.1, "half-angle {} deg, not {} deg", half.to_degrees(), want.to_degrees());
    assert!(off_z(axis) < 0.1, "the axis leans {} deg off Z", off_z(axis));
    assert!(apex[0].hypot(apex[1]) < 0.01 && (apex[2] - 100.0 / 3.0).abs() < 0.01, "apex {apex:?}, not (0, 0, 33.333)");
    assert_eq!(found.iter().filter(|s| matches!(s, Surface::Plane { .. })).count(), 2, "surfaces {found:?}");
}

/// A torus of major radius 20 and minor radius 5 in the XY plane about the origin.
#[test]
fn a_torus_gets_both_radii() {
    let found = surfaces(&regions_of(&qymcad_kernel::specimen::torus(20.0, 5.0).expect("a torus"), 0.05));
    assert_eq!(found.len(), 1, "surfaces {found:?}");
    let Surface::Torus { center, axis, major, minor } = found[0] else { panic!("no torus: {found:?}") };
    assert!((major - 20.0).abs() < 0.01 && (minor - 5.0).abs() < 0.01, "radii {major} and {minor}, not 20 and 5");
    assert!(center.iter().all(|c| c.abs() < 0.01), "centre {center:?}, not the origin");
    assert!(off_z(axis) < 0.1, "the axis leans {} deg off Z", off_z(axis));
}
