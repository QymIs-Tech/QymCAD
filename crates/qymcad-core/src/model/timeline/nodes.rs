//! THE NODES OF THE TIMELINE: live bodies, colours, names, the dirty mark, the sketch and profile features added.

use super::*;

impl Project {
    /// The live body of this lineage: walk forward through `consumed` while the body is consumed.
    ///
    /// Every operation creates a new body and consumes its input, so any reference recorded earlier (a sketch
    /// plane on a face, a tree selection) points at a body the model no longer holds from the second operation
    /// onwards. Many places need to ask which body is the current one, and the answer has to come from one
    /// place: copies of this walk drift apart silently and put geometry from the past on screen.
    ///
    /// A body that is not consumed is returned as it is. The chain is bounded from above: a broken timeline (a
    /// cycle in `consumed`) must not hang the application.
    pub fn live_body(&self, body: Id) -> Id {
        let consumed = self.consumed_bodies();
        let mut cur = body;
        for _ in 0..256 {
            if !consumed.contains(&cur) {
                break;
            }
            match self.timeline.iter().find(|n| n.kind.consumed().contains(&cur)).and_then(|n| n.kind.body()) {
                Some(next) => cur = next,
                None => break,
            }
        }
        cur
    }

    /// Lineage root of a body: walk back through `consumed_body` to the feature that consumes no body (an
    /// extrude, a primitive, a boolean — the original part). A colour is keyed by that root and therefore stays
    /// stable across operations.
    pub fn lineage_root(&self, body: Id) -> Id {
        let mut b = body;
        for _ in 0..10_000 {
            match self.timeline.iter().find(|n| n.kind.bodies().contains(&b)).and_then(|n| n.kind.consumed_body()) {
                Some(src) => b = src,
                None => break,
            }
        }
        b
    }

    /// Stable part index by lineage root (the order in which root bodies appear in the timeline), used by the
    /// default palette.
    pub(crate) fn part_color_index(&self, root: Id) -> usize {
        let mut idx = 0;
        for n in &self.timeline {
            if n.kind.body().is_some() && n.kind.consumed_body().is_none() {
                if n.kind.bodies().contains(&root) {
                    return idx;
                }
                idx += 1;
            }
        }
        0
    }

    /// Set the list of filled contours (`fill`) for an extrude or an operation feature, by its body `body`, and
    /// mark the node dirty for the rebuild. Contours in `fill` are not subtracted as holes (see
    /// `feature_profile_encoded_fill`).
    pub fn set_feature_fill(&mut self, body: Id, fill: Vec<Id>) {
        use crate::feature::FeatureKind;
        if let Some(n) = self.timeline.iter_mut().find(|n| n.id == body) {
            match &mut n.kind {
                FeatureKind::Extrude { fill: f, .. } | FeatureKind::Combine { fill: f, .. } => {
                    *f = fill;
                    n.dirty = true;
                }
                _ => {}
            }
        }
    }

    /// Index of a timeline node by id.
    /// RENAME A NODE OF THE BUILD TREE.
    ///
    /// One line of work, but it was written out in the interface: find the node by id, assign the name.
    /// Here it is a named operation instead, which is what the tree, a future rename from a script and a
    /// test all want to call. Returns `false` when there is no such node, so a caller cannot take a miss
    /// for a change.
    pub fn rename_node(&mut self, id: Id, name: impl Into<String>) -> bool {
        match self.timeline.iter_mut().find(|n| n.id == id) {
            Some(n) => {
                n.name = name.into();
                true
            }
            None => false,
        }
    }

    /// EVERY NODE IS STALE - the whole tree is to be built again.
    ///
    /// Asked for when what the tree was built ON has changed underneath it: the geometry kernel was
    /// swapped, the document was reopened, a setting that every feature reads was moved. Marking is all
    /// that happens here; WHO rebuilds and IN WHAT ORDER is the planner's business.
    pub fn mark_all_dirty(&mut self) {
        for n in self.timeline.iter_mut() {
            n.dirty = true;
        }
    }

