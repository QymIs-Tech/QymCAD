//! THE SKETCH WORKBENCH.
//!
//! Drawing entities, constraints and dimensions, dragging points, trimming, patterns, text and notes, the
//! diagnostics of degrees of freedom. Every function here works over `qymcad_ui_state::SketchCtx` - the records the sketch
//! edits - and asks the application for nothing.

use qymcad_ui_state::Notice;
use egui::{Color32, Pos2, Rect};
use egui_phosphor::regular as ph;
use qymcad_core::geom::Point2;
use qymcad_core::model::{Id, Project};
use qymcad_ui_state::grab::Grab;
use qymcad_ui_state::WinKind;

/// A CORNER AS THE PREVIEW DRAWS IT: where it stands, the two lines that make it, and whether it is in the set
/// already - which of the two colours of the scheme it is drawn in says.
pub(crate) struct Shown {
    point: Id,
    pair: Option<(Id, Id)>,
    in_set: bool,
}

/// The sign of a cross product, as the word an arc is stored with: the sketcher works out the turn from the
/// geometry under the pointer, and the model keeps it as a word rather than as a bare flag.
fn winding(ccw: bool) -> qymcad_core::feature::Winding {
    if ccw {
        qymcad_core::feature::Winding::Ccw
    } else {
        qymcad_core::feature::Winding::Cw
    }
}

/// WHAT A SKETCH STANDS ON, in words: a base plane, a work plane by its name, a face by the part it belongs to.
/// Reported behaviour: the properties said "face of body #37" - a number nobody sees anywhere else.
pub fn sketch_plane_words(project: &qymcad_core::model::Project, plane: qymcad_core::feature::SketchPlane) -> String {
    use qymcad_core::feature::{BasePlane, SketchPlane};
    match plane {
        SketchPlane::World(BasePlane::XY) => qymcad_i18n::tr("sk-plane-xy"),
        SketchPlane::World(BasePlane::XZ) => qymcad_i18n::tr("sk-plane-xz"),
        SketchPlane::World(BasePlane::YZ) => qymcad_i18n::tr("sk-plane-yz"),
        SketchPlane::Datum(id) => {
            let name = project.planes.iter().find(|p| p.id == id).map(|p| qymcad_i18n::name(&p.name)).unwrap_or_default();
            qymcad_i18n::tr1("sk-on-work-plane", "name", &name)
        }
        SketchPlane::Face(body, _) => {
            let part = project.body_owner(body).and_then(|c| project.components.iter().find(|x| x.id == c)).map(|c| qymcad_i18n::name(&c.name)).unwrap_or_default();
            qymcad_i18n::tr1("sk-on-body-face", "name", &part)
        }
    }
}

/// THE LENGTH OF CURVE `eid` of sketch `si`, mm: a line end to end, an arc along its span, a circle and an ellipse round
/// their circle (the ellipse by Ramanujan's formula).
fn curve_length(project: &Project, si: usize, eid: Id) -> f64 {
    use qymcad_core::model::EntityKind as EK;
    let Some(s) = project.sketches.get(si) else { return 0.0 };
    let at = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y)).unwrap_or_default();
    let dist = |p: (f64, f64), q: (f64, f64)| (q.0 - p.0).hypot(q.1 - p.1);
    match s.entities.iter().find(|e| e.id == eid).map(|e| e.kind) {
        Some(EK::Line { a, b }) => dist(at(a), at(b)),
        Some(EK::Arc { center, a, b, ccw }) => {
            let (c, pa, pb) = (at(center), at(a), at(b));
            let (a0, a1) = ((pa.1 - c.1).atan2(pa.0 - c.0), (pb.1 - c.1).atan2(pb.0 - c.0));
            let sweep = if ccw { (a1 - a0).rem_euclid(std::f64::consts::TAU) } else { (a0 - a1).rem_euclid(std::f64::consts::TAU) };
            dist(c, pa) * sweep
        }
        Some(EK::Circle { r, .. }) => std::f64::consts::TAU * r,
        Some(EK::Ellipse { c, ma, mi }) => {
            let (x, y) = (dist(at(c), at(ma)), dist(at(c), at(mi)));
            let h = ((x - y) / (x + y)).powi(2);
            std::f64::consts::PI * (x + y) * (1.0 + 3.0 * h / (10.0 + (4.0 - 3.0 * h).sqrt()))
        }
        None => 0.0,
    }
}

/// THE NAME OF A SKETCH ENTITY FOR A PERSON: "Line 3", "Circle 1".
///
/// The number is the ordinal among entities OF THE SAME kind rather than a running id: "Line 3" can be
/// found by eye, while "Line 47" says nothing.
pub fn sketch_entity_name(project: &qymcad_core::model::Project, si: usize, eid: Id) -> String {
    use qymcad_core::model::EntityKind as EK;
    let Some(s) = project.sketches.get(si) else { return String::new() };
    let Some(e) = s.entities.iter().find(|e| e.id == eid) else { return String::new() };
    let key = match e.kind {
        EK::Line { .. } => "ent-line",
        EK::Arc { .. } => "ent-arc",
        EK::Circle { .. } => "ent-circle",
        EK::Ellipse { .. } => "ent-ellipse",
    };
    let same = |k: &EK| std::mem::discriminant(k) == std::mem::discriminant(&e.kind);
    let n = s.entities.iter().filter(|x| same(&x.kind)).position(|x| x.id == eid).unwrap_or(0) + 1;
    qymcad_i18n::tr2("ent-named", "what", &qymcad_i18n::tr(key), "n", &n.to_string())
}

