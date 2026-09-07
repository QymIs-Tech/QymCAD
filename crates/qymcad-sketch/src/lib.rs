//! THE SKETCH WORKBENCH.
//!
//! Drawing entities, constraints and dimensions, dragging points, trimming, patterns, text and notes, the
//! diagnostics of degrees of freedom. Every function here works over `qymcad_ui_state::SketchCtx` - the records the sketch
//! edits - and asks the application for nothing.

use egui::{Color32, Pos2, Rect};
use egui_phosphor::regular as ph;
use qymcad_core::geom::Point2;
use qymcad_core::model::{Id, Project};
use qymcad_ui_state::grab::Grab;
use qymcad_ui_state::WinKind;


/// The sign of a cross product, as the word an arc is stored with: the sketcher works out the turn from the
/// geometry under the pointer, and the model keeps it as a word rather than as a bare flag.
fn winding(ccw: bool) -> qymcad_core::feature::Winding {
    if ccw {
        qymcad_core::feature::Winding::Ccw
    } else {
        qymcad_core::feature::Winding::Cw
    }
}

pub fn sketch_props(pr: &mut qymcad_ui_state::PropsCtx, ui: &mut egui::Ui, si: usize) {
    let lin = qymcad_ui_state::lineage_of(&*pr.project, Some(pr.project.sketches[si].id));
    if let Some(n) = qymcad_ui_state::props_header(ui, ph::POLYGON, "sk-props", qymcad_ui_state::NameSlot::Editable(pr.project.sketches[si].name.clone()), &lin) {
        pr.project.sketches[si].name = n;
    }
    // the plane the sketch is placed on
    {
        use qymcad_core::feature::{BasePlane, SketchPlane};
        let pl = match pr.project.sketches[si].plane {
            SketchPlane::World(BasePlane::XY) => qymcad_i18n::tr("sk-plane-xy"),
            SketchPlane::World(BasePlane::XZ) => qymcad_i18n::tr("sk-plane-xz"),
            SketchPlane::World(BasePlane::YZ) => qymcad_i18n::tr("sk-plane-yz"),
            SketchPlane::Datum(id) => qymcad_i18n::tr1("sk-on-work-plane", "id", &id.to_string()),
            SketchPlane::Face(body, _) => qymcad_i18n::tr1("sk-on-body-face", "b", &body.to_string()),
        };
        ui.label(egui::RichText::new(qymcad_i18n::tr1("sk-on", "what", &pl)).weak().small());
    }
    let cids = pr.project.sketches[si].contour_ids.clone();
    let src = pr.project.sketches[si].source;
    ui.label(qymcad_i18n::tr1("sk-contours-n", "n", &cids.len().to_string()));
    if let Some(srcid) = src {
        if let Some(sf) = pr.project.sources.iter().find(|x| x.id == srcid) {
            ui.label(egui::RichText::new(qymcad_i18n::tr2("sk-source", "name", &sf.name, "kb", &qymcad_i18n::num(sf.data.len() as f64 / 1024.0, 1))).weak().small());
        }
    }

    // --- while editing: a clean panel, with no clutter ---
    if qymcad_ui_state::edit_si(&*pr.project, &*pr.sketch_ses) == Some(si) {
        use qymcad_core::model::Constraint;
        // system points (the origin and the axes) and their `Fixed` constraints are hidden from the list
        // and the counter, and are never deleted
        let sys_pts: std::collections::HashSet<Id> = pr.project.sketches[si].system_ids().into_iter().collect();
        let is_sys = |c: &Constraint| matches!(c, Constraint::Fixed { p } if sys_pts.contains(p));
        let np = pr.project.sketches[si].points.iter().filter(|p| !sys_pts.contains(&p.id)).count();
        let nc = pr.project.sketches[si].constraints.iter().filter(|c| !is_sys(c)).count();
        ui.label(egui::RichText::new(qymcad_i18n::tr2("sk-counts", "np", &np.to_string(), "nc", &nc.to_string())).weak().small());
        // the degrees-of-freedom readout (the rank of the Jacobian, THROUGH THE CACHE: it is not computed
        // every frame)
        let diag = qymcad_ui_state::sketch_diag(&*pr.cache, &*pr.project, si);
        let (dof, redun) = diag.dof;
        let conflicts = diag.conflicts.len();
        let (dline, dcol) = sketch_dof_line(&*pr.cache, &*pr.project, pr.scheme, si);
        ui.label(egui::RichText::new(dline).color(dcol).small());
        if conflicts > 0 {
            // AN HONEST WORDING: it is A SET of constraints that conflicts, and no single one in it is
            // "the culprit" - any of them can be removed. It used to say that the dimensions contradicted
            // the geometry, although geometric constraints can conflict too, and the geometry has nothing
            // to do with it: it stands where the compromise between incompatible constraints put it.
            ui.label(
                egui::RichText::new(qymcad_i18n::tr1("sk-conflicts-n", "n", &conflicts.to_string()))
                    .color(pr.scheme.pal.error())
                    .small(),
            );
            ui.label(egui::RichText::new(qymcad_i18n::tr1("sk-conflict-advice", "icon", ph::RULER)).weak().small());
        } else if redun > 0 {
            ui.label(egui::RichText::new(qymcad_i18n::tr1("sk-redundant-n", "n", &redun.to_string())).color(pr.scheme.pal.note()).small());
        }
        if dof > 0 {
            ui.label(egui::RichText::new(qymcad_i18n::tr1("sk-dof-n", "n", &dof.to_string())).weak().small());
        }
        ui.separator();
        ui.checkbox(&mut pr.win.constraints, qymcad_i18n::tr("sk-show-constraints")).on_hover_text(qymcad_i18n::tr("sk-show-constraints-hint"));
        ui.separator();
        // the parameters of the project (the named dimensions and formulas)
        if ui.button(format!("{} {}", ph::FUNCTION, qymcad_i18n::tr("sk-params-btn"))).on_hover_text(qymcad_i18n::tr("sk-params-hint")).clicked() {
            pr.win.open(WinKind::Params);
        }
        // stitching coincident points cures a corner that has fallen apart on shapes already drawn
        if ui.button(format!("{} {}", ph::LINK, qymcad_i18n::tr("sk-stitch-btn"))).on_hover_text(qymcad_i18n::tr("sk-merge-ends-hint")).clicked() {
            let tol = (10.0 / pr.view.scale as f64).clamp(1e-4, 1.0);
            let n = pr.project.merge_close_points(si, tol);
            pr.project.solve_sketch(si);
            qymcad_ui_state::invalidate(&mut *pr.regen);
            *pr.status = if n > 0 { qymcad_i18n::tr1("sk-stitched-n", "n", &n.to_string()) } else { qymcad_i18n::tr("sk-no-coincident-points") };
        }
        ui.label(egui::RichText::new(qymcad_i18n::tr("sk-welcome")).weak().small());
        ui.separator();
        ui.label(egui::RichText::new(qymcad_i18n::tr("sk-constraints-and-dims")).strong());
        ui.label(egui::RichText::new(qymcad_i18n::tr1("sk-list-hint", "icon", ph::TRASH)).weak().small());
        if nc == 0 {
            ui.label(egui::RichText::new(qymcad_i18n::tr("sk-not-yet")).weak().small());
        }
        pr.hover.constraint = None;
        let mut cons = pr.project.sketches[si].constraints.clone();
        // REDUNDANT constraints (which ones exactly over-define the sketch) are marked in the list as
        // candidates for removal. BUT the harmless redundancy of CONSISTENT dimensions (reference ones,
        // not an error) is NOT painted red - that would be a false alarm. The warning appears only on
        // (a) a redundant GEOMETRIC constraint, which does need removing, and (b) a conflicting
        // dimension. They form an interdependent group: removing any marked one lifts the over-definition.
        let conflict_set = diag.conflicts.clone();
        let cur_sel = pr.gsel.constraint;
        let (mut rm, mut changed, mut sel_click, mut hov) = (None, false, None, None);
        let mut to_driven: Option<usize> = None;
        // Does the sketch contain fillets (structural tangencies)? Their Jacobian at the point of contact
        // is degenerate (parallel to the intrinsic of the arc), so the rank analysis falsely marks as
        // redundant not only the tangencies themselves but the constraints ENTANGLED with them (the
        // horizontals and verticals of a rectangle, the `PointOnLine` of a virtual corner). While fillets
        // are present, rank redundancy of GEOMETRIC constraints is unreliable, so none of them is painted
        // red (real contradictions of values are still caught by `sketch_conflicts`, which works on the
        // geometry and is reliable).
        let flagged = qymcad_ui_state::flagged_redundant(&*pr.cache, &*pr.project, si); // ONE rule for the list and for the canvas
        egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
            for (ci, c) in cons.iter_mut().enumerate() {
                if is_sys(c) {
                    continue; // a system `Fixed` on the origin or an axis is not shown
                }
                let is_sel = cur_sel == Some(ci);
                // is it a dimension? (consistent redundancy among dimensions is harmless and gets no warning)
                let is_dim = matches!(c, Constraint::Distance { .. } | Constraint::Angle { .. } | Constraint::DistancePL { .. } | Constraint::AngleLines { .. } | Constraint::ArcLength { .. } | Constraint::Diameter { .. } | Constraint::EdgeDistance { .. });
                // which redundant ones to mark is decided by ONE rule, shared with the canvas glyphs
                let redun_geom = flagged.contains(&ci);
                // A CONFLICTING constraint gets a separate, sharper mark: it is an error (the sketch does
                // not solve), whereas redundancy only means "this one may be removed". Both used to give
                // the same glyph.
                let in_conflict = conflict_set.contains(&ci);
                let is_driven = c.is_driven();
                let flag = redun_geom || in_conflict;
                let row = ui.horizontal(|ui| {
                    if in_conflict {
                        ui.colored_label(pr.scheme.pal.error(), ph::WARNING_OCTAGON)
                            .on_hover_text(qymcad_i18n::tr("sk-conflict-hint"));
                    } else if flag {
                        ui.colored_label(pr.scheme.pal.error_mild(), ph::WARNING).on_hover_text(qymcad_i18n::tr("sk-overdefined-hint"));
                    }
                    match c {
                        Constraint::Distance { d, .. } => {
                            if ui.selectable_label(is_sel, qymcad_i18n::tr("sk-dim")).clicked() {
                                sel_click = Some(ci);
                            }
                            changed |= ui.add(egui::DragValue::new(d).speed(0.2).range(0.01..=100000.0).suffix(qymcad_i18n::tr("unit-mm-suffix"))).changed();
                        }
                        Constraint::Angle { deg, .. } => {
                            if ui.selectable_label(is_sel, qymcad_i18n::tr("sk-angle")).clicked() {
                                sel_click = Some(ci);
                            }
                            changed |= ui.add(egui::DragValue::new(deg).speed(0.5).range(0.1..=359.9).suffix("°")).changed();
                        }
                        other => {
                            // THE PARTICIPANTS IN THE ROW: "Horizontal: Line 3". Without them a list of
                            // thirty constraints shows that constraints exist but gives no way to find the
                            // one wanted - four rows of "Horizontal" in a row are indistinguishable.
                            let parts = constraint_parts(&qymcad_ui_state::DrawCtx { cam: pr.cam, set: &*pr.set, scheme: pr.scheme, project: &*pr.project, active_path: pr.active_path }, si, other);
                            let text = if parts.is_empty() { qymcad_ui_state::constraint_label(other) } else { format!("{}: {}", qymcad_ui_state::constraint_label(other), parts.join(", ")) };
                            if ui.selectable_label(is_sel, text).clicked() {
                                sel_click = Some(ci);
                            }
                        }
                    }
                    // RESOLVE A CONFLICT IN ONE CLICK: the conflicting dimension becomes a driven one - it
                    // stops driving the geometry but stays on the drawing and shows the actual value. That
                    // is the standard way out in a professional CAD; without it the only option left is to
                    // delete the dimension.
                    if in_conflict && is_dim && !is_driven && ui.small_button(ph::RULER).on_hover_text(qymcad_i18n::tr("sk-make-driven-hint")).clicked() {
                        to_driven = Some(ci);
                    }
                    if ui.small_button(ph::TRASH).on_hover_text(qymcad_i18n::tr("sk-delete")).clicked() {
                        rm = Some(ci);
                    }
                })
                .response;
                if ui.rect_contains_pointer(row.rect) {
                    hov = Some(ci);
                }
            }
        });
        pr.hover.constraint = hov;
        if let Some(ci) = sel_click {
            pr.gsel.constraint = Some(ci);
        }
        if changed {
            pr.project.sketches[si].constraints = cons;
            pr.project.solve_sketch(si);
            qymcad_ui_state::invalidate(&mut *pr.regen);
        }
        if let Some(ci) = to_driven {
            make_dim_driven(&mut *pr.project, &mut *pr.regen, &mut *pr.status, si, ci);
        }
        if let Some(ci) = rm {
            pr.project.delete_sketch_constraint(si, ci);
            pr.gsel.constraint = None;
            qymcad_ui_state::invalidate(&mut *pr.regen);
        }
        return;
    }

    // NOT in edit mode: only what is relevant. A compact summary plus a way into editing. Geometry,
    // dimensions and constraints are edited INSIDE the sketch (through Edit or a double click). A body is
    // made from a sketch by the Extrude or Revolve command on the toolbar - a parametric feature in the
    // timeline - rather than by one-off buttons here.
    let (dline, dcol) = sketch_dof_line(&*pr.cache, &*pr.project, pr.scheme, si);
    ui.separator();
    ui.label(egui::RichText::new(dline).color(dcol).small());
    ui.add_space(2.0);
    if ui.button(format!("{} {}", ph::PENCIL_SIMPLE, qymcad_i18n::tr("sk-edit-btn"))).clicked() {
        pr.ask.push(qymcad_ui_state::PropsAsk::EnterSketch(si));
    }
    ui.label(egui::RichText::new(qymcad_i18n::tr("sk-body-from-sketch")).weak().small());

    ui.separator();
    if ui.button(format!("{} {}", ph::TRASH, qymcad_i18n::tr("act-delete-sketch"))).clicked() {
        qymcad_ui_state::ask_delete(&mut *pr.deferred, qymcad_ui_state::Sel::Sketch(si)); // the same path the tree takes - the question and the deletion
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
    let pt = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
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

/// What is under the cursor in a sketch: (kind, Id). 0 is a point, 1 an entity (a line, an arc, a
/// circle), 2 a primitive (a contour). Ordinary geometry takes priority over construction geometry.
pub fn sketch_hit(pick: &qymcad_ui_state::PickCtx, rect: Rect, pos: Pos2, si: usize) -> Option<(u8, Id)> {
    let sh = qymcad_ui_state::Sheet { view: *pick.view, rect: rect };
    use qymcad_core::model::EntityKind;
    let s = pick.project.sketches.get(si)?;
    let pt = |id: Id| s.points.iter().find(|p| p.id == id).map(|p| Point2::new(p.x, p.y));
    // the ends of THE AXES are not picked as ordinary points (they mark infinite lines); the origin
    // deliberately STAYS pickable (it is needed for a coincidence with the origin) - hence `axis_pts`
    // rather than `system_ids`
    let is_axis_ref = |id: Id| s.axis_pts.contains(&id);
    // 1) the nearest point
    let mut best_pt: Option<(f32, Id)> = None;
    for p in &s.points {
        if is_axis_ref(p.id) {
            continue;
        }
        let d = sh.at(Point2::new(p.x, p.y)).distance(pos);
        if d <= qymcad_ui_state::grab::grab(pick.set, Grab::Point) && best_pt.is_none_or(|(bd, _)| d < bd) {
            best_pt = Some((d, p.id));
        }
    }
    if let Some((_, id)) = best_pt {
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
            EntityKind::Arc { center, a, .. } => match (pt(center), pt(a)) {
                (Some(c), Some(pa)) => {
                    let sc = sh.at(c);
                    let r = ((pa.x - c.x).powi(2) + (pa.y - c.y).powi(2)).sqrt();
                    let rp = (sh.at(Point2::new(c.x + r, c.y)).x - sc.x).abs();
                    (sc.distance(pos) - rp).abs()
                }
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
    best_e.map(|(_, _, id)| (1u8, id))
}

/// THE ENTITIES A CONSTRAINT TOUCHES, derived from its points.
///
/// The list of constraints used to name only THE KIND: horizontal, horizontal, vertical, vertical - four
/// identical rows. On a sketch with thirty constraints such a list is useless: it shows that constraints
/// exist but gives no way to find the one wanted. Highlighting on hover already existed, but it answers
/// "where is this one" rather than "which one do I need".
pub fn constraint_parts(dc: &qymcad_ui_state::DrawCtx, si: usize, c: &qymcad_core::model::Constraint) -> Vec<String> {
    use qymcad_core::model::EntityKind as EK;
    let Some(s) = dc.project.sketches.get(si) else { return Vec::new() };
    let owner = |pid: Id| -> Option<Id> {
        s.entities
            .iter()
            .find(|e| match e.kind {
                EK::Line { a, b } => a == pid || b == pid,
                EK::Arc { center, a, b, .. } => center == pid || a == pid || b == pid,
                EK::Circle { center, .. } => center == pid,
                EK::Ellipse { c, ma, mi } => c == pid || ma == pid || mi == pid,
            })
            .map(|e| e.id)
    };
    let mut out: Vec<String> = Vec::new();
    for pid in c.points() {
        if let Some(eid) = owner(pid) {
            let name = sketch_entity_name(dc.project, si, eid);
            if !name.is_empty() && !out.contains(&name) {
                out.push(name);
            }
        }
    }
    out
}

/// A corner fillet radius or a chamfer leg, at the corner clicked.
pub fn corner_input_popup(cc: &mut qymcad_ui_state::CornerCtx, ctx: &egui::Context, rect: Rect) {
    // the popup for a corner fillet radius or a chamfer leg (Enter applies, Esc cancels)
    if let Some((si, pid, chamfer)) = cc.corner.at {
        let at = cc.corner.pos.unwrap_or_else(|| rect.center());
        let want_focus = std::mem::take(&mut cc.corner.focus);
        let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
        let (mut apply, mut cancel) = (false, false);
        let mut buf = std::mem::take(&mut cc.corner.buf);
        egui::Area::new(egui::Id::new(("cornerinput", si, pid))).fixed_pos(qymcad_ui_state::clamp_popup(at, rect) + egui::vec2(10.0, -10.0)).order(egui::Order::Foreground).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(if chamfer { qymcad_i18n::tr("cmd-leg") } else if pid == 0 { qymcad_i18n::tr("sk-r-all-corners") } else { qymcad_i18n::tr("sk-radius") });
                    let r0 = qymcad_ui_state::focus_edit(ui, &mut buf, 64.0, "", want_focus);
                    if r0.lost_focus() && enter {
                        apply = true;
                    }
                    if ui.button(ph::CHECK).clicked() {
                        apply = true;
                    }
                    if ui.button(ph::X).clicked() {
                        cancel = true;
                    }
                });
            });
        });
        cc.corner.buf = buf;
        if apply {
            let r = parse_num(cc.project, &cc.corner.buf.clone()).unwrap_or(0.0);
            if r > 1e-6 {
                cc.tool_prefs.fillet = r; // sticky: the next corner offers the same value
                let ok_n = if pid == 0 {
                    // the set from clicking a shape; failing that the selection; failing that the whole sketch
                    let only = cc.corner.only.take().or_else(|| {
                        let sel: std::collections::HashSet<Id> = cc.sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
                        (!sel.is_empty()).then_some(sel)
                    });
                    cc.project.fillet_all_corners_of(si, r, only.as_ref())
                } else if chamfer {
                    cc.project.chamfer_at_vertex(si, pid, r) as usize
                } else {
                    cc.project.fillet_at_vertex(si, pid, r) as usize
                };
                if ok_n > 0 {
                    cc.sel_sk.clear(); // the selection and whatever was waiting for it
                    qymcad_ui_state::invalidate(cc.regen);
                    *cc.status = if pid == 0 { qymcad_i18n::tr1("sk-filleted-n", "n", &ok_n.to_string()) } else { qymcad_i18n::tr("sk-done") };
                } else {
                    *cc.status = format!("{} {}", ph::WARNING, qymcad_i18n::tr("sk-fillet-too-big"));
                }
            }
            cc.corner.clear(); // ALL of the input: `only` (a restricted set of corners) otherwise travelled
            // into the next call, and "round every corner" silently worked on the old set
        }
        if cancel || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            cc.corner.clear();
        }
    }
}

/// An ellipse: width by height (the full axes are twice the semi-axes).
pub fn ellipse_input_popup(pl: &mut qymcad_ui_state::PlaceCtx, ctx: &egui::Context, rect: Rect, si: usize) {
    // an ellipse: width by height (the full axes are twice the major and twice the minor semi-axis) - the
    // entity is found through its centre handle
    if let Some((handle, click)) = pl.place.ellipse() {
        let cur = pl.project.ellipse_axes(si, handle);
        if let Some((rx, ry)) = cur {
            let (w0, h0) = (2.0 * rx, 2.0 * ry);
            let at = (qymcad_ui_state::Sheet { view: *pl.view, rect: rect }).at(Point2::new(click.x + rx, click.y + ry));
            let want_focus = std::mem::take(&mut pl.place.focus);
            if want_focus {
                pl.place.buf[0] = format!("{}", (w0 * 1000.0).round() / 1000.0);
                pl.place.buf[1] = format!("{}", (h0 * 1000.0).round() / 1000.0);
            }
            let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
            let (mut chg, mut close, mut got_focus) = (false, false, false);
            let mut buf = [std::mem::take(&mut pl.place.buf[0]), std::mem::take(&mut pl.place.buf[1])];
            egui::Area::new(egui::Id::new(("ellinput", si))).fixed_pos(qymcad_ui_state::clamp_popup(at, rect) + egui::vec2(10.0, -10.0)).order(egui::Order::Foreground).show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(qymcad_i18n::tr("sk-width-short"));
                        let r0 = qymcad_ui_state::focus_edit(ui, &mut buf[0], 60.0, "", want_focus);
                        chg |= r0.changed();
                        got_focus |= r0.has_focus();
                        ui.label(qymcad_i18n::tr("sk-height-short"));
                        let r1 = qymcad_ui_state::focus_edit(ui, &mut buf[1], 60.0, "", false);
                        chg |= r1.changed();
                        got_focus |= r1.has_focus();
                        if (r0.lost_focus() || r1.lost_focus()) && enter {
                            close = true;
                        }
                        if ui.button(ph::CHECK).clicked() {
                            close = true;
                        }
                    });
                });
            });
            pl.place.buf = buf.clone();
            if got_focus {
                pl.place.focus = false;
            }
            if chg {
                let nw = parse_num(pl.project, &buf[0]).unwrap_or(w0).max(0.02);
                let nh = parse_num(pl.project, &buf[1]).unwrap_or(h0).max(0.02);
                pl.project.set_ellipse_axes(si, handle, nw / 2.0, nh / 2.0);
                qymcad_ui_state::invalidate(pl.regen);
            }
            if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                pl.place.clear();
                pl.place.focus = false;
            }
        } else {
            pl.place.clear();
        }
    }
}

