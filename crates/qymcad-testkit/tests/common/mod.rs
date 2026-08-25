//! The shared loading of the `testbug.ron` fixture, for the tests that work against it.
use qymcad_core::model::Project;

/// The fixture together with the target of the shell, stated by meaning.
///
/// The name of a face is derived from the recipe of an operation and lives in the name table of the document,
/// so it cannot be written into a `.ron` as a number. There is nothing to migrate an old number from either: a
/// bare `.ron` holds no geometry, whereas a working bundle keeps it beside the mesh and the translation happens
/// by itself there. The target of the shell is therefore given by meaning: build, find the top face by its
/// geometry, and point at that.
pub fn testbug() -> Project {
    use qymcad_core::feature::FeatureKind;
    let mut p = qymcad_core::model::from_ron(include_str!("../testbug.ron")).expect("load");
    let (report, _s) = qymcad_testkit::regenerate(&mut p);
    if let Some((_, faces)) = report.built.iter().find(|(b, _)| *b == 199u64) {
        if let Some(top) = faces.iter().filter(|f| f.normal[2] > 0.9).max_by(|a, b| a.area.total_cmp(&b.area)) {
            for n in p.timeline.iter_mut() {
                if let FeatureKind::Shell { faces, .. } = &mut n.kind {
                    *faces = qymcad_core::refs::Ref::picks(&[top.id]);
                }
            }
        }
    }
    for n in p.timeline.iter_mut() {
        n.dirty = true;
    }
    p
}
