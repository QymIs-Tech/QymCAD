//! PLY: WRITTEN AND READ BACK the same mesh, and the files scanners write - text, binary in either byte order,
//! with the properties a scanner adds - read as they mean.
use qymcad_core::geom::{Mesh, Point3};
use qymcad_io::{export_ply, import_ply, import_ply_coloured};

/// A folder for the files, under `target`: nothing goes to `/tmp`, which lives in memory.
fn file(name: &str, bytes: Option<&[u8]>) -> String {
    let dir = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/ply-probe"));
    std::fs::create_dir_all(&dir).expect("a folder for the check");
    let p = dir.join(name);
    if let Some(b) = bytes {
        std::fs::write(&p, b).expect("written");
    }
    p.to_string_lossy().into_owned()
}

fn tetra(at: f64) -> Mesh {
    let v = |x: f64, y: f64, z: f64| Point3::new(at + x, y, z);
    Mesh { verts: vec![v(0.0, 0.0, 0.0), v(10.0, 0.0, 0.0), v(0.0, 10.0, 1.0 / 3.0), v(0.1, 0.2, 10.000000000000002)], tris: vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]], ..Default::default() }
}

/// TWO BODIES GO OUT AS ONE MESH AND COME BACK to the last bit - PLY holds one mesh, so they come back together.
#[test]
fn a_mesh_comes_back_to_the_last_bit() {
    let p = file("two.ply", None);
    export_ply(&[tetra(0.0), tetra(30.0)], &p).expect("the PLY is written");
    let back = import_ply(&p).expect("the PLY reads back");
    let mut want = tetra(0.0);
    let second = tetra(30.0);
    want.tris.extend(second.tris.iter().map(|t| t.map(|i| i + 4)));
    want.verts.extend(second.verts);
    assert_eq!(back.tris, want.tris, "the triangles came back renumbered");
    assert_eq!(back.verts.len(), want.verts.len());
    for (p, q) in want.verts.iter().zip(&back.verts) {
        assert!(p.x == q.x && p.y == q.y && p.z == q.z, "a vertex came back moved: {p:?} -> {q:?}");
    }
}

/// A SCANNER'S TEXT FILE: normals and colours around the coordinates, a quad, a comment - read past, fanned.
#[test]
fn a_scanners_text_file_reads_as_it_means() {
    let text = "ply\nformat ascii 1.0\ncomment made by a scanner\nelement vertex 4\nproperty float x\nproperty float nx\nproperty float y\nproperty float ny\nproperty float z\nproperty float nz\nproperty uchar red\nproperty uchar green\nproperty uchar blue\nelement face 1\nproperty list uchar int vertex_indices\nproperty uchar flags\nend_header\n\
                0 0 0 0 0 1 255 0 0\n10 0 0 0 0 1 0 255 0\n10 0 10 0 0 1 0 0 255\n0 0 10 0 0 1 9 9 9\n4 0 1 2 3 7\n";
    let m = import_ply(&file("scan.ply", Some(text.as_bytes()))).expect("reads");
    assert_eq!(m.verts.len(), 4);
    assert_eq!((m.verts[2].x, m.verts[2].y, m.verts[2].z), (10.0, 10.0, 0.0), "y and z were taken from the wrong columns");
    assert_eq!(m.tris, vec![[0, 1, 2], [0, 2, 3]], "a quad is two triangles fanned from its first corner");
}

/// A BIG-ENDIAN BINARY FILE in floats, with a list counted by a byte - built here byte by byte.
#[test]
fn a_big_endian_binary_file_reads_as_it_means() {
    let mut b = b"ply\nformat binary_big_endian 1.0\nelement vertex 3\nproperty float x\nproperty float y\nproperty float z\nproperty uchar quality\nelement face 1\nproperty list uchar ushort vertex_indices\nend_header\n".to_vec();
    for (x, y, z) in [(0.0f32, 0.0f32, 0.0f32), (2.5, 0.0, 0.0), (0.0, -4.0, 1.5)] {
        for c in [x, y, z] {
            b.extend_from_slice(&c.to_be_bytes());
        }
        b.push(200);
    }
    b.push(3);
    for i in [0u16, 1, 2] {
        b.extend_from_slice(&i.to_be_bytes());
    }
    let m = import_ply(&file("big.ply", Some(&b))).expect("reads");
    assert_eq!((m.verts[2].x, m.verts[2].y, m.verts[2].z), (0.0, -4.0, 1.5), "the bytes were read in the wrong order");
    assert_eq!(m.tris, vec![[0, 1, 2]]);
}

/// A BROKEN FILE IS REFUSED BY NAME, never read as something else.
#[test]
fn a_broken_file_is_refused_by_name() {
    let head = "ply\nformat ascii 1.0\nelement vertex 3\nproperty float x\nproperty float y\nproperty float z\nelement face 1\nproperty list uchar int vertex_indices\nend_header\n";
    let cases = [
        ("not-ply.ply", "solid a\nendsolid a\n".to_string(), "io-ply-not-ply"),
        ("truncated.ply", format!("{head}0 0 0\n1 0 0\n"), "io-ply-truncated"),
        ("bad-index.ply", format!("{head}0 0 0\n1 0 0\n0 1 0\n3 0 1 7\n"), "io-ply-bad-index"),
        ("no-format.ply", "ply\nelement vertex 0\nend_header\n".to_string(), "io-ply-bad-header"),
        ("no-faces.ply", "ply\nformat ascii 1.0\nelement vertex 1\nproperty float x\nproperty float y\nproperty float z\nend_header\n0 0 0\n".to_string(), "io-ply-no-faces"),
    ];
    for (name, text, code) in cases {
        let got = import_ply(&file(name, Some(text.as_bytes())));
        let tris = got.as_ref().map(|m| m.tris.len()).ok();
        assert_eq!(got.err().as_deref(), Some(code), "{name} came back as {tris:?} triangles");
    }
    assert!(export_ply(&[], &file("nothing.ply", None)).is_err(), "an empty set was written as a file");
}

