//! WHAT A CLICK TAKES for a command: a profile, a face, an edge, a hole's place, a reference.

use qymcad_ui_state::Notice;
use super::*;

/// A command driven by a sketch (extrude or revolve): pick a profile, set the size on the canvas, Enter.
/// THE CLICK THAT NAMES BODY B for a body-to-body boolean.
///
/// `hit` is the body the click landed on, worked out by the caller: finding it needs the whole scene and
/// acting on it needs the document, and the two borrows cannot be held at once.
///
/// A CLICK IN THE 3D VIEW WHILE AN EXTRUSION OR A REVOLUTION IS IN HAND picks the contour under it, and a second click
/// lets it go - as the status line asks ("click contours, they add up") and as a profile is picked in the professional
/// systems. A click past every contour says so and leaves what is gathered. Reported behaviour: a click on a contour in
/// 3D changed nothing - neither the picture, nor the bar, nor the status line.
pub fn profile_click_3d(pc: &mut qymcad_ui_state::PartCtx, rect: egui::Rect, pos: egui::Pos2) {
    let Some(si) = pc.cmd.sketch else { return };
    let basis = pc.cam.basis();
    let hit = qymcad_ui_state::contour_under_3d(&*pc.project, &qymcad_ui_state::Screen { cam: &*pc.cam, set: pc.set, rect, basis: &basis }, pos, si);
    match hit {
        Some(cid) => {
            if !pc.gsel.profiles.remove(&cid) {
                pc.gsel.profiles.insert(cid);
            }
            *pc.status = Notice::info(qymcad_i18n::tr1("sk-profiles-n", "n", &pc.gsel.profiles.len().to_string()));
        }
        None => *pc.status = Notice::refusal(qymcad_i18n::tr("sk-miss-contour")),
    }
}

/// WHAT A CLICK ON A BODY SELECTS: the face under it for the tools that want one, the body otherwise. For the tools
/// that take a whole body (the patterns of a body) the same body clicked again lets it go.
pub fn take_under_click(pc: &mut qymcad_ui_state::PartCtx, mesh: usize, face: Option<usize>, grip: Option<qymcad_ui_state::Sel>) {
    // WITH NOTHING IN HAND a corner or an edge under the cursor is taken for itself, and a face for itself: the body
    // is a double click away. A command keeps the body (or the face it asks for).
    let now = match (grip.filter(|_| !pc.armed.commanding()), face) {
        (Some(g), _) => g,
        (None, Some(fi)) => qymcad_ui_state::Sel::Face(mesh, fi),
        (None, None) => qymcad_ui_state::Sel::Mesh(mesh),
    };
    let again = matches!(pc.armed.cmd_kind(), 17 | 18) && *pc.sel == now;
    *pc.sel = if again { qymcad_ui_state::Sel::None } else { now };
}

/// A DOUBLE CLICK WITH NOTHING IN HAND TAKES THE WHOLE BODY of the face, edge or corner the click took: one click
/// is the piece, two are the body it belongs to.
pub fn whole_body_on_double_click(pc: &mut qymcad_ui_state::PartCtx, double: bool) {
    use qymcad_ui_state::Sel;
    if !double || pc.armed.commanding() {
        return;
    }
    let body = match *pc.sel {
        Sel::Face(mi, _) => Some(mi),
        Sel::Edge(b, _) | Sel::Vertex(b, _, _) => pc.project.mesh_index(b),
        _ => None,
    };
    if let Some(mi) = body {
        *pc.sel = Sel::Mesh(mi);
    }
}

/// A CLICK ON NOTHING: the selection of a face, a body or a component is cleared - and with a command in hand the
/// click is refused in words, the command's own ask repeated, and what the command has taken stays. Reported
/// behaviour: a click past the body with a fillet in hand said nothing, the status line as it was.
pub fn click_on_nothing(pc: &mut qymcad_ui_state::PartCtx) {
    if pc.armed.commanding() {
        *pc.status = Notice::refusal(qymcad_i18n::tr1("vp-miss-cmd", "ask", &cmd_hint(&*pc.armed, &*pc.gsel, &*pc.trim)));
        return;
    }
    if matches!(*pc.sel, qymcad_ui_state::Sel::Face(..) | qymcad_ui_state::Sel::Mesh(..) | qymcad_ui_state::Sel::Component(..) | qymcad_ui_state::Sel::Edge(..) | qymcad_ui_state::Sel::Vertex(..)) {
        *pc.sel = qymcad_ui_state::Sel::None;
    }
}

