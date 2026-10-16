//! PARTS MOVED: by a joint, by hand, pulled; the joints solved.

use super::*;

impl Project {
    /// Register an external reference: component `from` is authorised to reference a face of body `body` in
    /// another component. Idempotent — repeating the same pair returns the existing id.
    pub fn add_external_face_ref(&mut self, from: Id, body: Id, key: crate::feature::FaceKey) -> Id {
        use crate::feature::{ExternalGeom, ExternalRef};
        if let Some(r) = self.external_refs.iter().find(|r| r.from_component == from && r.source_body() == Some(body)) {
            return r.id;
        }
        let id = self.alloc_id();
        self.external_refs.push(ExternalRef { id, from_component: from, to_geometry: ExternalGeom::Face(body, key) });
        id
    }
    /// The external reference authorising `consumer` to reference body `body`, if there is one.
    pub fn external_ref_for(&self, consumer: Id, body: Id) -> Option<&crate::feature::ExternalRef> {
        self.external_refs.iter().find(|r| r.from_component == consumer && r.source_body() == Some(body))
    }
    /// Remove an external reference by id. A raw removal: consumer sketches sitting on a face of the source
    /// are left unauthorised afterwards and regenerate honestly breaks the part. The user-facing "break the
    /// link" is [`Project::break_external_ref`], which freezes the geometry first.
    pub fn remove_external_ref(&mut self, id: Id) {
        self.external_refs.retain(|r| r.id != id);
    }
    /// Which mate the pointer drives when this body is grabbed from this context.
    ///
    /// A mate promoted to the root with "control globally" acts in the root context, so every way of driving
    /// it has to work there, not half of them. The gizmo handle and dragging the body itself are two entrances
    /// to one action, and the choice between them does not depend on which node the mate lives in.
    ///
    /// Asking only "which mate drives the component of this context" answers about the subassembly as a whole,
    /// while a promoted mate drives a body inside it. The answer then comes back empty and the drag releases
    /// silently.
    ///
    /// The order of questions is therefore:
    /// 1. the mate driving this very body, when it acts in this context (its own home, or promoted);
    /// 2. otherwise the mate driving the node that is visible as a whole in this context.
    ///
    /// Anything grounded is never driven: it is the reference point.
    pub fn drive_joint_in_context(&self, owner: Id, ctx: Id) -> Option<Id> {
        let acts_here = |jid: Id| -> bool { self.joints.iter().find(|j| j.id == jid).is_some_and(|j| self.joint_in_context(j, ctx)) };
        if !self.is_grounded(owner) {
            if let Some(jid) = self.drive_joint_for(owner).filter(|j| acts_here(*j)) {
                return Some(jid);
            }
        }
        let comp = self.ancestor_child_of(ctx, owner).unwrap_or(owner);
        if comp == owner || self.is_grounded(comp) {
            return None;
        }
        self.drive_joint_for(comp).filter(|j| acts_here(*j))
    }

