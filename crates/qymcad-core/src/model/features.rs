//! FEATURES AND DATUMS ADDED AND DELETED: extrusions, revolutions, chamfers, shells, arrays, planes, axes, points, holes.

use super::*;

impl Project {
    /// Write the mesh of a body by id: replace the existing one or create a new body with that id.
    pub fn set_body_mesh(&mut self, id: Id, m: crate::geom::Mesh) {
        if let Some(idx) = self.mesh_index(id) {
            self.bodies[idx].mesh = m;
        } else {
            let name = format!("name-body#{}", self.bodies.len() + 1);
            self.bodies.push(Body { id, name, mesh: m, faces: Vec::new(), visible: true, sheet: false });
        }
    }

    /// Extrude the first closed contour of a sketch. Returns the id of the resulting body.
    pub fn add_extrude(&mut self, sketch: Id, height: f64) -> Id {
        self.add_extrude_on(sketch, 0, height, crate::feature::Reach::Forward, 0.0)
    }

    /// Revolve every closed contour of a sketch as one node.
    pub fn add_revolve(&mut self, sketch: Id, axis: u8, angle: f64) -> Id {
        self.add_revolve_axis(sketch, Vec::new(), axis, angle, 0, 0)
    }

    /// Revolve a specific contour `profile` (0 means every closed one). `axis_datum` is a datum axis; 0 uses
    /// the X or Y axis of the sketch.
    pub fn add_revolve_on(&mut self, sketch: Id, profile: Id, axis: u8, angle: f64) -> Id {
        let profiles = if profile == 0 { Vec::new() } else { vec![profile] };
        self.add_revolve_axis(sketch, profiles, axis, angle, 0, 0)
    }

