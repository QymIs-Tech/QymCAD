//! THE HIERARCHY OF COMPONENTS: depth, children, ancestors, reparenting, groups, mirrored parts.

use super::*;

impl Project {
    /// Depth of a component in the tree (its number of ancestors), used to order transform writes so that a
    /// parent is written before its children.
    pub(crate) fn component_depth(&self, id: Id) -> usize {
        let mut d = 0usize;
        let mut cur = self.components.iter().find(|c| c.id == id).and_then(|c| c.parent);
        while let Some(c) = cur {
            d += 1;
            cur = self.components.iter().find(|x| x.id == c).and_then(|x| x.parent);
        }
        d
    }
    /// Direct child components of component `id`.
    pub fn component_children(&self, id: Id) -> Vec<Id> {
        self.components.iter().filter(|c| c.parent == Some(id)).map(|c| c.id).collect()
    }
    /// Every descendant component of `id` (the whole subtree, excluding `id` itself).
    pub fn descendants(&self, id: Id) -> Vec<Id> {
        let mut out = Vec::new();
        let mut stack = vec![id];
        while let Some(cur) = stack.pop() {
            for c in self.components.iter().filter(|c| c.parent == Some(cur)) {
                out.push(c.id);
                stack.push(c.id);
            }
        }
        out
    }
    /// The component at the level of context `ctx` that contains `node`: the direct child of `ctx` in the
    /// ownership chain of `node`, or `node` itself when it already is a direct child. `None` means `node` lies
    /// outside the subtree of `ctx`.
    ///
    /// Needed so that clicking a body of a leaf part inside a subassembly operates on the subassembly as a
    /// unit — selection and mates at the level of the context rather than of the leaf.
    pub fn ancestor_child_of(&self, ctx: Id, node: Id) -> Option<Id> {
        let parent = |x: Id| self.components.iter().find(|c| c.id == x).and_then(|c| c.parent);
        let mut cur = node;
        loop {
            match parent(cur) {
                Some(p) if p == ctx => return Some(cur),
                Some(p) => cur = p,
                None => return None, // Reached the root without meeting `ctx`.
            }
        }
    }
    /// Whether component `node` lies inside the subtree of `ancestor`, including `ancestor` itself. Walks up
    /// through `parent`.
    pub fn component_is_within(&self, node: Id, ancestor: Id) -> bool {
        let mut cur = Some(node);
        while let Some(c) = cur {
            if c == ancestor {
                return true;
            }
            cur = self.components.iter().find(|x| x.id == c).and_then(|x| x.parent);
        }
        false
    }
    /// Lowest common ancestor of two components, walking up through `parent`. It includes the nodes
    /// themselves when one is an ancestor of the other. `None` means there is no common ancestor.
    pub fn common_ancestor(&self, a: Id, b: Id) -> Option<Id> {
        let parent = |x: Id| self.components.iter().find(|c| c.id == x).and_then(|c| c.parent);
        let mut chain_a = Vec::new();
        let mut cur = Some(a);
        while let Some(c) = cur {
            chain_a.push(c);
            cur = parent(c);
        }
        let mut cur = Some(b);
        while let Some(c) = cur {
            if chain_a.contains(&c) {
                return Some(c);
            }
            cur = parent(c);
        }
        None
    }
    /// Whether a joint is shown in context `ctx` (in the list, the glyphs and the popup).
    ///
    /// By default it appears in its own home (the lowest common ancestor of the connector owners). A `global`
    /// joint is promoted to the root and is visible both there, to be driven, and in its home, so the flag can
    /// be cleared. A joint whose home is the root ignores `global`.
    pub fn joint_in_context(&self, j: &crate::feature::Joint, ctx: Id) -> bool {
        let home = self.joint_home(j);
        // A mate with no home is shown at the root instead of disappearing. The home is derived from the
        // connector owners, so a lost connector leaves no home and the mate vanishes from the list everywhere
        // while still existing in the document: invisible, unselectable, undeletable, and skipped by the
        // solver. Measured on a real document where four mates out of five were in that state.
        if home.is_none() {
            return ctx == self.root;
        }
        home == Some(ctx) || (j.global && ctx == self.root && home != Some(self.root))
    }
    /// Re-parent component `id` (a part or a subassembly) under `target_parent`, that is, cut and paste.
    /// Moving the root, or moving a component into itself or its own subtree (a cycle), is refused. Returns
    /// whether it succeeded.
    pub fn reparent_component(&mut self, id: Id, target_parent: Id) -> bool {
        if id == self.root || id == target_parent {
            return false;
        }
        if !self.components.iter().any(|c| c.id == id) || !self.components.iter().any(|c| c.id == target_parent) {
            return false;
        }
        if self.component_is_within(target_parent, id) {
            return false; // The target lies inside the subtree being moved: that would be a cycle.
        }
        if self.component_is_part(target_parent) {
            return false; // A part inside a part is not allowed: a component goes into an assembly only.
        }
        // Preserve the world placement across the move: the new local transform is
        // `inv(world(target)) * world(id)`, computed before the mutation so that `world(id)` still uses the old
        // parent. This keeps a body from jumping when it is moved into a transformed subassembly; for a
        // subassembly sitting at zero, the usual case, the result is unchanged.
        let keep = crate::feature::mat_mul12(&crate::feature::mat_inv12(&self.world_transform(target_parent)), &self.world_transform(id));
        if let Some(c) = self.components.iter_mut().find(|c| c.id == id) {
            c.parent = Some(target_parent);
            c.transform = keep;
            return true;
        }
        false
    }
    /// Reorder a component among its siblings, so parts and subassemblies can be dragged in the tree.
    ///
    /// The tree draws children in `Project::components` order filtered by parent, so reordering means moving
    /// the element within that vector. `before = None` puts it at the end of the sibling list.
    ///
    /// Note for callers: component indices change afterwards. Anything that remembers a component by index
    /// (a tree selection) has to be recomputed by id, or the selection moves to a neighbour silently.
    ///
    /// Returns `false` when there is nothing to reorder: no such component, the target is not a sibling, or it
    /// is the component itself.
    pub fn reorder_component_before(&mut self, id: Id, before: Option<Id>) -> bool {
        if Some(id) == before || id == self.root {
            return false;
        }
        let Some(from) = self.component_index(id) else { return false };
        let par = self.components[from].parent;
        if par.is_none() {
            return false; // The root is not reordered.
        }
        // Siblings only. Dragging within a level changes the order, while moving to a different parent is a
        // separate operation (`reparent_component`) that also has to preserve the world placement.
        if let Some(b) = before {
            match self.component_index(b) {
                Some(bi) if self.components[bi].parent == par => {}
                _ => return false,
            }
        }
        let c = self.components.remove(from);
        let at = match before {
            Some(b) => self.component_index(b).unwrap_or(self.components.len()),
            None => {
                // To the end of its own siblings rather than of the whole vector, or children of other
                // parents would end up above it.
                self.components.iter().rposition(|x| x.parent == par).map(|i| i + 1).unwrap_or(self.components.len())
            }
        };
        self.components.insert(at.min(self.components.len()), c);
        true
    }