    /// Which component the pointer actually leads when this body is grabbed: the one the solver can move.
    ///
    /// The body is pulled, but what moves is whatever takes part in the assembly problem: the body itself when
    /// it participates in mates or constraints, otherwise the nearest ancestor that does. A body inside a
    /// subassembly therefore travels with that subassembly instead of tearing away from it, and the whole
    /// mechanism follows as a chain.
    ///
    /// `None` means nothing is led by the pointer: the target is grounded (it is the reference) or no mate
    /// reaches it from this context, in which case the drag has to fall through to view navigation rather than
    /// be wasted.
    ///
    /// Only mates acting in this context count (`joint_in_context`). That is exactly what "control globally"
    /// means: a mate that was not promoted is driven from the root neither by a handle nor by the pointer, or
    /// the flag would mean nothing.
    ///
    /// The search never goes above the context: a context is not dragged from inside itself.
    pub fn pull_target_component(&self, part: Id, ctx: Id) -> Option<Id> {
        let mut acting: std::collections::HashSet<Id> = std::collections::HashSet::new();
        for j in &self.joints {
            if !self.joint_in_context(j, ctx) {
                continue;
            }
            for cid in [j.a, j.b] {
                if let Some(o) = self.connector(cid).map(|c| c.owner) {
                    acting.insert(o);
                }
            }
        }
        // Constraints live where their members live: the home is the common ancestor, as for a mate. They
        // have no "control globally" flag, so the condition is shorter.
        for g in &self.mate_constraints {
            let mut owners: Vec<Id> = g.members.clone();
            owners.extend(g.anchors.iter().filter_map(|&c| self.connector(c).map(|x| x.owner)));
            owners.extend(g.faces.iter().map(|(o, _)| *o));
            if owners.len() < 2 {
                continue; // Nothing to hold with.
            }
            let mut home = self.common_ancestor(owners[0], owners[1]);
            for &o in owners.iter().skip(2) {
                home = home.and_then(|a| self.common_ancestor(a, o));
            }
            if home == Some(ctx) || (home.is_none() && ctx == self.root) {
                acting.extend(owners);
            }
        }
        // A grounded component is skipped rather than treated as a stopping point. Grounded means fixed
        // within its own assembly, not in the world: the base of a subassembly is grounded inside it and still
        // travels with it. Grabbing such a body is a request to move the subassembly, which is legitimate.
        let mut cur = Some(part);
        while let Some(c) = cur {
            if !self.is_grounded(c) && acting.contains(&c) {
                return Some(c);
            }
            if c == ctx {
                return None;
            }
            cur = self.components.iter().find(|x| x.id == c).and_then(|x| x.parent);
        }
        None
    }

    /// A PART NO MATE HOLDS, TAKEN BY THE HAND: the component standing directly in `ctx` that holds `part`, when it is
    /// not grounded and no mate or constraint of the context acts on it - it is moved as it is, following the pointer.
    /// A part that took part in no mate used to hand the drag to the camera, and it could be moved only by its gizmo.
    pub fn free_pull_component(&self, part: Id, ctx: Id) -> Option<Id> {
        if self.pull_target_component(part, ctx).is_some() || self.drive_joint_in_context(part, ctx).is_some() {
            return None;
        }
        // held anywhere is held: a mate not in effect here (a subassembly's own, not made global) still holds the
        // part in its assembly, and the hand leaves it alone
        let mut held: std::collections::HashSet<Id> = self.joints.iter().flat_map(|j| [j.a, j.b]).filter_map(|cid| self.connector(cid).map(|c| c.owner)).collect();
        for g in &self.mate_constraints {
            held.extend(g.members.iter().copied());
            held.extend(g.anchors.iter().filter_map(|&c| self.connector(c).map(|x| x.owner)));
            held.extend(g.faces.iter().map(|(o, _)| *o));
        }
        let mut c = part;
        for _ in 0..256 {
            if held.contains(&c) {
                return None;
            }
            let parent = self.components.iter().find(|x| x.id == c).and_then(|x| x.parent)?;
            if parent == ctx {
                return (!self.is_grounded(c)).then_some(c);
            }
            c = parent;
        }
        None
    }

    /// PUT A COMPONENT WHERE THE HAND LEFT IT (`to` - its placement in its parent), carrying every part grouped with
    /// it (groups chained through shared members) by the same motion. A group reads where its members stand when the
    /// problem is assembled, so moving one member alone would make the broken placement the one it holds. False, and
    /// nothing moves, when the component or anything grouped with it is grounded.
    pub fn move_component_by_hand(&mut self, comp: Id, to: [f64; 12]) -> bool {
        use crate::feature::{mat_inv12, mat_mul12};
        let mut carried = vec![comp];
        let mut i = 0;
        while i < carried.len() {
            let c = carried[i];
            for g in self.mate_constraints.iter().filter(|g| g.kind == crate::feature::ConstraintKind::Group && g.members.contains(&c)) {
                for &m in &g.members {
                    if !carried.contains(&m) {
                        carried.push(m);
                    }
                }
            }
            i += 1;
        }
        if carried.iter().any(|&c| self.is_grounded(c)) {
            return false;
        }
        // the motion in the world: new world of the component times the inverse of its old one
        let (old, world) = (self.component_transform(comp), self.world_transform(comp));
        let parent = mat_mul12(&world, &mat_inv12(&old));
        let motion = mat_mul12(&mat_mul12(&parent, &to), &mat_inv12(&world));
        for &m in &carried[1..] {
            let (local, world_m) = (self.component_transform(m), self.world_transform(m));
            let parent_m = mat_mul12(&world_m, &mat_inv12(&local));
            self.set_component_transform(m, mat_mul12(&mat_inv12(&parent_m), &mat_mul12(&motion, &world_m)));
        }
        self.set_component_transform(comp, to);
        true
    }

