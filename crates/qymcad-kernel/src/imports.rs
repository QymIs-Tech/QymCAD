//! THE BODIES BROUGHT IN FROM FILES, RAISED AGAIN FROM THE FILES EMBEDDED IN THE DOCUMENT - on opening a project, and by
//! a check that opens one the way the program does. One way for both: the copy a check once kept of it read every
//! source as STEP and knew nothing of the scale a file came in at.
use qymcad_core::model::{Id, Project};

/// A BODY AS A PROJECT SAVED IT: which body, and the bytes the bridge wrote it as (its shape and the names of its faces
/// and edges).
pub struct SavedBody {
    pub body: Id,
    pub bytes: Vec<u8>,
}

/// A BODY READ BACK.
pub struct ReadBody {
    pub body: Id,
    pub shape: crate::Shape,
}

/// THE BODIES OF A PROJECT READ BACK from the bytes it saved them as, each with the names it was saved with, under the
/// lock of the kernel. A body whose bytes do not read is left out: the document holds its mesh, and a rebuild asks for it.
pub fn restore(saved: Vec<SavedBody>) -> Vec<ReadBody> {
    let _gate = crate::kernel::kernel_gate();
    saved.into_iter().filter_map(|s| crate::Shape::from_brep_bytes(&s.bytes).map(|shape| ReadBody { body: s.body, shape })).collect()
}

/// A BODY BROUGHT IN, AT `factor` from the numbers of its file, about the origin of the file: its faces and edges keep
/// their names. `None` when the shape cannot be scaled.
pub fn scaled(body: &crate::Shape, factor: f64) -> Option<crate::Shape> {
    let _gate = crate::kernel::kernel_gate();
    body.transformed(&[factor, 0.0, 0.0, 0.0, 0.0, factor, 0.0, 0.0, 0.0, 0.0, factor, 0.0])
}

/// Restore the B-rep shapes of the imported solids from the embedded sources (`sources/`): each source is read once and
/// its solids laid out across the bodies, at the scale each came in at, with the names its faces and edges were given
/// when it came in.
///
/// A free function: it runs on a worker thread while a project loads. The imported bodies are stripped on saving as
/// outputs of the timeline; their geometry lives in the original file.
pub fn raise_imports(project: &Project) -> Vec<(Id, crate::Shape)> {
    use qymcad_core::feature::FeatureKind;
    use std::collections::HashMap;
    let mut by_src: HashMap<Id, Vec<Raised>> = HashMap::new();
    for n in &project.timeline {
        if let FeatureKind::Import { body, source, solid, scale } = n.kind {
            by_src.entry(source).or_default().push(Raised { body, solid, scale });
        }
    }
    let mut out = Vec::new();
    for (src, items) in by_src {
        let Some(sf) = project.sources.iter().find(|s| s.id == src) else { continue };
        if sf.data.is_empty() {
            continue;
        }
        let ext = if sf.ext.is_empty() { "step" } else { sf.ext.as_str() };
        // A NAME OF ITS OWN FOR EVERY RAISING: two raisings at once - a document opening while a file comes in, two
        // checks side by side - wrote and removed one and the same file, and one of them read nothing
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tmp = std::env::temp_dir().join(format!("qym_import_{}_{n}_{src}.{ext}", std::process::id()));
        if std::fs::write(&tmp, &sf.data).is_err() {
            continue;
        }
        // read by what the source is: an IGES source read as STEP gave nothing, and the part opened with no live body;
        // a shape is moved, not cloned, so each solid is taken by index exactly once
        let format = crate::ExactFormat::of_extension(ext);
        let mut shapes: Vec<Option<crate::Shape>> = crate::exact_solids(format, tmp.to_string_lossy().as_ref()).unwrap_or_default().into_iter().map(Some).collect();
        let _ = std::fs::remove_file(&tmp);
        for Raised { body, solid, scale } in items {
            if let Some(s) = shapes.get_mut(solid as usize).and_then(|o| o.take()) {
                // at the factor given when the file came in: a file can name the wrong unit
                let s = if (scale - 1.0).abs() > 1e-12 { s.transformed(&[scale, 0.0, 0.0, 0.0, 0.0, scale, 0.0, 0.0, 0.0, 0.0, scale, 0.0]).unwrap_or(s) } else { s };
                out.push((body, s));
            }
        }
    }
    named_as_they_came_in(project, out)
}

/// A body of an import to raise: which solid of its file, at what scale.
struct Raised {
    body: Id,
    solid: u32,
    scale: f64,
}

/// THE BODIES RAISED FROM THEIR FILES TAKE BACK THE NAMES THEY CAME IN WITH. Read from the file a body holds positional
/// numbers, while the document lists the names its faces and edges were given when it came in (by where the faces lie,
/// the one rule for a body without a recipe); a fillet on an edge of it would find nothing. The names are made again
/// by the same rule, against a copy of the document whose table already holds them, under the lock of the kernel.
fn named_as_they_came_in(project: &Project, raised: Vec<(Id, crate::Shape)>) -> Vec<(Id, crate::Shape)> {
    if raised.is_empty() {
        return raised;
    }
    let bodies: Vec<Id> = raised.iter().map(|(b, _)| *b).collect();
    let kernel = crate::OcctKernel { shapes: std::cell::RefCell::new(raised.into_iter().collect()), ..Default::default() };
    let mut names = project.clone();
    {
        let _gate = crate::kernel_gate();
        for body in bodies {
            let Some(feature) = names.timeline.iter().find(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::Import { body: b, .. } if b == body)).map(|n| n.id) else { continue };
            let scale = names.import_scale(body).filter(|f| *f > 0.0).unwrap_or(1.0);
            crate::naming::silent(&kernel, qymcad_core::feature::SilentNaming { body, feature, listed: None, scale, names: &mut names.names });
            unminted(project, body, &kernel);
        }
    }
    kernel.shapes.into_inner().into_iter().collect()
}

/// ONLY NAMES THE DOCUMENT HOLDS LEAVE HERE. A body raised before the import was ever rebuilt - the first time it comes
/// in - has no names in the document yet, and the copy mints them against a table of its own: descriptors the document
/// does not hold, which point past its table or at a name of another face. Those go back to positional numbers, and
/// the rebuild of the import names the body in the document itself.
fn unminted(project: &Project, body: Id, kernel: &crate::OcctKernel) {
    let shapes = kernel.shapes.borrow();
    let Some(s) = shapes.get(&body) else { return };
    let faces: Vec<u32> = s.face_places().iter().map(|f| f.id).collect();
    let edges: Vec<u32> = s.edges_with_ids().1;
    let free = faces.iter().chain(edges.iter()).filter(|d| !qymcad_core::names::NameTable::is_named(**d)).max().copied().unwrap_or(0);
    let mut next = free + 1;
    let mut back = |ids: &[u32]| -> Vec<qymcad_core::feature::Rename> {
        ids.iter()
            .filter(|d| qymcad_core::names::NameTable::is_named(**d) && !project.names.knows(**d))
            .map(|d| {
                next += 1;
                qymcad_core::feature::Rename { was: *d, now: next - 1 }
            })
            .collect()
    };
    let (face_back, edge_back) = (back(&faces), back(&edges));
    s.rename_faces(&face_back);
    s.rename_edges(&edge_back);
}