/// THE HOLE GOES WHERE THE FACE WAS CLICKED: the ray under `screen` meets the face's plane, and that point is written
/// into the two shifts from the face's centre along its own axes - the fields at the geometry, so the centre can then
/// be typed exactly.
pub fn place_hole_at(pc: &mut qymcad_ui_state::PartCtx, rect: Rect, screen: Pos2) {
    let (7, qymcad_ui_state::Sel::Face(mi, fi)) = (pc.armed.cmd_kind(), *pc.sel) else { return };
    let (Some(body), Some(face)) = (pc.project.mesh_id(mi), pc.project.bodies.get(mi).and_then(|b| b.faces.get(fi))) else { return };
    let (c, n) = ([face.centroid.x, face.centroid.y, face.centroid.z], face.normal);
    // the ray in the body's own frame: the view shows the body through its placement in the context
    let ctx = qymcad_ui_state::current_ctx_id(pc.active_path, pc.project);
    let inv = qymcad_core::feature::mat_inv12(&pc.project.body_display_transform(body, ctx));
    let (o, d) = qymcad_ui_state::screen_ray(pc.cam, rect, screen);
    let (lo, ld) = (qymcad_core::feature::apply12(&inv, o), qymcad_core::feature::apply12_dir(&inv, d));
    let Some(local) = qymcad_ui_state::ray_plane(lo, ld, c, n) else { return };
    let (u, v) = hole_shifts(c, n, local);
    // the same face clicked where the hole already stands (within 0.5 mm) lets it go; anywhere else on it moves the
    // hole there
    let here = (u - qymcad_ui_state::cmd_val(pc.cmd, "off_u")).hypot(v - qymcad_ui_state::cmd_val(pc.cmd, "off_v")) < 0.5;
    if pc.gsel.faces.contains(&face.id) && here {
        pc.gsel.faces.clear();
        *pc.sel = qymcad_ui_state::Sel::None;
        return;
    }
    pc.gsel.faces.clear();
    pc.gsel.faces.insert(face.id);
    for (key, val) in [("off_u", u), ("off_v", v)] {
        if let Some(p) = pc.cmd.params.iter_mut().find(|p| p.key == key) {
            p.val = (val * 1000.0).round() / 1000.0;
            p.txt = qymcad_i18n::num(p.val, 3);
        }
    }
}

/// How far `at` stands from the face centre `c` along the face's own two axes (those of a plane through `c` square to
/// `n`).
pub fn hole_shifts(c: [f64; 3], n: [f64; 3], at: [f64; 3]) -> (f64, f64) {
    let f = qymcad_core::feature::PlaneFrame::from_origin_normal(c, n, 0.0);
    let d = [at[0] - c[0], at[1] - c[1], at[2] - c[2]];
    let dot = |a: [f64; 3]| a[0] * d[0] + a[1] * d[1] + a[2] * d[2];
    (dot(f.x), dot(f.y))
}

/// The hole centre `u` and `v` off the face centre `c` along the face's own axes.
pub fn hole_centre(c: [f64; 3], n: [f64; 3], u: f64, v: f64) -> [f64; 3] {
    let f = qymcad_core::feature::PlaneFrame::from_origin_normal(c, n, 0.0);
    [c[0] + f.x[0] * u + f.y[0] * v, c[1] + f.x[1] * u + f.y[1] * v, c[2] + f.x[2] * u + f.y[2] * v]
}

