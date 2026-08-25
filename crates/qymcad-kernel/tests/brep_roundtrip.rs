//! A live body survives being written to a file, together with the names of its faces.
//!
//! The complaint was that making a cut in one part meant waiting for a rebuild. A measurement found the culprit:
//! an edit in an already built project costs a second, while the first operation after opening a file rebuilds
//! the whole timeline — the bundle holds meshes and faces, no body has a live B-rep, and the kernel builds
//! everything anew, taking 13.4 seconds over the 28 nodes of that file.
//!
//! The cure is what a professional CAD does: put the body itself into the file. Face names, however, do not go
//! into a BRep file — they live in separate maps of the kernel. They are written alongside, by their index in
//! the traversal. If that traversal order did not survive a write and read, fillets and chamfers would land on
//! other edges — silently and indistinguishably from correct work. So the check here is not that the file opens
//! but that the names hold.
use qymcad_kernel::Shape;

fn cube() -> Shape {
    Shape::extrude(&[0.0, 0.0, 20.0, 0.0, 20.0, 20.0, 0.0, 20.0], 20.0).expect("the cube")
}

/// The faces of a body as a name mapped to where it points and where its centre is: this is what identifies
/// whether the right face received the name.
fn named_faces(s: &Shape) -> Vec<(u32, [i64; 3], [i64; 3])> {
    let bodies = s.tessellate_auto(qymcad_core::model::GeomQuality::Normal.deflection_k());
    let mut out: Vec<(u32, [i64; 3], [i64; 3])> = bodies
        .first()
        .map(|(_, faces)| {
            faces
                .iter()
                .map(|f| {
                    let r = |v: f64| (v * 1e3).round() as i64; // a micron grid: numbers are compared, not bits
                    (f.id, [r(f.centroid.x), r(f.centroid.y), r(f.centroid.z)], [r(f.normal[0]), r(f.normal[1]), r(f.normal[2])])
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_unstable();
    out
}

/// Setup: the write and read round trip happened at all, and the body is the same.
#[test]
fn a_body_survives_the_round_trip() {
    let c = cube();
    let bytes = c.to_brep_bytes().expect("the body has to write");
    assert!(bytes.len() > 16, "the blob is suspiciously small: {}", bytes.len());
    let back = Shape::from_brep_bytes(&bytes).expect("the body has to read back");
    assert!(back.is_valid(), "the body that was read back is invalid");
    assert!((back.volume() - c.volume()).abs() < 1e-6, "the volume drifted: it was {} and became {}", c.volume(), back.volume());
}

/// The main point: the face names are the same. Were they to drift, chamfers and fillets would land in the
/// wrong place.
#[test]
fn face_names_survive_the_round_trip() {
    let c = cube();
    let before = named_faces(&c);
    assert_eq!(before.len(), 6, "setup: a cube has six named faces");
    let back = Shape::from_brep_bytes(&c.to_brep_bytes().expect("writing")).expect("reading");
    let after = named_faces(&back);
    assert_eq!(
        before, after,
        "the face names drifted after the round trip: fillets and chamfers will land on other faces silently\nbefore: {before:?}\nafter:  {after:?}"
    );
}

/// And after operations too: a body with changed topology is exactly where names live.
#[test]
fn names_survive_on_a_body_that_was_already_worked_on() {
    let c = cube();
    let split = c.split_faces([0.0, 0.0, 10.0], [0.0, 0.0, 1.0]).expect("the faces were split");
    let before = named_faces(&split);
    assert!(before.len() >= 10, "setup: the split added faces, and the count came out as {}", before.len());
    let back = Shape::from_brep_bytes(&split.to_brep_bytes().expect("writing")).expect("reading");
    assert_eq!(before, named_faces(&back), "the names on a modified body did not survive the round trip");
}

/// A foreign blob gives an honest refusal rather than a ghost body.
#[test]
fn a_foreign_blob_is_refused() {
    assert!(Shape::from_brep_bytes(b"").is_none(), "an empty blob has to be rejected");
    assert!(Shape::from_brep_bytes(b"NOPE not a body at all").is_none(), "a foreign blob has to be rejected");
}