/// A rectangle: width by height, rebuilt from the anchor corner so the signs are kept.
pub fn rect_input_popup(pl: &mut qymcad_ui_state::PlaceCtx, ctx: &egui::Context, rect: Rect, si: usize) {
    // a rectangle: width by height (text fields, with auto-focus, Tab and Enter)
    if let Some((a, b, ids)) = pl.place.rect() {
        let (w0, h0) = ((b.x - a.x).abs(), (b.y - a.y).abs());
        let (sx, sy) = ((b.x - a.x).signum(), (b.y - a.y).signum());
        let at = (qymcad_ui_state::Sheet { view: *pl.view, rect: rect }).at(Point2::new(a.x.max(b.x), a.y.max(b.y)));
        let want_focus = std::mem::take(&mut pl.place.focus);
        if want_focus {
            pl.place.buf[0] = format!("{}", (w0 * 1000.0).round() / 1000.0);
            pl.place.buf[1] = format!("{}", (h0 * 1000.0).round() / 1000.0);
        }
        let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
        let (mut chg, mut close, mut got_focus) = (false, false, false);
        let mut buf = [std::mem::take(&mut pl.place.buf[0]), std::mem::take(&mut pl.place.buf[1])];
        egui::Area::new(egui::Id::new(("rectinput", si))).fixed_pos(qymcad_ui_state::clamp_popup(at, rect) + egui::vec2(10.0, -10.0)).order(egui::Order::Foreground).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(qymcad_i18n::tr("sk-width-short"));
                    let r0 = qymcad_ui_state::focus_edit(ui, &mut buf[0], 60.0, "", want_focus);
                    chg |= r0.changed();
                    got_focus |= r0.has_focus();
                    ui.label(qymcad_i18n::tr("sk-height-short"));
                    let r1 = qymcad_ui_state::focus_edit(ui, &mut buf[1], 60.0, "", false);
                    chg |= r1.changed();
                    got_focus |= r1.has_focus();
                    if (r0.lost_focus() || r1.lost_focus()) && enter {
                        close = true;
                    }
                    if ui.button(ph::CHECK).clicked() {
                        close = true;
                    }
                });
            });
        });
        pl.place.buf = buf.clone();
        if got_focus {
            pl.place.focus = false;
        }
        if chg {
            let nw = parse_num(pl.project, &buf[0]).unwrap_or(w0).max(0.01);
            let nh = parse_num(pl.project, &buf[1]).unwrap_or(h0).max(0.01);
            let nb = Point2::new(a.x + if sx < 0.0 { -nw } else { nw }, a.y + if sy < 0.0 { -nh } else { nh });
            pl.project.delete_entities(si, &ids);
            let nids = pl.project.add_rect_entity(si, a.x, a.y, nb.x, nb.y, qymcad_core::feature::Purpose::Real);
            pl.place.set(qymcad_ui_state::PlacingShape::Rect(a, nb, nids));
            qymcad_ui_state::invalidate(pl.regen);
        }
        if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            pl.place.clear();
            pl.place.focus = false;
        }
    }
}

/// A polygon: the radius of the construction circle plus the rotation angle.
pub fn poly_input_popup(pl: &mut qymcad_ui_state::PlaceCtx, ctx: &egui::Context, rect: Rect, si: usize) {
    // a polygon: the radius of the construction circle plus the rotation angle (it is rebuilt)
    if let Some(cid) = pl.place.poly() {
        if let (Some((cx, cy, r)), Some(ang)) = (pl.project.polygon_circle(si, cid), pl.project.polygon_angle(si, cid)) {
            let at = (qymcad_ui_state::Sheet { view: *pl.view, rect: rect }).at(Point2::new(cx + r, cy));
            let want_focus = std::mem::take(&mut pl.place.focus);
            if want_focus {
                pl.place.buf[0] = format!("{}", (r * 1000.0).round() / 1000.0);
                pl.place.buf[1] = format!("{}", (ang.to_degrees() * 100.0).round() / 100.0);
            }
            let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
            let (mut chg, mut close, mut got_focus) = (false, false, false);
            let mut buf = [std::mem::take(&mut pl.place.buf[0]), std::mem::take(&mut pl.place.buf[1])];
            egui::Area::new(egui::Id::new(("polyinput", si))).fixed_pos(qymcad_ui_state::clamp_popup(at, rect) + egui::vec2(10.0, -10.0)).order(egui::Order::Foreground).show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(qymcad_i18n::tr("sk-radius"));
                        let r0 = qymcad_ui_state::focus_edit(ui, &mut buf[0], 60.0, "", want_focus);
                        chg |= r0.changed();
                        got_focus |= r0.has_focus();
                        ui.label(qymcad_i18n::tr("sk-angle-deg"));
                        let r1 = qymcad_ui_state::focus_edit(ui, &mut buf[1], 50.0, "", false);
                        chg |= r1.changed();
                        got_focus |= r1.has_focus();
                        if (r0.lost_focus() || r1.lost_focus()) && enter {
                            close = true;
                        }
                        if ui.button(ph::CHECK).clicked() {
                            close = true;
                        }
                    });
                });
            });
            pl.place.buf = buf.clone();
            if got_focus {
                pl.place.focus = false;
            }
            if chg {
                let nr = parse_num(pl.project, &buf[0]).unwrap_or(r).max(0.01);
                let na = parse_num(pl.project, &buf[1]).map(|d| d.to_radians()).unwrap_or(ang);
                pl.project.set_polygon_radius(si, cid, nr);
                pl.project.set_polygon_angle(si, cid, na);
                qymcad_ui_state::invalidate(pl.regen);
            }
            if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                pl.place.clear();
                pl.place.focus = false;
            }
        } else {
            pl.place.clear();
        }
    }
}

/// Automatic constraints while drawing the segment prev-p1-p2: horizontal or vertical, perpendicular to
/// the previous segment, and a point-on-edge for the new end. Every constraint is added ONLY if it is
/// independent (does not over-define the sketch), so no redundant ones appear.
pub fn infer_on_segment(project: &mut Project, view: qymcad_ui_state::View2d, si: usize, prev: Option<Point2>, p1: Point2, p2: Point2) {
    use qymcad_core::model::Constraint;
    let a = project.sketch_point_at(si, p1.x, p1.y, 1e-6);
    let b = project.sketch_point_at(si, p2.x, p2.y, 1e-6);
    let (dx, dy) = ((p2.x - p1.x).abs(), (p2.y - p1.y).abs());
    let tol = 0.06; // ~3.5°
    // 1) horizontal or vertical
    let mut axis_aligned = false;
    if dy <= dx * tol && dx > 1e-6 {
        axis_aligned = project.add_constraint_if_independent(si, Constraint::Horizontal { a, b });
    } else if dx <= dy * tol && dy > 1e-6 {
        axis_aligned = project.add_constraint_if_independent(si, Constraint::Vertical { a, b });
    }
    // 2) perpendicular to the previous segment (only when the new one did not land on an axis, or it
    // would be a duplicate)
    if !axis_aligned {
        if let Some(pv) = prev {
            let pa = project.sketch_point_at(si, pv.x, pv.y, 1e-6);
            let (ux, uy) = (p1.x - pv.x, p1.y - pv.y);
            let (vx, vy) = (p2.x - p1.x, p2.y - p1.y);
            let (lu, lv) = ((ux * ux + uy * uy).sqrt(), (vx * vx + vy * vy).sqrt());
            if lu > 1e-6 && lv > 1e-6 {
                let cosang = (ux * vx + uy * vy) / (lu * lv);
                if cosang.abs() < 0.06 {
                    project.add_constraint_if_independent(si, Constraint::Perpendicular { a: pa, b: a, c: a, d: b });
                }
            }
        }
    }
    // 2b) PARALLEL to the nearest non-axis line (when the new one did not land on an axis itself)
    if !axis_aligned {
        if let Some((la, lb)) = qymcad_pick::nearest_parallel_line(project, si, p1, p2, a, b) {
            project.add_constraint_if_independent(si, Constraint::Parallel { a: la, b: lb, c: a, d: b });
        }
    }
    // 2c) TANGENT to the nearest circle or arc that is almost tangent already
    if let Some((cen, r)) = qymcad_pick::nearest_tangent_circle(project, si, p1, p2) {
        project.add_constraint_if_independent(si, Constraint::Tangent { a, b, c: cen, r });
    }
    // 2d) EQUAL LENGTH to the nearest line of the same length
    if let Some((la, lb)) = qymcad_pick::nearest_equal_line(project, si, p1, p2, a, b) {
        project.add_constraint_if_independent(si, Constraint::Equal { a: la, b: lb, c: a, d: b });
    }
    // 3) point on an edge: the new end landed on an existing line (not its own), so it is tied to it
    if let Some((la, lb)) = qymcad_ui_state::line_under_point(project, &view, si, p2, a, b) {
        project.add_constraint_if_independent(si, Constraint::PointOnLine { p: b, a: la, b: lb });
    }
    // (a coincidence with a vertex happens by itself: `sketch_point_at(1e-6)` already shares the point)
    project.solve_sketch(si);
}

/// The pop-up entry of sizes right after a rectangle or a polygon is built.
/// THE POPUP OF THE CORNER TOOL (a fillet radius or a chamfer leg): Enter applies, Esc cancels.
pub fn place_input_popup(ed: qymcad_ui_state::Editing, corner: &mut qymcad_ui_state::CornerInput, place: &mut qymcad_ui_state::Placing, sel_sk: &mut qymcad_ui_state::SketchSelection, tool_prefs: &mut qymcad_ui_state::SketchToolPrefs, ctx: &egui::Context, rect: Rect) {
    corner_input_popup(
        &mut qymcad_ui_state::CornerCtx {
            corner,
            project: &mut *ed.project,
            sel_sk,
            regen: &mut *ed.regen,
            status: ed.status,
            tool_prefs,
        },
        ctx,
        rect,
    );
    let qymcad_ui_state::Sel::Sketch(si) = *ed.sel else {
        place.clear(); // everything unfinished in the drawing at once (otherwise one of the three is forgotten)
        return;
    };
    let pl = &mut qymcad_ui_state::PlaceCtx { place, project: ed.project, view: &*ed.view, regen: ed.regen };
    ellipse_input_popup(pl, ctx, rect, si);
    rect_input_popup(pl, ctx, rect, si);
    poly_input_popup(pl, ctx, rect, si);
}

/// FINISH THE SPLINE - what a double click does.
///
/// Split out of the event handling for the same reason as everything else: a test must finish the shape
/// through THE SAME code rather than a copy of its own. Esc cancels a spline - that is a different
/// action, and substituting it for finishing would mean checking the wrong thing.
pub fn finish_spline(project: &mut Project, regen: &mut qymcad_ui_state::Rebuilding, sel: qymcad_ui_state::Sel, tool: &mut qymcad_ui_state::SketchTool, view: &mut qymcad_ui_state::View2d) {
    if let qymcad_ui_state::Sel::Sketch(si) = sel {
        if tool.pts.len() >= 2 {
            let pts = std::mem::take(&mut tool.pts);
            project.add_spline(si, pts, qymcad_core::feature::Ends::Open, qymcad_core::feature::Purpose::of(tool.construction));
            qymcad_ui_state::invalidate(regen);
            view.initialized = false;
        }
    }
    tool.pts.clear();
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
pub fn begin_point_drag(drag: &mut qymcad_ui_state::Dragging, project: &Project, set: &qymcad_ui_state::Settings, sheet: qymcad_ui_state::Sheet, si: usize, pp: Pos2, skip: &std::collections::HashSet<Id>) -> bool {
    let qymcad_ui_state::Sheet { view, rect } = sheet;
    let driven = project.sketches[si].projected_points();
    let mut best: Option<(f32, usize)> = None;
    for pi in 0..project.sketches[si].points.len() {
        let p = project.sketches[si].points[pi];
        if skip.contains(&p.id) || driven.contains(&p.id) {
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

/// RESOLVE A CONFLICT IN ONE CLICK: the conflicting dimension becomes A DRIVEN one - it stops driving the
/// geometry but stays on the drawing and shows the actual value. That is the standard way out in a
/// professional CAD; without it the only option left is to delete the dimension and lose it from the
/// drawing.
///
/// It lives here rather than in the body of the panel: it is an operation on the document, and a test
/// drives it.
pub fn make_dim_driven(project: &mut Project, regen: &mut qymcad_ui_state::Rebuilding, status: &mut String, si: usize, ci: usize) -> bool {
    if project.auto_driven(si, ci) {
        project.solve_sketch(si);
        qymcad_ui_state::invalidate(regen);
        *status = qymcad_i18n::tr("sk-dim-now-driven");
        true
    } else {
        *status = qymcad_i18n::tr("sk-cannot-be-driven");
        false
    }
}

/// The ANGLE popup for the rotate tool (`move_op == 3`), placed at the picked centre. Enter or the tick
/// applies `rotate_entities`; editing the value gives a live preview (the ghost in `draw_move_preview`).
pub fn sketch_rotate_popup(ed: qymcad_ui_state::Editing, armed: &mut qymcad_ui_state::Armed, rot: &mut qymcad_ui_state::RotInput, sel_sk: &qymcad_ui_state::SketchSelection, tool: &mut qymcad_ui_state::SketchTool, ctx: &egui::Context, rect: Rect) {
    if armed.move_op() != 3 {
        return;
    }
    let qymcad_ui_state::Sel::Sketch(si) = *ed.sel else { return };
    let Some(base) = tool.move_base else { return };
    let eids: Vec<Id> = sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
    if eids.is_empty() {
        return;
    }
    let at = (qymcad_ui_state::Sheet { view: *ed.view, rect: rect }).at(base);
    let want_focus = std::mem::take(&mut rot.focus);
    let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
    let (mut chg, mut apply, mut got_focus) = (false, false, false);
    let mut buf = std::mem::take(&mut rot.buf);
    egui::Area::new(egui::Id::new(("rotinput", si))).fixed_pos(qymcad_ui_state::clamp_popup(at, rect) + egui::vec2(12.0, -12.0)).order(egui::Order::Foreground).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("{} {}", ph::ARROWS_CLOCKWISE, qymcad_i18n::tr("sk-angle-deg")));
                let r = qymcad_ui_state::focus_edit(ui, &mut buf, 64.0, &qymcad_i18n::tr("sk-angle-placeholder"), want_focus);
                chg |= r.changed();
                got_focus |= r.has_focus();
                if r.lost_focus() && enter {
                    apply = true;
                }
                if ui.button(ph::CHECK).on_hover_text(qymcad_i18n::tr("sk-apply-enter")).clicked() {
                    apply = true;
                }
            });
        });
    });
    rot.buf = buf;
    if got_focus {
        rot.focus = false;
    }
    if chg || want_focus {
        rot.angle = parse_num(ed.project, &rot.buf.clone()).unwrap_or(0.0); // the live preview
    }
    if apply {
        if let Some(v) = parse_num(ed.project, &rot.buf.clone()) {
            ed.project.rotate_entities(si, &eids, base.x, base.y, v);
            *ed.status = qymcad_i18n::tr1("sk-rotated-by", "a", &v.to_string());
        }
        *armed = qymcad_ui_state::Armed::None;
        tool.move_base = None;
        qymcad_ui_state::invalidate(ed.regen);
    }
}

pub fn try_constraint_inner(project: &mut Project, regen: &mut qymcad_ui_state::Rebuilding, sel: qymcad_ui_state::Sel, sel_sk: &mut qymcad_ui_state::SketchSelection, status: &mut String, code: u8) -> bool {
    use qymcad_core::model::Constraint;
    let qymcad_ui_state::Sel::Sketch(si) = sel else { return false };
    let pts = qymcad_ui_state::sel_point_ids(sel_sk);
    let mut lines = qymcad_ui_state::sel_line_pts(project, sel_sk, si);
    // coordinate axes picked as lines (kind 3): their straight lines are materialised and used as lines
    let axes: Vec<u64> = sel_sk.items.iter().filter(|(k, _)| *k == 3).map(|(_, id)| *id).collect();
    for w in axes {
        let (o, d) = project.ensure_axis(si, w as usize);
        lines.push((o, d));
    }
    let mut new: Vec<Constraint> = Vec::new();
    match code {
        0 if pts.len() >= 2 => new.push(Constraint::Coincident { a: pts[0], b: pts[1] }),
        // one point coincident with an entity: a circle or an arc gives point-on-circle (which takes
        // priority), otherwise a line or an axis gives point-on-line
        0 if pts.len() == 1 => {
            let cs = qymcad_ui_state::sel_circle_centers(project, sel_sk, si);
            if !cs.is_empty() {
                new.push(Constraint::PointOnCircle { p: pts[0], c: cs[0] });
            } else if !lines.is_empty() {
                new.push(Constraint::PointOnLine { p: pts[0], a: lines[0].0, b: lines[0].1 });
            }
        }
        1 if pts.len() >= 2 => new.push(Constraint::Horizontal { a: pts[0], b: pts[1] }),
        1 if !lines.is_empty() => new.push(Constraint::Horizontal { a: lines[0].0, b: lines[0].1 }),
        2 if pts.len() >= 2 => new.push(Constraint::Vertical { a: pts[0], b: pts[1] }),
        2 if !lines.is_empty() => new.push(Constraint::Vertical { a: lines[0].0, b: lines[0].1 }),
        3 if lines.len() >= 2 => new.push(Constraint::Parallel { a: lines[0].0, b: lines[0].1, c: lines[1].0, d: lines[1].1 }),
        4 if lines.len() >= 2 => new.push(Constraint::Perpendicular { a: lines[0].0, b: lines[0].1, c: lines[1].0, d: lines[1].1 }),
        5 if lines.len() >= 2 => new.push(Constraint::Equal { a: lines[0].0, b: lines[0].1, c: lines[1].0, d: lines[1].1 }),
        5 => {
            // equal radii of two circles
            let cs = qymcad_ui_state::sel_circle_centers(project, sel_sk, si);
            if cs.len() >= 2 {
                new.push(Constraint::EqualRadius { c1: cs[0], c2: cs[1] });
            }
        }
        6 if !pts.is_empty() => new.extend(pts.iter().map(|p| Constraint::Fixed { p: *p })),
        7 if lines.len() >= 2 => new.push(Constraint::Collinear { a: lines[0].0, b: lines[0].1, c: lines[1].0, d: lines[1].1 }),
        8 => {
            // concentricity is A REAL kind with a glyph of its own, not a fake made of coincident centres
            let centers = qymcad_ui_state::sel_circle_centers(project, sel_sk, si);
            if centers.len() >= 2 {
                new.push(Constraint::Concentric { c1: centers[0], c2: centers[1] });
            }
        }
        9 => {
            // tangency: a line plus a circle
            if let (Some((la, lb)), Some((cc, rr))) = (lines.first().copied(), qymcad_ui_state::sel_circle_cr(project, sel_sk, si)) {
                new.push(Constraint::Tangent { a: la, b: lb, c: cc, r: rr });
            }
        }
        10 => {
            // symmetry: two points about an axis (the picked line)
            if pts.len() >= 2 {
                if let Some((la, lb)) = lines.first().copied() {
                    new.push(Constraint::Symmetric { a: pts[0], b: pts[1], la, lb });
                }
            }
        }
        11 if pts.len() == 1 && !lines.is_empty() => {
            // midpoint: the point at the middle of the picked line
            new.push(Constraint::Midpoint { p: pts[0], a: lines[0].0, b: lines[0].1 });
        }
        _ => {}
    }
    if new.is_empty() {
        return false;
    }
    project.sketches[si].constraints.extend(new);
    let resid = project.solve_sketch(si);
    sel_sk.clear(); // the selection and whatever was waiting for it
    qymcad_ui_state::invalidate(regen);
    *status = if resid < 1e-3 { qymcad_i18n::tr("sk-constraint-added") } else { qymcad_i18n::tr1("sk-constraint-added-resid", "r", &qymcad_i18n::num(resid, 2)) };
    true
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
        (qymcad_i18n::tr("dof-conflict"), scheme.pal.error())
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
    *sk.status = qymcad_i18n::tr("sk-select-hint");
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
    let sh = qymcad_ui_state::Sheet { view: *sk.view, rect: rect };
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

/// The index of the dimension (a distance or an angle) whose caption is nearest to a screen point.
pub fn dim_at(sk: &mut qymcad_ui_state::SketchCtx, rect: Rect, pos: Pos2, si: usize) -> Option<usize> {
    let sh = qymcad_ui_state::Sheet { view: *sk.view, rect: rect };
    use qymcad_core::model::Constraint;
    // THE CONSTRAINTS ARE COPIED OUT FIRST. The loop calls back into the context (`dim_label_pos`), and a
    // reference into the document held across that call borrows the same record twice.
    let cs = sk.project.sketches.get(si)?.constraints.clone();
    let mut best: Option<(f32, usize)> = None;
    for ci in 0..cs.len() {
        // the distance to the caption
        let mut d = dim_label_pos(sk, rect, si, ci).map(|p| p.distance(pos)).unwrap_or(f32::INFINITY);
        // ...and to the dimension line itself (a click on the line picks it too)
        if let Some(Constraint::Distance { a, b, off, .. }) = cs.get(ci).cloned() {
            if let (Some(pa), Some(pb)) = (qymcad_ui_state::sketch_pt(sk.project, si, a), qymcad_ui_state::sketch_pt(sk.project, si, b)) {
                let (sa, sb) = (sh.at(pa), sh.at(pb));
                let dir = (sb - sa).normalized();
                let perp = egui::vec2(-dir.y, dir.x);
                let o = perp * (16.0 + off as f32 * sk.view.scale);
                d = d.min(qymcad_ui_state::screen_dist_seg(pos, sa + o, sb + o));
            }
        }
        if d <= qymcad_ui_state::grab::grab(sk.set, Grab::Label) && best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, ci));
        }
    }
    best.map(|(_, ci)| ci)
}

