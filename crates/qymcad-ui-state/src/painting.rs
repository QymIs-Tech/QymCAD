//! WHAT IS DRAWN AND WHERE: the painting, the screen, what is shown and what is hidden.

use super::*;

/// THE END OF A BAR, laid from the right, under an id of its own taken from the place in the code that draws it - not
/// one counted after what the bar holds before it, which changes with the tool in hand: egui marked the buttons at the
/// end with red frames each time ("the rectangle changed its id"). A call site is drawn once a frame.
#[track_caller]
pub fn end_of_bar<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> egui::InnerResponse<R> {
    let at = std::panic::Location::caller();
    let id = egui::Id::new(("the end of a bar", at.file(), at.line(), at.column()));
    ui.scope_builder(egui::UiBuilder::new().id(id), |ui| ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), add).inner)
}

/// THE LAYER OF A BOX OF FIELDS AT THE GEOMETRY: the layer of the windows, not above it. A box laid over every window
/// covered the settings and any other window dragged over the view; on the layer of the windows the one touched last
/// stands on top, and a window opened over the box covers it. Reported behaviour: the box of a radius over the
/// settings.
pub const AT_GEOMETRY: egui::Order = egui::Order::Middle;

/// WHAT A BOX OF FIELDS AT THE GEOMETRY HOLDS: the names of its fields are captions, not text to select - a drag
/// across them to reach a field selected the name instead.
pub fn at_geometry_ui(ui: &mut egui::Ui) {
    ui.style_mut().interaction.selectable_labels = false;
}

/// Keep the ANCHOR of a dimension input popup inside the viewport `rect`, so that with large values
/// (where the geometry runs off the edge) the little window does not fly out of the frame. The popup
/// is about 200x60 px, so room is left for it.
pub fn clamp_popup(p: Pos2, rect: Rect) -> Pos2 {
    let r = rect.shrink(4.0);
    Pos2::new(p.x.clamp(r.left(), (r.right() - 200.0).max(r.left())), p.y.clamp(r.top() + 32.0, (r.bottom() - 50.0).max(r.top() + 32.0)))
}

/// THE PLACEMENT CHANGED — THE SHAPE OF THE BODIES DID NOT.
///
/// The door for driving a part and for moving a component. `invalidate` declares the GEOMETRY stale,
/// and on a real assembly that cost 30-48 ms per frame: all 463,878 vertices of the scene buffer were
/// assembled again although one body had moved. Reported behaviour: the part drags along as if on a
/// rubber band.
pub fn invalidate_placement(regen: &mut Rebuilding) {
    regen.place_rev = regen.place_rev.wrapping_add(1);
}

pub fn invalidate(regen: &mut Rebuilding) {
    regen.geom_rev = regen.geom_rev.wrapping_add(1); // invalidate the 3D render cache
}

pub fn to_world(view: &View2d, rect: Rect, s: Pos2) -> Point2 {
    let c = rect.center();
    Point2::new((view.center.x + (s.x - c.x) / view.scale) as f64, (view.center.y - (s.y - c.y) / view.scale) as f64)
}

/// The index of the note under a screen point (a rough bounding box of the text).
pub fn note_at(project: &Project, view: &View2d, rect: Rect, pos: Pos2, si: usize) -> Option<usize> {
    let s = project.sketches.get(si)?;
    for (i, n) in s.notes.iter().enumerate() {
        let sp = (Sheet { view: *view, rect }).at(Point2::new(n.x, n.y));
        let w = (n.text.chars().count().max(1) as f32) * 8.0;
        let bb = Rect::from_min_max(sp + egui::vec2(-2.0, -17.0), sp + egui::vec2(w, 3.0));
        if bb.contains(pos) {
            return Some(i);
        }
    }
    None
}

/// The handle of a polygon under the cursor is the centre of its circumscribed (construction)
/// circle, when the cursor is close to that "rim". A circle counts as a rim when vertices hang on it
/// (PointOnCircle towards its centre). Returns the Id of the centre (so the radius can be retyped).
pub fn polygon_under(project: &Project, view: &View2d, rect: Rect, pos: Pos2, si: usize) -> Option<Id> {
    let sh = Sheet { view: *view, rect };
    use qymcad_core::model::{Constraint, EntityKind};
    let s = project.sketches.get(si)?;
    for e in &s.entities {
        let EntityKind::Circle { center, r } = e.kind else { continue };
        if !e.construction {
            continue;
        }
        let is_rim = s.constraints.iter().any(|c| matches!(c, Constraint::PointOnCircle { c: cc, .. } if *cc == center));
        if !is_rim {
            continue;
        }
        if let Some((cx, cy)) = s.points.iter().find(|q| q.id == center).map(|q| (q.x, q.y)) {
            let sc = sh.at(Point2::new(cx, cy));
            let rp = (sh.at(Point2::new(cx + r, cy)).x - sc.x).abs();
            if (sc.distance(pos) - rp).abs() <= 8.0 {
                return Some(center);
            }
        }
    }
    None
}

/// A dependable unit SCREEN direction of the line a -> b. When the screen segment degenerates (a
/// unit-long coordinate axis seen from far away puts both ends in one pixel), the direction is taken
/// from THE MODEL (by projecting a far point of the line); otherwise a dimension to an axis comes
/// out mangled, because its perpendicular degenerates.
pub fn line_screen_dir(project: &Project, view: &View2d, si: usize, a: Id, b: Id, rect: Rect) -> Option<egui::Vec2> {
    let sh = Sheet { view: *view, rect };
    let (pa, pb) = (sketch_pt(project, si, a)?, sketch_pt(project, si, b)?);
    let sa = sh.at(pa);
    let v = sh.at(pb) - sa;
    if v.length() > 0.5 {
        return Some(v.normalized());
    }
    let far = Point2::new(pa.x + (pb.x - pa.x) * 1e4, pa.y + (pb.y - pa.y) * 1e4);
    let d = sh.at(far) - sa;
    (d.length() > 1e-4).then(|| d.normalized())
}

pub fn sel_circle_centers(project: &Project, sel_sk: &SketchSelection, si: usize) -> Vec<Id> {
    use qymcad_core::model::EntityKind;
    let Some(s) = project.sketches.get(si) else { return Vec::new() };
    sel_sk
        .items
        .iter()
        .filter(|(k, _)| *k == 1)
        .filter_map(|(_, eid)| {
            s.entities.iter().find(|e| e.id == *eid).and_then(|e| match e.kind {
                EntityKind::Circle { center, .. } | EntityKind::Arc { center, .. } => Some(center),
                _ => None,
            })
        })
        .collect()
}

/// The text object under a screen point (by the bounding box of its glyphs). For selecting, moving and editing.
pub fn text_at(project: &Project, view: &View2d, rect: Rect, pos: Pos2, si: usize) -> Option<usize> {
    let sh = Sheet { view: *view, rect };
    let s = project.sketches.get(si)?;
    for i in 0..s.texts.len() {
        if let Some((minx, miny, maxx, maxy)) = project.sketch_text_bbox(si, i) {
            let p0 = sh.at(Point2::new(minx, miny));
            let p1 = sh.at(Point2::new(maxx, maxy));
            if Rect::from_two_pos(p0, p1).expand(5.0).contains(pos) {
                return Some(i);
            }
        }
    }
    None
}