    /// Whether a drop is allowed here. Asked before the drop: while the target is invalid the tree draws
    /// neither an insertion line nor a highlight, so the refusal is visible in advance rather than discovered
    /// after releasing.
    ///
    /// `onto` means dropping onto an item (into an assembly, or into a new subassembly together with a part);
    /// otherwise it is a reorder among siblings.
    ///
    /// The rules are the ones the operations themselves use, which is why they live here next to them:
    /// otherwise the highlight and the action would diverge unnoticed.
    pub fn tree_drop_allowed(&self, moving: &[Id], target: Id, onto: bool) -> bool {
        if target == self.root || self.component_index(target).is_none() {
            return false;
        }
        let live: Vec<Id> = moving.iter().copied().filter(|&m| m != target && m != self.root && self.component_index(m).is_some()).collect();
        if live.is_empty() {
            return false;
        }
        if onto {
            // An ancestor is not placed inside its own descendant: that would be a cycle.
            live.iter().any(|&m| !self.component_is_within(target, m))
        } else {
            // Reordering happens among siblings only; moving to a different parent is a separate
            // operation.
            let par = self.components.iter().find(|c| c.id == target).and_then(|c| c.parent);
            live.iter().any(|&m| self.components.iter().find(|c| c.id == m).and_then(|c| c.parent) == par)
        }
    }

