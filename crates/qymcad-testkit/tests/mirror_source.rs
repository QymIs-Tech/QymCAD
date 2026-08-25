//! Why a mirror used to lose its source: a measurement against the scenario document.
//!
//! Restricting the source to what stands earlier in the timeline, through `active_body_before`, broke real
//! documents: the mirror answered that the part had no source and the assembly stopped building. That was
//! reverted. This establishes why, before a correct restriction is looked for.
//!
//! `cargo test -p qymcad-testkit --test mirror_source -- --ignored --nocapture`
//!
//! WHAT THIS ANSWERS ON THE SCENARIO DOCUMENT TODAY, so that nobody spends an hour on it twice: the single
//! mirror there DOES report `SourcePartHasNoBody`, and that is CORRECT, not a defect. The scenario has a step
//! that deletes the source part's sketch on purpose and then demands that the mirror turn red rather than
//! keep silent over old geometry (`user_case.rs`, "THE MIRROR OF AN ORPHANED PART MUST TURN RED"). The saved
//! document is the state after that step, so its source part legitimately owns nothing. Measurements that run
//! on this file - `an_edit_keeps_every_reference` among them - therefore count failures present BEFORE their
//! own edit and ask only that the edit adds none.
const PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/user-case.qcad");

#[test]
#[ignore = "a measurement against the scenario document"]
fn where_the_mirror_looks_for_its_source() {
    if !std::path::Path::new(PATH).exists() {
        eprintln!("skipped: the file is missing");
        return;
    }
    let Ok(p) = qymcad_io::load_project(PATH) else {
        eprintln!("skipped: the file does not read");
        return;
    };
    let mirrors: Vec<(usize, qymcad_core::model::Id, qymcad_core::model::Id)> = p
        .timeline
        .iter()
        .enumerate()
        .filter_map(|(i, n)| match n.kind {
            qymcad_core::feature::FeatureKind::MirrorPart { src_comp, .. } => Some((i, n.id, src_comp)),
            _ => None,
        })
        .collect();
    eprintln!("mirrors in the document: {}", mirrors.len());
    for (i, id, src) in mirrors {
        let name = p.components.iter().find(|c| c.id == src).map(|c| c.name.clone()).unwrap_or_default();
        eprintln!("mirror {id} at position {i}, source {src} '{name}'");
        eprintln!("   active_body(src)            = {:?}", p.active_body(src));
        eprintln!("   active_body_before(src, {i}) = {:?}", p.active_body_before(src, i));
        let consumed = p.consumed_bodies();
        for (k, n) in p.timeline.iter().enumerate() {
            for b in n.kind.bodies() {
                if p.body_owner(b) == Some(src) {
                    let eater = p.timeline.iter().position(|x| x.kind.consumed().contains(&b));
                    eprintln!("   body {b} of node '{}' at position {k}: consumed={}, consumer at position {:?}", n.name, consumed.contains(&b), eater);
                }
            }
        }
        // WHEN THE LOOP ABOVE PRINTS NOTHING the source has no body in the timeline AT ALL, and that reads as
        // silence - the same silence this whole refactor is about. Silence is not an answer, so it is spelled
        // out here, with the place to look next.
        if !p.timeline.iter().flat_map(|n| n.kind.bodies()).any(|b| p.body_owner(b) == Some(src)) {
            eprintln!("   NO NODE OF THE TIMELINE PRODUCES A BODY OWNED BY {src}: the source of the mirror owns nothing");
            eprintln!("   components of the document (a part that produces nothing is the one to look at):");
            for c in &p.components {
                let own = p.timeline.iter().flat_map(|n| n.kind.bodies()).filter(|b| p.body_owner(*b) == Some(c.id)).count();
                eprintln!("      {} '{}' parent {:?}: bodies produced {own}", c.id, c.name, c.parent);
            }
        }
    }
}