    /// ADD A MOVE ON TOP OF THE MOVE A BODY ALREADY CARRIES.
    ///
    /// A body that has been moved once holds a `Move` node; moving it again must MULTIPLY the placements
    /// rather than replace them, or the second drag would throw the first away. The interface used to walk
    /// the tree, reach inside the node's kind and compose the matrices there - three steps in which the
    /// order of multiplication is easy to get backwards and impossible to test without a window.
    ///
    /// Returns `false` when the body carries no `Move` node: the caller then has a different job to do
    /// (make one), and telling the two apart is exactly what the answer is for.
    pub fn accumulate_move(&mut self, body: Id, extra: &[f64; 12]) -> bool {
        let Some(node) = self.timeline.iter_mut().find(|n| n.id == body) else { return false };
        let crate::feature::FeatureKind::Move { mat, .. } = &mut node.kind else { return false };
        // THE NEW MOVE GOES FIRST: what is being applied now acts on the result of what was applied
        // before. Swapped, a drag after a turn would move along the OLD axes.
        *mat = crate::feature::compose12(extra, mat);
        node.dirty = true;
        true
    }

    /// EVERY NODE THAT MAKES A BODY IS STALE - the shape has to be computed again.
    ///
    /// Narrower than [`Project::mark_all_dirty`] on purpose: a change of TOLERANCE alters the mesh and
    /// nothing else, so sketches and datums in the tree have no reason to be rebuilt.
    pub fn mark_bodies_dirty(&mut self) {
        for n in self.timeline.iter_mut() {
            if n.kind.body().is_some() {
                n.dirty = true;
            }
        }
    }

    pub fn timeline_index(&self, id: Id) -> Option<usize> {
        self.timeline.iter().position(|n| n.id == id)
    }

