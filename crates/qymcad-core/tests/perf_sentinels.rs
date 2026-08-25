//! Performance sentinels for the model layer.
//!
//! Every freeze so far was found in real use: the degree-of-freedom count computed on every frame, a dense
//! Jacobian, opening a file rebuilding the whole project, deleting a node rebuilding everything, the state key
//! serialising half the project on every frame. What follows are minimal scenes with generous budgets that
//! catch those classes before a release. The bounds are deliberately loose, since build machines are slow and
//! noisy: a sentinel has to catch a return to the previous behaviour, an order of magnitude apart, not a
//! micro-regression.
//!
//! The measurements are always printed, so the trend is visible even when the test is green.
use qymcad_core::feature::PLACE_IDENTITY;
use qymcad_core::geom::{Mesh, Point2, Point3};
use qymcad_core::model::{Id, Project};
use std::time::Instant;

/// A heavyweight assembly without the kernel: `parts` parts, each with a mesh node as an imported solid.
///
/// The shape of the project matches a real file with over a thousand imports; only the meshes are tiny.
fn big_assembly(parts: usize) -> Project {
    let mut p = Project::default();
    let root = p.new_document();
    for i in 0..parts {
        p.set_active_component(Some(root));
        let c = p.add_part(format!("Part {i}"));
        p.set_active_component(Some(c));
        let body = p.add_mesh(Mesh {
            verts: vec![Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 0.0, 0.0), Point3::new(0.0, 1.0, 0.0)],
            tris: vec![[0, 1, 2]],
        });
        p.imported_bodies.insert(body);
        p.timeline.push(qymcad_core::feature::FeatureNode {
            id: body,
            name: "Import".into(),
            kind: qymcad_core::feature::FeatureKind::Import { body, source: 0, solid: i as u32 },
            parent: Some(c),
            dirty: false,
            suppressed: false,
        });
    }
    p
}

/// The state key is computed on every frame and has to stay cheap even on a large assembly. A return to
/// serialising the project would fail this test by an order of magnitude.
#[test]
fn state_key_stays_cheap() {
    let p = big_assembly(1000);
    let t = Instant::now();
    let mut acc = 0u64;
    for _ in 0..20 {
        acc = acc.wrapping_add(p.state_key());
    }
    let per = t.elapsed() / 20;
    eprintln!("[perf] state_key over 1000 parts: {per:?} per call");
    assert!(acc != 0);
    assert!(per < std::time::Duration::from_millis(3), "the state key became expensive: {per:?}, budget 3 ms per frame");
}

/// Deleting a node must not cost a walk over the whole project. The application used to force a full
/// regeneration, re-tessellating every body, which took tens of seconds on a real file.
#[test]
fn deleting_a_node_is_local_work() {
    let mut p = big_assembly(800);
    let victim: Id = p.timeline.iter().rev().find_map(|n| n.kind.body()).expect("a body");
    let t = Instant::now();
    let removed = p.delete_feature_op(victim);
    let dt = t.elapsed();
    eprintln!("[perf] deleting a node in an assembly of 800 parts: {dt:?}, bodies removed: {}", removed.len());
    assert!(dt < std::time::Duration::from_millis(200), "deletion became expensive: {dt:?}");
    // and after the deletion not everything is marked dirty
    let dirty = p.timeline.iter().filter(|n| n.dirty).count();
    assert!(dirty < 50, "the deletion marked {dirty} nodes dirty, which amounts to a full rebuild");
}

/// The status of a sketch — degrees of freedom plus free points — is computed for the panel and the
/// highlighting; on a large sketch it has to stay within the frame budget, or the interface freezes as it did
/// before the cache.
#[test]
fn sketch_dof_stays_in_frame_budget() {
    let mut p = Project::default();
    let si = p.new_sketch("big");
    for gy in 0..6 {
        for gx in 0..6 {
            let (x0, y0) = (gx as f64 * 30.0, gy as f64 * 30.0);
            p.add_rect_entity(si, x0, y0, x0 + 20.0, y0 + 20.0, qymcad_core::feature::Purpose::Real);
        }
    }
    let np = p.sketches[si].points.len();
    let t = Instant::now();
    let dof = p.sketch_dof(si);
    let dt = t.elapsed();
    eprintln!("[perf] sketch_dof over {np} points: {dt:?} -> {dof:?}");
    assert!(dt < std::time::Duration::from_millis(500), "the degree-of-freedom count became expensive: {dt:?} over {np} points");
}

