//! THE REBUILD LAID OUT IN WAVES: what can be computed at the same time.
//!
//! Deliberately without any geometry: the question "is there anything to compute side by side" is arithmetic
//! over the graph the timeline already carries. A check on it is cheap and cannot lie - unlike a measurement
//! with a kernel in it.
use qymcad_core::model::Project;

/// A part of its own with a rectangle extruded into a body. Returns the body's id.
pub fn a_part(p: &mut Project, x: f64) -> u64 {
    let root = p.root;
    p.set_active_component(Some(root));
    let part = p.add_part(format!("Part {x}"));
    p.set_active_component(Some(part));
    let si = p.new_sketch("base");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "base");
    p.add_rect_entity(si, x, 0.0, x + 20.0, 20.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    p.add_extrude(sid, 10.0)
}

/// One more cut on top of `body`, in the same part: a link in a chain.
pub fn a_cut(p: &mut Project, body: u64, x: f64) -> u64 {
    let si = p.new_sketch("cut");
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "cut");
    p.add_rect_entity(si, x + 2.0, 2.0, x + 8.0, 8.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    p.add_combine(body, sid, 20.0, 0)
}

/// How many nodes stand in each wave.
fn shape_of(waves: &[Vec<u64>]) -> Vec<usize> {
    waves.iter().map(|w| w.len()).collect()
}

#[test]
fn three_parts_that_know_nothing_of_each_other_stand_in_one_wave() {
    let mut p = Project::default();
    p.new_document();
    for k in 0..3 {
        a_part(&mut p, k as f64 * 40.0);
    }
    let waves = p.rebuild_waves();
    // every part is a sketch node plus an extrude: the sketches are ready at once, the extrudes wait for them
    let total: usize = shape_of(&waves).iter().sum();
    assert_eq!(total, p.regen_plan().nodes.len(), "the waves hold a different number of nodes from the plan");
    assert!(waves.len() <= 2, "three parts that do not depend on one another fell into {} waves: {:?}", waves.len(), shape_of(&waves));
    assert!(waves.last().is_some_and(|w| w.len() == 3), "the three extrudes must stand together: {:?}", shape_of(&waves));
}

#[test]
fn a_chain_of_three_cuts_makes_three_waves_of_one() {
    let mut p = Project::default();
    p.new_document();
    let body = a_part(&mut p, 0.0);
    let a = a_cut(&mut p, body, 0.0);
    let b = a_cut(&mut p, a, 2.0);
    let c = a_cut(&mut p, b, 4.0);
    let waves = p.rebuild_waves();
    let last_three: Vec<usize> = shape_of(&waves).into_iter().rev().take(3).collect();
    assert_eq!(last_three, vec![1, 1, 1], "a chain must come out as waves of one: {:?}", shape_of(&waves));
    // and they stand in the order of the chain, not in any other
    let order: Vec<u64> = waves.iter().flatten().copied().collect();
    let at = |id: u64| order.iter().position(|x| *x == id).expect("the node is in the waves");
    assert!(at(a) < at(b) && at(b) < at(c), "the chain came out in the wrong order");
}

#[test]
fn a_mixture_puts_the_independent_parts_together_and_the_chain_apart() {
    let mut p = Project::default();
    p.new_document();
    let first = a_part(&mut p, 0.0);
    a_part(&mut p, 40.0);
    a_part(&mut p, 80.0);
    a_cut(&mut p, first, 0.0); // only the first part has anything built on it
    let waves = p.rebuild_waves();
    // the three extrudes stand together, and the cut on the first part waits for it: [3, 1]
    assert_eq!(shape_of(&waves), vec![3, 1], "the mixture came out as {:?}", shape_of(&waves));
}

/// One more cut on top of `body`, sketched on a face of `body`.
fn a_cut_on_its_face(p: &mut Project, body: u64, x: f64) -> u64 {
    let si = p.new_sketch("cut on the face");
    p.sketches[si].plane = qymcad_core::feature::SketchPlane::Face(body, qymcad_core::feature::FaceKey::default());
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, "cut on the face");
    p.add_rect_entity(si, x + 2.0, 2.0, x + 8.0, 8.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    p.add_combine(body, sid, 20.0, 0)
}

/// A CUT SKETCHED ON A FACE OF ITS OWN BODY STANDS IN THE WAVE AFTER THE BODY. The node of a sketch waited for the body
/// under it, and the cut for the sketch: every link of a chain of cuts on faces took two waves, the body, the sketch,
/// the cut (seven waves of one for this chain). Measured on the case example: the parts that rebuilt side by side in 11
/// waves of 2-5 nodes came in 29 waves of one, and the case on 19 threads took 9.1 s of work against 6.0. A sketch is
/// solved in its own plane, wherever the plane stands: the cut reads what the sketch stands on, the sketch does not.
#[test]
fn a_chain_of_cuts_on_faces_makes_a_wave_a_cut() {
    let mut p = Project::default();
    p.new_document();
    let body = a_part(&mut p, 0.0);
    let a = a_cut_on_its_face(&mut p, body, 0.0);
    let b = a_cut_on_its_face(&mut p, a, 2.0);
    let c = a_cut_on_its_face(&mut p, b, 4.0);
    let waves = p.rebuild_waves();
    // the extrude, then the three cuts one after another; the sketches, drawn and solved, are not rebuilt
    assert_eq!(shape_of(&waves), vec![1, 1, 1, 1], "a chain of three cuts on faces came out as {:?}", shape_of(&waves));
    let order: Vec<u64> = waves.iter().flatten().copied().collect();
    let at = |id: u64| order.iter().position(|x| *x == id).expect("the node is in the waves");
    assert!(at(a) < at(b) && at(b) < at(c), "the chain came out in the wrong order");
}