/// The exact curves of a sketch for a vector export: the edges of the contours (a segment, an arc, a
/// circle); where no exact edges exist, the polyline of points is used as segments.
pub fn sketch_export_edges(project: &qymcad_core::model::Project, si: usize) -> Vec<qymcad_core::geom::ProfEdge> {
    use qymcad_core::geom::ProfEdge;
    let mut out = Vec::new();
    let Some(sk) = project.sketches.get(si) else { return out };
    for cid in &sk.contour_ids {
        let Some(ci) = project.contour_index(*cid) else { continue };
        let c = &project.contours[ci];
        if !c.edges.is_empty() {
            out.extend(c.edges.iter().copied());
        } else {
            let n = c.points.len();
            let last = if c.closed { n } else { n.saturating_sub(1) };
            for i in 0..last {
                out.push(ProfEdge::Line { a: c.points[i], b: c.points[(i + 1) % n] });
            }
        }
    }
    out
}

/// THE NUMBER OF THE SPAN of a curve a point fell into (a span is the stretch between two neighbouring
/// intersections). It tells "the same segment we already cut" from "the next one" - without it, dragging
/// along one line would trim only its first piece.
pub fn trim_span_key(project: &qymcad_core::model::Project, si: usize, eid: Id, x: f64, y: f64) -> u32 {
    let inter = project.entity_intersections(si, eid);
    let Some(s) = project.sketches.get(si) else { return 0 };
    let Some(kind) = s.entities.iter().find(|e| e.id == eid).map(|e| e.kind) else { return 0 };
    let points_by_id: std::collections::HashMap<Id, &qymcad_core::model::SketchPoint> = s.points.iter().map(|p| (p.id, p)).collect(); // a table: these are looked up for every entity or constraint
    let pt = |id: Id| points_by_id.get(&id).copied().map(|q| (q.x, q.y));
    match kind {
        qymcad_core::model::EntityKind::Line { a, b } => {
            let (Some((ax, ay)), Some((bx, by))) = (pt(a), pt(b)) else { return 0 };
            let (dx, dy) = (bx - ax, by - ay);
            let len2 = dx * dx + dy * dy;
            if len2 < 1e-12 {
                return 0;
            }
            let param = |px: f64, py: f64| ((px - ax) * dx + (py - ay) * dy) / len2;
            let tc = param(x, y);
            inter.iter().filter(|(ix, iy)| param(*ix, *iy) < tc).count() as u32
        }
        qymcad_core::model::EntityKind::Circle { center, .. } | qymcad_core::model::EntityKind::Arc { center, .. } => {
            let Some((cx, cy)) = pt(center) else { return 0 };
            let ang = |px: f64, py: f64| (py - cy).atan2(px - cx).rem_euclid(std::f64::consts::TAU);
            let ac = ang(x, y);
            inter.iter().filter(|(ix, iy)| ang(*ix, *iy) < ac).count() as u32
        }
        _ => 0,
    }
}

/// READ WHAT WAS TYPED INTO A SKETCH FIELD: a number OR an expression over the global variables.
///
/// The size fields that follow drawing (the width and height of a rectangle or an ellipse, the radius and
/// angle of a polygon, a corner fillet, a rotation) used to read the value through `parse::<f64>()` -
/// that is, they accepted a bare number only. A formula or a global variable could not be typed, although
/// those are WORKING dimensions of a part: "width = housing - 2*wall" is an everyday thing.
///
/// An empty or broken expression gives `None`, and the caller keeps the previous value: no rubbish must
/// travel into the model.
pub fn parse_num(project: &qymcad_core::model::Project, text: &str) -> Option<f64> {
    let t = text.trim().replace(',', ".");
    if t.is_empty() {
        return None;
    }
    qymcad_core::expr::eval(&t, &project.param_map()).ok().filter(|v| v.is_finite())
}

/// Half a turn, in degrees: the bound of the opening between two lines.
const HALF_TURN_DEG: f64 = 180.0;

/// The longest length a dimension of a sketch takes, in mm - the same bound the fields of the tool bars keep.
pub const MAX_SKETCH_LENGTH: f64 = 10000.0;

/// WHAT STANDS LIT UNDER THE CURSOR BEFORE A CLICK: the geometry, or else the glyph of a constraint.
pub struct PreSelect {
    pub sketch: Option<(u8, Id)>,
    pub constraint: Option<usize>,
}

/// THE PRE-SELECT HIGHLIGHT, only while the sketch being edited is in selection mode - no drawing or dimension tool in
/// hand and nothing being dragged. With the fillet or the chamfer in hand a construction line is not lit: it is no side
/// of a corner, and a line lit under the cursor says that a click takes it.
pub fn pre_select(sk: &mut qymcad_ui_state::SketchCtx, rect: Rect, hover: Option<Pos2>, dragged: bool) -> PreSelect {
    let none = PreSelect { sketch: None, constraint: None };
    let (qymcad_ui_state::Sel::Sketch(si), Some(hp)) = (*sk.sel, hover) else { return none };
    let still = !dragged && sk.drag.pt().is_none() && sk.drag.mov().is_none();
    if qymcad_ui_state::edit_si(sk.project, sk.sketch_ses) != Some(si) || sk.armed.draw_kind() != 0 || sk.armed.dim_kind() != 0 || !still {
        return none;
    }
    let corner_tool = matches!(sk.armed.click_op(), 4 | 5);
    let construction = |id: Id| sk.project.sketches.get(si).is_some_and(|s| s.entities.iter().any(|e| e.id == id && e.construction));
    let sketch = sketch_hit(&sk.pick(), rect, hp, si).filter(|&(k, id)| !(corner_tool && k == 1 && construction(id)));
    // the glyph of a constraint is lit only where no geometry is
    let constraint = if sketch.is_none() { constraint_glyph_at(sk, rect, hp, si) } else { None };
    PreSelect { sketch, constraint }
}

