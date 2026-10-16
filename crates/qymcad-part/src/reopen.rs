//! A FEATURE REOPENED: its command taken again with its values, picks and options.

use qymcad_ui_state::Notice;
use super::*;

pub fn start_feat_cmd_edit(pc: &mut qymcad_ui_state::PartCtx, fid: Id) {
    use qymcad_core::feature::FeatureKind;
    let Some(node) = pc.project.timeline.iter().find(|n| n.id == fid).cloned() else { return };
    // EDITING AN EXISTING FEATURE is the same command, only with its parameters restored. It opens
    // through the same life cycle (`open` on every branch below) rather than by assigning a field:
    // otherwise the remains of the previous command seep into the edit. The view from BEFORE the edit is
    // remembered here - `open` carries it.
    let was_3d = *pc.mode_3d;
    pc.boolean.edit = None; // close the previous boolean edit mode (the arm below turns it on again)
    pc.boolean.pick = None;
    pc.gsel.profiles.clear();
    pc.gsel.edges.clear();
    match node.kind {
        FeatureKind::Extrude { sketch, ref profiles, height, reach, down, ref fill, .. } => {
            pc.cmd.open(pc.armed, 1, was_3d);
            pc.feat.op = 0;
            pc.feat.flip = reach == qymcad_core::feature::Reach::Backward; // restore the direction
            pc.cmd.sketch = pc.project.sketch_index(sketch);
            // ALL contours of the operation sit in the node itself (`profiles`), with no walking of the
            // chain. Editing means all of the contours.
            for &p in profiles {
                if p != 0 {
                    pc.gsel.profiles.insert(p);
                }
            }
            for &f in fill {
                pc.gsel.profiles.insert(f); // the filled inner ones are picked too (otherwise only one circle remains)
            }
            pc.cmd.extent = qymcad_ui_state::ExtentMode::from_extent(reach, down, false);
            if down.abs() > 1e-9 {
                pc.cmd.down = down;
            }
            pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-length", "height", height, 0.1, 10000.0)];
        }
        FeatureKind::Combine { sketch, ref profiles, height, op, extent, down, ref fill, .. } => {
            pc.cmd.open(pc.armed, 1, was_3d);
            pc.feat.op = match op {
                1 => 1,
                2 => 3,
                _ => 2,
            };
            pc.feat.flip = extent.reach == qymcad_core::feature::Reach::Backward; // restore the direction of the tool
            pc.cmd.sketch = pc.project.sketch_index(sketch);
            // ALL contours of the operation sit in the node itself (`profiles`), with no walking of the
            // chain. Editing means all of the contours.
            for &p in profiles {
                if p != 0 {
                    pc.gsel.profiles.insert(p);
                }
            }
            for &f in fill {
                pc.gsel.profiles.insert(f); // the filled inner ones are picked too
            }
            // restore the extent mode: through, symmetric, two-sided or to a length
            pc.cmd.extent = qymcad_ui_state::ExtentMode::from_extent(extent.reach, down, extent.through);
            if down.abs() > 1e-9 {
                pc.cmd.down = down;
            }
            // `sync_dir_cmd_params` adds the second-side field at the geometry, with the stored expression
            pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-depth", "height", height.abs(), 0.1, 10000.0)];
        }
        FeatureKind::Revolve { sketch, ref profiles, axis, angle, axis_datum, axis_line, reach, op, .. } => {
            pc.cmd.open(pc.armed, 3, was_3d);
            use qymcad_core::feature::Reach;
            pc.cmd.extent = if reach == Reach::BothWays { qymcad_ui_state::ExtentMode::Symmetric } else { qymcad_ui_state::ExtentMode::Length };
            pc.feat.flip = reach == Reach::Backward;
            pc.rev.axis = axis;
            pc.rev.axis_datum = axis_datum;
            pc.rev.axis_line = axis_line;
            pc.rev.pick_axis = false;
            pc.cmd.sketch = pc.project.sketch_index(sketch);
            // ALL contours of the node go into the selection: editing a multi-contour feature must open it
            // whole, otherwise Apply quietly reduces two areas to one.
            for c in profiles {
                pc.gsel.profiles.insert(*c);
            }
            // the core op (0 = cut, 1 = pad, 2 = intersect) maps to the bar switch (0 = add, 2 = cut,
            // 3 = intersect)
            pc.feat.op = match op {
                0 => 2,
                2 => 3,
                _ => 0,
            };
            pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-angle", "angle", angle, 1.0, 360.0)];
        }
        FeatureKind::Sweep { sketch, ref profiles, path_sketch, path, op, .. } => {
            // reopen the sweep: restore the profile, the path and the picked contours, with no picking
            pc.cmd.open(pc.armed, 8, was_3d);
            pc.cmd.sketch = pc.project.sketch_index(sketch);
            pc.sweep.prof_sid = sketch;
            pc.sweep.path_sid = path_sketch;
            pc.sweep.prof_cid = profiles.first().copied().unwrap_or(0);
            for c in profiles {
                pc.gsel.profiles.insert(*c);
            }
            pc.feat.op = match op {
                0 => 2,
                2 => 3,
                _ => 0,
            };
            pc.sweep.path_cid = path;
            pc.sweep.pick_path = false;
            pc.cmd.params.clear();
        }
        FeatureKind::Loft { sketches, contours, ruled, src, op, .. } => {
            // reopen the loft: restore the set of sections and the picked contours, with no picking
            pc.cmd.open(pc.armed, 9, was_3d);
            pc.cmd.sketch = sketches.first().and_then(|&s| pc.project.sketch_index(s));
            pc.loft.sids = sketches.clone();
            pc.loft.cids = (0..sketches.len()).map(|i| contours.get(i).copied().unwrap_or(0)).collect();
            pc.loft.ruled = ruled;
            pc.loft.result = if src == 0 { 0 } else { op + 1 }; // 0 = a new body; otherwise cut, union or intersect
            pc.loft.pick = false;
            pc.loft.pick_last = None;
            pc.cmd.params.clear();
        }
        FeatureKind::Fillet { src, radius, ref edges, ref at_vertices, .. } => {
            pc.cmd.open(pc.armed, 4, was_3d);
            qymcad_ui_state::select_body(&mut *pc.project, &mut *pc.sel, &mut *pc.view, src);
            refresh_edges(pc); // pull up the edges of the body (this clears the selection), then restore it
            restore_edge_picks(pc, src, edges);
            pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-radius", "radius", radius, 0.05, 1000.0)];
            // THE TABLE OF VERTICES - one field per vertex, each at its own place. The reference is
            // resolved against the live body: the name of a vertex is derived from its edges and survives
            // edits to its neighbours.
            let table: Vec<(qymcad_core::refs::Ref, f64)> = at_vertices.clone();
            for (r, val) in table {
                let Ok(found) = pc.project.resolve_vertex_refs(src, &r, "ref-what-fillet-vertex") else { continue };
                let Some(desc) = found.first().copied() else { continue };
                let Some(p) = pc.project.vertex_point(src, desc) else { continue };
                let key = format!("at{desc}");
                let prm = cmd_param_from(&*pc.project, fid, "f-radius-at-vertex", &key, val, 0.0, 1000.0);
                pc.cmd.params.push(prm.at(p));
            }
        }
        FeatureKind::Chamfer { src, dist, ref edges, mode, d2, flip, ref_face, .. } => {
            pc.cmd.open(pc.armed, 5, was_3d);
            qymcad_ui_state::select_body(&mut *pc.project, &mut *pc.sel, &mut *pc.view, src);
            refresh_edges(pc);
            restore_edge_picks(pc, src, edges);
            pc.chamfer.mode = mode; // restore the mode, the side and the second parameter
            pc.chamfer.flip = flip;
            pc.chamfer.ref_face = ref_face; // restore the hand-picked reference face
            pc.chamfer.pick_ref = false;
            use qymcad_core::feature::ChamferMode;
            let d2_def = if d2 > 0.0 {
                d2
            } else if mode == ChamferMode::DistAngle {
                45.0
            } else {
                1.5
            };
            pc.cmd.params = vec![
                cmd_param_from(&*pc.project, fid, "f-leg", "dist", dist, 0.05, 1000.0),
                cmd_param_from(&*pc.project, fid, qymcad_ui_state::chamfer_d2_label(mode), "d2", d2_def, 0.0, 1000.0),
            ];
        }
        FeatureKind::Shell { src, thickness, ref faces, side, .. } => {
            pc.cmd.open(pc.armed, 6, was_3d);
            qymcad_ui_state::select_body(&mut *pc.project, &mut *pc.sel, &mut *pc.view, src);
            // restore the MULTIPLE selection of open faces by their PERSISTENT ids - the highlight follows
            // the face selection
            pc.gsel.faces = live_picks(&*pc.project, src, faces, true);
            pc.gsel.faces_body = Some(src); // the faces belong to the body of the feature (that is the highlight scope)
            pc.opts.shell_side = side; // which way the wall goes
            pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-thickness", "thickness", thickness, 0.1, 1000.0)];
        }
        FeatureKind::Thicken { face, thickness, .. } => {
            pc.cmd.open(pc.armed, 28, was_3d);
            qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, &mut *pc.view_restore);
            *pc.mode_3d = true;
            pc.gsel.faces.clear();
            pc.gsel.faces.insert(face);
            pc.gsel.faces_body = node.parent.and_then(|_| {
                qymcad_ui_state::op_target_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: &*pc.set, scheme: pc.scheme, project: &*pc.project, active_path: pc.active_path }, *pc.sel)
            });
            pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-thickness", "thickness", thickness, -100000.0, 100000.0)];
            *pc.status = Notice::info(qymcad_i18n::tr("msg-edit-thicken"));
        }
        // A RECOGNISED BODY reopens the recognition tool on its own mesh, with its tolerance and sharp edge in the fields;
        // a polyhedron has no number to change, and the tool opens on it all the same rather than sending a person to
        // the right-hand panel
        FeatureKind::MeshRecognised { src, tol, sharp, .. } => {
            open_recognise_on(pc, src, was_3d, 0);
            pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-recognise-tol", "tol", tol, 0.1, 1000.0), cmd_param_from(&*pc.project, fid, "f-recognise-sharp", "sharp", sharp, 5.0, 90.0)];
        }
        FeatureKind::MeshSolid { src, .. } => open_recognise_on(pc, src, was_3d, 1),
        FeatureKind::SplitFace { plane, datum, offset, face, .. } => {
            pc.cmd.open(pc.armed, 29, was_3d);
            qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, &mut *pc.view_restore);
            *pc.mode_3d = true;
            pc.split.plane = Some(qymcad_ui_state::op_plane_pick(plane, datum, face));
            pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-offset", "offset", offset, -100000.0, 100000.0)];
            *pc.status = Notice::info(qymcad_i18n::tr("msg-edit-split-face"));
        }
        FeatureKind::SplitBody { plane, datum, offset, face, .. } => {
            // EDITING A CUT: restore the reference to the plane and the offset, exactly as at creation, so
            // that pressing Enter again does not recreate the feature from scratch and lose the link to the
            // datum.
            pc.cmd.open(pc.armed, 27, was_3d);
            qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, &mut *pc.view_restore);
            *pc.mode_3d = true;
            pc.split.plane = Some(qymcad_ui_state::op_plane_pick(plane, datum, face));
            pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-offset", "offset", offset, -100000.0, 100000.0)];
            *pc.status = Notice::info(qymcad_i18n::tr("msg-edit-split-body"));
        }
        // THE PATCH REOPENS on its edges and its kind, the bar showing both; Enter puts the edges picked back. Reported
        // behaviour: a double click on the patch in the tree opened nothing.
        FeatureKind::Patch { src, ref edges, tangent, .. } => {
            pc.cmd.open(pc.armed, 32, was_3d);
            qymcad_ui_state::select_body(&mut *pc.project, &mut *pc.sel, &mut *pc.view, src);
            refresh_edges(pc);
            restore_edge_picks(pc, src, edges);
            pc.opts.patch_tangent = tangent;
            pc.cmd.params = vec![];
        }
        // A COPY OF FACES, A STITCH AND A TRIM REOPEN on what they took, as every other command does. Reported behaviour:
        // a double click on their rows opened nothing, the bar empty.
        FeatureKind::FaceCopy { src, ref faces, .. } => {
            pc.cmd.open(pc.armed, 30, was_3d);
            qymcad_ui_state::select_body(&mut *pc.project, &mut *pc.sel, &mut *pc.view, src);
            pc.gsel.faces = live_picks(&*pc.project, src, faces, true);
            pc.gsel.faces_body = Some(src);
            pc.cmd.params = vec![];
        }
        FeatureKind::OffsetSurface { src, ref faces, dist, .. } => {
            pc.cmd.open(pc.armed, 36, was_3d);
            qymcad_ui_state::select_body(&mut *pc.project, &mut *pc.sel, &mut *pc.view, src);
            pc.gsel.faces = live_picks(&*pc.project, src, faces, true);
            pc.gsel.faces_body = Some(src);
            pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-distance", "dist", dist, -10000.0, 10000.0)];
        }
        // A FACE REPLACEMENT REOPENS on its faces and its sheet. Reported behaviour: a double click on its row opened
        // nothing, the bar empty.
        FeatureKind::SurfaceReplace { src, ref faces, surface, .. } => {
            pc.cmd.open(pc.armed, 31, was_3d);
            qymcad_ui_state::select_body(&mut *pc.project, &mut *pc.sel, &mut *pc.view, src);
            pc.gsel.faces = live_picks(&*pc.project, src, faces, true);
            pc.gsel.faces_body = Some(src);
            *pc.repl_surface = Some(surface);
            pc.cmd.params = vec![];
        }
        FeatureKind::Stitch { ref parts, tol, .. } => {
            pc.cmd.open(pc.armed, 33, was_3d);
            *pc.stitch_parts = parts.clone();
            pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-stitch-tol", "tol", tol, 1e-6, 10.0)];
        }
        FeatureKind::Trim { src, tool, keep, .. } => {
            pc.cmd.open(pc.armed, 34, was_3d);
            pc.trim.keep = Some((src, keep));
            pc.trim.tool = Some(tool);
            pc.cmd.params = vec![];
        }
        FeatureKind::RemoveFace { src, ref faces, .. } => {
            pc.cmd.open(pc.armed, 26, was_3d);
            qymcad_ui_state::select_body(&mut *pc.project, &mut *pc.sel, &mut *pc.view, src);
            pc.gsel.faces = live_picks(&*pc.project, src, faces, true);
            pc.gsel.faces_body = Some(src);
            pc.cmd.params = vec![];
        }
        FeatureKind::PushFace { src, ref face, dist, .. } => {
            // EDITING THE FEATURE: the picked face is restored by its persistent id and the offset as an
            // expression if there was one (`cmd_param_from` takes the text from the feature dims).
            pc.cmd.open(pc.armed, 25, was_3d);
            qymcad_ui_state::select_body(&mut *pc.project, &mut *pc.sel, &mut *pc.view, src);
            pc.gsel.faces = live_picks(&*pc.project, src, face, true);
            pc.gsel.faces_body = Some(src);
            pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-offset2", "dist", dist, -100000.0, 100000.0).nonzero()];
        }
        FeatureKind::Draft { src, ref faces, neutral, angle, flip, .. } => {
            pc.cmd.open(pc.armed, 23, was_3d);
            qymcad_ui_state::select_body(&mut *pc.project, &mut *pc.sel, &mut *pc.view, src);
            // restore the set of tilted faces and the neutral one by their persistent ids
            pc.gsel.faces = live_picks(&*pc.project, src, faces, true);
            pc.gsel.faces_body = Some(src); // the faces belong to the body of the feature (that is the highlight scope)
            pc.draft.neutral = live_picks(&*pc.project, src, &neutral, true).into_iter().next().unwrap_or(0);
            pc.draft.pick_neutral = false;
            pc.draft.flip = flip;
            pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-angle", "angle", angle, -60.0, 60.0).nonzero()];
        }
        FeatureKind::Hole { src, face, point, diameter, depth, kind, dia2, depth2, sketch, flip, .. } => {
            pc.cmd.open(pc.armed, 7, was_3d);
            qymcad_ui_state::select_body(&mut *pc.project, &mut *pc.sel, &mut *pc.view, src);
            pc.hole.kind = kind; // F1
            pc.hole.mode = if sketch != 0 { 1 } else { 0 }; // the placement mode
            pc.hole.sketch = if sketch != 0 { Some(sketch) } else { None };
            pc.hole.flip = flip;
            if sketch != 0 {
                // "by sketch": highlight the sketch of marks itself (no face pick is needed)
                if let Some(si) = pc.project.sketch_index(sketch) {
                    *pc.sel = qymcad_ui_state::Sel::Sketch(si);
                }
            } else if let Some((mi, fi)) =
                face_desc_of(&face).and_then(|d| qymcad_pick::resolve_face_sel(&*pc.project, src, &qymcad_core::feature::FaceKey { index: 0, centroid: [0.0; 3], normal: [0.0, 0.0, 1.0], id: d }))
            {
                // restore THE FACE SELECTION by its persistent `FaceKey`, then highlight it
                *pc.sel = qymcad_ui_state::Sel::Face(mi, fi);
            }
            pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-diameter", "diameter", diameter, 0.1, 10000.0), cmd_param_from(&*pc.project, fid, "f-depth", "depth", depth, 0.1, 10000.0)];
            // where it stands on its face, as the two shifts from the face centre the fields hold
            let (u, v) = match *pc.sel {
                qymcad_ui_state::Sel::Face(mi, fi) if sketch == 0 => {
                    pc.project.bodies.get(mi).and_then(|b| b.faces.get(fi)).map_or((0.0, 0.0), |f| hole_shifts([f.centroid.x, f.centroid.y, f.centroid.z], f.normal, point))
                }
                _ => (0.0, 0.0),
            };
            pc.cmd.params.push(cmd_param_from(&*pc.project, fid, "f-hole-u", "off_u", u, -100000.0, 100000.0));
            pc.cmd.params.push(cmd_param_from(&*pc.project, fid, "f-hole-v", "off_v", v, -100000.0, 100000.0));
            if kind != 0 {
                pc.cmd.params.push(cmd_param_from(&*pc.project, fid, "f-recess-d", "dia2", dia2, 0.1, 10000.0));
                pc.cmd.params.push(cmd_param_from(&*pc.project, fid, "f-recess-depth", "depth2", depth2, 0.1, 10000.0));
            }
        }
        // A THREAD: a double click reopens it with its parameters; the rim, the axis and the radius are
        // restored from `regen_edges`
        FeatureKind::Thread { src, edge, spec, length, lead_in, lead_out, .. } => {
            pc.cmd.open(pc.armed, 24, was_3d);
            qymcad_ui_state::select_body(&mut *pc.project, &mut *pc.sel, &mut *pc.view, src);
            pc.thread.src = Some(src);
            pc.thread.edge = edge;
            pc.thread.auger = false;
            pc.thread.internal = spec.internal;
            pc.thread.starts = spec.starts.max(1);
            pc.thread.left = spec.left;
            pc.thread.form = qymcad_ui_state::thread_standard_idx(spec.standard);
            qymcad_ui_state::restore_thread_axis(&mut *pc.project, &mut *pc.thread, src, edge);
            // THE SET OF FIELDS IS THE SAME AS AT CREATION. The fillets were missing here entirely: they
            // could be set only when the feature was first built, and on editing they vanished silently.
            pc.cmd.params = vec![
                cmd_param_from(&*pc.project, fid, "f-nominal-d", "nominal", spec.nominal_d, 0.5, 1000.0),
                cmd_param_from(&*pc.project, fid, "f-pitch-std", "pitch", spec.pitch, 0.0, 100.0),
                cmd_param_from(&*pc.project, fid, "f-length", "length", length, 0.1, 10000.0),
                cmd_param_from(&*pc.project, fid, "f-fit-clearance", "fit", spec.fit, 0.0, 5.0),
                cmd_param_from(&*pc.project, fid, "f-lead-in", "lead_in", lead_in, 0.0, 10000.0),
                cmd_param_from(&*pc.project, fid, "f-lead-out", "lead_out", lead_out, 0.0, 10000.0),
                cmd_param_from(&*pc.project, fid, "f-crest-fillet", "crest_r", spec.crest_r.unwrap_or(0.0), 0.0, 100.0),
                cmd_param_from(&*pc.project, fid, "f-root-fillet", "root_r", spec.root_r.unwrap_or(0.0), 0.0, 100.0),
            ];
            if spec.standard == qymcad_core::thread::ThreadStandard::Custom {
                pc.cmd.params.push(cmd_param_from(&*pc.project, fid, "f-profile-angle", "angle", spec.custom_angle, 5.0, 170.0));
                pc.cmd.params.push(cmd_param_from(&*pc.project, fid, "f-thread-depth", "depth", spec.custom_depth, 0.0, 1000.0));
            }
        }
        // AN AUGER - the same command, a mode of the timeline
        FeatureKind::Auger { src, edge, spec, length, lead_in, lead_out, .. } => {
            pc.cmd.open(pc.armed, 24, was_3d);
            qymcad_ui_state::select_body(&mut *pc.project, &mut *pc.sel, &mut *pc.view, src);
            pc.thread.src = Some(src);
            pc.thread.edge = edge;
            pc.thread.auger = true;
            pc.thread.starts = spec.starts.max(1);
            pc.thread.left = spec.left;
            qymcad_ui_state::restore_thread_axis(&mut *pc.project, &mut *pc.thread, src, edge);
            pc.cmd.params = vec![
                cmd_param_from(&*pc.project, fid, "f-outer-d", "outer", spec.outer_d, 0.5, 2000.0),
                cmd_param_from(&*pc.project, fid, "f-pitch", "pitch", spec.pitch, 0.1, 1000.0),
                cmd_param_from(&*pc.project, fid, "f-length", "length", length, 0.1, 10000.0),
                cmd_param_from(&*pc.project, fid, "f-flight-thickness", "thickness", spec.thickness, 0.1, 100.0),
                cmd_param_from(&*pc.project, fid, "f-edge-fillet", "edge_r", spec.edge_r, 0.0, 50.0),
                cmd_param_from(&*pc.project, fid, "f-taper-in", "lead_in", lead_in, 0.0, 10000.0),
                cmd_param_from(&*pc.project, fid, "f-taper-out", "lead_out", lead_out, 0.0, 10000.0),
            ];
        }
        // A MIRROR: a double click reopens the command; the plane is clicked in the viewport again
        FeatureKind::Mirror { src, plane, keep, datum, face, .. } => {
            pc.cmd.open(pc.armed, 16, was_3d);
            qymcad_ui_state::select_body(&mut *pc.project, &mut *pc.sel, &mut *pc.view, src);
            pc.opts.mirror_keep = keep;
            pc.mirror.plane = Some(qymcad_ui_state::op_plane_pick(plane, datum, face));
            pc.cmd.params.clear();
        }
        // THE PRIMITIVES: a double click reopens the command with the current sizes (the fields are the
        // keys the regen uses)
        FeatureKind::Box3 { dx, dy, dz, .. } => {
            pc.cmd.open(pc.armed, 10, was_3d);
            pc.cmd.params = vec![
                cmd_param_from(&*pc.project, fid, "f-length-x", "dx", dx, 0.1, 100000.0),
                cmd_param_from(&*pc.project, fid, "f-width-y", "dy", dy, 0.1, 100000.0),
                cmd_param_from(&*pc.project, fid, "f-height-z", "dz", dz, 0.1, 100000.0),
            ];
        }
        FeatureKind::Cylinder { r, h, .. } => {
            pc.cmd.open(pc.armed, 11, was_3d);
            pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-radius", "r", r, 0.05, 100000.0), cmd_param_from(&*pc.project, fid, "f-height", "h", h, 0.1, 100000.0)];
        }
        FeatureKind::Sphere { r, .. } => {
            pc.cmd.open(pc.armed, 12, was_3d);
            pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-radius", "r", r, 0.05, 100000.0)];
        }
        FeatureKind::Cone { r1, r2, h, .. } => {
            pc.cmd.open(pc.armed, 13, was_3d);
            pc.cmd.params = vec![
                cmd_param_from(&*pc.project, fid, "f-radius-bottom", "r1", r1, 0.0, 100000.0),
                cmd_param_from(&*pc.project, fid, "f-radius-top", "r2", r2, 0.0, 100000.0),
                cmd_param_from(&*pc.project, fid, "f-height", "h", h, 0.1, 100000.0),
            ];
        }
        FeatureKind::Torus { major, minor, .. } => {
            pc.cmd.open(pc.armed, 14, was_3d);
            pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-ring-r", "major", major, 0.1, 100000.0), cmd_param_from(&*pc.project, fid, "f-tube-r", "minor", minor, 0.1, 100000.0)];
        }
        FeatureKind::Prism { r, n, h, .. } => {
            pc.cmd.open(pc.armed, 15, was_3d);
            pc.prim.n = n;
            pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-radius-circ", "r", r, 0.1, 100000.0), cmd_param_from(&*pc.project, fid, "f-height", "h", h, 0.1, 100000.0)];
        }
        // AN ARRAY: a double click reopens the command; the count, the direction and the axis live in the
        // bar, the step and the angle at the geometry
        FeatureKind::LinearArray { src, dx, dy, dz, count, dx2, dy2, dz2, count2, dx3, dy3, dz3, count3, .. } => {
            pc.cmd.open(pc.armed, 17, was_3d);
            restore_bar_counts(pc.bar_exprs, pc.project, fid);
            qymcad_ui_state::select_body(&mut *pc.project, &mut *pc.sel, &mut *pc.view, src);
            let (dir, step) = qymcad_ui_state::arr_dir_of(dx, dy, dz);
            pc.arr.dir = dir;
            pc.arr.count = count.max(1);
            let two = count2 > 1 && (dx2.abs() + dy2.abs() + dz2.abs()) > 1e-9;
            pc.arr.two = two;
            let (dir2, s2) = qymcad_ui_state::arr_dir_of(dx2, dy2, dz2);
            pc.arr.dir2 = if two { dir2 } else { 1 };
            pc.arr.count2 = count2.max(1);
            let three = two && count3 > 1 && (dx3.abs() + dy3.abs() + dz3.abs()) > 1e-9;
            pc.arr.three = three;
            let (dir3, s3) = qymcad_ui_state::arr_dir_of(dx3, dy3, dz3);
            pc.arr.dir3 = if three { dir3 } else { 2 };
            pc.arr.count3 = count3.max(1);
            // the step is the expression text from the logical key (`store_cmd_exprs`); the numeric
            // fallback is the magnitude of the component
            pc.cmd.params = vec![cmd_param_from(&*pc.project, fid, "f-pitch", "step", step, 0.01, 100000.0)];
            // the second and third steps are added AT ONCE with the right magnitude as a fallback (the sync
            // does not know the value)
            if two {
                pc.cmd.params.push(cmd_param_from(&*pc.project, fid, "f-pitch2", "step2", s2, 0.01, 100000.0));
            }
            if three {
                pc.cmd.params.push(cmd_param_from(&*pc.project, fid, "f-pitch3", "step3", s3, 0.01, 100000.0));
            }
        }
        FeatureKind::CircularArray { src, count, angle, axis, .. } => {
            pc.cmd.open(pc.armed, 18, was_3d);
            restore_bar_counts(pc.bar_exprs, pc.project, fid);
            qymcad_ui_state::select_body(&mut *pc.project, &mut *pc.sel, &mut *pc.view, src);
            pc.arr.count = count.max(1);
            pc.arr.axis = axis;
            pc.arr.full = angle.abs() >= 359.9;
            pc.arr.two = false;
            pc.cmd.params = if pc.arr.full { vec![] } else { vec![cmd_param_from(&*pc.project, fid, "f-angle", "angle", angle, 1.0, 360.0)] };
        }
        // THE DATUMS: a double click reopens the command - a plane, a point or an axis is edited in place
        FeatureKind::Plane { plane } => {
            use qymcad_core::feature::SketchPlane;
            use qymcad_core::model::PlaneDef;
            let Some(pl) = pc.project.planes.iter().find(|p| p.id == plane).cloned() else { return };
            pc.datum.axis_ref = None;
            pc.datum.axis_mode = 0;
            match pl.def {
                PlaneDef::OffsetBase { base, dist } => {
                    pc.cmd.open(pc.armed, 20, was_3d);
                    pc.datum.plane_pick = Some(SketchPlane::World(base));
                    pc.cmd.params = datum_plane_fields_of(&*pc.project, fid, &pl, dist);
                }
                PlaneDef::OffsetFace { body, face, dist } => {
                    pc.cmd.open(pc.armed, 20, was_3d);
                    pc.datum.plane_pick = Some(SketchPlane::Face(body, face));
                    pc.cmd.params = datum_plane_fields_of(&*pc.project, fid, &pl, dist);
                }
                PlaneDef::OffsetPlane { plane: src, dist } => {
                    pc.cmd.open(pc.armed, 20, was_3d);
                    pc.datum.plane_pick = Some(SketchPlane::Datum(src));
                    pc.cmd.params = datum_plane_fields_of(&*pc.project, fid, &pl, dist);
                }
                PlaneDef::Manual | PlaneDef::FaceGone | PlaneDef::PlaneGone => {
                    *pc.status = Notice::info(qymcad_i18n::tr("msg-plane-manual"));
                    return;
                }
            }
        }
        FeatureKind::DatumPoint { point } => {
            use qymcad_core::model::PointDef;
            let Some(dp) = pc.project.datum_points.iter().find(|p| p.id == point).cloned() else { return };
            pc.cmd.open(pc.armed, 21, was_3d);
            match dp.def {
                // an associative point: it reopens in "at a vertex" mode with the current reference as the
                // preview; clicking a new vertex replaces it, and without a re-pick the definition is kept
                PointDef::AtVertex { body, edge, end } => {
                    pc.datum.pt_mode = 1;
                    pc.datum.pt_vert = Some((body, edge, end, dp.at));
                    pc.cmd.params.clear();
                }
                PointDef::Manual => {
                    pc.datum.pt_mode = 0;
                    pc.datum.pt_vert = None;
                    pc.cmd.params = vec![
                        cmd_param_from(&*pc.project, fid, "X", "x", dp.at[0], -1e7, 1e7),
                        cmd_param_from(&*pc.project, fid, "Y", "y", dp.at[1], -1e7, 1e7),
                        cmd_param_from(&*pc.project, fid, "Z", "z", dp.at[2], -1e7, 1e7),
                    ];
                }
            }
        }
        FeatureKind::DatumAxis { axis } => {
            use qymcad_core::model::AxisDef;
            let Some(da) = pc.project.datum_axes.iter().find(|a| a.id == axis).cloned() else { return };
            match da.def {
                AxisDef::Manual { .. } => {
                    pc.cmd.open(pc.armed, 22, was_3d);
                    pc.datum.axis_mode = 1;
                    pc.datum.axis_ref = None;
                    pc.cmd.params = vec![
                        cmd_param_from(&*pc.project, fid, "O.x", "ox", da.origin()[0], -1e7, 1e7),
                        cmd_param_from(&*pc.project, fid, "O.y", "oy", da.origin()[1], -1e7, 1e7),
                        cmd_param_from(&*pc.project, fid, "O.z", "oz", da.origin()[2], -1e7, 1e7),
                        cmd_param_from(&*pc.project, fid, "Dir.x", "dx", da.dir()[0], -1e7, 1e7),
                        cmd_param_from(&*pc.project, fid, "Dir.y", "dy", da.dir()[1], -1e7, 1e7),
                        cmd_param_from(&*pc.project, fid, "Dir.z", "dz", da.dir()[2], -1e7, 1e7),
                    ];
                }
                // associative axes and two-point ones reopen in "by an edge or a face" mode: the current
                // axis is the preview, and without a re-pick the definition is kept; clicking a new
                // reference replaces it
                AxisDef::TwoPoints { .. } | AxisDef::FromEdge { .. } | AxisDef::FromFace { .. } => {
                    pc.cmd.open(pc.armed, 22, was_3d);
                    pc.datum.axis_mode = 0;
                    pc.datum.axis_hit = None;
                    pc.datum.axis_ref = Some((da.origin(), da.dir()));
                    refresh_axis_edges(pc);
                    pc.cmd.params.clear();
                }
            }
        }
        FeatureKind::BodyBoolean { .. } => {
            // a body boolean has no dimension popup - its KIND is edited in the top bar (the edit mode of
            // `bool_tool_bar`)
            pc.boolean.edit = pc.project.timeline.iter().position(|n| n.id == fid);
            if let Some(ti) = pc.boolean.edit {
                *pc.sel = qymcad_ui_state::Sel::Feature(ti);
            }
            *pc.status = Notice::info(qymcad_i18n::tr("msg-bool-switch"));
            return;
        }
        _ => {
            // arrays, mirrors and moves have no command popup yet; they are edited in THE RIGHT-HAND PANEL
            // (the feature is already selected by the click)
            *pc.status = Notice::info(qymcad_i18n::tr("msg-params-on-right"));
            return;
        }
    }
    pc.cmd.edit = Some(fid);
    // the highlighted row of the tree STAYS on THE FEATURE BEING EDITED: `qymcad_ui_state::select_body(src)` in the
    // branches above moved the selection to the SOURCE body, one row higher, and the highlight appeared
    // to jump up a level while editing.
    if let Some(ti) = pc.project.timeline.iter().position(|n| n.id == fid) {
        *pc.sel = qymcad_ui_state::Sel::Feature(ti);
    }
    *pc.mode_3d = true;
    *pc.status = Notice::info(qymcad_i18n::tr("msg-edit-feature"));
}
