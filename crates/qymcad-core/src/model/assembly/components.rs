//! THE COMPONENTS: their kinds, places, visibility, transforms, grounding.

use super::*;

impl Project {
    /// Component kind by id.
    pub fn component_kind(&self, id: Id) -> Option<crate::feature::ComponentKind> {
        self.components.iter().find(|c| c.id == id).map(|c| c.kind)
    }
    /// CAN BODY `body` BE MADE A PART of its own: it is one of the bodies its part shows, and not the only one.
    pub fn may_be_made_a_part(&self, body: Id) -> bool {
        let Some(part) = self.body_owner(body).filter(|p| self.ctx_holds_bodies(*p)) else { return false };
        let consumed = self.consumed_bodies();
        !consumed.contains(&body) && self.component_bodies(part).into_iter().filter(|b| !consumed.contains(b)).count() >= 2
    }

    /// MAKE A PIECE A PART: body `body`, one of the several bodies of its part - a piece a cut or a split left - leaves
    /// the part and becomes the body of a new part beside it, in the same assembly and in the same place. The new part
    /// starts from a `Piece` node reading that body, so an edit of the cut that made the pieces rebuilds both. `None`
    /// when the body is no body of a part, or the only one it shows: taking it would leave the part empty. `name` is
    /// the new part's; the caller picks one free as it reads (see [`Project::free_component_name_shown`]).
    pub fn piece_to_part(&mut self, body: Id, name: String) -> Option<(Id, Id)> {
        use crate::feature::{FeatureKind, FeatureNode};
        if !self.may_be_made_a_part(body) {
            return None;
        }
        let part = self.body_owner(body)?;
        let was = self.active_component;
        let (home, place) = self.components.iter().find(|c| c.id == part).map(|c| (c.parent, c.transform))?;
        self.set_active_component(home);
        let new_part = self.add_part(name);
        if let Some(c) = self.components.iter_mut().find(|c| c.id == new_part) {
            c.transform = place;
        }
        let piece = self.alloc_id();
        self.push_timeline(FeatureNode { id: piece, name: "feat-name-piece-in".into(), kind: FeatureKind::Piece { src: body, body: piece }, parent: Some(new_part), dirty: true, suppressed: false });
        self.set_active_component(was);
        Some((new_part, piece))
    }

    /// Component index by id.
    pub fn component_index(&self, id: Id) -> Option<usize> {
        self.components.iter().position(|c| c.id == id)
    }
    /// Make a component active, so new nodes nest inside it. `None` selects the root.
    pub fn set_active_component(&mut self, id: Option<Id>) {
        self.active_component = id;
    }
    /// Placement of a component inside its parent (3x4). This does not rebuild any body, only its position in
    /// the assembly.
    /// SHOW OR HIDE A COMPONENT.
    ///
    /// One assignment, but it belongs here rather than in a panel: what "visible" means for a component is
    /// the document's rule, and the caches that depend on it are dropped by whoever owns them.
    /// Returns `false` when there is no such component, so a miss is told apart from a change.
    pub fn set_component_visible(&mut self, id: Id, visible: bool) -> bool {
        match self.components.iter_mut().find(|c| c.id == id) {
            Some(c) => {
                c.visible = visible;
                true
            }
            None => false,
        }
    }

    /// RENAME A COMPONENT.
    pub fn rename_component(&mut self, id: Id, name: impl Into<String>) -> bool {
        match self.components.iter_mut().find(|c| c.id == id) {
            Some(c) => {
                c.name = name.into();
                true
            }
            None => false,
        }
    }

    /// WHERE A MATE CONNECTOR SITS ON ITS BODY: the point, the turn and the offset, SET TOGETHER.
    ///
    /// They were assigned one by one from the panel, which is how a half-placed connector becomes possible:
    /// three separate writes have two moments in between when the record disagrees with itself. One call,
    /// one moment.
    pub fn set_connector_placement(&mut self, id: Id, point: crate::asm::connector::AttachPoint, rot_deg: f64, offset_xyz: [f64; 3]) -> bool {
        match self.connectors.iter_mut().find(|c| c.id == id) {
            Some(c) => {
                c.point = point;
                c.rot_deg = rot_deg;
                c.offset_xyz = offset_xyz;
                true
            }
            None => false,
        }
    }

    pub fn set_component_transform(&mut self, id: Id, mat: [f64; 12]) {
        if let Some(c) = self.components.iter_mut().find(|c| c.id == id) {
            c.transform = mat;
        }
    }
    /// Current placement of a component (3x4). The identity means it sits at the parent zero.
    pub fn component_transform(&self, id: Id) -> [f64; 12] {
        self.components.iter().find(|c| c.id == id).map(|c| c.transform).unwrap_or(crate::feature::PLACE_IDENTITY)
    }
    /// Move a component by a vector in parent space, added to the translation part.
    pub fn move_component(&mut self, id: Id, delta: [f64; 3]) {
        if let Some(c) = self.components.iter_mut().find(|c| c.id == id) {
            c.transform[3] += delta[0];
            c.transform[7] += delta[1];
            c.transform[11] += delta[2];
        }
    }
    /// Rotate a component about world axis `axis` (0 X, 1 Y, 2 Z) by `deg` degrees, composed on the left as
    /// R * T.
    pub fn rotate_component(&mut self, id: Id, axis: u8, deg: f64) {
        let r = rot_axis_mat(axis, deg);
        if let Some(i) = self.components.iter().position(|c| c.id == id) {
            self.components[i].transform = crate::feature::mat_mul12(&r, &self.components[i].transform);
        }
    }
    /// Ground or release a component: a grounded one is fixed in the assembly and mates do not drive it.
    pub fn set_grounded(&mut self, id: Id, grounded: bool) {
        if let Some(c) = self.components.iter_mut().find(|c| c.id == id) {
            c.grounded = grounded;
        }
    }
    /// Whether a component is grounded.
    pub fn is_grounded(&self, id: Id) -> bool {
        self.components.iter().find(|c| c.id == id).map(|c| c.grounded).unwrap_or(false)
    }
    /// Whether a component actually stands still: grounded itself, or holding something grounded inside.
    ///
    /// A grounded part means a part that stands in the world. But a part lives inside a subassembly, and the
    /// placement of that subassembly is its own business: move it and the grounded part travels with it, at
    /// which point the word "grounded" means nothing.
    ///
    /// Measured on a machine document: a grounded part sat inside a free subassembly. Every recompute moved
    /// the subassembly by 60 mm along Z and the grounded part honestly followed it: -240.000, -300.000,
    /// -360.000, -420.000. The assembly drifted apart by itself, without end.
    ///
    /// So an ancestor of something grounded stands still too. This is a consequence rather than a
    /// prohibition: a subassembly with something fixed inside cannot be moved, or the fixed thing is not
    /// fixed.
    pub fn is_effectively_grounded(&self, id: Id) -> bool {
        self.is_grounded(id) || self.descendants(id).into_iter().any(|d| self.is_grounded(d))
    }
}