/// What is under the cursor in a sketch: (kind, Id). 0 is a point, 1 an entity (a line, an arc, a
/// circle), 2 a primitive (a contour). Ordinary geometry takes priority over construction geometry.
pub fn sketch_hit(pick: &qymcad_ui_state::PickCtx, rect: Rect, pos: Pos2, si: usize) -> Option<(u8, Id)> {
    // a system id of 0 means "not materialised yet" and must not match a real point
    let p_eq = |sys: Id, id: Id| sys != 0 && sys == id;
    let sh = qymcad_ui_state::Sheet { view: *pick.view, rect };
    use qymcad_core::model::EntityKind;
    let s = pick.project.sketches.get(si)?;
    // the points by id from a table: looked up along the list for both ends of every entity, a drawing of 70 000
    // segments took 4 s a frame under the pointer
    let at: std::collections::HashMap<Id, Point2> = s.points.iter().map(|p| (p.id, Point2::new(p.x, p.y))).collect();
    let pt = |id: Id| at.get(&id).copied();
    // the ends of THE AXES are not picked as ordinary points (they mark infinite lines); the origin
    // deliberately STAYS pickable (it is needed for a coincidence with the origin) - hence `axis_pts`
    // rather than `system_ids`
    let is_axis_ref = |id: Id| s.axis_pts.contains(&id) || p_eq(s.frame, id);
    // 1) the nearest point.
    //
    // THE ORIGIN YIELDS TO ORDINARY GEOMETRY STANDING ON IT. Reported behaviour: "I cannot move the
    // circle away from the origin". A circle drawn at zero keeps a centre of its own - the stitching
    // leaves curve centres alone - but that centre and the origin are one point apart by nothing, and
    // the origin was picked first. The origin carries `Fixed`, so the drag moved nothing at all.
    //
    // It stays pickable, because a coincidence with the origin is made by clicking it; it simply loses
    // every tie to a real point.
    let mut best_pt: Option<(f32, Id)> = None;
    let mut best_sys: Option<(f32, Id)> = None;
    for p in &s.points {
        if is_axis_ref(p.id) {
            continue;
        }
        let d = sh.at(Point2::new(p.x, p.y)).distance(pos);
        if d > qymcad_ui_state::grab::grab(pick.set, Grab::Point) {
            continue;
        }
        let slot = if p_eq(s.origin, p.id) { &mut best_sys } else { &mut best_pt };
        if slot.is_none_or(|(bd, _)| d < bd) {
            *slot = Some((d, p.id));
        }
    }
    if let Some((_, id)) = best_pt.or(best_sys) {
        return Some((0u8, id));
    }
    // 2) the nearest entity (an ordinary one takes priority over construction geometry)
    let mut best_e: Option<(u8, f32, Id)> = None;
    for e in &s.entities {
        let d = match e.kind {
            EntityKind::Line { a, b } => match (pt(a), pt(b)) {
                (Some(pa), Some(pb)) => qymcad_ui_state::screen_dist_seg(pos, sh.at(pa), sh.at(pb)),
                _ => f32::INFINITY,
            },
            EntityKind::Circle { center, r } => match pt(center) {
                Some(c) => {
                    let sc = sh.at(c);
                    let rp = (sh.at(Point2::new(c.x + r, c.y)).x - sc.x).abs();
                    (sc.distance(pos) - rp).abs()
                }
                None => f32::INFINITY,
            },
            EntityKind::Arc { center, a, b, ccw } => match (pt(center), pt(a), pt(b)) {
                (Some(c), Some(pa), Some(pb)) => qymcad_pick::arc_screen_dist(&sh, pos, c, pa, pb, ccw),
                _ => f32::INFINITY,
            },
            EntityKind::Ellipse { c, ma, mi } => match (pt(c), pt(ma), pt(mi)) {
                (Some(pc), Some(pma), Some(pmi)) => {
                    // the distance to the outline of the ellipse, measured over samples
                    let major = ((pma.x - pc.x).powi(2) + (pma.y - pc.y).powi(2)).sqrt().max(1e-6);
                    let minor = ((pmi.x - pc.x).powi(2) + (pmi.y - pc.y).powi(2)).sqrt().max(1e-6);
                    let (ux, uy) = ((pma.x - pc.x) / major, (pma.y - pc.y) / major);
                    let (vx, vy) = (-uy, ux);
                    let (mut best, mut prev) = (f32::INFINITY, None::<Pos2>);
                    for k in 0..=48 {
                        let t = std::f64::consts::TAU * k as f64 / 48.0;
                        let (ct, st) = (t.cos(), t.sin());
                        let wp = Point2::new(pc.x + major * ct * ux + minor * st * vx, pc.y + major * ct * uy + minor * st * vy);
                        let sp = sh.at(wp);
                        if let Some(pp) = prev {
                            best = best.min(qymcad_ui_state::screen_dist_seg(pos, pp, sp));
                        }
                        prev = Some(sp);
                    }
                    best
                }
                _ => f32::INFINITY,
            },
        };
        if d <= qymcad_ui_state::grab::grab(pick.set, Grab::Curve) {
            let tier = if e.construction { 1u8 } else { 0u8 };
            if best_e.is_none_or(|(bt, bd, _)| (tier, d) < (bt, bd)) {
                best_e = Some((tier, d, e.id));
            }
        }
    }
    if let Some((_, _, id)) = best_e {
        return Some((1u8, id));
    }
    // 3) the nearest spline, by its curve: kept by its first node (`SEL_SPLINE`), as a spline has no id of its own
    let mut best_sp: Option<(u8, f32, Id)> = None;
    for (spi, sp) in s.splines.iter().enumerate() {
        let Some(&first) = sp.points.first() else { continue };
        let mut line: Vec<Pos2> = pick.project.spline_polyline(si, spi).into_iter().map(|p| sh.at(p)).collect();
        if sp.closed {
            line.extend(line.first().copied());
        }
        let d = line.windows(2).map(|w| qymcad_ui_state::screen_dist_seg(pos, w[0], w[1])).fold(f32::INFINITY, f32::min);
        let tier = if sp.construction { 1u8 } else { 0u8 };
        if d <= qymcad_ui_state::grab::grab(pick.set, Grab::Curve) && best_sp.is_none_or(|(bt, bd, _)| (tier, d) < (bt, bd)) {
            best_sp = Some((tier, d, first));
        }
    }
    best_sp.map(|(_, _, id)| (qymcad_ui_state::SEL_SPLINE, id))
}

