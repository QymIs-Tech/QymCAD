//! PICKING AND HIT TESTING: what is under the cursor.
//!
//! This used to live mixed in with the rendering and the commands in a 27-thousand-line `gui.rs`. Out of
//! that grew the defect with choosing an axis: the click handler ended up in the 2D half of the viewport
//! while the candidates were drawn and hit-tested in 3D — both halves were in one function, and nobody
//! noticed them drifting apart.

pub(crate) use qymcad_pick::*;
pub(crate) use qymcad_ui_state::PickCtx;
use super::*;

impl App {
    /// A RAY INTO A FACE WITH NO SIDE EFFECTS: (the body, the persistent id of the face, the point of the
    /// hit in the world).
    ///
    /// Besides searching, `pick_face_3d` also CHANGES the selection, the set of faces of a command and the
    /// highlight — the measuring tool needs none of that and is harmed by it: measure a gap and lose the
    /// selection of the part. The search is the same, only clean.
    pub(super) fn pick_face_ray(&self, rect: Rect, screen: Pos2) -> Option<(qymcad_core::model::Id, u32, [f64; 3])> {
        pick_face_ray(&self.painting(), rect, screen)
    }




    /// The vertex (an end of an edge) under the cursor among ALL the visible bodies -> (the body, the id
    /// of the edge, which end).
    pub(super) fn pick_vertex_any(&self, rect: Rect, pos: Pos2) -> Option<(Id, u32, bool)> {
        pick_vertex_any(&self.painting(), rect, pos)
    }



    /// THE FACE OF A PART UNDER THE CURSOR — a base plane does NOT intercept it.
    ///
    /// The assembly tools go by this resolution: collecting an anchor, editing an anchor, the secondary
    /// axis, the tangency, the width. All of them want a face of a part, and a miss must mean emptiness
    /// under the cursor rather than an invisible square a third of the scene across turning out to be
    /// nearer.
    pub(super) fn pick_part_face_at(&self, rect: Rect, screen: Pos2) -> Option<(qymcad_core::model::Id, qymcad_core::feature::FaceKey)> {
        pick_part_face_at(&self.painting(), rect, screen)
    }