/// The index of the active sketch (while in editing mode).
pub fn edit_si(project: &Project, sketch_ses: &SketchSession) -> Option<usize> {
    sketch_ses.editing.and_then(|id| project.sketch_index(id))
}

/// The radius of a curve, found by its CENTRE: a circle (r) or an arc (|centre to end|). The single
/// source for radius and diameter dimensions — they work the same for a circle and for an arc (after
/// trimming).
pub fn center_radius(dc: &DrawCtx, si: usize, c: Id) -> Option<f64> {
    radius_of(dc.project, si, c)
}

/// The radius of the circle or arc of sketch `si` centred at point `c`.
pub fn radius_of(project: &Project, si: usize, c: Id) -> Option<f64> {
    use qymcad_core::model::EntityKind;
    let s = project.sketches.get(si)?;
    s.entities.iter().find_map(|e| match e.kind {
        EntityKind::Circle { center, r } if center == c => Some(r),
        EntityKind::Arc { center, a, .. } if center == c => sketch_pt(project, si, a).zip(sketch_pt(project, si, center)).map(|(pa, pc)| ((pa.x - pc.x).powi(2) + (pa.y - pc.y).powi(2)).sqrt()),
        _ => None,
    })
}

/// TAKE DRAWING TOOL `kind` IN HAND: every other tool put down in one move (`exit_draw_tools`), whatever the one before
/// had collected cleared (`SketchTool::select`), and the selection of the geometry dropped - a drawing tool works on
/// nothing selected before it. Reported behaviour: a line selected, the circle taken and drawn, and the line stood
/// selected the whole time and after. The arrow of selection (kind 0) keeps the selection.
pub fn take_drawing_tool(t: &mut Tools, kind: u8) {
    exit_draw_tools(t);
    t.tool.select(t.armed, kind);
    if t.armed.draw_kind() != 0 {
        t.sel_sk.clear();
    }
}

/// LEAVING ALL THE TOOLS — the single transition from a mode back to selection.
///
/// The sketch modes are mutually exclusive: exactly one is active. This exit used to be written out
/// by hand in six places, and the sets of fields in the copies DIFFERED — one reset forgot an
/// unfinished import, another the pattern, a third the modify mode. So the exit exists in a single
/// copy, and entering any tool begins with it. The selection (`sel_sk.items`) survives: what is
/// cleared is the modes, not the work already done.
pub fn exit_draw_tools(t: &mut Tools) {
    let Tools { armed, annot, cmd, corner, dim, drag, gsel, inline, measure, pat, pending_import, picking, place, sel_sk, tool } = t;
    // THE CORNER TOOL'S PICKS ARE ITS OWN STATE: the lines it was offered are lit to show a person WHICH corner is
    // being rounded, and with the mode gone there is nothing to show - they stood lit with nothing in hand, and the
    // next mode read a selection that was not its own. (The button that offers the corner of two lines already
    // chosen reads them while no mode is in hand, so this costs that nothing.)
    if matches!(armed.click_op(), 4 | 5) {
        sel_sk.items.clear();
    }
    **armed = Armed::None; // ONE FIELD: letting go is saying that nothing is in hand
    tool.pts.clear();
    tool.circ_tan = None;
    sel_sk.constraint = None;
    sel_sk.modify = None;
    sel_sk.offset = OffsetAim::default();
    dim.pick.clear();
    dim.first = None;
    place.dim = None;
    pending_import.draw_pts = None;
    drag.clear();
    inline.clear();
    picking.clear(); // exclusivity: the shape pick of "Fillet all" does not survive a change of mode
    corner.put_down();
    measure.clear(); // both the FLAG and the points collected: otherwise they outlived the exit
    tool.move_base = None;
    tool.extend = None; // the line Extend held
    pat.edit = None;
    pat.center = None;
    cmd.close(armed); // the command is closed as a whole, not just a field zeroed out
    cmd.sketch = None;
    gsel.profiles.clear();
    // clear the viewport selection of a dimension, a note or a text (otherwise it stays highlighted while drawing)
    gsel.constraint = None;
    annot.note = None;
    annot.text = None;
}

/// Attach a tangency constraint between a new circle or arc (centre `cen`, radius `r`, at (cx, cy)) and the base edge.
pub fn add_tangent_to_edge(project: &mut Project, si: usize, eref: EdgeRef, cen: Id, cx: f64, cy: f64, r: f64) {
    use qymcad_core::model::Constraint;
    match eref {
        EdgeRef::Line { a, b } => {
            project.add_constraint_if_independent(si, Constraint::Tangent { a, b, c: cen, r });
        }
        EdgeRef::Circle { center, r: rr } => {
            if let Some(pc) = sketch_pt(project, si, center) {
                let dist = ((cx - pc.x).powi(2) + (cy - pc.y).powi(2)).sqrt();
                let external = (dist - (rr + r)).abs() <= (dist - (rr - r).abs()).abs();
                project.add_constraint_if_independent(si, Constraint::CircleTangent { c1: center, c2: cen, external });
            }
        }
    }
}

pub fn sketch_pt(project: &qymcad_core::model::Project, si: usize, id: Id) -> Option<Point2> {
    project.sketches.get(si)?.points.iter().find(|p| p.id == id).map(|p| Point2::new(p.x, p.y))
}

/// WHAT DRAWING READS. Fifty-five fields, every one of them SHARED: the whole of `render.rs` is `&self`,
/// so there is no mutability to reason about here and nothing to lose by assignment. That is why the file
/// went over whole rather than in batches - the traps that make a conversion slow are all about `&mut`.
///
/// Copy types travel by value so that matching and comparison keep working; the rest by reference.
pub struct Painting<'a> {
    pub armed: &'a Armed,
    pub boolean: &'a BoolCommand,
    pub active_path: &'a Vec<Id>,
    pub arr: ArrayParams,
    pub body_giz: &'a BodyGizmo,
    pub cache: &'a Caches,
    pub cam: Cam3,
    pub carr: &'a CompArrayCmd,
    pub clip: &'a Clipboard,
    pub cmd: &'a FeatCommand,
    pub comp_giz: CompGizmo,
    pub cursor: Option<Point2>,
    pub datum: &'a DatumCommand,
    pub draft: DraftParams,
    /// the chamfer in hand: its mode and its reference face, which the preview lays the legs by
    pub chamfer: ChamferParams,
    pub edges: &'a EdgeCache,
    pub handle_drag: Option<Handle>,
    pub feat: FeatTarget,
    pub gpu_ok: bool,
    pub gsel: &'a GeomSelection,
    pub hole: HoleCommand,
    pub hover: Hover,
    pub inline: InlineEdit,
    pub interference: &'a Interference,
    pub joint: &'a JointCommand,
    pub live: &'a LiveGeom,
    pub loft: &'a LoftParams,
    pub m3: &'a Measure3,
    pub mirror: &'a MirrorParams,
    pub mode_3d: bool,
    pub pat: PatternTool,
    pub pending_import: &'a PendingImport,
    pub picking: Picking,
    pub prim: PrimParams,
    pub project: &'a Project,
    pub regen: &'a Rebuilding,
    pub repl_surface: Option<Id>,
    pub rev: RevolveParams,
    pub rot: &'a RotInput,
    pub scheme: &'a SchemeUi,
    pub section: &'a SectionTool,
    pub snap_hint: Option<(Point2, u8)>,
    pub sel: Sel,
    pub sel_sk: &'a SketchSelection,
    pub set: &'a Settings,
    pub sk_pat: SketchPattern,
    pub sketch_hidden: &'a std::collections::HashSet<Id>,
    pub sketch_ses: SketchSession,
    pub split: &'a SplitParams,
    pub stitch_parts: &'a Vec<Id>,
    pub recognise: &'a RecogniseTool,
    pub sweep: SweepParams,
    pub thread: ThreadParams,
    pub tool: &'a SketchTool,
    pub tool_prefs: &'a SketchToolPrefs,
    pub trim: &'a TrimTool,
    pub view: View2d,
    pub view_dragging: bool,
    pub win: &'a Windows,
    pub workbench: Workbench,
}