/// THE ROW OF A SKETCH CLICKED AGAIN, the sketch already selected: a sweep lets go of its path, a loft of that section,
/// and the selection goes back to where the tool stood - a second click lets go every pick. Reported behaviour: the path of a sweep clicked
/// again in the tree stayed the path.
pub fn sketch_row_again(pc: &mut qymcad_ui_state::PartCtx, si: usize) {
    let Some(sid) = pc.project.sketches.get(si).map(|s| s.id) else { return };
    match pc.armed.cmd_kind() {
        8 if pc.sweep.path_sid == sid => {
            pc.sweep.path_sid = 0;
            pc.sweep.path_cid = 0;
            pc.sweep.pick_path = true;
        }
        9 if pc.loft.sids.contains(&sid) => {
            let i = pc.loft.sids.iter().position(|s| *s == sid).unwrap_or(0);
            pc.loft.sids.remove(i);
            pc.loft.cids.remove(i);
        }
        _ => return,
    }
    // the selection goes back to what the tool stood on before the pick: the profile of the sweep, the last section
    // of the loft left
    let back = if pc.armed.cmd_kind() == 8 { Some(pc.sweep.prof_sid) } else { pc.loft.sids.last().copied() };
    *pc.sel = back.and_then(|id| pc.project.sketches.iter().position(|s| s.id == id)).map_or(qymcad_ui_state::Sel::None, qymcad_ui_state::Sel::Sketch);
    if pc.armed.cmd_kind() == 9 {
        pc.loft.pick_last = back; // the selection moved there by this hand, not by a new pick: nothing to add
    }
}

/// THE CYLINDER OF A THREAD CLICKED AGAIN lets it go, as a second click lets go every pick: the thread is left with no
/// cylinder, the tool asks for one again. `true` when it let go.
pub fn thread_let_go(pc: &mut qymcad_ui_state::PartCtx, body: Id, rim: u32) -> bool {
    if pc.thread.src != Some(body) || pc.thread.edge != rim {
        return false;
    }
    pc.thread.src = None;
    pc.thread.edge = 0; // no rim: the bar asks for a cylinder again instead of naming the one let go
    pc.thread.radius = 0.0;
    // the fields go back to what the tool opened with: sized to a cylinder let go they would describe a thread on it
    if pc.cmd.edit.is_none() {
        qymcad_ui_state::set_thread_params(pc.cmd, *pc.thread);
    }
    *pc.status = Notice::info(qymcad_i18n::tr("cmd-thread-pick-hint"));
    true
}

/// A CLICK ON A FACE FOR A COMMAND THAT COLLECTS FACES: it is taken, or let go when taken already - `only_one` for the
/// tools that work on one face (push, thicken), where another face replaces it. A face let go is no longer lit as the
/// selection. Reported behaviour: a face clicked a second time stayed taken and lit.
pub fn toggle_face(pc: &mut qymcad_ui_state::PartCtx, id: Option<u32>, only_one: bool) {
    let Some(id) = id else { return };
    if pc.armed.cmd_kind() == 7 {
        return; // a hole's face is taken, moved to or let go by the point clicked - `place_hole_at`
    }
    let taken = pc.gsel.faces.remove(&id);
    if only_one {
        pc.gsel.faces.clear();
    }
    // A FACE LET GO takes its light with it: the click that let it go had just made it the selection of the window, and
    // the body stood lit and the face named in the properties with nothing taken (the shell, the draft)
    if !taken {
        pc.gsel.faces.insert(id);
    } else {
        *pc.sel = qymcad_ui_state::Sel::None;
    }
}

/// A DATUM POINT "BY COORDINATES": a click on a vertex types its X, Y and Z into the fields once (not associatively);
/// a click past every vertex says so.
pub fn datum_point_snap(pc: &mut qymcad_ui_state::PartCtx, hit: Option<[f64; 3]>) {
    match hit {
        Some(w) => {
            for (k, key) in ["x", "y", "z"].iter().enumerate() {
                if let Some(p) = pc.cmd.params.iter_mut().find(|p| &p.key == key) {
                    p.txt = format!("{:.3}", w[k]);
                    p.val = w[k];
                }
            }
            *pc.status = Notice::info(qymcad_i18n::tr("vp-point-bound"));
        }
        None => *pc.status = Notice::refusal(qymcad_i18n::tr("vp-miss-vertex-or-xyz")),
    }
}

