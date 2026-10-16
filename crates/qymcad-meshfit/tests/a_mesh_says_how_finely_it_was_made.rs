//! A MESH SAYS HOW FINELY IT WAS MADE. The tolerance a surface is fitted to a mesh within has to come from the mesh -
//! from how far its triangles stand from the surface they were cut from - and not from the size of the part: a millionth
//! of the diagonal (1.1e-3 mm on a part of 110) let a cylinder lie on a smooth wall only as a narrow strip, and a
//! smooth handle came back as 1 887 strips of cylinders.
use qymcad_meshfit::{chord_deflection, prepare, weld_tolerance};

/// The true sag of `mesh` from a ball of radius `r` about the origin (`ball`) or a cylinder of radius `r` along Z, as the
/// bulk of the mesh has it: per triangle the largest distance from the surface of its middle and the middles of its
/// sides, and of those the one a twentieth of the triangles reach - a few triangles at a ball's pole stand twice as far
/// as the rest, and a tolerance is to follow the rest.
fn true_sag(mesh: &qymcad_core::geom::Mesh, r: f64, ball: bool) -> f64 {
    let mut sags = Vec::new();
    for t in &mesh.tris {
        let mut worst: f64 = 0.0;
        let v: Vec<_> = t.iter().map(|&i| mesh.verts[i as usize]).collect();
        for (a, b) in [(1.0 / 3.0, 1.0 / 3.0), (0.5, 0.5), (0.5, 0.0), (0.0, 0.5)] {
            let c = 1.0 - a - b;
            let q = [v[0].x * a + v[1].x * b + v[2].x * c, v[0].y * a + v[1].y * b + v[2].y * c, v[0].z * a + v[1].z * b + v[2].z * c];
            let off = if ball { r - (q[0] * q[0] + q[1] * q[1] + q[2] * q[2]).sqrt() } else { r - (q[0] * q[0] + q[1] * q[1]).sqrt() };
            let on_a_cap = !ball && [v[0].z, v[1].z, v[2].z].iter().all(|z| (z - v[0].z).abs() < 1e-9);
            if !on_a_cap {
                worst = worst.max(off.abs());
            }
        }
        if worst > 0.0 {
            sags.push(worst);
        }
    }
    sags.sort_by(f64::total_cmp);
    sags[((sags.len() as f64 * 0.95) as usize).min(sags.len() - 1)]
}

#[test]
fn the_chord_deflection_is_read_off_the_mesh() {
    for (name, shape, r, ball) in [
        ("a ball of 50", qymcad_kernel::specimen::sphere(50.0).expect("a ball"), 50.0, true),
        ("a cylinder of 40", qymcad_kernel::specimen::cylinder(40.0, 80.0).expect("a cylinder"), 40.0, false),
    ] {
        for made in [0.1, 0.03, 0.01, 0.003] {
            let qymcad_core::geom::Built { mesh, .. } = qymcad_kernel::specimen::mesh(&shape, made).into_iter().next().expect("a mesh");
            let (read, truth) = (chord_deflection(&prepare(&mesh, weld_tolerance(&mesh)), 30.0), true_sag(&mesh, r, ball));
            assert!(read > 0.3 * truth && read < 1.5 * truth, "{name} meshed at {made}: the mesh stands {truth} off its surface and reads {read}");
        }
    }
    // A MESH WITH NO CURVE HAS NO DEFLECTION TO READ: a box is its corners and its planes
    let block = qymcad_kernel::specimen::extrude(&[0.0, 0.0, 40.0, 0.0, 40.0, 30.0, 0.0, 30.0], 10.0).expect("a block");
    let qymcad_core::geom::Built { mesh, .. } = qymcad_kernel::specimen::mesh(&block, 0.01).into_iter().next().expect("a mesh");
    assert_eq!(chord_deflection(&prepare(&mesh, weld_tolerance(&mesh)), 30.0), 0.0, "a block has no curve and read a deflection");
}