    pub fn drive_joint_for(&self, comp: Id) -> Option<Id> {
        let owner = |cid: Id| self.connector(cid).map(|c| c.owner);
        let mut in_graph: std::collections::HashSet<Id> = std::collections::HashSet::new();
        for j in &self.joints {
            if let Some(o) = owner(j.a) {
                in_graph.insert(o);
            }
            if let Some(o) = owner(j.b) {
                in_graph.insert(o);
            }
        }
        // The placement roots are the grounded components; when none of them is in the graph, the seed is the
        // smallest id, as in `place_tree`.
        let mut seen: std::collections::HashSet<Id> = self.components.iter().filter(|c| c.grounded).map(|c| c.id).collect();
        if seen.iter().all(|p| !in_graph.contains(p)) {
            if let Some(&s) = in_graph.iter().min() {
                seen.insert(s);
            }
        }
        if seen.contains(&comp) {
            return None; // The placement root itself (grounded or seed) is not driven by a joint.
        }
        // Relaxation: the first joint that reaches a new node becomes its parent edge.
        let mut progress = true;
        while progress {
            progress = false;
            for j in &self.joints {
                let (Some(oa), Some(ob)) = (owner(j.a), owner(j.b)) else { continue };
                let (ap, bp) = (seen.contains(&oa), seen.contains(&ob));
                if ap == bp {
                    continue;
                }
                let newc = if ap { ob } else { oa };
                if newc == comp {
                    return Some(j.id);
                }
                seen.insert(newc);
                progress = true;
            }
        }
        None
    }
    /// Frame of a mate in the space of context `base`: the origin and axes of the first anchor as the solver
    /// works with them.
    ///
    /// A bare connector frame disagrees with the axes the mate actually holds by, in two ways at once: for a
    /// slider on a flat face the travel axis lies in the plane rather than along the normal
    /// (`connector_frame_for_kind`), and the solver may turn the anchor when choosing the nearer mating side
    /// (`joint_flip_now`).
    ///
    /// That caused no visible failure only by accident: every caller of this frame takes just the origin from
    /// it, and the origin depends on neither the kind nor the turn. Being right by accident is a trap for
    /// later, since the name promises the axes of the mate while the axes were somebody else's.
    ///
    /// The direction of an individual degree still has to come from `joint_slot_axis`: for a pin-slot the
    /// travel belongs to the second anchor and cannot be reached from the frame of the first by any axis.
    pub fn joint_frame(&self, jid: Id, base: Id) -> Option<[f64; 12]> {
        let j = self.joints.iter().find(|x| x.id == jid)?;
        let oa = self.connector(j.a)?.owner;
        let (fa, _) = crate::asm::bridge::joint_local_frames(self, j)?;
        let mut m = crate::feature::mat_mul12(&self.relative_transform(oa, base), &fa);
        let side = crate::asm::bridge::joint_side_now(self, j);
        m = crate::feature::mat_mul12(&m, &crate::asm::bridge::side_turn12(side));
        Some(m)
    }
    /// Direction of degree of freedom `slot` in the space of `base`: one source for both the gizmo and the
    /// solver.
    ///
    /// For most kinds every degree lives in the frame of the first anchor, and the mate frame (`joint_frame`)
    /// supplies them all at once. A pin-slot is different: the slot belongs to the second anchor (the first
    /// anchor is the pin and the point of rotation, the second carries the travel). Building the frame always
    /// from the first anchor points the travel arrow one way while the body moves the other.
    ///
    /// So "where does this degree point" is asked here rather than derived from the frame by each caller.
    pub fn joint_slot_axis(&self, jid: Id, slot: usize, base: Id) -> Option<[f64; 3]> {
        let j = self.joints.iter().find(|x| x.id == jid)?;
        let (axis, _) = crate::asm::joint::slot_axis(crate::asm::bridge::kind_of(j.kind), slot)?;
        // Slot travel lives in the frame of the second anchor; everything else in that of the first.
        let own = if matches!(j.kind, crate::feature::JointKind::PinSlot) && slot == 1 { j.b } else { j.a };
        let conn = self.connector(own)?;
        // The frame is built for the joint kind rather than being a plain connector frame. For a slider on a
        // flat face the travel axis lies in the plane rather than along the normal
        // (`connector_frame_for_kind`); the solver knows that, while a gizmo asking for the plain frame draws
        // the arrow along the normal. Pulling the handle up then moves the body sideways. Found by the
        // acceptance matrix on live geometry.
        let f = self.connector_frame_for_kind(conn, j.kind)?;
        let mut m = crate::feature::mat_mul12(&self.relative_transform(conn.owner, base), &f.matrix12());
        // The anchor turn is the solver's turn (see `joint_flip_now`). The mating side is chosen as the
        // nearer one and the solver turns the first anchor itself, while the stored flag stays as it was. The
        // turn is 180 degrees about X and reverses the Y and Z axes, that is, the travel axis itself.
        // Measured on a slider between anchors facing each other: the arrow agreed with the travel by -1.0000,
        // pointing exactly against the motion. Slot travel is measured in the second anchor, which this turn
        // does not affect.
        if own == j.a {
            let side = crate::asm::bridge::joint_side_now(self, j);
            m = crate::feature::mat_mul12(&m, &crate::asm::bridge::side_turn12(side));
        }
        Some(match axis {
            0 => [m[0], m[4], m[8]],
            1 => [m[1], m[5], m[9]],
            _ => [m[2], m[6], m[10]],
        })
    }

