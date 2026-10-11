//! A MESH TURNED INTO A SOLID, through the document: a cube of twelve triangles, come in as a piece of a mesh, becomes a
//! solid by its node - six faces, the two triangles of each side merged into one, a volume of 1000 - in the part the
//! mesh stood in, and the mesh is consumed. Built with the live kernel: a mock has no faces to sew.
use qymcad_core::geom::{Mesh, Point3};
use qymcad_core::model::{ImportNode, Project};

/// A cube of `side` from twelve triangles, each turned out of the solid.
fn cube(side: f64) -> Mesh {
    let v = |x: f64, y: f64, z: f64| Point3::new(x * side, y * side, z * side);
    Mesh {
        verts: vec![v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(1.0, 1.0, 0.0), v(0.0, 1.0, 0.0), v(0.0, 0.0, 1.0), v(1.0, 0.0, 1.0), v(1.0, 1.0, 1.0), v(0.0, 1.0, 1.0)],
        tris: vec![[0, 2, 1], [0, 3, 2], [4, 5, 6], [4, 6, 7], [0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6], [3, 0, 4], [3, 4, 7]],
        ..Default::default()
    }
}

#[test]
fn a_mesh_piece_becomes_a_solid_of_its_flat_faces() {
    let mut p = Project::default();
    p.new_document();
    let source = p.add_source("cube.stl", vec![1]);
    let src = p.add_mesh(cube(10.0));
    p.import_tree_as_parts(vec![ImportNode { name: "cube".into(), body: Some(src), mesh: true, ..Default::default() }], source, "cube").expect("came in");
    let part = p.timeline.iter().find(|n| n.kind.owns_body(src)).and_then(|n| n.parent).expect("the mesh's part");
    let body = p.add_mesh_solid(src, 0.0);
    let (_report, shapes) = qymcad_testkit::regenerate(&mut p);
    let solid = shapes.get(&body).unwrap_or_else(|| panic!("no live solid for the node; the rebuild says {:?}", p.regen_errors));
    assert!((solid.volume() - 1000.0).abs() < 1e-6, "the solid holds {} mm^3, not the cube's 1000", solid.volume());
    // the faces a rebuild derives live in `regen_faces`: the body's own list is the window's to fill
    let faces = p.regen_faces.get(&body).map(|f| f.len());
    assert_eq!(faces, Some(6), "a side of two triangles is not one face");
    assert!(p.consumed_bodies().contains(&src), "the mesh stands beside the solid, a second body of the part");
    let node = p.timeline.iter().find(|n| n.id == body).expect("the node");
    assert_eq!(node.parent, Some(part), "the solid does not stand in the mesh's part");
}

/// A ball of `r` in `rings` x `segs` quads, its corners shared.
fn ball(r: f64, rings: usize, segs: usize) -> Mesh {
    let mut verts = vec![Point3::new(0.0, 0.0, r)];
    for i in 1..rings {
        for j in 0..segs {
            let (t, a) = (std::f64::consts::PI * i as f64 / rings as f64, std::f64::consts::TAU * j as f64 / segs as f64);
            verts.push(Point3::new(r * t.sin() * a.cos(), r * t.sin() * a.sin(), r * t.cos()));
        }
    }
    verts.push(Point3::new(0.0, 0.0, -r));
    let south = (verts.len() - 1) as u32;
    let id = |i: usize, j: usize| (1 + (i - 1) * segs + j % segs) as u32;
    let mut tris = Vec::new();
    for j in 0..segs {
        tris.push([0, id(1, j), id(1, j + 1)]);
        tris.push([south, id(rings - 1, j + 1), id(rings - 1, j)]);
    }
    for i in 1..rings - 1 {
        for j in 0..segs {
            tris.push([id(i, j), id(i + 1, j), id(i + 1, j + 1)]);
            tris.push([id(i, j), id(i + 1, j + 1), id(i, j + 1)]);
        }
    }
    Mesh { verts, tris, ..Default::default() }
}