/// FINISH THE SPLINE - what a double click does.
///
/// Split out of the event handling for the same reason as everything else: a test must finish the shape
/// through THE SAME code rather than a copy of its own. Esc cancels a spline - that is a different
/// action, and substituting it for finishing would mean checking the wrong thing.
pub fn finish_spline(sk: &mut qymcad_ui_state::SketchCtx) {
    lay_the_spline(sk, qymcad_core::feature::Ends::Open);
}

/// THE SPLINE DRAWN SO FAR PUT INTO THE SKETCH, open or closed, its nodes tied to what they were clicked on
/// (`tie_what_landed`): the nodes enter the document here, not at their clicks, so the ties are laid here too.
fn lay_the_spline(sk: &mut qymcad_ui_state::SketchCtx, ends: qymcad_core::feature::Ends) {
    if let qymcad_ui_state::Sel::Sketch(si) = *sk.sel {
        if sk.tool.pts.len() >= 2 {
            // THE BOUNDARY OF AN OPERATION: the spline enters the document here, not at its clicks, so this is the
            // step of undo named after it.
            qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, qymcad_i18n::tr("sk-spline"));
            let pts = std::mem::take(&mut sk.tool.pts);
            let before: std::collections::HashSet<Id> = sk.project.sketches.get(si).map(|s| s.points.iter().map(|q| q.id).collect()).unwrap_or_default();
            sk.project.add_spline(si, pts.clone(), ends, qymcad_core::feature::Purpose::of(sk.tool.construction));
            if tied(sk) {
                let made: std::collections::HashSet<Id> = sk.project.sketches.get(si).map(|s| s.points.iter().map(|q| q.id).filter(|id| !before.contains(id)).collect()).unwrap_or_default();
                tie_what_landed(&mut *sk.project, *sk.view, si, &pts, &made);
                sk.project.solve_sketch(si);
            }
            qymcad_ui_state::invalidate(&mut *sk.regen);
            sk.view.initialized = false;
            qymcad_ui_state::commit_edit(&mut sk.rebuild());
        }
    }
    sk.tool.pts.clear();
}

/// TWO CLICKS ON ONE PLACE, as the drawing tools see them: closer than the tolerance that welds the points of a
/// drawing, about 8 px on screen and never more than 0.4 mm. The two clicks of a double click land there - the
/// snap puts the second one exactly onto the node the first one made.
fn same_place(sk: &qymcad_ui_state::SketchCtx, a: Point2, b: Point2) -> bool {
    (a.x - b.x).hypot(a.y - b.y) <= (8.0 / sk.view.scale as f64).clamp(1e-4, 0.4)
}

/// Esc ON A CONSTRUCTION UNDER WAY. A spline has no last click - it is finished by a double click or by Esc, as
/// its hint says - so Esc keeps the nodes clicked so far as a spline. Any other shape is broken off.
pub fn end_construction(sk: &mut qymcad_ui_state::SketchCtx) {
    if sk.armed.draw_kind() == 9 {
        finish_spline(sk);
    } else {
        sk.tool.pts.clear();
    }
}

/// A CLICK IN A SKETCH: what it is right now - a step of a drawing tool, the placing of a dimension, or
/// a pick of geometry.
///
/// What a click means depends on the active mode, and that is precisely why the modes must be mutually
/// exclusive: while they were cleared by hand one field at a time, a click could mean two things at
/// once. Here that reading is gathered in one place and visible whole.
///
/// TAKE THE POINT UNDER THE CURSOR (the start of a drag). `skip` lists the points that are not dragged.
///
/// Split out of the event handling so that a test can repeat a drag through THE SAME code. While this sat
/// inside `if resp.drag_started()`, a test could not reach the logic without faking the event - that is,
/// it would have been checking its own fake.
pub fn begin_point_drag(
    drag: &mut qymcad_ui_state::Dragging,
    project: &Project,
    set: &qymcad_ui_state::Settings,
    sheet: qymcad_ui_state::Sheet,
    si: usize,
    pp: Pos2,
    skip: &std::collections::HashSet<Id>,
) -> bool {
    let qymcad_ui_state::Sheet { view, rect } = sheet;
    // WHAT THE SKETCH HOLDS IS NOT TAKEN INTO HAND AT ALL: the frame, the driven projections and the points
    // the person pinned. A pin carries no coordinates - it holds the point where it IS - so a pinned point
    // dragged by the mouse used to travel with the cursor and the pin travelled with it, silently. Refusing
    // to pick it up says the same thing honestly: it does not move.
    let held = project.sketches[si].held_points();
    let mut best: Option<(f32, usize)> = None;
    for pi in 0..project.sketches[si].points.len() {
        let p = project.sketches[si].points[pi];
        if skip.contains(&p.id) || held.contains(&p.id) {
            continue;
        }
        let sp = (qymcad_ui_state::Sheet { view, rect }).at(Point2::new(p.x, p.y));
        let d = sp.distance(pp);
        if best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, pi));
        }
    }
    match best {
        Some((d, pi)) if d <= qymcad_ui_state::grab::grab(set, Grab::Curve) => {
            *drag = qymcad_ui_state::Dragging::Point(si, pi);
            true
        }
        _ => false,
    }
}

