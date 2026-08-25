//! Assemblies: components, the tree, mates and external references.
//!
//! The split is mechanical: a method belongs here because it touches the fields of this subsystem alone, not
//! because of what it is called. Names are chosen by hand, so searching by name finds only what is expected.

use super::*;
use super::tess::*; // 2D sketch geometry: profiles, tessellation, region analysis.

impl Project {
    /// Colour of a part by mesh index (RGB), keyed by lineage root: a manual colour from `part_colors`, or the
    /// palette entry for the stable index of that root, so the shade does not change with every operation.
    pub fn mesh_color(&self, index: usize) -> [u8; 3] {
        match self.mesh_id(index) {
            Some(body) => {
                let root = self.lineage_root(body);
                self.part_colors.get(&root).copied().unwrap_or_else(|| default_part_color(self.part_color_index(root)))
            }
            None => default_part_color(index),
        }
    }
    /// Set a manual part colour, keyed by lineage root, so it applies to the whole feature chain of that
    /// part.
    pub fn set_mesh_color(&mut self, index: usize, rgb: [u8; 3]) {
        if let Some(body) = self.mesh_id(index) {
            let root = self.lineage_root(body);
            self.part_colors.insert(root, rgb);
        }
    }
    /// Clear a manual part colour and fall back to the palette entry for its lineage root.
    pub fn reset_mesh_color(&mut self, index: usize) {
        if let Some(body) = self.mesh_id(index) {
            let root = self.lineage_root(body);
            self.part_colors.remove(&root);
        }
    }
    /// Root assembly component of the document, created lazily. Returns its id.
    pub fn ensure_root(&mut self) -> Id {
        use crate::feature::{Component, ComponentKind, PLACE_IDENTITY};
        if self.root != 0 && self.components.iter().any(|c| c.id == self.root) {
            return self.root;
        }
        // A root component (`parent == None`) may already exist in the document.
        if let Some(c) = self.components.iter().find(|c| c.parent.is_none()) {
            self.root = c.id;
            return self.root;
        }
        let id = self.alloc_id();
        self.components.push(Component { id, name: "name-assembly".into(), kind: ComponentKind::Assembly, parent: None, transform: PLACE_IDENTITY, visible: true, grounded: false });
        self.root = id;
        id
    }
    /// Active context for creating nodes: `active_component`, or the root. It creates the root lazily, so the
    /// very first `add_*` is guaranteed to nest inside a real component rather than under id 0.
    pub fn active_ctx(&mut self) -> Id {
        self.ensure_root();
        self.active_component.filter(|id| *id != 0).unwrap_or(self.root)
    }
    /// Active context for queries taking `&self`: it does not create the root, and yields 0 when there is
    /// none yet.
    pub fn current_ctx(&self) -> Id {
        self.active_component.filter(|id| *id != 0).unwrap_or(self.root)
    }
    /// Add a component of the given kind to the active context. Returns its id.
    pub fn add_component_kind(&mut self, name: impl Into<String>, kind: crate::feature::ComponentKind) -> Id {
        use crate::feature::{Component, PLACE_IDENTITY};
        self.ensure_root();
        let id = self.alloc_id();
        // A part is never placed inside a part. A part is a body with a build history; a component inside it
        // would give a structure that does not exist in CAD: one tree row holding both nested parts and its own
        // construction. Attaching the component to the active context silently lets such a structure reach the
        // file. Instead the new component is lifted to the nearest assembly, which is what "new part" means
        // while standing inside a part.
        let mut ctx = self.active_ctx();
        while self.components.iter().any(|c| c.id == ctx && matches!(c.kind, crate::feature::ComponentKind::Part)) {
            match self.components.iter().find(|c| c.id == ctx).and_then(|c| c.parent) {
                Some(up) => ctx = up,
                None => break,
            }
        }
        let parent = Some(ctx);
        self.components.push(Component { id, name: name.into(), kind, parent, transform: PLACE_IDENTITY, visible: true, grounded: false });
        id
    }
    /// Import STEP solids as parts. The body meshes have already been added by the application (`add_mesh`),
    /// and what arrives here is `(body, name, source, solid index)`.
    ///
    /// One solid becomes one part in the active context; several become a subassembly named `group_name` with
    /// one part per solid, each holding an `Import` node that sketches, chamfers and fillets can be built on.
    /// Returns the id of the created root (a part or a subassembly). One method, because this is a topology
    /// operation.
    pub fn import_bodies_as_parts(&mut self, solids: Vec<(Id, String, Id, u32)>, group_name: &str) -> Option<Id> {
        use crate::feature::{FeatureKind, FeatureNode};
        if solids.is_empty() {
            return None;
        }
        self.ensure_root();
        let saved = self.active_component;
        let import_node = |p: &mut Self, body: Id, source: Id, solid: u32, parent: Id| {
            p.push_timeline(FeatureNode { id: body, name: "name-import".into(), kind: FeatureKind::Import { body, source, solid }, parent: Some(parent), dirty: true, suppressed: false });
        };
        let single = (solids.len() == 1).then(|| solids.first().cloned()).flatten();
        let created = if let Some((body, name, source, solid)) = single {
            let part = self.add_part(name);
            import_node(self, body, source, solid, part);
            part
        } else {
            let asm = self.add_assembly(group_name);
            self.set_active_component(Some(asm)); // The parts are created inside the subassembly.
            for (body, name, source, solid) in solids {
                let part = self.add_part(name);
                import_node(self, body, source, solid, part);
            }
            asm
        };
        self.set_active_component(saved);
        Some(created)
    }
    /// Component kind by id.
    pub fn component_kind(&self, id: Id) -> Option<crate::feature::ComponentKind> {
        self.components.iter().find(|c| c.id == id).map(|c| c.kind)
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
    /// Connector by id.
    pub fn connector(&self, id: Id) -> Option<&crate::feature::MateConnector> {
        self.connectors.iter().find(|c| c.id == id)
    }
    /// Create a connector on component `owner` with anchor `anchor`. Returns its id.
    pub fn add_connector(&mut self, owner: Id, anchor: crate::feature::AnchorRef) -> Id {
        let id = self.alloc_id();
        let n = self.connectors.len() + 1;
        self.connectors.push(crate::feature::MateConnector {
            id,
            name: format!("name-connector-n#{n}"),
            standalone: false,
            owner,
            anchor,
            point: Default::default(),
            rot_deg: 0.0,
            flip: false,
            offset_xyz: [0.0; 3],
            axis_ref: None,
        });
        id
    }

    /// Create a standalone connector: one made on its own rather than for a single mate.
    ///
    /// Such a connector survives the deletion of any mate: a second mate may be attached to it later, and
    /// deciding on the author's behalf that the anchor is no longer needed is not the model's call.
    pub fn add_connector_standalone(&mut self, owner: Id, anchor: crate::feature::AnchorRef) -> Id {
        let id = self.add_connector(owner, anchor);
        if let Some(c) = self.connectors.iter_mut().find(|c| c.id == id) {
            c.standalone = true;
        }
        id
    }

    /// Who uses this connector: the mates and constraints that reference it.
    ///
    /// Needed because a connector is now an element in its own right, and an element in its own right can be
    /// deleted. Deleting an anchor a mate rests on breaks that mate silently: it stays in the list and stops
    /// acting. The list of users is the answer to why the deletion is refused.
    pub fn connector_users(&self, conn: Id) -> Vec<Id> {
        let mut out: Vec<Id> = self.joints.iter().filter(|j| j.a == conn || j.b == conn).map(|j| j.id).collect();
        out.extend(self.mate_constraints.iter().filter(|g| g.anchors.contains(&conn)).map(|g| g.id));
        out
    }

    /// Delete a connector. `false` means mates reference it, in which case nothing is deleted.
    pub fn delete_connector(&mut self, conn: Id) -> bool {
        if !self.connector_users(conn).is_empty() {
            return false;
        }
        let before = self.connectors.len();
        self.connectors.retain(|c| c.id != conn);
        before != self.connectors.len()
    }
    /// Set the owner and the geometry of an anchor. When no connector with that id exists, it is recreated.
    ///
    /// The connector frame is derived from `anchor` while solving the mates, so no separate frame update is
    /// needed. Used by mate editing, to replace face, edge or vertex A or B without recreating the joint.
    ///
    /// Handling only the existing case (`if let Some(...)`) makes the method do nothing for a lost anchor
    /// while the interface still reports "anchor A replaced". That is what leaves a damaged document
    /// incurable: mates reference anchors the document no longer holds (removed by an older deletion defect),
    /// and replacing the anchor gives a cheerful answer and a dead mate.
    ///
    /// A mate remembers its anchor by id, so the anchor has to be recreated under the same id, or the mate
    /// keeps pointing at nothing. This is the only way to repair an already damaged document.
    pub fn set_connector_anchor(&mut self, cid: Id, owner: Id, anchor: crate::feature::AnchorRef) {
        if let Some(c) = self.connectors.iter_mut().find(|c| c.id == cid) {
            c.owner = owner;
            c.anchor = anchor;
            self.connector_geometry_changed(cid);
            return;
        }
        let n = self.connectors.len() + 1;
        self.connectors.push(crate::feature::MateConnector {
            id: cid,
            name: format!("name-connector-n#{n}"),
            standalone: false,
            owner,
            anchor,
            point: Default::default(),
            rot_deg: 0.0,
            flip: false,
            offset_xyz: [0.0; 3],
            axis_ref: None,
        });
    }

    /// The geometry of an anchor changed, so the mates resting on it decide their side again.
    ///
    /// Measured on the path "mate panel, replace anchor, B, click a face": the body turned by exactly 180.000
    /// degrees.
    ///
    /// The cause is the mating side. Aligning axes admits two sides, the nearer to the current placement is
    /// chosen, and the answer is frozen in the mate (`flip_decided`) because recomputing it on every solve
    /// would rock the body between two solutions. But what was frozen is the answer chosen for the previous
    /// pair of anchors: replace the face and the mate holds the body turned according to geometry that no
    /// longer exists.
    ///
    /// The second trace of the same edit is cleared here too: a "hold as built" declaration was captured in
    /// the frame of the old anchor. Keeping it would declare as "as built" a placement the body never
    /// occupied, so it is re-captured from the current placement rather than discarded: the request was to
    /// hold the body where it stands, and replacing an anchor does not cancel that request.
    ///
    /// Every path that changes the anchor axes has to call this (replacing a face, naming an axis, clearing
    /// that, flipping), or the defect returns through whichever door was forgotten.
    pub fn connector_geometry_changed(&mut self, cid: Id) {
        let touched: Vec<(Id, bool)> = self.joints.iter().filter(|j| j.a == cid || j.b == cid).map(|j| (j.id, j.as_built.is_some())).collect();
        for (jid, was_as_built) in touched {
            if let Some(j) = self.joints.iter_mut().find(|x| x.id == jid) {
                j.flip_decided = false;
            }
            if was_as_built {
                self.set_joint_as_built(jid); // Re-capture the placement, so the body stays where it stands.
            }
        }
    }

    /// Set the secondary axis of an anchor from an explicit pick, or clear the pick and fall back to deriving
    /// it from geometry.
    ///
    /// A method rather than a field write from the interface: for a slider the picked axis becomes the travel
    /// axis (`connector_frame_for_kind`), that is, the main axis of the frame, and the chosen mating side
    /// depends on it. Writing the field around the core would leave the frozen answer from the old axis and
    /// tumble the body exactly as replacing a face does.
    pub fn set_connector_axis_ref(&mut self, cid: Id, axis: Option<crate::feature::AnchorRef>) -> bool {
        let Some(c) = self.connectors.iter_mut().find(|c| c.id == cid) else { return false };
        if c.axis_ref == axis {
            return false;
        }
        c.axis_ref = axis;
        self.connector_geometry_changed(cid);
        true
    }

    /// Flip the mating side: the "reverse axis" control and the "other side" checkbox.
    ///
    /// Aligning axes admits two sides — the main axes of the anchors either point the same way or face each
    /// other. The automatic choice takes the nearer to the current placement, so the body does not move
    /// needlessly, while this control requests the opposite one and the body turns by 180 degrees.
    ///
    /// Marking the side as decided is required: without it the automatic choice picks the previous side on the
    /// very next solve (the body still stands where it was) and the control does nothing. Flipping the axis on
    /// the connector instead (`MateConnector::flip`) does not work either — the side is then chosen afresh as
    /// the nearer one and returns the body to its place, the two flips cancelling out.
    pub fn flip_joint_side(&mut self, jid: Id) -> bool {
        let Some(j) = self.joints.iter().find(|x| x.id == jid).cloned() else { return false };
        // The main axis is what flips; the secondary one stays as chosen. These are two separate controls and
        // must not be mixed — roll is turned by its own control on the connector.
        let side = crate::asm::bridge::joint_side_now(self, &j);
        let Some(x) = self.joints.iter_mut().find(|x| x.id == jid) else { return false };
        x.flip = !side.flip;
        x.roll_flip = side.roll_flip;
        x.flip_decided = true;
        true
    }

    /// The mating side the mate currently holds, for the checkbox in the interface.
    ///
    /// Reading the raw `flip` field is wrong: until the side is decided that field holds a default while the
    /// mate is held by the automatic answer, so the checkbox would show one thing and the body would stand
    /// according to another.
    pub fn joint_side_flipped(&self, jid: Id) -> bool {
        self.joints.iter().find(|x| x.id == jid).is_some_and(|j| crate::asm::bridge::joint_flip_now(self, j))
    }

    /// Change the joint kind while keeping the anchors.
    ///
    /// The mating side is decided again: the anchor frame is built for the kind
    /// (`connector_frame_for_kind`), and for a slider the main axis is the travel direction rather than the
    /// face normal. A frozen answer belongs to the frame of the other kind, so keeping it turns the body
    /// inside out when the kind changes.
    pub fn set_joint_kind(&mut self, jid: Id, kind: crate::feature::JointKind) -> bool {
        let Some(j) = self.joints.iter_mut().find(|x| x.id == jid) else { return false };
        if j.kind == kind {
            return false;
        }
        j.kind = kind;
        j.flip_decided = false;
        let was_as_built = j.as_built.is_some();
        if was_as_built {
            self.set_joint_as_built(jid);
        }
        true
    }

    /// Create a mate between connectors `a` and `b`. Returns its id.
    pub fn add_joint(&mut self, a: Id, b: Id, kind: crate::feature::JointKind) -> Id {
        let id = self.alloc_id();
        // The name is a catalogue key with an argument, not "key space number".
        //
        // `format!("{} {}", kind.label(), n)` produces "joint-kind-rigid 3", which the name translator does not
        // recognise: it only parses `key#argument`. The mate list and the popup then show the raw code instead
        // of a word.
        let name = format!("name-{}-n#{}", kind.label(), self.joints.len() + 1);
        self.joints.push(crate::feature::Joint {
            id,
            name,
            a,
            b,
            kind,
            angle: 0.0,
            offset: 0.0,
            offset2: 0.0,
            drive: [None; 3], // A new mate specifies nothing: the degrees stay free until a value is given.
            flip: false,
            roll_flip: false,
            flip_decided: false,
            global: false,
            as_built: None, // A mate aligns its anchors until it is told to hold as built.
            limit_min: [None; 3],
            limit_max: [None; 3],
        });
        id
    }

    /// Declare the current placement as the one the mate holds (hold as built).
    ///
    /// It captures the relative placement of the anchors right now and bakes it into the mate. After that the
    /// mate moves nothing — it only forbids the bodies to drift apart — and the free degrees of the kind are
    /// measured from the declared placement rather than from aligned anchors.
    ///
    /// `false` means the capture failed (an anchor did not resolve), and that is not silence: the mate is left
    /// unchanged and the caller has to report that the declaration did not happen.
    pub fn set_joint_as_built(&mut self, joint: Id) -> bool {
        // A declared placement has no side at all.
        //
        // "Hold as built" bakes in the exact relative placement of the anchors, so there is nothing left to
        // choose between "face to face" and "same direction" — the answer is already complete. Choosing a side
        // anyway lays another 180 degree turn on top of the baked placement and turns the body inside out:
        // measured, an axis assembly turned by 180.000 degrees and travelled 908.204 mm together with the
        // spindle.
        //
        // So the declaration clears the side and marks it decided: neither the solve nor any later guess
        // touches it again.
        if let Some(x) = self.joints.iter_mut().find(|x| x.id == joint) {
            x.flip = false;
            x.roll_flip = false; // The side is cleared in full: a declared placement has none.
            x.flip_decided = true;
        }
        let Some(j) = self.joints.iter().find(|x| x.id == joint).cloned() else { return false };
        // Take the frames without the already captured "as built", or the declaration would compose with
        // itself.
        let mut bare = j.clone();
        bare.as_built = None;
        let Some((wa, wb)) = crate::asm::bridge::joint_frames_now_of(self, &bare) else { return false };
        // The solver applies the anchor turn after the declaration rather than before it, and the order of
        // multiplication decides the result here.
        //
        // The frame the mate holds by is assembled as `W * (f * declared) * turn`, while what is captured comes
        // from an already turned frame. Equating the two gives `declared = turn * (wa^-1 * wb) * turn`, with the
        // turn on both sides rather than one. Measured on a rigid mate between anchors facing each other:
        // without this, pressing "hold as built" moved the body by 84.1190 mm, the opposite of what the button
        // promises.
        //
        // The side was cleared above, so the frame is taken as it is, with no turn on either side.
        let rel = crate::asm::bridge::pose_to12(&(wa.inverse() * wb));
        if let Some(x) = self.joints.iter_mut().find(|x| x.id == joint) {
            x.as_built = Some(rel);
        }
        true
    }

    /// Range to sweep a degree over when animating it: from where to where.
    ///
    /// The sweep exists so the motion of a mechanism can be seen rather than inferred from numbers. The bounds
    /// come from the same place the solver takes them, the limits of the mate:
    ///
    /// * both limits set: sweep between them;
    /// * a rotation with no limits: a full turn, since an angle has a natural end and nothing needs inventing;
    /// * a travel with no limits: measured from the driven body, see `driven_body_span`.
    pub fn joint_anim_range(&self, joint: Id, slot: usize) -> Option<(f64, f64)> {
        let j = self.joints.iter().find(|x| x.id == joint)?;
        if !j.kind.free_slots().get(slot).copied().unwrap_or(false) {
            return None; // The kind pins this degree, so there is nothing to sweep.
        }
        let (_, is_rotation) = crate::asm::joint::slot_axis(crate::asm::bridge::kind_of(j.kind), slot)?;
        match (j.limit_min[slot], j.limit_max[slot]) {
            (Some(lo), Some(hi)) if hi > lo => Some((lo, hi)),
            (None, None) if is_rotation => Some((0.0, 360.0)),
            // A travel without limits is swept as well. Refusing to animate without limits leaves an
            // unbounded slider impossible to view in motion, while a rotation had a default range (a full
            // turn) all along; a travel simply has no "full turn".
            //
            // The default travel is measured rather than invented: the length of the driven body along its own
            // axis of motion. That is how far it moves when displaced by its own size, so the motion is both
            // visible and proportional to the assembly instead of coming from a hard-coded number.
            (lo, hi) if !is_rotation => {
                let span = self.driven_body_span(joint, slot)?;
                let now = [j.angle, j.offset, j.offset2][slot];
                Some((lo.unwrap_or(now), hi.unwrap_or(now + span)))
            }
            _ => None,
        }
    }

    /// Length of the driven body along its axis of motion, in millimetres: the proportional default travel.
    ///
    /// The extent comes from the body and the direction from the same degree the solver uses, or "the length
    /// of the body" and "where it moves" would refer to different axes.
    fn driven_body_span(&self, joint: Id, slot: usize) -> Option<f64> {
        let j = self.joints.iter().find(|x| x.id == joint)?;
        let owner = self.connector(j.b)?.owner;
        let dir = self.joint_slot_axis(joint, slot, self.root)?;
        let mut span: f64 = 0.0;
        for body in self.component_bodies(owner) {
            let Some(mi) = self.mesh_index(body) else { continue };
            let Some(b) = self.bodies.get(mi).and_then(|b| b.mesh.bounds()) else { continue };
            // Projection of the bounding box onto the direction: the sum of the absolute side lengths along
            // the axis.
            let d = [(b.max.x - b.min.x) * dir[0], (b.max.y - b.min.y) * dir[1], (b.max.z - b.min.z) * dir[2]];
            span = span.max(d[0].abs() + d[1].abs() + d[2].abs());
        }
        (span > 1e-6).then_some(span)
    }

    /// Which degrees are sitting at a limit, as a (mate, slot) pair for each.
    ///
    /// A limit clamps a specified value silently (`Joint::clamp_free`), which is the worst kind of obedience:
    /// a value of 40 is entered, the body stops at 20, and nothing is said. From the outside that is
    /// indistinguishable from the program ignoring the input, while in fact it obeyed and hit a limit that was
    /// set earlier.
    ///
    /// It is computed from the fact, that is, from the reading: a degree is at a limit when its current value
    /// equals the boundary. Asking the specified value instead is useless, because it is already clamped and
    /// shows no difference.
    pub fn joints_at_limit(&self) -> Vec<(Id, usize)> {
        let mut out = Vec::new();
        for j in &self.joints {
            for slot in 0..3usize {
                let now = match slot {
                    0 => j.angle,
                    1 => j.offset,
                    _ => j.offset2,
                };
                // The tolerance is the solver's; no separate number is introduced here.
                let hit = [j.limit_min[slot], j.limit_max[slot]].iter().flatten().any(|b| (now - b).abs() <= crate::asm::iterate::TOL);
                if hit {
                    out.push((j.id, slot));
                }
            }
        }
        out
    }

    /// Move a degree to its limit.
    ///
    /// The point is not convenience but checking the design: a mechanism with limits has to be viewed in its
    /// extreme positions, which is where it meets its neighbours. Typing the boundary number by hand, and
    /// exactly matching the limit field, is work the program does better.
    ///
    /// `upper` selects the upper boundary, otherwise the lower one. Returns `false` when that boundary is not
    /// set: applying a limit position where no limit exists would mean inventing one.
    pub fn apply_limit_position(&mut self, joint: Id, slot: usize, upper: bool) -> bool {
        let Some(j) = self.joints.iter_mut().find(|x| x.id == joint) else { return false };
        if slot >= 3 {
            return false;
        }
        let bound = if upper { j.limit_max[slot] } else { j.limit_min[slot] };
        let Some(v) = bound else { return false };
        j.drive[slot] = Some(v);
        true
    }

    /// Swap the roles of the two bodies in a mate.
    ///
    /// The two sides of a mate are not equal: the first anchor is the one the second is brought to, and the
    /// solver moves the body of the second. Getting the order wrong is easy (the wrong body was clicked
    /// first), and without this the only fix is deleting the mate and rebuilding it together with its
    /// specified values, limits and name.
    ///
    /// The mating side is decided again: it was chosen once as the nearer one and recorded in `flip`, and
    /// after a role swap that answer belongs to a different pair. The "hold as built" declaration is cleared
    /// too, having been captured in the frame of the first anchor, which is now the other one.
    pub fn swap_joint_roles(&mut self, joint: Id) -> bool {
        let Some(j) = self.joints.iter_mut().find(|x| x.id == joint) else { return false };
        std::mem::swap(&mut j.a, &mut j.b);
        j.flip_decided = false;
        j.as_built = None;
        true
    }

    /// Cancel a "hold as built" declaration: the mate aligns its anchors again.
    pub fn clear_joint_as_built(&mut self, joint: Id) {
        if let Some(x) = self.joints.iter_mut().find(|x| x.id == joint) {
            x.as_built = None;
        }
    }
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
        match last {
            Some(r) => {
                self.mates_conflict = !r.converged;
                // Violated mates are reported by id rather than by a single "there is a conflict" flag.
                for (jid, v) in &r.violated {
                    report.errors.push((*jid as Id, format!("joint-miss#{v:.3}")));
                    if !self.mates_violated.contains(&(*jid as Id)) {
                        self.mates_violated.push(*jid as Id);
                    }
                }
            }
            None => {}
        }
        report
    }
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
    pub(super) fn free_part_name(&self) -> String {
        let taken: std::collections::HashSet<String> = self.components.iter().map(|c| c.name.clone()).collect();
        (1..)
            .map(|n| format!("name-part-n#{n}"))
            .find(|nm| !taken.contains(nm))
            .unwrap_or_else(|| "name-part-n#1".into())
    }