/// The screen position of a dimension caption (for editing in place and for hit testing).
pub fn dim_label_pos(sk: &mut qymcad_ui_state::SketchCtx, rect: Rect, si: usize, ci: usize) -> Option<Pos2> {
    let sh = qymcad_ui_state::Sheet { view: *sk.view, rect: rect };
    use qymcad_core::model::Constraint;
    let s = sk.project.sketches.get(si)?;
    // the caption offset `off` is stored in WORLD units and converted to pixels through the scale
    // (otherwise the caption does not scale with the geometry on zoom and drifts off screen). Angles (the
    // diameter) are left alone.
    let sc = sk.view.scale;
    match *s.constraints.get(ci)? {
        Constraint::Distance { a, b, off, axis, .. } => {
            let (pa, pb) = (qymcad_ui_state::sketch_pt(sk.project, si, a)?, qymcad_ui_state::sketch_pt(sk.project, si, b)?);
            let (sa, sb) = (sh.at(pa), sh.at(pb));
            let (la, lb, perp) = match axis {
                1 => {
                    let y = (sa.y + sb.y) / 2.0 + off as f32 * sc;
                    (Pos2::new(sa.x, y), Pos2::new(sb.x, y), egui::vec2(0.0, 1.0))
                }
                2 => {
                    let x = (sa.x + sb.x) / 2.0 + off as f32 * sc;
                    (Pos2::new(x, sa.y), Pos2::new(x, sb.y), egui::vec2(1.0, 0.0))
                }
                _ => {
                    let dir = (sb - sa).normalized();
                    let perp = egui::vec2(-dir.y, dir.x);
                    let o = 16.0 + off as f32 * sc;
                    (sa + perp * o, sb + perp * o, perp)
                }
            };
            Some(((la.to_vec2() + lb.to_vec2()) / 2.0 + perp * 8.0).to_pos2())
        }
        Constraint::DistancePL { p, a, b, off, .. } => {
            let (pp, pa, _pb) = (qymcad_ui_state::sketch_pt(sk.project, si, p)?, qymcad_ui_state::sketch_pt(sk.project, si, a)?, qymcad_ui_state::sketch_pt(sk.project, si, b)?);
            let (sp, sa) = (sh.at(pp), sh.at(pa));
            let ab = qymcad_ui_state::line_screen_dir(sk.project, sk.view, si, a, b, rect)?;
            let foot = sa + ab * (sp - sa).dot(ab);
            let perp = (sp - foot).normalized();
            let o = off as f32 * sc;
            let (lp, lf) = (sp + ab * o, foot + ab * o); // the leader runs along the line (see the drawing and the hit test)
            Some(((lp.to_vec2() + lf.to_vec2()) / 2.0 + perp * 8.0).to_pos2())
        }
        Constraint::EdgeDistance { c1, c2, m1, m2, off, .. } => {
            let (p1, p2) = (qymcad_ui_state::sketch_pt(sk.project, si, c1)?, qymcad_ui_state::sketch_pt(sk.project, si, c2)?);
            let r_of = |cid: Id| -> f64 {
                s.entities.iter().find_map(|e| match e.kind {
                    qymcad_core::model::EntityKind::Circle { center, r } if center == cid => Some(r),
                    qymcad_core::model::EntityKind::Arc { center, a, .. } if center == cid => qymcad_ui_state::sketch_pt(sk.project, si, a).zip(qymcad_ui_state::sketch_pt(sk.project, si, center)).map(|(pa, pc)| ((pa.x - pc.x).powi(2) + (pa.y - pc.y).powi(2)).sqrt()),
                    _ => None,
                }).unwrap_or(0.0)
            };
            let (r1, r2) = (r_of(c1), r_of(c2));
            let len = ((p2.x - p1.x).powi(2) + (p2.y - p1.y).powi(2)).sqrt().max(1e-9);
            let (ux, uy) = ((p2.x - p1.x) / len, (p2.y - p1.y) / len);
            let e1 = Point2::new(p1.x - m1 as f64 * r1 * ux, p1.y - m1 as f64 * r1 * uy);
            let e2 = Point2::new(p2.x + m2 as f64 * r2 * ux, p2.y + m2 as f64 * r2 * uy);
            let (sa, sb) = (sh.at(e1), sh.at(e2));
            let dir = (sb - sa).normalized();
            let perp = egui::vec2(-dir.y, dir.x);
            let o = 16.0 + off as f32 * sc;
            Some((((sa + perp * o).to_vec2() + (sb + perp * o).to_vec2()) / 2.0 + perp * 8.0).to_pos2())
        }
        Constraint::Angle { a, b, c, .. } => {
            let (pa, pb, pc) = (qymcad_ui_state::sketch_pt(sk.project, si, a)?, qymcad_ui_state::sketch_pt(sk.project, si, b)?, qymcad_ui_state::sketch_pt(sk.project, si, c)?);
            let (sa, sb, sc) = (sh.at(pa), sh.at(pb), sh.at(pc));
            let bis = ((sa - sb).normalized() + (sc - sb).normalized()).normalized();
            Some(sb + bis * 40.0)
        }
        Constraint::Diameter { c, off, .. } => {
            let cp = qymcad_ui_state::sketch_pt(sk.project, si, c)?;
            let r = qymcad_ui_state::center_radius(&qymcad_ui_state::DrawCtx { cam: sk.cam, set: sk.set, scheme: sk.scheme, project: sk.project, active_path: sk.active_path }, si, c)?; // a circle OR an arc
            let sc = sh.at(cp);
            // the diameter or radius label is placed AROUND THE CIRCLE - `off` holds the angle of the
            // leader in radians, in screen coordinates. By default (off = 0) it goes horizontally to the
            // right, but it can be dragged to any angle about the centre.
            let r_px = (sh.at(Point2::new(cp.x + r, cp.y)) - sc).length();
            let ang = off as f32;
            Some(sc + egui::vec2(ang.cos(), ang.sin()) * (r_px + 14.0))
        }
        Constraint::AngleLines { a, b, c, d, .. } => {
            let (pa, pb, pc, pd) = (qymcad_ui_state::sketch_pt(sk.project, si, a)?, qymcad_ui_state::sketch_pt(sk.project, si, b)?, qymcad_ui_state::sketch_pt(sk.project, si, c)?, qymcad_ui_state::sketch_pt(sk.project, si, d)?);
            let (sa, sb, sc, sd) = (sh.at(pa), sh.at(pb), sh.at(pc), sh.at(pd));
            let ix = qymcad_ui_state::lines_intersect(sa, sb, sc, sd).unwrap_or(((sb.to_vec2() + sd.to_vec2()) / 2.0).to_pos2());
            let bis = ((sb - ix).normalized() + (sd - ix).normalized()).normalized();
            Some(ix + bis * 40.0)
        }
        // an arc length: the caption sits at the middle of the arc, matching how it is drawn - otherwise
        // `dim_at` does not find it, and the dimension can be neither picked, nor edited, nor dragged.
        Constraint::ArcLength { c, a, b, off, .. } => {
            let (cp, pa, pb) = (qymcad_ui_state::sketch_pt(sk.project, si, c)?, qymcad_ui_state::sketch_pt(sk.project, si, a)?, qymcad_ui_state::sketch_pt(sk.project, si, b)?);
            let r = ((pa.x - cp.x).powi(2) + (pa.y - cp.y).powi(2)).sqrt();
            let mid = Point2::new((pa.x + pb.x) / 2.0 - cp.x, (pa.y + pb.y) / 2.0 - cp.y);
            let ml = (mid.x * mid.x + mid.y * mid.y).sqrt().max(1e-9);
            let ed = sh.at(Point2::new(cp.x + mid.x / ml * r, cp.y + mid.y / ml * r));
            Some((ed.to_vec2() + egui::vec2(10.0, -8.0 + off as f32 * sc)).to_pos2())
        }
        _ => None,
    }
}

/// Apply a geometric constraint to the sketch selection. Returns true when it was applied.
/// `code`: 0 coincident, 1 horizontal, 2 vertical, 3 parallel, 4 perpendicular, 5 equal, 6 fixed,
/// 7 collinear, 8 concentric.
pub fn try_constraint(sk: &mut qymcad_ui_state::SketchCtx, code: u8) -> bool {
    // THE BOUNDARY OF AN OPERATION: placing a constraint is a deliberate act and makes one undo step.
    qymcad_ui_state::begin_edit(sk.edits, sk.project, qymcad_i18n::tr("sk-constraint"));
    let ok = try_constraint_inner(sk.project, sk.regen, *sk.sel, sk.sel_sk, sk.status, code);
    if ok {
        qymcad_ui_state::commit_edit(&mut sk.rebuild());
    } else {
        qymcad_ui_state::abort_edit(&mut sk.rebuild()); // the constraint did not take, so it leaves no trace
    }
    ok
}

/// The constraint button: if the selection is enough, apply it; otherwise go into waiting mode (the
/// button lights up, the elements are picked, Esc cancels).
pub fn constraint_button(sk: &mut qymcad_ui_state::SketchCtx, code: u8) {
    qymcad_ui_state::exit_draw_tools(&mut qymcad_ui_state::tools_in!(sk));
    *sk.armed = qymcad_ui_state::Armed::None;
    sk.sel_sk.constraint = None;
    sk.sel_sk.modify = None;
    if try_constraint(sk, code) {
        sk.sel_sk.constraint = None;
    } else {
        // it could not be applied to the current selection, so the picking starts AFRESH, without any
        // elements left stuck: otherwise they would go into the constraint as `pts[0]` or `lines[0]` and
        // tie the wrong things together.
        sk.sel_sk.clear(); // the selection and whatever was waiting for it
        sk.sel_sk.constraint = Some(code);
        *sk.status = qymcad_i18n::tr("sk-pick-for-constraint");
    }
}

/// While the dimension follows the cursor, its offset (`off`) is updated to match.
pub fn update_placing_dim(sk: &mut qymcad_ui_state::SketchCtx, rect: Rect) {
    let sh = qymcad_ui_state::Sheet { view: *sk.view, rect: rect };
    use qymcad_core::model::Constraint;
    let Some(ci) = sk.place.dim else { return };
    let qymcad_ui_state::Sel::Sketch(si) = *sk.sel else {
        sk.place.dim = None;
        return;
    };
    let Some(cur) = *sk.cursor else { return };
    let sc = sh.at(cur);
    match sk.project.sketches[si].constraints.get(ci).cloned() {
        Some(Constraint::Distance { a, b, .. }) => {
            if let (Some(pa), Some(pb)) = (qymcad_ui_state::sketch_pt(sk.project, si, a), qymcad_ui_state::sketch_pt(sk.project, si, b)) {
                let (sa, sb) = (sh.at(pa), sh.at(pb));
                let mid = ((sa.to_vec2() + sb.to_vec2()) / 2.0).to_pos2();
                // THE ORIENTATION follows the cursor: to the side gives a vertical dimension (dy), above or
                // below a horizontal one (dx), anything else an aligned one.
                let (cx, cy) = (sc.x - mid.x, sc.y - mid.y);
                let new_axis = if cx.abs() > cy.abs() * 1.7 { 2u8 } else if cy.abs() > cx.abs() * 1.7 { 1u8 } else { 0u8 };
                // the offset of the line: along Y for a horizontal dimension, along X for a vertical one,
                // along the perpendicular for an aligned one.
                // The offset is in WORLD units: the screen shift divided by the scale (for an aligned one
                // the base gap of 16 px is subtracted)
                let vscale = sk.view.scale as f64;
                let off = match new_axis {
                    1 => (sc.y - mid.y) as f64 / vscale,
                    2 => (sc.x - mid.x) as f64 / vscale,
                    _ => {
                        let dir = (sb - sa).normalized();
                        let perp = egui::vec2(-dir.y, dir.x);
                        (((sc - sa).x * perp.x + (sc - sa).y * perp.y) as f64 - 16.0) / vscale
                    }
                };
                // the measured value for the chosen axis, in world coordinates
                let measured = match new_axis {
                    1 => (pa.x - pb.x).abs(),
                    2 => (pa.y - pb.y).abs(),
                    _ => ((pa.x - pb.x).powi(2) + (pa.y - pb.y).powi(2)).sqrt(),
                };
                if let Some(Constraint::Distance { off: o, axis, d, .. }) = sk.project.sketches[si].constraints.get_mut(ci) {
                    *o = off;
                    *axis = new_axis;
                    *d = measured; // while it flies it measures; the value is typed when it is placed
                }
            }
        }
        Some(Constraint::DistancePL { p, a, b, .. }) => {
            if let (Some(pp), Some(pa), Some(ab)) = (qymcad_ui_state::sketch_pt(sk.project, si, p), qymcad_ui_state::sketch_pt(sk.project, si, a), qymcad_ui_state::line_screen_dir(sk.project, sk.view, si, a, b, rect)) {
                let (sp, sa) = (sh.at(pp), sh.at(pa));
                let foot = sa + ab * (sp - sa).dot(ab);
                // the leader runs ALONG the line (ab), exactly as in the drawing, the hit test and the
                // caption. The label sticks to the cursor along the direction of the line: the cursor goes
                // up and the label goes up, not sideways.
                let off = (sc - foot).dot(ab) as f64 / sk.view.scale as f64; // WORLD units, through the scale
                if let Some(Constraint::DistancePL { off: o, .. }) = sk.project.sketches[si].constraints.get_mut(ci) {
                    *o = off;
                }
            }
        }
        _ => {
            sk.place.dim = None;
        }
    }
}

/// Project into the sketch whatever was clicked: an edge of the underlay, or - in "face contour" mode -
/// the whole contour of the face the sketch stands on.
pub fn project_clicked_edge(sk: &mut qymcad_ui_state::SketchCtx, si: usize, rect: Rect, pos: Pos2) {
    use qymcad_core::feature::SketchPlane;
    use qymcad_core::model::ProjSource;
    // "FACE CONTOUR" MODE works only for a sketch seated ON A FACE: a sketch on a world plane has no host
    // face, and projecting its contour would mean guessing.
    let face_mode = sk.tool.proj_face;
    let src = match (face_mode, sk.project.sketches.get(si).map(|s| s.plane)) {
        (true, Some(SketchPlane::Face(b, key))) if key.id != 0 => Some((sk.project.live_body(b), ProjSource::Face(key.id))),
        (true, _) => {
            *sk.status = qymcad_i18n::tr("sk-face-outline-only");
            return;
        }
        _ => nearest_ref_edge(sk, si, rect, pos).map(|(b, e)| (b, ProjSource::Edge(e))),
    };
    let Some((body, src)) = src else {
        *sk.status = qymcad_i18n::tr("sk-miss-click-edge");
        return;
    };
    qymcad_ui_state::begin_edit(sk.edits, sk.project, qymcad_i18n::tr("sk-project")); // THE BOUNDARY OF AN OPERATION, as with other sketch edits
    let id = qymcad_ui_state::with_kernel(&mut sk.rebuild(), |project, k| project.add_sketch_projection(si, body, src, k));
    *sk.status = if id == 0 {
        qymcad_i18n::tr("sk-not-projectable")
    } else {
        qymcad_i18n::tr("sk-projected-hint")
    };
    sk.project.solve_sketch(si);
    qymcad_ui_state::commit_edit(&mut sk.rebuild());
    qymcad_ui_state::invalidate(sk.regen);
}

/// A click with the dimension tool. Returns true when the click was handled.
pub fn dim_click(sk: &mut qymcad_ui_state::SketchCtx, rect: Rect, pos: Pos2) -> bool {
    // THE BOUNDARY OF AN OPERATION: placing a dimension is a deliberate act and makes one undo step.
    qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, qymcad_i18n::tr("sk-dim"));
    let r = dim_click_inner(sk, rect, pos);
    qymcad_ui_state::commit_edit(&mut sk.rebuild());
    r
}

pub fn dim_click_inner(sk: &mut qymcad_ui_state::SketchCtx, rect: Rect, pos: Pos2) -> bool {
    use qymcad_core::model::Constraint;
    let qymcad_ui_state::Sel::Sketch(si) = *sk.sel else {
        *sk.status = qymcad_i18n::tr("sk-pick-sketch-first");
        return true;
    };
    // the dimension follows the cursor: a click places it, OR (when it is a provisional length of a line
    // and a SECOND element was hit) switches it to a distance between the two.
    if let Some(ci) = sk.place.dim {
        if let Some(qymcad_ui_state::DimRef::Line(la, lb)) = sk.dim.first {
            // It switches to a distance ONLY on a click exactly on A VERTEX (not an end of this line) or
            // on AN AXIS. A click on the body of another line does NOT switch it - otherwise placing the
            // dimension line near the edge of a shape (a square, say) broke.
            let r2 = qymcad_pick::nearest_sketch_point(&sk.pick(), rect, pos, si).filter(|p| *p != la && *p != lb).map(qymcad_ui_state::DimRef::Point).or_else(|| {
                let o = (qymcad_ui_state::Sheet { view: *sk.view, rect: rect }).at(Point2::new(0.0, 0.0));
                if (pos.y - o.y).abs() <= qymcad_ui_state::grab::grab(&*sk.set, Grab::Guide) {
                    let (a, b) = sk.project.ensure_axis(si, 0);
                    Some(qymcad_ui_state::DimRef::Line(a, b))
                } else if (pos.x - o.x).abs() <= qymcad_ui_state::grab::grab(&*sk.set, Grab::Guide) {
                    let (a, b) = sk.project.ensure_axis(si, 1);
                    Some(qymcad_ui_state::DimRef::Line(a, b))
                } else {
                    None
                }
            }).or_else(|| {
                // a click on ANOTHER roughly parallel line gives the distance between the two lines (the
                // body of a line used to be ignored, and the gap between two parallels could not be
                // measured). Parallel ones only - a perpendicular edge of a square next to the leader does
                // NOT switch it, and a length is placed instead.
                qymcad_pick::nearest_line_entity(&sk.pick(), rect, pos, si)
                    .filter(|&(a2, b2)| (a2, b2) != (la, lb) && (a2, b2) != (lb, la))
                    .filter(|&(a2, b2)| qymcad_ui_state::lines_parallel(&sk.draw(), si, la, lb, a2, b2))
                    .map(|(a2, b2)| qymcad_ui_state::DimRef::Line(a2, b2))
            });
            if let Some(r2) = r2 {
                sk.project.sketches[si].constraints.remove(ci); // remove the provisional length
                sk.place.dim = None;
                sk.dim.first = None;
                make_between_dim(sk, si, qymcad_ui_state::DimRef::Line(la, lb), r2);
                return true;
            }
        }
        sk.place.dim = None;
        sk.dim.first = None;
        *sk.inline = qymcad_ui_state::InlineEdit::Dim(ci); // place it and go straight into typing the value
        sk.dim.focus = true;
        *sk.status = qymcad_i18n::tr("sk-dim-placed");
        return true;
    }
    // a radius or a diameter: a circle entity (a diameter or radius dimension) or a circle primitive
    if sk.armed.dim_kind() == 3 {
        if let Some(eid) = qymcad_pick::nearest_circle_entity(&sk.pick(), rect, pos, si) {
            // a circle gets a parametric diameter dimension (a Diameter constraint); an arc has its radius
            // edited
            let center = sk.project.sketches[si].entities.iter().find(|e| e.id == eid).and_then(|e| match e.kind {
                qymcad_core::model::EntityKind::Circle { center, .. } => Some(center),
                _ => None,
            });
            if let Some(c) = center {
                if let Some(ci) = sk.project.ensure_diameter(si, c, true) {
                    *sk.inline = qymcad_ui_state::InlineEdit::Dim(ci);
                    sk.dim.focus = true;
                    *sk.status = qymcad_i18n::tr("sk-enter-diameter");
                }
            } else {
                *sk.inline = qymcad_ui_state::InlineEdit::Circle(eid); // an arc or a fillet
                sk.dim.focus = true;
                *sk.status = qymcad_i18n::tr("sk-enter-arc-radius");
            }
            return true;
        }
        // the circumscribed circle of a polygon has its radius edited, through its centre handle
        if let Some(cid) = qymcad_ui_state::polygon_under(&*sk.project, &*sk.view, rect, pos, si) {
            sk.place.set(qymcad_ui_state::PlacingShape::Poly(cid));
            sk.place.focus = true;
            *sk.status = qymcad_i18n::tr("sk-enter-polygon-radius");
            return true;
        }
        *sk.status = qymcad_i18n::tr("sk-click-nearer-circle");
        return true;
    }
    // A LINEAR dimension takes TWO references (a point, a line or an axis). The first line gives a length
    // that follows the cursor, but a click on a second element switches it to a distance between them
    // (point to line, line to a parallel line, or to an axis). A single line and nothing else is a
    // length.
    if sk.armed.dim_kind() == 1 {
        let Some(r) = resolve_dim_ref(sk, rect, pos, si) else {
            *sk.status = if sk.dim.first.is_some() { qymcad_i18n::tr("sk-need-second") } else { qymcad_i18n::tr("sk-dim-pick-hint") };
            return true;
        };
        match sk.dim.first.take() {
            None => match r {
                // a coordinate axis as the first reference creates NO provisional length (an axis has no
                // length; this used to breed a service dimension of 1.0 on the axes). It simply waits for
                // the second element.
                qymcad_ui_state::DimRef::Line(a, b) if sk.project.sketches[si].axis_pts.contains(&a) || sk.project.sketches[si].axis_pts.contains(&b) => {
                    sk.dim.first = Some(r);
                    *sk.status = qymcad_i18n::tr("sk-axis-picked");
                }
                qymcad_ui_state::DimRef::Line(a, b) => {
                    // the provisional LENGTH of the line, following the cursor; a second click may switch it
                    // to a distance
                    if let (Some(pa), Some(pb)) = (qymcad_ui_state::sketch_pt(&*sk.project, si, a), qymcad_ui_state::sketch_pt(&*sk.project, si, b)) {
                        let d = ((pa.x - pb.x).powi(2) + (pa.y - pb.y).powi(2)).sqrt();
                        sk.project.sketches[si].constraints.push(Constraint::Distance { a, b, d, off: 0.0, expr: String::new(), driven: false, axis: 0 });
                        let ci = sk.project.sketches[si].constraints.len() - 1;
                        let (_, conflict) = qymcad_ui_state::finish_dim(&mut *sk.project, &mut *sk.regen, si, ci);
                        sk.place.dim = Some(ci);
                        sk.dim.first = Some(r);
                        *sk.status = if conflict {
                            format!("{} {}", ph::WARNING, qymcad_i18n::tr("sk-length-conflict"))
                        } else {
                            qymcad_i18n::tr("sk-length-hint")
                        };
                    }
                }
                qymcad_ui_state::DimRef::Point(_) => {
                    sk.dim.first = Some(r);
                    *sk.status = qymcad_i18n::tr("sk-point-picked");
                }
            },
            Some(r1) => make_between_dim(sk, si, r1, r),
        }
        return true;
    }
    // AN ANGULAR dimension takes 2 LINES (the angle between them, placed at their real or implied
    // intersection) OR 3 points (A, the vertex, then C). A LINE under the cursor takes priority: click two
    // lines and an angle appears.
    if let Some(lr) = resolve_dim_ref(sk, rect, pos, si).filter(|r| matches!(r, qymcad_ui_state::DimRef::Line(..))) {
        match sk.dim.first.take() {
            Some(prev @ qymcad_ui_state::DimRef::Line(a1, b1)) => {
                if matches!(lr, qymcad_ui_state::DimRef::Line(la, lb) if (la, lb) != (a1, b1)) {
                    sk.dim.pick.clear();
                    make_between_dim(sk, si, prev, lr); // two non-parallel lines give an AngleLines
                } else {
                    sk.dim.first = Some(prev); // the same line - wait for ANOTHER one
                }
            }
            _ => {
                sk.dim.first = Some(lr);
                sk.dim.pick.clear();
                *sk.status = qymcad_i18n::tr("sk-line1-picked");
            }
        }
        return true;
    }
    // otherwise it is the three-point mode (A, the vertex B, then C)
    let id = if let Some(r) = resolve_sketch_ref(sk, rect, pos, si) {
        qymcad_ui_state::materialize_ref(&mut *sk.project, si, r)
    } else {
        *sk.status = qymcad_i18n::tr("sk-angle-hint");
        return true;
    };
    if !sk.dim.pick.contains(&id) {
        sk.dim.pick.push(id);
    }
    if sk.dim.pick.len() >= 3 {
        let pick = std::mem::take(&mut sk.dim.pick);
        let (a, b, c) = (pick[0], pick[1], pick[2]);
        if let (Some(pa), Some(pb), Some(pc)) = (qymcad_ui_state::sketch_pt(&*sk.project, si, a), qymcad_ui_state::sketch_pt(&*sk.project, si, b), qymcad_ui_state::sketch_pt(&*sk.project, si, c)) {
            let (ux, uy) = (pa.x - pb.x, pa.y - pb.y);
            let (vx, vy) = (pc.x - pb.x, pc.y - pb.y);
            let deg = (ux * vy - uy * vx).atan2(ux * vx + uy * vy).abs().to_degrees();
            sk.project.sketches[si].constraints.push(Constraint::Angle { a, b, c, deg, expr: String::new(), driven: false });
            let ci = sk.project.sketches[si].constraints.len() - 1;
            let (redundant, conflict) = qymcad_ui_state::finish_dim(&mut *sk.project, &mut *sk.regen, si, ci);
            *sk.status = if conflict {
                format!("{} {}", ph::WARNING, qymcad_i18n::tr("sk-angle-conflict"))
            } else if redundant {
                qymcad_i18n::tr("sk-driven-angle")
            } else {
                qymcad_i18n::tr1("sk-angle-added", "a", &qymcad_i18n::num(deg, 1))
            };
        }
    }
    true
}