/// A PLY THAT COLOURS ITS FACES comes in in their colours: the mesh in the one most of its triangles have, and a colour
/// per triangle - a quad's colour on both triangles it fans into. The colour stands after the corners here; the owner's
/// print head writes it before them, and the door's own check reads that order.
#[test]
fn a_ply_that_colours_its_faces_comes_in_in_their_colours() {
    let text = "ply\nformat ascii 1.0\nelement vertex 5\nproperty float x\nproperty float y\nproperty float z\nelement face 4\nproperty list uchar int vertex_indices\nproperty uchar red\nproperty uchar green\nproperty uchar blue\nend_header\n\
0 0 0\n10 0 0\n10 10 0\n0 10 0\n5 5 8\n\
4 0 1 2 3 26 204 26\n3 0 1 4 204 26 26\n3 1 2 4 204 26 26\n3 2 3 4 204 26 26\n";
    let back = import_ply_coloured(&file("coloured.ply", Some(text.as_bytes()))).expect("the PLY reads");
    let (red, green) = ([204, 26, 26], [26, 204, 26]);
    assert_eq!(back.color, Some(red), "the mesh does not come in in the colour most of its triangles have");
    assert_eq!(back.tri_colors, [green, green, red, red, red], "the triangles do not come in in their faces' colours");
}

/// A PLY THAT COLOURS ITS VERTICES comes in in their colours, a triangle in the colour most of its corners have and its
/// first corner's where all three differ; where the faces carry colours of their own, those stand.
#[test]
fn a_ply_that_colours_its_vertices_comes_in_in_their_colours() {
    let (red, green, blue) = ([204, 26, 26], [26, 204, 26], [26, 51, 230]);
    let head = "ply\nformat ascii 1.0\nelement vertex 6\nproperty float x\nproperty float y\nproperty float z\nproperty uchar red\nproperty uchar green\nproperty uchar blue\n";
    let verts = "0 0 0 26 204 26\n10 0 0 204 26 26\n10 10 0 26 204 26\n0 10 0 26 204 26\n5 5 8 204 26 26\n0 20 0 26 51 230\n";
    // the base green, a corner of it and the apex red, one more point blue
    let faces = "4 0 1 2 3\n3 0 1 4\n3 1 2 4\n3 2 3 4\n3 3 4 5\n";
    let text = format!("{head}element face 5\nproperty list uchar int vertex_indices\nend_header\n{verts}{faces}");
    let back = import_ply_coloured(&file("vertex-coloured.ply", Some(text.as_bytes()))).expect("the PLY reads");
    assert_eq!(back.tri_colors, [green, green, red, red, green, green], "the triangles do not come in in their corners' colours");
    assert_eq!(back.color, Some(green), "the mesh does not come in in the colour most of its triangles have");
    // the faces' own colours stand over the vertices'
    let faces = "4 0 1 2 3 26 51 230\n3 0 1 4 26 51 230\n3 1 2 4 26 51 230\n3 2 3 4 26 51 230\n3 3 4 5 204 26 26\n";
    let text = format!("{head}element face 5\nproperty list uchar int vertex_indices\nproperty uchar red\nproperty uchar green\nproperty uchar blue\nend_header\n{verts}{faces}");
    let back = import_ply_coloured(&file("both-coloured.ply", Some(text.as_bytes()))).expect("the PLY reads");
    assert_eq!(back.tri_colors, [blue, blue, blue, blue, blue, red], "the faces' colours do not stand over the vertices'");
}

/// A PLY OF MANY COLOURS READS IN TIME: a scan colours its faces in tens of thousands of colours, and counting them must
/// not go over every colour met for every face - 60 000 faces, each of its own colour, read in under 3 s.
#[test]
fn a_ply_of_many_colours_reads_in_time() {
    let n = 60_000u32;
    let mut text = format!("ply\nformat ascii 1.0\nelement vertex {}\nproperty float x\nproperty float y\nproperty float z\nelement face {n}\nproperty list uchar int vertex_indices\nproperty uchar red\nproperty uchar green\nproperty uchar blue\nend_header\n", 3 * n);
    for k in 0..n {
        text += &format!("{k} 0 0\n{} 0 0\n{k} 1 0\n", k + 1);
    }
    for k in 0..n {
        text += &format!("3 {} {} {} {} {} {}\n", 3 * k, 3 * k + 1, 3 * k + 2, k % 256, (k / 256) % 256, k / 65536);
    }
    let p = file("many-colours.ply", Some(text.as_bytes()));
    let start = std::time::Instant::now();
    let back = import_ply_coloured(&p).expect("the PLY reads");
    let took = start.elapsed();
    assert_eq!(back.tri_colors.len(), n as usize, "the faces' colours were not all read");
    assert!(took.as_secs_f64() < 3.0, "60 000 faces of their own colours took {took:?} to read");
}