pub fn take_boolean_pick(pc: &mut qymcad_ui_state::PartCtx, hit: Option<Id>) {
    let Some((a, op)) = pc.boolean.pick else { return };
    match hit {
        // a second click on body B lets it go, as a second click on any pick does
        Some(b) if pc.boolean.b == Some(b) => {
            pc.boolean.b = None;
            *pc.status = Notice::info(qymcad_i18n::tr("tb-bool-pick-b"));
        }
        Some(b) if b != a => {
            // A TRIAL ON THE LIVE SHAPES FIRST: an intersection that gives no body, or a cut that removes nothing (the two
            // pieces of a split only touch), is refused in words and the document is not touched, rather than laying a
            // node that goes red. Without both shapes (a mesh, a body not built yet) the rebuild judges it as before.
            if let (0 | 2, Some(sa), Some(sb)) = (op, pc.live.shapes.get(&a), pc.live.shapes.get(&b)) {
                if let Err(why) = qymcad_kernel::ask::boolean_gives_body(sa, sb, op) {
                    *pc.status = Notice::refusal(qymcad_i18n::tr(match why {
                        qymcad_kernel::BoolVerdict::NothingInCommon => "vp-bool-nothing-in-common",
                        qymcad_kernel::BoolVerdict::CutRemovedNothing => "vp-bool-cut-removes-nothing",
                        _ => "vp-bool-failed",
                    }));
                    return;
                }
            }
            // B IS TAKEN, NOT SPENT: it is shown as the tool until Enter, and nothing enters the document before
            pc.boolean.b = Some(b);
            *pc.status = Notice::info(qymcad_i18n::tr("vp-bool-b-taken"));
        }
        Some(_) => *pc.status = Notice::info(qymcad_i18n::tr("vp-this-is-a")),
        None => *pc.status = Notice::refusal(qymcad_i18n::tr("vp-miss-body-b")),
    }
}

/// A COMMAND WAITING FOR A SKETCH TAKES THE FIRST ONE THAT GETS SELECTED.
///
/// ONE PLACE, called once a frame after the panels have drawn. A sketch can be picked in the tree or in
/// the viewport, and putting "if something is waiting, continue it" into both would be two copies of one
/// decision - they drift apart at the first edit of one of them. Here the wait does not care where the
/// selection came from.
pub fn take_sketch_if_waiting(pc: &mut qymcad_ui_state::PartCtx) {
    let Some(kind) = pc.picking.sketch_for() else { return };
    let qymcad_ui_state::Sel::Sketch(_) = *pc.sel else { return };
    pc.picking.clear(); // cleared BEFORE the command starts: it opens its own picking, and a stale wait would outlive it
                        // BY KIND, because the four tools that need a sketch open through three different doors. Sending them
                        // all to `start_sketch_cmd` would open an extrude where a loft was asked for.
    match kind {
        8 => start_sweep_cmd(pc),
        9 => start_loft_cmd(pc),
        k => start_sketch_cmd(pc, k),
    }
}

/// Open the HALF-SKETCHER for picking the contour of a sweep or loft slot (`sid` is the slot's sketch).
/// The same mechanism as in Extrude: the sketch is shown flat and a click on a contour fills the slot
/// (see the 2D click handler and `set_contour_slot`). It replaces cycling through contours with arrows.
pub fn begin_contour_pick(pc: &mut qymcad_ui_state::PartCtx, slot: qymcad_ui_state::ContourSlot, sid: Id) {
    let Some(si) = pc.project.sketch_index(sid) else {
        *pc.status = Notice::refusal(qymcad_i18n::tr("g-slot-sketch-missing"));
        return;
    };
    *pc.picking = qymcad_ui_state::Picking::Contour(slot);
    pc.cmd.sketch = Some(si);
    *pc.sel = qymcad_ui_state::Sel::Sketch(si); // the picker shows the selected sketch, as `enter_contour_reselect` says
    qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, &mut *pc.view_restore); // the view is borrowed while the contour is picked and restored on the way out
    *pc.mode_3d = false;
    pc.view.initialized = false;
    pc.cmd.focus = false;
    let what = match slot {
        qymcad_ui_state::ContourSlot::SweepProfile => "g-slot-profile",
        qymcad_ui_state::ContourSlot::SweepPath => "g-slot-path",
        qymcad_ui_state::ContourSlot::LoftSection(_) => "g-slot-section",
    };
    *pc.status = Notice::info(qymcad_i18n::tr1("g-contour-pick-of", "what", &qymcad_i18n::tr(what)));
}