    /// THE ONE PLACE THE BORROWS ARE SPLIT for picking: three shared borrows instead of the application.
    pub(super) fn pick_ctx(&self) -> PickCtx<'_> {
        PickCtx { project: &self.project, set: &self.set, view: &self.viewing.view }
    }


    pub(super) fn pick_sketch_plane_at(&self, rect: Rect, screen: Pos2) -> Option<qymcad_core::feature::SketchPlane> {
        pick_sketch_plane_at(&self.painting(), rect, screen)
    }


    /// The DATUM POINT under the cursor -> (its Id, its world position). For a two-point axis (kept
    /// parametric through `TwoPoints`).
    pub(super) fn pick_datum_point_at(&self, rect: Rect, pos: Pos2) -> Option<(Id, [f64; 3])> {
        pick_datum_point_at(&self.painting(), rect, pos)
    }
















    /// A CLICK ON A VERTEX IN THE FILLET: create or remove its radius field. `true` means a hit.
    pub(super) fn pick_fillet_vertex(&mut self, rect: Rect, screen: Pos2) -> bool {
        let Some((desc, p)) = fillet_vertex_at(&qymcad_ui_state::Screen { cam: &self.viewing.cam, set: &self.set, rect, basis: &self.viewing.cam.basis() }, &self.tools.armed, &self.edges, &self.tools.gsel, &mut self.project, screen) else { return false };
        let key = format!("at{desc}");
        if let Some(i) = self.tools.cmd.params.iter().position(|p| p.key == key) {
            self.tools.cmd.params.remove(i);
            self.status = crate::i18n::tr("pk-vertex-radius-off");
        } else {
            let base = qymcad_ui_state::cmd_val(&self.tools.cmd, "radius");
            self.tools.cmd.params.push(qymcad_ui_state::CmdParam::new("f-radius-at-vertex", &key, base, 0.0, 1000.0).at(p));
            self.status = crate::i18n::tr("pk-vertex-radius-on");
        }
        true
    }

    pub(super) fn pick_edge_3d(&mut self, rect: Rect, screen: Pos2) -> bool {
        // A VERTEX OUTRANKS AN EDGE WHEN IT IS UNDER THE CURSOR: a corner is aimed at in order to set a
        // radius there, and hitting an edge instead would clear the selection rather than fine-tune it.
        if self.pick_fillet_vertex(rect, screen) {
            return true;
        }
        let Some(id) = edge_at(&self.active_path, self.viewing.cam, &self.edges, &self.project, &self.set, rect, screen) else { return false };
        if !self.tools.gsel.edges.insert(id) {
            self.tools.gsel.edges.remove(&id);
        }
        // A SINGLE EDGE BREAKS THE DESCRIPTION: "every edge of the face except this one" is not something
        // we express, and leaving the description as it stands would be a lie.
        self.tools.gsel.described = None;
        self.status = crate::i18n::tr1("pk-edges-selected", "n", &self.tools.gsel.edges.len().to_string());
        true
    }


    /// A click on a FACE of a body in the chamfer or the fillet -> select or clear ALL the edges of that
    /// face (a click on a face of a cube gives its 4 edges). It toggles: if every edge of the face is
    /// already selected they are cleared, otherwise they are added. `true` means a face was hit.
    pub(super) fn pick_face_edges_fillet(&mut self, rect: Rect, pos: Pos2) -> bool {
        let Some(body) = self.edges.body else { return false };
        let Some(fid) = pick_face_persist_id(&self.painting(), rect, pos).filter(|&f| f != 0) else { return false };
        // THE SECOND SIDE OF A JUNCTION. The junction item of the menu put the command into waiting for a
        // second pick — and here it is. The reference is assembled only now: a junction has two sides and
        // is not described by one.
        if let Some(first) = self.tools.gsel.between_first.take() {
            if fid != first {
                let q = qymcad_core::refs::Query::Between(
                    Box::new(qymcad_core::refs::Query::Id(first)),
                    Box::new(qymcad_core::refs::Query::Id(fid)),
                );
                self.apply_expansion("expand-between-done", q);
                return true;
            }
            // the same face was clicked — there is no junction with itself, so the wait goes on
            self.tools.gsel.between_first = Some(first);
            self.status = crate::i18n::tr("expand-between-pick-second");
            return true;
        }
        let eids = match self.live.shapes.get(&body) {
            Some(shape) => shape.face_edge_ids(fid),
            None => return false,
        };
        // only the edges that really exist in the body are taken (ids from `edge_ids`), so a face of
        // another body will not be caught
        let live: std::collections::HashSet<u32> = self.edges.ids.iter().copied().collect();
        let eids: Vec<u32> = eids.into_iter().filter(|id| live.contains(id)).collect();
        if eids.is_empty() {
            return false;
        }
        let all_sel = eids.iter().all(|id| self.tools.gsel.edges.contains(id));
        for id in &eids {
            if all_sel {
                self.tools.gsel.edges.remove(id);
            } else {
                self.tools.gsel.edges.insert(*id);
            }
        }
        // THE FACE ITSELF IS REMEMBERED rather than only its edges of today: the intention is "this whole
        // rim", and after an edit that added edges it must stay true.
        if all_sel {
            self.tools.gsel.described = None; // the face was cleared, so there is no description any more
            self.tools.gsel.last_face = None;
        } else {
            self.tools.gsel.describe_edges_of_face(fid);
            self.tools.gsel.last_face = Some((fid, body)); // the expand-the-selection menu will ask about it
            self.tools.gsel.last_edge = None; // a face was asked about, not an edge
        }
        self.status = crate::i18n::trn("pk-face-edges", &[("n", &eids.len().to_string()), ("what", &if all_sel { crate::i18n::tr("pk-removed") } else { crate::i18n::tr("pk-added") }), ("total", &self.tools.gsel.edges.len().to_string())]);
        true
    }










    /// Choose a font of one's own (TTF or OTF) — the bytes go into the cache.
    pub(super) fn pick_font(&mut self) {
        self.ask_open_file(rfd::AsyncFileDialog::new().add_filter(crate::i18n::tr("pk-font"), &["ttf", "otf", "TTF", "OTF"]), |app, p| match std::fs::read(&p) {
            Ok(b) => {
                app.font_cache = Some(b);
                app.status = crate::i18n::tr1("pk-font-is", "name", &p.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default());
            }
            Err(e) => app.status = crate::i18n::tr1("pk-font-error", "error", &e.to_string()),
        });
    }


    pub(super) fn pick_dxf(&mut self) {
        self.ask_open_file(rfd::AsyncFileDialog::new().add_filter("DXF", &["dxf"]), |app, p| app.open_dxf(p.to_string_lossy().into_owned()));
    }

    pub(super) fn pick_stl(&mut self) {
        self.ask_open_file(rfd::AsyncFileDialog::new().add_filter("STL", &["stl"]), |app, p| crate::gui::open_stl(&mut app.regen, p.to_string_lossy().into_owned()));
    }

    pub(super) fn pick_step(&mut self) {
        self.ask_open_file(rfd::AsyncFileDialog::new().add_filter("STEP", &["step", "stp"]), |app, p| crate::gui::open_step(&mut app.regen, p.to_string_lossy().into_owned()));
    }

    pub(super) fn pick_svg(&mut self) {
        self.ask_open_file(rfd::AsyncFileDialog::new().add_filter("SVG", &["svg"]), |app, p| {
            let path = p.to_string_lossy().into_owned();
            match import_svg(&path) {
                Ok(sk) => app.arm_sketch_import(sk.curves, &path),
                Err(e) => app.status = crate::i18n::tr1("pk-svg-error", "error", &e.to_string()),
            }
        });
    }
















    /// Picking the target of a thread — a cylindrical face gives the source body plus the circular rim edge
    /// (the axis and the radius by fact), plus a heuristic for inner or outer (by the direction of the
    /// normal relative to the axis).
    pub(super) fn pick_thread_target(&mut self, rect: Rect, pos: Pos2) {
        let Some(AxisHit::Face(body, fid)) = pick_axis_at(&self.painting(), rect, pos) else {
            self.status = crate::i18n::tr("pk-miss-cylinder");
            return;
        };
        // the edges of this face -> the circular rims among the `regen_edges` of the body. A cylinder has
        // TWO rims (top and bottom) — the one NEAREST to the point of the click is taken (a defect: `.find`
        // used to take an arbitrary one, so a thread began at a random end). That way the SIDE OF ENTRY is
        // chosen by the click: click near the end you want and the thread runs from there.
        let eids = self.live.shapes.get(&body).map(|s| s.face_edge_ids(fid)).unwrap_or_default();
        let basis = self.viewing.cam.basis();
        let scr = qymcad_ui_state::Screen { cam: &self.viewing.cam, set: &self.set, rect: rect, basis: &basis };
        let wt = self.project.body_display_transform(body, qymcad_ui_state::current_ctx_id(&self.active_path, &self.project));
        // THE RADIUS IS TAKEN FROM THE FACE ITSELF rather than from the nearest rim. If a chamfer has been
        // cut at the end, the edges of a cylindrical face include rims of DIFFERENT radii, and "the nearest
        // to the click" gives the wrong one — the thread is built on the wrong surface, and the report was
        // that a thread cannot be drawn where there is a chamfer. This also catches a click on the chamfer
        // itself: on a cone the radius changes along the axis and the spread is large.
        let face_tris = self
            .project
            .mesh_index(body)
            .and_then(|mi| self.project.bodies.get(mi).and_then(|b| b.faces.iter().find(|f| f.id == fid)).map(|f| (mi, f.triangles.clone())));
        let candidates: Vec<_> = self
            .project
            .regen_edges
            .get(&body)
            .map(|es| es.iter().filter(|e| e.is_circular() && eids.contains(&e.id)).cloned().collect())
            .unwrap_or_default();
        let face_r = face_tris.as_ref().and_then(|(mi, tris)| {
            candidates.first().and_then(|e| qymcad_core::geom::cyl_face_radius(&self.project.bodies[*mi].mesh, tris, e.center, e.axis))
        });
        if let Some((_, spread)) = face_r {
            if spread > 0.08 {
                self.status = crate::i18n::tr("pk-not-a-cylinder");
                return;
            }
        }
        let circ = candidates
            .iter()
            .filter(|e| face_r.map(|(m, _)| (e.radius - m).abs() < 0.05 * m.max(1e-6)).unwrap_or(true))
            .min_by(|a, b| {
                let da = scr.at(qymcad_core::feature::apply12(&wt, a.center)).0.distance(pos);
                let db = scr.at(qymcad_core::feature::apply12(&wt, b.center)).0.distance(pos);
                da.total_cmp(&db)
            })
            .map(|e| (e.id, e.center, e.axis, e.radius));
        let Some((eid, center, mut axis, radius)) = circ else {
            self.status = crate::i18n::tr("pk-no-round-rim");
            return;
        };
        // The axis is turned ALONG THE CHOSEN FACE: a thread runs where the cylinder itself lies. Computing
        // it over the whole mesh ("where there are more vertices") will not do — with a chamfer at the end
        // the rim ends up at its base, and on a part such as a boss the thread ran INTO THE AIR, towards the
        // end. If the face is unavailable, the former way over the mesh remains.
        match face_tris.as_ref() {
            Some((mi, tris)) => axis = qymcad_core::geom::axis_along_face(&self.project.bodies[*mi].mesh, tris, center, axis),
            None => {
                if let Some(mi) = self.project.mesh_index(body) {
                    axis = qymcad_core::model::orient_axis_into_mesh(center, axis, &self.project.bodies[mi].mesh.verts);
                }
            }
        }
        self.params.thread.src = Some(body);
        self.params.thread.edge = eid;
        self.params.thread.axis = (center, axis);
        self.params.thread.radius = radius;
        self.params.thread.internal = qymcad_ui_state::cyl_face_is_internal(&self.project, body, fid, center, axis);
        // the size comes FROM THE GEOMETRY — all that is left is choosing a standard (the nominal is
        // already filled in)
        if self.tools.cmd.edit.is_none() {
            qymcad_ui_state::set_thread_params(&mut self.tools.cmd, self.params.thread);
        }
        self.status = crate::i18n::trn(
            "pk-thread-target",
            &[
                ("what", &if self.params.thread.internal { crate::i18n::tr("pk-hole") } else { crate::i18n::tr("pk-cylinder") }),
                ("d", &crate::i18n::num(radius * 2.0, 1)),
                ("next", &if self.params.thread.auger { crate::i18n::tr("pk-set-flight") } else { crate::i18n::tr("pk-pick-standard") }),
            ],
        );
    }













    pub(super) fn pick_face_3d(&mut self, rect: Rect, screen: Pos2) {
        let basis = self.viewing.cam.basis();
        let scr = qymcad_ui_state::Screen { cam: &self.viewing.cam, set: &self.set, rect: rect, basis: &basis };
        let ctx = qymcad_ui_state::current_ctx_id(&self.active_path, &self.project);
        // ONLY WHAT IS DRAWN gets picked: bodies consumed by modifiers (the hidden old extrusion under the
        // final one) and the result of the feature being edited are NOT picked, otherwise a click lands in
        // a hidden body — the defect where one place gave three different selections. The criterion is the
        // same as in the rendering.
        let consumed = qymcad_ui_state::consumed_bodies(&self.project);
        let edit_hide = qymcad_ui_state::edit_hidden_bodies(&self.tools.cmd, &self.project);
        let edit_src = if !edit_hide.is_empty() { qymcad_ui_state::edit_src_body(&self.tools.cmd, &self.project) } else { None };
        // A TOOL FOR BODIES DOES NOT CATCH ON A SURFACE. A copy of a face lies EXACTLY ON the face of the
        // part, and a click there landed on the sheet: the shell got a face of a surface and honestly
        // refused (`OpFailed(Shell)`), while what was seen was "I clicked the face of the part and it does
        // not work". The tools for which a sheet is lawful (thicken, replace a face, stitch, trim, patch,
        // copy a face) are not included here — a surface is what they want.
        let solid_only = matches!(self.tools.armed.cmd_kind(), 4 | 5 | 6 | 7 | 23 | 25 | 26 | 27 | 29);
        let mut best: Option<(f64, usize, usize)> = None; // (the depth, the mesh, the triangle)
        for (mi, mesh) in self.project.bodies.iter().map(|b| &b.mesh).enumerate() {
            if !qymcad_ui_state::body_shown(qymcad_ui_state::body_view_of!(self), mi) {
                continue;
            }
            if solid_only && self.project.bodies[mi].sheet {
                continue;
            }
            // ...AND NOT ON A NEIGHBOURING PART EITHER. While working in context the bodies of the
            // neighbours are shown as ghosts, so that their geometry can be REFERRED to (a sketch on the
            // face of a neighbour, top-down). But a tool editing THE CURRENT part cannot take them as a
            // target: isolation rejects such a reference, the node rolls back, and what is seen is "I
            // clicked and nothing happened" — with no reason and no red. The worst kind of breakage: no
            // trace is left at all.
            if solid_only {
                let mine = self.project.mesh_id(mi).and_then(|b| self.project.body_owner(b)).is_some_and(|o| o == ctx || self.project.component_is_within(o, ctx));
                if !mine {
                    continue;
                }
            }
            let b = self.project.mesh_id(mi);
            if b.is_some_and(|b| edit_hide.contains(&b)) || b.is_some_and(|b| consumed.contains(&b) && Some(b) != edit_src) {
                continue; // a hidden body is not picked
            }
            // a body is picked where it is drawn — in the frame of the active context
            let wt = self.project.mesh_id(mi).map(|b| self.project.body_display_transform(b, ctx)).unwrap_or(qymcad_core::feature::PLACE_IDENTITY);
            let tp = |v: [f64; 3]| if qymcad_core::feature::is_identity12(&wt) { v } else { qymcad_core::feature::apply12(&wt, v) };
            for ti in 0..mesh.tris.len() {
                let t = mesh.triangle(ti);
                let (wa, wb, wc) = (tp([t[0].x, t[0].y, t[0].z]), tp([t[1].x, t[1].y, t[1].z]), tp([t[2].x, t[2].y, t[2].z]));
                if qymcad_ui_state::section_tri_hidden(&self.side.section, wa, wb, wc) {
                    continue; // THE SECTION: what is hidden is not picked, so clicks reach the innards
                }
                // A BACK-FACING triangle, as in `rasterize_3d`. Reported behaviour: a click on a wheel
                // selected a part ten centimetres away from it — a wrapping face gives a back-facing
                // triangle a wrong depth
                if v_dot(v_norm(v_cross(v_sub(wb, wa), v_sub(wc, wa))), basis.2) >= 0.0 {
                    continue;
                }
                let (pa, da) = scr.at(wa);
                let (pb, db) = scr.at(wb);
                let (pc, dc) = scr.at(wc);
                if point_in_tri(screen, pa, pb, pc) {
                    let depth = tri_depth_at(screen, pa, da, pb, db, pc, dc);
                    if best.is_none_or(|(bd, _, _)| depth < bd) {
                        best = Some((depth, mi, ti));
                    }
                }
            }
        }
        if let Some((_, mi, ti)) = best {
            // In an Assembly a click on a body selects the COMPONENT in the tree (and switches on the
            // placement gizmo); in a Part it selects a FACE (for features and for anchoring a sketch). The
            // behaviour follows the workbench, with no manual switching.
            if matches!(self.workbench, Workbench::Assembly) {
                // The isolation of the selection: a body of a leaf part inside a subassembly selects THE
                // SUBASSEMBLY (the direct child of the active context) rather than the leaf.
                // `highlight_mesh_set(Component)` will highlight its whole subtree. A click on a body
                // directly in the context selects that component itself.
                let ctx = qymcad_ui_state::current_ctx_id(&self.active_path, &self.project);
                let comp = self.project.mesh_id(mi).and_then(|b| self.project.body_owner(b)).and_then(|owner| self.project.ancestor_child_of(ctx, owner));
                if let Some(idx) = comp.and_then(|cid| self.project.components.iter().position(|c| c.id == cid)) {
                    self.chosen.sel = Sel::Component(idx);
                } else {
                    self.chosen.sel = Sel::Mesh(mi);
                }
            } else {
                // A face is selected ONLY under a command that asks for one (the shell, the hole); an
                // ordinary click in a Part selects THE BODY and switches on the move gizmo.
                let want_face = matches!(self.tools.armed.cmd_kind(), 6 | 7 | 23 | 25 | 26 | 28 | 30 | 31);
                let fi = if want_face { self.project.bodies.get(mi).and_then(|b| b.faces.iter().position(|f| f.triangles.contains(&(ti as u32)))) } else { None };
                self.chosen.sel = match fi {
                    Some(fi) => Sel::Face(mi, fi),
                    None => Sel::Mesh(mi),
                };
                // The shell and the draft: multi-selection of faces strictly within ONE body — the ids of
                // faces are local to a body (OCCT numbers them from zero in each). A click on a face of
                // ANOTHER body starts the selection afresh on it, otherwise the ids of neighbouring bodies
                // get confused and the highlight leaks across.
                if matches!(self.tools.armed.cmd_kind(), 6 | 23 | 25 | 26 | 28 | 30) && fi.is_some() {
                    // command 31 is NOT included here: there a click on ANOTHER body means "here is the
                    // surface" rather than "start the selection afresh" (see below).
                    let clicked_body = self.project.mesh_id(mi);
                    if self.tools.gsel.faces_body != clicked_body {
                        self.tools.gsel.faces.clear();
                        self.params.draft.neutral = 0;
                        self.tools.gsel.faces_body = clicked_body;
                    }
                }
                // The shell: a click on a face ADDS or REMOVES its id from the multi-selection (by the
                // persistent id)
                if self.tools.armed.cmd_kind() == 6 {
                    if let Some(id) = fi.and_then(|fi| self.project.bodies.get(mi).and_then(|b| b.faces.get(fi))).map(|f| f.id) {
                        if !self.tools.gsel.faces.remove(&id) {
                            self.tools.gsel.faces.insert(id);
                        }
                    }
                }
                // PUSH FACE: the face is EXACTLY ONE — a click replaces the previous one rather than
                // accumulating a set. Pushing several faces by one offset means a different result on each
                // of them (their normals differ), and that cannot be predicted.
                // THICKEN follows the same logic: the face is EXACTLY ONE, and one plate comes out of it.
                if self.tools.armed.cmd_kind() == 25 || self.tools.armed.cmd_kind() == 28 {
                    if let Some(id) = fi.and_then(|fi| self.project.bodies.get(mi).and_then(|b| b.faces.get(fi))).map(|f| f.id) {
                        self.tools.gsel.faces.clear();
                        self.tools.gsel.faces.insert(id);
                    }
                }
                // REMOVE FACE: a multi-selection — a feature may consist of several faces (a stepped hole,
                // a boss with a chamfer). A click adds or removes.
                // REPLACE FACE: a click on a SHEET chooses the surface, a click on a body collects the
                // faces to be replaced. The document itself tells them apart (`sheet`) rather than a mode
                // switch: an extra mode here would force one to remember which step one is on.
                // TRIM: the first click on a SHEET says both WHAT is being cut and WHICH part is kept — the
                // point of the hit is the answer about the side. The second click, on a body, gives the
                // tool.
                if self.tools.armed.cmd_kind() == 34 {
                    let clicked = self.project.mesh_id(mi);
                    let is_sheet = self.project.bodies.get(mi).is_some_and(|b| b.sheet);
                    if is_sheet && self.side.trim.keep.is_none() {
                        if let (Some(id), Some((_, _, at))) = (clicked, self.pick_face_ray(rect, screen)) {
                            self.side.trim.keep = Some((id, at));
                            self.status = crate::i18n::tr("msg-trim-pick-tool");
                        }
                    } else if let Some(id) = clicked {
                        if self.side.trim.keep.map(|(b, _)| b) == Some(id) {
                            self.side.trim.keep = None; // a repeated click on the same sheet starts afresh
                            self.status = crate::i18n::tr("msg-trim");
                        } else {
                            self.side.trim.tool = if self.side.trim.tool == Some(id) { None } else { Some(id) };
                            self.status = crate::i18n::tr(if self.side.trim.tool.is_some() { "msg-trim-tool-picked" } else { "msg-trim-pick-tool" });
                        }
                    }
                }
                // STITCH: a click on a SHEET adds it to the set or removes it. Clicking a body is
                // pointless — surfaces are what get stitched, and that must be said at once rather than
                // after Enter.
                if self.tools.armed.cmd_kind() == 33 {
                    let clicked = self.project.mesh_id(mi);
                    let is_sheet = self.project.bodies.get(mi).is_some_and(|b| b.sheet);
                    match (is_sheet, clicked) {
                        (true, Some(id)) => {
                            if let Some(at) = self.params.stitch_parts.iter().position(|x| *x == id) {
                                self.params.stitch_parts.remove(at);
                            } else {
                                self.params.stitch_parts.push(id);
                            }
                            self.status = crate::i18n::tr1("msg-stitch-picked", "n", &self.params.stitch_parts.len().to_string());
                        }
                        _ => self.status = crate::i18n::tr("msg-stitch-only-sheets"),
                    }
                }
                if self.tools.armed.cmd_kind() == 31 {
                    let clicked = self.project.mesh_id(mi);
                    let is_sheet = self.project.bodies.get(mi).is_some_and(|b| b.sheet);
                    if is_sheet {
                        self.params.repl_surface = if self.params.repl_surface == clicked { None } else { clicked };
                        self.status = crate::i18n::tr(if self.params.repl_surface.is_some() { "msg-surface-picked" } else { "msg-surface-unpicked" });
                    } else if let Some(id) = fi.and_then(|fi| self.project.bodies.get(mi).and_then(|b| b.faces.get(fi))).map(|f| f.id) {
                        if self.tools.gsel.faces_body != clicked {
                            self.tools.gsel.faces.clear();
                            self.tools.gsel.faces_body = clicked;
                        }
                        if !self.tools.gsel.faces.remove(&id) {
                            self.tools.gsel.faces.insert(id);
                        }
                    }
                }
                if matches!(self.tools.armed.cmd_kind(), 26 | 30) {
                    if let Some(id) = fi.and_then(|fi| self.project.bodies.get(mi).and_then(|b| b.faces.get(fi))).map(|f| f.id) {
                        if !self.tools.gsel.faces.remove(&id) {
                            self.tools.gsel.faces.insert(id);
                        }
                    }
                }
                // The draft: in the neutral-face mode a click sets the neutral face (by the persistent
                // id), otherwise a click ADDS or REMOVES a face from the set of faces to be drafted.
                if self.tools.armed.cmd_kind() == 23 {
                    if let Some(id) = fi.and_then(|fi| self.project.bodies.get(mi).and_then(|b| b.faces.get(fi))).map(|f| f.id) {
                        if self.params.draft.pick_neutral {
                            self.params.draft.neutral = if self.params.draft.neutral == id { 0 } else { id };
                            self.params.draft.pick_neutral = false;
                            self.tools.gsel.faces.remove(&id); // the neutral face cannot also be a drafted one
                        } else if !self.tools.gsel.faces.remove(&id) {
                            self.tools.gsel.faces.insert(id);
                        }
                    }
                }
            }
        } else if matches!(self.chosen.sel, Sel::Face(..) | Sel::Mesh(..) | Sel::Component(..)) {
            // a click into emptiness clears the selection of a face, a body or a component
            self.chosen.sel = Sel::None;
        }
    }



}
