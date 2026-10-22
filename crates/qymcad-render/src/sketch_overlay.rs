//! A SKETCH ON THE CANVAS: its curves moved, trimmed, clipped, its grid, its regions, contours and constraints.

use super::*;

/// Draw a ghost of entity `eid`, transforming its points with the function `f` (world -> world). Shared by
/// the move/copy preview (the selected entities shifted by cursor minus base) and by the pattern preview.
pub fn draw_entity_xform(pick: &PickCtx, painter: &egui::Painter, rect: Rect, si: usize, eid: Id, f: &dyn Fn(f64, f64) -> (f64, f64), stroke: Stroke) {
    let sh = qymcad_ui_state::Sheet { view: *pick.view, rect };
    let Some(s) = pick.project.sketches.get(si) else { return };
    let Some(e) = s.entities.iter().find(|e| e.id == eid) else {
        // a point that stands alone is a shape of its own: its ghost is a dot where the map takes it
        if let Some(q) = s.points.iter().find(|q| q.id == eid) {
            let (x, y) = f(q.x, q.y);
            painter.circle_stroke(sh.at(Point2::new(x, y)), 3.5, stroke);
        }
        return;
    };
    // the curve laid out where it stands, each of its points taken through the map
    let Some(curve) = qymcad_core::model::curve::Curve::of(s, e) else { return };
    let line: Vec<Pos2> = curve.polyline().into_iter().map(|p| f(p.x, p.y)).map(|(x, y)| sh.at(Point2::new(x, y))).collect();
    painter.add(egui::Shape::line(line, stroke));
}

pub fn active_sketch_contour_ids(armed: &qymcad_ui_state::Armed, cmd: &FeatCommand, project: &Project, sketch_ses: SketchSession) -> Option<std::collections::HashSet<Id>> {
    sketch_ses
        .editing
        .and_then(|sid| project.sketches.iter().find(|s| s.id == sid))
        .map(|s| s.contour_ids.iter().copied().collect())
        .or_else(|| (armed.commanding()).then(|| cmd.sketch.and_then(|si| project.sketches.get(si))).flatten().map(|s| s.contour_ids.iter().copied().collect()))
}

pub fn hidden_contour_ids(project: &Project, sketch_hidden: &std::collections::HashSet<Id>) -> std::collections::HashSet<Id> {
    project.sketches.iter().filter(|s| sketch_hidden.contains(&s.id)).flat_map(|s| s.contour_ids.iter().copied()).collect()
}

/// WHETHER THE SHARED TOGGLE IS HIDING THE OUTLINES RIGHT NOW.
///
/// It exists for the ASSEMBLY only: there the sketches number in the dozens across all the components and
/// hiding them one by one is impractical. Inside a Part there is no such toggle - every sketch has a
/// checkbox of its own there, and a second, shared switch would duplicate it (and once already hid what
/// had just been asked to be shown).
///
/// The outlines of the sketch CURRENTLY IN HAND are the ones being edited in the sketcher or picked as a
/// Part command's profile. `None` means neither, and the screen simply shows the model. This lives in one
/// method because it decides not only the "show only this one" filter but also whether the shared toggle
/// may hide it - and two places computing the same thing each in their own way drift apart sooner or later.
pub fn contours_switched_off(set: &Settings, workbench: Workbench) -> bool {
    matches!(workbench, Workbench::Assembly) && !set.show_contours
}