/// The largest turn a rotation takes, in degrees: ten whole turns.
const TEN_TURNS: f64 = 3600.0;

/// THE ANGLE OF A TURN as typed, or in words why it cannot be taken: not a number, no turn at all, or past ten whole
/// turns.
fn angle_of(project: &Project, text: &str) -> Result<f64, String> {
    match parse_num(project, text) {
        None => Err(qymcad_i18n::tr("sk-angle-not-number")),
        Some(v) if v.abs() < 1e-9 => Err(qymcad_i18n::tr("cmd-value-zero")),
        Some(v) if v.abs() > TEN_TURNS => Err(qymcad_i18n::tr2("cmd-value-out-of-range", "lo", &(-TEN_TURNS).to_string(), "hi", &TEN_TURNS.to_string())),
        Some(v) => Ok(v),
    }
}

/// The size of every line and circle of sketch `si`, in the order of its entities: a line's length, a circle's
/// radius. Nothing for an arc or an ellipse, whose size its points hold.
fn shape_sizes(project: &Project, si: usize) -> Vec<f64> {
    use qymcad_core::model::EntityKind;
    let Some(s) = project.sketches.get(si) else { return Vec::new() };
    let points_by_id: std::collections::HashMap<Id, &qymcad_core::model::SketchPoint> = s.points.iter().map(|p| (p.id, p)).collect(); // a table: these are looked up for every entity or constraint
    let at = |id: Id| points_by_id.get(&id).copied().map(|p| (p.x, p.y));
    s.entities
        .iter()
        .filter_map(|e| match e.kind {
            EntityKind::Line { a, b } => Some(at(a).zip(at(b)).map_or(0.0, |(a, b)| (a.0 - b.0).hypot(a.1 - b.1))),
            EntityKind::Circle { r, .. } => Some(r),
            _ => None,
        })
        .collect()
}

/// Which way a line is turned to lie: level, or upright.
#[derive(Clone, Copy)]
pub(crate) enum Level {
    Horizontal,
    Vertical,
}

/// Turn line `(a, b)` of sketch `si` to lie `level`, keeping its length and the way it runs from `a` to `b`: about its
/// middle, or about an end the sketch holds so the held end stays where it is.
fn turn_to_level(project: &mut Project, si: usize, (a, b): (Id, Id), level: Level) {
    let Some(s) = project.sketches.get_mut(si) else { return };
    let held = s.held_points();
    let at = |s: &qymcad_core::model::Sketch, id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
    let (Some((ax, ay)), Some((bx, by))) = (at(s, a), at(s, b)) else { return };
    let len = (bx - ax).hypot(by - ay);
    if len < 1e-9 {
        return;
    }
    // the way the line runs along the new axis: as it ran along it, or forward where it did not run along it at all
    let (dx, dy) = match level {
        Level::Horizontal => (if bx < ax { -len } else { len }, 0.0),
        Level::Vertical => (0.0, if by < ay { -len } else { len }),
    };
    let ((nax, nay), (nbx, nby)) = if held.contains(&a) {
        ((ax, ay), (ax + dx, ay + dy))
    } else if held.contains(&b) {
        ((bx - dx, by - dy), (bx, by))
    } else {
        let (mx, my) = ((ax + bx) / 2.0, (ay + by) / 2.0);
        ((mx - dx / 2.0, my - dy / 2.0), (mx + dx / 2.0, my + dy / 2.0))
    };
    for q in s.points.iter_mut() {
        if q.id == a {
            (q.x, q.y) = (nax, nay);
        } else if q.id == b {
            (q.x, q.y) = (nbx, nby);
        }
    }
}

/// THE LINES CLICKED AT THEIR MIDDLE READ AS THEIR MIDDLES WHERE THE CONSTRAINT TAKES POINTS: Coincident always,
/// Horizontal and Vertical when points are picked with them - a line and a point are not one line nor two points, so
/// "the centre above the middle of the line" is what is meant. Every other constraint takes the lines as lines, and so
/// do Horizontal and Vertical on lines alone. Each such line is replaced in the selection by a point held at its middle
/// (`materialize_ref`, the dimension tool's door), laid inside the constraint's own step of undo.
fn middles_for(project: &mut Project, si: usize, sel_sk: &mut qymcad_ui_state::SketchSelection, code: u8) {
    let with_points = sel_sk.items.iter().any(|(k, _)| *k == 0);
    let reads = match code {
        0 => true,
        1 | 2 => with_points,
        _ => false,
    };
    if !reads || sel_sk.at_middle.is_empty() {
        return;
    }
    for eid in std::mem::take(&mut sel_sk.at_middle) {
        let Some(slot) = sel_sk.items.iter().position(|&it| it == (1, eid)) else { continue };
        let Some((a, b)) = line_ends_of(project, si, eid) else { continue };
        sel_sk.items[slot] = (0, qymcad_ui_state::materialize_ref(project, si, qymcad_ui_state::SketchRef::Midpoint(a, b)));
    }
}

