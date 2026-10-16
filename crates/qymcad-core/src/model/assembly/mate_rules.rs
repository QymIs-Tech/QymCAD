//! THE CONSTRAINTS AND RELATIONS OF MATES: added, deleted, their faults, the timeline of mates.

use super::*;

impl Project {
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
            faces: vec![(owner_a, a.clone()), (owner_b, b.clone())],
        });
        self.lay_down_a_standing_cylinder(owner_a, &a, owner_b, &b);
        id
    }

    /// A CYLINDER STANDING ON ITS END IS TURNED TO LIE. With its axis along the plane's normal it touches the plane
    /// nowhere, and the solver cannot leave that pose: the derivative of "axis parallel to the plane" is zero at a
    /// right angle. So the part that is free (the cylinder's, else the plane's) is turned a quarter about a line
    /// across the axis through the cylinder's own axis point, and the solve lays it on the face from there.
    pub(crate) fn lay_down_a_standing_cylinder(&mut self, owner_a: Id, a: &crate::feature::AnchorRef, owner_b: Id, b: &crate::feature::AnchorRef) {
        use crate::feature::{apply12, mat_inv12, mat_mul12, AnchorRef};
        let cyl = |r: &AnchorRef| match r {
            AnchorRef::FaceCenter(body, key) => self.face_cylinder(*body, key),
            _ => None,
        };
        let plane = |r: &AnchorRef| match r {
            AnchorRef::FaceCenter(body, key) if self.face_cylinder(*body, key).is_none() => Some(self.resolve_face(*body, key)),
            _ => None,
        };
        let Some(((co, (o, ax, _)), (po, (_, n)))) = cyl(a).map(|c| (owner_a, c)).zip(plane(b).map(|p| (owner_b, p))).or_else(|| cyl(b).map(|c| (owner_b, c)).zip(plane(a).map(|p| (owner_a, p))))
        else {
            return;
        };
        let turn_dir = |m: &[f64; 12], v: [f64; 3]| [m[0] * v[0] + m[1] * v[1] + m[2] * v[2], m[4] * v[0] + m[5] * v[1] + m[6] * v[2], m[8] * v[0] + m[9] * v[1] + m[10] * v[2]];
        let (wc, wp) = (self.world_transform(co), self.world_transform(po));
        let (wax, wn) = (turn_dir(&wc, ax), turn_dir(&wp, n));
        if (wax[0] * wn[0] + wax[1] * wn[1] + wax[2] * wn[2]).abs() < 0.999 {
            return; // not standing: the solve finds the way itself
        }
        let moving = if !self.is_grounded(co) {
            co
        } else if !self.is_grounded(po) {
            po
        } else {
            return;
        };
        // a line across the axis: the world axis least aligned with it, made square to it
        let k = (0..3).min_by(|&i, &j| wax[i].abs().total_cmp(&wax[j].abs())).unwrap_or(0);
        let mut e = [0.0; 3];
        e[k] = 1.0;
        let d = e[0] * wax[0] + e[1] * wax[1] + e[2] * wax[2];
        let u = [e[0] - d * wax[0], e[1] - d * wax[1], e[2] - d * wax[2]];
        let l = (u[0] * u[0] + u[1] * u[1] + u[2] * u[2]).sqrt();
        let u = [u[0] / l, u[1] / l, u[2] / l];
        // a quarter turn about `u` through the cylinder's axis point (Rodrigues, sin 1, cos 0)
        let p = apply12(&wc, o);
        let r = [u[0] * u[0], u[0] * u[1] - u[2], u[0] * u[2] + u[1], u[1] * u[0] + u[2], u[1] * u[1], u[1] * u[2] - u[0], u[2] * u[0] - u[1], u[2] * u[1] + u[0], u[2] * u[2]];
        let t = [p[0] - (r[0] * p[0] + r[1] * p[1] + r[2] * p[2]), p[1] - (r[3] * p[0] + r[4] * p[1] + r[5] * p[2]), p[2] - (r[6] * p[0] + r[7] * p[1] + r[8] * p[2])];
        let turn = [r[0], r[1], r[2], t[0], r[3], r[4], r[5], t[1], r[6], r[7], r[8], t[2]];
        // the turn is made in the world; the part's own placement is in its parent
        let (local, world) = (self.component_transform(moving), self.world_transform(moving));
        let parent = mat_mul12(&world, &mat_inv12(&local));
        self.set_component_transform(moving, mat_mul12(&mat_inv12(&parent), &mat_mul12(&turn, &world)));
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
    // THE INDEX IS THE MEANING: Gauss-Jordan over a 4x5 matrix - rows and columns are what it is about.
    #[allow(clippy::needless_range_loop)]
    pub fn face_sphere(&self, body: Id, key: &crate::feature::FaceKey) -> Option<([f64; 3], f64)> {
        // A face with an axis - every normal square to one line - is a cylinder, and no sphere has one. The normal
        // check below cannot tell them apart on a squat cylinder: at radius 10 and length 20 its facets lean only
        // 0.95 off the direction from the centre of the sphere through both rims (radius 14.14).
        if self.face_axis(body, key).is_some() {
            return None;
        }
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
                        pts.push([v.x, v.y, v.z]);
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
        let span = pts.iter().map(|p| ((p[0] - f.centroid.x).powi(2) + (p[1] - f.centroid.y).powi(2) + (p[2] - f.centroid.z).powi(2)).sqrt()).fold(0.0f64, f64::max);
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
            let u = [(p1.x - p0.x), (p1.y - p0.y), (p1.z - p0.z)];
            let w = [(p2.x - p0.x), (p2.y - p0.y), (p2.z - p0.z)];
            let n = [u[1] * w[2] - u[2] * w[1], u[2] * w[0] - u[0] * w[2], u[0] * w[1] - u[1] * w[0]];
            let nl = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            if nl < 1e-12 {
                continue;
            }
            let mid = [(p0.x + p1.x + p2.x) / 3.0 - c[0], (p0.y + p1.y + p2.y) / 3.0 - c[1], (p0.z + p1.z + p2.z) / 3.0 - c[2]];
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

    pub(crate) fn add_mate_constraint(&mut self, kind: crate::feature::ConstraintKind, members: Vec<Id>, anchors: Vec<Id>, name_key: &str) -> Id {
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
        self.joints.iter().filter_map(|j| why(j.a).or_else(|| why(j.b)).or_else(|| on_moving_child(j.a)).or_else(|| on_moving_child(j.b)).map(|w| (j.id, w))).collect()
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
            let touches: Vec<Id> = g.members.iter().copied().chain(g.anchors.iter().filter_map(|c| self.connector(*c).map(|x| x.owner))).chain(g.faces.iter().map(|(o, _)| *o)).collect();
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