    pub(super) fn body_parent(&mut self) -> Id {
        let ctx = self.active_ctx();
        if self.ctx_holds_bodies(ctx) {
            return ctx;
        }
        // An empty part is reused rather than duplicated. A new document already creates one part, and the
        // first feature would create a second, leaving an empty row in the tree with nothing behind it: two
        // parts listed where one was made.
        let free = self
            .components
            .iter()
            .find(|c| {
                c.kind == crate::feature::ComponentKind::Part && c.parent == Some(ctx) && !self.timeline.iter().any(|n| n.parent == Some(c.id))
            })
            .map(|c| c.id);
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
    pub(super) fn add_mirror_part_rigid(&mut self, part: Id, sa_src: Id, n_sa: [f64; 3], wn: [f64; 3], asm: Id) -> Id {
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
        self.push_timeline(FeatureNode { id: body, name: "name-mirror-part".into(), kind: FeatureKind::MirrorPart { src_comp: part, ln, body }, parent: Some(new_part), dirty: true, suppressed: false });
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
        let (src_name, src_parent) = self
            .components
            .iter()
            .find(|c| c.id == src_comp)
            .map(|c| (c.name.clone(), c.parent))
            .unwrap_or(("name-part".into(), None));
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

// --- Geometry of mate anchors ---
//
// A mate connector is a full coordinate system on a body, and resolving it from a geometry reference (a face,
// an edge, a vertex, a base plane) is the assembly's work rather than the general core's. This section is
// closed by call: the outside needs `connector_matrix` and `connector_frame`, and the rest is their
// machinery.

impl Project {
    /// Extent of a cylindrical face along its axis: where its ends are.
    ///
    /// Needed so an anchor can be placed at an end of a hole rather than only at its midpoint. Returns the
    /// coordinates of both ends along the axis, measured from point `axis_pt`.
    pub fn cyl_face_span(&self, body: Id, key: &crate::feature::FaceKey, axis_pt: [f64; 3], axis: [f64; 3]) -> Option<(f64, f64)> {
        let faces = self.regen_faces.get(&body)?;
        let f = faces.iter().find(|f| f.id == key.id)?;
        let mesh = self.mesh_index(body).map(|i| &self.bodies[i].mesh)?;
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for &ti in &f.triangles {
            let Some(t) = mesh.tris.get(ti as usize) else { continue };
            for &vi in t {
                let Some(v) = mesh.verts.get(vi as usize) else { continue };
                let d = (v.x - axis_pt[0]) * axis[0] + (v.y - axis_pt[1]) * axis[1] + (v.z - axis_pt[2]) * axis[2];
                lo = lo.min(d);
                hi = hi.max(d);
            }
        }
        (lo <= hi).then_some((lo, hi))
    }

    /// Principal direction of a face: the X axis of its anchor, derived from the body itself.
    ///
    /// Building the anchor frame as "Z is the normal, X is world Z cross the normal" makes the X axis depend on
    /// a world axis rather than on the shape of the body. A mate aligns whole frames, rotation about the normal
    /// included, so that rotation comes out arbitrary relative to the geometry: the normals meet correctly
    /// while the body settles at a random roll.
    ///
    /// An anchor has to be an object whose axes come from the geometry it stands on. Here X is the principal
    /// direction of the face: the direction of greatest spread of its vertices within the face plane, which for
    /// a rectangular patch is its long side. It is deterministic, independent of world axes, and computed the
    /// same way on both bodies.
    pub fn face_principal_dir(&self, body: Id, key: &crate::feature::FaceKey) -> Option<[f64; 3]> {
        let faces = self.regen_faces.get(&body)?;
        let f = faces.iter().find(|f| f.id == key.id)?;
        let mesh = self.mesh_index(body).map(|i| &self.bodies[i].mesh)?;
        let n = {
            let l = (f.normal[0].powi(2) + f.normal[1].powi(2) + f.normal[2].powi(2)).sqrt();
            if l < 1e-12 { return None }
            [f.normal[0] / l, f.normal[1] / l, f.normal[2] / l]
        };
        // Face vertices, each counted once, projected into the face plane.
        //
        // Counting triangle vertices in sequence is wrong: on a rectangular face made of two triangles the
        // endpoints of the shared diagonal are counted twice and the spread skews towards that diagonal.
        // Measured on a 40 by 20 side face: instead of the long side (1,0,0) the result was (0.978, 0, -0.208),
        // a direction between the side and the diagonal. The travel axis of a slider comes from here, so the
        // body would move at an angle.
        let mut seen: std::collections::HashSet<u32> = std::collections::HashSet::new();
        let mut pts: Vec<[f64; 3]> = Vec::new();
        for &ti in &f.triangles {
            let Some(t) = mesh.tris.get(ti as usize) else { continue };
            for &vi in t {
                if !seen.insert(vi) {
                    continue;
                }
                if let Some(v) = mesh.verts.get(vi as usize) {
                    let d = [v.x - f.centroid.x, v.y - f.centroid.y, v.z - f.centroid.z];
                    let along = d[0] * n[0] + d[1] * n[1] + d[2] * n[2];
                    pts.push([d[0] - n[0] * along, d[1] - n[1] * along, d[2] - n[2] * along]);
                }
            }
        }
        if pts.len() < 3 {
            return None;
        }
        // Direction of greatest spread: power iteration on the covariance matrix.
        let mut c = [[0.0f64; 3]; 3];
        for p in &pts {
            for i in 0..3 {
                for j in 0..3 {
                    c[i][j] += p[i] * p[j];
                }
            }
        }
        // The iteration starts not from world X but from the axis least aligned with the normal.
        //
        // With world X as the starting vector, a face whose normal points along X has all its points in the YZ
        // plane: the very first step gives zero and the function honestly answers "unknown". Every face turned
        // towards world X — about a third of them in any assembly — then has no principal direction at all,
        // which is why a slider moves in the wrong direction whatever is selected: without a principal
        // direction the travel axis comes from world axes.
        let least = (0..3).min_by(|&i, &j| n[i].abs().total_cmp(&n[j].abs())).unwrap_or(0);
        let mut v = {
            let mut e = [0.0; 3];
            e[least] = 1.0;
            let along = e[0] * n[0] + e[1] * n[1] + e[2] * n[2];
            let w = [e[0] - n[0] * along, e[1] - n[1] * along, e[2] - n[2] * along];
            let l = (w[0] * w[0] + w[1] * w[1] + w[2] * w[2]).sqrt();
            if l < 1e-12 {
                return None;
            }
            [w[0] / l, w[1] / l, w[2] / l]
        };
        for _ in 0..64 {
            let w = [
                c[0][0] * v[0] + c[0][1] * v[1] + c[0][2] * v[2],
                c[1][0] * v[0] + c[1][1] * v[1] + c[1][2] * v[2],
                c[2][0] * v[0] + c[2][1] * v[1] + c[2][2] * v[2],
            ];
            // Keep the direction within the face plane.
            let along = w[0] * n[0] + w[1] * n[1] + w[2] * n[2];
            let w = [w[0] - n[0] * along, w[1] - n[1] * along, w[2] - n[2] * along];
            let l = (w[0] * w[0] + w[1] * w[1] + w[2] * w[2]).sqrt();
            if l < 1e-12 {
                return None;
            }
            v = [w[0] / l, w[1] / l, w[2] / l];
        }
        // The sign is made deterministic: the spread is symmetric, so it is fixed by the largest coordinate,
        // or the same face would yield X on one run and -X on the next.
        let k = (0..3).max_by(|&i, &j| v[i].abs().total_cmp(&v[j].abs())).unwrap_or(0);
        if v[k] < 0.0 {
            v = [-v[0], -v[1], -v[2]];
        }
        Some(v)
    }

    /// Axis of a cylindrical face (a hole, a shaft); `None` for a planar face.
    ///
    /// A face anchor that builds its frame from the normal fails on a cylinder, where the normal depends on the
    /// point of the surface and cancels to zero around the circle, leaving the rotation axis arbitrary and
    /// turning the loop inside out. The correct pattern is next door: an anchor on a circular edge takes the
    /// axis of the circle.
    ///
    /// How it is computed: on a cylinder every facet normal is perpendicular to the axis, so the axis is the
    /// direction orthogonal to all of them — the eigenvector of the matrix sum of n * n^T with the smallest
    /// eigenvalue. A planar face differs in that the spread of its normals is near zero, in which case there is
    /// no axis.
    pub fn face_axis(&self, body: Id, key: &crate::feature::FaceKey) -> Option<([f64; 3], [f64; 3])> {
        let faces = self.regen_faces.get(&body)?;
        let f = faces.iter().find(|f| f.id == key.id)?;
        let mesh = self.mesh_index(body).map(|i| &self.bodies[i].mesh)?;
        // Facet normals of the face.
        let mut ns: Vec<[f64; 3]> = Vec::new();
        for &ti in &f.triangles {
            let Some(t) = mesh.tris.get(ti as usize) else { continue };
            let (p0, p1, p2) = (mesh.verts.get(t[0] as usize)?, mesh.verts.get(t[1] as usize)?, mesh.verts.get(t[2] as usize)?);
            let u = [p1.x - p0.x, p1.y - p0.y, p1.z - p0.z];
            let v = [p2.x - p0.x, p2.y - p0.y, p2.z - p0.z];
            let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
            let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            if l > 1e-12 {
                ns.push([n[0] / l, n[1] / l, n[2] / l]);
            }
        }
        if ns.len() < 3 {
            return None;
        }
        // Σ nnᵀ
        let mut a = [[0.0f64; 3]; 3];
        for n in &ns {
            for i in 0..3 {
                for j in 0..3 {
                    a[i][j] += n[i] * n[j];
                }
            }
        }
        // The eigenvector of the smallest eigenvalue, by inverse iteration from three starting axes.
        let cnt = ns.len() as f64;
        let mut best: Option<([f64; 3], f64)> = None;
        for start in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]] {
            let mut v = start;
            for _ in 0..64 {
                // v <- (cnt*I - A)*v: shifting the spectrum makes the former smallest eigenvalue the largest.
                let w = [
                    cnt * v[0] - (a[0][0] * v[0] + a[0][1] * v[1] + a[0][2] * v[2]),
                    cnt * v[1] - (a[1][0] * v[0] + a[1][1] * v[1] + a[1][2] * v[2]),
                    cnt * v[2] - (a[2][0] * v[0] + a[2][1] * v[1] + a[2][2] * v[2]),
                ];
                let l = (w[0] * w[0] + w[1] * w[1] + w[2] * w[2]).sqrt();
                if l < 1e-12 {
                    break;
                }
                v = [w[0] / l, w[1] / l, w[2] / l];
            }
            // Spread of the normals along v: near zero along the axis of a cylinder.
            let q = v[0] * (a[0][0] * v[0] + a[0][1] * v[1] + a[0][2] * v[2])
                + v[1] * (a[1][0] * v[0] + a[1][1] * v[1] + a[1][2] * v[2])
                + v[2] * (a[2][0] * v[0] + a[2][1] * v[1] + a[2][2] * v[2]);
            if best.as_ref().is_none_or(|(_, bq)| q < *bq) {
                best = Some((v, q));
            }
        }
        let (axis, q) = best?;
        // On a planar face every normal is the same, so there are two orthogonal directions and the choice
        // between them is arbitrary. A cylinder is recognised by its normals being noticeably spread: the
        // summed normal is much shorter than the facet count, whereas for a plane the two are equal.
        let sum = ns.iter().fold([0.0f64; 3], |s, n| [s[0] + n[0], s[1] + n[1], s[2] + n[2]]);
        let sum_len = (sum[0] * sum[0] + sum[1] * sum[1] + sum[2] * sum[2]).sqrt();
        if sum_len > 0.9 * cnt || q > 1e-6 * cnt {
            return None; // A planar face (or anything but a cylinder) has no axis.
        }
        Some(([f.centroid.x, f.centroid.y, f.centroid.z], axis))
    }

    /// Axis anchor of an edge by id: for a circular edge (the rim of a hole or a cylinder) the centre of the
    /// circle and its axis; for a straight edge the midpoint and the tangent.
    ///
    /// One source for both an axis connector and a datum axis built from an edge, so that a concentric pick on
    /// a hole gives the axis through its centre rather than a point on the rim.
    pub fn resolve_edge_axis(&self, body: Id, edge_id: u32) -> Option<([f64; 3], [f64; 3])> {
        self.regen_edges.get(&body)?.iter().find(|e| e.id == edge_id).map(|e| e.axis_ref())
    }

    /// The direction a piece of geometry defines, for the optional second pick of a connector's secondary
    /// axis.
    ///
    /// An edge gives its own direction (for a circular edge, the axis of the circle) and a face gives its
    /// normal (for a cylindrical face, its axis). A vertex and the body origin define no direction: expecting a
    /// side from a point would mean inventing the answer, so this honestly returns nothing.
    pub fn anchor_direction(&self, r: &crate::feature::AnchorRef) -> Option<[f64; 3]> {
        use crate::feature::AnchorRef;
        match r {
            AnchorRef::EdgeMid(b, e) => self.resolve_edge_axis(*b, *e).map(|(_, d)| d),
            AnchorRef::FaceCenter(b, k) => match self.face_axis(*b, k) {
                Some((_, ax)) => Some(ax),
                None => Some(self.resolve_face(*b, k).1),
            },
            AnchorRef::BasePlane(p) => Some(p.frame().normal()),
            AnchorRef::Origin | AnchorRef::Vertex(..) => None,
        }
    }

    /// Reference direction of an edge: the normal of its adjacent face at its midpoint, in body local space.
    ///
    /// This is the secondary axis of a connector: an edge has one axis of its own, along itself, and without a
    /// second axis the roll of the frame is undefined. A zero vector in `ref_dir` (a dangling edge, or an
    /// analysis without adjacent faces) honestly means "none", and the caller then falls back to deriving it
    /// from a world axis.
    pub fn edge_ref_dir(&self, body: Id, edge_id: u32) -> Option<[f64; 3]> {
        let e = self.regen_edges.get(&body)?.iter().find(|e| e.id == edge_id)?;
        let n = e.ref_dir;
        (n[0] * n[0] + n[1] * n[1] + n[2] * n[2] > 1e-12).then_some(n)
    }

    /// Resolve a vertex, that is, an endpoint of an edge of body `body` by edge id (`at_end` selects the end
    /// or the start), into a point in body local space.
    pub fn resolve_vertex(&self, body: Id, edge_id: u32, at_end: bool) -> Option<[f64; 3]> {
        self.regen_edges.get(&body)?.iter().find(|e| e.id == edge_id).map(|e| if at_end { e.b } else { e.a })
    }

    /// The component in whose local space the anchor geometry of a connector lives.
    ///
    /// For `Origin` and `BasePlane` the geometry is defined in the space of the placement owner itself, that
    /// is, `conn.owner`. For faces, edges and vertices it is the owner of body `body` (a leaf part), even when
    /// `conn.owner` is a subassembly above it.
    pub(super) fn connector_geom_owner(&self, conn: &crate::feature::MateConnector) -> Option<Id> {
        use crate::feature::AnchorRef;
        match &conn.anchor {
            AnchorRef::Origin | AnchorRef::BasePlane(_) => Some(conn.owner),
            AnchorRef::FaceCenter(body, _) | AnchorRef::EdgeMid(body, _) | AnchorRef::Vertex(body, _, _) => self.body_owner(*body),
        }
    }

    /// Frame of a connector in the local space of its placement owner (`conn.owner`), with the flip and the
    /// offsets applied.
    ///
    /// When `owner` is a subassembly (at the level of the joint context) while the geometry resolves in a leaf
    /// body, the frame is built in leaf space and then carried into the space of `owner` through
    /// `relative_transform`.
    pub fn connector_frame(&self, conn: &crate::feature::MateConnector) -> Option<crate::feature::PlaneFrame> {
        use crate::feature::{AnchorRef, PlaneFrame};
        // An anchor on a deleted body does not resolve.
        //
        // Otherwise `resolve_face` returns the old centroid fingerprint stored in the key itself, the solver
        // gets a garbage frame and scatters bodies to wild coordinates — which is what used to justify deleting
        // the mate outright. The document's memory is queried rather than the timeline: "not in the timeline"
        // covers both "deleted" and "not built yet", and those are different things (see
        // `Project::dead_bodies`).
        if !self.dead_bodies.is_empty() {
            if let AnchorRef::FaceCenter(b, _) | AnchorRef::EdgeMid(b, _) | AnchorRef::Vertex(b, _, _) = &conn.anchor {
                if self.dead_bodies.contains(b) {
                    return None;
                }
            }
        }
        let mut fr = match &conn.anchor {
            AnchorRef::Origin => PlaneFrame { origin: [0.0; 3], x: [1.0, 0.0, 0.0], y: [0.0, 1.0, 0.0] },
            AnchorRef::BasePlane(b) => b.frame(),
            AnchorRef::FaceCenter(body, key) => {
                // A cylindrical face (a hole or a shaft) gives an axis rather than a normal. The normal of a
                // cylinder depends on the point and cancels to zero around the circle, so a revolute mate on a
                // hole would spin the body about a random direction. A planar face keeps centre plus normal.
                match self.face_axis(*body, key) {
                    Some((o, ax)) => {
                        // Attachment point on a cylinder: the midpoint or an end. This choice is the point of
                        // the feature — holes of different lengths do not meet at their midpoints, while their
                        // ends meet exactly.
                        let o = match self.cyl_face_span(*body, key, o, ax) {
                            Some((lo, hi)) => {
                                let d = conn.point.along(lo, hi);
                                [o[0] + ax[0] * d, o[1] + ax[1] * d, o[2] + ax[2] * d]
                            }
                            None => o,
                        };
                        PlaneFrame::from_origin_normal(o, ax, 0.0)
                    }
                    None => {
                        let (c, n) = self.resolve_face(*body, key);
                        // The secondary axis comes from the body rather than from world axes.
                        // `from_origin_normal` derives X as world Z cross the normal, so two mating faces get
                        // inconsistent secondary axes and pinning the roll turns the body by a random angle.
                        // That stayed invisible while nothing held the roll.
                        match self.face_principal_dir(*body, key) {
                            Some(x) => PlaneFrame::from_origin_axes(c, n, x),
                            None => PlaneFrame::from_origin_normal(c, n, 0.0),
                        }
                    }
                }
            }
            AnchorRef::EdgeMid(body, eid) => {
                // Axis anchor: a circular edge (the rim of a hole or a cylinder) gives the centre plus the axis
                // of the circle, a concentric anchor like a face axis; a straight edge gives the midpoint plus
                // the tangent along it.
                let (o, dir) = self.resolve_edge_axis(*body, *eid)?;
                // The secondary axis comes from the adjacent face rather than from world Z. Otherwise the roll
                // of the frame depends on how the body happens to lie in the world, the two mating bodies derive
                // it differently, and the mate settles the body at an arbitrary rotation.
                match self.edge_ref_dir(*body, *eid) {
                    Some(x) => PlaneFrame::from_origin_axes(o, dir, x),
                    None => PlaneFrame::from_origin_normal(o, dir, 0.0),
                }
            }
            AnchorRef::Vertex(body, eid, end) => {
                // Point anchor: the origin is the vertex itself, not a projection.
                //
                // The axes come from the edge the vertex belongs to rather than from the world. World axes mean
                // "however the body happens to lie", and a rigid mate on two vertices then turns the body in an
                // arbitrary direction. The main axis points away from the vertex along the edge (opposite
                // directions at the start and at the end, so each corner keeps its own frame) and the secondary
                // axis is the normal of the adjacent face.
                let p = self.resolve_vertex(*body, *eid, *end)?;
                let (_, dir) = self.regen_edges.get(body)?.iter().find(|e| e.id == *eid).map(|e| (e.mid, e.dir))?;
                let z = if *end { [-dir[0], -dir[1], -dir[2]] } else { dir };
                match self.edge_ref_dir(*body, *eid) {
                    Some(x) => PlaneFrame::from_origin_axes(p, z, x),
                    None => PlaneFrame::from_origin_normal(p, z, 0.0),
                }
            }
        };
        // An explicitly picked secondary axis outranks a derived one. A square face has no long side at all
        // and the automatic answer is arbitrary; pointing at an edge then puts the axis along it. It is placed
        // perpendicular to the main axis: the pick chooses a side, it does not replace the anchor.
        if let Some(dir) = conn.axis_ref.as_ref().and_then(|r| self.anchor_direction(r)) {
            fr = PlaneFrame::from_origin_axes(fr.origin, fr.normal(), dir);
        }
        if conn.flip {
            // A 180 degree turn about X flips Y, and with it the normal (X cross Y).
            fr.y = [-fr.y[0], -fr.y[1], -fr.y[2]];
        }
        // Rotation of the secondary axis by an arbitrary angle: the reorientation control. The axis is derived
        // from geometry, but which way it should point is the author's decision.
        if conn.rot_deg != 0.0 {
            let n = fr.normal();
            let ang = conn.rot_deg.to_radians();
            let (s, c) = ang.sin_cos();
            let rot = |v: [f64; 3]| {
                let d = v[0] * n[0] + v[1] * n[1] + v[2] * n[2];
                let cr = [n[1] * v[2] - n[2] * v[1], n[2] * v[0] - n[0] * v[2], n[0] * v[1] - n[1] * v[0]];
                [v[0] * c + cr[0] * s + n[0] * d * (1.0 - c), v[1] * c + cr[1] * s + n[1] * d * (1.0 - c), v[2] * c + cr[2] * s + n[2] * d * (1.0 - c)]
            };
            fr.x = rot(fr.x);
            fr.y = rot(fr.y);
        }
        // Offsets along all three connector axes. With only the main one available there is no way to move an
        // anchor sideways, and the body gets moved by guesswork instead.
        let d = conn.offset_xyz;
        if d != [0.0; 3] {
            let n = fr.normal();
            for k in 0..3 {
                fr.origin[k] += fr.x[k] * d[0] + fr.y[k] * d[1] + n[k] * d[2];
            }
        }
        // The geometry resolves in leaf space; when the placement owner is a subassembly above it, the frame
        // (with the flip and offsets already applied in leaf space) is carried into owner space.
        if let Some(leaf) = self.connector_geom_owner(conn) {
            if leaf != conn.owner {
                fr = fr.transformed(&self.relative_transform(leaf, conn.owner));
            }
        }
        Some(fr)
    }

    /// Connector frame built for a joint kind: for a slider on a flat face the travel axis lies in the plane.
    ///
    /// The travel axis is the Z of the frame, and for a planar face Z is the normal, so the body moves away
    /// from the face although the face was picked precisely as a guide. For a slider the frame is therefore
    /// turned: Z becomes the principal direction of the face (the long side of a rail) and the normal moves
    /// into X. The planes stay in contact while the motion runs along them.
    ///
    /// This does not apply to a cylindrical face: there Z is the axis of the cylinder and sliding along the
    /// axis is exactly what is wanted (a shaft in a hole). Turning that frame would break a working case.
    pub fn connector_frame_for_kind(&self, conn: &crate::feature::MateConnector, kind: crate::feature::JointKind) -> Option<crate::feature::PlaneFrame> {
        use crate::feature::{AnchorRef, JointKind, PlaneFrame};
        let fr = self.connector_frame(conn)?;
        if !matches!(kind, JointKind::Slider) {
            return Some(fr);
        }
        // An explicitly picked axis is the travel axis, whatever the anchor is.
        //
        // Handling this for face anchors only (below) leaves an edge anchor or an origin anchor changing only
        // the roll while the travel stays as it was. For an origin anchor the frame is the identity and its
        // normal is world Z, so the slider moves up and down no matter what is picked.
        //
        // The pick goes into the X axis of the frame (see `connector_frame`) while a slider travels along the
        // normal, so the frame has to be rebuilt with the picked direction as its normal.
        if let Some(raw) = conn.axis_ref.as_ref().and_then(|r| self.anchor_direction(r)) {
            // The direction is taken from the pick itself rather than from the frame. In the frame it becomes
            // the secondary axis, that is, projected across the main one; for an edge running along the main
            // axis that projection degenerates to zero and the frame honestly answers with an arbitrary
            // rotation. Measured: `pointing_at_an_edge_sets_the_anchor_axis_from_the_frame` went red on such an
            // edge.
            let dir = if let Some(leaf) = self.connector_geom_owner(conn) {
                if leaf != conn.owner {
                    crate::feature::apply12_dir(&self.relative_transform(leaf, conn.owner), raw)
                } else {
                    raw
                }
            } else {
                raw
            };
            return Some(PlaneFrame::from_origin_axes(fr.origin, dir, fr.normal()));
        }
        let AnchorRef::FaceCenter(body, key) = &conn.anchor else { return Some(fr) };
        if self.face_axis(*body, key).is_some() {
            return Some(fr); // A cylinder already has the right axis.
        }
        // The principal direction of the face is what the body slides along: on a rail, its long side.
        //
        // Without one, any direction in the plane is used. A face may be square, or its geometry may not be
        // raised yet, but even an arbitrary in-plane axis beats the normal: along the face the body at least
        // slides, whereas along the normal it separates from it. Answering "unknown" here drops the mate from
        // the problem silently and the body does not move at all.
        //
        // A picked axis outranks a derived one: when an edge was picked, `connector_frame` has already put its
        // direction into the X axis of the frame, and guessing the long side of the face on top of that is
        // wrong.
        let dir = self.face_principal_dir(*body, key).unwrap_or(fr.x); // The picked axis was handled above.
        let n = fr.normal();
        Some(PlaneFrame::from_origin_axes(fr.origin, dir, n))
    }

    /// Bodies grounded inside a moving subassembly, where grounding tells a lie.
    ///
    /// Grounded reads as "stands still and will not move". But a body lives inside a subassembly, and the
    /// subassembly is driven by its own mate: when it moves, the grounded body moves with it without shifting a
    /// millimetre relative to its neighbours. The word has been said and means nothing.
    ///
    /// Measured on a machine document: a grounded part inside a beam subassembly travelled 100.000 mm with the
    /// beam once the gantry was given a travel value. Three such parts existed in that document and nothing
    /// reported them.
    ///
    /// This is a name rather than a prohibition. The arrangement is legitimate — grounding a body inside a
    /// subassembly is ordinary when the subassembly is built on its own — but it has to be known that the body
    /// is not fixed in the world.
    pub fn grounded_inside_moving(&self) -> Vec<Id> {
        self.components
            .iter()
            .filter(|c| c.grounded)
            .filter(|c| {
                // Walk up from the parent: if any ancestor is driven by a mate, the body moves.
                let mut cur = c.parent;
                while let Some(x) = cur {
                    if x == self.root {
                        return false;
                    }
                    if self.drive_joint_for(x).is_some() {
                        return true;
                    }
                    cur = self.components.iter().find(|k| k.id == x).and_then(|k| k.parent);
                }
                false
            })
            .map(|c| c.id)
            .collect()
    }

    /// Whether an anchor stands on a body that moves inside its own declared owner.
    ///
    /// An anchor is declared on a component while taking its geometry from a body. Usually that body lies
    /// inside the same component and stands still within it, which is honest. But when a moving part appears on
    /// the path from the geometry to the declared owner, the anchor moves whenever that part moves: the mate
    /// holds on to something that does not stand still itself.
    ///
    /// One answer to two questions: why a mate is faulty (`joint_faults`) and whether an anchor may be placed
    /// here (the tool, before creation). Two separate checks of the same thing would drift apart silently.
    pub fn anchor_sits_on_moving_part(&self, owner: Id, anchor: &crate::feature::AnchorRef) -> bool {
        use crate::feature::AnchorRef;
        let body = match anchor {
            AnchorRef::FaceCenter(b, _) | AnchorRef::EdgeMid(b, _) | AnchorRef::Vertex(b, _, _) => *b,
            AnchorRef::Origin | AnchorRef::BasePlane(_) => return false,
        };
        let Some(leaf) = self.body_owner(body) else { return false };
        let mut cur = Some(leaf);
        while let Some(x) = cur {
            if x == owner {
                return false; // Reached the declared owner without meeting anything movable on the way.
            }
            if self.drive_joint_for(x).is_some() {
                return true;
            }
            cur = self.components.iter().find(|k| k.id == x).and_then(|k| k.parent);
        }
        false
    }

    /// Mates whose anchor did not resolve, so the interface can report them.
    ///
    /// Measured on a real document: none of its five edge-based mates had a connector frame, because the bodies
    /// were imported, the live B-rep is not raised without being asked, and an edge axis is read from it. The
    /// bridge dropped such mates from the problem silently (`else { continue }`): the assembly looks assembled,
    /// nothing moves, and no explanation is given.
    ///
    /// Silence is the worst outcome, because a broken mate then looks the same as a mishandled drag. Computed
    /// on the spot, without state: the answer depends only on whether the geometry is raised right now.
    pub fn unresolved_joints(&self) -> Vec<Id> {
        self.joint_faults().into_iter().map(|(id, _)| id).collect()
    }

    /// Build a group from components: they are fixed to each other where they stand.
    ///
    /// A group moves nothing when it is created; it only forbids the bodies to drift apart from then on. It
    /// grounds nothing either: one of the bodies still has to be attached to something, or the set is free as
    /// a whole.
    pub fn add_group(&mut self, members: &[Id]) -> Id {
        // The name is a catalogue key with an argument, as for mates: the core knows no interface language,
        // and a literal name in the code would make translation endless (the `i18n::ratchet` guard catches
        // exactly that).
        self.add_mate_constraint(crate::feature::ConstraintKind::Group, members.to_vec(), Vec::new(), "name-group-n")
    }

    /// Width: the tab stands midway between the two walls.
    ///
    /// It relates the bodies symmetrically and leaves two degrees of freedom — translation within the
    /// mid-plane and rotation about it. It holds exactly one thing: the distances to the two walls are equal.
    /// Where the body sits along the slot is the business of other mates.
    pub fn add_width(&mut self, walls: &[Id; 2], tab: Id) -> Id {
        self.add_mate_constraint(crate::feature::ConstraintKind::Width, Vec::new(), vec![walls[0], walls[1], tab], "name-width-n")
    }

    /// Tangency between two surfaces. It has no connectors at all.
    ///
    /// The supported pair is cylinder against plane, where the shaft lies on the plane. Geometrically that is
    /// two conditions at once: the axis is parallel to the plane (otherwise the cylinder cuts through it) and
    /// the axis-to-plane distance equals the radius.
    pub fn add_tangent(&mut self, owner_a: Id, a: crate::feature::AnchorRef, owner_b: Id, b: crate::feature::AnchorRef) -> Id {
        let id = self.alloc_id();
        let kind = crate::feature::ConstraintKind::Tangent;
        let n = self.mate_constraints.iter().filter(|c| c.kind == kind).count() + 1;
        self.mate_constraints.push(crate::feature::MateConstraint {
            id,
            name: format!("name-tangent-n#{n}"),
            kind,
            members: Vec::new(),
            anchors: Vec::new(),
            faces: vec![(owner_a, a), (owner_b, b)],
        });
        id
    }

    /// A cylindrical face in full: a point on the axis, the axis, and the radius.
    ///
    /// `face_axis` supplies no radius, while the radius is what tangency is about — the distance to the plane
    /// equals the radius. It is computed from the mesh as the mean distance of the face vertices from the
    /// axis; derived geometry offers no other source, and inventing one is not an option.
    pub fn face_cylinder(&self, body: Id, key: &crate::feature::FaceKey) -> Option<([f64; 3], [f64; 3], f64)> {
        let (o, ax) = self.face_axis(body, key)?;
        let faces = self.regen_faces.get(&body)?;
        let f = faces.iter().find(|f| f.id == key.id)?;
        let mesh = self.mesh_index(body).map(|i| &self.bodies[i].mesh)?;
        let mut sum = 0.0;
        let mut cnt = 0usize;
        let mut seen: std::collections::HashSet<u32> = std::collections::HashSet::new();
        for &ti in &f.triangles {
            let Some(t) = mesh.tris.get(ti as usize) else { continue };
            for &vi in t {
                if !seen.insert(vi) {
                    continue;
                }
                let Some(v) = mesh.verts.get(vi as usize) else { continue };
                let d = [v.x - o[0], v.y - o[1], v.z - o[2]];
                let along = d[0] * ax[0] + d[1] * ax[1] + d[2] * ax[2];
                let perp = [d[0] - ax[0] * along, d[1] - ax[1] * along, d[2] - ax[2] * along];
                sum += (perp[0] * perp[0] + perp[1] * perp[1] + perp[2] * perp[2]).sqrt();
                cnt += 1;
            }
        }
        (cnt > 0).then(|| (o, ax, sum / cnt as f64))
    }

    /// A spherical face in full: centre and radius.
    ///
    /// Found by least squares over the face vertices, but not as a sphere fitted to anything at all: two
    /// guards are required, or a plane and a cylinder would pass as spheres too.
    ///
    /// * The fit has to be tight: every vertex lies at exactly the radius from the centre, within 2 per cent.
    ///   The vertices of a cylinder spread along its axis and fail this check.
    /// * The face has to be commensurate with the radius. A flat patch is approximated beautifully by a huge
    ///   sphere — the fit is tight and there is no sphere at all — so the face is required to span at least a
    ///   tenth of the radius; anything smaller is a plane, and tangency there is computed differently.
    ///
    /// The equation is linear: `2c.v - k = |v|^2`, where `k = |c|^2 - r^2`. Fitting `|v - c| = r` directly is
    /// non-linear, while this form has the same four unknowns and is solved in one pass.
    pub fn face_sphere(&self, body: Id, key: &crate::feature::FaceKey) -> Option<([f64; 3], f64)> {
        let faces = self.regen_faces.get(&body)?;
        let f = faces.iter().find(|f| f.id == key.id)?;
        let mesh = self.mesh_index(body).map(|i| &self.bodies[i].mesh)?;
        let mut pts: Vec<[f64; 3]> = Vec::new();
        let mut seen: std::collections::HashSet<u32> = std::collections::HashSet::new();
        for &ti in &f.triangles {
            let Some(t) = mesh.tris.get(ti as usize) else { continue };
            for &vi in t {
                if seen.insert(vi) {
                    if let Some(v) = mesh.verts.get(vi as usize) {
                        pts.push([v.x as f64, v.y as f64, v.z as f64]);
                    }
                }
            }
        }
        if pts.len() < 12 {
            return None; // Too few vertices to tell a sphere from anything else.
        }
        // Normal equations, 4 by 4, for the unknowns (cx, cy, cz, k).
        let mut m = [[0.0f64; 5]; 4];
        for p in &pts {
            let row = [2.0 * p[0], 2.0 * p[1], 2.0 * p[2], -1.0];
            let rhs = p[0] * p[0] + p[1] * p[1] + p[2] * p[2];
            for i in 0..4 {
                for j in 0..4 {
                    m[i][j] += row[i] * row[j];
                }
                m[i][4] += row[i] * rhs;
            }
        }
        // Gaussian elimination with partial pivoting.
        for col in 0..4 {
            let piv = (col..4).max_by(|&x, &y| m[x][col].abs().partial_cmp(&m[y][col].abs()).unwrap_or(std::cmp::Ordering::Equal))?;
            if m[piv][col].abs() < 1e-12 {
                return None; // The system is degenerate: there is no sphere here.
            }
            m.swap(col, piv);
            let d = m[col][col];
            for j in col..5 {
                m[col][j] /= d;
            }
            for i in 0..4 {
                if i != col && m[i][col].abs() > 0.0 {
                    let k = m[i][col];
                    for j in col..5 {
                        m[i][j] -= k * m[col][j];
                    }
                }
            }
        }
        let c = [m[0][4], m[1][4], m[2][4]];
        let r2 = c[0] * c[0] + c[1] * c[1] + c[2] * c[2] - m[3][4];
        if r2 <= 1e-12 {
            return None;
        }
        let r = r2.sqrt();
        let dist = |p: &[f64; 3]| ((p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2) + (p[2] - c[2]).powi(2)).sqrt();
        let worst = pts.iter().map(|p| (dist(p) - r).abs()).fold(0.0f64, f64::max);
        if worst > 0.02 * r {
            return None; // The fit is loose: not a sphere.
        }
        let span = pts.iter().map(|p| ((p[0] - f.centroid.x as f64).powi(2) + (p[1] - f.centroid.y as f64).powi(2) + (p[2] - f.centroid.z as f64).powi(2)).sqrt()).fold(0.0f64, f64::max);
        if span < 0.1 * r {
            return None; // The face is far smaller than the fitted radius: a plane, not a piece of a sphere.
        }
        // The normal of a sphere points away from its centre. That is the definition, and without this check a
        // cylinder passes as a sphere: its side surface is triangulated without interior vertices, so every
        // vertex sits on one of the two rims, and two rims lie on one sphere. Measured on a shaft of diameter
        // 10 and length 40: the fit was perfect, the radius came out as 20.616 = sqrt(5^2 + 20^2), and tangency
        // against a plate was computed against a sphere that does not exist. A tight vertex fit is necessary
        // but not sufficient.
        for &ti in &f.triangles {
            let Some(t) = mesh.tris.get(ti as usize) else { continue };
            let (Some(p0), Some(p1), Some(p2)) = (mesh.verts.get(t[0] as usize), mesh.verts.get(t[1] as usize), mesh.verts.get(t[2] as usize)) else { continue };
            let u = [(p1.x - p0.x) as f64, (p1.y - p0.y) as f64, (p1.z - p0.z) as f64];
            let w = [(p2.x - p0.x) as f64, (p2.y - p0.y) as f64, (p2.z - p0.z) as f64];
            let n = [u[1] * w[2] - u[2] * w[1], u[2] * w[0] - u[0] * w[2], u[0] * w[1] - u[1] * w[0]];
            let nl = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            if nl < 1e-12 {
                continue;
            }
            let mid = [((p0.x + p1.x + p2.x) as f64) / 3.0 - c[0], ((p0.y + p1.y + p2.y) as f64) / 3.0 - c[1], ((p0.z + p1.z + p2.z) as f64) / 3.0 - c[2]];
            let ml = (mid[0] * mid[0] + mid[1] * mid[1] + mid[2] * mid[2]).sqrt();
            if ml < 1e-12 {
                return None;
            }
            let cos = (n[0] * mid[0] + n[1] * mid[1] + n[2] * mid[2]).abs() / (nl * ml);
            if cos < 0.9 {
                return None; // The normal does not point away from the centre: the surface is not spherical.
            }
        }
        Some((c, r))
    }

    fn add_mate_constraint(&mut self, kind: crate::feature::ConstraintKind, members: Vec<Id>, anchors: Vec<Id>, name_key: &str) -> Id {
        let id = self.alloc_id();
        let n = self.mate_constraints.iter().filter(|c| c.kind == kind).count() + 1;
        self.mate_constraints.push(crate::feature::MateConstraint { id, name: format!("{name_key}#{n}"), kind, members, anchors, faces: Vec::new() });
        id
    }

    /// Delete an assembly constraint (a group, a width) by id.
    pub fn delete_group(&mut self, id: Id) {
        self.mate_constraints.retain(|g| g.id != id);
    }

    /// A relation between two degrees of freedom.
    ///
    /// The slots are not chosen here: for kinds with mixed degrees (rack, screw) the rotation has to come first
    /// — see `RelationKind::slots_are_rotations`. A mismatched selection is reported by `relation_faults`
    /// rather than by a relation that silently does nothing.
    pub fn add_relation(&mut self, kind: crate::feature::RelationKind, a: Id, slot_a: usize, b: Id, slot_b: usize, value: f64) -> Id {
        let id = self.alloc_id();
        let n = self.relations.iter().filter(|r| r.kind == kind).count() + 1;
        let name_key = match kind {
            crate::feature::RelationKind::Gear => "name-gear-n",
            crate::feature::RelationKind::RackPinion => "name-rack-pinion-n",
            crate::feature::RelationKind::Screw => "name-screw-n",
            crate::feature::RelationKind::Linear => "name-linear-n",
        };
        let mut r = crate::feature::MateRelation { id, name: format!("{name_key}#{n}"), kind, a, slot_a, b, slot_b, value, reversed: false, phase: 0.0 };
        // The phase is captured exactly here, once. A relation ties motion together rather than readings, so
        // creating it must not turn a body that stands in an arbitrary position. Capturing the phase later is
        // wrong: it would then describe a placement the relation itself has already produced.
        r.phase = self.relation_phase(&r).unwrap_or(0.0);
        self.relations.push(r);
        id
    }

    /// How far the two degrees currently differ, in solver units (radians and millimetres).
    ///
    /// Computed from the placements stored in the document rather than from a solution: the phase is needed
    /// before the solver has touched anything.
    pub fn relation_phase(&self, r: &crate::feature::MateRelation) -> Option<f64> {
        let val = |jid: Id, slot: usize| -> Option<(f64, bool)> {
            let j = self.joints.iter().find(|j| j.id == jid)?;
            let kind = crate::asm::bridge::kind_of(j.kind);
            let (_, is_rotation) = crate::asm::joint::slot_axis(kind, slot)?;
            let (wa, wb) = crate::asm::bridge::joint_frames_now(self, j)?;
            let v = crate::asm::bridge::measured_slot(kind, slot, &wa, &wb)?;
            Some((if is_rotation { v.to_radians() } else { v }, is_rotation))
        };
        let (va, rot_a) = val(r.a, r.slot_a)?;
        let (vb, rot_b) = val(r.b, r.slot_b)?;
        let k = crate::asm::bridge::relation_coefficient(r, rot_a, rot_b)?;
        Some(vb - k * va)
    }

    /// Delete a relation by id.
    pub fn delete_relation(&mut self, id: Id) {
        self.relations.retain(|r| r.id != id);
    }

    /// What is wrong with a relation, one code per relation, as `joint_faults` does for mates.
    ///
    /// A relation rests on degrees of freedom that belong to other mates, so it can break without being
    /// touched: delete a mate or change its kind, and the slot it referenced no longer exists. Staying silent
    /// about that would leave the relation not acting while it still looks like it holds.
    pub fn relation_faults(&self) -> Vec<(Id, &'static str)> {
        self.relations
            .iter()
            .filter_map(|r| {
                let (ra, rb) = r.kind.slots_are_rotations();
                if r.kind.needs_two_mates() && r.a == r.b {
                    return Some((r.id, "r-fault-same-mate"));
                }
                if !r.kind.needs_two_mates() && r.a != r.b {
                    return Some((r.id, "r-fault-two-mates"));
                }
                for (jid, slot, want_rotation) in [(r.a, r.slot_a, ra), (r.b, r.slot_b, rb)] {
                    let Some(j) = self.joints.iter().find(|j| j.id == jid) else {
                        return Some((r.id, "r-fault-mate-lost"));
                    };
                    let Some((_, is_rotation)) = crate::asm::joint::slot_axis(crate::asm::bridge::kind_of(j.kind), slot) else {
                        return Some((r.id, "r-fault-slot-lost"));
                    };
                    if is_rotation != want_rotation {
                        return Some((r.id, "r-fault-slot-kind"));
                    }
                }
                None
            })
            .collect()
    }

    /// Whether a body reaches something grounded through mates and groups.
    ///
    /// A set attached to nothing is free as a whole, however tightly its members are fixed to each other.
    /// Breadth-first traversal: a mate connects the owners of its connectors, a group connects all its
    /// members.
    pub fn reaches_ground(&self, id: Id) -> bool {
        let mut seen = vec![id];
        let mut queue = vec![id];
        while let Some(c) = queue.pop() {
            if self.is_grounded(c) {
                return true;
            }
            let step = |n: Id, seen: &mut Vec<Id>, queue: &mut Vec<Id>| {
                if !seen.contains(&n) {
                    seen.push(n);
                    queue.push(n);
                }
            };
            for j in &self.joints {
                let (oa, ob) = (self.connector(j.a).map(|x| x.owner), self.connector(j.b).map(|x| x.owner));
                match (oa, ob) {
                    (Some(a), Some(b)) if a == c => step(b, &mut seen, &mut queue),
                    (Some(a), Some(b)) if b == c => step(a, &mut seen, &mut queue),
                    _ => {}
                }
            }
            for g in &self.mate_constraints {
                if g.members.contains(&c) {
                    for &m in &g.members {
                        step(m, &mut seen, &mut queue);
                    }
                }
            }
        }
        false
    }

    /// Components fixed to `comp` by groups, including `comp` itself: the set that travels together.
    pub fn group_mates(&self, comp: Id) -> Vec<Id> {
        let mut out = vec![comp];
        for g in &self.mate_constraints {
            if g.kind == crate::feature::ConstraintKind::Group && g.members.contains(&comp) {
                for &m in &g.members {
                    if !out.contains(&m) {
                        out.push(m);
                    }
                }
            }
        }
        out
    }

    /// Delete a mate together with the connectors left with nothing to do.
    ///
    /// One core method, because this is a topology operation. Done twice in the interface and differently, one
    /// of the copies counted orphans against the mates of the current assembly only: deleting a mate in one
    /// subassembly also removed the connectors of mates in every other context. Those mates stayed in the
    /// document and never acted again. Measured on a real document: five mates, two connectors.
    pub fn delete_joint(&mut self, id: Id) {
        // The anchors of this mate specifically, not "every anchor no mate uses".
        //
        // The latter removes anchors belonging to other things: a width constraint rests on connectors without
        // being a mate, so deleting any mate took its anchors away while the width constraint stayed in the
        // list and stopped acting — silently, which is the worst way.
        let own: Vec<Id> = self.joints.iter().filter(|j| j.id == id).flat_map(|j| [j.a, j.b]).collect();
        self.joints.retain(|j| j.id != id);
        for c in own {
            // A standalone anchor is left alone entirely: it was created on its own and a second mate may be
            // attached to it later.
            let standalone = self.connector(c).is_some_and(|x| x.standalone);
            if !standalone && self.connector_users(c).is_empty() {
                self.connectors.retain(|x| x.id != c);
            }
        }
    }

    /// Faulty mates with their reason: one source for the solver report, the list and the glyphs.
    ///
    /// Such a mate never enters the computation, the bridge skipping it. While the reason was named nowhere,
    /// this looked like an assembled assembly whose bodies do not move — measured on a real document where
    /// four of five mates referenced connectors the document no longer held.
    ///
    /// The reason is a catalogue key, so the report and the interface use the same words.
    pub fn joint_faults(&self) -> Vec<(Id, &'static str)> {
        let why = |c: Id| match self.connector(c) {
            None => Some("j-fault-connector-lost"),
            Some(c) => self.connector_frame(c).is_none().then_some("j-fault-anchor-lost"),
        };
        // An anchor attached to a moving body inside its own assembly is a fault.
        //
        // An anchor is declared on a component while taking its geometry from a body. Usually that body lies
        // inside the same component and stands still within it, which is honest. But when the body belongs to a
        // moving part inside that same component, the anchor moves whenever that part moves: the mate holds on
        // to something that does not stand still itself.
        //
        // Measured on a machine document: anchor B of one mate was declared on a subassembly while its geometry
        // lay in a plate that moves inside that same subassembly under another mate. The loop is: the plate
        // moves, the anchor moves with it, the subassembly catches up, the plate moves again. Every recompute
        // shifted the assembly by exactly 60 mm, endlessly, while both parts of the solve converged to a
        // residual of 1e-10 — each was right on its own.
        //
        // Staying silent is not an option: from the outside the assembly drifts apart by itself, and the cause
        // is looked for anywhere except the anchor that was placed on the wrong body.
        let on_moving_child = |cid: Id| -> Option<&'static str> {
            let c = self.connector(cid)?;
            self.anchor_sits_on_moving_part(c.owner, &c.anchor).then_some("j-fault-anchor-on-moving-part")
        };
        self.joints
            .iter()
            .filter_map(|j| why(j.a).or_else(|| why(j.b)).or_else(|| on_moving_child(j.a)).or_else(|| on_moving_child(j.b)).map(|w| (j.id, w)))
            .collect()
    }

