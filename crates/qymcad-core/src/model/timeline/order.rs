//! THE ORDER OF THE TIMELINE: rollback, pushing, suppressing, reordering, edge references translated.

use super::*;

impl Project {
    /// Rollback bar: build only the first `n` timeline nodes (`None` builds everything). Marks dirty the nodes from
    /// the higher of its two places down: a node above both rests on nothing the bar moved across, as with suppression.
    /// Reported behaviour: the bar dragged one row up, above a rounding, and every node was marked - the rebuild took
    /// all 27 nodes of the part, its two threads among them, for minutes.
    pub fn set_rollback(&mut self, n: Option<usize>) {
        let was = self.rollback_place();
        self.rollback = n.filter(|&n| n < self.timeline.len());
        let from = was.min(self.rollback_place());
        for nd in self.timeline.iter_mut().skip(from) {
            nd.dirty = true;
        }
    }

    /// WHERE THE ROLLBACK BAR STANDS: the count of nodes built.
    pub fn rollback_place(&self) -> usize {
        self.rollback.unwrap_or(self.timeline.len())
    }

    /// Append a node to the timeline, or insert it at the rollback bar when one is active, which is how
    /// building between features works: the new node becomes the last active one and the bar moves past it.
    /// Every `add_*` goes through this, which makes the rollback bar genuinely the place where building
    /// continues. Returns the node id.
    pub(crate) fn push_timeline(&mut self, node: crate::feature::FeatureNode) -> Id {
        let (id, part) = (node.id, node.parent);
        let link = node.kind.consumed_body().zip(node.kind.body());
        let at = match self.rollback {
            Some(r) if r <= self.timeline.len() => {
                self.timeline.insert(r, node);
                self.rollback = Some(r + 1);
                // A STEP PUT IN ABOVE GOES INTO THE CHAIN: the node below that carried the same body on now carries
                // on from the one put in. Reported behaviour: a cut put in by the rollback above a rounding left the
                // rounding on the body without the cut, and the part held two bodies (11000 and 11882.9 mm^3).
                if let Some((src, out)) = link {
                    if let Some(next) = self.timeline.iter_mut().skip(r + 1).find(|n| n.parent == part && n.kind.consumed_body() == Some(src)) {
                        next.kind.remap_body_input(src, out);
                        next.dirty = true;
                    }
                }
                r
            }
            _ => {
                self.timeline.push(node);
                self.timeline.len() - 1
            }
        };
        if let Some(part) = part {
            self.bring_instances_after(part, at);
        }
        id
    }

    /// A PART'S INSTANCES FOLLOW WHAT IT GETS LATER.
    ///
    /// An instance - a clone, a pattern's copy - repeats its source's body from what stands above it in the
    /// timeline (`active_body_before`), so a feature added to the source after the instance was made stood below
    /// it and never reached it: a box united with the original left its clone as it was. The instance nodes of
    /// `part` above `at` move to just after it, together with every node between that reads what they make (a
    /// sketch on a clone's face, say), so no node ends up above its input. A moved node is rebuilt. Nodes leave
    /// only from above `at` and land at or below it, so the rollback bar keeps the same nodes above it.
    pub(crate) fn bring_instances_after(&mut self, part: Id, at: usize) {
        use crate::feature::FeatureKind;
        let mut made: std::collections::HashSet<Id> = std::collections::HashSet::new();
        let mut going: Vec<usize> = Vec::new();
        for i in 0..at {
            let instance = matches!(self.timeline[i].kind, FeatureKind::PartInstance { .. } | FeatureKind::ComponentPattern { .. }) && self.timeline[i].kind.copy_source() == Some(part);
            if instance || (!made.is_empty() && self.node_reads_any(i, &made)) {
                made.extend(self.timeline[i].kind.declares());
                going.push(i);
            }
        }
        if going.is_empty() {
            return;
        }
        let mut moved: Vec<crate::feature::FeatureNode> = going.iter().rev().map(|&i| self.timeline.remove(i)).collect();
        moved.reverse();
        let after = at + 1 - going.len(); // the new node moved up by as many as left from above it
        for (k, mut n) in moved.into_iter().enumerate() {
            n.dirty = true;
            self.timeline.insert(after + k, n);
        }
    }

