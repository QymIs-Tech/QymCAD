//! EDITING A SKETCH: what is under the pointer, contours, boxes, the labels of constraints.

use crate::Notice;
use super::*;

/// Select all the geometry of a sketch: entities (lines, arcs, circles), primitives and free points
/// (apart from the system ones — the origin and the axes). Used by Ctrl+A.
pub fn select_all_sketch(annot: &mut AnnotEdit, gsel: &mut GeomSelection, project: &Project, sel_sk: &mut SketchSelection, status: &mut Notice, si: usize) {
    let Some(s) = project.sketches.get(si) else { return };
    let sys: std::collections::HashSet<Id> = s.system_ids().into_iter().collect();
    // points already taken by entities and primitives: they are not added as separate points
    let mut used: std::collections::HashSet<Id> = std::collections::HashSet::new();
    let mut sel: Vec<(u8, Id)> = Vec::new();
    for e in &s.entities {
        sel.push((1, e.id));
        match e.kind {
            qymcad_core::model::EntityKind::Line { a, b } => {
                used.insert(a);
                used.insert(b);
            }
            qymcad_core::model::EntityKind::Circle { center, .. } => {
                used.insert(center);
            }
            qymcad_core::model::EntityKind::Arc { center, a, b, .. } => {
                used.insert(center);
                used.insert(a);
                used.insert(b);
            }
            qymcad_core::model::EntityKind::Ellipse { c, ma, mi } => {
                used.insert(c);
                used.insert(ma);
                used.insert(mi);
            }
        }
    }
    // lone points (not part of an entity and not system ones)
    for p in &s.points {
        if !used.contains(&p.id) && !sys.contains(&p.id) {
            sel.push((0, p.id));
        }
    }
    let n = sel.len();
    sel_sk.items = sel;
    gsel.constraint = None;
    annot.note = None;
    *status = Notice::info(qymcad_i18n::tr1("g-selected-n", "n", &n.to_string()));
}

/// Delete the current sketch selection (entities, primitives and points, apart from the system ones).
pub fn delete_sketch_sel(project: &mut Project, regen: &mut Rebuilding, sel_sk: &mut SketchSelection, status: &mut Notice, si: usize) {
    let eids: Vec<Id> = sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
    let eids: Vec<Id> = {
        let s = &project.sketches[si];
        eids.into_iter().filter(|e| s.entities.iter().any(|x| x.id == *e)).collect()
    };
    let sys: std::collections::HashSet<Id> = {
        let s = &project.sketches[si];
        s.immovable_points().into_iter().collect() // system and driven points: they are not deleted one by one
    };
    let pids: Vec<Id> = sel_sk.items.iter().filter(|(k, id)| *k == 0 && !sys.contains(id)).map(|(_, id)| *id).collect();
    // A PIECE OF A PROJECTION DELETED cuts it: the rest is ordinary geometry held by what it plainly is
    // (`delete_sketch_curves`)
    if !eids.is_empty() {
        project.delete_sketch_curves(si, &eids);
    }
    if !pids.is_empty() {
        project.delete_points(si, &pids);
    }
    project.solve_sketch(si);
    sel_sk.clear(); // the selection, and whatever was waiting on it
    invalidate(regen);
    *status = Notice::info(qymcad_i18n::tr("g-deleted"));
}

/// Turn a reference object into a REAL point (creating the midpoint or the origin if needed) -> its Id.
pub fn materialize_ref(project: &mut Project, si: usize, r: SketchRef) -> Id {
    use qymcad_core::model::{Constraint, SketchPoint};
    match r {
        SketchRef::Point(id) => id,
        SketchRef::Origin => project.ensure_origin(si),
        SketchRef::Midpoint(a, b) => {
            // reuse an existing midpoint if a Midpoint constraint is already there
            if let Some(p) = project.sketches[si].constraints.iter().find_map(|c| match c {
                Constraint::Midpoint { p, a: ca, b: cb } if (*ca == a && *cb == b) || (*ca == b && *cb == a) => Some(*p),
                _ => None,
            }) {
                return p;
            }
            let (pa, pb) = (sketch_pt(project, si, a), sketch_pt(project, si, b));
            let (mx, my) = match (pa, pb) {
                (Some(pa), Some(pb)) => ((pa.x + pb.x) * 0.5, (pa.y + pb.y) * 0.5),
                _ => (0.0, 0.0),
            };
            let id = project.alloc_id();
            project.sketches[si].points.push(SketchPoint { id, x: mx, y: my });
            project.sketches[si].constraints.push(Constraint::Midpoint { p: id, a, b });
            id
        }
    }
}