    /// Direction of the angle zero of a mate in the space of `base`: rotation limits are measured from it.
    ///
    /// The angle of a mate is measured between the secondary axes of the anchors (`measured_slot`), so the
    /// zero lies along the secondary axis of the first anchor. The gizmo has to draw the limit arc from that
    /// same zero, or the drawn range and the real one diverge silently.
    pub fn joint_zero_dir(&self, jid: Id, base: Id) -> Option<[f64; 3]> {
        let j = self.joints.iter().find(|x| x.id == jid)?;
        let conn = self.connector(j.a)?;
        let f = self.connector_frame_for_kind(conn, j.kind)?;
        let m = crate::feature::mat_mul12(&self.relative_transform(conn.owner, base), &f.matrix12());
        Some([m[0], m[4], m[8]])
    }

    /// Remaining degrees of freedom of a component: 0 when grounded, 6 when free (no mates), otherwise the
    /// value computed by the assembly solver.
    pub fn component_dof(&self, id: Id) -> u8 {
        if self.is_grounded(id) {
            return 0;
        }
        // A set attached to nothing is free as a whole.
        //
        // The solver counts degrees of freedom relative to neighbours: two bodies rigidly fixed to each other
        // have zero between them, which is true — they do not move relative to one another. That is not what
        // is being asked, though. The question is whether the body is fully constrained, and a floating set is
        // constrained by nothing: it can be moved as a whole. Grouping bodies does not fix them; one of them
        // still has to be attached to something.
        //
        // Computed by walking the mates and groups rather than by rank: the rank of the whole problem would
        // cost a decomposition of a 6N by 6N matrix per tree row.
        if !self.reaches_ground(id) {
            return 6;
        }
        // With assembly mates present, the true count comes from the constraint solver, which accounts for
        // stacked mates (coaxial plus coincident gives zero). Estimating it as "the smallest joint dof" ignores
        // that accumulation and reports 2 where the answer is 0. The degrees of freedom of a specific body are
        // computed by the same solver that places the assembly, or the interface would show one thing while
        // the behaviour was another.
        if let Some((problem, comps, _)) = crate::asm::bridge::problem_of(self) {
            if let Some(bi) = comps.iter().position(|&c| c == id) {
                return crate::asm::iterate::body_dof(&problem, bi).min(6) as u8;
            }
        }
        // Otherwise (purely mechanical joints forming a tree) the smallest joint dof is used.
        let mut best: Option<u8> = None;
        for j in &self.joints {
            let oa = self.connector(j.a).map(|c| c.owner);
            let ob = self.connector(j.b).map(|c| c.owner);
            if oa == Some(id) || ob == Some(id) {
                let d = j.kind.dof();
                best = Some(best.map_or(d, |b| b.min(d)));
            }
        }
        best.unwrap_or(6)
    }