/// WHERE A LINE MADE TANGENT TOUCHES THE CIRCLE, when that is a point of the line and not only of the straight line it
/// lies on: the middle of a line picked at its middle (`at_middle`), held there so the line is carried onto the circle by
/// it; or, where the foot of the centre falls off the line, the end nearer to it, so the line runs on to the circle.
/// Tangency alone touches the circle with the straight line the segment lies on: a line apart from the circle turned to
/// it and stayed apart. Reported behaviour: "the line is not lengthened; with the centre of the line picked, the line is
/// not carried onto the circle by its middle". None where the foot already falls on the line.
fn touch_point(project: &mut Project, si: usize, sel_sk: &qymcad_ui_state::SketchSelection, (a, b): (Id, Id), centre: Id) -> Option<Id> {
    if sel_sk.at_middle.iter().any(|&e| line_ends_of(project, si, e) == Some((a, b))) {
        return Some(qymcad_ui_state::materialize_ref(project, si, qymcad_ui_state::SketchRef::Midpoint(a, b)));
    }
    let (pa, pb, c) = (qymcad_ui_state::sketch_pt(project, si, a)?, qymcad_ui_state::sketch_pt(project, si, b)?, qymcad_ui_state::sketch_pt(project, si, centre)?);
    let (dx, dy) = (pb.x - pa.x, pb.y - pa.y);
    let len2 = dx * dx + dy * dy;
    if len2 < 1e-18 {
        return None;
    }
    let t = ((c.x - pa.x) * dx + (c.y - pa.y) * dy) / len2;
    if t < 0.0 {
        Some(a)
    } else if t > 1.0 {
        Some(b)
    } else {
        None
    }
}