pub fn move_body_at(rc: &mut RebuildCtx, mi: usize, mat: [f64; 12]) {
    begin_edit(rc.edits, rc.project, qymcad_i18n::tr("status-move-body")); // THE OPERATION BOUNDARY
                                                                           // a body of a solid or of a raw mesh alike: a node of the timeline, the move made by the rebuild - of a mesh in the
                                                                           // document, its faces carried with it
    let movable = brep_at(rc.live, rc.project, mi).or_else(|| rc.project.mesh_id(mi).filter(|b| rc.project.mesh_born(*b)));
    if let Some(id) = movable {
        let nb = rc.project.add_move(id, mat);
        mark_dirty_for_rebuild(rc); // the document is marked; the scheduler does the computing
        select_body(rc.project, rc.sel, rc.view, nb);
    }
    commit_edit(rc);
}

pub fn constraint_label(c: &qymcad_core::model::Constraint) -> String {
    use qymcad_core::model::Constraint as C;
    // A DIMENSION'S VALUE: either a number, or "expression = number". The expression is typed by hand and is kept as it is.
    let value = |num: String, expr: &str| if expr.trim().is_empty() { num } else { format!("{expr} = {num}") };
    // "Dimension = 5.0" or "Dimension (5.0), driven" — one pair of messages for every kind of dimension.
    let dim = |key: &str, val: String, driven: bool| {
        if driven {
            qymcad_i18n::tr2("con-dim-driven", "what", &qymcad_i18n::tr(key), "value", &val)
        } else {
            qymcad_i18n::tr2("con-dim", "what", &qymcad_i18n::tr(key), "value", &val)
        }
    };
    match c {
        C::Fixed { .. } => qymcad_i18n::tr("con-fixed"),
        C::Horizontal { .. } => qymcad_i18n::tr("con-horizontal"),
        C::Vertical { .. } => qymcad_i18n::tr("con-vertical"),
        // never listed, but named where a conflict has to name what holds the rectangle
        C::Orientation { .. } => qymcad_i18n::tr("con-rect-turn"),
        C::Coincident { .. } => qymcad_i18n::tr("con-coincident"),
        C::Distance { d, driven, expr, .. } => dim("con-name-distance", value(qymcad_i18n::num(*d, 1), expr), *driven),
        C::Parallel { .. } => qymcad_i18n::tr("con-parallel"),
        C::Perpendicular { .. } => qymcad_i18n::tr("con-perpendicular"),
        C::Equal { .. } => qymcad_i18n::tr("con-equal-length"),
        C::EqualRadius { .. } => qymcad_i18n::tr("con-equal-radius"),
        C::CircleTangent { external, .. } => qymcad_i18n::tr(if *external { "con-circle-tangent-out" } else { "con-circle-tangent-in" }),
        C::EdgeDistance { d, driven, expr, .. } => dim("con-name-edge-distance", value(qymcad_i18n::num(*d, 1), expr), *driven),
        C::PointOnCircle { .. } => qymcad_i18n::tr("con-point-on-circle"),
        C::Concentric { .. } => qymcad_i18n::tr("con-concentric"),
        C::Angle { deg, driven, .. } => dim("con-name-angle", format!("{}°", qymcad_i18n::num(*deg, 0)), *driven),
        C::AngleLines { deg, driven, expr, .. } => dim("con-name-angle", value(format!("{}°", qymcad_i18n::num(*deg, 0)), expr), *driven),
        C::ArcLength { len, driven, expr, .. } => dim("con-name-arc-length", value(qymcad_i18n::num(*len, 1), expr), *driven),
        C::Collinear { .. } => qymcad_i18n::tr("con-collinear"),
        C::Midpoint { .. } => qymcad_i18n::tr("con-midpoint"),
        C::Tangent { .. } => qymcad_i18n::tr("con-tangent"),
        C::Symmetric { .. } => qymcad_i18n::tr("con-symmetric"),
        C::PointOnLine { .. } => qymcad_i18n::tr("con-point-on-line"),
        // the diameter and radius signs are symbols rather than words: the same in every language
        C::Diameter { d, driven, diam, expr, .. } => {
            let val = value(qymcad_i18n::num(*d, 1), expr);
            let pfx = if *diam { "Ø" } else { "R" };
            if *driven {
                qymcad_i18n::tr2("con-dim-driven", "what", pfx, "value", &val)
            } else {
                qymcad_i18n::tr2("con-dim", "what", pfx, "value", &val)
            }
        }
        C::DistancePL { d, driven, expr, .. } => dim("con-name-distance-pl", value(qymcad_i18n::num(*d, 1), expr), *driven),
    }
}