    /// Group the selection and the drop target into a new subassembly: dropping selected parts or
    /// subassemblies onto another one creates a subassembly holding all of them plus the target.
    ///
    /// The subassembly appears where the target was, with an identity placement, so nothing inside it moves:
    /// `reparent_component` recomputes the local transforms to preserve the world placement.
    ///
    /// Returns the id of the new subassembly. `None` when there is nothing to group: the target is the root,
    /// or the set contains no component that can be moved into it.
    pub fn group_components_into_assembly(&mut self, dragged: &[Id], target: Id, name: impl Into<String>) -> Option<Id> {
        let ti = self.component_index(target)?;
        let par = self.components[ti].parent?; // The root is never nested inside anything.
                                               // Members: the target plus the selection, without duplicates and without anything that contains the
                                               // target. Placing an ancestor into a group holding its own descendant is a cycle;
                                               // `reparent_component` would refuse it anyway, but that would leave a half-empty group and no
                                               // explanation.
        let mut members: Vec<Id> = vec![target];
        for &d in dragged {
            if d == target || d == self.root || members.contains(&d) {
                continue;
            }
            if self.component_index(d).is_none() || self.component_is_within(target, d) {
                continue;
            }
            members.push(d);
        }
        if members.len() < 2 {
            return None; // A group of one is not a group.
        }
        let asm = self.add_component_kind(name, crate::feature::ComponentKind::Assembly);
        // The new assembly may have been created in the wrong place (`add_component_kind` attaches it to the
        // active context), so it is placed next to the target explicitly.
        if let Some(ai) = self.component_index(asm) {
            self.components[ai].parent = Some(par);
            self.components[ai].transform = crate::feature::PLACE_IDENTITY;
        }
        let mut moved = 0;
        for m in members {
            if self.reparent_component(m, asm) {
                moved += 1;
            }
        }
        if moved == 0 {
            // Nothing moved, so the empty assembly is not left as debris in the tree. A component has no
            // timeline node (`add_component_kind` only touches `components`), so removing it from there is
            // enough.
            self.components.retain(|c| c.id != asm);
            return None;
        }
        Some(asm)
    }

    /// World transform of a component: the composition of `Component.transform` up the tree to the root. The
    /// root (`parent == None`) keeps its own transform as it is, usually the identity.
    pub fn world_transform(&self, id: Id) -> [f64; 12] {
        use crate::feature::{mat_mul12, PLACE_IDENTITY};
        let Some(c) = self.components.iter().find(|c| c.id == id) else {
            return PLACE_IDENTITY;
        };
        match c.parent {
            Some(p) => mat_mul12(&self.world_transform(p), &c.transform),
            None => c.transform,
        }
    }
    /// A free name for a new part.
    ///
    /// Taking the number as "how many parts exist plus one" produces two identically named parts as soon as
    /// one of them was created by another path, and they are indistinguishable in the tree: one is selected
    /// while the other is edited.
    /// The first name of the "Part n" kind no component holds yet.
    pub fn free_part_name(&self) -> String {
        let taken: std::collections::HashSet<String> = self.components.iter().map(|c| c.name.clone()).collect();
        (1..).map(|n| format!("name-part-n#{n}")).find(|nm| !taken.contains(nm)).unwrap_or_else(|| "name-part-n#1".into())
    }