/// A click in the sketch workbench: pick the nearest point or entity (Shift adds to the selection).
pub fn sketch_select_click(sk: &mut qymcad_ui_state::SketchCtx, rect: Rect, pos: Pos2, additive: bool) {
    let sh = qymcad_ui_state::Sheet { view: *sk.view, rect: rect };
    let qymcad_ui_state::Sel::Sketch(si) = *sk.sel else { return };
    // a click on a text object picks it (Del removes it, a double click edits the string or the height,
    // dragging moves it)
    if let Some(ti) = qymcad_ui_state::text_at(&*sk.project, &*sk.view, rect, pos, si) {
        sk.annot.text = Some(ti);
        sk.annot.note = None;
        sk.gsel.constraint = None;
        sk.sel_sk.clear(); // the selection and whatever was waiting for it
        *sk.status = qymcad_i18n::tr("sk-text-selected");
        return;
    }
    sk.annot.text = None;
    // a click on a note picks it (Del removes it, a double click edits it, dragging moves it)
    if let Some(ni) = qymcad_ui_state::note_at(&*sk.project, &*sk.view, rect, pos, si) {
        sk.annot.note = Some(ni);
        sk.gsel.constraint = None;
        sk.sel_sk.clear(); // the selection and whatever was waiting for it
        *sk.status = qymcad_i18n::tr("sk-note-selected");
        return;
    }
    sk.annot.note = None;
    // a click on a constraint glyph or a dimension caption PICKS it (Del removes it) rather than
    // deleting it at once
    if let Some(ci) = constraint_glyph_at(sk, rect, pos, si).or_else(|| dim_at(sk, rect, pos, si)) {
        sk.gsel.constraint = Some(ci);
        sk.sel_sk.clear(); // the selection and whatever was waiting for it
        *sk.status = qymcad_i18n::tr("sk-constraint-selected");
        return;
    }
    sk.gsel.constraint = None;
    // while a constraint or modify tool is active, the selection accumulates WITHOUT Shift: elements are
    // clicked one at a time, and the constraint applies as soon as there are enough of them.
    let additive = additive || sk.sel_sk.constraint.is_some() || sk.sel_sk.modify.is_some();
    let mut hit = sketch_hit(&sk.pick(), rect, pos, si);
    // a click on the origin materialises the reference point and picks it (for constraints and dimensions)
    if hit.is_none() && sh.at(Point2::new(0.0, 0.0)).distance(pos) <= qymcad_ui_state::grab::grab(sk.set, Grab::Point) {
        let o = sk.project.ensure_origin(si);
        hit = Some((0u8, o));
    }
    // a click on the diameter or radius caption of a circle or an arc (outside the contour) picks it as an
    // entity
    if hit.is_none() {
        if let Some(eid) = qymcad_pick::nearest_circle_entity(&sk.pick(), rect, pos, si) {
            hit = Some((1u8, eid));
        }
    }
    // a click on the X or Y axis (when no other geometry is near) picks the axis as a line (kind 3, id 0
    // or 1)
    if hit.is_none() {
        let o = sh.at(Point2::new(0.0, 0.0));
        let near_x = (pos.y - o.y).abs() <= qymcad_ui_state::grab::grab(sk.set, Grab::Guide); // the horizontal X axis (y = 0)
        let near_y = (pos.x - o.x).abs() <= qymcad_ui_state::grab::grab(sk.set, Grab::Guide); // the vertical Y axis (x = 0)
        if near_x && (!near_y || (pos.y - o.y).abs() <= (pos.x - o.x).abs()) {
            hit = Some((3u8, 0));
        } else if near_y {
            hit = Some((3u8, 1));
        }
    }
    match hit {
        Some(refr) => {
            if !additive {
                sk.sel_sk.clear(); // the selection and whatever was waiting for it
            }
            if let Some(p) = sk.sel_sk.items.iter().position(|r| *r == refr) {
                sk.sel_sk.items.remove(p); // a second click deselects it
            } else {
                sk.sel_sk.items.push(refr);
            }
        }
        None => {
            if !additive {
                sk.sel_sk.clear(); // the selection and whatever was waiting for it
            }
        }
    }
    // a deferred constraint or operation: apply it as soon as the selection is enough
    if let Some(code) = sk.sel_sk.constraint {
        if try_constraint(sk, code) {
            sk.sel_sk.constraint = None;
        }
    }
    if let Some(op) = sk.sel_sk.modify {
        if qymcad_ui_state::try_modify(qymcad_ui_state::editing_in!(sk), &mut *sk.sel_sk, *sk.sk_pat, &*sk.tool_prefs, op) {
            sk.sel_sk.modify = None;
        }
    }
}

/// The in-place editors of dimension values in the viewport (a double click, or the radius tool).
pub fn dim_editor(sk: &mut qymcad_ui_state::SketchCtx, ctx: &egui::Context, rect: Rect) {
    use qymcad_core::model::{Constraint, EntityKind};
    let qymcad_ui_state::Sel::Sketch(si) = *sk.sel else {
        sk.inline.clear();
        sk.inline.clear();
        return;
    };
    // the radius of a circle or an arc entity
    if let Some(eid) = sk.inline.circle() {
        let info = sk.project.sketches.get(si).and_then(|s| s.entities.iter().find(|e| e.id == eid)).and_then(|e| match e.kind {
            EntityKind::Circle { center, r } => Some((center, r, false)),
            EntityKind::Arc { center, a, .. } => qymcad_ui_state::sketch_pt(&*sk.project, si, a).zip(qymcad_ui_state::sketch_pt(&*sk.project, si, center)).map(|(pa, c)| (center, ((pa.x - c.x).powi(2) + (pa.y - c.y).powi(2)).sqrt(), true)),
            _ => None,
        });
        if let (Some((_center, r, is_arc)), Some(cp)) = (info, info.and_then(|(c, ..)| qymcad_ui_state::sketch_pt(&*sk.project, si, c))) {
            let at = (qymcad_ui_state::Sheet { view: *sk.view, rect: rect }).at(Point2::new(cp.x + r, cp.y));
            let mut rr = r;
            let (mut chg, mut close) = (false, false);
            let want_focus = sk.dim.focus;
            let mut got_focus = false;
            let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
            // the text buffer of the radius, with auto-focus and Enter, as with a linear dimension
            let mut buf = std::mem::take(&mut sk.dim.buf);
            if want_focus {
                // THROUGH THE COMMON DOOR: no longer than four digits, with no tail. A raw `format!("{}")`
                // printed the whole truth about an f64 - "12.750000000000002" in the radius field.
                buf = qymcad_core::expr::fmt_num(if is_arc { r } else { 2.0 * r }); // a radius for an arc, a diameter for a circle
            }
            let mut buf_changed = false;
            egui::Area::new(egui::Id::new(("circedit", si, eid))).fixed_pos(qymcad_ui_state::clamp_popup(at, rect) + egui::vec2(8.0, -10.0)).order(egui::Order::Foreground).show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(if is_arc { qymcad_i18n::tr("sk-radius") } else { qymcad_i18n::tr("sk-diameter") });
                        let rsp = qymcad_ui_state::focus_edit(ui, &mut buf, 80.0, &qymcad_i18n::tr("sk-mm"), want_focus);
                        if rsp.changed() {
                            buf_changed = true;
                        }
                        got_focus = rsp.has_focus();
                        if rsp.lost_focus() && enter {
                            close = true;
                        }
                        if ui.button(ph::CHECK).clicked() {
                            close = true;
                        }
                    });
                });
            });
            if got_focus {
                sk.dim.focus = false;
            }
            sk.dim.buf = buf.clone();
            if buf_changed {
                // the radius or diameter of an arc: the value is evaluated when an expression is typed. It
                // is stored as a number (an arc has no parametric dimension constraint), just like any
                // value set by dragging - but "w/2" can be typed now.
                if let Some(v) = parse_num(&*sk.project, &buf) {
                    rr = (if is_arc { v } else { v * 0.5 }).max(0.01); // Ø -> r
                    chg = true;
                }
            }
            if chg {
                if is_arc {
                    // a fillet arc has its radius edited through the dimension constraint (parametrically);
                    // failing that, the fillet is recomputed geometrically; failing that, it is a plain arc
                    if !sk.project.set_fillet_radius_dim(si, eid, rr.max(0.01)) && !sk.project.set_fillet_radius(si, eid, rr.max(0.01)) {
                        sk.project.set_arc_radius(si, eid, rr.max(0.01));
                    }
                } else {
                    if let Some(e) = sk.project.sketches.get_mut(si).and_then(|s| s.entities.iter_mut().find(|e| e.id == eid)) {
                        if let EntityKind::Circle { r, .. } = &mut e.kind {
                            *r = rr.max(0.01);
                        }
                    }
                    sk.project.regen_sketch(si);
                }
                qymcad_ui_state::invalidate(&mut *sk.regen);
            }
            if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                sk.inline.clear();
                sk.dim.buf.clear();
                sk.dim.focus = false;
            }
        } else {
            sk.inline.clear();
        }
    }
    let Some(ci) = sk.inline.dim() else { return };
    let Some(at) = dim_label_pos(sk, rect, si, ci) else {
        sk.inline.clear();
        return;
    };
    let mut changed = false;
    let mut close = false;
    let want_focus = sk.dim.focus; // focus is requested until the field actually takes it
    let mut got_focus = false;
    let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
    // the current state of the dimension: the value, the expression, whether it is driven, whether it is
    // an angle
    let (cur_val, cur_expr, is_driven, is_angle) = match sk.project.sketches[si].constraints.get(ci) {
        // `DistancePL` stores a SIGNED d (which side); the magnitude is what gets shown and edited
        Some(Constraint::DistancePL { d, expr, driven, .. }) => (d.abs(), expr.clone(), driven, false),
        Some(Constraint::Distance { d, expr, driven, .. }) | Some(Constraint::Diameter { d, expr, driven, .. }) | Some(Constraint::EdgeDistance { d, expr, driven, .. }) => (*d, expr.clone(), driven, false),
        Some(Constraint::ArcLength { len, expr, driven, .. }) => (*len, expr.clone(), driven, false),
        Some(Constraint::Angle { deg, expr, driven, .. }) | Some(Constraint::AngleLines { deg, expr, driven, .. }) => (*deg, expr.clone(), driven, true),
        _ => {
            sk.inline.clear();
            return;
        }
    };
    // diameter or radius: which of the two the field shows
    let diam_mode = matches!(sk.project.sketches[si].constraints.get(ci), Some(Constraint::Diameter { diam: true, .. }));
    let is_diameter = matches!(sk.project.sketches[si].constraints.get(ci), Some(Constraint::Diameter { .. }));
    // the buffer of the field: on opening it holds the current expression, or the number
    let mut buf = std::mem::take(&mut sk.dim.buf);
    if want_focus {
        buf = if cur_expr.trim().is_empty() { qymcad_core::expr::fmt_num(cur_val) } else { cur_expr.clone() };
    }
    let eval_res = if buf.trim().is_empty() { None } else { Some(sk.project.eval_expr(&buf)) };
    let (mut toggle_driven, mut toggle_diam) = (false, false);
    let mut name_taken = false;
    let is_edge = matches!(sk.project.sketches[si].constraints.get(ci), Some(Constraint::EdgeDistance { .. }));
    let mut toggle_edge = false;
    let label = if is_angle { qymcad_i18n::tr("sk-angle") } else if is_diameter { if diam_mode { "Ø" } else { "R" }.to_string() } else if is_edge { qymcad_i18n::tr("sk-tangent-short") } else { qymcad_i18n::tr("sk-dim") };
    // A DRIVER BELONGS TO ANY DIMENSION, NOT ONLY TO A LINEAR ONE.
    //
    // This used to read "only `Constraint::Distance`", and the name field appeared solely on a distance
    // between two points. The question came up plainly: why does a drive field exist in some places and
    // not in others? There was no explainable logic behind it: the driver name WAS STORED as a pair of
    // points, so an angle, a diameter, a distance to a line, an arc length or a tangent gap simply had
    // nothing to be named by. Now a driver is identified by a set of entities, and any dimension can be
    // named.
    let dim_refs: Option<Vec<Id>> = sk.project.sketches[si].constraints.get(ci).and_then(Project::dim_refs);
    let sid = sk.project.sketches[si].id;
    let cur_name: String = dim_refs
        .as_ref()
        .map(|r| {
            let t = qymcad_core::model::DimTarget::Sketch { sketch: sid, refs: Project::dim_key_pub(r) };
            qymcad_i18n::name(&sk.project.name_of_target(&t))
        })
        .unwrap_or_default();
    // THE TEXT LIVES IN THE BUFFER OF THE FIELD ITSELF WHILE IT IS BEING TYPED. The model is touched
    // once, on Enter.
    let key_refs = dim_refs.clone().unwrap_or_default();
    // WHAT WAS ASKED FOR: a new driver name. It is applied AFTER the drawing - during it the document is
    // lent to the field (which reads the project), and there is no reason to change the model mid-frame.
    let mut name_commit: Option<String> = None;
    let mut name_owner: Option<String> = None;
    egui::Area::new(egui::Id::new(("dimedit", si, ci))).fixed_pos(qymcad_ui_state::clamp_popup(at, rect) + egui::vec2(8.0, -10.0)).order(egui::Order::Foreground).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(label);
                if is_diameter && ui.button(if diam_mode { "->R" } else { "->Ø" }).on_hover_text(qymcad_i18n::tr("sk-toggle-rad-dia")).clicked() {
                    toggle_diam = true;
                }
                if is_edge && ui.button(qymcad_i18n::tr("sk-edge-toggle")).on_hover_text(qymcad_i18n::tr("sk-near-far-edge")).clicked() {
                    toggle_edge = true;
                }
                if *is_driven {
                    ui.label(egui::RichText::new(qymcad_i18n::tr1("sk-driven-value", "v", &qymcad_i18n::num(cur_val, 3))).color(sk.scheme.pal.text_dim()));
                } else {
                    // ONE field for the whole project: a number or an expression (50, w/2, len+5), the list
                    // of drivers, auto-focus with the previous text selected, and Enter to apply.
                    let fid = egui::Id::new(("dimval", si, ci));
                    let o = qymcad_ui_state::expr_field_autofocus(ui, &*sk.project, fid, &buf, 100.0, &qymcad_i18n::tr("sk-expr-example"), want_focus);
                    buf = o.text;
                    got_focus = o.resp.has_focus();
                    if o.committed && enter {
                        close = true;
                    }
                }
                if ui.selectable_label(*is_driven, qymcad_i18n::tr("sk-ref-short")).on_hover_text(qymcad_i18n::tr("sk-driven-hint")).clicked() {
                    toggle_driven = true;
                }
                if ui.button(ph::CHECK).clicked() {
                    close = true;
                }
            });
            if !is_driven {
                match &eval_res {
                    Some(Ok(v)) => {
                        ui.label(egui::RichText::new(format!("= {v:.3}")).weak().small());
                    }
                    Some(Err(e)) => {
                        // THROUGH THE COMMON DOOR: the `Display` of the error is in one language only, and
                        // an interface in another showed a raw "unexpected token: /".
                        ui.label(egui::RichText::new(qymcad_i18n::error_words::expr_error_text(e)).color(sk.scheme.pal.error_mild()).small());
                    }
                    None => {}
                }
                ui.label(egui::RichText::new(qymcad_i18n::tr("sk-enter-apply-expr")).weak().small());
            }
            if dim_refs.is_some() {
                // WHETHER THE NAME WILL DO. An empty one is legitimate: the dimension simply stops being a
                // driver.
                let ok = |nm: &str| {
                    nm.is_empty() || (qymcad_core::drivers::check_ident(nm).is_ok() && !sk.project.driver_name_taken(nm, sid, &key_refs))
                };
                ui.horizontal(|ui| {
                    // THE CAPTION IS TRANSLATABLE. A raw "driver:" used to stand here - the only string of
                    // the popup that went around the language catalogue.
                    ui.label(qymcad_i18n::tr("sk-driver-label")).on_hover_text(qymcad_i18n::tr("sk-driver-name-hint"));
                    let id = egui::Id::new(("dimdrv", si, ci));
                    let r = qymcad_ui_state::name_field(ui, &*sk.project, id, &cur_name, 110.0, &qymcad_i18n::tr("sk-name-placeholder"), &ok);
                    let nm = r.text.trim().to_string();
                    if r.committed && nm != cur_name.trim() {
                        name_commit = Some(nm.clone());
                    }
                    // A TAKEN NAME GIVES A YELLOW LINE BELOW AND BLOCKS APPLYING. Not a single letter is
                    // lost while this happens: type `len`, `lena` or `length` - until the name is free, the
                    // popup simply refuses to apply and says why.
                    name_taken = !nm.is_empty() && !ok(&nm);
                    if name_taken {
                        name_owner = sk.project.name_owner(&nm).map(|o| if o.path.is_empty() { qymcad_i18n::tr("par-owner-project") } else { o.path });
                    }
                });
                if name_taken {
                    // WHOSE NAME IT IS, not merely "taken": otherwise the namesake has to be hunted for
                    // across the whole project by hand. A name with no owner is simply unusable in itself.
                    let msg = match &name_owner {
                        Some(w) => qymcad_i18n::tr2("par-name-taken", "name", "", "where", w),
                        None => qymcad_i18n::tr("sk-driver-name-bad"),
                    };
                    ui.label(egui::RichText::new(&msg).color(sk.scheme.pal.warning()).small()).on_hover_text(qymcad_i18n::tr("sk-driver-name-taken-hint"));
                }
            }
        });
    });
    if got_focus || *is_driven {
        sk.dim.focus = false; // focus was taken (or there is nothing to focus) - stop asking
    }
    sk.dim.buf = buf.clone();
    // a snapshot of the dimension AND of the point positions BEFORE the edit, so it can be ROLLED BACK if
    // the new value CONFLICTS (over-defines the sketch). Otherwise the solver silently tilted a vertical
    // or moved a fixed point - the soft least-squares compromise. An inconsistent dimension is rejected
    // rather than allowed to break the sketch.
    let old_con = sk.project.sketches[si].constraints.get(ci).cloned();
    // THE TEXT DIFFERS FROM WHAT IS STORED, so there is something to apply on commit. TEXTS are compared
    // rather than the number parsed again: parsing a value in the sketcher must go through one door
    // (`parse_num`), and a second `parse::<f64>()` is caught at once by the ratchet in `expr_fields.rs`.
    let shown_before = if cur_expr.trim().is_empty() { qymcad_core::expr::fmt_num(cur_val) } else { cur_expr.clone() };
    let value_differs = buf.trim() != shown_before.trim();
    let apply_value = close && !is_driven && value_differs;
    // apply the changes to the constraint
    let mut new_buf: Option<String> = None; // refresh the field after a value edit (a diameter/radius swap, say)
    if let Some(c) = sk.project.sketches[si].constraints.get_mut(ci) {
        if toggle_driven {
            match c {
                Constraint::Distance { driven, .. } | Constraint::DistancePL { driven, .. } | Constraint::Angle { driven, .. } | Constraint::Diameter { driven, .. } | Constraint::AngleLines { driven, .. } | Constraint::ArcLength { driven, .. } | Constraint::EdgeDistance { driven, .. } => *driven = !*driven,
                _ => {}
            }
            changed = true;
        }
        if toggle_diam {
            if let Constraint::Diameter { d, diam, expr, .. } = c {
                *d = if *diam { *d * 0.5 } else { *d * 2.0 }; // diameter to radius: the value is recomputed
                *diam = !*diam;
                if expr.trim().is_empty() {
                    new_buf = Some(qymcad_core::expr::fmt_num(*d)); // refresh the field
                }
            }
            changed = true;
        }
        // THE VALUE IS APPLIED ON COMMIT, NOT ON EVERY LETTER.
        //
        // Every keystroke used to edit the constraint, solve the sketch again and mark the document for a
        // rebuild: typing "125" meant three rebuilds, and on the intermediate "1" and "12" the sketch
        // honestly rebuilt itself to a different size. The answer is visible anyway - the line "= 42.500"
        // lives under the field, and it costs the document nothing.
        if apply_value {
            let t = buf.trim();
            let num = t.parse::<f64>().ok();
            match c {
                // `DistancePL`: d is signed (it says which side); the magnitude is typed and the sign is kept
                Constraint::DistancePL { d, expr, .. } => {
                    if let Some(v) = num {
                        *d = if *d < 0.0 { -v.abs() } else { v.abs() };
                        expr.clear();
                    } else {
                        *expr = buf.clone();
                    }
                }
                Constraint::Distance { d, expr, .. } | Constraint::Diameter { d, expr, .. } | Constraint::EdgeDistance { d, expr, .. } => {
                    if let Some(v) = num {
                        *d = v;
                        expr.clear();
                    } else {
                        *expr = buf.clone();
                    }
                }
                Constraint::ArcLength { len, expr, .. } => {
                    if let Some(v) = num {
                        *len = v;
                        expr.clear();
                    } else {
                        *expr = buf.clone();
                    }
                }
                Constraint::Angle { deg, expr, .. } | Constraint::AngleLines { deg, expr, .. } => {
                    if let Some(v) = num {
                        *deg = v;
                        expr.clear();
                    } else {
                        *expr = buf.clone();
                    }
                }
                _ => {}
            }
            changed = true;
        }
    }
    if let Some(nb) = new_buf {
        sk.dim.buf = nb; // the field shows the new value (after a diameter/radius swap)
    }
    if toggle_edge {
        // near edge against far edge: the non-zero m values are inverted and d is recomputed from the
        // geometry, so nothing jumps
        let edge = match sk.project.sketches[si].constraints.get(ci) {
            Some(Constraint::EdgeDistance { c1, c2, m1, m2, .. }) => Some((c1, c2, m1, m2)),
            _ => None,
        };
        if let Some((c1, c2, m1, m2)) = edge {
            let (nm1, nm2) = (if *m1 != 0 { -*m1 } else { 0 }, if *m2 != 0 { -*m2 } else { 0 });
            let nd = sk.project.measure_edge_distance(si, *c1, nm1, *c2, nm2);
            if let Some(Constraint::EdgeDistance { m1, m2, d, expr, .. }) = sk.project.sketches[si].constraints.get_mut(ci) {
                *m1 = nm1;
                *m2 = nm2;
                *d = nd;
                expr.clear();
            }
            changed = true;
        }
    }
    // THE DRIVER NAME GOES IN AS ONE OPERATION, ON ENTER.
    //
    // This was the main trouble that got reported: the name was written into the model on EVERY letter,
    // and `mark_param_dependents_dirty()` was called right after, marking EVERY node carrying dimensions
    // dirty - that is, the whole project was rebuilt per letter. Now the edit arrives once.
    if let (Some(refs), Some(nm)) = (dim_refs.clone(), name_commit) {
        let mut ed = qymcad_ui_state::edit_over(sk.rebuild(), qymcad_i18n::tr("sk-driver-step"));
        let old = cur_name.trim().to_string();
        if !old.is_empty() && !nm.is_empty() {
            // RENAMING CARRIES THE FORMULAS WITH IT - otherwise the expressions stay pointed at a name
            // that no longer exists, and the model breaks silently.
            let _ = ed.project().rename_driver(&old, &nm);
        } else {
            ed.project().add_named_dim(nm.clone(), sid, refs);
        }
        drop(ed);
        // ONLY WHAT DEPENDS ON THIS NAME is recomputed, not the whole timeline of every body.
        if !nm.is_empty() {
            sk.project.mark_param_dependents_dirty_for(&nm);
        }
        qymcad_ui_state::mark_dirty_for_rebuild(&mut sk.rebuild());
    }
    if changed {
        // a snapshot of the positions BEFORE solving (the sketch is consistent right now)
        let old_pts: Vec<(f64, f64)> = sk.project.sketches[si].points.iter().map(|p| (p.x, p.y)).collect();
        let resid = sk.project.solve_sketch(si);
        // a large residual means the system is INCONSISTENT (the new dimension conflicts with a vertical,
        // a fixed point or another constraint). The solver would converge to about 1e-6; a threshold of
        // 1e-2 mm is above the noise and below any real conflict. The dimension and the positions are
        // rolled back and solved again, so the sketch returns to a consistent state instead of breaking
        // silently.
        if resid > 1e-2 {
            if let (Some(oc), Some(c)) = (old_con.clone(), sk.project.sketches[si].constraints.get_mut(ci)) {
                *c = oc;
            }
            for (p, (x, y)) in sk.project.sketches[si].points.iter_mut().zip(old_pts) {
                p.x = x;
                p.y = y;
            }
            sk.project.solve_sketch(si);
            *sk.status = qymcad_i18n::tr1("sk-dim-incompatible", "r", &qymcad_i18n::num(resid, 2));
        }
        qymcad_ui_state::invalidate(&mut *sk.regen);
    }
    if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        sk.inline.clear();
        sk.dim.buf.clear();
        sk.dim.focus = false;
        if dim_refs.is_some() && !sk.project.named_dims.is_empty() {
            qymcad_ui_state::mark_dirty_for_rebuild(&mut sk.rebuild()); // the document is marked; the planner does the counting - the driver reaches the bodies that consume it
        }
    }
}