pub fn enter_contour_reselect(pc: &mut qymcad_ui_state::PartCtx) {
    let Some(si) = pc.cmd.sketch.filter(|_| matches!(pc.armed.cmd_kind(), 1 | 3)) else {
        return;
    };
    // THE PICKER SHOWS THE SELECTED SKETCH: its regions, the ones taken lit. Reopened for editing, a feature has its
    // node selected - the row of the tree stays on it - and the picker opened over it showed no region at all.
    *pc.sel = qymcad_ui_state::Sel::Sketch(si);
    qymcad_ui_state::borrow_view(*pc.cam, *pc.mode_3d, *pc.view, &mut *pc.view_restore); // the view is borrowed while contours are picked and restored on the way out
    *pc.mode_3d = false; // back to the flat half-sketcher: a click on a contour changes the set of profiles
    pc.view.initialized = false;
    pc.cmd.focus = false;
    *pc.status = Notice::info(qymcad_i18n::tr("g-contour-reselect"));
}

/// The straight edges of ALL visible (unconsumed) bodies go into `axis_edges` (the body plus a local
/// polyline), so that the axis of a pattern or a datum can be picked on an edge of ANY body rather than
/// only the selected one. Called when the pick begins.
pub fn refresh_axis_edges(pc: &mut qymcad_ui_state::PartCtx) {
    qymcad_ui_state::ensure_brep(&mut pc.rebuild()); // the candidates are edges of the LIVE B-rep; without it the list is empty and there is nothing to click
    pc.edges.axes.clear();
    let consumed = qymcad_ui_state::consumed_bodies(&*pc.project);
    for mi in 0..pc.project.bodies.len() {
        if !qymcad_ui_state::body_shown(qymcad_ui_state::body_view_in!(pc), mi) {
            continue;
        }
        let Some(b) = pc.project.mesh_id(mi) else { continue };
        if consumed.contains(&b) {
            continue;
        }
        let Some(shape) = pc.live.shapes.get(&b) else { continue };
        // A SMOOTH EDGE IS NOT OFFERED: the seam of a cylinder is a straight line of the B-rep with the same face on
        // both sides, invisible as an edge to a person, and a click on the side of a shaft took it - the axis came out
        // on the rim (35, 0) instead of up the middle (25, 0). The tangent seams of fillets go the same way.
        let (polys, ids, _, smooth) = shape.edges_full_smooth();
        for ((poly, id), sm) in polys.into_iter().zip(ids).zip(smooth) {
            if id != 0 && !sm && qymcad_ui_state::is_straight_poly(&poly) {
                pc.edges.axes.push((b, id, poly));
            }
        }
    }
}