/// The bodies consumed by modifier features (hidden, so that only the result is seen): the core's answer
/// (`Project::consumed_bodies`), less the sources of a step that has built nothing yet.
///
/// A STEP NOT BUILT YET HIDES NOTHING ON SCREEN: what was there before stays until its result is in. The core
/// counts such a step as consuming - a document is read that way before any rebuild - and so the screen went
/// empty for as long as the step took. Reported behaviour: the rollback bar dragged back down under a rounding of
/// 36 edges, and the part vanished from the viewport while the rounding was computed.
pub fn consumed_bodies(project: &qymcad_core::model::Project) -> std::collections::HashSet<Id> {
    let built = |b: &Id| project.mesh_index(*b).is_some_and(|mi| !project.bodies[mi].mesh.tris.is_empty());
    // the sources taken by a step above the bar that has a result on screen, or makes no body at all
    let taken: std::collections::HashSet<Id> = project
        .timeline
        .iter()
        .take(project.rollback_place())
        .filter(|n| {
            let outs = n.kind.bodies();
            outs.is_empty() || outs.iter().any(built)
        })
        .flat_map(|n| n.kind.consumed())
        .collect();
    let mut eaten = project.consumed_bodies();
    eaten.retain(|b| taken.contains(b));
    eaten
}

/// Is the cylindrical face an INTERNAL one (a hole)? The core decides, from the face's triangles; here
/// we only fetch the mesh and the face.
pub fn cyl_face_is_internal(project: &qymcad_core::model::Project, body: Id, fid: u32, center: [f64; 3], axis: [f64; 3]) -> bool {
    let Some(mi) = project.mesh_index(body) else { return false };
    let Some(f) = project.bodies.get(mi).and_then(|b| b.faces.iter().find(|f| f.id == fid)) else { return false };
    qymcad_core::geom::cyl_face_is_internal(&project.bodies[mi].mesh, &f.triangles, center, axis)
}

/// THE VIEW REVISION: what is on screen depends both on the shape of the bodies and on where they
/// stand.
///
/// Caches that depend on the placement are keyed by it (world bounding boxes, section caps, the scene
/// buffer). Caches that depend ONLY on the shape (a body's edges) still look at `geom_rev`.
pub fn view_rev(regen: &Rebuilding) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (regen.geom_rev, regen.place_rev).hash(&mut h);
    h.finish()
}

/// The world snapping point under the cursor for PLACING a primitive: a vertex > a datum point > the
/// centre of a face.
/// The cursor's ray in WORLD space. The camera is orthographic (`Screen::at` gives sx = rel . right,
/// sy = rel . up, depth = rel . fwd), so every ray is parallel to `fwd`; the ray's origin is taken in
/// the plane of `target`, leaving the depth free.
pub fn screen_ray(cam: &Cam3, rect: Rect, screen: Pos2) -> ([f64; 3], [f64; 3]) {
    let (right, up, fwd) = cam.basis();
    let c = rect.center();
    let sx = ((screen.x - c.x) / cam.scale) as f64;
    let sy = (-(screen.y - c.y) / cam.scale) as f64; // the screen Y is inverted (see `Screen::at`)
    let t = cam.target;
    let o = [t[0] + right[0] * sx + up[0] * sy, t[1] + right[1] * sx + up[1] * sy, t[2] + right[2] * sx + up[2] * sy];
    (o, fwd)
}

/// The current context (the last one on the path) — the component being edited.
pub fn current_ctx_id(active_path: &[Id], project: &Project) -> Id {
    active_path.last().copied().unwrap_or(project.root)
}

/// The SOURCE body consumed by the feature being edited: while editing it is shown, so its face and edges are visible.
pub fn edit_src_body(cmd: &FeatCommand, project: &Project) -> Option<Id> {
    let fid = cmd.edit?;
    project.timeline.iter().find(|n| n.id == fid).and_then(|n| n.kind.consumed_body())
}

/// The bodies HIDDEN while a modifier feature is being edited: the feature itself plus its whole
/// DESCENDANT chain (whatever consumes it, transitively). The source is shown, with its faces and
/// edges visible and selectable — a temporary rollback to the feature being edited. It works IN THE
/// MIDDLE of a chain too (mid-chain used to show the final model, with the face sitting on a hidden
/// body). Empty when a base feature is being edited (it has no source), so its body is not hidden.
pub fn edit_hidden_bodies(cmd: &FeatCommand, project: &Project) -> std::collections::HashSet<Id> {
    let mut hide = std::collections::HashSet::new();
    let Some(fid) = cmd.edit else { return hide };
    let Some(node) = project.timeline.iter().find(|n| n.id == fid) else { return hide };
    if node.kind.consumed_body().is_none() {
        return hide; // a base feature: its body stays visible
    }
    let Some(b0) = node.kind.body() else { return hide };
    hide.insert(b0);
    // a forward closure over consumed(): everything that sits on b0, transitively
    let mut changed = true;
    while changed {
        changed = false;
        for n in &project.timeline {
            if n.kind.consumed().iter().any(|c| hide.contains(c)) {
                if let Some(b) = n.kind.body() {
                    if hide.insert(b) {
                        changed = true;
                    }
                }
            }
        }
    }
    hide
}

/// IS THE SCENE BEING MOVED BY HAND RIGHT NOW.
///
/// Four different things can have hold of it at once: a body's gizmo, a component's gizmo, a joint being
/// dragged, and a part pulled along its joint. Everything expensive is skipped while any of them has
/// hold, and asking about one and forgetting the rest is how a heavy recompute lands in the middle of a
/// drag and the picture jerks.
#[derive(Clone, Copy)]
pub struct SceneDrag<'a> {
    pub body_giz: &'a BodyGizmo,
    pub comp_giz: CompGizmo,
    pub joint: &'a JointCommand,
    pub part_pull: &'a Option<(Id, [f64; 3], [f64; 3])>,
}

