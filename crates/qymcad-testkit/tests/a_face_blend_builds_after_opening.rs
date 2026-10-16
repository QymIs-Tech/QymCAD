//! A FILLET OR A CHAMFER OF "EVERY EDGE OF THIS FACE" BUILDS ON A DOCUMENT JUST OPENED.
//!
//! A click on a face is stored as the query `Adjacent(Id(face))`. Opening a bundle brings back the faces of a
//! body and its live B-rep, not its edges: `regen_edges` is derived and filled by the post pass of a rebuild.
//! Reported behaviour: on a cylinder opened from a file a fillet of the top face was refused with "Not one of
//! the 1 named edges is left in the body", while the same steps on a cylinder built in the session worked. A
//! chamfer resolves its edges through the same path.
use qymcad_core::errors::CoreError;
use qymcad_core::model::{Id, Project};
use qymcad_core::refs::{Query, Ref};

/// The blend laid on the edges of a face.
#[derive(Clone, Copy, Debug)]
enum Blend {
    Fillet,
    Chamfer,
}

impl Blend {
    /// Lay the blend of size 2 on `edges` of `body`; returns its node.
    fn add(self, p: &mut Project, body: Id, edges: Ref) -> Id {
        match self {
            Blend::Fillet => p.add_fillet_ref(body, 2.0, edges),
            Blend::Chamfer => p.add_chamfer_ref(body, 2.0, edges),
        }
    }
}

/// A cylinder R10 x 20 written into a bundle with its live body and opened again, the way the window opens it:
/// the live bodies seeded from the bundle, `regen_edges` empty.
struct Opened {
    project: Project,
    shapes: std::collections::HashMap<Id, qymcad_kernel::Shape>,
    body: Id,
}

fn a_cylinder_opened_from_a_file(tag: &str) -> Opened {
    let mut p = Project::default();
    p.new_document();
    let body = p.add_cylinder(10.0, 20.0);
    let (rep, shapes) = qymcad_testkit::regenerate(&mut p);
    assert!(rep.errors.is_empty(), "setup: the cylinder builds: {:?}", rep.errors);
    for (b, faces) in p.regen_faces.clone() {
        if let Some(i) = p.mesh_index(b) {
            p.bodies[i].faces = faces;
        }
    }
    let path = std::env::temp_dir().join(format!("qym-face-blend-{tag}-{}.qcad", std::process::id())).to_string_lossy().into_owned();
    qymcad_io::save_project_with_brep(&p, &path, &shapes.iter().filter_map(|(id, s)| s.to_brep_bytes().map(|b| (*id, b))).collect::<Vec<_>>()).expect("the document is written");
    let opened = qymcad_io::load_project_with_brep(&path).expect("the document opens");
    let _ = std::fs::remove_file(&path);
    let shapes = opened.breps.iter().filter_map(|(id, b)| qymcad_kernel::specimen::read(b).map(|s| (*id, s))).collect();
    let mut p = opened.project;
    for i in 0..p.bodies.len() {
        if let (Some(b), false) = (p.mesh_id(i), p.bodies[i].faces.is_empty()) {
            let faces = p.bodies[i].faces.clone();
            p.regen_faces.insert(b, faces);
        }
    }
    assert!(!p.regen_edges.contains_key(&body), "GUARD: an opened document holds no edges of the cylinder, otherwise there is no trap");
    Opened { project: p, shapes, body }
}

/// The top face of the cylinder: the one whose normal points up.
fn top_face(p: &Project, body: Id) -> u32 {
    p.regen_faces[&body].iter().find(|f| f.normal[2] > 0.9).expect("the top face of the cylinder").id
}

/// The blend of the top face builds on the top rim alone: a cylinder has 3 faces, a blend of one rim adds 1, of both
/// rims 2.
fn builds_on_a_cylinder_just_opened(blend: Blend) {
    let Opened { project: mut p, shapes, body } = a_cylinder_opened_from_a_file(&format!("builds-{blend:?}"));
    let top = top_face(&p, body);
    let f = blend.add(&mut p, body, Ref::many(Query::Adjacent(Box::new(Query::Id(top)))));
    let (rep, shapes) = qymcad_testkit::regenerate_dirty_with_shapes(&mut p, shapes);
    assert!(rep.errors.is_empty(), "a {blend:?} of the top face of a cylinder just opened must build: {:?}", rep.errors);
    let faces = p.regen_faces.get(&f).map(|fs| fs.len()).unwrap_or(0);
    assert_eq!(faces, 4, "the {blend:?} takes the top rim alone: 3 faces of the cylinder and 1 of its own, and there are {faces}");
    let full = std::f64::consts::PI * 100.0 * 20.0;
    let v = shapes.get(&f).map(|s| s.volume()).unwrap_or(full);
    assert!(v < full - 1.0, "the {blend:?} takes material away: {v:.2} against {full:.2}");
}

/// A FACE THAT IS GONE IS NOT "A NAMED EDGE". The query names a face; the refusal must not count it as an edge
/// whose name came from above, and it must not take every edge either.
fn says_so_in_its_own_words_when_its_face_is_gone(blend: Blend) {
    let Opened { project: mut p, shapes, body } = a_cylinder_opened_from_a_file(&format!("gone-{blend:?}"));
    let gone = 0x6000_7777; // a name no face of the cylinder has ever carried
    let f = blend.add(&mut p, body, Ref::many(Query::Adjacent(Box::new(Query::Id(gone)))));
    let (rep, _) = qymcad_testkit::regenerate_dirty_with_shapes(&mut p, shapes);
    let said = rep.errors.iter().find(|(id, _)| *id == f).map(|(_, e)| e.clone());
    assert!(matches!(said, Some(CoreError::DescribedEdgesNotFound)), "a {blend:?} of the edges of a lost face says {said:?}");
}

#[test]
fn a_face_fillet_builds_on_a_cylinder_just_opened() {
    builds_on_a_cylinder_just_opened(Blend::Fillet);
}

#[test]
fn a_face_chamfer_builds_on_a_cylinder_just_opened() {
    builds_on_a_cylinder_just_opened(Blend::Chamfer);
}

#[test]
fn a_face_fillet_whose_face_is_gone_says_so_in_its_own_words() {
    says_so_in_its_own_words_when_its_face_is_gone(Blend::Fillet);
}

#[test]
fn a_face_chamfer_whose_face_is_gone_says_so_in_its_own_words() {
    says_so_in_its_own_words_when_its_face_is_gone(Blend::Chamfer);
}