/// The closed contour of sketch `si` under a screen point (point-in-polygon; the smaller area wins).
pub fn contour_under_2d(project: &Project, view: &View2d, rect: Rect, screen: Pos2, si: usize) -> Option<Id> {
    let mut best: Option<(f64, Id)> = None;
    for cid in sketch_closed_contours(project, si) {
        let ci = project.contour_index(cid)?;
        let pts: Vec<Pos2> = project.contours[ci].points.iter().map(|p| (Sheet { view: *view, rect }).at(*p)).collect();
        if pts.len() >= 3 && point_in_poly(screen, &pts) {
            let area = poly_area(&pts);
            if best.is_none_or(|(ba, _)| area < ba) {
                best = Some((area, cid));
            }
        }
    }
    best.map(|(_, id)| id)
}

/// The closed contour of sketch `si` under a screen point of the 3D view: each contour lifted into the world by the
/// sketch's frame - as the command's arrow is - and laid onto the screen; the smaller area wins, as on the sheet.
pub fn contour_under_3d(project: &Project, scr: &Screen, screen: Pos2, si: usize) -> Option<Id> {
    let frame = project.sketch_frame(si)?;
    let mut best: Option<(f64, Id)> = None;
    for cid in sketch_closed_contours(project, si) {
        let Some(ci) = project.contour_index(cid) else { continue };
        let pts: Vec<Pos2> = project.contours[ci]
            .points
            .iter()
            .map(|p| {
                let w = frame.lift(*p);
                scr.at([w.x, w.y, w.z]).0
            })
            .collect();
        if pts.len() >= 3 && point_in_poly(screen, &pts) {
            let area = poly_area(&pts);
            if best.is_none_or(|(ba, _)| area < ba) {
                best = Some((area, cid));
            }
        }
    }
    best.map(|(_, id)| id)
}

/// The geometry of the active sketch, for snapping: segments and circles (arcs count as circles).
/// Returns (lines as [(A, B)], circles as [(centre, radius)]).
/// THE EDGES OF A SKETCH READY FOR SNAPPING AND PICKING, told apart by their shape.
///
/// It was `(Vec<(Point2, Point2)>, Vec<(Point2, f64)>)` - which of the two is a segment and which a circle
/// could only be worked out from the body of the function.
pub struct ActiveEdges {
    /// The straight ones, as their two endpoints.
    pub lines: Vec<(Point2, Point2)>,
    /// The round ones: circles whole, and arcs with the part of their circle they run over.
    pub circles: Vec<Rim>,
}

/// THE RIM OF A CIRCLE OR AN ARC, as the cursor snaps to it.
pub struct Rim {
    pub centre: Point2,
    pub radius: f64,
    /// For an arc, the part of the circle it runs over; `None` for a whole circle.
    pub arc: Option<RimArc>,
}

/// THE PART OF ITS CIRCLE AN ARC RUNS OVER: from its start to its end, counter-clockwise or not.
pub struct RimArc {
    pub from: Point2,
    pub to: Point2,
    pub ccw: bool,
}