    /// Placement pass: propagation over the tree plus iterative closing of any loops. It writes
    /// `Component.transform` and works in world space, so nesting stays correct.
    pub fn solve_joints(&mut self) -> crate::feature::JointReport {
        use crate::feature::JointReport;
        let mut report = JointReport::default();
        self.mates_conflict = false; // Cleared here; the solve sets it again on a conflict.
        self.mates_violated.clear(); // The by-id list of conflicting mates is filled by the same solve.
                                     // Constraints without mates are work too. Returning early on "no mates" leaves a group or a width
                                     // constraint in a document with no mates doing nothing at all, silently.
        if self.joints.is_empty() && self.mate_constraints.is_empty() {
            return report;
        }
        // Clamp the free slots to their limits before propagation: the driving parameters of the tree are read
        // from the joint values directly, so the clamped value is what places the components.
        for j in &mut self.joints {
            j.clamp_free();
        }
        // One solving path for every kind. Two paths — mechanical joints propagated over a tree and assembly
        // mates solved numerically — are two different sets of mathematics with different behaviour, and a
        // mixed assembly lands in one or the other unpredictably. Both are now joints between anchors and are
        // solved by one solver (`asm`).
        //
        // A mate the solver did not take is named. The bridge skips a mate without a connector or without a
        // resolvable anchor silently, and the assembly then looks assembled although half its mates do not
        // act. Measured on a real document: five mates, two connectors.
        for (jid, why) in self.joint_faults() {
            report.errors.push((jid, why.to_string()));
        }
        // Parts of the document are solved independently, and a nested part lags one step behind its parent.
        //
        // Dragging a subassembly with no mates inside it moves everything smoothly; adding one mate inside it
        // makes exactly the body driven by that mate lag, while the other bodies do not.
        //
        // Why: the document is decomposed into independent parts (`asm/decompose.rs`) and a nested mate forms
        // a part of its own. Every part starts from one initial placement captured before the solve, so the
        // outer mate moves the subassembly while the nested one is computed against the old placement of the
        // parent. The result is written back in local coordinates relative to the already updated parent
        // (parents are written before children). The error is exactly one parent step per frame: the body of
        // the nested mate follows and catches up on the next frame, which reads as rubber-banding.
        //
        // The fix is to repeat the solve until the placement settles. The second pass starts from the updated
        // ancestor placements, so a nested part is computed against the parent it will be written relative to.
        //
        // It costs almost nothing: one solve on a real machine document takes 0.3 ms. At most three passes
        // run, and the loop exits as soon as the placement stops changing.
        let nested = self.components.iter().any(|c| c.parent.is_some_and(|p| p != self.root));
        let passes = if nested { 3 } else { 1 };
        let mut last: Option<crate::asm::decompose::AssemblyReport> = None;
        for k in 0..passes {
            let before: Vec<[f64; 12]> = self.components.iter().map(|c| c.transform).collect();
            last = crate::asm::bridge::solve_project(self);
            let after = self.components.iter().map(|c| c.transform);
            let settled = before.iter().zip(after).all(|(a, b)| a.iter().zip(b.iter()).all(|(x, y)| (x - y).abs() < 1e-9));
            if settled || last.is_none() {
                let _ = k;
                break;
            }
        }
        if let Some(r) = last {
            self.mates_conflict = !r.converged;
            // Violated mates are reported by id rather than by a single "there is a conflict" flag.
            for (jid, v) in &r.violated {
                report.errors.push((*jid as Id, format!("joint-miss#{v:.3}")));
                if !self.mates_violated.contains(&(*jid as Id)) {
                    self.mates_violated.push(*jid as Id);
                }
            }
        }
        report
    }
}
