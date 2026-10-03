//! THE EXAMPLE DOCUMENTS: EVERY PART ONE BODY. A part is one body; the pieces of "Split body" and of a cut gone right
//! through are the named exception, and those become parts only by "Make Part". A document the program ships as an
//! example is read as it stands, every part of it holding one body no later step uses up.
use qymcad_core::model::{Id, Project};

/// The bodies of part `part` a later step does not use up.
fn live(p: &Project, part: Id) -> Vec<Id> {
    let consumed: std::collections::HashSet<Id> = p.timeline.iter().flat_map(|n| n.kind.consumed()).collect();
    p.component_bodies(part).into_iter().filter(|b| !consumed.contains(b) && p.mesh_index(*b).is_some()).collect()
}

/// No part of the example `name` holds more than one body.
fn one_body_per_part(name: &str) {
    let path = format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"));
    let p = qymcad_io::load_project(&path).unwrap_or_else(|e| panic!("the example {name} opens: {e:?}"));
    let many: Vec<(Id, Vec<Id>)> = p
        .components
        .iter()
        .filter(|c| c.kind == qymcad_core::feature::ComponentKind::Part)
        .map(|c| (c.id, live(&p, c.id)))
        .filter(|(_, b)| b.len() > 1)
        .collect();
    assert!(many.is_empty(), "parts of {name} holding more than one body: {many:?}");
}

#[test]
fn every_part_of_the_case_is_one_body() {
    one_body_per_part("QymBoxPc-Case.qcad");
}

#[test]
fn every_part_of_the_filter_is_one_body() {
    one_body_per_part("Filter-v2.qcad");
}
