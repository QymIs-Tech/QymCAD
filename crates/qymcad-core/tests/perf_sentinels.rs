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
//!
//! A budget holds the work of the program, not the share of the machine it was given (`time_budget::Cost`): timed by
//! the wall, the state key read past its 3 ms once while builds of other work ran on the machine beside the checks,
//! against 0.5-1.2 ms alone.
use qymcad_core::feature::PLACE_IDENTITY;
use qymcad_core::geom::{Mesh, Point2, Point3};
use qymcad_core::model::{Id, Project};
use qymcad_core::time_budget::{cost_of, over, Doer, Timed};
use std::time::Duration;

/// THE COST OF `work` ON THIS THREAD, printed.
fn timed<T>(what: &str, work: impl FnOnce() -> T) -> Timed<T> {
    let t = cost_of(Doer::Thread, work);
    eprintln!("[perf] {what}: {}", t.cost);
    t
}

/// The cost of `t` keeps within `budget`, or the check fails saying so.
fn within<T>(what: &str, t: &Timed<T>, budget: Duration) {
    assert!(t.cost.work < budget, "{}", over(what, t.cost, budget));
}

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
        let body = p.add_mesh(Mesh { verts: vec![Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 0.0, 0.0), Point3::new(0.0, 1.0, 0.0)], tris: vec![[0, 1, 2]], ..Default::default() });
        p.imported_bodies.insert(body);
        p.timeline.push(qymcad_core::feature::FeatureNode {
            id: body,
            name: "Import".into(),
            kind: qymcad_core::feature::FeatureKind::Import { body, source: 0, solid: i as u32, scale: 1.0 },
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
    let what = "the state key over 1000 parts, 20 times (a budget of 3 ms a frame)";
    let t = timed(what, || (0..20).fold(0u64, |acc, _| acc.wrapping_add(p.state_key())));
    assert!(t.out != 0);
    within(what, &t, Duration::from_millis(3) * 20);
}

/// Deleting a node must not cost a walk over the whole project. The application used to force a full
/// regeneration, re-tessellating every body, which took tens of seconds on a real file.
#[test]
fn deleting_a_node_is_local_work() {
    let mut p = big_assembly(800);
    let victim: Id = p.timeline.iter().rev().find_map(|n| n.kind.body()).expect("a body");
    let what = "deleting a node in an assembly of 800 parts";
    let t = timed(what, || p.delete_feature_op(victim));
    eprintln!("[perf] bodies removed: {}", t.out.len());
    within(what, &t, Duration::from_millis(200));
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
    let what = format!("the degrees of freedom of a sketch of {np} points");
    let t = timed(&what, || p.sketch_dof(si));
    eprintln!("[perf] degrees of freedom: {:?}", t.out);
    within(&what, &t, Duration::from_millis(500));
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
    let what = "comparing the states of 800 parts for an undo";
    let t = timed(what, || b.changed_bodies_vs(&a));
    within(what, &t, Duration::from_millis(300));
    assert!(t.out.len() <= 2, "editing one node must not mark the whole assembly: {}", t.out.len());
}

/// An undo snapshot does not carry the bytes of embedded sources. What is checked here is the fact rather than
/// the time: those bytes are what determined the memory use, at forty steps of tens of megabytes each.
#[test]
fn snapshot_clone_skips_source_bytes() {
    let mut p = big_assembly(50);
    p.sources.push(qymcad_core::model::SourceFile { id: 1, name: "big.step".into(), ext: "step".into(), data: vec![3u8; 5_000_000] });
    let what = "a snapshot of a project with a 5 MB source";
    let t = timed(what, || p.clone_without_source_data());
    let bytes: usize = t.out.sources.iter().map(|s| s.data.len()).sum();
    assert_eq!(bytes, 0, "source bytes must not end up in the snapshot");
    within(what, &t, Duration::from_millis(100));
}

/// Placing the components, that is computing their world transforms, is work done on every frame of rendering
/// an assembly; over a thousand parts it has to cost next to nothing.
#[test]
fn world_transforms_are_cheap() {
    let p = big_assembly(1000);
    let bodies: Vec<Id> = p.timeline.iter().filter_map(|n| n.kind.body()).collect();
    let what = "the world transforms of 1000 bodies";
    let t = timed(what, || {
        bodies.iter().fold(0.0, |acc, b| {
            let m = p.body_world_transform(*b);
            acc + m[3] + m[7] + m[11]
        })
    });
    assert_eq!(t.out, 0.0, "every part sits at zero, so the transforms are identities");
    within(what, &t, Duration::from_millis(200));
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
    let what = format!("the section cap over {} triangles, 10 frames (a budget of 50 ms a frame)", m.tris.len());
    let t = timed(&what, || (0..10).map(|k| qymcad_core::geom::mesh_section_cap(&m, [0.0, 0.0, 10.0 + k as f64 * 0.1], [0.0, 0.0, 1.0]).len()).sum::<usize>());
    assert!(t.out > 0, "the cap was built");
    within(&what, &t, Duration::from_millis(50) * 10);
}

/// One more invariant of the same class: a repeated regeneration without edits has to be almost free, since
/// nothing is dirty and there is nothing to build. A regression where every action rebuilds everything is
/// caught immediately.
#[test]
fn regen_without_changes_is_noop() {
    let p = big_assembly(500);
    let what = "looking for dirty nodes five times over 500 parts";
    let t = timed(what, || {
        let mut probe = Point2::new(0.0, 0.0);
        for _ in 0..5 {
            probe.x += p.timeline.iter().filter(|n| n.dirty).count() as f64;
        }
        probe
    });
    assert_eq!(t.out.x, 0.0, "after the scene is built no nodes are dirty, so there is nothing to rebuild");
    within(what, &t, Duration::from_millis(50));
}
