//! One export policy for every format (`Project::export_kind`).
//!
//! There used to be no policy at all: STEP silently skipped a body without a live B-rep while STL just as
//! silently wrote out its stored mesh, so two files from one project disagreed about their contents. One
//! classification, shared by both formats, has to tell three cases apart: a live B-rep, an imported mesh that
//! never had one, and a failed rebuild where the recipe exists but the B-rep does not.
use qymcad_core::geom::{Mesh, Point2};
use qymcad_core::model::{ExportKind, Id, Project};

fn extruded_body(p: &mut Project, name: &str) -> Id {
    let sid = p.add_line_sketch(name, vec![Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), Point2::new(10.0, 10.0), Point2::new(0.0, 10.0)], true);
    p.add_sketch_node(sid, name);
    let b = p.add_extrude(sid, 5.0);
    p.finish_base_body(b, 1)
}

/// A mesh body without a timeline node: exactly what an STL import produces.
fn imported_mesh_body(p: &mut Project) -> Id {
    let b = p.add_mesh(Mesh::default());
    p.imported_bodies.insert(b);
    b
}

#[test]
fn export_kind_separates_brep_mesh_import_and_failed_regen() {
    let mut p = Project::default();
    p.new_document();
    let feat = extruded_body(&mut p, "s");
    let mesh = imported_mesh_body(&mut p);

    let mut bad = Vec::new();
    // 1. a live B-rep exists, so exact geometry goes into both formats
    if p.export_kind(feat, true) != ExportKind::Brep {
        bad.push(format!("a feature with a live shape gave {:?}, expected Brep", p.export_kind(feat, true)));
    }
    // 2. an imported mesh has no shape and cannot have one, which is not an error: the body is valid
    if p.export_kind(mesh, false) != ExportKind::MeshOnly {
        bad.push(format!("an imported mesh gave {:?}, expected MeshOnly", p.export_kind(mesh, false)));
    }
    // 3. The crucial distinction: the body has a recipe, a timeline node, but no B-rep — a failed rebuild.
    //    This is the case STL used to write out silently, as the last successful mesh, while STEP silently
    //    discarded it.
    if p.export_kind(feat, false) != ExportKind::Stale {
        bad.push(format!("a feature without a shape gave {:?}, expected Stale", p.export_kind(feat, false)));
    }
    // 4. an import with a live B-rep, a STEP solid, is an ordinary Brep rather than a mesh
    if p.export_kind(mesh, true) != ExportKind::Brep {
        bad.push(format!("an imported body with a shape gave {:?}, expected Brep", p.export_kind(mesh, true)));
    }
    assert!(bad.is_empty(), "classification of bodies for export:\n{}", bad.join("\n"));
}

/// The classification survives the deletion of a node: a body with neither a recipe nor a shape is an orphaned
/// mesh rather than a failed rebuild, or the export would report a rebuild error that never happened.
#[test]
fn body_without_timeline_node_is_mesh_only_not_stale() {
    let mut p = Project::default();
    p.new_document();
    let b = extruded_body(&mut p, "s");
    assert_eq!(p.export_kind(b, false), ExportKind::Stale, "while the node lives it is a failed rebuild");
    let node = p.timeline.iter().find(|n| n.kind.body() == Some(b)).map(|n| n.id).expect("the node of the feature");
    p.delete_feature_op(node);
    assert_eq!(p.export_kind(b, false), ExportKind::MeshOnly, "with the node gone the body is no longer a failed rebuild");
}

/// Editing a datum no longer forces a rebuild of the whole project: only the nodes that depend on datums are
/// marked dirty. A body on an ordinary base plane must be left alone, since rebuilding everything on a large
/// assembly means tens of seconds of freeze.
#[test]
fn mark_datum_consumers_dirty_touches_only_datum_dependents() {
    use qymcad_core::feature::SketchPlane;
    use qymcad_core::model::{PlaneDef, WorkPlane};
    let mut p = Project::default();
    p.new_document();
    let plain = extruded_body(&mut p, "on the base XY plane"); // a sketch on World(XY)

    // a datum plane and a body on it
    let pid = p.add_plane(WorkPlane { name: "Datum".into(), origin: [0.0, 0.0, 30.0], normal: [0.0, 0.0, 1.0], def: PlaneDef::Manual, ..Default::default() });
    let sid = p.add_line_sketch("on the datum", vec![Point2::new(0.0, 0.0), Point2::new(5.0, 0.0), Point2::new(5.0, 5.0), Point2::new(0.0, 5.0)], true);
    if let Some(si) = p.sketch_index(sid) {
        p.sketches[si].plane = SketchPlane::Datum(pid);
    }
    p.add_sketch_node(sid, "on the datum");
    let on_datum = p.add_extrude(sid, 3.0);

    for n in &mut p.timeline {
        n.dirty = false;
    }
    p.mark_datum_consumers_dirty();

    let dirty = |b: Id| p.timeline.iter().find(|n| n.kind.body() == Some(b)).is_some_and(|n| n.dirty);
    assert!(dirty(on_datum), "a body on a datum plane has to be rebuilt");
    assert!(!dirty(plain), "a body on the base XY plane need not be touched, or this is the same forced rebuild");
    assert!(p.timeline.iter().any(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::Plane { .. }) && n.dirty), "the datum node itself is dirty too");
}
