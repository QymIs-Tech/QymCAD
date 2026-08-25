//! The reproduction harness: a headless regeneration of a loaded project by the real kernel — the very same
//! `qymcad_kernel::OcctKernel` the application drives, shared code rather than a copy, since the copies had
//! drifted apart in their tessellation and the tests then measured something other than what appears on screen.
//! It exists to debug defects against a particular file.
#![allow(clippy::too_many_arguments, dead_code)]
use qymcad_core::model::{Id, Project};
use qymcad_kernel::OcctKernel;
use std::collections::HashMap;


/// A forced regeneration of the whole project by the real kernel. It returns the report and the cache of live
/// shapes by body.
pub fn regenerate(project: &mut Project) -> (qymcad_core::feature::RegenReport, HashMap<Id, qymcad_kernel::Shape>) {
    regenerate_with_shapes(project, HashMap::new())
}

/// A regeneration without forcing: only what is already marked dirty is rebuilt, which is how the application
/// behaves after a local edit such as changing a dimension or deleting a node. The kernel cache is seeded with
/// ready shapes.
pub fn regenerate_dirty_with_shapes(project: &mut Project, shapes: HashMap<Id, qymcad_kernel::Shape>) -> (qymcad_core::feature::RegenReport, HashMap<Id, qymcad_kernel::Shape>) {
    let _gate = qymcad_kernel::kernel_gate();
    let kernel = OcctKernel { shapes: std::cell::RefCell::new(shapes), quality_k: project.geom_quality.deflection_k() };
    let report = project.regenerate(&kernel);
    (report, kernel.shapes.into_inner())
}

/// As [`regenerate`], but with the kernel cache seeded with ready shapes, which is what the application does
/// when opening a project: imported bodies are restored from the embedded STEP before the rebuild. This is what
/// an honest measurement of opening a large file rests on.
pub fn regenerate_with_shapes(project: &mut Project, shapes: HashMap<Id, qymcad_kernel::Shape>) -> (qymcad_core::feature::RegenReport, HashMap<Id, qymcad_kernel::Shape>) {
    for n in &mut project.timeline {
        if n.kind.body().is_some() {
            n.dirty = true;
        }
    }
    let _gate = qymcad_kernel::kernel_gate();
    let kernel = OcctKernel { shapes: std::cell::RefCell::new(shapes), quality_k: project.geom_quality.deflection_k() };
    let report = project.regenerate(&kernel);
    (report, kernel.shapes.into_inner())
}

/// Restore the live bodies of imports from their embedded sources: the same thing the application does when
/// opening a document, through `restore_import_shapes_for` and `ensure_brep`.
///
/// Why it belongs here. An imported body comes from a STEP file and has no recipe, while a rebuild can only
/// re-tessellate a shape that is already live. A document opened from a bundle has no live shapes — they are
/// raised on demand — so `regen_faces` and `regen_edges` stayed empty for imports. All the derived geometry is
/// taken from those: the principal direction of a face, the axis of a cylinder, the reference direction of an
/// edge. Without them a joint on the face of an imported part has no geometric direction and falls back to the
/// world axes.
///
/// In a real assembly nearly every part is imported, so a headless check without this step measures a different
/// document from the one on screen.
pub fn restore_import_shapes(project: &Project) -> HashMap<Id, qymcad_kernel::Shape> {
    use qymcad_core::feature::FeatureKind;
    let mut by_src: HashMap<Id, Vec<(Id, u32)>> = HashMap::new();
    for n in &project.timeline {
        if let FeatureKind::Import { body, source, solid } = n.kind {
            by_src.entry(source).or_default().push((body, solid));
        }
    }
    let mut out = HashMap::new();
    for (src, items) in by_src {
        let Some(sf) = project.sources.iter().find(|s| s.id == src) else { continue };
        if sf.data.is_empty() {
            continue;
        }
        let ext = if sf.ext.is_empty() { "step" } else { sf.ext.as_str() };
        let tmp = std::env::temp_dir().join(format!("qym_repro_import_{src}.{ext}"));
        if std::fs::write(&tmp, &sf.data).is_err() {
            continue;
        }
        // a shape does not clone: each solid is taken by index exactly once
        let mut shapes: Vec<Option<qymcad_kernel::Shape>> = qymcad_kernel::step_solids(tmp.to_string_lossy().as_ref()).unwrap_or_default().into_iter().map(Some).collect();
        let _ = std::fs::remove_file(&tmp);
        for (body, solid) in items {
            if let Some(s) = shapes.get_mut(solid as usize).and_then(|o| o.take()) {
                out.insert(body, s);
            }
        }
    }
    out
}

/// Open a document exactly as it is seen on screen: with the bodies of imports restored and a full rebuild. It
/// returns the report of the regeneration.
pub fn open_like_the_app(project: &mut Project) -> qymcad_core::feature::RegenReport {
    let shapes = restore_import_shapes(project);
    regenerate_with_shapes(project, shapes).0
}