/// THE FLAT FRAME A 3D SCENE IS DRAWN INTO: where the camera stands, how the world is turned to face
/// the screen, the rectangle it lands in, and the settings that decide perspective against orthographic.
///
/// These four travel together everywhere a world point has to become a screen point. `basis` is the
/// camera's own three directions (`Cam3::basis`), computed once per drawing pass and carried along
/// rather than recomputed per point.
#[derive(Clone, Copy)]
pub struct Screen<'a> {
    pub cam: &'a Cam3,
    pub set: &'a Settings,
    pub rect: Rect,
    pub basis: &'a ([f64; 3], [f64; 3], [f64; 3]),
}

impl Screen<'_> {
    /// Where a world point lands, and how deep it is.
    pub fn at(&self, p: [f64; 3]) -> (Pos2, f64) {
        let (cam, set, rect, basis) = (self.cam, self.set, self.rect, self.basis);
        let rel = v_sub(p, cam.target);
        let sx = v_dot(rel, basis.0);
        let sy = v_dot(rel, basis.1);
        let depth = v_dot(rel, basis.2);
        // Perspective: the eye sits a finite `d_eye` behind the target plane along -fwd, and the screen
        // offset is divided by (1 + depth/d_eye). In orthographic mode `inv_d_eye` is 0, so f = 1 and the
        // projection matches the previous one exactly. The denominator is clamped from below (a point in
        // front of the eye) — unreachable in a CAD orbit, but it guards against degeneracy.
        let inv = persp_inv_d_eye(cam, set, rect.height() * 0.5);
        let f = (1.0 / (1.0 + depth * inv).max(0.05)) as f32;
        let c = rect.center();
        (Pos2::new(c.x + sx as f32 * f * cam.scale, c.y - sy as f32 * f * cam.scale), depth)
    }
}

/// The body is shown only as CONTEXT (a neighbouring part while `show_context` is on) rather than as
/// the active part, so it is drawn as a ghost and not picked as one's own (edit-in-context).
pub fn body_is_ghost(dc: &DrawCtx, mi: usize) -> bool {
    let ctx = current_ctx_id(dc.active_path, dc.project);
    if ctx == dc.project.root {
        return false;
    }
    match dc.project.mesh_id(mi).and_then(|b| dc.project.body_owner(b)) {
        Some(owner) => !dc.project.component_is_within(owner, ctx),
        None => false,
    }
}

/// IS A HIT UNDER THE CURSOR NEARER THAN THE BEST ONE SO FAR? By depth - except that a copy of a face lies exactly on
/// the face it was taken from, at the same depth, and which of the two a click got was down to which body came first:
/// always the part. Where a tool wants a surface - a stitch; a replace-face once the faces to replace are taken; the
/// first click of a trim - a sheet at the same depth as a solid wins; for every other tool, and for the steps that want
/// the body (the faces to replace, the body to trim with), the depth alone decides, as before.
pub fn nearer_hit(depth: f64, sheet: bool, best: Option<(f64, bool)>, prefer_sheets: bool) -> bool {
    match best {
        None => true,
        Some((best_depth, best_sheet)) => {
            // the same triangles copied: the depths agree to the rounding of the projection
            let tie = (depth - best_depth).abs() <= 1e-6 * best_depth.abs().max(1.0);
            if prefer_sheets && tie {
                sheet && !best_sheet
            } else {
                depth < best_depth
            }
        }
    }
}