/// A click with a drawing tool: it adds entities to the active sketch.
pub fn sketch_tool_click(sk: &mut qymcad_ui_state::SketchCtx, rect: Rect, pos: Pos2) {
    // THE BOUNDARY OF AN OPERATION: a click with a drawing tool is a deliberate act, so it makes one undo
    // step with a name of its own. The step used to be "noticed" by the frame, so what got undone in a
    // sketch was not an action but whatever had accumulated between observations.
    let name = match sk.armed.draw_kind() {
        1 => &qymcad_i18n::tr("sk-line"),
        2 => &qymcad_i18n::tr("sk-rect"),
        3 => &qymcad_i18n::tr("sk-circle"),
        4 => &qymcad_i18n::tr("sk-arc"),
        5 => &qymcad_i18n::tr("sk-point"),
        6 => &qymcad_i18n::tr("sk-polygon"),
        7 => &qymcad_i18n::tr("sk-spline"),
        8 => &qymcad_i18n::tr("sk-ellipse"),
        9 => &qymcad_i18n::tr("sk-text"),
        11 => &qymcad_i18n::tr("sk-slot"),
        _ => &qymcad_i18n::tr("sk-drawing"),
    };
    qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, name);
    sketch_tool_click_inner(sk, rect, pos);
    qymcad_ui_state::commit_edit(&mut sk.rebuild());
}

pub fn sketch_tool_click_inner(sk: &mut qymcad_ui_state::SketchCtx, rect: Rect, pos: Pos2) {
    let w = snap_world(sk, rect, pos);
    let Some(si) = qymcad_ui_state::edit_si(&*sk.project, &*sk.sketch_ses) else { return };
    // a new click finishes the previous dimension entry
    sk.place.clear(); // everything unfinished in the drawing, all at once
    let con = sk.tool.construction;
    match sk.armed.draw_kind() {
        1 => {
            // a chain of lines: every further click adds a segment
            if let Some(&last) = sk.tool.pts.last() {
                let prev = (sk.tool.pts.len() >= 2).then(|| sk.tool.pts[sk.tool.pts.len() - 2]);
                sk.project.add_line_entity(si, last.x, last.y, w.x, w.y, qymcad_core::feature::Purpose::of(con));
                if !con && sk.set.auto_constrain {
                    // automatic constraints: horizontal or vertical, perpendicular to the previous segment,
                    // point-on-edge
                    infer_on_segment(&mut *sk.project, *sk.view, si, prev, last, w);
                }
                // stitch the ends to nearby existing vertices, so the corners do not fall apart. The
                // tolerance is PERCEPTUAL, measured on screen: it used to be clamped to 2.0 mm, so at a
                // distant zoom it welded ANY points within 2 mm, collapsing small geometry and neighbouring
                // shapes (parts under 2 mm could not be built at all). Now it is about 8 px with a low
                // ceiling of 0.4 mm: at high zoom it is nearly nothing (small detail survives), and at any
                // zoom it merges only what the eye cannot tell apart anyway. Snapping already returns the
                // exact coordinates of a vertex on a hit.
                let tol = (8.0 / sk.view.scale as f64).clamp(1e-4, 0.4);
                sk.project.merge_close_points(si, tol);
                qymcad_ui_state::invalidate(&mut *sk.regen);
            }
            sk.tool.pts.push(w);
        }
        2 => {
            sk.tool.pts.push(w);
            let need = if sk.tool_prefs.rect_mode == 2 { 3 } else { 2 };
            if sk.tool.pts.len() == need {
                // (the ids of the sides, corner a and corner b for the width-by-height editor, whether it is
                // axis-aligned)
                let (ids, ra, rb, axis_aligned) = match sk.tool_prefs.rect_mode {
                    1 => {
                        // centre plus a corner: the opposite corner is its mirror through the centre
                        let (c, cr) = (sk.tool.pts[0], sk.tool.pts[1]);
                        let a = Point2::new(2.0 * c.x - cr.x, 2.0 * c.y - cr.y);
                        (sk.project.add_rect_entity(si, a.x, a.y, cr.x, cr.y, qymcad_core::feature::Purpose::of(con)), a, cr, true)
                    }
                    2 => {
                        // three points give a rotated rectangle
                        let (p1, p2, p3) = (sk.tool.pts[0], sk.tool.pts[1], sk.tool.pts[2]);
                        (sk.project.add_rect3_entity(si, Point2::new(p1.x, p1.y), Point2::new(p2.x, p2.y), Point2::new(p3.x, p3.y), qymcad_core::feature::Purpose::of(con)), p1, p2, false)
                    }
                    _ => {
                        let (a, b) = (sk.tool.pts[0], sk.tool.pts[1]);
                        (sk.project.add_rect_entity(si, a.x, a.y, b.x, b.y, qymcad_core::feature::Purpose::of(con)), a, b, true)
                    }
                };
                sk.tool.pts.clear();
                qymcad_ui_state::invalidate(&mut *sk.regen);
                if !con && axis_aligned {
                    sk.place.set(qymcad_ui_state::PlacingShape::Rect(ra, rb, ids)); // typing the width and height (axis-aligned)
                    sk.place.focus = true;
                }
            }
        }
        3 if sk.tool_prefs.circ_mode == 2 => {
            // A TANGENT circle: the first click picks the base edge, the second the centre (the radius
            // follows from the tangency)
            if sk.tool.circ_tan.is_none() {
                if let Some(eref) = ref_edge_at(sk, rect, pos, si) {
                    sk.tool.circ_tan = Some(eref);
                    *sk.status = qymcad_i18n::tr("sk-tangent-base-picked");
                } else {
                    *sk.status = qymcad_i18n::tr("sk-tangent-pick-base");
                }
            } else if let Some(eref) = sk.tool.circ_tan.take() {
                let r = qymcad_ui_state::tangent_radius_to_edge(&sk.draw(), si, eref, w);
                if r > 1e-6 {
                    let eid = sk.project.add_circle_entity(si, w.x, w.y, r, qymcad_core::feature::Purpose::of(con));
                    let cen = sk.project.sketch_point_at(si, w.x, w.y, 1e-6);
                    qymcad_ui_state::add_tangent_to_edge(&mut *sk.project, si, eref, cen, w.x, w.y, r);
                    sk.project.solve_sketch(si);
                    qymcad_ui_state::invalidate(&mut *sk.regen);
                    if !con {
                        *sk.inline = qymcad_ui_state::InlineEdit::Circle(eid);
                        sk.dim.focus = true;
                    }
                } else {
                    *sk.status = qymcad_i18n::tr("sk-centre-on-edge");
                }
            }
        }
        3 => {
            sk.tool.pts.push(w);
            if sk.tool.pts.len() == 2 {
                let (p0, p1) = (sk.tool.pts[0], sk.tool.pts[1]);
                let (cx, cy, r) = if sk.tool_prefs.circ_mode == 1 {
                    // by two points: the ends of a diameter
                    let d = ((p1.x - p0.x).powi(2) + (p1.y - p0.y).powi(2)).sqrt();
                    (0.5 * (p0.x + p1.x), 0.5 * (p0.y + p1.y), 0.5 * d)
                } else {
                    // centre plus radius
                    (p0.x, p0.y, ((p1.x - p0.x).powi(2) + (p1.y - p0.y).powi(2)).sqrt())
                };
                let eid = sk.project.add_circle_entity(si, cx, cy, r, qymcad_core::feature::Purpose::of(con));
                sk.tool.pts.clear();
                qymcad_ui_state::invalidate(&mut *sk.regen);
                if !con {
                    *sk.inline = qymcad_ui_state::InlineEdit::Circle(eid); // straight into typing the radius or diameter
                    sk.dim.focus = true;
                }
            }
        }
        4 if sk.tool_prefs.arc_mode == 2 => {
            // A TANGENT arc: the start (the end of a line or an arc) plus the end; a smooth continuation
            sk.tool.pts.push(w);
            if sk.tool.pts.len() == 2 {
                let (s, e) = (sk.tool.pts[0], sk.tool.pts[1]);
                if let Some((t, eref)) = qymcad_pick::arc_tangent_ref(&sk.draw(), si, s) {
                    if let Some((cx, cy, r, ccw)) = qymcad_ui_state::tangent_arc(s, t, e) {
                        sk.project.add_arc_entity(si, Point2::new(cx, cy), Point2::new(s.x, s.y), Point2::new(e.x, e.y), winding(ccw), qymcad_core::feature::Purpose::of(con));
                        let cen = sk.project.sketch_point_at(si, cx, cy, 1e-6);
                        qymcad_ui_state::add_tangent_to_edge(&mut *sk.project, si, eref, cen, cx, cy, r);
                        sk.project.solve_sketch(si);
                        qymcad_ui_state::invalidate(&mut *sk.regen);
                    } else {
                        *sk.status = qymcad_i18n::tr("sk-arc-end-on-tangent");
                    }
                } else {
                    *sk.status = qymcad_i18n::tr("sk-tangent-arc-hint");
                }
                sk.tool.pts.clear();
            }
        }
        4 => {
            sk.tool.pts.push(w);
            if sk.tool.pts.len() == 3 {
                let (p0, p1, p2) = (sk.tool.pts[0], sk.tool.pts[1], sk.tool.pts[2]);
                if sk.tool_prefs.arc_mode == 1 {
                    // by three points: the start, the end and a point on the arc give the circumscribed circle
                    let (s, e, m) = (p0, p1, p2);
                    if let Some((cx, cy, _r)) = qymcad_ui_state::circumcircle(s, e, m) {
                        // the orientation start-mid-end: counter-clockwise when the triple turns that way
                        let ccw = (m.x - s.x) * (e.y - s.y) - (m.y - s.y) * (e.x - s.x) > 0.0;
                        sk.project.add_arc_entity(si, Point2::new(cx, cy), Point2::new(s.x, s.y), Point2::new(e.x, e.y), winding(ccw), qymcad_core::feature::Purpose::of(con));
                    } else {
                        *sk.status = qymcad_i18n::tr("sk-points-collinear-arc");
                    }
                } else {
                    // the centre, the start and the end
                    let (c, a, b) = (p0, p1, p2);
                    let ccw = (a.x - c.x) * (b.y - c.y) - (a.y - c.y) * (b.x - c.x) > 0.0;
                    sk.project.add_arc_entity(si, Point2::new(c.x, c.y), Point2::new(a.x, a.y), Point2::new(b.x, b.y), winding(ccw), qymcad_core::feature::Purpose::of(con));
                }
                sk.tool.pts.clear();
                qymcad_ui_state::invalidate(&mut *sk.regen);
            }
        }
        5 => {
            // a point is a single node
            sk.project.sketch_point_at(si, w.x, w.y, 1e-6);
            qymcad_ui_state::invalidate(&mut *sk.regen);
        }
        6 => {
            // a polygon: the centre plus a vertex (the number of sides and the kind come from the options bar)
            sk.tool.pts.push(w);
            if sk.tool.pts.len() == 2 {
                let (c, vtx) = (sk.tool.pts[0], sk.tool.pts[1]);
                let n = sk.tool_prefs.poly_n.max(3);
                let half = std::f64::consts::PI / n as f64;
                // the radius of the circumscribed circle (through the vertices), by the click mode
                let r_click = ((vtx.x - c.x).powi(2) + (vtx.y - c.y).powi(2)).sqrt().max(1e-6);
                let rr = match sk.tool_prefs.poly_mode {
                    1 => r_click * half.cos(),                              // inscribed, touching the edges
                    2 => sk.tool_prefs.poly_edge.max(0.01) / (2.0 * half.sin()),  // by the length of an edge
                    _ => r_click,                                           // circumscribed, through the vertices
                };
                // the vertex direction, normalised to the required radius rr (the angle of the click is kept)
                let (ux, uy) = ((vtx.x - c.x) / r_click, (vtx.y - c.y) / r_click);
                // A PARAMETRIC regular polygon: the circumscribed circle plus constraints
                let (center, _sides) = sk.project.add_polygon_param(si, Point2::new(c.x, c.y), Point2::new(c.x + ux * rr, c.y + uy * rr), n, qymcad_core::feature::Purpose::of(con));
                sk.tool.pts.clear();
                qymcad_ui_state::invalidate(&mut *sk.regen);
                sk.place.set(qymcad_ui_state::PlacingShape::Poly(center)); // typing the radius and angle through the centre handle
                sk.place.focus = true;
            }
        }
        7 => {
            // a slot: two centres plus a point that sets the width
            sk.tool.pts.push(w);
            if sk.tool.pts.len() == 3 {
                let (a, b, e) = (sk.tool.pts[0], sk.tool.pts[1], sk.tool.pts[2]);
                let (dx, dy) = (b.x - a.x, b.y - a.y);
                let len = (dx * dx + dy * dy).sqrt().max(1e-6);
                let r = ((e.x - a.x) * (-dy) + (e.y - a.y) * dx).abs() / len;
                sk.project.add_slot_entity(si, Point2::new(a.x, a.y), Point2::new(b.x, b.y), r.max(0.5), qymcad_core::feature::Purpose::of(con));
                sk.tool.pts.clear();
                qymcad_ui_state::invalidate(&mut *sk.regen);
            }
        }
        8 => {
            // an ellipse: the centre plus a corner of the bounding rectangle, then the width and height are typed
            sk.tool.pts.push(w);
            if sk.tool.pts.len() == 2 {
                let (cc, corner) = (sk.tool.pts[0], sk.tool.pts[1]);
                let (rx, ry) = ((corner.x - cc.x).abs(), (corner.y - cc.y).abs());
                // A PARAMETRIC ellipse entity (the axes are perpendicular and the semi-axes can carry
                // dimensions); the rotation is 0, so the axes follow X and Y
                let center = sk.project.add_ellipse_entity(si, Point2::new(cc.x, cc.y), rx.max(0.5), ry.max(0.5), 0.0, qymcad_core::feature::Purpose::of(con));
                sk.tool.pts.clear();
                qymcad_ui_state::invalidate(&mut *sk.regen);
                sk.place.set(qymcad_ui_state::PlacingShape::Ellipse(center, cc)); // typing the semi-axes through the centre handle
                sk.place.focus = true;
            }
        }
        9 => {
            // a spline: a set of nodes (a double click or Esc finishes it)
            sk.tool.pts.push(w);
        }
        10 => {
            // a circle through three points
            sk.tool.pts.push(w);
            if sk.tool.pts.len() == 3 {
                if let Some((cx, cy, r)) = qymcad_ui_state::circumcircle(sk.tool.pts[0], sk.tool.pts[1], sk.tool.pts[2]) {
                    let eid = sk.project.add_circle_entity(si, cx, cy, r, qymcad_core::feature::Purpose::of(con));
                    qymcad_ui_state::invalidate(&mut *sk.regen);
                    if !con {
                        *sk.inline = qymcad_ui_state::InlineEdit::Circle(eid); // straight into typing the diameter, as with an ordinary circle
                        sk.dim.focus = true;
                    }
                } else {
                    *sk.status = qymcad_i18n::tr("sk-points-collinear-circle");
                }
                sk.tool.pts.clear();
            }
        }
        11 if sk.tool_prefs.text_note => {
            // a text note, which is not geometry
            if !sk.tool_prefs.text.trim().is_empty() {
                sk.project.add_note(si, w.x, w.y, sk.tool_prefs.text.clone());
                *sk.status = qymcad_i18n::tr("sk-note-added");
            }
        }
        11 if sk.tool_prefs.text.trim().is_empty() => {
            // AN EMPTY STRING IS NO REASON TO STAY SILENT. The tool was armed, the click went through, and
            // nothing happened: there was no telling whether the program was broken or something had been
            // left undone.
            *sk.status = qymcad_i18n::tr("sk-text-needs-string");
        }
        11 => {
            // text AS GEOMETRY: a parametric object (the outlines of the glyphs) that can be selected,
            // moved, scaled and retyped - not the loose contours it used to be
            let glyphs = qymcad_ui_state::bake_text_glyphs(&mut *sk.font_cache, w.x, w.y, sk.tool_prefs.text_h, &sk.tool_prefs.text.clone());
            let n = glyphs.len();
            if n > 0 {
                let id = sk.project.add_sketch_text(
                    si,
                    qymcad_core::model::TextSpec { at: w, height: sk.tool_prefs.text_h, angle: 0.0, text: sk.tool_prefs.text.clone(), glyphs },
                    qymcad_core::feature::Purpose::of(sk.tool.construction),
                );
                sk.annot.text = sk.project.sketches[si].texts.iter().position(|t| t.id == id);
                qymcad_ui_state::invalidate(&mut *sk.regen);
                sk.view.initialized = false;
                *sk.status = qymcad_i18n::tr1("sk-text-placed", "n", &n.to_string());
            } else if qymcad_ui_state::default_font(&mut *sk.font_cache).is_none() {
                *sk.status = qymcad_i18n::tr("sk-font-not-found");
            } else {
                *sk.status = qymcad_i18n::tr("sk-text-empty");
            }
        }
        _ => {}
    }
}