impl Rim {
    /// WHETHER A POINT OF THE CIRCLE LIES ON THE RIM ITSELF: anywhere for a circle, within its sweep for an arc. A snap
    /// on the rest of an arc's circle stood where nothing is drawn - beside a fillet R20, on the far side of its circle.
    pub fn holds(&self, p: Point2) -> bool {
        let Some(arc) = &self.arc else { return true };
        let ang = |q: Point2| (q.y - self.centre.y).atan2(q.x - self.centre.x);
        let (a0, a1, at) = (ang(arc.from), ang(arc.to), ang(p));
        let tau = std::f64::consts::TAU;
        let (start, sweep) = if arc.ccw { (a0, (a1 - a0).rem_euclid(tau)) } else { (a1, (a0 - a1).rem_euclid(tau)) };
        (at - start).rem_euclid(tau) <= sweep + 1e-9
    }
}

pub fn active_edges(dc: &DrawCtx, si: usize) -> ActiveEdges {
    use qymcad_core::model::EntityKind;
    let mut lines = Vec::new();
    let mut circs = Vec::new();
    if let Some(s) = dc.project.sketches.get(si) {
        let points_by_id: std::collections::HashMap<Id, &qymcad_core::model::SketchPoint> = s.points.iter().map(|p| (p.id, p)).collect(); // a table: these are looked up for every entity or constraint
        let pt = |id: Id| points_by_id.get(&id).copied().map(|p| Point2::new(p.x, p.y));
        for e in &s.entities {
            match e.kind {
                EntityKind::Line { a, b } => {
                    if let (Some(pa), Some(pb)) = (pt(a), pt(b)) {
                        lines.push((pa, pb));
                    }
                }
                EntityKind::Circle { center, r } => {
                    if let Some(c) = pt(center) {
                        circs.push(Rim { centre: c, radius: r, arc: None });
                    }
                }
                EntityKind::Arc { center, a, b, ccw } => {
                    if let (Some(c), Some(pa), Some(pb)) = (pt(center), pt(a), pt(b)) {
                        circs.push(Rim { centre: c, radius: ((pa.x - c.x).powi(2) + (pa.y - c.y).powi(2)).sqrt(), arc: Some(RimArc { from: pa, to: pb, ccw }) });
                    }
                }
                EntityKind::Ellipse { .. } => {} // drawn by its own outline (as a profile)
            }
        }
    }
    ActiveEdges { lines, circles: circs }
}

/// The entity nearest to a screen point (within 12 px): for a line the distance to the segment, for
/// a circle or an arc the distance to the rim.
pub fn entity_near(pick: &PickCtx, rect: Rect, pos: Pos2, si: usize) -> Option<Id> {
    let s = pick.project.sketches.get(si)?;
    let points_by_id: std::collections::HashMap<Id, &qymcad_core::model::SketchPoint> = s.points.iter().map(|p| (p.id, p)).collect(); // a table: these are looked up for every entity or constraint
    let scr = |id: Id| points_by_id.get(&id).copied().map(|q| (Sheet { view: *pick.view, rect }).at(Point2::new(q.x, q.y)));
    let seg_d = |p: Pos2, a: Pos2, b: Pos2| -> f32 {
        let (vx, vy) = (b.x - a.x, b.y - a.y);
        let l2 = vx * vx + vy * vy;
        if l2 < 1e-6 {
            return p.distance(a);
        }
        let t = ((p.x - a.x) * vx + (p.y - a.y) * vy) / l2;
        let t = t.clamp(0.0, 1.0);
        p.distance(Pos2::new(a.x + vx * t, a.y + vy * t))
    };
    use qymcad_core::model::EntityKind;
    let mut best: Option<(f32, Id)> = None;
    for e in &s.entities {
        let d = match e.kind {
            EntityKind::Line { a, b } => match (scr(a), scr(b)) {
                (Some(pa), Some(pb)) => seg_d(pos, pa, pb),
                _ => continue,
            },
            EntityKind::Circle { center, r } => match scr(center) {
                Some(c) => (pos.distance(c) - (r * pick.view.scale as f64) as f32).abs(),
                _ => continue,
            },
            EntityKind::Arc { center, a, .. } => match (scr(center), scr(a)) {
                (Some(c), Some(pa)) => (pos.distance(c) - c.distance(pa)).abs(),
                _ => continue,
            },
            _ => continue,
        };
        if d < 12.0 && best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, e.id));
        }
    }
    best.map(|(_, id)| id)
}