/// The triangle is hidden by the section (by its centroid — for PICKING; the renderer cuts it properly by clipping).
pub fn section_tri_hidden(section: &SectionTool, a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> bool {
    section.plane.is_some() && section_hidden(section, [(a[0] + b[0] + c[0]) / 3.0, (a[1] + b[1] + c[1]) / 3.0, (a[2] + b[2] + c[2]) / 3.0])
}

/// Whether to show a body (by mesh index) in the current context: the visibility tick AND the part's
/// isolation — inside a Part ONLY its own bodies are visible (each part is its own space and frame);
/// in the root assembly all of them are.
pub fn body_shown(bv: BodyView, mi: usize) -> bool {
    // THE BODY OF A RED NODE IS NOT SHOWN. A node that failed to build still has a body: it is created
    // together with the node and, until the first successful build, carries the source's mesh. Showing
    // it means drawing the part's ghost right next to it — two bodies on screen although the operation
    // failed. Caught by the fuzzer: a fillet applied to a surface goes red, and the part ends up with
    // two visible bodies.
    //
    // An empty body is hidden for the same reason: there is nothing to draw in it, yet it counts in
    // the lists of "what we show".
    // THE SOURCE OF A RED NODE, which stays on screen in place of the result that failed to build
    let mut red_source = false;
    if let Some(b) = bv.project.bodies.get(mi) {
        if b.mesh.tris.is_empty() {
            return false;
        }
        // THE BODY OF A RED NODE IS A GHOST. Only that body ITSELF is hidden: the source must stay on
        // screen, otherwise a failed operation wipes out the whole part (its body having been consumed
        // by the node that failed to build). That is the rule in full: on failure we show what was
        // there BEFORE, not emptiness and not two bodies at once.
        // NOTHING IS RED IN THE ORDINARY CASE, and then there is nothing to look for. The scan walks the whole
        // timeline for every body; on a document of 1296 bodies that is 1.7 million comparisons per pass, and
        // the pass happens on every frame.
        let red = |id: Id| !bv.project.regen_errors.is_empty() && bv.project.timeline.iter().any(|n| n.kind.owns_body(id) && bv.project.regen_errors.contains_key(&n.id));
        if red(b.id) {
            return false;
        }
        // The consuming node is red, so the source stays visible - AS THE PART'S BODY, under every rule below: its
        // own tick, its component's, the context. Reported behaviour: a part with a broken feature was seen as a
        // ghost from every other component, with In context off and with its tick off in the tree, because this
        // answered "shown" before any of them was asked.
        red_source = body_is_consumed(bv.cache, bv.project, bv.regen, b.id) && bv.project.timeline.iter().any(|n| n.kind.consumed().contains(&b.id) && bv.project.regen_errors.contains_key(&n.id));
    }
    let ctx = current_ctx_id(bv.active_path, bv.project);
    // A CONSUMED BODY IS NOT A BODY BUT A STEP OF HISTORY. A part is one body, and every operation
    // carries it along: an extrude, a cut, a fillet produce a NEW body and consume the previous one.
    // The previous ones stayed "visible": they were only out of sight because the resulting body
    // covers them. They were still drawn and still walked through while picking — and the cost grew
    // with the length of the timeline rather than with the number of parts. Hence the freeze while
    // picking an edge anchor: on a part with twenty operations, twenty copies of the same shape were
    // walked through.
    //
    // THERE IS ONE EXCEPTION — THE SOURCE OF THE FEATURE BEING EDITED. While editing a modifier, one
    // looks at the state BEFORE it: the feature's result and its descendant chain are hidden, and the
    // consumed source is shown — otherwise there is nothing to click on and nothing to edit. The
    // exception used to stand LOWER, in the list of visible bodies, and was dead code: control
    // reached this point first and threw the source away as consumed. It showed up as pressing Edit
    // on a fillet and the part disappearing entirely.
    if !red_source && bv.project.mesh_id(mi).is_some_and(|b| body_is_consumed(bv.cache, bv.project, bv.regen, b) && Some(b) != edit_src_body(bv.cmd, bv.project)) {
        return false;
    }
    let owner = bv.project.mesh_id(mi).and_then(|b| bv.project.body_owner(b));
    // Hiding an INDIVIDUAL body by hand (the body's tick in the tree) always applies, whatever the context.
    if !bv.project.bodies.get(mi).is_none_or(|b| b.visible) {
        return false;
    }
    let Some(owner) = owner else {
        return ctx == bv.project.root; // a body with no owner is visible only in the root assembly
    };
    // The root assembly: visibility follows the chain of component ticks all the way to the root.
    if ctx == bv.project.root {
        return component_chain_visible(bv.project, owner, None);
    }
    // Inside a part, its own bodies; inside a subassembly, the bodies of all its parts (the subtree).
    // The ticks of the child parts hide their bodies, while the context component ITSELF (`stop`) is
    // excluded — on entering a hidden subassembly we see its contents according to its descendants' ticks.
    if bv.project.component_is_within(owner, ctx) {
        return component_chain_visible(bv.project, owner, Some(ctx));
    }
    // IN THE SKETCHER (while editing a sketch) the 3D bodies of in-context neighbours are NOT drawn at
    // all: they get in the way of building the part. The face being built on is shown by highlighting
    // its edges (`draw_sketch_face_edges`), not as a body.
    if bv.sketch_ses.editing.is_some() {
        return false;
    }
    // In-context mode: the bodies of NEIGHBOURING parts (in the parent assembly) are shown as ghosts,
    // so that their geometry can be referred to (top-down). Otherwise isolation hides them.
    if bv.win.context {
        if let Some(parent) = bv.project.components.iter().find(|c| c.id == ctx).and_then(|c| c.parent) {
            if bv.project.component_is_within(owner, parent) {
                return component_chain_visible(bv.project, owner, Some(parent));
            }
        }
    }
    false
}

/// Reported behaviour: the base planes XY/XZ/YZ are impossible to hit — they flash for a moment and
/// vanish as soon as the cursor moves by a pixel. A fixed half-size of 60 mm makes a clickable spot the
/// size of a pinhead at the scale of a typical assembly (hundreds of millimetres across, with the camera
/// far back) — the square is barely visible and cannot be hovered. So it is scaled by the actual
/// bounding box of the visible scene, as the section gizmo is; an empty scene keeps the old default.
pub fn plane_pick_half_size(pn: &Painting) -> f64 {
    let mut lo = [f64::MAX; 3];
    let mut hi = [f64::MIN; 3];
    for SceneMesh { mesh, world: wt, .. } in visible_mesh_items(pn) {
        if let Some(b) = mesh.bounds() {
            for c in [[b.min.x, b.min.y, b.min.z], [b.max.x, b.max.y, b.max.z]] {
                let w = qymcad_core::feature::apply12(&wt, c);
                for k in 0..3 {
                    lo[k] = lo[k].min(w[k]);
                    hi[k] = hi[k].max(w[k]);
                }
            }
        }
    }
    if lo[0] > hi[0] {
        return 60.0; // an empty scene
    }
    let diag = ((hi[0] - lo[0]).powi(2) + (hi[1] - lo[1]).powi(2) + (hi[2] - lo[2]).powi(2)).sqrt();
    (diag * 0.3).clamp(60.0, 500.0)
}

/// The display transform of a datum (a point, an axis or a plane, by its Id) in the active context's
/// frame — so that a part's datums travel with it in an assembly, just as its bodies do. None means
/// another component's datum (isolation: we do not draw it).
/// A PLANE PICKED IN THE VIEW as an origin and a normal in the frame of the context: a base plane as it is, a datum
/// carried from its owner's frame, a face from its body's. One answer for every tool that takes a plane by a click - the
/// mirrored copy and the section had a copy of it each.
pub fn plane_in_context(pn: &Painting, plane: qymcad_core::feature::SketchPlane) -> Option<([f64; 3], [f64; 3])> {
    use qymcad_core::feature::{apply12, apply12_dir, SketchPlane, PLACE_IDENTITY};
    match plane {
        SketchPlane::World(bp) => {
            let f = bp.frame();
            Some((f.origin, f.normal()))
        }
        SketchPlane::Datum(id) => pn.project.planes.iter().find(|p| p.id == id).map(|p| {
            // a datum is stored in its owner's LOCAL frame - its transform carries it into the context
            let wt = datum_render_transform(pn, id).unwrap_or(PLACE_IDENTITY);
            (apply12(&wt, p.origin), apply12_dir(&wt, p.normal))
        }),
        SketchPlane::Face(body, key) => {
            let wt = pn.project.body_display_transform(body, current_ctx_id(pn.active_path, pn.project));
            Some((apply12(&wt, key.centroid), apply12_dir(&wt, key.normal)))
        }
    }
}

pub fn datum_render_transform(pn: &Painting, datum_id: Id) -> Option<[f64; 12]> {
    use qymcad_core::feature::FeatureKind as FK;
    // the visibility tick in the tree (by a stable Id): a hidden datum is drawn nowhere
    if pn.datum.hidden.contains(&datum_id) {
        return None;
    }
    let owner = pn
        .project
        .timeline
        .iter()
        .find(|n| {
            matches!(n.kind, FK::DatumPoint { point } if point == datum_id)
                || matches!(n.kind, FK::DatumAxis { axis } if axis == datum_id)
                || matches!(n.kind, FK::Plane { plane } if plane == datum_id)
        })
        .and_then(|n| n.parent)?;
    // A datum is visible ONLY when the visibility context is the owner itself or one of its
    // descendants (we are "inside" the owner). So a Part's datums are NOT shown in the assembly or at
    // the root (owner is the Part, viz is the Assembly, and viz is not inside owner), but they appear
    // as soon as one enters the part. An ancestor assembly's datums, meanwhile, stay visible from a
    // nested part (ancestor reference geometry). `viz_ctx_id` accounts for sketch editing — the datums
    // of the sketch's owner are visible even when it was entered by a double click from the root.
    let viz = viz_ctx_id(pn.active_path, pn.project, pn.sketch_ses);
    if !pn.project.component_is_within(viz, owner) {
        return None; // another component's datum, or a part's datum seen from the assembly: not shown
    }
    // An ANCESTOR's datum (the owner being a strict ancestor of the context: a plane or an axis of the
    // ASSEMBLY, seen from a nested subassembly or part) is shown ONLY with in-context mode on. The
    // context's own datums are always visible. This keeps the parent's reference geometry from
    // cluttering the part by default.
    if owner != viz && !pn.win.context {
        return None;
    }
    // the placement is RELATIVE to the active context, just as bodies are, so that the datum travels with the part
    Some(pn.project.relative_transform(owner, current_ctx_id(pn.active_path, pn.project)))
}

/// Whether the chain of components from `owner` up through its parents is visible (every `visible`
/// tick is on). The walk stops BEFORE `stop` (`stop` itself and its ancestors are not checked) — so
/// on entering a hidden subassembly or part (the context being `stop`), its contents are shown
/// according to the ticks of its own descendants.
/// IS SKETCH `sketch` SHOWN BY THE TICKS OF ITS COMPONENTS, seen from context `ctx`: the same chain its part's bodies go
/// by - a sketch of a part whose tick is off is hidden with the part. Reported behaviour: in an assembly the sketches of
/// a hidden part stayed on screen with "Sketch outlines" on, over the place the part was hidden from.
pub fn sketch_shown_by_components(project: &qymcad_core::model::Project, sketch: Id, ctx: Id) -> bool {
    let Some(owner) = project.node_component(sketch) else { return ctx == project.root };
    if ctx == project.root {
        return component_chain_visible(project, owner, None);
    }
    !project.component_is_within(owner, ctx) || component_chain_visible(project, owner, Some(ctx))
}

pub fn component_chain_visible(project: &qymcad_core::model::Project, owner: Id, stop: Option<Id>) -> bool {
    let mut cur = Some(owner);
    while let Some(id) = cur {
        if Some(id) == stop {
            break;
        }
        let Some(c) = project.components.iter().find(|c| c.id == id) else {
            break;
        };
        if !c.visible {
            return false;
        }
        cur = c.parent;
    }
    true
}

/// `inv_d_eye = 1/d_eye` for perspective (0 in orthographic mode). `d_eye = world_half_h / tan(fov/2)`,
/// where `world_half_h = half_h_points / scale`. The single source of the formula for `Screen::at` and for
/// the GPU shader.
pub fn persp_inv_d_eye(cam: &Cam3, set: &Settings, half_h_points: f32) -> f64 {
    if set.projection == Projection::Ortho {
        return 0.0;
    }
    // THE HALF-TANGENT FROM THE ANGLE: the setting holds an angle in degrees (which is what one
    // pictures), while the projection needs the half-tangent. The conversion lives here alone, at the
    // point of use.
    (set.persp_fov_deg.to_radians() / 2.0).tan() * cam.scale as f64 / (half_h_points.max(1.0) as f64)
}

/// Whether a body is consumed — cached, because it is asked for every body on every frame.
pub fn body_is_consumed(cache: &Caches, project: &Project, regen: &Rebuilding, body: Id) -> bool {
    let mut c = cache.consumed.borrow_mut();
    if c.rev != view_rev(regen) {
        c.rev = view_rev(regen);
        c.value = consumed_bodies(project);
    }
    c.value.contains(&body)
}

/// Is the point HIDDEN by the section? (the side along the normal is hidden; "Flip" changes the normal's sign)
pub fn section_hidden(section: &SectionTool, p: [f64; 3]) -> bool {
    match section_eff(section) {
        Some((o, n)) => (p[0] - o[0]) * n[0] + (p[1] - o[1]) * n[1] + (p[2] - o[2]) * n[2] > 0.0,
        None => false,
    }
}

/// The VISIBILITY context: while a sketch is being edited it is the sketch's owner component (so
/// isolation works however the sketch was entered, even by a double click from the root); otherwise
/// it is the active context. `active_path` is NOT touched — that keeps the drill-in/drill-out stack
/// invariant (every push of the path is paired with one on `nav_stash`).
pub fn viz_ctx_id(active_path: &[Id], project: &Project, sketch_ses: SketchSession) -> Id {
    if let Some(sid) = sketch_ses.editing {
        if let Some(owner) = project.sketch_owner(sid) {
            return owner;
        }
    }
    current_ctx_id(active_path, project)
}

pub fn v_sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// The EFFECTIVE section plane: the base one, plus the tilts (deg about U and V), plus the shift along the normal.
pub fn section_eff(section: &SectionTool) -> Option<([f64; 3], [f64; 3])> {
    let (o, n0) = section.plane?;
    let nn = v_norm(n0);
    let a = if nn[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
    let u = v_norm(v_cross(a, nn));
    let v = v_cross(nn, u);
    let rot = |vec: [f64; 3], axis: [f64; 3], deg: f64| -> [f64; 3] {
        let (s, c) = deg.to_radians().sin_cos();
        let cr = v_cross(axis, vec);
        let d = v_dot(axis, vec);
        [vec[0] * c + cr[0] * s + axis[0] * d * (1.0 - c), vec[1] * c + cr[1] * s + axis[1] * d * (1.0 - c), vec[2] * c + cr[2] * s + axis[2] * d * (1.0 - c)]
    };
    let n = v_norm(rot(rot(nn, u, section.rot[0]), v, section.rot[1]));
    let o = [o[0] + n[0] * section.offset, o[1] + n[1] * section.offset, o[2] + n[2] * section.offset];
    Some((o, n))
}

/// The visible bodies to draw: (highlighted, ghosted, base colour, mesh, world transform).
/// The single source for the CPU raster and for the GPU pass (it accounts for consumed bodies, feature
/// editing, the assembly context and the body gizmo's preview).
/// The first element of the tuple is the mesh's index in `project.meshes` (for the normals cache).
/// ONE BODY AS THE SCENE SEES IT: where it stands, what colour it is, and how it is being shown.
///
/// It was `(usize, bool, bool, [u8; 3], &Mesh, [f64; 12])`, and the catalogue of debt names it as ITS OWN
/// example: to learn what the second `bool` meant one had to read the body of the function. Every caller
/// read it as `for (_, _, _, _, mesh, wt)` - four holes in a row, and getting one wrong would have drawn
/// the ghosts as solid.
pub struct SceneMesh<'a> {
    /// The mesh's index in `project.bodies`.
    pub index: usize,
    /// HIGHLIGHTED BY THE SELECTION. A body lights itself; a component lights its whole subtree.
    pub hot: bool,
    /// DRAWN AS A GHOST: a part outside the active context, or an operation's preview.
    pub ghost: bool,
    /// Its colour.
    pub tint: [u8; 3],
    /// The mesh itself.
    pub mesh: &'a qymcad_core::geom::Mesh,
    /// The WORLD transform of the owning component, with the gizmo's drag laid over it.
    pub world: [f64; 12],
}
/// THE COLOURS OF A BODY'S FACES, where a file coloured single faces of it: the distinct colours in the order the faces
/// meet them, and for every one of the mesh's `n_tris` triangles the place of its colour in that list - `None` where the
/// triangle takes the body's own. Both empty for a body with no face of a colour of its own, which is nearly every body;
/// a document with none at all answers without looking at a single face. Both painters draw from this one list, the
/// software raster and the device's scene, so they cannot tell a face's colour differently.
pub fn face_palette(project: &Project, mi: usize, n_tris: usize) -> (Vec<[u8; 3]>, Vec<Option<u8>>) {
    let none = (Vec::new(), Vec::new());
    if project.face_colors.is_empty() && project.tri_colors.is_empty() {
        return none;
    }
    let Some(body) = project.mesh_id(mi) else { return none };
    // a piece of a mesh whose file coloured it triangle by triangle. Whether it has a palette is the mesh's own answer,
    // whatever `n_tris` a caller passes, so the rows of the look table agree between the frame's table and the scene.
    if let Some((palette, places)) = project.tri_colors.get(&project.lineage_root(body)) {
        if places.len() == project.bodies[mi].mesh.tris.len() {
            let per = if n_tris == places.len() { places.iter().map(|&k| Some(k)).collect() } else { Vec::new() };
            return (palette.clone(), per);
        }
    }
    let Some(coloured) = project.face_colors.get(&project.lineage_root(body)).filter(|f| !f.is_empty()) else { return none };
    let faces = project.regen_faces.get(&body).map(|f| f.as_slice()).unwrap_or(project.bodies[mi].faces.as_slice());
    let (mut palette, mut per) = (Vec::new(), vec![None; n_tris]);
    for f in faces {
        let Some(c) = coloured.iter().find(|(id, _)| *id == f.id && f.id != 0).map(|(_, c)| *c) else { continue };
        let k = match palette.iter().position(|p| *p == c) {
            Some(k) => k,
            None if palette.len() < 255 => {
                palette.push(c);
                palette.len() - 1
            }
            None => continue, // past 255 colours a face takes the body's own
        };
        for &t in &f.triangles {
            if let Some(slot) = per.get_mut(t as usize) {
                *slot = Some(k as u8);
            }
        }
    }
    if palette.is_empty() {
        per.clear();
    }
    (palette, per)
}

/// THE TALLEST BODY ON SCREEN, along Z. Says whether the shapes came back after a reload.
pub fn tallest_body(pn: &Painting) -> f64 {
    visible_mesh_items(pn).iter().filter_map(|m| m.mesh.bounds()).map(|bb| bb.max.z - bb.min.z).fold(0.0, f64::max)
}

/// WHAT `body_shown` ACTUALLY DEPENDS ON, as one number - the key of the cache below.
///
/// The geometry revision alone is not enough: the answer also turns on the context, on the "show neighbours"
/// toggle, on a sketch being edited, on the feature being edited, and on every tick box in the tree. Keyed by
/// the revision alone, the list stayed stale until the next rebuild - switching "in context" changed nothing
/// on screen. Hashing two fields per body and per component costs microseconds on 1296 bodies.
pub(crate) fn shown_bodies_key(pn: &Painting) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    view_rev(pn.regen).hash(&mut h);
    current_ctx_id(pn.active_path, pn.project).hash(&mut h);
    pn.win.context.hash(&mut h);
    pn.sketch_ses.editing.is_some().hash(&mut h);
    pn.cmd.edit.hash(&mut h);
    pn.project.regen_errors.len().hash(&mut h);
    for b in &pn.project.bodies {
        (b.visible, b.mesh.tris.is_empty()).hash(&mut h);
    }
    for c in &pn.project.components {
        (c.visible, c.parent).hash(&mut h);
    }
    h.finish()
}