    pub(crate) fn body_parent(&mut self) -> Id {
        let ctx = self.active_ctx();
        if self.ctx_holds_bodies(ctx) {
            return ctx;
        }
        // An empty part is reused rather than duplicated. A new document already creates one part, and the
        // first feature would create a second, leaving an empty row in the tree with nothing behind it: two
        // parts listed where one was made.
        let free = self.components.iter().find(|c| c.kind == crate::feature::ComponentKind::Part && c.parent == Some(ctx) && !self.timeline.iter().any(|n| n.parent == Some(c.id))).map(|c| c.id);
        if let Some(free) = free {
            self.set_active_component(Some(free));
            return free;
        }
        let part = self.add_part(self.free_part_name());
        self.set_active_component(Some(part));
        part
    }
    /// A mirrored part inside a rigidly mirrored subassembly (see `add_mirror_component`).
    ///
    /// The rotation of the part relative to the subassembly stays exactly as in the original and is not
    /// mirrored, while its local offset relative to the subassembly is reflected by the same formula as the
    /// world zero of the subassembly. Otherwise a hand to the right of a shoulder would end up to the left of
    /// it in the mirrored arm, the offset staying on the same world side instead of being reflected.
    ///
    /// This keeps the internal structure — who stands where relative to the parent — a geometrically honest
    /// mirror of the original rather than a set of unrelated absolute numbers.
    pub(crate) fn add_mirror_part_rigid(&mut self, part: Id, sa_src: Id, n_sa: [f64; 3], wn: [f64; 3], asm: Id) -> Id {
        let src_name = self.components.iter().find(|c| c.id == part).map(|c| c.name.clone()).unwrap_or("name-part".into());
        let saved = self.active_component;
        self.active_component = Some(asm);
        let new_part = self.add_part(format!("name-mirror-of#{src_name}"));
        self.active_component = saved;
        let l_p = self.relative_transform(part, sa_src); // The original local transform of the part inside the subassembly.
                                                         // The part offset (a point in subassembly local space) is reflected through the plane (0, n_sa) — the
                                                         // same formula as for the world zero of the subassembly, with the local space of `sa_src` playing the
                                                         // role of the world. The rotation is left alone.
        let lt = [l_p[3], l_p[7], l_p[11]];
        let ld = lt[0] * n_sa[0] + lt[1] * n_sa[1] + lt[2] * n_sa[2];
        let mut l_new = l_p;
        l_new[3] = lt[0] - 2.0 * ld * n_sa[0];
        l_new[7] = lt[1] - 2.0 * ld * n_sa[1];
        l_new[11] = lt[2] - 2.0 * ld * n_sa[2];
        self.set_component_transform(new_part, l_new);
        // The geometry follows the single-part case: the normal is pulled back through the part's own world
        // rotation.
        let ln = crate::feature::apply12_dir(&crate::feature::mat_inv12(&self.world_transform(part)), wn);
        let body = self.alloc_id();
        use crate::feature::{FeatureKind, FeatureNode};
        self.push_timeline(FeatureNode {
            id: body,
            name: "name-mirror-part".into(),
            kind: FeatureKind::MirrorPart { src_comp: part, ln, body },
            parent: Some(new_part),
            dirty: true,
            suppressed: false,
        });
        body
    }
    /// Mirror of a single part: a new sibling of the source (under `src_parent`).
    ///
    /// The placement is split into shape and position. The shape is reflected in local space through a plane
    /// passing through the part's own local zero — no translation, only a rotated normal — while the position
    /// puts the local zero of the copy at the reflection of the local zero of the source through the world
    /// plane `wo`/`wn`, keeping the orientation of the source.
    ///
    /// The identity `T * Refl_A(x) = Refl_{T(A)}(T(x))` for a rigid `T` guarantees the same world geometry as a
    /// direct world reflection; what changes is where the gizmo sits, so it travels with the mirrored body
    /// instead of staying at the source.
    pub fn add_mirror_part(&mut self, src_comp: Id, wo: [f64; 3], wn: [f64; 3]) -> Id {
        let (src_name, src_parent) = self.components.iter().find(|c| c.id == src_comp).map(|c| (c.name.clone(), c.parent)).unwrap_or(("name-part".into(), None));
        let saved = self.active_component;
        self.active_component = src_parent.or(saved);
        let part = self.add_part(format!("name-mirror-of#{src_name}"));
        self.active_component = saved;
        let wt = self.world_transform(src_comp);
        let ln = crate::feature::apply12_dir(&crate::feature::mat_inv12(&wt), wn); // Rotation only; the plane passes through the local zero.
                                                                                   // The world point (the local zero of the source) reflected through (wo, wn): p' = p - 2*((p-wo).n)*n.
        let t = [wt[3], wt[7], wt[11]];
        let d = (t[0] - wo[0]) * wn[0] + (t[1] - wo[1]) * wn[1] + (t[2] - wo[2]) * wn[2];
        let t_new = [t[0] - 2.0 * d * wn[0], t[1] - 2.0 * d * wn[1], t[2] - 2.0 * d * wn[2]];
        let mut wt_new = wt; // Same orientation and scale as the source; only the world zero changes.
        wt_new[3] = t_new[0];
        wt_new[7] = t_new[1];
        wt_new[11] = t_new[2];
        let pw = self.components.iter().find(|c| c.id == part).and_then(|c| c.parent).map(|pp| self.world_transform(pp)).unwrap_or(crate::feature::PLACE_IDENTITY);
        self.set_component_transform(part, crate::feature::mat_mul12(&crate::feature::mat_inv12(&pw), &wt_new));
        let body = self.alloc_id();
        use crate::feature::{FeatureKind, FeatureNode};
        self.push_timeline(FeatureNode { id: body, name: "name-mirror-part".into(), kind: FeatureKind::MirrorPart { src_comp, ln, body }, parent: Some(part), dirty: true, suppressed: false });
        body
    }
}