    /// Add a datum point (a `DatumPoint` timeline node) and return its id.
    pub fn add_datum_point(&mut self, mut dp: DatumPoint) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        if dp.id == 0 {
            dp.id = self.alloc_id();
        }
        let (id, name) = (dp.id, dp.name.clone());
        let parent = Some(self.active_ctx());
        self.datum_points.push(dp);
        self.push_timeline(FeatureNode { id, name, kind: FeatureKind::DatumPoint { point: id }, parent, dirty: false, suppressed: false });
        id
    }

    /// Add a datum axis (a `DatumAxis` timeline node) and return its id.
    pub fn add_datum_axis(&mut self, mut da: DatumAxis) -> Id {
        use crate::feature::{FeatureKind, FeatureNode};
        if da.id == 0 {
            da.id = self.alloc_id();
        }
        let (id, name) = (da.id, da.name.clone());
        let parent = Some(self.active_ctx());
        // AN AXIS TAKEN FROM SOMETHING IS WHERE THAT THING IS ONLY ONCE A REBUILD HAS LOOKED: an edge, a round face or
        // two points are resolved in the timeline pass. Left clean, a window that rebuilds only what is marked never
        // ran that pass, and the axis stood at its default - the world Z through the origin.
        let derived = !matches!(da.def, AxisDef::Manual { .. });
        self.datum_axes.push(da);
        self.push_timeline(FeatureNode { id, name, kind: FeatureKind::DatumAxis { axis: id }, parent, dirty: derived, suppressed: false });
        id
    }

    /// Delete a datum plane by id, from `planes` and from the timeline together. One method rather than
    /// logic inlined in the interface. Returns whether the plane existed.
    /// Delete a datum plane ALONE, as a base is deleted (decided 24.09): what stands on it stays, red with the reason.
    /// A sketch on it is put on a snapshot of where the plane stood (`PlaneDef::PlaneGone`), so it does not move and
    /// goes red saying its plane was deleted; an offset plane built from it stays where it stands, a plane of its own.
    pub fn delete_plane(&mut self, id: Id) -> bool {
        use crate::feature::SketchPlane;
        let Some(gone) = self.planes.iter().find(|p| p.id == id).cloned() else { return false };
        let on_plane: Vec<(usize, Id)> = self.sketches.iter().enumerate().filter(|(_, s)| matches!(s.plane, SketchPlane::Datum(pid) if pid == id)).map(|(si, s)| (si, s.id)).collect();
        if !on_plane.is_empty() {
            let snap =
                self.add_plane(WorkPlane { name: "name-plane-deleted-frozen".into(), origin: gone.origin, normal: gone.normal, rot_deg: gone.rot_deg, def: PlaneDef::PlaneGone, ..Default::default() });
            for (si, sid) in on_plane {
                self.sketches[si].plane = SketchPlane::Datum(snap);
                self.mark_node_dirty(sid); // the rebuild visits it and marks it red
            }
        }
        for p in self.planes.iter_mut().filter(|p| matches!(p.def, PlaneDef::OffsetPlane { plane, .. } if plane == id)) {
            p.def = PlaneDef::Manual; // it keeps the origin and the normal it was last resolved to
        }
        self.drop_plane_node(id)
    }

    /// Delete a datum plane WITH what is built on it (the "and what is built on it" box): the sketches on it and what
    /// they make, and the offset planes built from it, recursively.
    pub fn delete_plane_with_dependents(&mut self, id: Id) -> bool {
        use crate::feature::SketchPlane;
        if !self.planes.iter().any(|p| p.id == id) {
            return false;
        }
        let on_plane: Vec<Id> = self.sketches.iter().filter(|s| matches!(s.plane, SketchPlane::Datum(pid) if pid == id)).map(|s| s.id).collect();
        for sid in on_plane {
            self.delete_sketch_with_dependents(sid);
        }
        let children: Vec<Id> = self.planes.iter().filter(|p| matches!(p.def, PlaneDef::OffsetPlane { plane, .. } if plane == id)).map(|p| p.id).collect();
        for cid in children {
            self.delete_plane_with_dependents(cid);
        }
        self.drop_plane_node(id)
    }

    /// The plane itself goes, and the mirrors and splits that named it go red on the rebuild.
    pub(crate) fn drop_plane_node(&mut self, id: Id) -> bool {
        use crate::feature::FeatureKind;
        let had = self.planes.iter().any(|p| p.id == id);
        // Neither mirror nor section degrades. The reference is left dangling so that regenerate sees the
        // missing plane and goes red honestly.
        //
        // Degrading a mirror to the world plane (datum = 0) sounds defensible — a mirror about another
        // plane is still a mirror — but the measurement refutes it: a mirror about a datum at x = 50 gave a
        // part with face centres from x = 10 to x = 90, and deleting the datum moved them to -30..30. The
        // part moved and not a single node went red. Section has refused in the same situation for a long
        // time, for the same reason: a silent substitution is invisible until someone notices the part is
        // in the wrong place.
        for n in &mut self.timeline {
            let touches = match &n.kind {
                FeatureKind::Mirror { datum, .. } | FeatureKind::SplitBody { datum, .. } => *datum == id,
                _ => false,
            };
            if touches {
                n.dirty = true;
            }
        }
        self.planes.retain(|p| p.id != id);
        self.timeline.retain(|n| !matches!(n.kind, FeatureKind::Plane { plane } if plane == id));
        had
    }

    /// Delete a datum point by id, from `datum_points` and from the timeline together. Two-point axes and
    /// patterns that used it degrade through regenerate, resolving to their defaults. Returns whether the
    /// point existed.
    pub fn delete_datum_point(&mut self, id: Id) -> bool {
        let had = self.datum_points.iter().any(|p| p.id == id);
        if !had {
            return false;
        }
        // Two-point axes that use this point: mark their nodes dirty. Resolution will freeze or degrade the
        // axis to its previous value, but the rebuild has to run cleanly, without stale consumer geometry.
        let affected: Vec<Id> = self.datum_axes.iter().filter(|a| matches!(a.def, AxisDef::TwoPoints { a: pa, b: pb } if pa == id || pb == id)).map(|a| a.id).collect();
        self.datum_points.retain(|p| p.id != id);
        self.timeline.retain(|n| !matches!(n.kind, crate::feature::FeatureKind::DatumPoint { point } if point == id));
        for aid in affected {
            for n in &mut self.timeline {
                if matches!(n.kind, crate::feature::FeatureKind::DatumAxis { axis } if axis == aid) {
                    n.dirty = true;
                }
            }
        }
        had
    }

    /// Delete a datum axis by id, from `datum_axes` and from the timeline together. Circular patterns around
    /// it degrade through regenerate, resolving to world Z. Returns whether the axis existed.
    pub fn delete_datum_axis(&mut self, id: Id) -> bool {
        use crate::feature::FeatureKind;
        let had = self.datum_axes.iter().any(|a| a.id == id);
        if !had {
            return false;
        }
        // Degrading references: a revolve around this axis falls back to the sketch X or Y axis
        // (`axis_datum = 0`), and a circular pattern falls back to world Z (`axis = 0`). Without marking the
        // nodes dirty here the degradation happened only inside regenerate and left stale geometry behind.
        //
        // The measured price of this behaviour is high: the part moves silently. A revolve around an axis
        // at x = 40 produced a body at x = 40 and, without the axis, at x = 0; a pattern around an axis at
        // x = 60 spread its centres over 0..120 and, without the axis, over -10..10 — with no red node in
        // either case. Neighbouring behaviour (section, mirror) refuses to build in the same situation.
        //
        // Here, however, degradation is a recorded contract rather than code that merely happened: the
        // tests `delete_datum_axis_degrades_circular_array` and `revolve_around_datum_axis_uses_axis_kernel`
        // assert it. Changing that contract is a separate decision.
        for n in &mut self.timeline {
            match &mut n.kind {
                FeatureKind::Revolve { axis_datum, .. } => {
                    if *axis_datum == id {
                        *axis_datum = 0;
                        n.dirty = true;
                    }
                }
                FeatureKind::CircularArray { axis, .. } if *axis == id => {
                    *axis = 0;
                    n.dirty = true;
                }
                _ => {}
            }
        }
        self.datum_axes.retain(|a| a.id != id);
        self.timeline.retain(|n| !matches!(n.kind, FeatureKind::DatumAxis { axis } if axis == id));
        had
    }

    /// Add a chamfer on edges of body `src` (an empty `edges` means every edge). Returns the id of the result.
    pub fn add_chamfer(&mut self, src: Id, dist: f64, edges: Vec<u32>) -> Id {
        use crate::feature::ChamferMode;
        self.add_chamfer_ex(src, dist, ChamferShape { mode: ChamferMode::Symmetric, d2: 0.0, flip: false, ref_face: 0 }, edges)
    }

    /// Add a shell of body `src`: remove faces `faces` and leave a wall of `thickness`. Returns the id of the
    /// result.
    pub fn add_shell(&mut self, src: Id, thickness: f64, faces: Vec<u32>, outward: bool) -> Id {
        let side = if outward { crate::feature::ShellSide::Outward } else { crate::feature::ShellSide::Inward };
        self.add_shell_mode(src, thickness, faces, side)
    }

    /// Add a linear pattern of body `src`: `count` copies spaced by (dx, dy, dz). Returns the result id.
    pub fn add_linear_array(&mut self, src: Id, dx: f64, dy: f64, dz: f64, count: u32) -> Id {
        self.add_linear_array_grid(src, ArrayAxis { d: [dx, dy, dz], count }, ArrayAxis::none())
    }

    /// Grid pattern: direction one (`d1` by `count`) and direction two (`d2` by `count2`).
    #[allow(clippy::too_many_arguments)]
    pub fn add_linear_array_grid(&mut self, src: Id, a: ArrayAxis, b: ArrayAxis) -> Id {
        let ([dx, dy, dz], count) = (a.d, a.count);
        let ([dx2, dy2, dz2], count2) = (b.d, b.count);
        self.add_linear_array_grid3(src, [ArrayAxis { d: [dx, dy, dz], count }, ArrayAxis { d: [dx2, dy2, dz2], count: count2 }, ArrayAxis::none()])
    }

    /// Add a circular pattern of body `src`: `count` copies over `angle` degrees. `axis` is a datum axis id,
    /// or 0 for world Z.
    pub fn add_circular_array(&mut self, src: Id, count: u32, angle: f64) -> Id {
        self.add_circular_array_axis(src, count, angle, 0)
    }

    /// Mirrored component in an assembly: a new sibling of the source whose body is the reflection of the
    /// active body of `src_comp` through the world plane `wo`/`wn` (a click on XY, XZ, YZ, a datum or a
    /// face).
    ///
    /// The shape is associative — editing the source rebuilds it — while the placement is free, so the
    /// mirrored component can be moved with the gizmo. A part yields one mirrored part.
    ///
    /// A subassembly yields a mirrored sibling subassembly reflected as one rigid whole. The world mirror
    /// plane is used once, on the assembly itself: its placement is the reflected world zero of the source,
    /// with the orientation preserved. Inside it, the rotation of each part relative to the subassembly is
    /// left exactly as in the original, while its local offset is reflected by the same formula in the
    /// local space of the subassembly (`add_mirror_part_rigid`). Without that, the mutual arrangement of
    /// the parts — which one is left of which — would not be mirrored with the whole and would end up on
    /// the original side. Returns the ids of the mirror nodes.
    pub fn add_mirror_component(&mut self, src_comp: Id, wo: [f64; 3], wn: [f64; 3]) -> Vec<Id> {
        if self.component_is_part(src_comp) {
            return vec![self.add_mirror_part(src_comp, wo, wn)];
        }
        let (src_name, src_parent) = self.components.iter().find(|c| c.id == src_comp).map(|c| (c.name.clone(), c.parent)).unwrap_or(("name-assembly".into(), None));
        let saved = self.active_component;
        self.active_component = src_parent.or(saved);
        let asm = self.add_assembly(format!("name-mirror-of#{src_name}"));
        self.active_component = saved;
        // One reflected frame for the whole subassembly: the orientation of the source, with the world zero
        // reflected through (wo, wn) — the same formula as for a single part, applied to the subassembly as
        // one rigid body.
        let ssrc = self.world_transform(src_comp);
        let n_sa = crate::feature::apply12_dir(&crate::feature::mat_inv12(&ssrc), wn); // Normal in subassembly local space, reused for the child offsets.
        let t = [ssrc[3], ssrc[7], ssrc[11]];
        let d = (t[0] - wo[0]) * wn[0] + (t[1] - wo[1]) * wn[1] + (t[2] - wo[2]) * wn[2];
        let t_new = [t[0] - 2.0 * d * wn[0], t[1] - 2.0 * d * wn[1], t[2] - 2.0 * d * wn[2]];
        let mut rfull = ssrc;
        rfull[3] = t_new[0];
        rfull[7] = t_new[1];
        rfull[11] = t_new[2];
        let ap = self.components.iter().find(|c| c.id == asm).and_then(|c| c.parent).map(|pp| self.world_transform(pp)).unwrap_or(crate::feature::PLACE_IDENTITY);
        self.set_component_transform(asm, crate::feature::mat_mul12(&crate::feature::mat_inv12(&ap), &rfull));
        // Every part in the subtree that holds bodies (`src_comp` itself is an assembly, so walk its descendants).
        let parts: Vec<Id> = self.descendants(src_comp).into_iter().filter(|&c| self.component_is_part(c) && self.timeline.iter().any(|n| n.parent == Some(c) && n.kind.body().is_some())).collect();
        let mut out = Vec::new();
        for part in parts {
            out.push(self.add_mirror_part_rigid(part, src_comp, n_sa, wn, asm));
        }
        out
    }

    /// Datum plane offset from a face by `dist`, used by mirrors and by sketches on a face. Associative: it
    /// is resolved during regenerate.
    pub fn add_plane_from_face(&mut self, body: Id, face: crate::feature::FaceKey, dist: f64) -> Id {
        let wp = WorkPlane { name: "name-plane-from-face".into(), def: PlaneDef::OffsetFace { body, face, dist }, ..Default::default() };
        self.add_plane(wp)
    }

    /// Snapshot a face of another component's body into a fixed (`Manual`) datum plane owned by the active
    /// component.
    ///
    /// Unlike a live external reference — associative top-down design that pins a part to a neighbour's
    /// face — this is a one-off snapshot: the world frame of the source face is converted into the local
    /// space of the consumer, through its current `world_transform`, and stored as coordinates. After that
    /// the part moves independently and is mated by hand. Returns the id of the new datum plane, whose
    /// parent is the active context, that is, the consumer.
    pub fn snapshot_face_plane(&mut self, body: Id, face: &crate::feature::FaceKey) -> Id {
        let consumer = self.active_ctx();
        self.snapshot_face_plane_for(consumer, body, face)
    }

    /// Like [`Project::snapshot_face_plane`], but with the consumer given explicitly. Breaking an external
    /// reference does not happen from the active context: the part to repair is the one that held the
    /// reference, wherever the current context happens to be.
    pub fn snapshot_face_plane_for(&mut self, consumer: Id, body: Id, face: &crate::feature::FaceKey) -> Id {
        use crate::feature::{apply12, apply12_dir, mat_inv12, mat_mul12};
        // The face resolves in source-body local space: source local, then world, then consumer local.
        let (c, n) = self.resolve_face(body, face);
        let src_to_world = self.body_world_transform(body);
        let world_to_consumer = mat_inv12(&self.world_transform(consumer));
        let m = mat_mul12(&world_to_consumer, &src_to_world);
        let origin = apply12(&m, c);
        let normal = apply12_dir(&m, n);
        let wp = WorkPlane { name: "name-plane-from-face-frozen".into(), origin, normal, rot_deg: 0.0, tilt: Default::default(), def: PlaneDef::Manual, ..Default::default() };
        self.add_plane(wp)
    }

    /// Datum plane offset from a base plane (XY, XZ, YZ) by `dist` (parametric through the `dist` feature
    /// dimension).
    pub fn add_offset_plane(&mut self, base: crate::feature::BasePlane, dist: f64) -> Id {
        let wp = WorkPlane { name: "name-plane".into(), def: PlaneDef::OffsetBase { base, dist }, ..Default::default() };
        self.add_plane(wp)
    }

    /// Datum plane offset from another datum plane `src` by `dist` (parametric).
    pub fn add_offset_from_plane(&mut self, src: Id, dist: f64) -> Id {
        let wp = WorkPlane { name: "name-plane".into(), def: PlaneDef::OffsetPlane { plane: src, dist }, ..Default::default() };
        self.add_plane(wp)
    }

    /// Datum point at coordinates `at` (parametric through the `x`, `y` and `z` feature dimensions).
    pub fn add_point_at(&mut self, at: [f64; 3]) -> Id {
        self.add_datum_point(DatumPoint { name: "name-datum-point".into(), at, ..Default::default() })
    }

    /// Datum point bound associatively to a vertex of a body (an endpoint of a persistent edge): it travels
    /// with the vertex when the source is rebuilt. `at` is the initial snapshot in body local space and is
    /// refined by resolution.
    pub fn add_point_at_vertex(&mut self, at: [f64; 3], body: Id, edge: u32, end: bool) -> Id {
        self.add_datum_point(DatumPoint { name: "name-datum-point".into(), at, def: PointDef::AtVertex { body, edge, end }, ..Default::default() })
    }

    /// Manual datum axis: an origin and a direction (normalised at the point of use).
    pub fn add_axis_manual(&mut self, origin: [f64; 3], dir: [f64; 3]) -> Id {
        self.add_datum_axis(DatumAxis { name: "name-datum-axis".into(), def: AxisDef::Manual { origin, dir }, ..Default::default() })
    }

    /// Datum axis through two datum points `a` and `b` (parametric: origin = a and dir = norm(b - a), both
    /// resolved during regenerate).
    pub fn add_axis_two_points(&mut self, a: Id, b: Id) -> Id {
        self.add_datum_axis(DatumAxis { name: "name-datum-axis".into(), def: AxisDef::TwoPoints { a, b }, ..Default::default() })
    }

    /// Datum axis bound associatively to a straight edge of a body (persistent id): it travels with the edge
    /// when the body is rebuilt.
    pub fn add_axis_from_edge(&mut self, body: Id, edge: u32) -> Id {
        self.add_datum_axis(DatumAxis { name: "name-datum-axis".into(), def: AxisDef::FromEdge { body, edge }, ..Default::default() })
    }

    /// Datum axis bound associatively to the axis of a cylindrical or conical face of a body (persistent
    /// id).
    pub fn add_axis_from_face(&mut self, body: Id, face: u32) -> Id {
        self.add_datum_axis(DatumAxis { name: "name-datum-axis".into(), def: AxisDef::FromFace { body, face }, ..Default::default() })
    }

    /// Add a hole to body `src` (a cylinder of `diameter` and `depth`, at `point` along `normal`).
    ///
    /// The hole is placed by a face reference (`face`, with a persistent id): the centre and normal are
    /// resolved from that face on every rebuild, so the hole travels with the face. The stored
    /// `face.centroid` and `face.normal` are kept as a fallback fingerprint.
    pub fn add_hole(&mut self, src: Id, face: crate::feature::FaceKey, diameter: f64, depth: f64) -> Id {
        self.add_hole_typed(src, face, HoleTool { kind: 0, diameter, depth, dia2: 0.0, depth2: 0.0 })
    }
}