/// THE VISIBLE BODIES - as a list from a cache rather than recomputed for every body every frame.
///
/// Deciding whether a body is visible means finding its owner (a walk over the timeline) and following the
/// chain of tick boxes up the tree. That is not expensive in itself, but it repeats for EVERY body: in the
/// picking loop it hung the application while an edge anchor was being chosen, and in the scene it cost 175 ms
/// per frame on a document of 1296 bodies - felt as the whole program going sluggish, settings windows
/// included.
pub fn shown_bodies(pn: &Painting) -> Vec<(usize, Id)> {
    let key = shown_bodies_key(pn);
    let ctx = current_ctx_id(pn.active_path, pn.project);
    {
        let c = pn.cache.shown_bodies.borrow();
        if c.rev == key && c.value.ctx == ctx {
            return c.value.list.clone();
        }
    }
    let list: Vec<(usize, Id)> = (0..pn.project.bodies.len()).filter(|&mi| body_shown(pn.body_view(), mi)).filter_map(|mi| pn.project.mesh_id(mi).map(|b| (mi, b))).collect();
    pn.cache.shown_bodies.borrow_mut().put(key, ShownBodies { ctx, list: list.clone() });
    list
}

/// THE PATH OF CONTEXTS AFTER STEPPING INTO `cid` FROM `path`: one level deeper - or, for a clone, the way from the
/// root to its original. A clone has no timeline of its own; stepping into it is editing the part it repeats, and
/// that part may stand in another subassembly.
pub fn path_into(path: &[Id], project: &Project, cid: Id) -> Vec<Id> {
    let origin = project.instance_origin(cid);
    if origin == cid {
        let mut deeper = path.to_vec();
        deeper.push(cid);
        return deeper;
    }
    let mut chain = vec![origin];
    let mut cur = origin;
    // the parents up to the root; the bound guards a loop in a damaged document, no tree is that deep
    while let Some(parent) = project.components.iter().find(|c| c.id == cur).and_then(|c| c.parent) {
        if chain.len() > 256 {
            break;
        }
        chain.push(parent);
        cur = parent;
    }
    chain.reverse();
    chain
}