/// A translucent grid in the background + the highlighted axis lines through the origin.
pub fn draw_sketch_grid(scheme: &SchemeUi, set: &Settings, view: View2d, painter: &egui::Painter, rect: Rect) {
    let sh = qymcad_ui_state::Sheet { view, rect };
    let tl = qymcad_ui_state::to_world(&view, rect, rect.min);
    let br = qymcad_ui_state::to_world(&view, rect, rect.max);
    let (x0, x1) = (tl.x.min(br.x), tl.x.max(br.x));
    let (y0, y1) = (tl.y.min(br.y), tl.y.max(br.y));
    // a grid step whose on-screen interval is at least ~8 px
    let mut step = set.snap.grid.max(0.1);
    let sc = view.scale as f64;
    while step * sc < 8.0 {
        step *= 5.0;
    }
    // THE GRID COMES FROM THE PALETTE. It used to be translucent white: on a light background such a grid cannot be seen at all.
    let g = scheme.pal.grid();
    let minor = Stroke::new(1.0, qymcad_scheme::a(g, 40));
    let major = Stroke::new(1.0, qymcad_scheme::a(g, 90));
    let (kx0, kx1) = ((x0 / step).floor() as i64, (x1 / step).ceil() as i64);
    let (ky0, ky1) = ((y0 / step).floor() as i64, (y1 / step).ceil() as i64);
    if (kx1 - kx0) < 600 && (ky1 - ky0) < 600 {
        for k in kx0..=kx1 {
            let x = k as f64 * step;
            let st = if k % 5 == 0 { major } else { minor };
            painter.line_segment([sh.at(Point2::new(x, y0)), sh.at(Point2::new(x, y1))], st);
        }
        for k in ky0..=ky1 {
            let y = k as f64 * step;
            let st = if k % 5 == 0 { major } else { minor };
            painter.line_segment([sh.at(Point2::new(x0, y)), sh.at(Point2::new(x1, y))], st);
        }
    }
    // the axis lines are drawn by draw_axes ALONE (nothing is duplicated here)
}

/// The ghost of an insertion of geometry from the clipboard: green copies of the clipboard
/// entities following the cursor so that the anchor point coincides with it. Drawn straight from
/// the clipboard data (the entities are not in the sketch yet).
pub fn draw_clip_ghost(clip: &Clipboard, cursor: Option<Point2>, scheme: &SchemeUi, view: View2d, painter: &egui::Painter, rect: Rect) {
    let sh = qymcad_ui_state::Sheet { view, rect };
    if !clip.geom_place {
        return;
    }
    let Some(clip) = clip.geom.as_ref() else { return };
    let Some(cur) = cursor else { return };
    let (ox, oy) = (cur.x - clip.ref_x, cur.y - clip.ref_y);
    let col = scheme.pal.clip();
    let stroke = Stroke::new(1.4, col);
    let pt = |id: Id| clip.points.iter().find(|(pid, ..)| *pid == id).map(|(_, x, y)| Point2::new(x + ox, y + oy));
    // each curve of the clipboard where it would land: its points shifted
    for e in &clip.entities {
        if let Some(curve) = qymcad_core::model::curve::Curve::of_placed(&pt, e) {
            painter.add(egui::Shape::line(curve.polyline().into_iter().map(|p| sh.at(p)).collect(), stroke));
        }
    }
    painter.circle_filled(sh.at(cur), 3.0, col);
}

/// THE TRIANGLES OF A REGION OF A SKETCH, in the sketch's plane: contour `cid` minus its direct children - the material
/// an extrusion takes from it (`Project::feature_holes`). Regions told apart this way never overlap, so their fills do
/// not add up where contours nest.
pub fn region_triangles(project: &qymcad_core::model::Project, cid: Id) -> std::rc::Rc<[[Point2; 3]]> {
    let Some(ci) = project.contour_index(cid) else { return std::rc::Rc::from(Vec::new()) };
    let outer = &project.contours[ci];
    if !outer.closed || outer.points.len() < 3 {
        return std::rc::Rc::from(Vec::new());
    }
    let holes: Vec<&[Point2]> = project
        .sketch_of_contour(cid)
        .map(|sid| project.feature_holes(sid, cid))
        .unwrap_or_default()
        .into_iter()
        .filter_map(|h| project.contour_index(h).map(|hi| &project.contours[hi].points[..]))
        .collect();
    // THE TRIANGLES ARE REMEMBERED BY THE POINTS THEY ARE MADE OF: the fill of a picked region and of the one under the
    // pointer is drawn every frame, and laid anew each frame a ring between a closed spline and its offset, 14 600 points,
    // took 0.8 s a frame - the pointer over the sketch hung the picking of a region. The same outline answers the same
    // triangles; the last few are kept.
    let key = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for ring in std::iter::once(&outer.points[..]).chain(holes.iter().copied()) {
            ring.len().hash(&mut h);
            for q in ring {
                q.x.to_bits().hash(&mut h);
                q.y.to_bits().hash(&mut h);
            }
        }
        h.finish()
    };
    if let Some(tris) = REGION_TRIANGLES.with(|m| m.borrow().iter().find(|r| r.key == key).map(|r| r.tris.clone())) {
        return tris;
    }
    // the fill is laid over points no farther apart than the eye tells: a ring laid out to 0.005 mm took 0.8 s to fill
    // the first time, the cutting into triangles growing as the square of its points
    let (lo, hi) = outer.points.iter().fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), q| ([lo[0].min(q.x), lo[1].min(q.y)], [hi[0].max(q.x), hi[1].max(q.y)]));
    let tol = (0.02f64).max(1e-4 * (hi[0] - lo[0]).hypot(hi[1] - lo[1]));
    let owned: Vec<Vec<Point2>> = holes.iter().map(|h| thinned(h, tol)).collect();
    let tris: std::rc::Rc<[[Point2; 3]]> = std::rc::Rc::from(qymcad_core::geom::triangulate_with_holes(&thinned(&outer.points, tol), &owned));
    REGION_TRIANGLES.with(|m| {
        let mut m = m.borrow_mut();
        if m.len() >= REMEMBERED_REGIONS {
            m.remove(0);
        }
        m.push(RememberedRegion { key, tris: tris.clone() });
    });
    tris
}

