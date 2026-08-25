//! The rebuild graph: editing a node touches only its descendants, and editing a parameter only what refers to
//! it.
//!
//! The timeline is linear and already a topological order, so the propagation of dirtiness is correct in itself.
//! The gap was elsewhere: the graph could not be asked which nodes depend on a given one, so the code marked
//! dirty with a margin — editing any parameter raised every feature carrying an expression, and the rebuild
//! called that on every pass. These checks hold the boundaries.
use qymcad_core::model::{Param, Project};

fn line_sketch(p: &mut Project, name: &str) -> (u64, usize) {
    let sid = p.add_sketch(name, vec![], None);
    p.add_sketch_node(sid, name);
    let si = p.sketch_index(sid).unwrap();
    p.add_rect_entity(si, 0.0, 0.0, 10.0, 10.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    (sid, si)
}

/// A chain: dependent nodes are visible through `dependents` and independent ones are not.
#[test]
fn dependents_sees_the_chain_and_stops_there() {
    let mut p = Project::default();
    p.new_document();
    let (s1, _) = line_sketch(&mut p, "Sketch 1");
    let base = p.add_extrude_multi(s1, Vec::new(), 10.0, qymcad_core::feature::Reach::Forward, 0.0, Vec::new());
    let (s2, si2) = line_sketch(&mut p, "Sketch 2");
    let cut = p.add_combine_multi_op(base, s2, p.sketches[si2].contour_ids.clone(), 5.0, 0, qymcad_core::feature::Extent::default(), 0.0, Vec::new());
    // an independent part: its own chain, with no shared inputs
    let (s3, _) = line_sketch(&mut p, "Sketch 3");
    let other = p.add_extrude_multi(s3, Vec::new(), 3.0, qymcad_core::feature::Reach::Forward, 0.0, Vec::new());

    let dep = p.dependents(base);
    assert!(dep.contains(&cut), "the cut depends on the base: {dep:?}");
    assert!(!dep.contains(&other), "an unrelated part does not depend on the base: {dep:?}");

    let dep_cut = p.dependents(cut);
    assert!(!dep_cut.contains(&base), "a source does not depend on its consumer");
    assert!(dep_cut.is_empty() || !dep_cut.contains(&other), "and neither does an unrelated part");
    let _ = s3;
}

/// A parameter: editing a name raises only the features where that name is actually mentioned.
#[test]
fn only_features_mentioning_the_parameter_get_dirty() {
    let mut p = Project::default();
    p.new_document();
    p.parameters.push(Param { name: "H".into(), expr: "10".into(), value: 10.0 });
    p.parameters.push(Param { name: "W".into(), expr: "20".into(), value: 20.0 });
    let (s1, _) = line_sketch(&mut p, "Sketch 1");
    let a = p.add_extrude_multi(s1, Vec::new(), 10.0, qymcad_core::feature::Reach::Forward, 0.0, Vec::new());
    let (s2, _) = line_sketch(&mut p, "Sketch 2");
    let b = p.add_extrude_multi(s2, Vec::new(), 20.0, qymcad_core::feature::Reach::Forward, 0.0, Vec::new());
    p.set_feat_dim(a, "height", "H".into());
    p.set_feat_dim(b, "height", "W*2".into());
    for n in p.timeline.iter_mut() {
        n.dirty = false;
    }

    p.mark_param_dependents_dirty_for("H");
    let dirty: Vec<u64> = p.timeline.iter().filter(|n| n.dirty).map(|n| n.id).collect();
    assert!(dirty.contains(&a), "the feature whose expression uses H is marked: {dirty:?}");
    assert!(!dirty.contains(&b), "the feature whose expression uses W*2 is left alone: {dirty:?}");
}

/// The name of a parameter must not be found inside another name: `L` is not mentioned in `Length`.
#[test]
fn parameter_name_is_matched_as_a_whole_identifier() {
    assert!(qymcad_core::expr::mentions("L*2", "L"));
    assert!(qymcad_core::expr::mentions("2*L + 3", "L"));
    assert!(qymcad_core::expr::mentions("Length/2", "Length"));
    assert!(!qymcad_core::expr::mentions("Length/2", "L"), "L is not part of Length");
    assert!(!qymcad_core::expr::mentions("HOLE_D", "D"), "D is not the tail of HOLE_D");
    assert!(!qymcad_core::expr::mentions("", "L"));
}