pub fn visible_mesh_items<'a>(pn: &'a Painting) -> Vec<SceneMesh<'a>> {
    // the selection highlight: a body highlights itself; a component highlights its whole subtree (the part or subassembly entire)
    let hl = highlight_mesh_set(pn.project, &pn.sel);
    // every mesh carries the WORLD transform of its owning component: a body is built in the part's
    // local frame, and `world_transform` (composed up the assembly tree) places it into the world.
    // While a modifier feature (shell, hole, fillet, chamfer and so on) is being edited, the state
    // BEFORE it is shown: its RESULT and its whole descendant chain are hidden. The consumed SOURCE
    // is shown by `body_shown`, which is also what throws consumed bodies away — keeping a SECOND
    // check of the same thing here was a mistake: the exception for the source ended up behind an
    // earlier refusal and never worked.
    let edit_hide = edit_hidden_bodies(pn.cmd, pn.project);
    let ctx = current_ctx_id(pn.active_path, pn.project); // the placement is RELATIVE to the active context (a part sits at the origin, an assembly in place)
                                                          // THE PLACEMENT IS ASKED ONCE PER OWNER, not once per body. `body_display_transform` inverts and
                                                          // multiplies its way up the component tree, looking every level up by a linear scan; on a document of
                                                          // 1296 bodies that pass is what the frame was being spent on. Bodies of one part share one answer.
    let mut placed: std::collections::HashMap<Id, [f64; 12]> = std::collections::HashMap::new();
    // WHOSE EACH BODY IS, in ONE pass over the timeline instead of one pass per body. Asked body by body
    // it is 1.7 million comparisons on a document of 1296 bodies, every frame.
    // THE FIRST NODE THAT OWNS THE BODY, exactly as `Project::body_owner` answers: a body travels through
    // the nodes that consume and remake it, and the later ones may sit in another component. Taking the
    // last one instead turned parts into ghosts of a foreign context and placed them by the wrong frame.
    let mut owner_of: std::collections::HashMap<Id, Option<Id>> = std::collections::HashMap::new();
    for n in &pn.project.timeline {
        for b in n.kind.bodies() {
            owner_of.entry(b).or_insert(n.parent);
        }
    }
    // A GHOST IS A PROPERTY OF THE OWNER, not of the body: every body of one part answers alike.
    let root = pn.project.root;
    let mut ghostly: std::collections::HashMap<Id, bool> = std::collections::HashMap::new();
    shown_bodies(pn)
            .into_iter()
            .filter(|(_, b)| !edit_hide.contains(b)) // the feature being edited plus its descendant chain
            .filter_map(|(mi, b)| pn.project.bodies.get(mi).map(|body| (mi, b, &body.mesh)))
            .map(|(mi, id, m)| {
                let owner = owner_of.get(&id).copied().flatten();
                let ghost = match owner {
                    Some(o) if ctx != root => *ghostly.entry(o).or_insert_with(|| !pn.project.component_is_within(o, ctx)),
                    _ => false,
                };
                let mut wt = match owner {
                    Some(o) => *placed.entry(o).or_insert_with(|| pn.project.relative_transform(o, ctx)),
                    None => qymcad_core::feature::PLACE_IDENTITY,
                };
                // THE BODY GIZMO'S PREVIEW: while dragging, the accumulated transform is laid over this body
                // (with no B-rep rebuild until release). Ctrl snaps.
                if let Some((dmi, _, _)) = pn.body_giz.drag {
                    if dmi == mi {
                        if let Some(accum) = body_giz_accum(pn.body_giz, pn.set, pn.body_giz.snap) {
                            wt = compose12(&accum, &wt);
                        }
                    }
                }
                SceneMesh {
                    index: mi,
                    hot: hl.contains(&mi),
                    ghost,
                    tint: pn.project.mesh_color(mi),
                    mesh: m,
                    world: wt,
                }
            })
            .collect()
}

