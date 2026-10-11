//! A HEAVY MESH IS OFFERED ITS SIMPLIFICATION when it is picked for recognition: the field "Simplify, mm" gets a
//! ten-thousandth of the mesh's size, and a light mesh keeps zero. Reported behaviour: a body made from a
//! 178 000-triangle mesh as it is rebuilt for minutes after every operation, and nothing said so beforehand.
use qymcad_core::geom::{Mesh, Point3};
use qymcad_part::{suggest_simplify, HEAVY_MESH};
use qymcad_ui_state::{CmdParam, FeatCommand};

fn strip(tris: usize, length: f64) -> Mesh {
    let n = tris / 2 + 1;
    let mut verts = Vec::new();
    for i in 0..n {
        let x = length * i as f64 / (n - 1) as f64;
        verts.extend([Point3::new(x, 0.0, 0.0), Point3::new(x, 1.0, 0.0)]);
    }
    let tris = (0..n - 1)
        .flat_map(|i| {
            let k = 2 * i as u32;
            [[k, k + 2, k + 1], [k + 1, k + 2, k + 3]]
        })
        .collect();
    Mesh { verts, tris, ..Default::default() }
}

fn command() -> FeatCommand {
    FeatCommand { params: vec![CmdParam::new("f-recognise-tol", "tol", 1.0, 0.1, 1000.0), CmdParam::new("f-recognise-simplify", "simplify", 0.0, 0.0, 10.0)], ..Default::default() }
}

#[test]
fn a_heavy_mesh_gets_a_simplification_of_its_size() {
    let mut cmd = command();
    suggest_simplify(&mut cmd, &strip(HEAVY_MESH + 10, 100.0));
    let v = cmd.params.iter().find(|p| p.key == "simplify").map(|p| p.val);
    assert_eq!(v, Some(0.01), "a mesh of 100 mm is offered {v:?} mm, not a ten-thousandth of its size");
}

#[test]
fn a_light_mesh_keeps_every_triangle() {
    let mut cmd = command();
    suggest_simplify(&mut cmd, &strip(1000, 100.0));
    assert_eq!(cmd.params.iter().find(|p| p.key == "simplify").map(|p| p.val), Some(0.0));
}