/// The index of the constraint whose glyph is nearest to a screen point (for deleting by click).
pub fn constraint_glyph_at(sk: &mut qymcad_ui_state::SketchCtx, rect: Rect, pos: Pos2, si: usize) -> Option<usize> {
    qymcad_pick::constraint_glyphs(&sk.pick(), rect, si).into_iter().find(|(_, at, _)| at.distance(pos) <= qymcad_ui_state::grab::grab(&*sk.set, Grab::Label)).map(|(ci, _, _)| ci)
}

/// THE START OF A DRAG IN A SKETCH: what exactly was grabbed - a text, a note, a dimension caption, a
/// point, a spline handle, the move gizmo or a selection box.
///
/// This is A PRIORITY CHAIN: the links are tried in order, and the first one that fires takes the drag
/// for itself. While it lay inside the viewport the order of the links was invisible, and each link
/// checked "is it taken" with ITS OWN set of conditions, which had drifted apart: dragging a text still
/// fired the dimension branch. Now the chain has a name, and the question
/// "is something already being dragged?" is asked in exactly one place - `qymcad_ui_state::Dragging::active`.
pub fn sketch_drag_start(sk: &mut qymcad_ui_state::SketchCtx, ctx: &egui::Context, resp: &egui::Response, rect: Rect, ctrl: bool, handle: Option<qymcad_core::geom::Point2>) {
    let sh = qymcad_ui_state::Sheet { view: *sk.view, rect: rect };
            if resp.drag_started() && ctx.input(|i| i.pointer.button_down(egui::PointerButton::Primary)) {
                let pp = resp.interact_pointer_pos();
                // moving a text object (the highest priority among the captions)
                if !sk.drag.active() && !ctrl {
                    if let (qymcad_ui_state::Sel::Sketch(si), Some(pp)) = (*sk.sel, pp) {
                        if let Some(ti) = qymcad_ui_state::text_at(&*sk.project, &*sk.view, rect, pp, si) {
                            *sk.drag = qymcad_ui_state::Dragging::Text(ti);
                            sk.annot.text = Some(ti);
                        }
                    }
                }
                // moving a note (it takes priority)
                if !sk.drag.active() && !ctrl {
                    if let (qymcad_ui_state::Sel::Sketch(si), Some(pp)) = (*sk.sel, pp) {
                        if let Some(ni) = qymcad_ui_state::note_at(&*sk.project, &*sk.view, rect, pp, si) {
                            *sk.drag = qymcad_ui_state::Dragging::Note(ni);
                        }
                    }
                }
                // offsetting a dimension line: the caption of a linear dimension is dragged
                if !sk.drag.active() && !ctrl {
                    if let (qymcad_ui_state::Sel::Sketch(si), Some(pp)) = (*sk.sel, pp) {
                        if let Some(ci) = dim_at(sk, rect, pp, si) {
                            // a diameter or radius, an arc length and a tangent distance are draggable too -
                            // their labels used to be impossible to move.
                            if matches!(sk.project.sketches[si].constraints.get(ci), Some(qymcad_core::model::Constraint::Distance { .. }) | Some(qymcad_core::model::Constraint::DistancePL { .. }) | Some(qymcad_core::model::Constraint::Diameter { .. }) | Some(qymcad_core::model::Constraint::ArcLength { .. }) | Some(qymcad_core::model::Constraint::EdgeDistance { .. })) {
                                *sk.drag = qymcad_ui_state::Dragging::Dim(ci);
                                sk.gsel.constraint = Some(ci); // grabbing a dimension selects it, even on a tiny drag
                                sk.annot.note = None;
                            }
                        } else if qymcad_ui_state::edit_si(&*sk.project, &*sk.sketch_ses) == Some(si) {
                            if let Some((center, diam)) = passive_radius_label_at(sk, rect, pp, si) {
                                // on A CIRCLE or AN ARC with no dimension the label is automatic, so a
                                // DRIVEN dimension is materialised (it changes no degrees of freedom) to let
                                // the label be turned about the centre instead of dragging the whole
                                // geometry - grabbing the radius used to fall through into moving the entire
                                // arc. A circle gets a diameter, an arc a radius.
                                use qymcad_core::model::Constraint;
                                let r = qymcad_ui_state::center_radius(&sk.draw(), si, center).unwrap_or(0.0);
                                let ang = qymcad_ui_state::sketch_pt(&*sk.project, si, center).map(|cp| { let sc = sh.at(cp); (pp.y - sc.y).atan2(pp.x - sc.x) as f64 }).unwrap_or(0.0);
                                let d = if diam { 2.0 * r } else { r };
                                sk.project.sketches[si].constraints.push(Constraint::Diameter { c: center, d, off: ang, expr: String::new(), driven: true, diam });
                                let ci = sk.project.sketches[si].constraints.len() - 1;
                                *sk.drag = qymcad_ui_state::Dragging::Dim(ci);
                                sk.gsel.constraint = Some(ci);
                                sk.annot.note = None;
                            }
                        }
                    }
                }
                // a tangent handle of a spline takes the highest priority - it is grabbed before a point
                if !sk.drag.active() && !ctrl {
                    if let (qymcad_ui_state::Sel::Sketch(si), Some(pp)) = (*sk.sel, pp) {
                        if qymcad_ui_state::edit_si(&*sk.project, &*sk.sketch_ses) == Some(si) {
                            let mut best: Option<(f32, usize, usize)> = None;
                            for spi in 0..sk.project.sketches[si].splines.len() {
                                for (ki, (_knot, hend)) in sk.project.spline_handles(si, spi).into_iter().enumerate() {
                                    let d = sh.at(hend).distance(pp);
                                    if d <= qymcad_ui_state::grab::grab(sk.set, Grab::Point) && best.is_none_or(|(bd, _, _)| d < bd) {
                                        best = Some((d, spi, ki));
                                    }
                                }
                            }
                            if let Some((_, spi, ki)) = best {
                                *sk.drag = qymcad_ui_state::Dragging::Handle(si, spi, ki);
                            }
                        }
                    }
                }
                // a point of the selected typed sketch is dragged when the cursor is near it
                if !sk.drag.active() && !ctrl {
                    if let (qymcad_ui_state::Sel::Sketch(si), Some(pp)) = (*sk.sel, pp) {
                        if sk.project.is_typed_sketch(si) {
                            // the points of arcs (the centre, the tangencies of fillets) are not dragged, or
                            // the fillet breaks
                            let mut arc_pts: std::collections::HashSet<Id> = sk.project.sketches[si].entities.iter().flat_map(|e| match e.kind {
                                qymcad_core::model::EntityKind::Arc { center, a, b, .. } => vec![center, a, b],
                                _ => vec![],
                            }).collect();
                            // reference points (the origin, the axes, materialised midpoints) are not dragged
                            arc_pts.extend(sk.project.sketches[si].system_ids());
                            for c in &sk.project.sketches[si].constraints {
                                match c {
                                    qymcad_core::model::Constraint::Midpoint { p, .. } => {
                                        arc_pts.insert(*p);
                                    }
                                    // a pinned point is not dragged - it is fixed
                                    qymcad_core::model::Constraint::Fixed { p } => {
                                        arc_pts.insert(*p);
                                    }
                                    _ => {}
                                }
                            }
                            // DRIVEN POINTS (projections of the geometry of a body) are not dragged: their
                            // position is set by the part, and a projection dragged by hand would snap back
                            // at the very first rebuild, silently undoing the work.
                            begin_point_drag(&mut *sk.drag, &*sk.project, sk.set, qymcad_ui_state::Sheet { view: *sk.view, rect }, si, pp, &arc_pts);
                        }
                    }
                }
                // moving the whole selected geometry: the cursor is on A SELECTED entity, not on a point
                if !sk.drag.active() && sk.drag.mov().is_none() && !ctrl {
                    if let (qymcad_ui_state::Sel::Sketch(si), Some(pp)) = (*sk.sel, pp) {
                        if sk.project.is_typed_sketch(si) && !sk.sel_sk.items.is_empty() {
                            if let Some(h) = sketch_hit(&sk.pick(), rect, pp, si) {
                                if sk.sel_sk.items.contains(&h) {
                                    let ids = sketch_sel_points(&*sk.project, &*sk.sel_sk, si);
                                    if !ids.is_empty() {
                                        *sk.drag = qymcad_ui_state::Dragging::Move(si, ids);
                                    }
                                }
                            }
                        }
                    }
                }
                // a selection box works with the primary button ONLY, and with no other drag mode active
                // (not while a dimension, a note or a point is being dragged, and not while the middle
                // button pans)
                let no_other = !sk.drag.active() && sk.drag.mov().is_none() && sk.drag.handle().is_none();
                let primary = resp.drag_started_by(egui::PointerButton::Primary);
                if no_other && primary {
                    let on_empty = matches!(*sk.sel, qymcad_ui_state::Sel::Sketch(si) if qymcad_ui_state::edit_si(&*sk.project, &*sk.sketch_ses) == Some(si))
                        && pp.is_some_and(|p| if let qymcad_ui_state::Sel::Sketch(si) = *sk.sel { sketch_hit(&sk.pick(), rect, p, si).is_none() } else { false });
                    if ctrl || on_empty {
                        sk.tree_sel.box_start = resp.interact_pointer_pos();
                    } else if let (Some(hw), Some(pp)) = (handle, resp.interact_pointer_pos()) {
                        if sh.at(hw).distance(pp) <= qymcad_ui_state::grab::grab(sk.set, Grab::Point) {
                            sk.body_giz.dragging = true;
                        }
                    }
                }
            }
}

/// CONTINUING A DRAG IN A SKETCH: whatever was grabbed is carried until the button is released.
///
/// The paired phase to `sketch_drag_start`: that one decides WHAT was grabbed, this one what to do with
/// it every frame. While both lay in one piece there was no telling "picking an object" from "carrying
/// it", and those are different things: the first fires once, the second on every frame.
///
/// POWER TRIM: trimming BY DRAGGING - the cursor passes through several segments, and every one it
/// crossed gets trimmed.
///
/// Trimming by click existed and remains; dragging is for where a lot has to be cut in a row - taking
/// apart a grid of construction lines one click at a time means dozens of careful hits.
///
/// A TRAIL is followed rather than a single point: between frames the cursor jumps by tens of pixels,
/// and checking only the final position would miss the segments the mouse flew across. Every segment is
/// cut once per drag - otherwise the same piece would keep being trimmed until its neighbours
/// disappeared.
pub fn power_trim_drag(sk: &mut qymcad_ui_state::SketchCtx, resp: &egui::Response, rect: Rect) {
    if sk.armed.click_op() != 1 {
        return;
    }
    if resp.drag_started() {
        sk.trim.path.clear();
        sk.trim.done.clear();
    }
    if !resp.dragged() {
        if resp.drag_stopped() {
            sk.trim.path.clear();
            sk.trim.done.clear();
        }
        return;
    }
    let Some(now) = resp.interact_pointer_pos() else { return };
    let prev = sk.trim.path.last().copied().unwrap_or(now);
    sk.trim.path.push(now);
    power_trim_sweep(sk, rect, prev, now);
}

/// ONE pass of the trail from `prev` to `now`: it cuts everything it went through. Returns the number of
/// spans trimmed.
///
/// Split out of the drag handler so that a test can drive it: a real `egui::Response` cannot be built in
/// a headless test, while cutting along a trail is exactly the logic the tool exists for.
pub fn power_trim_sweep(sk: &mut qymcad_ui_state::SketchCtx, rect: Rect, prev: Pos2, now: Pos2) -> usize {
    let qymcad_ui_state::Sel::Sketch(si) = *sk.sel else { return 0 };
    // SAMPLING ALONG THE TRAIL: between frames the cursor covers a noticeable distance, so the segment
    // from `prev` to `now` is walked in steps of a couple of pixels rather than checked only at its end.
    let steps = ((prev.distance(now) / 3.0).ceil() as usize).clamp(1, 64);
    let mut cut = 0;
    for k in 0..=steps {
        let t = k as f32 / steps as f32;
        let at = egui::pos2(prev.x + (now.x - prev.x) * t, prev.y + (now.y - prev.y) * t);
        let w = qymcad_ui_state::to_world(&*sk.view, rect, at);
        let hit = qymcad_pick::nearest_line_eid(&sk.pick(), rect, at, si).map(|e| (e, true)).or_else(|| qymcad_pick::nearest_circle_entity(&sk.pick(), rect, at, si).map(|e| (e, false)));
        let Some((eid, is_line)) = hit else { continue };
        // ONE SPAN, ONE CUT PER DRAG: the key is the entity plus the span that was hit. Without it a
        // trembling cursor standing still would go on trimming the neighbouring pieces.
        let span = trim_span_key(&*sk.project, si, eid, w.x, w.y);
        if !sk.trim.done.insert((eid, span)) {
            continue;
        }
        let ok = if is_line { sk.project.trim_line(si, eid, w.x, w.y) } else { sk.project.trim_curve(si, eid, w.x, w.y) };
        if ok {
            cut += 1;
        }
    }
    if cut > 0 {
        sk.sel_sk.clear();
        qymcad_ui_state::invalidate(&mut *sk.regen);
        *sk.status = qymcad_i18n::tr1("sk-trimmed-n", "n", &sk.trim.done.len().to_string());
    }
    cut
}

/// Move the selected object (a contour or a body) in XY.
pub fn translate_selected(sk: &mut qymcad_ui_state::SketchCtx, dx: f64, dy: f64) {
    match *sk.sel {
        qymcad_ui_state::Sel::Contour(i) => {
            if let Some(c) = sk.project.contours.get_mut(i) {
                c.translate(dx, dy);
                qymcad_ui_state::invalidate(&mut *sk.regen);
            }
        }
        qymcad_ui_state::Sel::Mesh(i)
            if i < sk.project.bodies.len() =>
        {
            qymcad_ui_state::move_body_at(&mut sk.rebuild(), i, qymcad_ui_state::mat_translate(dx, dy, 0.0)); // a B-rep gets a Move feature; a raw mesh is simply shifted
        }
        _ => {}
    }
}

/// The reference object under the cursor: a point or centre > the midpoint of a line > the origin.
/// Used for dimensions and constraints between any geometry.
pub fn resolve_sketch_ref(sk: &mut qymcad_ui_state::SketchCtx, rect: Rect, pos: Pos2, si: usize) -> Option<qymcad_ui_state::SketchRef> {
    let sh = qymcad_ui_state::Sheet { view: *sk.view, rect: rect };
    use qymcad_core::model::EntityKind;
    // 1) the nearest point (the centres of circles and arcs are points too)
    if let Some(id) = qymcad_pick::nearest_sketch_point(&sk.pick(), rect, pos, si) {
        return Some(qymcad_ui_state::SketchRef::Point(id));
    }
    // 2) the midpoint of a segment
    if let Some(s) = sk.project.sketches.get(si) {
        let mut best: Option<(f32, (Id, Id))> = None;
        for e in &s.entities {
            if let EntityKind::Line { a, b } = e.kind {
                if let (Some(pa), Some(pb)) = (qymcad_ui_state::sketch_pt(&*sk.project, si, a), qymcad_ui_state::sketch_pt(&*sk.project, si, b)) {
                    let mid = Point2::new((pa.x + pb.x) * 0.5, (pa.y + pb.y) * 0.5);
                    let d = sh.at(mid).distance(pos);
                    if d <= qymcad_ui_state::grab::grab(sk.set, Grab::Point) && best.is_none_or(|(bd, _)| d < bd) {
                        best = Some((d, (a, b)));
                    }
                }
            }
        }
        if let Some((_, ab)) = best {
            return Some(qymcad_ui_state::SketchRef::Midpoint(ab.0, ab.1));
        }
    }
    // 3) the origin
    // the origin is a point: 11 px in place became the Point role (10 px at normal precision)
    if sh.at(Point2::new(0.0, 0.0)).distance(pos) <= qymcad_ui_state::grab::grab(sk.set, Grab::Point) {
        return Some(qymcad_ui_state::SketchRef::Origin);
    }
    None
}

/// The reference under the cursor for a dimension: a vertex or centre (Point) > a line entity (Line)
/// > a coordinate axis (Line) > a midpoint or the origin (Point). Materialises the midpoint, the
/// > origin or the axis.
pub fn resolve_dim_ref(sk: &mut qymcad_ui_state::SketchCtx, rect: Rect, pos: Pos2, si: usize) -> Option<qymcad_ui_state::DimRef> {
    // 1) a vertex or the centre of a circle (a real point)
    if let Some(id) = qymcad_pick::nearest_sketch_point(&sk.pick(), rect, pos, si) {
        return Some(qymcad_ui_state::DimRef::Point(id));
    }
    // 2) a line entity under the cursor
    if let Some((a, b)) = qymcad_pick::nearest_line_entity(&sk.pick(), rect, pos, si) {
        return Some(qymcad_ui_state::DimRef::Line(a, b));
    }
    // 2b) a circle or arc under the cursor (a click on the rim) resolves to its CENTRE as a point
    // reference, openly rather than silently: the dimension is taken from the centre. The status line
    // says so.
    if let Some(eid) = qymcad_pick::nearest_circle_entity(&sk.pick(), rect, pos, si) {
        let center = sk.project.sketches.get(si).and_then(|s| s.entities.iter().find(|e| e.id == eid)).and_then(|e| match e.kind {
            qymcad_core::model::EntityKind::Circle { center, .. } | qymcad_core::model::EntityKind::Arc { center, .. } => Some(center),
            _ => None,
        });
        if let Some(c) = center {
            *sk.status = qymcad_i18n::tr("g-dim-from-centre");
            return Some(qymcad_ui_state::DimRef::Point(c));
        }
    }
    // 3) a coordinate axis
    let o = (qymcad_ui_state::Sheet { view: *sk.view, rect: rect }).at(Point2::new(0.0, 0.0));
    if (pos.y - o.y).abs() <= qymcad_ui_state::grab::grab(sk.set, Grab::Guide) {
        let (a, b) = sk.project.ensure_axis(si, 0);
        return Some(qymcad_ui_state::DimRef::Line(a, b));
    }
    if (pos.x - o.x).abs() <= qymcad_ui_state::grab::grab(sk.set, Grab::Guide) {
        let (a, b) = sk.project.ensure_axis(si, 1);
        return Some(qymcad_ui_state::DimRef::Line(a, b));
    }
    // 4) the midpoint of a line or the origin (through the shared reference resolver)
    if let Some(r) = resolve_sketch_ref(sk, rect, pos, si) {
        return Some(qymcad_ui_state::DimRef::Point(qymcad_ui_state::materialize_ref(&mut *sk.project, si, r)));
    }
    None
}

/// The base edge under a screen point (a line, a circle or an arc) — for a tangent circle.
pub fn ref_edge_at(sk: &mut qymcad_ui_state::SketchCtx, rect: Rect, pos: Pos2, si: usize) -> Option<qymcad_ui_state::EdgeRef> {
    use qymcad_core::model::EntityKind;
    let (k, eid) = sketch_hit(&sk.pick(), rect, pos, si)?;
    if k != 1 {
        return None;
    }
    let s = sk.project.sketches.get(si)?;
    let e = s.entities.iter().find(|e| e.id == eid)?;
    match e.kind {
        EntityKind::Line { a, b } => Some(qymcad_ui_state::EdgeRef::Line { a, b }),
        EntityKind::Circle { center, r } => Some(qymcad_ui_state::EdgeRef::Circle { center, r }),
        EntityKind::Arc { center, a, .. } => {
            let (pc, pa) = (qymcad_ui_state::sketch_pt(&*sk.project, si, center)?, qymcad_ui_state::sketch_pt(&*sk.project, si, a)?);
            Some(qymcad_ui_state::EdgeRef::Circle { center, r: ((pa.x - pc.x).powi(2) + (pa.y - pc.y).powi(2)).sqrt() })
        }
        EntityKind::Ellipse { .. } => None,
    }
}