    /// Suppress or unsuppress feature `ti`. The node is marked dirty, so regenerate removes its body from view
    /// (cascading to its consumers) or rebuilds it when it is switched back on. Returns the new state.
    pub fn set_feature_suppressed(&mut self, ti: usize, on: bool) -> bool {
        if let Some(n) = self.timeline.get_mut(ti) {
            n.suppressed = on;
            n.dirty = true;
        }
        // Everything below becomes dirty too. Marking a single node rebuilds that node while the features
        // resting on its body keep the previous shape, so the model on screen stops being the result of its own
        // timeline; switching the feature back on then appears to change nothing until a full rebuild is
        // forced.
        //
        // Marking runs from the node downwards rather than over everything: the timeline is ordered, and what
        // stands above cannot rest on the suppressed feature by construction.
        for n in self.timeline.iter_mut().skip(ti + 1) {
            n.dirty = true;
        }
        on
    }

    /// Output ids of a node, that is, what it produces: a body plus a datum or a sketch (plane, axis, point,
    /// sketch). Used by the ordering check.
    pub(crate) fn node_output_ids(&self, kind: &crate::feature::FeatureKind) -> Vec<Id> {
        use crate::feature::FeatureKind as FK;
        let mut o: Vec<Id> = kind.bodies();
        match *kind {
            FK::Sketch { sketch } => o.push(sketch),
            FK::Plane { plane } => o.push(plane),
            FK::DatumAxis { axis } => o.push(axis),
            FK::DatumPoint { point } => o.push(point),
            _ => {}
        }
        o
    }

    /// Whether the node order is valid: every reference of a node (a body, a sketch, a datum) is produced
    /// before it, with no forward references. Datum dependencies count too (an offset plane above its source, a
    /// sketch on a datum and so on).
    pub(crate) fn order_valid(&self, order: &[usize]) -> bool {
        let all: std::collections::HashSet<Id> = order.iter().flat_map(|&i| self.node_output_ids(&self.timeline[i].kind)).collect();
        let mut produced: std::collections::HashSet<Id> = std::collections::HashSet::new();
        for &idx in order {
            let k = &self.timeline[idx].kind;
            for req in self.node_required_refs(k) {
                if all.contains(&req) && !produced.contains(&req) {
                    return false; // A reference to something not yet built above it in the timeline.
                }
            }
            for out in self.node_output_ids(k) {
                produced.insert(out);
            }
        }
        true
    }

    /// Whether moving `from` to `to` would be valid, without mutating. Used to grey out the "move up" and
    /// "move down" commands when the move would break a dependency, as in a linear chain of a fillet over an
    /// extrude.
    pub fn can_reorder_feature(&self, from: usize, to: usize) -> bool {
        if from >= self.timeline.len() || to >= self.timeline.len() || from == to {
            return false;
        }
        let to2 = if to > from { to - 1 } else { to };
        let mut order: Vec<usize> = (0..self.timeline.len()).collect();
        let item = order.remove(from);
        order.insert(to2, item);
        self.order_valid(&order) || (from.abs_diff(to2) == 1 && self.chain_swap(from.min(to2)).is_some())
    }

    /// TWO CHANGES OF ONE BODY IN A ROW, at `i` and `i + 1`: the second takes as its only input the body the first
    /// made, and the first takes one body. Such a pair may change places: each then drills, rounds or cuts the body
    /// the other left. Answers (the first's input, the first's body, the second's body). Plain reordering refused
    /// them, the second standing on the first's output: two holes of one part could never swap.
    pub(crate) fn chain_swap(&self, i: usize) -> Option<(Id, Id, Id)> {
        let (a, b) = (&self.timeline.get(i)?.kind, &self.timeline.get(i + 1)?.kind);
        let (a_in, a_out, b_in, b_out) = (a.inputs(), a.bodies(), b.inputs(), b.bodies());
        let ([s], [a_out], [b_out]) = (a_in.as_slice(), a_out.as_slice(), b_out.as_slice()) else { return None };
        if b_in.as_slice() != [*a_out] {
            return None;
        }
        // both have to be re-pointable at another body; a node whose input is not a plain `src` is not
        let (mut ta, mut tb) = (a.clone(), b.clone());
        ta.remap_body_input(*s, *b_out);
        tb.remap_body_input(*a_out, *s);
        (ta.inputs() == vec![*b_out] && tb.inputs() == vec![*s]).then_some((*s, *a_out, *b_out))
    }