pub fn refresh_edges(pc: &mut qymcad_ui_state::PartCtx) {
    // the B-rep cache is brought up ONLY when the edges are really needed - under the fillet and chamfer
    // commands or while edges are actively being picked. This method is called EVERY FRAME in 3D, and an
    // unconditional `ensure_brep` turned lazy B-rep building into eager: "Preparing B-rep" started right
    // after any project was opened, before anything had been done at all.
    //
    // Reported behaviour: the choice of a sketch origin on a face had disappeared. Binding a sketch's origin
    // takes VERTICES and EDGES from the live B-rep, which is no longer built on opening. Open a file, start a
    // new sketch, hover a face - there is nothing to snap to, the green marker never appears, and the origin
    // silently falls back to the default. qymcad_ui_state::Picking a sketch plane is as much "the edges are really needed" as
    // a chamfer is.
    if qymcad_ui_state::needs_live_brep(qymcad_ui_state::doing_in!(pc), &*pc.project, pc.set) {
        qymcad_ui_state::ensure_brep(&mut pc.rebuild());
    }
    // EDITING a fillet or a chamfer: the edges always belong to the feature's SOURCE BODY (a selection fix
    // put `sel` back on the node being edited, and `qymcad_ui_state::selected_body` then returned the OUTPUT body, so the edge
    // selection was cleared and the highlight vanished). The source of the feature being edited is aimed at
    // explicitly. A patch the same: its output is a sheet of its own, and a reopened patch highlighted no edge.
    let edit_src = pc.cmd.edit.and_then(|fid| {
        pc.project.timeline.iter().find(|n| n.id == fid).and_then(|n| match n.kind {
            qymcad_core::feature::FeatureKind::Fillet { src, .. } | qymcad_core::feature::FeatureKind::Chamfer { src, .. } | qymcad_core::feature::FeatureKind::Patch { src, .. } => Some(src),
            _ => None,
        })
    });
    // a Part is one body, so under Chamfer/Fillet the edges of that single body are available at once,
    // without clicking the body first (press the button, then click edges). When no body is explicitly
    // selected, the context's active_body is taken.
    //
    // PATCH BELONGS HERE TOO. It gathers EDGES just as the fillet and the chamfer do, but it used to take the
    // body only from the tree selection: press Patch without selecting a part, and not a single edge could be
    // picked. To a person that is "the tool does not work", indistinguishable from having clicked the wrong place.
    let cur = edit_src.or_else(|| qymcad_ui_state::selected_body(&*pc.project, &*pc.sel)).or_else(|| {
        if matches!(pc.armed.cmd_kind(), 4 | 5 | 32) {
            qymcad_ui_state::current_body(&qymcad_ui_state::DrawCtx { cam: pc.cam, set: &*pc.set, scheme: pc.scheme, project: &*pc.project, active_path: pc.active_path })
        } else {
            None
        }
    });
    let body_changed = cur != pc.edges.body;
    // the refresh happens not only when the body CHANGES but also when the same body is REBUILT (geom_rev
    // changed) - otherwise edge_polys and edge_ids stay with the old topology, and the highlight lands on
    // edges belonging to something else, or on ones that no longer exist.
    if body_changed || pc.edges.rev != pc.regen.geom_rev {
        pc.edges.body = cur;
        pc.edges.rev = pc.regen.geom_rev;
        if body_changed {
            pc.gsel.edges.clear(); // a different body - the edge selection is not carried over
        }
        let (polys, ids) = cur
            .and_then(|b| pc.live.shapes.get(&b).map(|s| s.edges_full_smooth()))
            .map(|(p, i, _, sm)| {
                if matches!(pc.armed.cmd_kind(), 4 | 5) {
                    // SMOOTH edges (the tangent seams of fillets) are NOT offered for a chamfer or a fillet -
                    // there is nothing there to round, and picking them only piled up red nodes
                    let (mut fp, mut fi) = (Vec::new(), Vec::new());
                    for k in 0..i.len() {
                        if !sm.get(k).copied().unwrap_or(false) {
                            fp.push(p[k].clone());
                            fi.push(i[k]);
                        }
                    }
                    (fp, fi)
                } else {
                    (p, i)
                }
            })
            .unwrap_or_default();
        pc.edges.polys = polys;
        pc.edges.ids = ids; // the persistent edge ids, parallel to the polylines
                            // drop the picked ids the body no longer has (it was rebuilt), so that no phantoms are lit
        if !pc.gsel.edges.is_empty() {
            let live: std::collections::HashSet<u32> = pc.edges.ids.iter().copied().collect();
            pc.gsel.edges.retain(|id| live.contains(id));
        }
    }
}

/// A CLICK OF THE SECTION TOOL while it picks its plane: the plane taken puts the section on, and the same plane
/// clicked again lets it go - as a pick of every command is let go. The tool stays picking until Apply or Enter.
pub fn section_pick_click(section: &mut qymcad_ui_state::SectionTool, regen: &mut qymcad_ui_state::Rebuilding, status: &mut qymcad_ui_state::Notice, plane: Option<([f64; 3], [f64; 3])>) {
    let Some((o, n)) = plane else {
        *status = Notice::refusal(qymcad_i18n::tr("vp-miss-plane-datum-face"));
        return;
    };
    let same = section.plane.is_some_and(|(po, pn)| (0..3).all(|i| (po[i] - o[i]).abs() < 1e-6 && (pn[i] - n[i]).abs() < 1e-6));
    if same {
        section.plane = None;
        *status = Notice::info(qymcad_i18n::tr("tb-section-pick"));
    } else {
        section.plane = Some((o, n));
        section.offset = 0.0;
        section.rot = [0.0, 0.0];
        *status = Notice::info(qymcad_i18n::tr("vp-section-on"));
    }
    qymcad_ui_state::invalidate(regen);
}
