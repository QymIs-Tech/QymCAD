//! A COPIED PART KEEPS ITS REFERENCES TO GEOMETRY.
//!
//! Reported behaviour: copying and pasting a part that has a thread breaks the thread — the part
//! flies off somewhere and fails with an error. That is exactly what one real document holds: node
//! 253, a copy of a threaded part, fails with `ThreadRimNotFound` while its original (node 243)
//! builds.
//!
//! THE CAUSE IS NOT THE THREAD BUT THE CLONE. A face or an edge is addressed not by an id but by a
//! NAME from the recipe: "wall of feature 43 from entity 7". Cloning a subtree honestly remapped the
//! ids (bodies, sketches, datums) and left the names alone — but a copied feature has a different id,
//! so its faces are named differently. The copy's reference kept pointing at an edge of the ORIGINAL,
//! and the copy did not have it.
//!
//! The thread simply shouts loudest (it needs that exact rim or it refuses). A shell, a draft, a
//! fillet and a hole on a face would break the same way, silently — which is why more than the thread
//! is checked here.
use qymcad_core::model::{Id, Project};
use qymcad_core::thread::{ThreadSpec, ThreadStandard};

/// A round edge of radius about `r` on a body — the same thing a person picks with a click.
fn rim(p: &mut Project, body: Id, r: f64) -> u32 {
    let _ = qymcad_testkit::regenerate(p);
    let e = p.regen_edges.get(&body).cloned().unwrap_or_default();
    e.iter()
        .filter(|e| e.radius > 1e-9 && (e.radius - r).abs() < 0.05)
        .map(|e| e.id)
        .next()
        .unwrap_or_else(|| panic!("body {body} has no round edge of radius {r}"))
}

fn metric(d: f64, pitch: f64) -> ThreadSpec {
    ThreadSpec { standard: ThreadStandard::MetricIso, nominal_d: d, pitch, internal: false, fit: 0.2, ..Default::default() }
}

/// A ring-shaped part: a d20x30 shaft with an M20 thread on the top rim. Returns (component, thread body).
fn threaded_part(p: &mut Project) -> (Id, Id) {
    let shaft = p.add_cylinder(10.0, 30.0);
    let e = rim(p, shaft, 10.0);
    let t = p.add_thread(shaft, e, metric(20.0, 2.5), 20.0, 1.0, 1.0);
    let body = p.finish_base_body(t, 1);
    let comp = p.body_owner(body).expect("owner of the part");
    (comp, body)
}

/// The LIVE body of a component: the one not consumed by a later operation (the shaft is eaten by the thread).
fn live_body(p: &Project, comp: Id) -> Id {
    let eaten = p.consumed_bodies();
    p.component_bodies(comp).into_iter().find(|b| !eaten.contains(b)).expect("the live body of the component")
}

/// Body volumes after a full rebuild — they show whether the thread built or not.
fn volumes(p: &mut Project) -> (Vec<(Id, f64)>, Vec<String>) {
    let (report, shapes) = qymcad_testkit::regenerate(p);
    let errs: Vec<String> = report.errors.iter().map(|(id, e)| format!("node {id}: {e:?}")).collect();
    let mut v: Vec<(Id, f64)> = shapes.iter().map(|(id, s)| (*id, s.volume())).collect();
    v.sort_by_key(|(id, _)| *id);
    (v, errs)
}

/// A COPY OF A THREADED PART BUILDS — AND COMES OUT THE SAME SHAPE. Exactly the reported case.
#[test]
fn a_copied_part_with_a_thread_still_builds() {
    let mut p = Project::default();
    p.new_document();
    let (comp, body) = threaded_part(&mut p);
    let (v0, errs) = volumes(&mut p);
    assert!(errs.is_empty(), "the original builds cleanly: {errs:?}");
    let vol_src = v0.iter().find(|(id, _)| *id == body).map(|(_, v)| *v).expect("volume of the original");
    assert!(vol_src > 1.0, "the thread removed something from the shaft: {vol_src}");

    let root = p.root;
    let copy = p.clone_component(comp, root).expect("the copied part");
    let (v1, errs) = volumes(&mut p);
    assert!(errs.is_empty(), "A COPY OF A THREADED PART MUST BUILD: {errs:?}");

    // the body of the part is the one not eaten by a later operation (the shaft is eaten by the thread)
    let live = live_body(&p, copy);
    let vol_copy = v1.iter().find(|(id, _)| *id == live).map(|(_, v)| *v).expect("volume of the copy");
    assert!(
        (vol_copy - vol_src).abs() < vol_src * 1e-6,
        "the copy must be the same shape: original {vol_src:.3}, copy {vol_copy:.3} — the thread did not land on the copy"
    );
}

/// THE COPY'S REFERENCE LEADS TO ITS OWN GEOMETRY, NOT TO THE ORIGINAL.
///
/// The volume check above stays green even if the copy "accidentally" found an edge of the original —
/// and that is a landmine: delete the original and the copy falls apart. So the name itself is
/// examined here.
#[test]
fn the_copy_points_at_its_own_edge_not_at_the_source() {
    use qymcad_core::feature::FeatureKind as FK;
    let mut p = Project::default();
    p.new_document();
    let (comp, _) = threaded_part(&mut p);
    let root = p.root;
    let copy = p.clone_component(comp, root).expect("the copied part");
    let _ = qymcad_testkit::regenerate(&mut p);

    let edge_of = |p: &Project, c: Id| -> u32 {
        p.timeline
            .iter()
            .filter(|n| n.parent == Some(c))
            .find_map(|n| if let FK::Thread { edge, .. } = n.kind { Some(edge) } else { None })
            .expect("the thread node")
    };
    let (src_edge, copy_edge) = (edge_of(&p, comp), edge_of(&p, copy));
    assert_ne!(src_edge, copy_edge, "the copy carried away the NAME of the original's edge — exactly the defect that broke the thread");

    // and that name really belongs to a feature of the copy rather than to someone else's
    let name = p.names.edge(copy_edge).expect("the name of the copy's edge");
    let face = p.names.get(name.faces[0]).expect("the face name from the pair");
    let owner = p.timeline.iter().find(|n| n.id == face.feature).and_then(|n| n.parent);
    assert_eq!(owner, Some(copy), "the copy's edge must be named by ITS feature, not by a feature of the original");
}

/// DELETING THE ORIGINAL DOES NOT BREAK THE COPY. The point of the whole thing: a copy is a part in
/// its own right.
#[test]
fn deleting_the_source_leaves_the_copy_alone() {
    let mut p = Project::default();
    p.new_document();
    let (comp, _) = threaded_part(&mut p);
    let root = p.root;
    let copy = p.clone_component(comp, root).expect("the copied part");
    let (v0, _) = volumes(&mut p);
    let copy_body = live_body(&p, copy);
    let before = v0.iter().find(|(id, _)| *id == copy_body).map(|(_, v)| *v).expect("volume of the copy");

    p.delete_component(comp);
    let (v1, errs) = volumes(&mut p);
    assert!(errs.is_empty(), "after deleting the original the copy builds: {errs:?}");
    let after = v1.iter().find(|(id, _)| *id == copy_body).map(|(_, v)| *v).expect("the copy is still there");
    assert!((after - before).abs() < before * 1e-6, "the shape of the copy does not depend on the original: was {before:.3}, now {after:.3}");
}