    /// Move timeline node `from` to position `to`, reordering the history. Refused when it would break a
    /// dependency (an input body ending up below its consumer). Marks everything dirty. Returns whether it was
    /// done.
    pub fn reorder_feature(&mut self, from: usize, to: usize) -> bool {
        if from >= self.timeline.len() || to >= self.timeline.len() || from == to {
            return false;
        }
        let to2 = if to > from { to - 1 } else { to };
        let mut order: Vec<usize> = (0..self.timeline.len()).collect();
        let item = order.remove(from);
        order.insert(to2, item);
        if !self.order_valid(&order) {
            // two changes of one body swap by re-pointing their inputs: the second now takes the first's input, the
            // first takes the second's body, and what stood on the second's body stands on the first's
            let i = from.min(to2);
            let Some((s, a_out, b_out)) = (from.abs_diff(to2) == 1).then(|| self.chain_swap(i)).flatten() else { return false };
            self.timeline[i + 1].kind.remap_body_input(a_out, s);
            self.timeline[i].kind.remap_body_input(s, b_out);
            for n in &mut self.timeline[i + 2..] {
                n.kind.remap_body_input(b_out, a_out);
            }
            for sk in &mut self.sketches {
                if let crate::feature::SketchPlane::Face(body, _) = &mut sk.plane {
                    if *body == b_out {
                        *body = a_out;
                    }
                }
            }
            self.timeline.swap(i, i + 1);
            for nd in &mut self.timeline {
                nd.dirty = true;
            }
            return true;
        }
        let n = self.timeline.remove(from);
        self.timeline.insert(to2, n);
        for nd in &mut self.timeline {
            nd.dirty = true;
        }
        true
    }

    /// Translate the stored edge references of body `src` through the rename map of this pass and write the
    /// result back into the node, so that the guess happens exactly once, as it does for faces. The snapshots in
    /// `edge_refs` travel with the references: they are needed to repair a real topology change.
    pub(crate) fn translate_edge_refs(&mut self, node_id: Id, src: Id, edges: &[u32], emap: &EdgeRenames, cur: &[crate::geom::MeshEdge]) -> Vec<u32> {
        let Some(map) = emap.get(&src) else { return edges.to_vec() };
        let alive = |id: u32| cur.is_empty() || cur.iter().any(|e| e.id == id);
        // Only what is lost gets translated. The map accumulates along the whole ancestor chain, and an old
        // number from a distant ancestor may happen to hit a live edge of the current body, in which case
        // translating would move the reference onto somebody else's geometry. So: if the number is alive the
        // reference is correct and is left alone; if it is gone and the translation yields an existing edge, it
        // is translated; otherwise it is left to the snapshot repair.
        let out: Vec<u32> = edges
            .iter()
            .map(|e| match map.get(e) {
                Some(&new) if !alive(*e) && alive(new) => new,
                _ => *e,
            })
            .collect();
        if out == edges {
            return out;
        }
        if let Some(n) = self.timeline.iter_mut().find(|n| n.id == node_id) {
            match &mut n.kind {
                // Translated names are written back only for a hand-picked set: a descriptive query stores no
                // names, and replacing it with a list of numbers would take away everything it exists for.
                crate::feature::FeatureKind::Fillet { edges, .. } | crate::feature::FeatureKind::Chamfer { edges, .. } if !edges.query.picked_descs().is_empty() => {
                    *edges = crate::refs::Ref::picks(&out);
                }
                _ => {}
            }
        }
        if let Some(snaps) = self.edge_refs.get_mut(&node_id) {
            for snap in snaps.iter_mut() {
                if let Some(&new) = map.get(&snap.id) {
                    snap.id = new;
                }
            }
        }
        out
    }
}
