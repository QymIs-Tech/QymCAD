mod common;
#[test]
fn span_sanity() {
    let p = common::testbug();
    // for each body node of the part under test, what span `feature_op_span` computes
    let mut shown = std::collections::HashSet::new();
    for n in &p.timeline {
        if n.parent != Some(277u64 as qymcad_core::model::Id) { continue; }
        let Some(_b) = n.kind.body() else { continue };
        if shown.contains(&n.id) { continue; }
        let span = p.feature_op_span(n.id);
        for s in &span { shown.insert(*s); }
        let name = &n.name;
        eprintln!("node {} '{}' -> a span of {} nodes: {:?}", n.id, name, span.len(), span);
    }
}