/// THE POINTS OF A RING THINNED TO `tol`: a point is kept where the points since the last one kept would stand farther
/// than `tol` off the chord to the next.
fn thinned(ring: &[Point2], tol: f64) -> Vec<Point2> {
    let Some(&first) = ring.first() else { return Vec::new() };
    let mut out = vec![first];
    let mut since: Vec<Point2> = Vec::new();
    for w in ring.windows(2) {
        let (a, next) = (out[out.len() - 1], w[1]);
        let (dx, dy) = (next.x - a.x, next.y - a.y);
        let len = dx.hypot(dy).max(1e-12);
        let off = since.iter().map(|q| ((q.x - a.x) * dy - (q.y - a.y) * dx).abs() / len).fold(0.0, f64::max);
        if off > tol {
            out.push(w[0]);
            since.clear();
        }
        since.push(w[1]);
    }
    if let Some(&last) = ring.last() {
        if out.last().is_none_or(|q| q.x != last.x || q.y != last.y) {
            out.push(last);
        }
    }
    if out.len() < 3 {
        return ring.to_vec();
    }
    out
}

/// How many regions the triangles are remembered of: the picked ones and the one under the pointer.
const REMEMBERED_REGIONS: usize = 16;

/// THE TRIANGLES OF A REGION by the key of the points they are made of.
struct RememberedRegion {
    key: u64,
    tris: std::rc::Rc<[[Point2; 3]]>,
}

