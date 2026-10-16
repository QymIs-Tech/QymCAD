//! THE BODIES OF A REBUILD PREPARED: imports, instances, threads, holes, splits, arrays, the node itself.

use super::*;

impl Project {
    /// Import: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    pub(crate) fn regen_import(&mut self, p: &mut Pass, body: Id) -> bool {
        // An external STEP solid has no recipe, so this only re-tessellates the shape from the kernel
        // cache - and lands it the way every result lands, so its faces and edges are named by the one rule (where
        // nothing else is known about a face, by where it lies). Without a shape in the kernel (a mock, or before
        // restoration from the source) the already loaded mesh is kept.
        //
        // THROUGH THE DOOR, as every body: a body read from a file comes as the file holds it - a face pinched to two
        // places, a crack, a shell the check calls broken - and nothing cleaned it, the operations on it meeting the flaw.
        // Landed through the door it is cleaned and checked; one the door cannot repair is kept as it came, in yellow.
        self.regen_warnings.remove(&p.node);
        match p.kernel.land_import(body) {
            Some(crate::feature::ImportLanded { built, kept }) => {
                if let Some(why) = kept {
                    self.regen_warnings.insert(p.node, why);
                }
                self.apply_regen(p.landing(), body, Ok(built))
            }
            None => true, // The body is valid (a mesh exists), so the node is clean and its consumers continue.
        }
    }

    /// PartInstance: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    pub(crate) fn regen_partinstance(&mut self, p: &mut Pass, at: usize, src_comp: Id, body: Id) -> bool {
        // An instance body is a copy of the active source body as it is. The position comes from the
        // transform of the copied component, driven by the pattern, so the geometry is not moved.
        //
        // Only from what lies above: the same forward-reference rule as for a mirror.
        let res = match self.active_body_before(src_comp, at) {
            None => Err(crate::errors::CoreError::SourcePartHasNoBody),
            Some(sb) => p.kernel.transform_body(body, sb, crate::feature::PLACE_IDENTITY),
        };
        self.apply_regen(p.landing(), body, res)
    }

    /// MirrorPart: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    pub(crate) fn regen_mirrorpart(&mut self, p: &mut Pass, at: usize, src_comp: Id, ln: [f64; 3], body: Id) -> bool {
        // The mirror plane passes through the local zero of the source, and the normal `ln` was fixed
        // in its local space at creation (see `add_mirror_part` and `add_mirror_part_rigid`), so the
        // coordinate system of the copy is not mirrored and keeps the orientation of the source. The
        // placement of a mirror is never touched by regenerate — it is moved by hand — while the
        // shape is associative and rebuilds with the source.
        let res = if ln[0] * ln[0] + ln[1] * ln[1] + ln[2] * ln[2] < 0.25 {
            Err(crate::errors::CoreError::MirrorPlaneUnset)
        } else {
            // Only from what lies above: a timeline node may not rest on a body built by a node
            // below it.
            //
            // Reverting this restriction on the suspicion that it broke a real document was wrong:
            // measured, that document contained zero mirrors and could not have been affected, and
            // it failed to open because of the nesting ladder in the selected-edge list — an
            // unrelated defect. The restriction stands, and "the part has no source" is stated
            // honestly: that part really has no body left.
            match self.active_body_before(src_comp, at) {
                None => Err(crate::errors::CoreError::SourcePartHasNoBody),
                Some(sb) => p.kernel.mirror_plane(
                    body,
                    sb,
                    crate::feature::PlaneAt { origin: [0.0; 3], normal: ln },
                    crate::feature::Mirrored { keeps: crate::feature::MirrorKeeps::Image, names: crate::feature::ImageNames::Source },
                ),
            }
        };
        self.apply_regen(p.landing(), body, res)
    }

    /// WHAT REFUSES A THREAD BEFORE THE KERNEL IS ASKED, on the cylinder of radius `r` whose rim centre and axis
    /// along the face are `rim`: the side of the material, then the size, the depth, the pitch, the turns, and the
    /// length against the face. The thread's geometry and the radius it is cut at when nothing refuses.
    pub(crate) fn thread_verdict(
        &self,
        src: Id,
        rim: ([f64; 3], [f64; 3], f64),
        spec: &mut crate::thread::ThreadSpec,
        length: f64,
    ) -> Result<(crate::thread::ThreadGeom, f64), crate::errors::CoreError> {
        let (c, ax, r) = rim;
        // The material side comes from the geometry rather than from the checkbox. A groove
        // pointing the wrong way cuts empty space: on a shaft marked "internal" it removed
        // 1.8 cm^3 instead of 13 and left flat discs instead of turns. There is no case
        // where a thread into empty space is wanted, so the geometry wins.
        if let Some(side) = self.cyl_side_of_body(src, c, ax, r, length) {
            spec.internal = side;
        }
        // an ISO nut's groove starts at the wall of the hole as drilled, the standard's profile cut off there;
        // a hole under D1 is cut from D1. Set in the void short of the wall, at D1 of a hole of 8.5, the
        // sweep cut nothing at all past a length of 4.
        let g = spec.geometry_in(Some(2.0 * r));
        let self_r = r; // the radius of the face itself
        let r = if spec.internal && spec.standard == crate::thread::ThreadStandard::MetricIso { r.max(g.minor_d * 0.5) } else { r };
        // Validation before the kernel: bad parameters otherwise produce a silent no-op or
        // crash the sweep.
        let turns = length / g.lead.max(1e-9);
        // A THREAD OF ITS OWN SIZE: the face is the size the thread is - a shaft within 5 % (or half a pitch) of
        // the nominal, a hole drilled from the basic minor diameter less half a pitch up to 5 % over the nominal.
        // Reported behaviour: M10 on a cylinder of 20 stood green, 519 mm^3 cut off it in 76 faces.
        let face_d = 2.0 * self_r;
        let off_size = if spec.internal { face_d < g.minor_d - 0.5 * g.pitch || face_d > g.major_d * 1.05 } else { (face_d - g.major_d).abs() > (0.05 * g.major_d).max(0.5 * g.pitch) };
        let bad = if off_size {
            Some(crate::errors::CoreError::ThreadNotItsSize { face: face_d, nominal: spec.nominal_d })
        } else if !spec.internal && g.depth >= r * 0.95 {
            Some(crate::errors::CoreError::ThreadDepthTooDeep { depth: g.depth, radius: r, dia: spec.nominal_d, pitch: g.pitch })
        } else if g.pitch < 0.05 {
            Some(crate::errors::CoreError::ThreadPitchTooSmall { pitch: g.pitch })
        } else if spec.starts > crate::feature::MAX_HELIX_STARTS {
            Some(crate::errors::CoreError::TooManyStarts { starts: spec.starts, limit: crate::feature::MAX_HELIX_STARTS })
        } else if turns > 400.0 {
            Some(crate::errors::CoreError::ThreadTooManyTurns { turns })
        } else if length <= 1e-6 {
            Some(crate::errors::CoreError::ThreadLengthUnset)
        } else {
            None
        };
        if let Some(e) = bad {
            return Err(e);
        }
        // A THREAD NO LONGER THAN ITS FACE: past the end of the cylinder the sweep runs in the air or into a
        // shoulder. Reported behaviour: a thread of 101 on a cylinder 10 long was taken and went red after the
        // rebuild. The face's length is read off the mesh; a mesh that does not show it refuses nothing here.
        if let Some(face) = self.mesh_index(src).and_then(|mi| crate::geom::cyl_span_from_mesh(&self.bodies[mi].mesh, c, ax, self_r)) {
            if length > face + 1e-3 {
                return Err(crate::errors::CoreError::ThreadLongerThanFace { length, face });
            }
        }
        Ok((g, r))
    }

    /// HOW LONG THE CYLINDER A THREAD IS PUT ON RUNS from its rim, read off the mesh, the axis turned along the face
    /// as the rebuild turns it; `None` when the mesh does not show it.
    pub fn thread_face_span(&self, src: Id, rim: ([f64; 3], [f64; 3], f64)) -> Option<f64> {
        let (c, axis, r) = rim;
        let mi = self.mesh_index(src)?;
        let ax = crate::geom::axis_along_cylinder(&self.bodies[mi].mesh, c, axis, r).unwrap_or(axis);
        crate::geom::cyl_span_from_mesh(&self.bodies[mi].mesh, c, ax, r)
    }

    /// WHAT REFUSES THE THREAD a command would make, for the command to say it beside its field before Enter: the
    /// rim as the command picked it, its axis turned along the face as the rebuild turns it.
    pub fn thread_refusal(&self, src: Id, rim: ([f64; 3], [f64; 3], f64), spec: crate::thread::ThreadSpec, length: f64) -> Option<crate::errors::CoreError> {
        let (c, axis, r) = rim;
        let ax = self.mesh_index(src).and_then(|mi| crate::geom::axis_along_cylinder(&self.bodies[mi].mesh, c, axis, r)).unwrap_or(axis);
        let mut spec = spec;
        self.thread_verdict(src, (c, ax, r), &mut spec, length).err()
    }

    /// Thread: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    pub(crate) fn prep_thread(&mut self, p: &mut Pass, run: HelixRun, spec: crate::thread::ThreadSpec) -> crate::feature::KernelJob {
        let HelixRun { src, edge, length, lead_in, lead_out, body } = run;
        let mut spec = spec;
        // Parametric like every feature: the size, pitch and clearance may be expressions.
        spec.nominal_d = p.dim("nominal", spec.nominal_d);
        spec.pitch = p.dim("pitch", spec.pitch);
        spec.fit = p.dim("fit", spec.fit);
        // The crest and root radii are parametric too; zero means "as the standard says".
        spec.crest_r = {
            let v = p.dim("crest_r", spec.crest_r.unwrap_or(0.0));
            (v > 1e-9).then_some(v)
        };
        spec.root_r = {
            let v = p.dim("root_r", spec.root_r.unwrap_or(0.0));
            (v > 1e-9).then_some(v)
        };
        let length = p.dim("length", length);
        let lead_in = p.dim("lead_in", lead_in);
        let lead_out = p.dim("lead_out", lead_out);

        match self.helical_axis(p.kernel, src, edge) {
            Some((c, ax, r)) => {
                match self.thread_verdict(src, (c, ax, r), &mut spec, length) {
                    Err(e) => crate::feature::KernelJob::refused(e),
                    Ok((g, r)) => {
                        let feature = p.node;
                        // the volume of the source is read BEFORE the parcel leaves: the check below compares
                        // against it, and the document does not travel with the worker
                        let src_v = self.mesh_index(src).map(|i| self.bodies[i].mesh.volume()).unwrap_or(0.0);
                        let profile = crate::thread::encode_edges(&g.groove);
                        let (lead, starts, left, relief) = (g.lead, spec.starts.max(1), spec.left, spec.radial_relief());
                        crate::feature::KernelJob::new(vec![src], move |k| {
                            k.helical(crate::feature::Helical {
                                body,
                                src,
                                origin: c,
                                dir: ax,
                                radius: r,
                                profile: &profile,
                                length,
                                lead,
                                starts,
                                left,
                                fuse: false, // a thread is subtracted
                                lead_in,
                                lead_out,
                                feature,
                                crest_relief: relief,
                            })
                            .and_then(|built| {
                                // The result is checked, not only the input: a thread that removed
                                // nothing is a refusal rather than a success, or a smooth part is
                                // reported as done. The groove side already came from the geometry, so
                                // what is caught here is the rest: too fine a pitch, a degenerate
                                // profile, a miss against the face.
                                let got = src_v - built.mesh.volume();
                                if src_v > 0.0 && got < 1e-6 * src_v {
                                    Err(crate::errors::CoreError::ThreadRemovedNothing { before: src_v, after: built.mesh.volume() })
                                } else {
                                    Ok(built)
                                }
                            })
                        })
                    }
                }
            }
            None => crate::feature::KernelJob::refused(crate::errors::CoreError::ThreadRimNotFound),
        }
    }

    /// Auger: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    pub(crate) fn regen_auger(&mut self, p: &mut Pass, run: HelixRun, spec: crate::thread::AugerSpec) -> bool {
        let HelixRun { src, edge, length, lead_in, lead_out, body } = run;
        let mut spec = spec;
        spec.outer_d = p.dim("outer", spec.outer_d);
        spec.pitch = p.dim("pitch", spec.pitch);
        spec.thickness = p.dim("thickness", spec.thickness);
        spec.edge_r = p.dim("edge_r", spec.edge_r);
        let length = p.dim("length", length);
        let lead_in = p.dim("lead_in", lead_in);
        let lead_out = p.dim("lead_out", lead_out);
        let res = match self.helical_axis(p.kernel, src, edge) {
            // the starts are checked before the flight faces are named: the names are reserved per start
            Some(_) if spec.starts > crate::feature::MAX_HELIX_STARTS => Err(crate::errors::CoreError::TooManyStarts { starts: spec.starts, limit: crate::feature::MAX_HELIX_STARTS }),
            Some((c, ax, r)) => {
                // the shaft diameter is taken from the geometry, so it stays associative
                spec.shaft_d = r * 2.0;
                if spec.flight_height() <= 1e-6 {
                    Err(crate::errors::CoreError::AugerOuterNotBigger { outer: spec.outer_d, shaft: spec.shaft_d })
                } else if spec.pitch <= 1e-6 || length <= 1e-6 {
                    Err(crate::errors::CoreError::AugerBadPitchOrLength)
                } else {
                    p.kernel
                        .helical(crate::feature::Helical {
                            body,
                            src,
                            origin: c,
                            dir: ax,
                            radius: r,
                            profile: &crate::thread::encode_edges(&spec.flight_profile()),
                            length,
                            lead: spec.lead(),
                            starts: spec.starts.max(1),
                            left: spec.left,
                            fuse: true, // an auger flight is welded on
                            lead_in,
                            lead_out,
                            feature: p.node,
                            crest_relief: 0.0,
                        })
                        .and_then(|built| {
                            // An auger flight is welded on, so the volume has to grow. If it did
                            // not, the flight landed beside the shaft, and reporting success would
                            // mean a smooth shaft.
                            let src_v = self.mesh_index(src).map(|i| self.bodies[i].mesh.volume()).unwrap_or(0.0);
                            if src_v > 0.0 && built.mesh.volume() <= src_v * 1.001 {
                                Err(crate::errors::CoreError::AugerAddedNothing { before: src_v, after: built.mesh.volume() })
                            } else {
                                Ok(built)
                            }
                        })
                }
            }
            None => Err(crate::errors::CoreError::AugerRimNotFound),
        };
        self.apply_regen(p.landing(), body, res)
    }

    /// Hole: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    pub(crate) fn prep_hole(&mut self, p: &mut Pass, src: Id, at: HoleSite, tool: HoleTool, body: Id) -> crate::feature::KernelJob {
        let HoleSite { face, point, normal, sketch, flip } = at;
        // THE PARAMETRIC OVERRIDES ARE APPLIED HERE, and the tool is rebuilt from them. Handing the
        // stored `tool` to the kernel would drop every expression driving a hole's diameter or depth -
        // silently, because the numbers are of the right type either way.
        let tool =
            HoleTool { kind: tool.kind, diameter: p.dim("diameter", tool.diameter), depth: p.dim("depth", tool.depth).abs(), dia2: p.dim("dia2", tool.dia2), depth2: p.dim("depth2", tool.depth2) };
        let job = if sketch != 0 {
            // At the isolated points of a sketch: one frame per point, with every cut applied by a
            // single boolean.
            let pls = self.sketch_hole_points(sketch, flip);
            if pls.is_empty() {
                crate::feature::KernelJob::refused(crate::errors::CoreError::NoIsolatedPointsForHoles)
            } else {
                // The wall of each hole is named after the sketch point that placed it, so adding a
                // point does not rename the neighbouring holes.
                let pts = self.sketch_hole_point_ids(sketch);
                let sites: Vec<Id> = (0..pls.len()).map(|i| pts.get(i).copied().unwrap_or(i as Id + 1)).collect();
                let feature = p.node;
                crate::feature::KernelJob::new(vec![src], move |k| k.holes(body, src, tool, &pls, &sites, feature))
            }
        } else {
            // The face is found by recipe through the query, and the hole travels with it.
            //
            // A refusal stops the operation rather than merely marking the node. Recording an error
            // and drilling at the stale point anyway lets the success immediately clear the mark,
            // which is exactly the silent guessing queries were introduced to remove. Found by an
            // end-to-end run against the real kernel.
            let picked = !matches!(face.query, crate::refs::Query::Id(0)); // The sketch-driven form
                                                                           // picks no face.
            match self.face_by_ref(p.node, src, &face, "ref-what-hole-face") {
                Err(_) if picked => crate::feature::KernelJob::refused(crate::errors::CoreError::FaceNotFound),
                resolved => {
                    // THE HOLE STANDS WHERE IT WAS PLACED: the stored point brought onto the face as it stands now
                    // (along its normal), not the face's centre - two holes in one face stood in each other, the
                    // second green and drilling nothing. A hole placed before holds the centre it was given then.
                    let (point, normal) = match resolved {
                        Ok(c) => {
                            let off = (0..3).map(|i| (point[i] - c.centroid[i]) * c.normal[i]).sum::<f64>();
                            ([point[0] - off * c.normal[0], point[1] - off * c.normal[1], point[2] - off * c.normal[2]], c.normal)
                        }
                        Err(_) => (point, normal),
                    };
                    // The tool (a cylinder plus a counterbore or countersink) is placed at the face
                    // point along its normal and cuts inwards.
                    let pl = crate::feature::PlaneFrame::from_origin_normal(point, normal, 0.0).matrix12();
                    let feature = p.node;
                    crate::feature::KernelJob::new(vec![src], move |k| k.hole(body, src, tool, pl, feature))
                }
            }
        };
        job
    }

    /// A CUT OR AN INTERSECTION THAT PARTED BODY `body` IN SEVERAL SOLIDS makes each solid a body of the node's part, as
    /// splitting a body does. Every piece the node had keeps its body: it takes the free solid nearest the point it was
    /// at, the node's own body first. A piece with no solid left loses its body (what stood on it turns red); a solid
    /// with no piece gets a new body, the biggest first. Returns whether every piece landed.
    pub(crate) fn part_the_pieces(
        &mut self,
        ti: usize,
        body: Id,
        kernel: &dyn crate::feature::Kernel,
        dirty: &mut std::collections::HashSet<Id>,
        report: &mut crate::feature::RegenReport,
        emap: &mut EdgeRenames,
    ) -> bool {
        let node = self.timeline[ti].id;
        // only a body that was one piece is parted: the copies of a pattern are pieces of one body before any cut, and
        // a rod through the row leaves them so; a body that is to stay whole counts as one solid, and the bodies of the
        // pieces it had go
        let parts = self.timeline[ti].kind.consumed_body().is_none_or(|s| kernel.body_pieces(s) <= 1);
        let solids = if parts { kernel.body_solids(body) } else { Vec::new() };
        let had: Vec<crate::feature::CutPiece> = self.timeline[ti].kind.cut_pieces().to_vec();
        let dist = |a: [f64; 3], b: [f64; 3]| (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f64>();
        let mut free: Vec<usize> = (0..solids.len()).collect();
        let mut plan: Vec<(Id, usize)> = Vec::new();
        if solids.len() >= 2 {
            let biggest = free.iter().copied().max_by(|&x, &y| solids[x].1.total_cmp(&solids[y].1)).unwrap_or(0);
            let asked: Vec<(Id, [f64; 3])> = if had.is_empty() { vec![(body, solids[biggest].0)] } else { had.iter().map(|p| (p.body, p.at)).collect() };
            for (id, at) in asked {
                let Some(pos) = (0..free.len()).min_by(|&x, &y| dist(solids[free[x]].0, at).total_cmp(&dist(solids[free[y]].0, at))) else { break };
                plan.push((id, free.remove(pos)));
            }
            free.sort_by(|&x, &y| solids[y].1.total_cmp(&solids[x].1));
            for k in free {
                plan.push((self.alloc_id(), k));
            }
        }
        // the bodies of pieces that have no solid any more go, and with them their faces
        for p in had.iter().filter(|p| p.body != body && !plan.iter().any(|(id, _)| *id == p.body)) {
            if let Some(mi) = self.mesh_index(p.body) {
                self.remove_mesh(mi);
            }
            self.regen_faces.remove(&p.body);
            dirty.insert(p.body);
        }
        let mut clear = true;
        // the other pieces are taken out of the whole first, and the node's own body keeps its piece last
        for &(id, k) in plan.iter().skip(1).chain(plan.first()) {
            let res = kernel.take_solid(id, body, k);
            clear &= self.apply_regen(Landing { node, dirty: &mut *dirty, report: &mut *report, kernel, emap: &mut *emap }, id, res);
        }
        if let Some(v) = self.timeline[ti].kind.cut_pieces_mut() {
            *v = plan.iter().map(|&(id, k)| crate::feature::CutPiece { body: id, at: solids[k].0 }).collect();
        }
        clear
    }

    /// SplitBody: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    pub(crate) fn regen_splitbody(&mut self, p: &mut Pass, src: Id, at: Option<([f64; 3], [f64; 3])>, offset: f64, bodies: &[Id]) -> bool {
        // The cutting plane is a reference (as for a mirror): a world plane, a datum or a face (see `op_plane`), resolved
        // here, in timeline order, so a face that moved carries the split with it; otherwise a split would be a one-off
        // cut against forgotten coordinates.
        // A deleted plane means no cut. Falling back to a world plane is not acceptable here: a
        // mirror about another plane is still a mirror, while a split along another plane breaks the
        // body in a different place, silently. A red node beats a quietly different part.
        let lost = at.is_none();
        let (o0, n) = at.unwrap_or(([0.0; 3], [0.0, 0.0, 1.0]));
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        let res = if lost {
            Err(crate::errors::CoreError::CutPlaneDeleted)
        } else if len < 1e-9 {
            Err(crate::errors::CoreError::ZeroNormal)
        } else {
            // The offset along the normal is parametric (the `offset` feature dimension), so the
            // split moves by formula and follows a global parameter like an extrude dimension.
            let d = eval_dim(p.dims, "offset", offset, p.vars);
            let u = [n[0] / len, n[1] / len, n[2] / len];
            let o = [o0[0] + u[0] * d, o0[1] + u[1] * d, o0[2] + u[2] * d];
            p.kernel.split_body(bodies, src, o, u, p.node)
        };
        // A split writes SEVERAL bodies, so the answer is the conjunction: the node is clean only if every
        // piece of it landed.
        let mut clear = true;
        match res {
            Ok(parts) => {
                // The number of pieces is fixed at creation. Moving the plane so the body divides
                // differently is not a reason to lose a piece silently or to leave a ghost body from
                // the previous split: the node reports an honest error and the bodies stay as they
                // were.
                if parts.len() != bodies.len() {
                    let err = crate::errors::CoreError::SplitPieceCount { got: parts.len(), want: bodies.len() };
                    self.regen_errors.insert(p.node, err.clone());
                    p.report.errors.push((p.node, err));
                    clear = false;
                } else {
                    for (b, part) in bodies.iter().zip(parts) {
                        clear &= self.apply_regen(p.landing(), *b, Ok(part));
                    }
                }
            }
            Err(e) => {
                for &b in bodies {
                    clear &= self.apply_regen(p.landing(), b, Err(e.clone()));
                }
            }
        }
        clear
    }

    /// LinearArray: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    pub(crate) fn prep_lineararray(&mut self, p: &mut Pass, src: Id, axes: [ArrayAxis; 3], body: Id) -> crate::feature::KernelJob {
        // the counts are dimensions of the node as the steps are: a count typed as `k` follows `k`
        let mut axes = axes;
        for (a, key) in axes.iter_mut().zip(["count", "count2", "count3"]) {
            a.count = p.dim(key, a.count as f64).round().max(1.0) as u32;
        }
        // the product is checked before anything is reserved: past the limit the pattern is refused, not built
        let counts = [axes[0].count, axes[1].count, axes[2].count];
        let Some(total) = crate::feature::pattern_instances(&counts) else {
            return crate::feature::KernelJob::refused(crate::feature::pattern_too_large(&counts));
        };
        // a pattern of one copy is the body alone: it stood green and changed nothing
        if total < 2 {
            return crate::feature::KernelJob::refused(crate::errors::CoreError::ArrayOfOne);
        }
        // Parametric: the step lives as an expression per vector component, so global parameters move
        // the pattern. An empty expression keeps the stored number. The keys are the field names in the
        // document ("dx", "dx2", "dx3"), so the suffix is the axis number - the first carries none.
        let step: Vec<[f64; 3]> = axes
            .iter()
            .enumerate()
            .map(|(n, a)| {
                let sfx = if n == 0 { String::new() } else { (n + 1).to_string() };
                [p.dim(&format!("dx{sfx}"), a.d[0]), p.dim(&format!("dy{sfx}"), a.d[1]), p.dim(&format!("dz{sfx}"), a.d[2])]
            })
            .collect();
        // A 3D grid: direction one (i*d1) by two (j*d2) by three (k*d3). A count of one or less in
        // the second or third direction reduces the dimensionality.
        let (c1, c2, c3) = (axes[0].count.max(1), axes[1].count.max(1), axes[2].count.max(1));
        let mut ts: Vec<[f64; 12]> = Vec::with_capacity(total as usize);
        for i in 0..c1 {
            for j in 0..c2 {
                for k in 0..c3 {
                    let n = [i as f64, j as f64, k as f64];
                    let at = |c: usize| n[0] * step[0][c] + n[1] * step[1][c] + n[2] * step[2][c];
                    ts.push(translate_mat(at(0), at(1), at(2)));
                }
            }
        }
        let feature = p.node;
        crate::feature::KernelJob::new(vec![src], move |k| k.pattern(body, src, &ts, feature))
    }

    /// CircularArray: one branch of the timeline rebuild. Returns whether the node's error record may
    /// be cleared (see `apply_regen`).
    pub(crate) fn prep_circulararray(&mut self, p: &mut Pass, src: Id, count: u32, angle: f64, axis: Id, body: Id) -> crate::feature::KernelJob {
        // Parametric: the angle is an expression over global parameters. An empty expression keeps
        // the stored number.
        let angle = p.dim("angle", angle);
        // the count is a dimension of the node as the angle is: a count typed as `k` follows `k`
        let count = p.dim("count", count as f64).round().max(1.0) as u32;
        if crate::feature::pattern_instances(&[count]).is_none() {
            return crate::feature::KernelJob::refused(crate::feature::pattern_too_large(&[count]));
        }
        if count < 2 {
            return crate::feature::KernelJob::refused(crate::errors::CoreError::ArrayOfOne);
        }
        let c = count.max(1);
        let step = if angle.abs() >= 359.9 { 360.0 / c as f64 } else { angle / c as f64 };
        // Rotation axis: world Z when `axis` is zero, otherwise a datum axis (origin and direction,
        // resolved earlier in this pass).
        let (org, dir) = if axis != 0 {
            self.datum_axes.iter().find(|d| d.id == axis).map(|d| (d.origin(), d.dir())).unwrap_or(([0.0; 3], [0.0, 0.0, 1.0]))
        } else {
            ([0.0; 3], [0.0, 0.0, 1.0])
        };
        let ts: Vec<[f64; 12]> = (0..c).map(|i| rot_about_axis(org, dir, i as f64 * step)).collect();
        let feature = p.node;
        crate::feature::KernelJob::new(vec![src], move |k| k.pattern(body, src, &ts, feature))
    }

    /// WHAT CAN BE COMPUTED RIGHT NOW, starting from node `from`.
    ///
    /// A node joins the list while nothing it needs is made by a node standing between here and it. That is a
    /// stricter question than "which nodes are in one wave": a wave says they do not depend on ONE ANOTHER,
    /// while this says they do not depend on anything not yet written into the document. Preparing a node
    /// whose source is computed but not yet applied resolves its references against geometry that is not
    /// there - measured on the scenario document as 13 refusals and half the bodies missing.
    ///
    /// `held` are the nodes already computed and waiting to be written down: they must not be prepared a
    /// second time (a preparation mints the names of the faces to come), and what they make is not current
    /// until the rebuild reaches them.
    ///
    /// `walked` are the places this pass has already been through. A batch is written down as soon as it is
    /// computed, ahead of the walk, so the next batch starts ABOVE nodes that are finished - and each of them
    /// still looks as if it needed rebuilding, its input being dirty. Offered again, they were rebuilt once per
    /// batch: 74 times each on the scenario document. What they made is current, so they hold nothing back.
    ///
    /// Except `from` itself: that is the place the walk is standing on, not one it has been through. Skipping it
    /// left its body out of what is still to be built, the node standing on it was prepared on the body from
    /// before the edit, and a thread on the scenario document could no longer find its rim.
    pub(crate) fn ready_from(&self, from: usize, dirty: &std::collections::HashSet<Id>, held: &std::collections::HashSet<Id>, walked: &std::collections::HashSet<usize>, limit: usize) -> Vec<usize> {
        let mut ahead: std::collections::HashSet<Id> = std::collections::HashSet::new();
        let mut ready: Vec<usize> = Vec::new();
        for j in from..self.timeline.len().min(limit) {
            if j != from && walked.contains(&j) {
                continue;
            }
            let mine = &self.timeline[j];
            let behind = self.node_reads_any(j, &ahead);
            if mine.suppressed || !self.node_needs(j, dirty) {
                // CLEAN FOR NOW IS NOT CLEAN. Its source is still to be rebuilt, so it will be rebuilt too when the
                // walk reaches it (a suppressed modifier passes the new source through), and what it makes is not
                // current either. Skipping it silently offered the node standing on it a body from before the edit.
                if behind {
                    ahead.extend(mine.kind.declares());
                }
                continue;
            }
            let waiting = held.contains(&mine.id) || behind;
            if !waiting {
                ready.push(j);
            }
            // whatever this node makes is not current until it has been rebuilt
            for d in mine.kind.declares() {
                ahead.insert(d);
            }
        }
        ready
    }

    /// WHAT WOULD BE COMPUTED SIDE BY SIDE from node `from`, with the document fully dirty: the rule above,
    /// asked from outside. A check of the rule is worth more than a check of the timing it buys.
    pub fn ready_batch_from(&self, from: usize) -> Vec<Id> {
        let dirty = std::collections::HashSet::new();
        let held = std::collections::HashSet::new();
        let walked = std::collections::HashSet::new();
        self.ready_from(from, &dirty, &held, &walked, self.rollback.unwrap_or(usize::MAX)).into_iter().filter_map(|j| self.timeline.get(j).map(|n| n.id)).collect()
    }

    /// THE ONE DOOR TO A NODE'S PARCEL: what has to be built for this node, and what it needs to build it.
    ///
    /// `None` means the node is one of the five that are computed in place: an import re-tessellates a ready
    /// shape, a body split returns several bodies at once, a part instance and a part mirror follow the active
    /// body of their source at this point of the timeline, and an auger checks the volume it removed against the
    /// document.
    ///
    /// Preparing a parcel CHANGES THE DOCUMENT (it mints the names of the faces to come), so this is called
    /// from the thread that owns it - once per node, in timeline order, exactly as before.
    pub(crate) fn prep_node(&mut self, p: &mut Pass, kind: &FeatureKind) -> Option<(Id, crate::feature::KernelJob)> {
        let job = match kind {
            FeatureKind::Extrude { sketch, profiles, height, reach, down, fill, body } => {
                (*body, self.prep_extrude(p, Profile { sketch: *sketch, profiles }, ExtrudeSpan { height: *height, down: *down, reach: *reach, fill }, *body))
            }
            FeatureKind::Revolve { sketch, profiles, axis, angle, axis_datum, axis_line, reach, src, op, body, .. } => {
                let prof = Profile { sketch: *sketch, profiles };
                let ax = RevolveAxis { axis: *axis, datum: *axis_datum, line: *axis_line };
                (*body, self.prep_revolve(p, prof, ax, *angle, *reach, BodyOp { src: *src, op: *op, body: *body }))
            }
            FeatureKind::Sweep { sketch, profiles, path_sketch, path, src, op, body, .. } => {
                (*body, self.prep_sweep(p, Profile { sketch: *sketch, profiles }, *path_sketch, *path, BodyOp { src: *src, op: *op, body: *body }))
            }
            FeatureKind::Loft { sketches, contours, ruled, src, op, surface, body, .. } => {
                (*body, self.prep_loft(p, sketches.clone(), contours.clone(), *ruled, *surface, BodyOp { src: *src, op: *op, body: *body }))
            }
            FeatureKind::Box3 { dx, dy, dz, body } => (*body, self.prep_box3(p, *dx, *dy, *dz, *body)),
            FeatureKind::Cylinder { r, h, body } => (*body, self.prep_cylinder(p, *r, *h, *body)),
            FeatureKind::Sphere { r, body } => (*body, self.prep_sphere(p, *r, *body)),
            FeatureKind::Cone { r1, r2, h, body } => (*body, self.prep_cone(p, *r1, *r2, *h, *body)),
            FeatureKind::Torus { major, minor, body } => (*body, self.prep_torus(p, *major, *minor, *body)),
            FeatureKind::Prism { r, n, h, body } => (*body, self.prep_prism(p, *r, *n, *h, *body)),
            FeatureKind::Combine { src, sketch, profiles, height, op, extent, down, fill, body, .. } => {
                let prof = Profile { sketch: *sketch, profiles };
                let span = CombineSpan { height: *height, down: *down, extent: *extent, fill };
                (*body, self.prep_combine(p, prof, span, BodyOp { src: *src, op: *op, body: *body }))
            }
            FeatureKind::Fillet { src, radius, edges, at_vertices, body } => (*body, self.prep_fillet(p, *src, *radius, edges, at_vertices, *body)),
            FeatureKind::Chamfer { src, dist, edges, mode, d2, flip, ref_face, body } => {
                (*body, self.prep_chamfer(p, *src, *dist, edges, ChamferShape { mode: *mode, d2: *d2, flip: *flip, ref_face: *ref_face }, *body))
            }
            FeatureKind::Shell { src, thickness, faces, side, body } => (*body, self.prep_shell(p, *src, *thickness, faces, *side, *body)),
            FeatureKind::RemoveFace { src, faces, body } => (*body, self.prep_removeface(p, *src, faces, *body)),
            FeatureKind::PushFace { src, face, dist, body } => (*body, self.prep_pushface(p, *src, face, *dist, *body)),
            FeatureKind::Patch { src, edges, tangent, body } => (*body, self.prep_patch(p, *src, edges, *tangent, *body)),
            FeatureKind::SurfaceReplace { src, faces, surface, body } => (*body, self.prep_surfacereplace(p, *src, faces, *surface, *body)),
            FeatureKind::FaceCopy { src, faces, body } => (*body, self.prep_facecopy(p, *src, faces, *body)),
            FeatureKind::OffsetSurface { src, faces, dist, body } => (*body, self.prep_offset_surface(p, *src, faces, *dist, *body)),
            FeatureKind::Trim { src, tool, keep, body } => (*body, self.prep_trim(*src, *tool, *keep, *body)),
            FeatureKind::Stitch { parts, tol, body } => (*body, self.prep_stitch(p, parts, *tol, *body)),
            FeatureKind::MeshSolid { src, body, simplify } => (*body, self.prep_mesh_solid(*src, *body, *simplify)),
            FeatureKind::MeshRecognised { src, body, tol, sharp, simplify } => (*body, self.prep_mesh_recognised(*src, *body, *tol, *sharp, *simplify)),
            FeatureKind::Thicken { src, face, thickness, join, body } => (*body, self.prep_thicken(p, *src, *face, *thickness, *join, *body)),
            FeatureKind::SplitFace { src, plane, datum, offset, body, face } => (*body, self.prep_splitface(p, *src, self.op_plane(*plane, *datum, *face), *offset, *body)),
            FeatureKind::Draft { src, faces, neutral, angle, flip, body } => (*body, self.prep_draft(p, *src, faces, DraftShape { neutral: neutral.clone(), angle: *angle, flip: *flip }, *body)),
            FeatureKind::LinearArray { src, dx, dy, dz, count, dx2, dy2, dz2, count2, dx3, dy3, dz3, count3, body } => {
                let axes = [ArrayAxis { d: [*dx, *dy, *dz], count: *count }, ArrayAxis { d: [*dx2, *dy2, *dz2], count: *count2 }, ArrayAxis { d: [*dx3, *dy3, *dz3], count: *count3 }];
                (*body, self.prep_lineararray(p, *src, axes, *body))
            }
            FeatureKind::CircularArray { src, count, angle, axis, body } => (*body, self.prep_circulararray(p, *src, *count, *angle, *axis, *body)),
            FeatureKind::Mirror { src, plane, keep, datum, body, face } => {
                (*body, self.prep_mirror(p, *src, (*datum == 0 && face.is_none()).then_some(*plane), *keep, self.op_plane(*plane, *datum, *face), *body))
            }
            FeatureKind::Move { src, mat, body } => (*body, self.prep_move(*src, *mat, *body)),
            FeatureKind::Piece { src, body } => {
                let (src, body) = (*src, *body);
                (body, crate::feature::KernelJob::new(vec![src], move |k| k.transform_body(body, src, crate::feature::PLACE_IDENTITY)))
            }
            FeatureKind::BodyBoolean { a, b, op, body, .. } => (*body, self.prep_bodyboolean(*a, *b, *op, *body)),
            FeatureKind::Hole { src, face, point, normal, diameter, depth, kind, dia2, depth2, sketch, flip, body, .. } => {
                let at = HoleSite { face: face.clone(), point: *point, normal: *normal, sketch: *sketch, flip: *flip };
                let tool = HoleTool { kind: *kind, diameter: *diameter, depth: *depth, dia2: *dia2, depth2: *depth2 };
                (*body, self.prep_hole(p, *src, at, tool, *body))
            }
            FeatureKind::Thread { src, edge, spec, length, lead_in, lead_out, body } => {
                let run = HelixRun { src: *src, edge: *edge, length: *length, lead_in: *lead_in, lead_out: *lead_out, body: *body };
                (*body, self.prep_thread(p, run, *spec))
            }
            // BUILT IN PLACE by the walk of the timeline (`None`, see above), or nothing to build: a sketch, a datum, a
            // piece of a mesh whose mesh is its geometry. Named one by one, not as "everything else": a kind of node
            // added to `FeatureKind` does not build until it is told here how it builds - under "everything else" it
            // built nothing and said nothing (found by laying a made-up kind of node).
            FeatureKind::Sketch { .. }
            | FeatureKind::Plane { .. }
            | FeatureKind::DatumPoint { .. }
            | FeatureKind::DatumAxis { .. }
            | FeatureKind::SplitBody { .. }
            | FeatureKind::Import { .. }
            | FeatureKind::PartInstance { .. }
            | FeatureKind::ComponentPattern { .. }
            | FeatureKind::MirrorPart { .. }
            | FeatureKind::Auger { .. }
            | FeatureKind::MeshPiece { .. } => return None,
        };
        Some(job)
    }
}