/// THE WAVES SAY WHEN, NEVER WHETHER: their union is exactly what a sequential pass would rebuild.
#[test]
fn the_waves_hold_the_same_nodes_as_the_plan() {
    let mut p = Project::default();
    p.new_document();
    let a = a_part(&mut p, 0.0);
    let b = a_part(&mut p, 40.0);
    a_cut(&mut p, a, 0.0);
    a_cut(&mut p, b, 0.0);
    let mut from_waves: Vec<u64> = p.rebuild_waves().into_iter().flatten().collect();
    let mut from_plan = p.regen_plan().nodes;
    from_waves.sort_unstable();
    from_plan.sort_unstable();
    assert_eq!(from_waves, from_plan, "the waves and the sequential plan disagree about what to rebuild");
}

/// AND NOTHING STANDS IN A WAVE BEFORE WHAT IT DEPENDS ON.
#[test]
fn no_node_stands_before_its_own_input() {
    let mut p = Project::default();
    p.new_document();
    let body = a_part(&mut p, 0.0);
    let cut = a_cut(&mut p, body, 0.0);
    a_cut(&mut p, cut, 2.0);
    let waves = p.rebuild_waves();
    let wave_of: std::collections::HashMap<u64, usize> = waves.iter().enumerate().flat_map(|(w, ids)| ids.iter().map(move |id| (*id, w))).collect();
    for nd in &p.timeline {
        let Some(&mine) = wave_of.get(&nd.id) else { continue };
        for input in nd.kind.inputs() {
            // an input made by another node OF THIS REBUILD has to be ready in an earlier wave
            if let Some(maker) = p.timeline.iter().find(|n| n.kind.declares().contains(&input)) {
                if let Some(&theirs) = wave_of.get(&maker.id) {
                    assert!(theirs < mine, "node {} stands in wave {mine} while its input is made in wave {theirs}", nd.id);
                }
            }
        }
    }
}

/// WHAT IS COMPUTED SIDE BY SIDE is not the wave but what is ready: a node whose source stands between here
/// and it must wait, even when the wave says the two do not depend on one another.
mod ready {
    use super::{a_cut, a_part};
    use qymcad_core::model::Project;

    /// Three parts that know nothing of one another: all three heads are ready at once.
    #[test]
    fn independent_parts_are_all_ready_together() {
        let mut p = Project::default();
        p.new_document();
        for k in 0..3 {
            a_part(&mut p, k as f64 * 40.0);
        }
        for n in &mut p.timeline {
            n.dirty = true;
        }
        let ready = p.ready_batch_from(0);
        assert!(ready.len() >= 3, "three independent parts gave {} ready nodes: {ready:?}", ready.len());
    }

    /// A CHAIN GIVES ONE. This is the threshold of the whole rule: were two links of a chain computed side by
    /// side, the second would be built from a body that is not there yet - which is exactly the fault the
    /// measurement caught (13 refusals, half the bodies missing).
    #[test]
    fn a_chain_offers_only_its_first_link() {
        let mut p = Project::default();
        p.new_document();
        let body = a_part(&mut p, 0.0);
        let a = a_cut(&mut p, body, 0.0);
        let b = a_cut(&mut p, a, 2.0);
        a_cut(&mut p, b, 4.0);
        for n in &mut p.timeline {
            n.dirty = true;
        }
        let ready = p.ready_batch_from(0);
        assert!(!ready.contains(&a), "a cut standing on a body built in the same batch was offered for it: {ready:?}");
        assert!(!ready.contains(&b), "the second link of the chain was offered too: {ready:?}");
    }

    /// AND FROM THE MIDDLE OF A CHAIN, the link standing there is ready and the one after it is not.
    #[test]
    fn from_the_middle_the_next_link_is_ready_and_the_one_after_is_not() {
        let mut p = Project::default();
        p.new_document();
        let body = a_part(&mut p, 0.0);
        let a = a_cut(&mut p, body, 0.0);
        let b = a_cut(&mut p, a, 2.0);
        for n in &mut p.timeline {
            n.dirty = true;
        }
        let at = p.timeline.iter().position(|n| n.id == a).expect("the first cut is in the timeline");
        let ready = p.ready_batch_from(at);
        assert!(ready.contains(&a), "the link the rebuild stands on must be ready: {ready:?}");
        assert!(!ready.contains(&b), "the link after it must wait: {ready:?}");
    }

    /// A LINK THAT IS CLEAN FOR NOW STILL HOLDS BACK WHAT STANDS ON IT.
    ///
    /// One parameter drives the base and the last cut; the cut between them did not change. At the moment the
    /// batch is formed the middle cut looks clean - its source has not been rebuilt yet, so nothing it reads is
    /// dirty - but it WILL be rebuilt when the walk reaches it. Offering the last cut now builds it on the
    /// middle body as it was before the edit, and the sequential rebuild would not.
    #[test]
    fn a_link_clean_for_now_still_holds_back_what_stands_on_it() {
        let mut p = Project::default();
        p.new_document();
        let base = a_part(&mut p, 0.0);
        let middle = a_cut(&mut p, base, 0.0);
        let last = a_cut(&mut p, middle, 2.0);
        for n in &mut p.timeline {
            n.dirty = n.id == base || n.id == last;
        }
        let at = p.timeline.iter().position(|n| n.id == base).expect("the base is in the timeline");
        let ready = p.ready_batch_from(at);
        assert!(ready.contains(&base), "the base the rebuild stands on must be ready: {ready:?}");
        assert!(!ready.contains(&last), "the last cut was offered while the cut under it is still to be rebuilt: {ready:?}");
    }
}
