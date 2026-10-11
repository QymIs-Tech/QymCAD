//! OBJ: WRITTEN AND READ BACK, the same meshes - and the files other programs write, read as they mean.
use qymcad_core::geom::{Mesh, Point3};
use qymcad_io::{export_obj, import_obj};

/// A folder for the files, under `target`: nothing goes to `/tmp`, which lives in memory.
fn file(name: &str, text: Option<&str>) -> String {
    let dir = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/obj-probe"));
    std::fs::create_dir_all(&dir).expect("a folder for the check");
    let p = dir.join(name);
    if let Some(t) = text {
        std::fs::write(&p, t).expect("written");
    }
    p.to_string_lossy().into_owned()
}

fn cube(at: f64) -> Mesh {
    let v = |x: f64, y: f64, z: f64| Point3::new(at + x, y, z);
    let verts = vec![v(0.0, 0.0, 0.0), v(10.0, 0.0, 0.0), v(10.0, 10.0, 0.0), v(0.0, 10.0, 0.0), v(0.0, 0.0, 10.0), v(10.0, 0.0, 10.0), v(10.0, 10.0, 10.0), v(0.0, 10.0, 10.0)];
    let tris = vec![[0, 2, 1], [0, 3, 2], [4, 5, 6], [4, 6, 7], [0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6], [3, 0, 4], [3, 4, 7]];
    Mesh { verts, tris, ..Default::default() }
}

/// TWO BODIES GO OUT AND TWO COME BACK, every coordinate to the last bit, every triangle in its place.
#[test]
fn two_bodies_come_back_the_same_to_the_last_bit() {
    let mut odd = cube(30.0);
    odd.verts[6] = Point3::new(40.1, 10.000000000000002, 1.0 / 3.0); // numbers a short decimal form would bend
    let out = [cube(0.0), odd];
    let p = file("two.obj", None);
    export_obj(&out, &p).expect("the OBJ is written");
    let back = import_obj(&p).expect("the OBJ reads back");
    assert_eq!(back.len(), 2, "two bodies went out, {} came back", back.len());
    assert_eq!((back[0].name.as_str(), back[1].name.as_str()), ("body_1", "body_2"), "the objects came back without their names");
    for (a, b) in out.iter().zip(&back) {
        assert_eq!(a.tris, b.mesh.tris, "the triangles came back renumbered");
        for (p, q) in a.verts.iter().zip(&b.mesh.verts) {
            assert!(p.x == q.x && p.y == q.y && p.z == q.z, "a vertex came back moved: {p:?} -> {q:?}");
        }
    }
}

/// WHAT OTHER PROGRAMS WRITE: polygons, indices from the end, `v/vt/vn`, comments, and no `o` at all.
#[test]
fn polygons_back_indices_and_slashes_read_as_they_mean() {
    let text = "# a quad and a triangle, the way many programs write them\n\
                mtllib x.mtl\nv 0 0 0\nv 10 0 0\nv 10 10 0\nv 0 10 0\nvt 0 0\nvn 0 0 1\n\
                usemtl a\ns 1\nf 1/1/1 2/1/1 3/1/1 4/1/1\n\
                v 0 0 5\nf -5 -4 -1\n";
    let back = import_obj(&file("quad.obj", Some(text))).expect("reads");
    assert_eq!(back.len(), 1, "a file without objects is one body");
    assert_eq!(back[0].mesh.tris.len(), 3, "a quad is two triangles and a triangle is one");
    assert_eq!(back[0].mesh.verts.len(), 5, "every used vertex once");
    let z5 = back[0].mesh.verts.iter().position(|p| p.z == 5.0).expect("the vertex counted from the end is there") as u32;
    assert!(back[0].mesh.tris[2].contains(&z5), "the index -1 did not reach the last vertex so far");
}

/// A CONCAVE POLYGON IS CUT INSIDE ITSELF. A fan from the first corner is right only for a convex polygon: a
/// remesher's file of 3 672 quads had 1 077 that the fan folded over their own neighbour. An arrowhead whose notch
/// is the fourth corner, and an L whose inner corner is the second.
#[test]
fn a_concave_polygon_is_cut_inside_itself() {
    let cases = [("dart.obj", "v 0 0 0\nv 4 2 0\nv 0 4 0\nv 1 2 0\nf 1 2 3 4\n", 6.0), ("ell.obj", "v 2 1 0\nv 1 1 0\nv 1 2 0\nv 0 2 0\nv 0 0 0\nv 2 0 0\nf 1 2 3 4 5 6\n", 3.0)];
    for (name, text, area) in cases {
        let m = &import_obj(&file(name, Some(text))).expect("reads")[0].mesh;
        let mut sum = 0.0;
        for i in 0..m.tris.len() {
            let [a, b, c] = m.triangle(i);
            let z = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
            assert!(z > 0.0, "{name}: triangle {i} is folded over: {:?}", m.tris[i]);
            sum += z / 2.0;
        }
        assert!((sum - area).abs() < 1e-9, "{name}: the triangles cover {sum}, the polygon is {area}");
    }
}

/// A BROKEN FILE IS REFUSED BY NAME, never read as something else.
#[test]
fn a_broken_file_is_refused_by_name() {
    let cases = [
        ("out-of-range.obj", "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 9\n", "io-obj-bad-index#4"),
        ("not-a-number.obj", "v 0 zero 0\n", "io-obj-bad-line#1"),
        ("empty.obj", "# nothing\n", "io-obj-no-faces"),
        ("index-zero.obj", "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 0 1 2\n", "io-obj-bad-index#4"),
    ];
    for (name, text, code) in cases {
        let got = import_obj(&file(name, Some(text)));
        let bodies = got.as_ref().map(|m| m.len()).ok();
        assert_eq!(got.err().as_deref(), Some(code), "{name} came back as {bodies:?} bodies");
    }
    assert!(export_obj(&[], &file("nothing.obj", None)).is_err(), "an empty set was written as a file");
}

/// AN OBJECT TAKES THE COLOUR OF ITS MATERIAL: `usemtl` names it, the `.mtl` beside the file gives its `Kd`, taken as
/// sRGB the way programs write it. A name that is only a writer's placeholder ("empty_2") is no name.
#[test]
fn an_object_takes_the_colour_of_its_material() {
    file("colours.mtl", Some("newmtl dark\nKd 0.149 0.149 0.165\nnewmtl red\nKd 0.8 0.1 0.1\n"));
    let obj = "mtllib colours.mtl\nv 0 0 0\nv 1 0 0\nv 0 1 0\nv 5 0 0\nv 6 0 0\nv 5 1 0\no bolt\nusemtl dark\nf 1 2 3\no empty_2\nusemtl red\nf 4 5 6\n";
    let back = import_obj(&file("colours.obj", Some(obj))).expect("reads");
    assert_eq!(back.iter().map(|m| (m.name.as_str(), m.color)).collect::<Vec<_>>(), [("bolt", Some([38, 38, 42])), ("", Some([204, 26, 26]))]);
}