/// Every point belonging to the current sketch selection: the points themselves plus the ends and
/// centres of the picked entities. Used to move geometry as a whole.
pub fn sketch_sel_points(project: &Project, sel_sk: &qymcad_ui_state::SketchSelection, si: usize) -> Vec<Id> {
    use qymcad_core::model::EntityKind;
    let Some(s) = project.sketches.get(si) else { return vec![] };
    let mut ids: std::collections::HashSet<Id> = std::collections::HashSet::new();
    for (k, id) in &sel_sk.items {
        match k {
            0 => {
                ids.insert(*id);
            }
            &qymcad_ui_state::SEL_SPLINE => {
                if let Some(sp) = s.splines.iter().find(|sp| sp.points.first() == Some(id)) {
                    ids.extend(sp.points.iter().copied());
                }
            }
            1 => {
                if let Some(e) = s.entities.iter().find(|e| e.id == *id) {
                    match e.kind {
                        EntityKind::Line { a, b } => {
                            ids.insert(a);
                            ids.insert(b);
                        }
                        EntityKind::Circle { center, .. } => {
                            ids.insert(center);
                        }
                        EntityKind::Arc { center, a, b, .. } => {
                            ids.insert(center);
                            ids.insert(a);
                            ids.insert(b);
                        }
                        EntityKind::Ellipse { c, ma, mi } => {
                            ids.insert(c);
                            ids.insert(ma);
                            ids.insert(mi);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    // DRIVEN POINTS ARE SUBTRACTED FROM A GROUP MOVE: selecting half a sketch by a box together with a
    // projection would move the projection too - and at the very first rebuild it would snap back into
    // place, leaving its own geometry adrift. What gets moved is what really belongs to the person.
    let driven = s.projected_points();
    ids.retain(|id| !driven.contains(id));
    ids.into_iter().collect()
}

/// THE DEFINEDNESS OF A SKETCH in one line - the single definition for the whole application.
///
/// The line was produced in two places by two copies, and they had drifted apart: one said "CONSTRAINT
/// CONFLICT" and the other "DIMENSION CONFLICT" about the same state. The priority follows the meaning:
/// A CONFLICT (a real error), then under-defined (there is something left to add), then harmless
/// redundancy (consistent reference dimensions, NOT an error), then fully defined. Any `redun > 0` used
/// to paint it red, and consistent reference dimensions looked like a fault.
pub fn sketch_dof_line(cache: &qymcad_ui_state::Caches, project: &Project, scheme: &qymcad_ui_state::SchemeUi, si: usize) -> (String, Color32) {
    let diag = qymcad_ui_state::sketch_diag(cache, project, si);
    let (dof, redun) = diag.dof;
    let (lbl, col) = if !diag.conflicts.is_empty() {
        (qymcad_i18n::tr("dof-conflict"), qymcad_ui_state::tone_colour(&scheme.pal, qymcad_ui_state::Tone::Error))
    } else if dof > 0 {
        (qymcad_i18n::tr("dof-underdefined"), scheme.pal.underdefined())
    } else if redun > 0 {
        (qymcad_i18n::tr("dof-defined-ref"), scheme.pal.ok_soft())
    } else {
        (qymcad_i18n::tr("dof-fully-defined"), scheme.pal.ok())
    };
    (qymcad_i18n::tr2("dof-line", "n", &dof.to_string(), "state", &lbl), col)
}

/// RELEASE: a full solve with no drag residual, and the undo step is closed.
pub fn finish_point_drag(sk: &mut qymcad_ui_state::SketchCtx) {
    sk.drag.clear();
    if let qymcad_ui_state::Sel::Sketch(si) = *sk.sel {
        sk.project.solve_sketch(si);
        qymcad_ui_state::invalidate(sk.regen);
    }
    qymcad_ui_state::commit_edit(&mut sk.rebuild());
}

/// Selection mode (the arrow): every tool is switched off and geometry is picked by click.
pub fn sketch_select_mode(sk: &mut qymcad_ui_state::SketchCtx) {
    qymcad_ui_state::exit_draw_tools(&mut qymcad_ui_state::tools_in!(sk)); // the single transition from a mode back to selection
    *sk.status = Notice::info(qymcad_i18n::tr("sk-select-hint"));
}

/// Degrees of freedom, redundancy and free points (a wrapper over `qymcad_ui_state::sketch_diag` for the older call
/// sites).
pub fn sketch_status(sk: &mut qymcad_ui_state::SketchCtx, si: usize) -> ((i32, i32), Vec<bool>) {
    let d = qymcad_ui_state::sketch_diag(sk.cache, sk.project, si);
    (d.dof, d.free)
}

/// The underlay edge under the cursor (the name of the edge plus the body), by distance in SCREEN pixels,
/// as with every other sketch pick: in world millimetres the threshold would depend on the zoom.
pub fn nearest_ref_edge(sk: &mut qymcad_ui_state::SketchCtx, si: usize, rect: Rect, pos: Pos2) -> Option<(Id, u32)> {
    let sh = qymcad_ui_state::Sheet { view: *sk.view, rect };
    let (body, edges) = sketch_ref_edges_2d_ids(sk, si);
    if body == 0 {
        return None;
    }
    let mut best: Option<(f32, u32)> = None;
    for (id, poly) in &edges {
        for w in poly.windows(2) {
            let (a, b) = (sh.at(w[0]), sh.at(w[1]));
            // the distance to the segment in screen pixels (a projection onto it, clamped to the ends)
            let (vx, vy) = ((b.x - a.x) as f64, (b.y - a.y) as f64);
            let (wx, wy) = ((pos.x - a.x) as f64, (pos.y - a.y) as f64);
            let len2 = vx * vx + vy * vy;
            let t = if len2 > 1e-12 { ((wx * vx + wy * vy) / len2).clamp(0.0, 1.0) } else { 0.0 };
            let d = ((wx - vx * t).powi(2) + (wy - vy * t).powi(2)).sqrt() as f32;
            if best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, *id));
            }
        }
    }
    best.filter(|(d, _)| *d <= qymcad_ui_state::grab::grab(sk.set, Grab::Curve)).map(|(_, id)| (body, id))
}

/// The edges of the REFERENCE body, projected into the 2D frame of sketch `si` (polylines in sketch
/// coordinates). The body is a face of ITS OWN part (`SketchPlane::Face`) always, or a NEIGHBOUR (an
/// in-context datum snapshot) only during the creation session (`sketch_ref_body`). They come from the
/// `shape` and land exactly. Empty when there is no reference or no frame.
///
/// The same as [`App::sketch_ref_edges_2d`], but WITH THE NAMES of the edges and the source body - for
/// PICKING: to project an edge it is not enough to draw it, one has to know which edge was clicked.
pub fn sketch_ref_edges_2d_ids(sk: &mut qymcad_ui_state::SketchCtx, si: usize) -> (Id, Vec<(u32, Vec<Point2>)>) {
    use qymcad_core::feature::SketchPlane;
    let Some(s) = sk.project.sketches.get(si) else { return (0, Vec::new()) };
    let body = match s.plane {
        SketchPlane::Face(b, _) => Some(sk.project.live_body(b)),
        _ => sk.cmd.ref_body,
    };
    let (Some(body), Some(frame)) = (body, sk.project.sketch_frame(si)) else { return (0, Vec::new()) };
    let Some(shape) = sk.live.shapes.get(&body) else { return (0, Vec::new()) };
    let rel = match (sk.project.sketch_owner(s.id), sk.project.body_owner(body)) {
        (Some(so), Some(bo)) if so != bo => sk.project.relative_transform(bo, so),
        _ => qymcad_core::feature::PLACE_IDENTITY,
    };
    let ident = qymcad_core::feature::is_identity12(&rel);
    let Some(edges) = qymcad_pick::body_edges_cached(sk.cache, sk.live, sk.regen, body) else { return (0, Vec::new()) };
    let (polys, ids) = (&edges.polys, &edges.ids);
    let face_edges: Option<std::collections::HashSet<u32>> = match &s.plane {
        SketchPlane::Face(_, fid) if fid.id != 0 => Some(shape.face_edge_ids(fid.id).into_iter().collect()),
        _ => None,
    };
    let out = polys
        .iter()
        .zip(ids.iter().copied())
        .filter(|(_, id)| *id != 0 && face_edges.as_ref().is_none_or(|set| set.contains(id)))
        .map(|(poly, id)| {
            let pts = poly
                .iter()
                .map(|p| {
                    let mut l = [p[0] as f64, p[1] as f64, p[2] as f64];
                    if !ident {
                        l = qymcad_core::feature::apply12(&rel, l);
                    }
                    frame.project(qymcad_core::geom::Point3::new(l[0], l[1], l[2]))
                })
                .collect();
            (id, pts)
        })
        .collect();
    (body, out)
}

/// A click in the sketch workbench: pick the nearest point or entity (Shift adds to the selection).
/// WHICH DRAWING TOOL A HOTKEY ACTION NAMES, or `None` when it names something else.
///
/// The table belongs to the sketch, not to the window: the window's job is to hear the key and to take the
/// tool, and it had eleven lines of "this action means that number" written into it - one per tool, growing
/// by one with every tool added.
pub fn tool_for_action(action: &str) -> Option<u8> {
    Some(match action {
        "sketch.line" => 1,
        "sketch.rect" => 2,
        "sketch.circle" => 3,
        "sketch.arc" => 4,
        "sketch.point" => 5,
        "sketch.polygon" => 6,
        "sketch.slot" => 7,
        "sketch.ellipse" => 8,
        "sketch.spline" => 9,
        "sketch.text" => 11,
        _ => return None,
    })
}

/// The two ends of line `eid` of sketch `si`.
fn line_ends_of(project: &Project, si: usize, eid: Id) -> Option<(Id, Id)> {
    match project.sketches.get(si)?.entities.iter().find(|e| e.id == eid)?.kind {
        qymcad_core::model::EntityKind::Line { a, b } => Some((a, b)),
        _ => None,
    }
}

/// The ends of the line of sketch `si` whose middle stands at `at`, where the snap put its triangle.
fn line_with_middle_at(project: &Project, si: usize, at: Point2) -> Option<(Id, Id)> {
    let s = project.sketches.get(si)?;
    let pt = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
    s.entities.iter().find_map(|e| match e.kind {
        qymcad_core::model::EntityKind::Line { a, b } => {
            let ((ax, ay), (bx, by)) = (pt(a)?, pt(b)?);
            (((ax + bx) / 2.0 - at.x).abs() < 1e-9 && ((ay + by) / 2.0 - at.y).abs() < 1e-9).then_some((a, b))
        }
        _ => None,
    })
}

/// DELETE WHAT IS SELECTED INSIDE A SKETCH: a text, a note, a constraint, or the geometry.
///
/// One thing at a time and in that order, because a text and a note are picked separately from the
/// geometry and a person pressing Delete means the one they just clicked.
///
/// This lived in the frame's key handler, inside the application object, although every line of it is
/// about a sketch and nothing about a window: deleting an entity, a point, a constraint, a note. It is the
/// sketcher's, and it is here.
pub fn delete_selected_in_sketch(sk: &mut qymcad_ui_state::SketchCtx, si: usize) {
    // THE BOUNDARY OF AN OPERATION: one step of undo, named after the tool; a key that deleted nothing lays none
    qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, qymcad_i18n::tr("sk-delete"));
    delete_selected_in(sk, si);
    qymcad_ui_state::commit_edit_if_changed(&mut sk.rebuild());
}

fn delete_selected_in(sk: &mut qymcad_ui_state::SketchCtx, si: usize) {
    if let Some(ti) = sk.annot.text.take() {
        if ti < sk.project.sketches[si].texts.len() {
            sk.project.delete_sketch_text(si, ti);
            qymcad_ui_state::invalidate(&mut *sk.regen);
            *sk.status = Notice::info(qymcad_i18n::tr("in-text-deleted"));
        }
        return;
    }
    if let Some(ni) = sk.annot.note.take() {
        if ni < sk.project.sketches[si].notes.len() {
            sk.project.sketches[si].notes.remove(ni);
            *sk.status = Notice::info(qymcad_i18n::tr("in-note-deleted"));
        }
        return;
    }
    if let Some(ci) = sk.gsel.constraint.take() {
        if ci < sk.project.sketches[si].constraints.len() {
            sk.project.delete_sketch_constraint(si, ci); // also cleans up an orphaned midpoint
            qymcad_ui_state::invalidate(&mut *sk.regen);
            *sk.status = Notice::info(qymcad_i18n::tr("in-constraint-deleted"));
        }
        return;
    }
    let eids: Vec<Id> = sk.sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
    // system points (the origin and the axes) and DRIVEN ones (projections of a body) are not deleted one
    // by one
    let sys: std::collections::HashSet<Id> = sk.project.sketches[si].immovable_points();
    let mut pids: Vec<Id> = sk.sel_sk.items.iter().filter(|(k, id)| *k == 0 && !sys.contains(id)).map(|(_, id)| *id).collect();
    // a spline picked by its curve goes with its nodes, as a line goes with its ends
    let s = &sk.project.sketches[si];
    pids.extend(
        sk.sel_sk
            .items
            .iter()
            .filter(|(k, _)| *k == qymcad_ui_state::SEL_SPLINE)
            .filter_map(|(_, id)| s.splines.iter().find(|sp| sp.points.first() == Some(id)))
            .flat_map(|sp| sp.points.iter().copied()),
    );
    if eids.is_empty() && pids.is_empty() {
        return;
    }
    if !eids.is_empty() {
        sk.project.delete_sketch_curves(si, &eids); // a piece of a projection cuts it
    }
    if !pids.is_empty() {
        sk.project.delete_points(si, &pids); // the points go together with the lines and arcs incident to them
    }
    sk.project.solve_sketch(si);
    sk.sel_sk.clear(); // the selection and whatever was waiting for it
    qymcad_ui_state::invalidate(&mut *sk.regen);
    *sk.status = Notice::info(qymcad_i18n::tr("in-deleted"));
}

/// EDIT -> INSERT IN A SKETCH: with geometry in the clipboard, the placing is armed (a ghost, then a click), the tool in
/// hand put down first - one thing in hand at a time, as the copy does. False outside a sketch or with nothing to insert,
/// when the paste goes to the tree.
pub fn arm_paste(sk: &mut qymcad_ui_state::SketchCtx) -> bool {
    if qymcad_ui_state::edit_si(&*sk.project, &*sk.sketch_ses).is_none() || !sk.clip.geom.as_ref().is_some_and(|c| !c.is_empty()) {
        return false;
    }
    qymcad_ui_state::exit_draw_tools(&mut qymcad_ui_state::tools_in!(sk));
    (sk.clip.geom_pending, sk.clip.geom_place) = (None, true);
    *sk.status = Notice::info(qymcad_i18n::tr("g-insert-click"));
    true
}

mod props;
pub use props::*;
mod corners;
pub use corners::*;
mod popups;
pub use popups::*;
mod ties;
pub use ties::*;
mod constraints;
pub use constraints::*;
mod dims;
pub use dims::*;
mod extend;
pub use extend::*;
mod menu;
pub use menu::*;
mod clicks;
pub use clicks::*;
mod offset;
pub use offset::*;
mod drag;
pub use drag::*;
mod snap;
pub use snap::*;