/// The composition of two 3x4 affine transforms (row-major): the result is `a` after `b` (apply b, then a). Used by the body gizmo.
pub fn compose12(a: &[f64; 12], b: &[f64; 12]) -> [f64; 12] {
    let al = [a[0], a[1], a[2], a[4], a[5], a[6], a[8], a[9], a[10]];
    let bl = [b[0], b[1], b[2], b[4], b[5], b[6], b[8], b[9], b[10]];
    let (tb, ta) = ([b[3], b[7], b[11]], [a[3], a[7], a[11]]);
    let mut l = [0.0; 9];
    for r in 0..3 {
        for c in 0..3 {
            l[r * 3 + c] = al[r * 3] * bl[c] + al[r * 3 + 1] * bl[3 + c] + al[r * 3 + 2] * bl[6 + c];
        }
    }
    let t = [
        al[0] * tb[0] + al[1] * tb[1] + al[2] * tb[2] + ta[0],
        al[3] * tb[0] + al[4] * tb[1] + al[5] * tb[2] + ta[1],
        al[6] * tb[0] + al[7] * tb[1] + al[8] * tb[2] + ta[2],
    ];
    [l[0], l[1], l[2], t[0], l[3], l[4], l[5], t[1], l[6], l[7], l[8], t[2]]
}

/// The indices of the bodies highlighted as "selected" in 3D. Select a body and that body lights up;
/// select a component (a part or a subassembly) in the tree or by a click in the assembly and its
/// WHOLE subtree lights up, so that it is plain that this part or subassembly was selected entire.
pub fn highlight_mesh_set(project: &Project, sel: &Sel) -> std::collections::HashSet<usize> {
    let mut hl = std::collections::HashSet::new();
    match *sel {
        Sel::Mesh(i) | Sel::Face(i, _) => {
            hl.insert(i);
        }
        Sel::Component(ci) => {
            if let Some(cid) = project.components.get(ci).map(|c| c.id) {
                let mut sub: std::collections::HashSet<Id> = project.descendants(cid).into_iter().collect();
                sub.insert(cid);
                for (mi, b) in project.bodies.iter().map(|b| b.id).enumerate() {
                    if project.body_owner(b).is_some_and(|o| sub.contains(&o)) {
                        hl.insert(mi);
                    }
                }
            }
        }
        _ => {}
    }
    hl
}

/// The transform accumulated by a body gizmo over the current drag (in the Part's world frame): a shift
/// along an axis OR a rotation about a ring through the fixed origin. `snap` rounds to the step (the
/// grid, or the angle). None means there is no drag.
pub fn body_giz_accum(body_giz: &BodyGizmo, set: &Settings, snap: bool) -> Option<[f64; 12]> {
    let (_, o, amt) = body_giz.drag?;
    if let Some(ax) = body_giz.axis {
        let step = body_snap_mm(set);
        let d = if snap { (amt / step).round() * step } else { amt };
        let mut t = qymcad_core::feature::PLACE_IDENTITY;
        t[[3, 7, 11][ax as usize]] = d; // a shift along the world axis `ax`
        Some(t)
    } else if let Some(ax) = body_giz.ring {
        let deg = if snap { (amt / body_snap_deg(set)).round() * body_snap_deg(set) } else { amt };
        Some(rot_about_point(ax, deg, o))
    } else {
        None
    }
}

/// A rotation by `deg` about the WORLD axis `ax` (0 = X, 1 = Y, 2 = Z) through the point `o`, as a 3x4
/// affine transform. Used by the body gizmo.
pub fn rot_about_point(ax: u8, deg: f64, o: [f64; 3]) -> [f64; 12] {
    let (s, c) = deg.to_radians().sin_cos();
    let r = match ax {
        0 => [1.0, 0.0, 0.0, 0.0, c, -s, 0.0, s, c],
        1 => [c, 0.0, s, 0.0, 1.0, 0.0, -s, 0.0, c],
        _ => [c, -s, 0.0, s, c, 0.0, 0.0, 0.0, 1.0],
    };
    // the translation is o - R*o, which keeps the centre still
    let ro = [r[0] * o[0] + r[1] * o[1] + r[2] * o[2], r[3] * o[0] + r[4] * o[1] + r[5] * o[2], r[6] * o[0] + r[7] * o[1] + r[8] * o[2]];
    [r[0], r[1], r[2], o[0] - ro[0], r[3], r[4], r[5], o[1] - ro[1], r[6], r[7], r[8], o[2] - ro[2]]
}

/// The snapping step for moving a body (mm): the grid step from the snapping panel.
pub fn body_snap_mm(set: &Settings) -> f64 {
    set.snap.grid.max(0.01)
}

/// The snapping step for rotating a body (deg): the rotation field from the snapping panel.
pub fn body_snap_deg(set: &Settings) -> f64 {
    set.snap.rot_deg.max(0.1)
}