/// A HEAVY MESH MADE LIGHTER BY ITS NODE: a fine ball of 57 000 triangles turned into a solid within 0.01 mm comes out a
/// fraction of the faces, of the same volume. Reported behaviour: a body made from a mesh as it is hung the window on
/// every operation after it.
#[test]
fn a_heavy_mesh_becomes_a_lighter_solid_of_its_volume() {
    let mut p = Project::default();
    p.new_document();
    let source = p.add_source("ball.stl", vec![1]);
    let fine = ball(10.0, 120, 240);
    let src = p.add_mesh(fine.clone());
    p.import_tree_as_parts(vec![ImportNode { name: "ball".into(), body: Some(src), mesh: true, ..Default::default() }], source, "ball").expect("came in");
    let body = p.add_mesh_solid(src, 0.01);
    let (_report, shapes) = qymcad_testkit::regenerate(&mut p);
    let solid = shapes.get(&body).unwrap_or_else(|| panic!("no live solid for the node; the rebuild says {:?}", p.regen_errors));
    let want: f64 = fine
        .tris
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|v| fine.verts[v as usize]);
            (a.x * (b.y * c.z - b.z * c.y) - a.y * (b.x * c.z - b.z * c.x) + a.z * (b.x * c.y - b.y * c.x)) / 6.0
        })
        .sum();
    let faces = p.regen_faces.get(&body).map(|f| f.len()).unwrap_or(0);
    assert!(faces * 3 <= fine.tris.len(), "the solid has {faces} faces of a mesh of {} triangles", fine.tris.len());
    assert!((solid.volume() - want).abs() < 0.005 * want, "the solid holds {} against the mesh's {want}", solid.volume());
}

/// A cylinder of `r` by `h` in `segs` x `rows` quads with fanned caps, its corners shared.
fn cylinder(r: f64, h: f64, segs: usize, rows: usize) -> Mesh {
    let mut verts = Vec::new();
    for i in 0..=rows {
        for j in 0..segs {
            let a = std::f64::consts::TAU * j as f64 / segs as f64;
            verts.push(Point3::new(r * a.cos(), r * a.sin(), h * i as f64 / rows as f64));
        }
    }
    let (bottom, top) = (verts.len() as u32, verts.len() as u32 + 1);
    verts.push(Point3::new(0.0, 0.0, 0.0));
    verts.push(Point3::new(0.0, 0.0, h));
    let id = |i: usize, j: usize| (i * segs + j % segs) as u32;
    let mut tris = Vec::new();
    for i in 0..rows {
        for j in 0..segs {
            tris.push([id(i, j), id(i, j + 1), id(i + 1, j + 1)]);
            tris.push([id(i, j), id(i + 1, j + 1), id(i + 1, j)]);
        }
    }
    for j in 0..segs {
        tris.push([bottom, id(0, j + 1), id(0, j)]);
        tris.push([top, id(rows, j), id(rows, j + 1)]);
    }
    Mesh { verts, tris, ..Default::default() }
}

/// A MESH MADE LIGHTER IS RECOGNISED WITHIN ITS SIMPLIFICATION: a fine cylinder simplified by 0.01 mm stands off the true
/// cylinder by up to that, and held to a hundred-thousandth of its size no cylinder lay on it - measured on a smooth
/// handle simplified so, 8 544 regions came out of 21 234 triangles. Recognised within the simplification, the
/// cylinder comes back as its three faces.
#[test]
fn a_simplified_mesh_is_recognised_within_its_simplification() {
    let mut p = Project::default();
    p.new_document();
    let source = p.add_source("rod.stl", vec![1]);
    let src = p.add_mesh(cylinder(10.0, 40.0, 360, 80));
    p.import_tree_as_parts(vec![ImportNode { name: "rod".into(), body: Some(src), mesh: true, ..Default::default() }], source, "rod").expect("came in");
    let body = p.add_mesh_recognised(src, 1.0, 30.0, 0.01);
    let (_report, shapes) = qymcad_testkit::regenerate(&mut p);
    let solid = shapes.get(&body).unwrap_or_else(|| panic!("no live solid for the node; the rebuild says {:?}", p.regen_errors));
    let faces = p.regen_faces.get(&body).map(|f| f.len()).unwrap_or(0);
    let want = std::f64::consts::PI * 100.0 * 40.0;
    assert!(faces == 3, "the rod came back as {faces} faces, not its side and two ends");
    assert!((solid.volume() - want).abs() < 0.005 * want, "the rod holds {} against {want}", solid.volume());
}