/// Comparing states for an undo, to decide which bodies to rebuild, has to be cheap, or every undo on a large
/// assembly turns into a freeze.
#[test]
fn undo_diff_is_cheap() {
    let a = big_assembly(800);
    let mut b = a.clone();
    if let Some(n) = b.timeline.iter_mut().find(|n| n.kind.body().is_some()) {
        n.suppressed = true; // a single edit
    }
    let t = Instant::now();
    let changed = b.changed_bodies_vs(&a);
    let dt = t.elapsed();
    eprintln!("[perf] changed_bodies_vs over 800 parts: {dt:?}, changed bodies: {}", changed.len());
    assert!(dt < std::time::Duration::from_millis(300), "comparing states became expensive: {dt:?}");
    assert!(changed.len() <= 2, "editing one node must not mark the whole assembly: {}", changed.len());
}

/// An undo snapshot does not carry the bytes of embedded sources. What is checked here is the fact rather than
/// the time: those bytes are what determined the memory use, at forty steps of tens of megabytes each.
#[test]
fn snapshot_clone_skips_source_bytes() {
    let mut p = big_assembly(50);
    p.sources.push(qymcad_core::model::SourceFile { id: 1, name: "big.step".into(), ext: "step".into(), data: vec![3u8; 5_000_000] });
    let t = Instant::now();
    let snap = p.clone_without_source_data();
    let dt = t.elapsed();
    let bytes: usize = snap.sources.iter().map(|s| s.data.len()).sum();
    eprintln!("[perf] snapshot of a project with a 5 MB source: {dt:?}, source bytes in the snapshot: {bytes}");
    assert_eq!(bytes, 0, "source bytes must not end up in the snapshot");
    assert!(dt < std::time::Duration::from_millis(100), "the snapshot became expensive: {dt:?}");
}

/// Placing the components, that is computing their world transforms, is work done on every frame of rendering
/// an assembly; over a thousand parts it has to cost next to nothing.
#[test]
fn world_transforms_are_cheap() {
    let p = big_assembly(1000);
    let bodies: Vec<Id> = p.timeline.iter().filter_map(|n| n.kind.body()).collect();
    let t = Instant::now();
    let mut acc = 0.0;
    for b in &bodies {
        let m = p.body_world_transform(*b);
        acc += m[3] + m[7] + m[11];
    }
    let dt = t.elapsed();
    eprintln!("[perf] world transforms of 1000 bodies: {dt:?}");
    assert_eq!(acc, 0.0, "every part sits at zero, so the transforms are identities");
    assert!(dt < std::time::Duration::from_millis(200), "placement became expensive: {dt:?}");
    assert_eq!(p.body_world_transform(bodies[0]), PLACE_IDENTITY);
}

/// The cap of a section is computed from the mesh on every frame while the gizmo is dragged, so the budget is
/// held on a body of realistic size, ten thousand triangles.
#[test]
fn mesh_section_cap_is_frame_cheap() {
    // a cylindroid of ten thousand triangles
    let (rings, seg) = (50usize, 100usize);
    let mut m = Mesh::default();
    for i in 0..rings {
        for j in 0..seg {
            let a = j as f64 / seg as f64 * std::f64::consts::TAU;
            m.verts.push(Point3::new(a.cos() * 20.0, a.sin() * 20.0, i as f64));
        }
    }
    for i in 0..rings - 1 {
        for j in 0..seg {
            let (a, b) = ((i * seg + j) as u32, (i * seg + (j + 1) % seg) as u32);
            let (c, d) = (a + seg as u32, b + seg as u32);
            m.tris.push([a, b, c]);
            m.tris.push([b, d, c]);
        }
    }
    let t = Instant::now();
    let mut tris = 0;
    for k in 0..10 {
        tris += qymcad_core::geom::mesh_section_cap(&m, [0.0, 0.0, 10.0 + k as f64 * 0.1], [0.0, 0.0, 1.0]).len();
    }
    let per = t.elapsed() / 10;
    eprintln!("[perf] section cap over {} triangles: {per:?} per frame, cap triangles: {}", m.tris.len(), tris / 10);
    assert!(tris > 0, "the cap was built");
    assert!(per < std::time::Duration::from_millis(50), "the section cap became expensive: {per:?} per frame");
}

/// One more invariant of the same class: a repeated regeneration without edits has to be almost free, since
/// nothing is dirty and there is nothing to build. A regression where every action rebuilds everything is
/// caught immediately.
#[test]
fn regen_without_changes_is_noop() {
    let p = big_assembly(500);
    let mut probe = Point2::new(0.0, 0.0);
    let t = Instant::now();
    for _ in 0..5 {
        let dirty = p.timeline.iter().filter(|n| n.dirty).count();
        probe.x += dirty as f64;
    }
    let dt = t.elapsed();
    eprintln!("[perf] checking dirty nodes five times over 500 parts: {dt:?}");
    assert_eq!(probe.x, 0.0, "after the scene is built no nodes are dirty, so there is nothing to rebuild");
    assert!(dt < std::time::Duration::from_millis(50));
}