/// The centre of the curve whose PASSIVE radius or diameter label (a curve WITHOUT a Diameter
/// constraint) is under the cursor. Returns (center, diam): a circle gives a diameter (diam = true),
/// an arc gives a radius (diam = false). A circle's label sits in the off=0 style (to the right,
/// beyond the rim); an arc's sits by the middle of the arc, where the passive leader draws it.
pub fn passive_radius_label_at(sk: &mut qymcad_ui_state::SketchCtx, rect: Rect, pos: Pos2, si: usize) -> Option<(Id, bool)> {
    let sh = qymcad_ui_state::Sheet { view: *sk.view, rect: rect };
    use qymcad_core::model::{Constraint, EntityKind};
    let s = sk.project.sketches.get(si)?;
    for e in &s.entities {
        let (center, diam) = match e.kind {
            EntityKind::Circle { center, .. } => (center, true),
            EntityKind::Arc { center, .. } => (center, false),
            _ => continue,
        };
        if s.constraints.iter().any(|x| matches!(x, Constraint::Diameter { c, .. } if *c == center)) {
            continue;
        }
        let Some(cp) = qymcad_ui_state::sketch_pt(&*sk.project, si, center) else { continue };
        let Some(r) = qymcad_ui_state::center_radius(&sk.draw(), si, center) else { continue };
        let sc = sh.at(cp);
        let r_px = (sh.at(Point2::new(cp.x + r, cp.y)) - sc).length();
        // the leader's direction: to the right for a circle (off=0), towards the middle of the arc for an arc, as it is drawn
        let dir = if diam {
            egui::vec2(1.0, 0.0)
        } else if let EntityKind::Arc { a, b, .. } = e.kind {
            match (qymcad_ui_state::sketch_pt(&*sk.project, si, a), qymcad_ui_state::sketch_pt(&*sk.project, si, b)) {
                (Some(pa), Some(pb)) => {
                    let m = sh.at(Point2::new((pa.x + pb.x) / 2.0, (pa.y + pb.y) / 2.0)) - sc;
                    if m.length() > 1e-3 { m.normalized() } else { egui::vec2(1.0, 0.0) }
                }
                _ => egui::vec2(1.0, 0.0),
            }
        } else {
            egui::vec2(1.0, 0.0)
        };
        let label_at = sc + dir * (r_px + 14.0);
        // a radius mark is a LABEL: 15 px in place became the Label role (12 px at normal precision)
        if label_at.distance(pos) <= qymcad_ui_state::grab::grab(sk.set, Grab::Label) {
            return Some((center, diam));
        }
    }
    None
}

pub fn power_trim_path_test(sk: &mut qymcad_ui_state::SketchCtx, rect: Rect, a: Pos2, b: Pos2) -> usize {
    sk.trim.path.clear();
    sk.trim.done.clear();
    power_trim_sweep(sk, rect, a, b)
}

/// A test facade: CONTINUE the same drag (the memory of the spans already trimmed is kept).
pub fn power_trim_path_test_continue(sk: &mut qymcad_ui_state::SketchCtx, rect: Rect, a: Pos2, b: Pos2) -> usize {
    power_trim_sweep(sk, rect, a, b)
}

pub fn sketch_drag_update(sk: &mut qymcad_ui_state::SketchCtx, ctx: &egui::Context, resp: &egui::Response, rect: Rect) {
    let sh = qymcad_ui_state::Sheet { view: *sk.view, rect: rect };
            if let Some(ti) = sk.drag.text() {
                // moving a text object (it shifts the parameters, the baked glyphs and the contours)
                if resp.dragged() {
                    if let qymcad_ui_state::Sel::Sketch(si) = *sk.sel {
                        let d = resp.drag_delta();
                        let (wx, wy) = (d.x as f64 / sk.view.scale as f64, -d.y as f64 / sk.view.scale as f64);
                        sk.project.move_sketch_text(si, ti, wx, wy);
                        qymcad_ui_state::invalidate(&mut *sk.regen);
                    }
                }
                if resp.drag_stopped() {
                    sk.drag.clear();
                }
            } else if let Some(ni) = sk.drag.note() {
                // moving a note
                if resp.dragged() {
                    if let qymcad_ui_state::Sel::Sketch(si) = *sk.sel {
                        let d = resp.drag_delta();
                        let (wx, wy) = (d.x as f64 / sk.view.scale as f64, -d.y as f64 / sk.view.scale as f64);
                        if let Some(n) = sk.project.sketches.get_mut(si).and_then(|s| s.notes.get_mut(ni)) {
                            n.x += wx;
                            n.y += wy;
                        }
                    }
                }
                if resp.drag_stopped() {
                    sk.drag.clear();
                }
            } else if let Some(ci) = sk.drag.dim() {
                // offsetting a dimension line (editing `off`, with no recomputation of the geometry), with
                // THE AXIS taken into account: a horizontal dimension offsets along screen Y, a vertical one
                // along X, an aligned one or a point-to-line along the perpendicular.
                if resp.dragged() {
                    if let qymcad_ui_state::Sel::Sketch(si) = *sk.sel {
                        use qymcad_core::model::Constraint;
                        // the diameter or radius label travels AROUND THE CIRCLE - `off` is the absolute
                        // angle of the leader (in radians) from the centre to the cursor. It is set directly
                        // rather than by a delta, so the label sticks to the pointer.
                        if let Some(Constraint::Diameter { c, .. }) = sk.project.sketches[si].constraints.get(ci).cloned() {
                            if let (Some(cp), Some(pp)) = (qymcad_ui_state::sketch_pt(&*sk.project, si, c), resp.interact_pointer_pos()) {
                                let sc = sh.at(cp);
                                let ang = (pp.y - sc.y).atan2(pp.x - sc.x) as f64;
                                if let Some(Constraint::Diameter { off, .. }) = sk.project.sketches[si].constraints.get_mut(ci) {
                                    *off = ang;
                                }
                            }
                            qymcad_ui_state::invalidate(&mut *sk.regen);
                        } else {
                        let dl = resp.drag_delta();
                        let dadd = match sk.project.sketches[si].constraints.get(ci).cloned() {
                            Some(Constraint::Distance { a, b, axis, .. }) => match axis {
                                1 => dl.y as f64,
                                2 => dl.x as f64,
                                _ => {
                                    if let (Some(pa), Some(pb)) = (qymcad_ui_state::sketch_pt(&*sk.project, si, a), qymcad_ui_state::sketch_pt(&*sk.project, si, b)) {
                                        let (sa, sb) = (sh.at(pa), sh.at(pb));
                                        let dir = (sb - sa).normalized();
                                        let perp = egui::vec2(-dir.y, dir.x);
                                        (dl.x * perp.x + dl.y * perp.y) as f64
                                    } else {
                                        0.0
                                    }
                                }
                            },
                            Some(Constraint::DistancePL { a, b, .. }) => {
                                // the dimension line slides ALONG the line (ab) rather than across it - that
                                // is what lets it be raised or lowered over the geometry instead of merely
                                // moved nearer or further.
                                if let Some(ab) = qymcad_ui_state::line_screen_dir(&*sk.project, &*sk.view, si, a, b, rect) {
                                    (dl.x * ab.x + dl.y * ab.y) as f64
                                } else {
                                    0.0
                                }
                            }
                            // an arc length: `off` is the vertical screen shift of the caption (see the drawing)
                            Some(Constraint::ArcLength { .. }) => dl.y as f64,
                            // a tangent distance: `off` runs along the perpendicular to the line of centres c1-c2
                            Some(Constraint::EdgeDistance { c1, c2, .. }) => {
                                if let (Some(p1), Some(p2)) = (qymcad_ui_state::sketch_pt(&*sk.project, si, c1), qymcad_ui_state::sketch_pt(&*sk.project, si, c2)) {
                                    let dir = (sh.at(p2) - sh.at(p1)).normalized();
                                    let perp = egui::vec2(-dir.y, dir.x);
                                    (dl.x * perp.x + dl.y * perp.y) as f64
                                } else {
                                    0.0
                                }
                            }
                            _ => 0.0,
                        };
                        let dadd = dadd / sk.view.scale as f64; // a screen delta becomes WORLD units, so the offset scales with the zoom
                        if let Some(Constraint::Distance { off, .. }) | Some(Constraint::DistancePL { off, .. }) | Some(Constraint::ArcLength { off, .. }) | Some(Constraint::EdgeDistance { off, .. }) = sk.project.sketches[si].constraints.get_mut(ci) {
                            *off += dadd;
                        }
                        }
                    }
                }
                if resp.drag_stopped() {
                    sk.drag.clear();
                }
            } else if let Some((si, spi, ki)) = sk.drag.handle() {
                // dragging a tangent handle of a spline changes its shape (the tangent becomes explicit)
                if resp.dragged() {
                    if let Some(pp) = resp.interact_pointer_pos() {
                        let w = qymcad_ui_state::to_world(&*sk.view, rect, pp);
                        sk.project.set_spline_handle(si, spi, ki, w.x, w.y);
                        qymcad_ui_state::invalidate(&mut *sk.regen);
                    }
                }
                if resp.drag_stopped() {
                    sk.drag.clear();
                }
            } else if let Some((si, pi)) = sk.drag.pt() {
                // A DRAG IS ONE OPERATION: it opens on the first frame of the drag and closes on the
                // release. The intermediate frames do not enter the step - an undo returns the point to
                // where it was BEFORE the drag rather than to the previous frame.
                if resp.drag_started() {
                    qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, qymcad_i18n::tr("status-move-point"));
                }
                if resp.dragged() {
                    if let Some(pp) = resp.interact_pointer_pos() {
                        drag_point_to(sk, si, pi, rect, pp);
                    }
                }
                if resp.drag_stopped() {
                    finish_point_drag(sk);
                }
            } else if let Some((si, ids)) = sk.drag.mov() {
                // moving the selected geometry as a whole: all of its points are shifted, then the
                // constraints are solved
                if resp.dragged() {
                    let d = resp.drag_delta();
                    let (dx, dy) = (d.x as f64 / sk.view.scale as f64, -d.y as f64 / sk.view.scale as f64);
                    if let Some(s) = sk.project.sketches.get_mut(si) {
                        for p in s.points.iter_mut() {
                            if ids.contains(&p.id) {
                                p.x += dx;
                                p.y += dy;
                            }
                        }
                    }
                    sk.project.solve_sketch_drag_fast(si, None); // a frame of the move takes the fast path
                    qymcad_ui_state::invalidate(&mut *sk.regen);
                }
                if resp.drag_stopped() {
                    sk.drag.clear();
                    sk.project.solve_sketch(si); // the final full solve, including evaluating the parameters
                    qymcad_ui_state::invalidate(&mut *sk.regen);
                }
            } else if sk.body_giz.dragging {
                if resp.dragged() {
                    let d = resp.drag_delta();
                    translate_selected(sk, d.x as f64 / sk.view.scale as f64, -d.y as f64 / sk.view.scale as f64);
                }
                if resp.drag_stopped() {
                    sk.body_giz.dragging = false;
                }
            } else if sk.tree_sel.box_start.is_some() {
                if resp.drag_stopped() {
                    if let (Some(a), Some(b)) = (sk.tree_sel.box_start, resp.interact_pointer_pos()) {
                        // without Shift the box REPLACES the selection; with Shift it adds to it
                        if !ctx.input(|i| i.modifiers.shift) && matches!(*sk.sel, qymcad_ui_state::Sel::Sketch(_)) {
                            sk.sel_sk.clear(); // the selection and whatever was waiting for it
                        }
                        qymcad_ui_state::box_select(qymcad_ui_state::editing_in!(sk), &mut *sk.sel_sk, *sk.sketch_ses, rect, a, b);
                    }
                    sk.tree_sel.box_start = None;
                }
            } else if resp.dragged_by(egui::PointerButton::Primary) {
                // the fallback pan is for the left button only - a middle drag is served by the EXPLICIT
                // handler below (otherwise the middle button would pan twice, at double speed)
                let d = resp.drag_delta();
                sk.view.center.x -= d.x / sk.view.scale;
                sk.view.center.y += d.y / sk.view.scale;
            }
}

/// DRAG THE TAKEN POINT to where the cursor is - one frame of a drag.
pub fn drag_point_to(sk: &mut qymcad_ui_state::SketchCtx, si: usize, pi: usize, rect: Rect, pp: Pos2) {
    let w = snap_world(sk, rect, pp);
    // the point is pinned to the cursor: it follows the mouse while defined geometry resists (the solver
    // runs with a strong drag residual).
    let pid = sk.project.sketches.get(si).and_then(|s| s.points.get(pi)).map(|p| p.id);
    if let Some(id) = pid {
        // a drag frame takes the fast path (no re-evaluation of the parameters, a reduced iteration
        // budget). The full solve happens on release.
        sk.project.solve_sketch_drag_fast(si, Some((id, w.x, w.y)));
    }
    qymcad_ui_state::invalidate(&mut *sk.regen);
}

/// AN EVENT BECOMES A POINT (the same split as for a click in 3D).
pub fn sketch_click(sk: &mut qymcad_ui_state::SketchCtx, ctx: &egui::Context, resp: &egui::Response, rect: Rect) {
    if !resp.clicked() {
        return;
    }
    let Some(pos) = resp.interact_pointer_pos() else { return };
    sketch_click_at(sk, ctx, pos, rect);
}

/// AN ACTION AT A POINT on the sketch canvas - the same thing the mouse does, but with no `egui` event.
pub fn sketch_click_at(sk: &mut qymcad_ui_state::SketchCtx, ctx: &egui::Context, pos: egui::Pos2, rect: Rect) {
    {
        {
                    if sk.picking.fillet_all() {
                        // a click on a shape follows the connected chain and opens the radius popup
                        if let qymcad_ui_state::Sel::Sketch(si) = *sk.sel {
                            if let Some(eid) = qymcad_ui_state::entity_near(&sk.pick(), rect, pos, si) {
                                let comp = sk.project.connected_entities(si, eid);
                                sk.corner.at = Some((si, 0, false));
                                sk.corner.only = Some(comp);
                                sk.corner.pos = Some(pos);
                                sk.corner.buf = qymcad_core::expr::fmt_num(sk.tool_prefs.fillet);
                                sk.corner.focus = true;
                                sk.picking.clear();
                            } else {
                                *sk.status = qymcad_i18n::tr("sk-click-shape-line");
                            }
                        }
                    } else if sk.armed.draw_kind() != 0 {
                        sketch_tool_click(sk, rect, pos);
                    } else if sk.armed.dim_kind() != 0 {
                        dim_click(sk, rect, pos);
                    } else if sk.armed.measuring() {
                        let w = snap_world(sk, rect, pos);
                        if sk.measure.pts.len() >= 2 {
                            sk.measure.pts.clear(); // a new measurement
                        }
                        sk.measure.pts.push(w);
                        if sk.measure.pts.len() == 2 {
                            let (a, b) = (sk.measure.pts[0], sk.measure.pts[1]);
                            let d = ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt();
                            *sk.status = qymcad_i18n::trn("sk-distance-dxdy", &[("d", &qymcad_i18n::num(d, 3)), ("dx", &qymcad_i18n::num(b.x - a.x, 3)), ("dy", &qymcad_i18n::num(b.y - a.y, 3))]);
                        }
                    } else if sk.pending_import.draw_pts.is_some() {
                        let w = snap_world(sk, rect, pos);
                        if let Some(pts) = sk.pending_import.draw_pts.as_mut() {
                            pts.push(w);
                        }
                    } else if sk.armed.click_op() == 6 {
                        // PROJECT THE GEOMETRY OF A BODY: a click on an edge of the underlay takes it into
                        // the sketch as a driven entity. The underlay was drawn before as well - but only as
                        // a picture: it could be snapped to and not taken as geometry.
                        if let qymcad_ui_state::Sel::Sketch(si) = *sk.sel {
                            project_clicked_edge(sk, si, rect, pos);
                        }
                    } else if sk.armed.click_op() == 4 || sk.armed.click_op() == 5 {
                        // a click on a corner opens the RADIUS or LEG popup, and it applies only on Enter or
                        // the tick (a default of 3 mm used to be applied silently, and on a small part that
                        // failed)
                        if let qymcad_ui_state::Sel::Sketch(si) = *sk.sel {
                            if let Some(pid) = qymcad_pick::nearest_vertex(&sk.pick(), rect, pos, si) {
                                sk.corner.at = Some((si, pid, sk.armed.click_op() == 5));
                                sk.corner.pos = Some(pos);
                                sk.corner.buf = qymcad_core::expr::fmt_num(sk.tool_prefs.fillet);
                                sk.corner.focus = true;
                            } else {
                                *sk.status = qymcad_i18n::tr("sk-click-corner");
                            }
                        }
                    } else if sk.armed.click_op() != 0 {
                        // trimming, extending or breaking by click: a line first, then a circle or an arc
                        if let qymcad_ui_state::Sel::Sketch(si) = *sk.sel {
                            let w = qymcad_ui_state::to_world(&*sk.view, rect, pos);
                            let line_eid = qymcad_pick::nearest_line_eid(&sk.pick(), rect, pos, si);
                            let ok = if let Some(eid) = line_eid {
                                match sk.armed.click_op() {
                                    1 => sk.project.trim_line(si, eid, w.x, w.y),
                                    2 => sk.project.extend_line(si, eid, w.x, w.y),
                                    3 => sk.project.break_line(si, eid, w.x, w.y),
                                    _ => false,
                                }
                            } else {
                                false
                            };
                            // not a line (or the line did not work) - try a circle or an arc
                            let ok = ok
                                || qymcad_pick::nearest_circle_entity(&sk.pick(), rect, pos, si).is_some_and(|eid| match sk.armed.click_op() {
                                    1 => sk.project.trim_curve(si, eid, w.x, w.y),
                                    2 => sk.project.extend_curve(si, eid, w.x, w.y),
                                    3 => sk.project.break_curve(si, eid, w.x, w.y),
                                    _ => false,
                                });
                            if ok {
                                sk.sel_sk.clear(); // the selection and whatever was waiting for it
                                qymcad_ui_state::invalidate(&mut *sk.regen);
                                *sk.status = qymcad_i18n::tr("sk-done");
                            } else if line_eid.is_none() && qymcad_pick::nearest_circle_entity(&sk.pick(), rect, pos, si).is_none() {
                                *sk.status = qymcad_i18n::tr("sk-click-curve");
                            } else {
                                *sk.status = qymcad_i18n::tr("sk-op-failed-no-intersection");
                            }
                        }
                    } else if sk.clip.geom_pending.is_some() && matches!(*sk.sel, qymcad_ui_state::Sel::Sketch(_)) {
                        // a click on THE ANCHOR point takes the geometry into the buffer (on a cut, the
                        // source is removed)
                        let qymcad_ui_state::Sel::Sketch(si) = *sk.sel else { return }; // the selection may have changed between frames - do not crash
                        let w = snap_world(sk, rect, pos);
                        // it should not be empty, but the program must not crash over that
                        let Some((eids, cut)) = sk.clip.geom_pending.take() else { return };
                        let clip = sk.project.copy_sketch_geometry(si, &eids, w.x, w.y);
                        if cut {
                            sk.project.delete_entities(si, &eids);
                            sk.project.solve_sketch(si);
                            qymcad_ui_state::invalidate(&mut *sk.regen);
                        }
                        // the anchor point has been clicked, so the selection is cleared - visually the copy
                        // is finished
                        sk.sel_sk.clear(); // the selection and whatever was waiting for it
                        let n = clip.entities.len();
                        sk.clip.geom = Some(clip);
                        *sk.status = qymcad_i18n::tr2("sk-clipboard", "what", &if cut { qymcad_i18n::tr("sk-cut-done") } else { qymcad_i18n::tr("sk-copied") }, "n", &n.to_string());
                    } else if sk.clip.geom_place && matches!(*sk.sel, qymcad_ui_state::Sel::Sketch(_)) {
                        // a placement click pastes the buffer so that the anchor lands on the clicked point
                        let qymcad_ui_state::Sel::Sketch(si) = *sk.sel else { return }; // the selection may have changed between frames - do not crash
                        let w = snap_world(sk, rect, pos);
                        if let Some(clip) = sk.clip.geom.clone() {
                            let ids = sk.project.paste_sketch_geometry(si, &clip, w.x, w.y);
                            sk.project.solve_sketch(si);
                            sk.sel_sk.items = ids.into_iter().map(|id| (1u8, id)).collect();
                            qymcad_ui_state::invalidate(&mut *sk.regen);
                            *sk.status = qymcad_i18n::tr("sk-pasted");
                        }
                        sk.clip.geom_place = false;
                    } else if sk.armed.pat_op() != 0 && matches!(*sk.sel, qymcad_ui_state::Sel::Sketch(_)) {
                        // an array: pick the entities, then (for a circular one) click THE CENTRE of
                        // rotation, then Enter
                        let shift = ctx.input(|i| i.modifiers.shift);
                        let has_sel = sk.sel_sk.items.iter().any(|(k, _)| *k == 1);
                        if !has_sel {
                            sketch_select_click(sk, rect, pos, shift);
                            *sk.status = if sk.sel_sk.items.iter().any(|(k, _)| *k == 1) {
                                if sk.armed.pat_op() == 2 { qymcad_i18n::tr("sk-click-rot-centre") } else { qymcad_i18n::tr("sk-params-above") }
                            } else {
                                qymcad_i18n::tr("sk-click-for-array")
                            };
                        } else if shift {
                            // Shift continues picking the source
                            sketch_select_click(sk, rect, pos, true);
                        } else if sk.armed.pat_op() == 2 {
                            // circular: a click sets or moves the centre, snapping to an intersection or a vertex
                            sk.pat.center = Some(snap_world(sk, rect, pos));
                            *sk.status = qymcad_i18n::tr("sk-centre-set-params");
                        }
                    } else if sk.armed.move_op() != 0 && matches!(*sk.sel, qymcad_ui_state::Sel::Sketch(_)) {
                        // an interactive move or copy: the selection, then the base point, then the target
                        let qymcad_ui_state::Sel::Sketch(si) = *sk.sel else { return }; // the selection may have changed between frames - do not crash
                        let w = snap_world(sk, rect, pos);
                        let eids: Vec<Id> = sk.sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
                        if eids.is_empty() {
                            let shift = ctx.input(|i| i.modifiers.shift);
                            sketch_select_click(sk, rect, pos, shift);
                            if !sk.sel_sk.items.iter().any(|(k, _)| *k == 1) {
                                *sk.status = qymcad_i18n::tr("sk-click-for-move");
                            } else {
                                *sk.status = qymcad_i18n::tr("sk-click-base-point");
                            }
                        } else if sk.tool.move_base.is_none() {
                            sk.tool.move_base = Some(w);
                            if sk.armed.move_op() == 3 {
                                // the centre is set, so the angle is typed in the popup - no target click is
                                // awaited
                                sk.rot.angle = 0.0;
                                sk.rot.buf = "0".into();
                                sk.rot.focus = true;
                                *sk.status = qymcad_i18n::tr("sk-centre-set-angle");
                            } else {
                                *sk.status = qymcad_i18n::tr("sk-click-target-point");
                            }
                        } else if sk.armed.move_op() != 3 {
                            let Some(base) = sk.tool.move_base.take() else { return };
                            let (dx, dy) = (w.x - base.x, w.y - base.y);
                            if sk.armed.move_op() == 2 {
                                let ids = sk.project.copy_entities(si, &eids, dx, dy);
                                sk.sel_sk.items = ids.into_iter().map(|id| (1u8, id)).collect(); // select the copies
                                *sk.status = qymcad_i18n::tr("sk-copied");
                            } else {
                                sk.project.move_entities(si, &eids, dx, dy);
                                *sk.status = qymcad_i18n::tr("sk-moved");
                            }
                            *sk.armed = qymcad_ui_state::Armed::None;
                            qymcad_ui_state::invalidate(&mut *sk.regen);
                        }
                    } else if sk.armed.cmd_kind() == 3 && sk.rev.pick_line {
                        // the axis of revolution is picked BY CLICKING a line of the sketch. It used to be
                        // chosen from a list of "Line 1 / Line 2 / Line 3", where a number tells nothing
                        // about which one is wanted.
                        let si = sk.cmd.sketch.unwrap_or(0);
                        let cands = qymcad_ui_state::profile_axis_lines(&*sk.project, si);
                        match qymcad_pick::nearest_line_id(&sk.pick(), rect, pos, si, &cands) {
                            Some(eid) => {
                                sk.rev.axis_line = eid;
                                sk.rev.axis_datum = 0;
                                sk.rev.pick_line = false;
                                *sk.mode_3d = true;
                                sk.view.initialized = false;
                                let n = cands.iter().position(|l| *l == eid).map(|i| i + 1).unwrap_or(1);
                                *sk.status = format!("{} {}", ph::CHECK, qymcad_i18n::tr1("g-rev-axis", "what", &qymcad_ui_state::axis_line_label(&*sk.project, si, eid, n)));
                            }
                            None => *sk.status = qymcad_i18n::tr("sk-miss-line"),
                        }
                    } else if let Some(slot) = sk.picking.contour() {
                        // picking the contour for a sweep or loft slot through the half-sketcher: a click on
                        // a contour fills the slot and returns to 3D (as in Extrude, but a single pick into a
                        // particular slot).
                        let cands = qymcad_ui_state::slot_candidates(&*sk.loft, &*sk.project, *sk.sweep, slot);
                        if let Some(cid) = qymcad_ui_state::slot_contour_under_2d(&sk.pick(), rect, pos, &cands) {
                            qymcad_ui_state::set_contour_slot(&mut *sk.loft, &mut *sk.sweep, slot, cid);
                            sk.picking.clear();
                            *sk.mode_3d = true;
                            sk.view.initialized = false;
                            *sk.status = qymcad_i18n::tr("sk-contour-picked");
                        } else {
                            *sk.status = qymcad_i18n::tr("sk-miss-contour");
                        }
                    } else if sk.armed.commanding() {
                        // a click on a contour ALWAYS adds to the selection (as Ctrl used to): a single click
                        // does NOT leave the profile-picking mode, and only Enter moves on to the dimension.
                        // A miss (a click past every contour) does NOT clear the set - otherwise an accidental
                        // near-miss wiped every profile gathered so far; clearing happens only on Esc or on a
                        // repeated click.
                        if let Some(si) = sk.cmd.sketch {
                            if let Some(cid) = qymcad_ui_state::contour_under_2d(&*sk.project, &*sk.view, rect, pos, si) {
                                if !sk.gsel.profiles.remove(&cid) {
                                    sk.gsel.profiles.insert(cid); // a repeated click on a contour deselects it
                                }
                                *sk.status = qymcad_i18n::tr1("sk-profiles-n", "n", &sk.gsel.profiles.len().to_string());
                            }
                        }
                    } else if *sk.workbench == qymcad_ui_state::Workbench::Sketch && matches!(*sk.sel, qymcad_ui_state::Sel::Sketch(_)) {
                        // the sketch workbench: a click picks a point or an entity (Shift adds to the selection)
                        let shift = ctx.input(|i| i.modifiers.shift);
                        sketch_select_click(sk, rect, pos, shift);
                    } else {
                        qymcad_pick::pick_contour(&*sk.project, &mut *sk.sel, sk.set, *sk.view, rect, pos);
                    }
                }
            }
}

