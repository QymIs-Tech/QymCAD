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
    let kernel = OcctKernel { shapes: std::cell::RefCell::new(shapes), quality_k: project.geom_quality.deflection_k(), ..Default::default() };
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
    let kernel = OcctKernel { shapes: std::cell::RefCell::new(shapes), quality_k: project.geom_quality.deflection_k(), ..Default::default() };
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
    qymcad_kernel::imports::raise_imports(project).into_iter().collect()
}

/// Open a document exactly as it is seen on screen: with the bodies of imports restored and a full rebuild. It
/// returns the report of the regeneration.
pub fn open_like_the_app(project: &mut Project) -> qymcad_core::feature::RegenReport {
    let shapes = restore_import_shapes(project);
    regenerate_with_shapes(project, shapes).0
}

/// The round edge of radius about `r` of `body`: where a person clicks to place a thread or an auger. The project is
/// rebuilt first, so the edges are current. Panics when there is none.
pub fn round_edge(project: &mut Project, body: Id, r: f64) -> u32 {
    let _ = regenerate(project);
    let edges = project.regen_edges.get(&body).cloned().unwrap_or_default();
    edges.iter().find(|e| e.radius > 1e-9 && (e.radius - r).abs() < 0.05).map(|e| e.id).unwrap_or_else(|| panic!("body {body} has no round edge of radius {r}"))
}

/// THE KINDS OF NODE A TIMELINE HOLDS, by the names of the variants of `FeatureKind` as its source declares them. Read
/// from the source rather than listed by hand: a kind added to the enum is a kind the matrices have not seen, and a
/// list kept beside the enum would not know of it.
///
/// The enum stands in `feature.rs` or in one of its modules in `feature/`: each is read, and the one that declares it is
/// taken.
pub fn feature_kinds() -> Vec<String> {
    let core = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../qymcad-core/src");
    let mut files = vec![core.join("feature.rs")];
    files.extend(std::fs::read_dir(core.join("feature")).into_iter().flatten().flatten().map(|e| e.path()));
    let text = files.iter().filter_map(|f| std::fs::read_to_string(f).ok()).find(|t| t.contains("pub enum FeatureKind {")).expect("the source of the kinds of node");
    let body = text.split("pub enum FeatureKind {").nth(1).expect("the enum of the kinds of node");
    let mut kinds = Vec::new();
    for line in body.lines() {
        if line.starts_with('}') {
            break;
        }
        // a variant stands at the indent of the enum's body; its fields and its comments stand deeper or start with `/`
        let Some(rest) = line.strip_prefix("    ") else { continue };
        if rest.starts_with(char::is_uppercase) {
            kinds.push(rest.chars().take_while(|c| c.is_alphanumeric()).collect());
        }
    }
    assert!(kinds.len() >= 40, "the source declares {} kinds of node, fewer than it has - it is read wrong", kinds.len());
    kinds
}

/// The name of the variant of a kind of node.
pub fn kind_name(kind: &qymcad_core::feature::FeatureKind) -> String {
    format!("{kind:?}").chars().take_while(|c| c.is_alphanumeric()).collect()
}

/// The kind of the node of `project` that makes `body`: the last one, as an edit stands after the node it edits.
pub fn kind_making(project: &Project, body: Id) -> Option<String> {
    project.timeline.iter().rev().find(|n| n.kind.bodies().contains(&body)).map(|n| kind_name(&n.kind))
}

/// A KIND OF NODE A MATRIX HAS NO ROW FOR, and why it has none: a node that builds no body by an operation (a sketch, a
/// plane, a copy of a part placed in an assembly) has nothing for the matrix to look at.
pub struct Uncovered {
    pub kind: &'static str,
    pub why: &'static str,
}

/// WHAT A MATRIX LEAVES UNSEEN: every kind of node with neither a row (`rows`, the kinds its rows lay) nor a reason
/// (`uncovered`), every reason given for a kind that has a row after all, and every reason for a kind there is not.
pub fn coverage_gaps(rows: &[&str], uncovered: &[Uncovered]) -> Vec<String> {
    let kinds = feature_kinds();
    let mut gaps = Vec::new();
    for k in &kinds {
        let has_row = rows.contains(&k.as_str());
        let excused = uncovered.iter().any(|u| u.kind == k);
        match (has_row, excused) {
            (false, false) => gaps.push(format!("{k}: no row, and no reason given for none")),
            (true, true) => gaps.push(format!("{k}: has a row and a reason for none")),
            _ => {}
        }
    }
    for k in rows.iter().copied().chain(uncovered.iter().map(|u| u.kind)) {
        if !kinds.iter().any(|x| x == k) {
            gaps.push(format!("{k}: no such kind of node"));
        }
    }
    gaps
}

pub mod real_documents;
