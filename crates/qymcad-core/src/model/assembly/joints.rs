//! CONNECTORS AND JOINTS: added, anchored, flipped, driven, limited.

use super::*;

impl Project {
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
    pub(crate) fn driven_body_span(&self, joint: Id, slot: usize) -> Option<f64> {
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

    /// SET THE DRIVER ON ONE DEGREE OF FREEDOM, held inside its limits.
    ///
    /// Four places in the interface used to do this by hand - dragging the gizmo, restoring a value after a
    /// sweep, stepping the sweep itself, and reading the numbers off the command bar - and they did not
    /// agree: the gizmo clamped to the limit, the other three did not. No wrong picture came of it (the
    /// sweep interpolates between two values that are already in range), but "a driver never leaves its
    /// limits" was held by four separate memories instead of by the code. Here it is held once.
    ///
    /// `None` clears the driver: the degree of freedom goes back to being free, and the solver decides it.
    /// Returns `false` when there is no such mate or no such slot, so a caller cannot mistake a miss for a
    /// change.
    pub fn set_joint_drive(&mut self, joint: Id, slot: usize, value: Option<f64>) -> bool {
        if slot >= 3 {
            return false;
        }
        let Some(j) = self.joints.iter_mut().find(|x| x.id == joint) else { return false };
        j.drive[slot] = value.map(|v| j.clamp_slot(slot, v));
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
}