/// Snapping the cursor: a vertex or a centre (always), then a midpoint, an intersection, a point on an
/// edge, an axis, a grid node. It sets `snap_hint` (the type for the glyph) and returns a world point.
/// The type codes: 0 vertex, 1 grid, 2 axis, 3 midpoint, 4 centre, 5 intersection, 6 on an edge.
pub fn snap_world(sk: &mut qymcad_ui_state::SketchCtx, rect: Rect, screen: Pos2) -> Point2 {
    let sh = qymcad_ui_state::Sheet { view: *sk.view, rect };
    let w = qymcad_ui_state::to_world(&*sk.view, rect, screen);
    let sd = |p: Point2, view: &qymcad_ui_state::View2d| qymcad_ui_state::Sheet { view: *view, rect }.at(p).distance(screen);
    // the centres of the circles and arcs of the active sketch, so a centre can be told from a plain vertex
    let centers: std::collections::HashSet<u64> = qymcad_ui_state::edit_si(&*sk.project, &*sk.sketch_ses)
        .and_then(|si| sk.project.sketches.get(si))
        .map(|s| {
            s.entities
                .iter()
                .filter_map(|e| match e.kind {
                    qymcad_core::model::EntityKind::Circle { center, .. } | qymcad_core::model::EntityKind::Arc { center, .. } => Some(center),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    // THE VERTICES of existing geometry snap ALWAYS - that is topology. Type 4 marks the centres.
    // THE ACTIVE SKETCH ONLY: every sketch and every contour of the project used to be walked, so the
    // cursor stuck to the vertices of other people's sketches (on different planes the 2D coordinates
    // coincide) and to the tessellation of every circle in the project - random snaps to the wrong place
    // plus a cost of O(the whole model) per frame. The reference geometry of neighbours and of the host
    // face arrives separately, below, through `qymcad_pick::sketch_ref_edges_2d`, already projected into the sketch.
    let mut best: Option<(f32, Point2, u8)> = None;
    if let Some(asi) = qymcad_ui_state::edit_si(&*sk.project, &*sk.sketch_ses) {
        if let Some(s) = sk.project.sketches.get(asi) {
            for sp in &s.points {
                let p = Point2::new(sp.x, sp.y);
                let d = sd(p, &*sk.view);
                let ty = if centers.contains(&sp.id) { 4 } else { 0 };
                if best.is_none_or(|(bd, _, _)| d < bd) {
                    best = Some((d, p, ty));
                }
            }
            for cid in &s.contour_ids {
                let Some(ci) = sk.project.contour_index(*cid) else { continue };
                for p in &sk.project.contours[ci].points {
                    let d = sd(*p, &*sk.view);
                    if best.is_none_or(|(bd, _, _)| d < bd) {
                        best = Some((d, *p, 0));
                    }
                }
            }
        }
    }
    // the edges of the REFERENCE body (a face of its own part, or a neighbour during the creation
    // session), projected into the sketch. THE ENDS of those edges (the corners of the outline) are
    // VERTICES (type 0, taking priority). The nearest point ON an edge and the INTERSECTIONS with it come
    // below, in the inference: a point on an edge used to stand as a vertex and SHORT-CIRCUITED the rest,
    // making it impossible to snap to the intersection of a construction line with the outline of a part.
    let ref_edges = qymcad_ui_state::edit_si(&*sk.project, &*sk.sketch_ses).map(|si| qymcad_pick::sketch_ref_edges_2d(&*sk.cache, &*sk.cmd, &*sk.live, &*sk.project, &*sk.regen, si)).unwrap_or_default();
    for poly in &ref_edges {
        for end in [poly.first(), poly.last()].into_iter().flatten() {
            let d = sd(*end, &*sk.view);
            if best.is_none_or(|(bd, _, _)| d < bd) {
                best = Some((d, *end, 0));
            }
        }
    }
    if let Some((d, p, ty)) = best {
        if d <= qymcad_ui_state::grab::grab(sk.set, Grab::Snap) {
            *sk.snap_hint = Some((p, ty));
            return p;
        }
    }
    if !sk.set.snap.on {
        *sk.snap_hint = None;
        return w;
    }
    // the inference snaps of the active sketch: a midpoint, then an intersection, then a point on an edge.
    // `cand` holds (the screen distance, the point, the type); a smaller (type, distance) wins.
    let mut cand: Option<(f32, Point2, u8)> = None;
    if let Some(si) = qymcad_ui_state::edit_si(&*sk.project, &*sk.sketch_ses) {
        let qymcad_ui_state::ActiveEdges { lines, circles: circs } = qymcad_ui_state::active_edges(&sk.draw(), si);
        // the segments of the projected outlines of the reference body, used for INTERSECTIONS with the
        // sketch lines and for points on an edge. That is how the intersection of a construction line with
        // a face or the outline of a part becomes snappable.
        let ref_segs: Vec<(Point2, Point2)> = ref_edges.iter().flat_map(|poly| poly.windows(2).map(|s| (s[0], s[1]))).collect();
        // the priority: a midpoint (3) over an intersection (5) over a point on an edge (6)
        // 1) the midpoints of segments (SKETCH lines only - the midpoints of a tessellated outline are noise)
        for (a, b) in &lines {
            let m = Point2::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
            let d = sd(m, &*sk.view);
            if d <= qymcad_ui_state::grab::grab(sk.set, Grab::Snap) && cand.is_none_or(|(bd, _, bty)| (3u8, d) < (bty, bd)) {
                cand = Some((d, m, 3));
            }
        }
        // 2) intersections: sketch with sketch, sketch with a circle, AND sketch with THE OUTLINE OF A PART
        // (a construction line against a face)
        for i in 0..lines.len() {
            for j in (i + 1)..lines.len() {
                if let Some(p) = qymcad_ui_state::seg_seg_intersect(lines[i].0, lines[i].1, lines[j].0, lines[j].1) {
                    let d = sd(p, &*sk.view);
                    if d <= qymcad_ui_state::grab::grab(sk.set, Grab::Snap) && cand.is_none_or(|(bd, _, bty)| (5u8, d) < (bty, bd)) {
                        cand = Some((d, p, 5));
                    }
                }
            }
            for (c, r) in &circs {
                for p in qymcad_ui_state::seg_circle_intersect(lines[i].0, lines[i].1, *c, *r) {
                    let d = sd(p, &*sk.view);
                    if d <= qymcad_ui_state::grab::grab(sk.set, Grab::Snap) && cand.is_none_or(|(bd, _, bty)| (5u8, d) < (bty, bd)) {
                        cand = Some((d, p, 5));
                    }
                }
            }
            for (ra, rb) in &ref_segs {
                if let Some(p) = qymcad_ui_state::seg_seg_intersect(lines[i].0, lines[i].1, *ra, *rb) {
                    let d = sd(p, &*sk.view);
                    if d <= qymcad_ui_state::grab::grab(sk.set, Grab::Snap) && cand.is_none_or(|(bd, _, bty)| (5u8, d) < (bty, bd)) {
                        cand = Some((d, p, 5));
                    }
                }
            }
        }
        // 3) a point on an edge (the cursor projected onto a sketch segment, THE OUTLINE OF A PART or a
        // circle) plus A TIE TO THE GRID ALONG the face (face against grid, type 5, which outranks a plain
        // point on an edge): across the face it sticks to the face, along it to the nearest grid line, so
        // the intersection of a face and the grid can be hit exactly.
        let gsz = sk.set.snap.grid.max(0.1);
        for (a, b) in lines.iter().chain(ref_segs.iter()) {
            if let Some(p) = qymcad_ui_state::project_on_seg(w, *a, *b) {
                if let Some(gpt) = qymcad_ui_state::grid_cross_on_seg(p, *a, *b, gsz) {
                    let d = sd(gpt, &*sk.view);
                    if d <= qymcad_ui_state::grab::grab(sk.set, Grab::Snap) && cand.is_none_or(|(bd, _, bty)| (5u8, d) < (bty, bd)) {
                        cand = Some((d, gpt, 5));
                    }
                }
                let d = sd(p, &*sk.view);
                if d <= qymcad_ui_state::grab::grab(sk.set, Grab::Snap) && cand.is_none_or(|(bd, _, bty)| (6u8, d) < (bty, bd)) {
                    cand = Some((d, p, 6));
                }
            }
        }
        for (c, r) in &circs {
            let dir = Point2::new(w.x - c.x, w.y - c.y);
            let len = (dir.x * dir.x + dir.y * dir.y).sqrt();
            if len > 1e-9 {
                let p = Point2::new(c.x + dir.x / len * r, c.y + dir.y / len * r);
                let d = sd(p, &*sk.view);
                if d <= qymcad_ui_state::grab::grab(sk.set, Grab::Snap) && cand.is_none_or(|(bd, _, bty)| (6u8, d) < (bty, bd)) {
                    cand = Some((d, p, 6));
                }
            }
        }
    }
    if let Some((_, p, ty)) = cand {
        *sk.snap_hint = Some((p, ty));
        return p;
    }
    // snapping to the X = 0 and Y = 0 axes and to the origin. The free coordinate ALONG the axis is still
    // pulled towards the grid nodes (otherwise snapping by cells disappeared along an axis).
    let g = sk.set.snap.grid.max(0.1);
    let snap_to_grid = |v: f64| (v / g).round() * g;
    let near_x0 = (sh.at(Point2::new(0.0, w.y)).x - screen.x).abs() <= qymcad_ui_state::grab::grab(sk.set, Grab::Snap);
    let near_y0 = (sh.at(Point2::new(w.x, 0.0)).y - screen.y).abs() <= qymcad_ui_state::grab::grab(sk.set, Grab::Snap);
    if near_x0 || near_y0 {
        // the coordinate ACROSS the axis becomes 0; along the axis it goes to the grid, if a node is near
        // on screen
        let gx = snap_to_grid(w.x);
        let gy = snap_to_grid(w.y);
        let ax = if near_x0 { 0.0 } else if (sh.at(Point2::new(gx, w.y)).x - screen.x).abs() <= qymcad_ui_state::grab::grab(sk.set, Grab::Snap) { gx } else { w.x };
        let ay = if near_y0 { 0.0 } else if (sh.at(Point2::new(w.x, gy)).y - screen.y).abs() <= qymcad_ui_state::grab::grab(sk.set, Grab::Snap) { gy } else { w.y };
        let ap = Point2::new(ax, ay);
        *sk.snap_hint = Some((ap, 2)); // an axis
        return ap;
    }
    // a grid node
    let gp = Point2::new(snap_to_grid(w.x), snap_to_grid(w.y));
    if sh.at(gp).distance(screen) <= qymcad_ui_state::grab::grab(sk.set, Grab::Snap) {
        *sk.snap_hint = Some((gp, 1)); // the grid
        return gp;
    }
    *sk.snap_hint = None;
    w
}

/// Create a dimension BETWEEN two references (a point or a line) and let it follow the cursor until it is
/// placed.
pub fn make_between_dim(sk: &mut qymcad_ui_state::SketchCtx, si: usize, r1: qymcad_ui_state::DimRef, r2: qymcad_ui_state::DimRef) {
    use qymcad_core::model::Constraint;
    let pt = |project: &qymcad_core::model::Project, id: Id| qymcad_ui_state::sketch_pt(project, si, id);
    // the perpendicular distance from point p to the line a-b
    // a SIGNED perpendicular distance (the side matters: the point must not mirror to the other side
    // during the solve)
    let perp = |project: &qymcad_core::model::Project, p: Id, a: Id, b: Id| -> f64 {
        match (pt(project, p), pt(project, a), pt(project, b)) {
            (Some(pp), Some(pa), Some(pb)) => {
                let (dx, dy) = (pb.x - pa.x, pb.y - pa.y);
                let len = (dx * dx + dy * dy).sqrt().max(1e-9);
                (dx * (pp.y - pa.y) - dy * (pp.x - pa.x)) / len
            }
            _ => 0.0,
        }
    };
    let c: Option<Constraint> = match (r1, r2) {
        (qymcad_ui_state::DimRef::Point(p1), qymcad_ui_state::DimRef::Point(p2)) => {
            let d = match (pt(&*sk.project, p1), pt(&*sk.project, p2)) {
                (Some(a), Some(b)) => ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt(),
                _ => 0.0,
            };
            Some(Constraint::Distance { a: p1, b: p2, d, off: 0.0, expr: String::new(), driven: false, axis: 0 })
        }
        (qymcad_ui_state::DimRef::Point(p), qymcad_ui_state::DimRef::Line(a, b)) | (qymcad_ui_state::DimRef::Line(a, b), qymcad_ui_state::DimRef::Point(p)) => {
            let d = perp(&*sk.project, p, a, b);
            Some(Constraint::DistancePL { p, a, b, d, off: 0.0, expr: String::new(), driven: false })
        }
        (qymcad_ui_state::DimRef::Line(a1, b1), qymcad_ui_state::DimRef::Line(a2, b2)) => {
            let (ax1, ax2) = (qymcad_ui_state::is_axis_line(&*sk.project, si, a1, b1), qymcad_ui_state::is_axis_line(&*sk.project, si, a2, b2));
            if ax1 ^ ax2 {
                // when ONE of the lines is a coordinate axis, the result is ALWAYS a distance (the position
                // of the line or its end relative to the axis) and NOT an angle. A line perpendicular to an
                // axis used to give 90 degrees or a driven 0.0. The end of the geometry FURTHER from the
                // axis is taken, so the value is not a degenerate zero.
                let (ga, gb, axa, axb) = if ax2 { (a1, b1, a2, b2) } else { (a2, b2, a1, b1) };
                let (da, db) = (perp(&*sk.project, ga, axa, axb), perp(&*sk.project, gb, axa, axb));
                let (p, d) = if db.abs() > da.abs() { (gb, db) } else { (ga, da) };
                Some(Constraint::DistancePL { p, a: axa, b: axb, d, off: 0.0, expr: String::new(), driven: false })
            } else {
                // parallel? then it is the distance between the lines (a perpendicular from the end of one
                // to the other)
                let (d1, d2) = match (pt(&*sk.project, a1), pt(&*sk.project, b1), pt(&*sk.project, a2), pt(&*sk.project, b2)) {
                    (Some(pa1), Some(pb1), Some(pa2), Some(pb2)) => ((pb1.x - pa1.x, pb1.y - pa1.y), (pb2.x - pa2.x, pb2.y - pa2.y)),
                    _ => ((1.0, 0.0), (0.0, 1.0)),
                };
                let cross = d1.0 * d2.1 - d1.1 * d2.0;
                let (l1, l2) = ((d1.0 * d1.0 + d1.1 * d1.1).sqrt(), (d2.0 * d2.0 + d2.1 * d2.1).sqrt());
                if cross.abs() / (l1 * l2).max(1e-9) < 0.05 {
                    // parallel: the distance from the end a1 to the line (a2, b2)
                    let d = perp(&*sk.project, a1, a2, b2);
                    Some(Constraint::DistancePL { p: a1, a: a2, b: b2, d, off: 0.0, expr: String::new(), driven: false })
                } else if let (Some(pa1), Some(pb1), Some(pa2), Some(pb2)) = (pt(&*sk.project, a1), pt(&*sk.project, b1), pt(&*sk.project, a2), pt(&*sk.project, b2)) {
                    // THE ANGLE between the lines. The directions of both lines are ORIENTED OUTWARDS FROM
                    // their (virtual) INTERSECTION, so the angle equals the visual opening between the
                    // segments. Otherwise the direction (b - a) follows the order of the points of the line
                    // and may point INTO the vertex, showing THE SUPPLEMENT (30 degrees where the eye sees
                    // 150).
                    let ix = qymcad_ui_state::line_line_ix(pa1, pb1, pa2, pb2).unwrap_or(pb1);
                    let far = |p: Point2, q: Point2| (q.x - ix.x).powi(2) + (q.y - ix.y).powi(2) > (p.x - ix.x).powi(2) + (p.y - ix.y).powi(2);
                    // (start is the end nearer to ix, end the further one), so the direction end - start
                    // points outwards
                    let (na1, nb1, ea, eb) = if far(pa1, pb1) { (a1, b1, pa1, pb1) } else { (b1, a1, pb1, pa1) };
                    let (na2, nb2, ec, ed) = if far(pa2, pb2) { (a2, b2, pa2, pb2) } else { (b2, a2, pb2, pa2) };
                    let (u, v) = ((eb.x - ea.x, eb.y - ea.y), (ed.x - ec.x, ed.y - ec.y));
                    let deg = (u.0 * v.1 - u.1 * v.0).atan2(u.0 * v.0 + u.1 * v.1).abs().to_degrees();
                    Some(Constraint::AngleLines { a: na1, b: nb1, c: na2, d: nb2, deg, expr: String::new(), driven: false })
                } else {
                    None
                }
            }
        }
    };
    if let Some(c) = c {
        sk.project.sketches[si].constraints.push(c);
        let ci = sk.project.sketches[si].constraints.len() - 1;
        let (redundant, conflict) = qymcad_ui_state::finish_dim(&mut *sk.project, &mut *sk.regen, si, ci);
        sk.place.dim = Some(ci);
        *sk.status = if conflict {
            format!("{} {}", ph::WARNING, qymcad_i18n::tr("sk-dim-conflict"))
        } else if redundant {
            qymcad_i18n::tr("sk-driven-dim-hint")
        } else {
            qymcad_i18n::tr("sk-drag-dim-hint")
        };
    }
}