thread_local! {
    static REGION_TRIANGLES: std::cell::RefCell<Vec<RememberedRegion>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// FILL THE TRIANGLES OF A REGION, each corner placed on the screen by `at`.
pub(crate) fn fill_region(painter: &egui::Painter, tris: &[[Point2; 3]], at: impl Fn(Point2) -> Pos2, colour: Color32) {
    let mut mesh = egui::Mesh::default();
    for t in tris {
        let base = mesh.vertices.len() as u32;
        for p in t {
            mesh.colored_vertex(at(*p), colour);
        }
        mesh.add_triangle(base, base + 1, base + 2);
    }
    if !mesh.is_empty() {
        painter.add(egui::Shape::mesh(mesh));
    }
}

/// THE REGION UNDER THE POINTER that a click would take: a contour of the slot of a sweep or a loft being picked, or
/// of the sketch of the running command - by the rule the click picks by (`slot_contour_under_2d`,
/// `contour_under_2d`), so what is lit is what the click takes.
pub(crate) fn region_under(pn: &Painting, rect: Rect, at: Point2) -> Option<Id> {
    let screen = qymcad_ui_state::Sheet { view: pn.view, rect }.at(at);
    match pn.picking.contour() {
        Some(slot) => {
            let pick = qymcad_ui_state::PickCtx { project: pn.project, set: pn.set, view: &pn.view };
            qymcad_ui_state::slot_contour_under_2d(&pick, rect, screen, &qymcad_ui_state::slot_candidates(pn.loft, pn.project, pn.sweep, slot))
        }
        None => qymcad_ui_state::contour_under_2d(pn.project, &pn.view, rect, screen, pn.cmd.sketch?),
    }
}

/// THE REGIONS OF A PROFILE PICKER, FILLED: the picked ones in the colour of a profile, the one under the pointer
/// lighter, both under the lines. The owner's request: a picked region shown only by its outline did not tell a ring
/// from its rectangle, nor which region a click would take.
pub(crate) fn fill_picked_regions(pn: &Painting, painter: &egui::Painter, rect: Rect) {
    let sheet = qymcad_ui_state::Sheet { view: pn.view, rect };
    let picked: Vec<Id> = if pn.armed.commanding() { pn.gsel.profiles.iter().copied().collect() } else { Vec::new() };
    let slot: Vec<Id> = pn.picking.contour().map(|s| qymcad_ui_state::slot_current_cid(pn.loft, pn.project, pn.sweep, s)).into_iter().collect();
    for cid in picked.iter().chain(&slot) {
        fill_region(painter, &region_triangles(pn.project, *cid), |p| sheet.at(p), qymcad_scheme::a(pn.scheme.pal.contour_profile(), 110));
    }
    let picking = pn.armed.commanding() || pn.picking.contour().is_some();
    if let (true, Some(at)) = (picking, pn.cursor) {
        if let Some(cid) = region_under(pn, rect, at).filter(|c| !picked.contains(c) && !slot.contains(c)) {
            fill_region(painter, &region_triangles(pn.project, cid), |p| sheet.at(p), qymcad_scheme::a(pn.scheme.pal.contour_hover(), 90));
        }
    }
}

pub fn draw_contours(pn: &Painting, painter: &egui::Painter, rect: Rect) {
    // while a sketch is being edited, ONLY its outlines are shown (the neighbouring sketches and the
    // part's original one do not get in the way); outside editing, the filter goes by owner (sketch
    // isolation, other components hidden). The half-sketcher of a Part command (feat_cmd, not editing)
    // behaves the same way: the outlines of other sketches are hidden - they get in the way of picking a
    // closed contour.
    let edit_only = active_sketch_contour_ids(pn.armed, pn.cmd, pn.project, pn.sketch_ses);
    // The sketch being edited is ALWAYS visible: that is why it was opened. The rest go by their own
    // checkbox, and in an assembly by the shared toggle as well.
    let edit_only_none = edit_only.is_none();
    if edit_only_none && contours_switched_off(pn.set, pn.workbench) {
        return;
    }
    let hidden_cids = hidden_contour_ids(pn.project, pn.sketch_hidden);
    // THE REGIONS ARE FILLED, under the lines: the picked ones, and the one a click would take
    if !edit_only_none {
        fill_picked_regions(pn, painter, rect);
    }
    // the contours picked by the active operation are brighter
    let selected: &[Id] = &[];
    let hovered = qymcad_ui_state::hovered_contour(pn.cursor, pn.project, pn.view);
    let obj_sel = if let Sel::Contour(i) = pn.sel { Some(i) } else { None };
    let base = Stroke::new(1.5, pn.scheme.pal.contour_idle());
    let hot = Stroke::new(2.5, pn.scheme.pal.active());
    let obj = Stroke::new(2.5, pn.scheme.pal.ok());
    let hover = Stroke::new(2.0, pn.scheme.pal.contour_hover());
    let foreign_cids = qymcad_pick::foreign_contour_ids(pn);
    // the contour of the active sweep or loft slot (lit in the half-sketcher, like a picked profile)
    let pick_cid = pn.picking.contour().map(|s| qymcad_ui_state::slot_current_cid(pn.loft, pn.project, pn.sweep, s));
    for (i, c) in pn.project.contours.iter().enumerate() {
        if c.points.len() < 2 {
            continue;
        }
        let cid = pn.project.contour_id(i);
        if edit_only.is_none() && cid.is_some_and(|id| hidden_cids.contains(&id)) {
            continue; // the sketch is hidden by its own checkbox in the tree
        }
        match &edit_only {
            Some(only) if !cid.is_some_and(|id| only.contains(&id)) => continue,  // not the current sketch
            None if cid.is_some_and(|id| foreign_cids.contains(&id)) => continue, // another component
            _ => {}
        }
        let mut pts: Vec<Pos2> = c.points.iter().map(|p| (qymcad_ui_state::Sheet { view: pn.view, rect }).at(*p)).collect();
        if c.closed {
            pts.push(pts[0]);
        }
        // the profile is picked for a command (extrude, cut, revolve): its REGION is filled above - the contour minus its
        // direct children, so nested regions never overlap and their fills do not add up into a pale wash, which is
        // what filling whole contours did - and its outline is drawn bright and thick over the fill.
        let is_profile = pn.armed.commanding() && pn.project.contour_id(i).is_some_and(|id| pn.gsel.profiles.contains(&id) || pick_cid == Some(id));
        let stroke = if is_profile {
            Stroke::new(3.0, pn.scheme.pal.contour_profile())
        } else if obj_sel == Some(i) {
            obj
        } else if pn.project.contour_id(i).is_some_and(|id| selected.contains(&id)) {
            hot
        } else if hovered == Some(i) {
            hover
        } else {
            base
        };
        painter.add(egui::Shape::line(pts, stroke));
    }
}

/// Highlighting the edges of the reference body in the 2D sketcher - the edges only (no fill, no 3D body).
pub fn draw_sketch_face_edges(pn: &Painting, painter: &egui::Painter, rect: Rect) {
    let Some(si) = qymcad_ui_state::edit_si(pn.project, &pn.sketch_ses) else { return };
    let col = qymcad_scheme::a(pn.scheme.pal.sketch_face_edge(), 140);
    for poly in qymcad_pick::sketch_ref_edges_2d(pn.cache, pn.cmd, pn.live, pn.project, pn.regen, si) {
        let pts: Vec<Pos2> = poly.iter().map(|p| (qymcad_ui_state::Sheet { view: pn.view, rect }).at(*p)).collect();
        for k in 0..pts.len().saturating_sub(1) {
            painter.line_segment([pts[k], pts[k + 1]], Stroke::new(1.0, col));
        }
    }
}

/// Highlighting the selected entities + the glyphs of the geometric constraints.
pub fn draw_sketch_constraints(pn: &Painting, painter: &egui::Painter, rect: Rect, si: usize) {
    let sh = qymcad_ui_state::Sheet { view: pn.view, rect };
    let Some(s) = pn.project.sketches.get(si) else { return };
    // a table, built only when a point is looked for: with nothing hovered, selected or arguing, none is
    let points_by_id: std::cell::OnceCell<std::collections::HashMap<Id, &qymcad_core::model::SketchPoint>> = std::cell::OnceCell::new();
    let pt = |id: Id| points_by_id.get_or_init(|| s.points.iter().map(|p| (p.id, p)).collect()).get(&id).copied().map(|p| Point2::new(p.x, p.y));
    // hovering a constraint (its glyph or its row in the list) lights the points and edges it holds
    if let Some(ci) = pn.hover.constraint {
        let pts = pn.project.sketch_constraint_points(si, ci);
        let hc = pn.scheme.pal.active();
        for id in &pts {
            if let Some(p) = pt(*id) {
                painter.circle_stroke(sh.at(p), 6.0, Stroke::new(1.8, hc));
            }
        }
        // edges with both ends in the set are lit as a line
        for e in &s.entities {
            if let Some(qymcad_core::model::LineParts { a, b }) = e.kind.as_line() {
                if pts.contains(&a) && pts.contains(&b) {
                    if let (Some(pa), Some(pb)) = (pt(a), pt(b)) {
                        painter.line_segment([sh.at(pa), sh.at(pb)], Stroke::new(2.0, hc));
                    }
                }
            }
        }
    }
    // highlighting the selected entities (white) and the hovered ones (blue, thinner)
    for e in &s.entities {
        let sel = pn.sel_sk.items.contains(&qymcad_ui_state::SketchItem::curve(e.id));
        let hov = pn.hover.sketch == Some(qymcad_ui_state::SketchItem::curve(e.id));
        if !sel && !hov {
            continue;
        }
        let hl = if sel { Stroke::new(2.5, pn.scheme.pal.emphasis()) } else { Stroke::new(2.0, pn.scheme.pal.preview()) };
        // ONE HIGHLIGHT FOR EVERY KIND: the curve as the curve module lays it out
        let Some(curve) = qymcad_core::model::curve::Curve::of(s, e) else { continue };
        painter.add(egui::Shape::line(curve.polyline().into_iter().map(|p| sh.at(p)).collect(), hl));
        // A LINE PICKED AT ITS MIDDLE SHOWS IT: a ring of the snap's colour round a dot of the selection's, at the middle -
        // lit as any picked line, the middle taken was not seen
        if let (Some(line), true) = (e.kind.as_line(), sel && pn.sel_sk.at_middle.contains(&e.id)) {
            if let (Some(pa), Some(pb)) = (pt(line.a), pt(line.b)) {
                let mid = sh.at(Point2::new((pa.x + pb.x) / 2.0, (pa.y + pb.y) / 2.0));
                painter.circle_stroke(mid, MIDDLE_RING, Stroke::new(1.6, pn.scheme.pal.snap_marker()));
                painter.circle_filled(mid, 2.5, pn.scheme.pal.emphasis());
            }
        }
    }
    // REDUNDANT constraints are marked SPECIFICALLY (an orange-yellow badge); one of them can be deleted
    let diag = qymcad_ui_state::sketch_diag(pn.cache, pn.project, si);
    // MARKED BY THE SAME RULE AS THE CONSTRAINT LIST. This used to be the raw `diag.redundant`, and the
    // canvas diverged from the list: on a slot (two semicircles and two tangents) the list was clean while
    // the tangency glyphs burned with "redundant constraint". The rank analysis marks tangencies falsely -
    // their Jacobian at the point of tangency is parallel to the arc's intrinsic.
    let redundant = qymcad_ui_state::flagged_redundant(pn.cache, pn.project, si);
    // THE ARGUING SET: the geometry held by conflicting constraints, in red. Otherwise a conflict shows
    // only in the panel and on the dimensions, and which part of the sketch does not solve has to be
    // guessed. Constraints ALWAYS argue several at a time, so the whole set is lit rather than one culprit.
    if !diag.conflicts.is_empty() {
        let cc = pn.scheme.pal.error();
        let mut hot: std::collections::HashSet<Id> = std::collections::HashSet::new();
        for &ci in &diag.conflicts {
            hot.extend(pn.project.sketch_constraint_points(si, ci));
        }
        for e in &s.entities {
            // a curve wholly inside the set - all the points it stands on - is drawn red; otherwise only the participating
            // points
            if qymcad_core::model::entity_points(e).iter().all(|id| hot.contains(id)) {
                if let Some(curve) = qymcad_core::model::curve::Curve::of(s, e) {
                    painter.add(egui::Shape::line(curve.polyline().into_iter().map(|p| sh.at(p)).collect(), Stroke::new(2.2, cc)));
                }
            }
        }
        for id in &hot {
            if let Some(p) = pt(*id) {
                painter.circle_stroke(sh.at(p), 5.0, Stroke::new(2.0, cc));
            }
        }
    }
    for (ci, at, g) in qymcad_pick::visible_constraint_glyphs(pn, rect, si) {
        let bg = if Some(ci) == pn.gsel.constraint {
            pn.scheme.pal.constraint_selected()
        } else if Some(ci) == pn.hover.constraint {
            pn.scheme.pal.constraint_hover()
        } else if diag.conflicts.contains(&ci) {
            pn.scheme.pal.error() // arguing: red (an error), which outranks redundancy
        } else if redundant.contains(&ci) {
            pn.scheme.pal.warning() // redundant: orange-yellow
        } else {
            pn.scheme.pal.constraint_ok()
        };
        let r = Rect::from_center_size(at, egui::vec2(15.0, 15.0));
        painter.rect_filled(r, 2.0, bg);
        paint_gly(painter, at, 4.5, g, pn.scheme.pal.glyph_text());
    }
}

/// The radius of the ring that marks a line picked at its middle, px.
pub const MIDDLE_RING: f32 = 6.0;

/// THE COPY THE OFFSET IN HAND WOULD MAKE, on the side of the pointer, at the distance of the bar - what Enter makes.
pub(crate) fn draw_offset_preview(pn: &Painting, painter: &egui::Painter, sh: &qymcad_ui_state::Sheet) {
    if pn.sel_sk.modify != Some(qymcad_ui_state::EditTool::Offset) {
        return;
    }
    // the side a click fixed, in the colour of a fixed corner; before the click, the side of the pointer
    let fixed = pn.sel_sk.offset.at;
    let (Some(si), Some(cur)) = (qymcad_ui_state::edit_si(pn.project, &pn.sketch_ses), fixed.or(pn.cursor)) else { return };
    let eids: Vec<Id> = pn.sel_sk.curves().collect();
    if eids.is_empty() {
        return;
    }
    let dist = pn.project.offset_toward(si, &eids, pn.tool_prefs.offset, cur);
    let stroke = Stroke::new(1.5, if fixed.is_some() { pn.scheme.pal.preview_corner_fixed() } else { pn.scheme.pal.preview_corner_new() });
    for pl in pn.project.offset_preview(si, &eids, dist) {
        painter.add(egui::Shape::line(pl.into_iter().map(|p| sh.at(p)).collect(), stroke));
    }
}