/// Store the picked contour `cid` into the sweep or loft slot.
pub fn set_contour_slot(loft: &mut LoftParams, sweep: &mut SweepParams, slot: ContourSlot, cid: Id) {
    match slot {
        ContourSlot::SweepProfile => sweep.prof_cid = cid,
        ContourSlot::SweepPath => sweep.path_cid = cid,
        ContourSlot::LoftSection(i) => {
            if let Some(c) = loft.cids.get_mut(i) {
                *c = cid;
            }
        }
    }
}

/// The slot's candidate contours (for hit-testing in the half-sketcher). A path allows open contours.
pub fn slot_candidates(loft: &LoftParams, project: &Project, sweep: SweepParams, slot: ContourSlot) -> Vec<Id> {
    match slot {
        ContourSlot::SweepProfile => project.sweep_profile_contours(sweep.prof_sid),
        ContourSlot::SweepPath => project.sweep_path_contours(sweep.path_sid),
        ContourSlot::LoftSection(i) => project.sweep_profile_contours(loft.sids.get(i).copied().unwrap_or(0)),
    }
}

/// A rubber-band selection: while a sketch is being edited the band selects its entities.
pub fn box_select(ed: Editing, sel_sk: &mut SketchSelection, sketch_ses: SketchSession, rect: Rect, a: Pos2, b: Pos2) {
    if let Sel::Sketch(si) = *ed.sel {
        if sketch_ses.editing.is_some() {
            box_select_sketch(ed, sel_sk, rect, a, b, si);
        }
    }
}

/// The label of a candidate axis line for the command bar — construction lines are marked as such.
pub fn axis_line_label(project: &qymcad_core::model::Project, si: usize, line: Id, n: usize) -> String {
    let constr = project.sketches.get(si).and_then(|s| s.entities.iter().find(|e| e.id == line)).map(|e| e.construction).unwrap_or(false);
    if constr {
        qymcad_i18n::tr1("sk-axis-line-n", "n", &n.to_string())
    } else {
        qymcad_i18n::tr1("sk-line-n", "n", &n.to_string())
    }
}

/// Finish adding dimension `ci`: solve and CLASSIFY it. Redundant-but-CONSISTENT becomes a driven
/// (reference) dimension. A CONFLICT with other constraints is NOT muffled - such a dimension is NOT
/// made driven (otherwise it would "measure" averaged geometry and hide the conflict; making it driven
/// is available by hand, with the button in the list of constraints). Returns (redundant, conflicting).
///
/// The rank analysis is called DIRECTLY here, past the diagnostics cache: this is a one-off action
/// right after an edit, and the answer is needed for the fresh state rather than for an imprint of the
/// previous frame.
pub fn finish_dim(project: &mut Project, regen: &mut Rebuilding, si: usize, ci: usize) -> (bool, bool) {
    project.solve_sketch(si);
    let redundant = project.dim_redundant(si, ci);
    let conflict = project.sketch_conflicts(si).contains(&ci);
    if redundant && !conflict {
        project.auto_driven(si, ci);
        project.solve_sketch(si);
    }
    invalidate(regen);
    (redundant, conflict)
}

/// The lineage of object `id` BY NAMES. `None` means the object has no Id of its own (a face)
/// and has no lineage.
pub fn lineage_of(project: &qymcad_core::model::Project, id: Option<Id>) -> Lineage {
    let Some(id) = id else { return Lineage::default() };
    let name_of = |nid: Id| -> String { project.timeline.iter().find(|n| n.id == nid).map(|n| qymcad_i18n::name(&n.name)).unwrap_or_else(|| format!("#{nid}")) };
    // "WHAT CREATED IT" for a timeline node is the node itself, and there is no point repeating
    // it under its own heading: what is selected is already written there. What is shown is the
    // input it stands on.
    let built_on: Vec<String> = match project.timeline.iter().find(|n| n.id == id) {
        Some(node) => node.kind.inputs().iter().map(|i| project.creator_of(*i).map(name_of).unwrap_or_else(|| format!("#{i}"))).collect(),
        None => project.creator_of(id).map(name_of).into_iter().collect(),
    };
    Lineage { built_on, dependents: project.dependents_of(id).into_iter().map(name_of).collect() }
}