    /// Add a sketch node to the timeline, when it is not there yet. Returns the node id, which equals the
    /// sketch id.
    pub fn add_sketch_node(&mut self, sketch: Id, name: impl Into<String>) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        if self.timeline_index(sketch).is_none() {
            let parent = Some(self.active_ctx());
            self.push_timeline(FeatureNode { id: sketch, name: name.into(), kind: FeatureKind::Sketch { sketch }, parent, dirty: false, suppressed: false });
        }
        sketch
    }

    /// Extrude a specific closed contour `profile` (zero takes the first). Holes are subtracted automatically
    /// and `reach` says which way the body grows from the sketch plane. Returns the body id.
    pub fn add_extrude_on(&mut self, sketch: Id, profile: Id, height: f64, reach: crate::feature::Reach, down: f64) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode {
            id: body,
            name: "feat-name-extrude".into(),
            kind: FeatureKind::Extrude { sketch, profiles: vec![profile], height, reach, down, fill: Vec::new(), body },
            parent,
            dirty: true,
            suppressed: false,
        });
        body
    }

    /// Extrude several contours as one operation: one node, one body. An empty `profiles` takes the first
    /// contour.
    pub fn add_extrude_multi(&mut self, sketch: Id, profiles: Vec<Id>, height: f64, reach: crate::feature::Reach, down: f64, fill: Vec<Id>) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode {
            id: body,
            name: "feat-name-extrude".into(),
            kind: FeatureKind::Extrude { sketch, profiles, height, reach, down, fill, body },
            parent,
            dirty: true,
            suppressed: false,
        });
        body
    }

    /// Revolve. `axis_datum` is a datum axis id (zero uses the sketch X or Y axis), including an associative
    /// one built from an edge or a cylindrical face (`add_axis_from_edge`, `add_axis_from_face`). `axis_line` is
    /// the id of a sketch centreline (zero means none) and takes priority over the datum and the sketch axes.
    /// The axis has to lie in the sketch plane.
    pub fn add_revolve_axis(&mut self, sketch: Id, profiles: Vec<Id>, axis: u8, angle: f64, axis_datum: Id, axis_line: Id) -> Id {
        self.add_revolve_multi_op(sketch, profiles, super::RevolveAxis { axis, datum: axis_datum, line: axis_line }, super::RevolveTurn { angle, reach: crate::feature::Reach::default() }, 0, 1)
    }

    /// Revolve every contour and apply the boolean against body `src` within one node.
    ///
    /// A command over two contours used to create one `Revolve` node per contour plus one `BodyBoolean` node per
    /// cut — four nodes for one action. In the timeline that reads as a revolve falling apart into two features
    /// doing an add instead of a cut; it could only be edited one contour at a time, and deleting any of the
    /// nodes took the whole chain below with it. One operation is now one node, as for an extrude.
    #[allow(clippy::too_many_arguments)]
    pub fn add_revolve_multi_op(&mut self, sketch: Id, profiles: Vec<Id>, ax: super::RevolveAxis, turn: super::RevolveTurn, src: Id, op: u8) -> Id {
        let (super::RevolveAxis { axis, datum: axis_datum, line: axis_line }, super::RevolveTurn { angle, reach }) = (ax, turn);
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode {
            id: body,
            name: "feat-name-revolve".into(),
            kind: FeatureKind::Revolve { sketch, profiles, axis, angle, axis_datum, axis_line, reach, src, op, body, pieces: Vec::new() },
            parent,
            dirty: true,
            suppressed: false,
        });
        body
    }

    /// Revolve with the direction the angle is swept in: forwards, back, or half each way.
    pub fn add_revolve_axis_ex(&mut self, sketch: Id, profiles: Vec<Id>, ax: super::RevolveAxis, turn: super::RevolveTurn) -> Id {
        let (super::RevolveAxis { axis, datum: axis_datum, line: axis_line }, super::RevolveTurn { angle, reach }) = (ax, turn);
        self.add_revolve_multi_op(sketch, profiles, super::RevolveAxis { axis, datum: axis_datum, line: axis_line }, super::RevolveTurn { angle, reach }, 0, 1)
    }

    /// Sweep: a profile (`sketch`, `profile`) along a path (`path_sketch`, `path`). A zero `profile` or `path`
    /// picks the first suitable contour automatically. Returns the body id; the node is dirty and is built by
    /// the next rebuild.
    pub fn add_sweep(&mut self, sketch: Id, profiles: Vec<Id>, path_sketch: Id, path: Id) -> Id {
        self.add_sweep_multi_op(sketch, profiles, path_sketch, path, 0, 1)
    }

    /// Sweep every profile contour and apply the boolean against body `src` within one node. Same reasoning as
    /// [`Project::add_revolve_multi_op`].
    pub fn add_sweep_multi_op(&mut self, sketch: Id, profiles: Vec<Id>, path_sketch: Id, path: Id, src: Id, op: u8) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode {
            id: body,
            name: "feat-name-sweep".into(),
            kind: FeatureKind::Sweep { sketch, profiles, path_sketch, path, src, op, body, pieces: Vec::new() },
            parent,
            dirty: true,
            suppressed: false,
        });
        body
    }

    /// Loft: a body through an ordered set of section sketches `sketches` (at least two). `contours` gives the
    /// selected contour per section (zero takes the first closed one; a shorter list leaves the rest automatic)
    /// and `ruled` produces straight faces.
    ///
    /// A zero `src` makes a separate new body; otherwise the lofted solid is combined with body `src` through
    /// `op` (0 cut, 1 union, 2 intersection), giving a lofted cut or boss. Returns the body id; the node is
    /// dirty and is built by the next rebuild.
    pub fn add_loft(&mut self, sketches: Vec<Id>, contours: Vec<Id>, ruled: bool, src: Id, op: u8, surface: bool) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        let body = self.alloc_id();
        let parent = Some(self.body_parent());
        self.push_timeline(FeatureNode {
            id: body,
            name: "feat-name-loft".into(),
            kind: FeatureKind::Loft { sketches, contours, ruled, src, op, surface, body, pieces: Vec::new() },
            parent,
            dirty: true,
            suppressed: false,
        });
        body
    }

    /// Owning component of a timeline node, by id (the node parent); `None` is the document root.
    pub fn node_component(&self, node_id: Id) -> Option<Id> {
        self.timeline.iter().find(|n| n.id == node_id).and_then(|n| n.parent)
    }

    /// Bodies belonging to component `id`: the timeline nodes of that component that produce a body.
    pub fn component_bodies(&self, id: Id) -> Vec<Id> {
        self.timeline.iter().flat_map(|n| n.kind.bodies().into_iter().filter(move |b| n.owner_of(*b) == Some(id))).collect()
    }

    /// Sketches belonging to component `id`.
    pub fn sketches_of_component(&self, id: Id) -> Vec<Id> {
        self.timeline
            .iter()
            .filter(|n| n.parent == Some(id))
            .filter_map(|n| match n.kind {
                crate::feature::FeatureKind::Sketch { sketch } => Some(sketch),
                _ => None,
            })
            .collect()
    }
}