    /// 3x4 matrix of a connector frame in the local space of its component.
    pub fn connector_matrix(&self, conn_id: Id) -> Option<[f64; 12]> {
        let c = self.connector(conn_id)?;
        self.connector_frame(c).map(|f| f.matrix12())
    }

    /// Mate list of a context: everything that holds the bodies together, in one list.
    ///
    /// Mates, constraints and relations belong in one list, each row with its own icon, its own state and the
    /// same menu. Three separate loops in the panel, each with its own row, its own delete button and its own
    /// idea of whether an item is sound, are three views of one thing that drift apart silently and show what
    /// is not there.
    ///
    /// The list is decided here rather than in the panel: the panel draws what it is given and cannot invent
    /// anything of its own. The order is by id, that is, by creation time; the order does not affect the
    /// solution, since everything is solved at once.
    pub fn mate_timeline(&self, ctx: Id) -> Vec<crate::feature::MateEntry> {
        use crate::feature::{MateEntry, MateItem, MateState};
        let jf = self.joint_faults();
        let rf = self.relation_faults();
        let mut out: Vec<MateEntry> = Vec::new();
        for j in &self.joints {
            if !self.joint_in_context(j, ctx) {
                continue;
            }
            let state = match jf.iter().find(|(id, _)| *id == j.id) {
                Some((_, why)) => MateState::Faulty(why),
                None if self.mates_violated.contains(&j.id) => MateState::Violated,
                None => MateState::Ok,
            };
            let touches = [j.a, j.b].iter().filter_map(|c| self.connector(*c).map(|c| c.owner)).collect();
            out.push(MateEntry { item: MateItem::Joint, id: j.id, name: j.name.clone(), kind_label: j.kind.label(), state, touches });
        }
        for g in &self.mate_constraints {
            // Which bodies a constraint holds: for a group its members, for a width constraint the owners of
            // its anchors, for tangency the owners of its surfaces. A constraint belonging to another assembly
            // does not enter this list.
            let touches: Vec<Id> = g
                .members
                .iter()
                .copied()
                .chain(g.anchors.iter().filter_map(|c| self.connector(*c).map(|x| x.owner)))
                .chain(g.faces.iter().map(|(o, _)| *o))
                .collect();
            if !touches.iter().any(|&m| self.component_is_within(m, ctx)) {
                continue;
            }
            // A lost anchor is the same failure as for a mate and is named the same way: the constraint exists
            // in the document and has nothing to hold with.
            let state = if g.anchors.iter().any(|c| self.connector(*c).is_none()) { MateState::Faulty("j-fault-connector-lost") } else { MateState::Ok };
            out.push(MateEntry { item: MateItem::Constraint, id: g.id, name: g.name.clone(), kind_label: g.kind.label(), state, touches });
        }
        for r in &self.relations {
            // A relation lives where its mates live: it has no bodies of its own, tying degrees together
            // instead.
            let mates: Vec<&crate::feature::Joint> = self.joints.iter().filter(|j| j.id == r.a || j.id == r.b).collect();
            if mates.is_empty() || !mates.iter().any(|j| self.joint_in_context(j, ctx)) {
                continue;
            }
            let state = match rf.iter().find(|(id, _)| *id == r.id) {
                Some((_, why)) => MateState::Faulty(why),
                None if self.mates_violated.contains(&r.id) => MateState::Violated,
                None => MateState::Ok,
            };
            let touches = mates.iter().flat_map(|j| [j.a, j.b]).filter_map(|c| self.connector(c).map(|c| c.owner)).collect();
            out.push(MateEntry { item: MateItem::Relation, id: r.id, name: r.name.clone(), kind_label: r.kind.label(), state, touches });
        }
        out.sort_by_key(|e| e.id);
        out
    }
}