/// THE OUTLINE OF A SKETCH ENTITY as a polyline in sketch coordinates: a line its two ends, a circle, an arc and an
/// ellipse 48 pieces of themselves - enough for a box to tell touched from missed at any zoom a box is drawn at.
pub fn entity_outline(s: &qymcad_core::model::Sketch, kind: &qymcad_core::model::EntityKind) -> Vec<Point2> {
    use qymcad_core::model::EntityKind;
    let pt = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| Point2::new(q.x, q.y));
    let ring = |c: Point2, u: (f64, f64), v: (f64, f64), a0: f64, sweep: f64| -> Vec<Point2> {
        (0..=48)
            .map(|i| {
                let t = a0 + sweep * i as f64 / 48.0;
                Point2::new(c.x + u.0 * t.cos() + v.0 * t.sin(), c.y + u.1 * t.cos() + v.1 * t.sin())
            })
            .collect()
    };
    match *kind {
        EntityKind::Line { a, b } => [pt(a), pt(b)].into_iter().flatten().collect(),
        EntityKind::Circle { center, r } => pt(center).map(|c| ring(c, (r, 0.0), (0.0, r), 0.0, std::f64::consts::TAU)).unwrap_or_default(),
        EntityKind::Arc { center, a, b, ccw } => match (pt(center), pt(a), pt(b)) {
            (Some(c), Some(pa), Some(pb)) => {
                let r = ((pa.x - c.x).powi(2) + (pa.y - c.y).powi(2)).sqrt();
                let (a0, a1) = ((pa.y - c.y).atan2(pa.x - c.x), (pb.y - c.y).atan2(pb.x - c.x));
                let mut sweep = (a1 - a0).rem_euclid(std::f64::consts::TAU);
                if !ccw {
                    sweep -= std::f64::consts::TAU;
                }
                ring(c, (r, 0.0), (0.0, r), a0, sweep)
            }
            _ => Vec::new(),
        },
        EntityKind::Ellipse { c, ma, mi } => match (pt(c), pt(ma), pt(mi)) {
            (Some(c), Some(pa), Some(pi)) => ring(c, (pa.x - c.x, pa.y - c.y), (pi.x - c.x, pi.y - c.y), 0.0, std::f64::consts::TAU),
            _ => Vec::new(),
        },
    }
}

/// Does the segment p-q cross the box [x0, x1] x [y0, y1] (screen)? Tested against the four sides; an end inside is
/// the caller's own test.
pub(crate) fn segment_crosses_box(p: Pos2, q: Pos2, x0: f32, x1: f32, y0: f32, y1: f32) -> bool {
    let cross = |a: Pos2, b: Pos2, c: Pos2, d: Pos2| {
        let o = |p: Pos2, q: Pos2, r: Pos2| (q.x - p.x) * (r.y - p.y) - (q.y - p.y) * (r.x - p.x);
        let (d1, d2, d3, d4) = (o(c, d, a), o(c, d, b), o(a, b, c), o(a, b, d));
        (d1 > 0.0) != (d2 > 0.0) && (d3 > 0.0) != (d4 > 0.0)
    };
    let (a, b, c, d) = (egui::pos2(x0, y0), egui::pos2(x1, y0), egui::pos2(x1, y1), egui::pos2(x0, y1));
    cross(p, q, a, b) || cross(p, q, b, c) || cross(p, q, c, d) || cross(p, q, d, a)
}

/// A rubber-band selection of sketch entities: left to right means enclosure (wholly inside), right to
/// left means crossing (merely touched) — the usual CAD convention.
pub fn box_select_sketch(ed: Editing, sel_sk: &mut SketchSelection, rect: Rect, a: Pos2, b: Pos2, si: usize) {
    let sh = Sheet { view: *ed.view, rect };
    let crossing = b.x < a.x;
    let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
    let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
    let inside = |p: Pos2| p.x >= x0 && p.x <= x1 && p.y >= y0 && p.y <= y1;
    // gather the hits first (`s` holds a borrow of self, so they are pushed afterwards)
    let (mut add_ent, mut add_pt): (Vec<Id>, Vec<Id>) = (Vec::new(), Vec::new());
    {
        let Some(s) = ed.project.sketches.get(si) else { return };
        // THE SHAPE ITSELF IS TESTED, not its ends: a line that crosses the box with both ends outside is touched, and
        // a circle is inside only when all of it is, not its centre alone
        let pts_of = |e: &qymcad_core::model::SketchEntity| -> Vec<Pos2> { entity_outline(s, &e.kind).into_iter().map(|p| sh.at(p)).collect() };
        for e in &s.entities {
            let pts = pts_of(e);
            if pts.is_empty() {
                continue;
            }
            let hit = if crossing {
                pts.iter().any(|p| inside(*p)) || pts.windows(2).any(|w| segment_crosses_box(w[0], w[1], x0, x1, y0, y1))
            } else {
                pts.iter().all(|p| inside(*p))
            };
            if hit {
                add_ent.push(e.id);
            }
        }
        for p in &s.points {
            if inside(sh.at(Point2::new(p.x, p.y))) {
                add_pt.push(p.id);
            }
        }
    }
    for id in add_ent {
        if !sel_sk.items.contains(&(1, id)) {
            sel_sk.items.push((1, id));
        }
    }
    for id in add_pt {
        if !sel_sk.items.contains(&(0, id)) {
            sel_sk.items.push((0, id));
        }
    }
    *ed.status = Notice::info(qymcad_i18n::tr1("g-selected-n", "n", &sel_sk.items.len().to_string()));
}

/// The closed contours of sketch `si` (by Id) that will do as the profile of a feature.
pub fn sketch_closed_contours(project: &qymcad_core::model::Project, si: usize) -> Vec<Id> {
    project.sketches.get(si).map(|s| s.contour_ids.iter().copied().filter(|cid| project.contour_profile_xy(*cid).is_some()).collect()).unwrap_or_default()
}

/// The contour under the cursor among the slot's CANDIDATES (for the half-sketcher of a sweep or a
/// loft). A closed one with the cursor inside wins (the smaller area is the nearer); otherwise the
/// NEAREST polyline within the threshold, which is how an OPEN path gets caught, since point-in-polygon
/// will not take it. `cands` holds the slot's candidate contours.
pub fn slot_contour_under_2d(pick: &PickCtx, rect: Rect, screen: Pos2, cands: &[Id]) -> Option<Id> {
    let seg_d = |p: Pos2, a: Pos2, b: Pos2| -> f32 {
        let ab = b - a;
        let l2 = ab.length_sq();
        let t = if l2 <= 1e-6 { 0.0 } else { ((p - a).dot(ab) / l2).clamp(0.0, 1.0) };
        (p - (a + ab * t)).length()
    };
    let mut inside: Option<(f64, Id)> = None; // closed, cursor inside: ranked by area
    let mut near: Option<(f32, Id)> = None; // ranked by distance to the polyline
    for &cid in cands {
        let Some(ci) = pick.project.contour_index(cid) else { continue };
        let c = &pick.project.contours[ci];
        if c.points.len() < 2 {
            continue;
        }
        let pts: Vec<Pos2> = c.points.iter().map(|p| (Sheet { view: *pick.view, rect }).at(*p)).collect();
        if c.closed && pts.len() >= 3 && point_in_poly(screen, &pts) {
            let area = poly_area(&pts);
            if inside.is_none_or(|(ba, _)| area < ba) {
                inside = Some((area, cid));
            }
        }
        let n = pts.len();
        let segs = if c.closed { n } else { n - 1 };
        let mut d = f32::INFINITY;
        for k in 0..segs {
            d = d.min(seg_d(screen, pts[k], pts[(k + 1) % n]));
        }
        if d < 8.0 && near.is_none_or(|(bd, _)| d < bd) {
            near = Some((d, cid));
        }
    }
    inside.map(|(_, id)| id).or(near.map(|(_, id)| id))
}
