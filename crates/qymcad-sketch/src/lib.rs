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

/// A CORNER AS THE PREVIEW DRAWS IT: where it stands, the two lines that make it, and whether it is in the set
/// already - which of the two colours of the scheme it is drawn in says.
struct Shown {
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

pub fn sketch_props(pr: &mut qymcad_ui_state::PropsCtx, ui: &mut egui::Ui, si: usize) {
    let lin = qymcad_ui_state::lineage_of(&*pr.project, Some(pr.project.sketches[si].id));
    if let Some(n) = qymcad_ui_state::props_header(ui, ph::POLYGON, "sk-props", qymcad_ui_state::NameSlot::Editable(pr.project.sketches[si].name.clone()), &lin) {
        pr.project.sketches[si].name = n;
    }
    // the plane the sketch is placed on
    let pl = sketch_plane_words(&*pr.project, pr.project.sketches[si].plane);
    ui.label(egui::RichText::new(qymcad_i18n::tr1("sk-on", "what", &pl)).weak().small());
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
            ui.label(egui::RichText::new(qymcad_i18n::tr1("sk-conflicts-n", "n", &conflicts.to_string())).color(pr.scheme.pal.error()).small());
            ui.label(egui::RichText::new(qymcad_i18n::tr1("sk-conflict-advice", "icon", ph::RULER)).weak().small());
        } else if redun > 0 {
            ui.label(egui::RichText::new(qymcad_i18n::tr1("sk-redundant-n", "n", &redun.to_string())).color(pr.scheme.pal.note()).small());
        }
        if dof > 0 {
            ui.label(egui::RichText::new(qymcad_i18n::tr1("sk-dof-n", "n", &dof.to_string())).weak().small());
        }
        // the parts of the sketch its last solve had no time for (`Sketch::left_unsolved`)
        let left = pr.project.sketches[si].left_unsolved;
        if left > 0 {
            ui.label(egui::RichText::new(qymcad_i18n::tr1("sk-unsolved-left", "n", &left.to_string())).color(pr.scheme.pal.note()).small());
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
        let holders = pr.project.sketches[si].corner_holders();
        egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
            for (ci, c) in cons.iter_mut().enumerate() {
                if is_sys(c) {
                    continue; // a system `Fixed` on the origin or an axis is not shown
                }
                if matches!(c, Constraint::Orientation { .. }) || holders.contains(&ci) {
                    continue; // the turn of a rectangle is the rectangle's own, and a cut corner's holders are the corner's
                }
                let is_sel = cur_sel == Some(ci);
                // is it a dimension? (consistent redundancy among dimensions is harmless and gets no warning)
                let is_dim = matches!(
                    c,
                    Constraint::Distance { .. }
                        | Constraint::Angle { .. }
                        | Constraint::DistancePL { .. }
                        | Constraint::AngleLines { .. }
                        | Constraint::ArcLength { .. }
                        | Constraint::Diameter { .. }
                        | Constraint::EdgeDistance { .. }
                );
                // which redundant ones to mark is decided by ONE rule, shared with the canvas glyphs
                let redun_geom = flagged.contains(&ci);
                // A CONFLICTING constraint gets a separate, sharper mark: it is an error (the sketch does
                // not solve), whereas redundancy only means "this one may be removed". Both used to give
                // the same glyph.
                let in_conflict = conflict_set.contains(&ci);
                let is_driven = c.is_driven();
                let flag = redun_geom || in_conflict;
                let row = ui
                    .horizontal(|ui| {
                        if in_conflict {
                            ui.colored_label(pr.scheme.pal.error(), ph::WARNING_OCTAGON).on_hover_text(qymcad_i18n::tr("sk-conflict-hint"));
                        } else if flag {
                            ui.colored_label(pr.scheme.pal.error_mild(), ph::WARNING).on_hover_text(qymcad_i18n::tr("sk-overdefined-hint"));
                        }
                        match c {
                            Constraint::Distance { d, .. } => {
                                if ui.selectable_label(is_sel, qymcad_i18n::tr("sk-dim")).clicked() {
                                    sel_click = Some(ci);
                                }
                                // a dimension standing past the field is shown as it stands: put into the range on every frame, it
                                // was written and solved as an edit, and the window never came to rest
                                changed |= ui.add(egui::DragValue::new(d).speed(0.2).range(0.01..=100000.0).clamp_existing_to_range(false).suffix(qymcad_i18n::tr("unit-mm-suffix"))).changed();
                            }
                            Constraint::Angle { deg, .. } => {
                                if ui.selectable_label(is_sel, qymcad_i18n::tr("sk-angle")).clicked() {
                                    sel_click = Some(ci);
                                }
                                changed |= ui.add(egui::DragValue::new(deg).speed(0.5).range(0.1..=359.9).clamp_existing_to_range(false).suffix("°")).changed();
                            }
                            other => {
                                // THE PARTICIPANTS IN THE ROW: "Horizontal: Line 3". Without them a list of
                                // thirty constraints shows that constraints exist but gives no way to find the
                                // one wanted - four rows of "Horizontal" in a row are indistinguishable.
                                let parts =
                                    constraint_parts(&qymcad_ui_state::DrawCtx { cam: pr.cam, set: &*pr.set, scheme: pr.scheme, project: &*pr.project, active_path: pr.active_path }, si, other);
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
        qymcad_ui_state::ask_delete(&mut *pr.deferred, qymcad_ui_state::Sel::Sketch(si));
        // the same path the tree takes - the question and the deletion
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

/// SET THE VALUE OF DIMENSION `ci`, REFUSING ONE THAT CANNOT EXIST. On a refusal, the catalogue key of the reason.
///
/// Reported behaviour: "negative dimensions in sketches ... while you are erasing, 100 becomes 1 - and at
/// that moment the geometry gets thrown into the negative, and since negative dimensions do not exist the
/// whole part goes crooked and you cannot get it back."
///
/// A LENGTH OF ZERO OR LESS IS NOT A DIMENSION. Measured on a rectangle 100 x 40 with a distance dimension
/// on its lower edge: setting that dimension to -100 collapses the edge to nothing - both ends land on
/// (50, 0) - and leaves a residual of 100, an inconsistent system. `DistancePL` already took the magnitude
/// and kept its own sign, so the rule was known; it was applied to one kind of dimension out of six.
///
/// AN ANGLE HAS BOUNDS OF ITS OWN: between two lines it is the opening, over 0 and under 180 deg, as in the
/// professional systems - the side is where the dimension stands, not a sign; through three points it goes round
/// the vertex and stays under a whole turn.
///
/// WHY REFUSING RATHER THAN ROLLING BACK. The editor does roll a conflicting value back afterwards, and
/// that saved the sketch here. But a rollback is a cure applied after the damage: it waits for the residual
/// to rise above a threshold, and a value that quietly solves to a mirrored answer never trips it.
pub fn set_dim_value(project: &mut Project, si: usize, ci: usize, v: f64) -> Result<(), &'static str> {
    use qymcad_core::model::Constraint as C;
    let Some(c) = project.sketches.get_mut(si).and_then(|s| s.constraints.get_mut(ci)) else { return Err("sk-dim-gone") };
    if !v.is_finite() {
        return Err("sk-dim-must-be-positive");
    }
    match c {
        // the angle between two lines is their opening, more than 0 and less than 180 deg; which side it is on is
        // where the dimension stands, not a sign
        C::AngleLines { .. } if v <= 0.0 || v >= HALF_TURN_DEG => Err("sk-angle-range"),
        // three points go round the vertex, so the opening may pass 180 deg, but not a whole turn
        C::Angle { .. } if v <= 0.0 || v >= 2.0 * HALF_TURN_DEG => Err("sk-angle-range-3pt"),
        C::Angle { deg, expr, .. } | C::AngleLines { deg, expr, .. } => {
            *deg = v;
            expr.clear();
            Ok(())
        }
        _ if v <= 0.0 => Err("sk-dim-must-be-positive"),
        // the lengths a sketch takes end where the fields of the bars end: 10 m
        _ if v > MAX_SKETCH_LENGTH => Err("sk-dim-too-large"),
        // `DistancePL` keeps its own sign - it says which side of the line - and takes the magnitude typed
        C::DistancePL { d, expr, .. } => {
            *d = if *d < 0.0 { -v } else { v };
            expr.clear();
            Ok(())
        }
        C::Distance { d, expr, .. } | C::Diameter { d, expr, .. } | C::EdgeDistance { d, expr, .. } => {
            *d = v;
            expr.clear();
            Ok(())
        }
        C::ArcLength { len, expr, .. } => {
            *len = v;
            expr.clear();
            Ok(())
        }
        _ => Err("sk-dim-gone"),
    }
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

/// THE ARC OF THE PREVIEW AS A LINE ON THE SCREEN, walked from `from` to `to` the short way round.
///
/// The two angles are read off the SCREEN points and not off the drawing's own coordinates, and that is the whole
/// difference between the arc that will be made and the arc that is shown: the sheet turns the drawing over
/// (`Sheet::at` takes the sign of y off), so an angle taken before the turn is walked the other way round on the
/// screen, and the preview of a fillet came out on the far side of the horizontal line while the fillet itself landed
/// in the corner.
pub fn corner_arc_points(centre: Pos2, r_px: f32, from: Pos2, to: Pos2) -> Vec<Pos2> {
    let dx = |p: Pos2| (p.x - centre.x) as f64;
    let dy = |p: Pos2| (p.y - centre.y) as f64;
    let a0 = dy(from).atan2(dx(from));
    let a1 = dy(to).atan2(dx(to));
    let mut sweep = a1 - a0;
    while sweep <= -std::f64::consts::PI {
        sweep += std::f64::consts::TAU;
    }
    while sweep > std::f64::consts::PI {
        sweep -= std::f64::consts::TAU;
    }
    const STEPS: usize = 24;
    (0..=STEPS)
        .map(|k| {
            let a = a0 + sweep * k as f64 / STEPS as f64;
            Pos2::new(centre.x + a.cos() as f32 * r_px, centre.y + a.sin() as f32 * r_px)
        })
        .collect()
}

/// THE COLOUR ONE CORNER OF THE SET IS DRAWN IN, which is a consequence of whether it is in the set and of nothing
/// else.
///
/// **THE SCHEME IS READ HERE, ON EVERY FRAME, AND NOT KEPT IN THE STATE OF THE TOOL.** The two colours used to be
/// written into `CornerInput` by the one window that had a scheme to hand: every other way of opening a corner
/// field - a click on a shape, a command, a mode change that clears the state - left them at the transparent
/// default of a `Color32`, and the preview was not drawn at all. Nothing here is a number of a colour.
pub fn corner_colour(scheme: &qymcad_ui_state::SchemeUi, fixed: bool) -> egui::Color32 {
    if fixed {
        scheme.pal.preview_corner_fixed()
    } else {
        scheme.pal.preview_corner_new()
    }
}

/// THE CORNER AS IT WOULD BE, drawn on the sheet: the two ends the lines will be cut at and the arc or the straight
/// cut that will stand between them. Nothing of the sketch is changed — the lines keep the ends they have, and what
/// is drawn is only where the corner will go. A value too big for the corner is drawn as nothing.
///
/// **EVERY CORNER IN THE SET IS DRAWN, NOT ONLY THE ONE UNDER THE CURSOR.** One answer cuts them all, so a person
/// has to see where they are all going before answering. `fixed` says which is which: the corners IN the set are
/// drawn in the accent colour of the scheme, and the one the cursor is pointing at but has not yet named is amber.
///
/// **THE VALUE IS GIVEN, AND NOT READ OUT OF THE TOOL.** It used to be read from `corner.buf`, and the field had
/// already taken the buffer out of the tool by the time the preview was drawn - so the read found an empty string,
/// the value could not be read, and nothing was drawn at all. The preview showed nothing whatever the radius was,
/// amber or violet.
fn draw_corner_preview(cc: &qymcad_ui_state::CornerCtx, ctx: &egui::Context, rect: Rect, si: usize, corners: &[Shown], chamfer: bool, value: f64) {
    let v = value;
    if v <= 1e-6 {
        return;
    }
    let sh = qymcad_ui_state::Sheet { view: *cc.view, rect };
    let at = |p: [f64; 2]| sh.at(qymcad_core::geom::Point2::new(p[0], p[1]));
    let (col_fixed, col_new) = (corner_colour(cc.scheme, true), corner_colour(cc.scheme, false));
    // ITS OWN LAYER, clipped to the sheet: the corners belong to the drawing, not to the panel. ONE LAYER FOR THE
    // WHOLE SET, so that a set of corners is drawn as the one thing being done rather than as several.
    // UNDER THE BOXES: the box of the value is a window over the sheet, and a preview drawn over it ran across its field
    let layer = ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, egui::Id::new(("cornerpreview", si)))).with_clip_rect(rect);
    for &Shown { point: pid, pair, in_set } in corners {
        let Some(pair) = pair else { continue };
        let Some(b) = cc.project.corner_blend(si, pid, pair, corner_cut(cc, chamfer, v)) else { continue };
        // THE COLOUR IS A CONSEQUENCE OF WHETHER THE CORNER IS IN THE SET, not a thing of its own: a corner that is
        // in it keeps the two lines it was made by and is drawn as such, and the one the cursor is pointing at -
        // the only corner a click can still make - is drawn apart from them.
        let col = if in_set { col_fixed } else { col_new };
        match b.arc {
            None => {
                layer.line_segment([at(b.ends[0]), at(b.ends[1])], egui::Stroke::new(2.0, col));
            }
            Some((c, r)) => {
                // the arc is walked in the sheet's own scale, so it looks the same whatever the zoom
                let pts = corner_arc_points(at(c), (r * cc.view.scale as f64) as f32, at(b.ends[0]), at(b.ends[1]));
                layer.add(egui::Shape::line(pts, egui::Stroke::new(2.0, col)));
            }
        }
        // where the lines will be cut: two small marks, so it is seen that the edges themselves will move
        for e in b.ends {
            layer.circle_stroke(at(e), 3.0, egui::Stroke::new(1.4, col));
        }
    }
}

/// THE CUT THE TOOL IN HAND MAKES OF `value`: a fillet of that size in the way of the bar - a radius, a chord or an
/// arc length - or a chamfer in the mode of the bar with its second value. The preview and Enter both take it, so the
/// corner shown is the corner made.
fn corner_cut(cc: &qymcad_ui_state::CornerCtx, chamfer: bool, value: f64) -> qymcad_core::model::CornerCut {
    if !chamfer {
        return qymcad_core::model::CornerCut::Fillet(qymcad_core::model::FilletSize { by: cc.tool_prefs.fillet_by, value });
    }
    let second = parse_num(cc.project, &cc.corner.buf2).unwrap_or(cc.tool_prefs.chamfer_second);
    qymcad_core::model::CornerCut::Chamfer(qymcad_core::model::ChamferLegs { mode: cc.tool_prefs.chamfer_mode, first: value, second })
}

/// THE SET SAID IN WORDS: how many corners one answer will cut.
fn corner_set_says(sk: &qymcad_ui_state::SketchCtx) -> String {
    let n = sk.corner.set.standing().len();
    if n == 0 {
        return qymcad_i18n::tr("sk-click-corner");
    }
    qymcad_i18n::tr1("sk-corner-set-n", "n", &n.to_string())
}

/// WHAT A CLICK AT A POINT DID, said in words rather than left to be read off the drawing.
fn corner_point_says(sk: &qymcad_ui_state::SketchCtx, act: qymcad_ui_state::PointAct) -> String {
    // **WHAT A CLICK AT A POINT SAYS IS THE COUNT OF THE SET, NOT ONLY WHAT IT DID TO THAT ONE POINT.** What the set
    // holds is the question a person has after every click, and it went unanswered at a point: the same click that
    // brought a corner back said nothing at all, and a click that emptied the set said only that the corner of that
    // point was gone - with the rest of the set still standing and no word of it.
    let said = match act {
        qymcad_ui_state::PointAct::PointAdded => qymcad_i18n::tr("sk-corner-point-added"),
        qymcad_ui_state::PointAct::PointRemoved => qymcad_i18n::tr("sk-corner-point-off"),
        qymcad_ui_state::PointAct::LineHidden => qymcad_i18n::tr("sk-corner-set-off"),
        qymcad_ui_state::PointAct::LineShown => return corner_set_says(sk),
        qymcad_ui_state::PointAct::TakenByLines => qymcad_i18n::tr("sk-corner-set-taken"),
        qymcad_ui_state::PointAct::Nothing => return qymcad_i18n::tr("sk-click-corner"),
    };
    format!("{}. {}", said, corner_set_says(sk))
}

/// THE NAME OF A POINT, for the words that say where a line was refused.
fn corner_point_name(sk: &qymcad_ui_state::SketchCtx, si: usize, pid: Id) -> String {
    sk.project.point_xy(si, pid).map(|(x, y)| format!("({}, {})", qymcad_i18n::num(x, 2), qymcad_i18n::num(y, 2))).unwrap_or_default()
}

/// THE YELLOW PREVIEW OF A CORNER THAT IS NOT NAMED YET: the corner a click at the point under the cursor WOULD
/// name. It is drawn only while the cursor stands inside the zone of that point, and it is amber because it is not
/// in the set - a click there makes it violet. A point that already has a corner standing in it shows nothing at
/// all: it is one place, and there is nothing to add there.
fn corner_hover_under(cc: &qymcad_ui_state::CornerCtx, ctx: &egui::Context, rect: Rect, si: usize) -> Option<(Id, (Id, Id))> {
    let p = ctx.input(|i| i.pointer.hover_pos()).filter(|p| rect.contains(*p))?;
    let zone = qymcad_ui_state::corner_zone_px(cc.set);
    let id = qymcad_pick::nearest_point_within(&cc.pick(), rect, p, si, zone)?;
    let w = qymcad_ui_state::to_world(cc.view, rect, p);
    let pair = cc.corner.set.read_at_point(cc.project, si, id, Some((w.x, w.y)))?;
    Some((id, pair))
}

/// THE PREVIEW ON THE SHEET, drawn whether or not the field is up.
///
/// It used to be drawn from inside the field, which meant that the amber preview of the first corner - the corner the
/// cursor is on before anything has been chosen at all - was never drawn: there was no field yet to draw it from.
/// The first corner of the tool was therefore the one corner a person could not see before naming, and the whole
/// point of the amber is to see it first.
fn draw_the_corner_preview(cc: &qymcad_ui_state::CornerCtx, ctx: &egui::Context, rect: Rect) {
    let Some((si, _, chamfer)) = cc.corner.at else {
        // **NOTHING OF A CORNER IS DRAWN WITH ANOTHER TOOL IN HAND.** Without the field there is nothing but the
        // amber preview, and it belongs to the fillet and to the chamfer alone: the corner of a drawing lies under
        // the cursor of every tool that goes near it, and rounding it belongs to one of these two.
        if !matches!(cc.armed.click_op(), 4 | 5) {
            return;
        }
        let qymcad_ui_state::Sel::Sketch(si) = *cc.sel else { return };
        let Some((pid, pair)) = corner_hover_under(cc, ctx, rect, si) else { return };
        draw_corner_preview(cc, ctx, rect, si, &[Shown { point: pid, pair: Some(pair), in_set: false }], cc.armed.click_op() == 5, value_for_preview(cc));
        return;
    };
    // WHAT IS DRAWN IS THE SET PLUS THE ONE THE CURSOR IS POINTING AT, and the colour is a consequence of which of
    // the two it is: the corners in the set are violet and the one that is not named yet is amber. Both colours
    // are read off the scheme HERE, on every frame - never written into the state of the tool, where every other
    // way of opening this field left them at the transparent default of a Color32 and drew nothing.
    // a corner following the pointer is not named yet, and is drawn so
    let in_set = cc.corner.aim == qymcad_ui_state::Aim::Fixed;
    let mut shown: Vec<Shown> = cc.corner.set.standing().iter().map(|&(point, pair)| Shown { point, pair: Some(pair), in_set }).collect();
    if let Some((point, pair)) = corner_hover_under(cc, ctx, rect, si) {
        if !shown.iter().any(|c| c.point == point) {
            shown.push(Shown { point, pair: Some(pair), in_set: false });
        }
    }
    draw_corner_preview(cc, ctx, rect, si, &shown, chamfer, value_for_preview(cc));
}

/// THE VALUE THE PREVIEW IS DRAWN WITH: what is in the field, or the radius of the bar where there is no field yet.
fn value_for_preview(cc: &qymcad_ui_state::CornerCtx) -> f64 {
    parse_num(cc.project, &cc.corner.buf).unwrap_or(cc.tool_prefs.fillet)
}

/// WHERE THE BOX OF THE SET STANDS: beside its first corner, on the outside of it - away from the middle of the cut
/// towards the corner and on past it by `CORNER_BOX_GAP` - and growing that way, so the corner, the two ends the lines
/// are cut at and the arc between them stay in sight. It stood on the place clicked, which is on the corner or on one
/// of its lines: measured on a rectangle 40 x 30, the box covered the corner, both cut ends and the middle of the arc.
///
/// The corner of the box nearest the corner is placed by the size the box had the frame before (`size`). An area given
/// a pivot instead is measured on its first frame and drawn on none: the box did not show at all in the frame it opened.
fn corner_box_place(cc: &qymcad_ui_state::CornerCtx, rect: Rect, si: usize, first: Option<(Id, (Id, Id))>, chamfer: bool, size: egui::Vec2) -> Option<Pos2> {
    let (pid, pair) = first?;
    // a value too big for the corner draws no preview, and the side of the corner is the same at any value
    let v = value_for_preview(cc);
    let b = cc.project.corner_blend(si, pid, pair, corner_cut(cc, chamfer, v)).or_else(|| cc.project.corner_blend(si, pid, pair, corner_cut(cc, chamfer, CORNER_BOX_PROBE)))?;
    let sh = qymcad_ui_state::Sheet { view: *cc.view, rect };
    let at = |p: [f64; 2]| sh.at(qymcad_core::geom::Point2::new(p[0], p[1]));
    let corner = at(b.vertex);
    let out = corner - at([(b.ends[0][0] + b.ends[1][0]) / 2.0, (b.ends[0][1] + b.ends[1][1]) / 2.0]);
    let out = if out.length() > 1e-3 { out.normalized() } else { egui::vec2(1.0, -1.0).normalized() };
    let near = corner + out * CORNER_BOX_GAP;
    // OUTWARD WHERE THERE IS ROOM, and across the corner's other side where there is not: a box at the top of the sheet
    // stood under the bar and read as a field of it
    let side = |grow: f32, at: f32, len: f32, lo: f32, hi: f32| if (grow >= 0.0 && at + len <= hi) || (grow < 0.0 && at - len < lo) { at } else { at - len };
    let left = side(out.x, near.x, size.x, rect.left(), rect.right());
    let top = side(out.y, near.y, size.y, rect.top(), rect.bottom());
    Some(egui::pos2(left.clamp(rect.left(), (rect.right() - size.x).max(rect.left())), top.clamp(rect.top(), (rect.bottom() - size.y).max(rect.top()))))
}

/// The size of the box of the corner tools on the frame it first opens, before egui has measured it, in points: one row
/// of a label, a field and two buttons, and the line that counts the set.
const CORNER_BOX_GUESS: egui::Vec2 = egui::vec2(320.0, 56.0);

/// How far the box of the corner tools stands off the corner, in pixels.
const CORNER_BOX_GAP: f32 = 16.0;

/// The size a corner is read at for the side the box stands on, where the value typed draws no preview, in mm.
const CORNER_BOX_PROBE: f64 = 1e-3;

pub fn corner_input_popup(cc: &mut qymcad_ui_state::CornerCtx, ctx: &egui::Context, rect: Rect) {
    // THE SIDE OF A CORNER WITH MORE THAN ONE TO GIVE IS WHERE THE POINTER IS, while the field is open: the corner of a
    // crossing follows it into the quarter it stands in, and the first leg of a chamfer is the line it is nearer to.
    // Reported behaviour: "the side of the fillet or the chamfer is chosen by where the cursor is, with the preview
    // drawn there".
    let following = cc.corner.aim == qymcad_ui_state::Aim::Following;
    if let (true, Some((si, _, _)), Some(pos)) = (following, cc.corner.at, ctx.input(|i| i.pointer.hover_pos()).filter(|p| rect.contains(*p))) {
        let w = qymcad_ui_state::to_world(cc.view, rect, pos);
        if cc.corner.set.steer(&*cc.project, si, (w.x, w.y)) == qymcad_ui_state::Steered::Yes {
            cc.corner.near = Some(Point2::new(w.x, w.y));
        }
    }
    // THE PREVIEW IS DRAWN FIRST, AND ALWAYS: it belongs to the sheet and not to the field, and a person deciding
    // where to click cannot see it if the field has to be open before it appears.
    draw_the_corner_preview(cc, ctx, rect);
    // A CORNER FOLLOWING THE POINTER HAS NO BOX YET: the box would stand beside a corner that moves, and run from the
    // pointer that goes to it. A click fixes the corner, and the box opens then.
    if following && cc.corner.at.is_some() {
        *cc.status = qymcad_i18n::tr("sk-corner-click-to-fix");
        return;
    }
    // THE FIELD OF THE WHOLE SET, not of one corner: one value cuts every corner the chosen lines make, and the
    // set is beside the field rather than under the pointer, so it does not move with it (Enter applies, Esc
    // cancels, and both act on the whole set as one step of undo).
    let Some((si, pid, chamfer)) = cc.corner.at else { return };

    // THE WHOLE SET, AS IT STANDS THIS FRAME: what one answer will cut. Read on every frame, because a corner of
    // the set can be one line further on or another the moment a line is chosen or let go.
    let standing = cc.corner.set.standing();

    // **A BOX WITH NOTHING TO CUT IN IT IS NOT DRAWN.** The box is the value for the set, and where there is no
    // corner in the set there is nothing the value is for: the amber corner under the cursor says what a click would
    // name, and a box standing over the drawing with a number in it invites a number to be typed for a set that does
    // not exist. It is also what it was before - the box used to open on the tool being taken, over the middle of
    // the drawing, with the set empty and nothing to answer.
    if standing.is_empty() {
        return;
    }

    let at = cc.corner.pos.unwrap_or_else(|| rect.center());
    let want_focus = std::mem::take(&mut cc.corner.focus);
    let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
    let (mut apply, mut cancel) = (false, false);
    let mut buf = std::mem::take(&mut cc.corner.buf);

    // ONE VALUE CUTS THE WHOLE SET, so it is held by the tightest corner in it AND by what the lines between the
    // corners spend on themselves. A value that fits one corner and overruns the one beside it would leave a
    // half-cut drawing behind, and rounding every corner of a rectangle takes half its short side for that reason.
    let limit = cc.corner.set.limit(cc.project, si, qymcad_ui_state::corner_tool(chamfer, cc.tool_prefs), cc.tool_prefs.fillet_by);
    // WHY THIS VALUE CANNOT BE TAKEN, said of the value as it is typed and again at Enter - one sentence, read in
    // two places, so that what the box says beside the field is the very reason Enter refuses it for.
    let judge = |project: &qymcad_core::model::Project, text: &str| -> Option<String> {
        match qymcad_core::expr::eval(text.trim(), &project.param_map()) {
            Err(e) => Some(qymcad_i18n::error_words::expr_error_text(&e)),
            Ok(v) if !v.is_finite() || v <= 1e-6 => Some(qymcad_i18n::tr("cmd-value-zero")),
            // A VALUE THE CORNERS CANNOT TAKE says so as it is typed: past the leg or the radius they hold, and
            // nothing is drawn to promise what cannot be built.
            Ok(v) if limit.is_some_and(|l| v >= l * (1.0 - 1e-9)) => Some(qymcad_i18n::tr("sk-fillet-too-big")),
            Ok(_) if standing.is_empty() => Some(qymcad_i18n::tr("sk-no-corner-here")),
            Ok(_) => None,
        }
    };
    // THE SECOND VALUE OF A CHAMFER of two legs or of a leg and an angle: the corners of the set are cut by both,
    // and one answer still cuts all of them.
    let second_used = chamfer && cc.tool_prefs.chamfer_mode != qymcad_core::feature::ChamferMode::Symmetric;
    let angle = cc.tool_prefs.chamfer_mode == qymcad_core::feature::ChamferMode::DistAngle;
    let judge2 = |project: &qymcad_core::model::Project, text: &str| -> Option<String> {
        match qymcad_core::expr::eval(text.trim(), &project.param_map()) {
            Err(e) => Some(qymcad_i18n::error_words::expr_error_text(&e)),
            Ok(v) if !v.is_finite() || v <= 1e-6 => Some(qymcad_i18n::tr("cmd-value-zero")),
            Ok(v) if angle && v.to_radians() >= std::f64::consts::PI => Some(qymcad_i18n::tr("sk-fillet-too-big")),
            Ok(_) => None,
        }
    };
    // WHY THE VALUE AS TYPED IS REFUSED, said in the box and not only under the pointer. A tooltip exists only while
    // the pointer rests on the glyph beside it, and the reason belongs to the value: a person reading back what they
    // typed has their eyes on the field, not on a warning mark they have to find first.
    let mut said: Option<String> = None;
    let bar_typed = |key: &'static str| ctx.memory(|m| m.has_focus(qymcad_ui_state::bar_field_id(key)));
    let area = egui::Id::new(("cornerinput", si, pid));
    let size = ctx.memory(|m| m.area_rect(area)).map_or(CORNER_BOX_GUESS, |r| r.size());
    let at = corner_box_place(cc, rect, si, standing.first().copied(), chamfer, size).unwrap_or_else(|| qymcad_ui_state::clamp_popup(at, rect) + egui::vec2(10.0, -10.0));
    egui::Area::new(area).fixed_pos(at).order(egui::Order::Foreground).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                // "R of every corner" is the field of FILLET ALL, and of nothing else: it is the one that rounds a
                // whole contour rather than a corner of it, and the zero in the field it is opened with is what says
                // so.
                ui.label(if chamfer {
                    qymcad_i18n::tr(qymcad_ui_state::chamfer_d1_label(cc.tool_prefs.chamfer_mode))
                } else if pid == 0 {
                    qymcad_i18n::tr("sk-r-all-corners")
                } else {
                    qymcad_i18n::tr(qymcad_ui_state::fillet_label(cc.tool_prefs.fillet_by))
                });
                let r0 = qymcad_ui_state::focus_edit(ui, &mut buf, 64.0, "", want_focus);
                // THE REASON, TAKEN FROM THE TEXT AS IT IS TYPED - before the value of the bar is put over it below.
                // Reading it after would read a buffer that has already been given back, and a value a person has
                // typed and is still typing would be judged by something else than what they wrote.
                said = judge(&*cc.project, &buf);
                if let Some(why) = &said {
                    ui.label(egui::RichText::new(ph::WARNING).color(ui.visuals().warn_fg_color)).on_hover_text(why);
                }
                // ONE VALUE IN TWO PLACES: the radius on the bar and the one at the corners. Typed at the corners it
                // is the bar's too; while the corner field is not the one being typed in, it shows the bar's. Two
                // fields apart, a radius typed on the bar was not the one Enter applied - the corners kept the radius
                // of the time before.
                if r0.has_focus() {
                    if let Some(v) = parse_num(cc.project, &buf).filter(|v| *v > 1e-6) {
                        cc.tool_prefs.fillet = v;
                    }
                } else if bar_typed("sk_fillet") && !enter && !r0.lost_focus() && cc.corner.why.is_none() && parse_num(cc.project, &buf).is_none_or(|v| (v - cc.tool_prefs.fillet).abs() > 1e-12) {
                    // the bar's value is taken only by a field at rest: in the frame of Enter it would put the old radius
                    // over the text being applied, and after a refusal over the text refused
                    buf = qymcad_core::expr::fmt_num(cc.tool_prefs.fillet);
                }
                // Enter in the field at the corners applies it, or Enter in the radius field of the bar (below)
                if enter && (r0.lost_focus() || r0.has_focus()) {
                    apply = true;
                }
                // Enter in the radius field of the bar is this Enter too
                if qymcad_ui_state::bar_enter_take(ui.ctx()) {
                    apply = true;
                }
                // THE SECOND VALUE OF A CHAMFER of two legs or of a leg and an angle, beside the first and before the
                // buttons, so Tab goes from the one field to the other; it is the bar's too, as the first leg is.
                // Every corner of the set is cut by both values, and Enter in either applies.
                if second_used {
                    ui.label(qymcad_i18n::tr(qymcad_ui_state::chamfer_d2_label(cc.tool_prefs.chamfer_mode)));
                    let r2 = qymcad_ui_state::focus_edit(ui, &mut cc.corner.buf2, 64.0, "", false);
                    if let Some(why) = judge2(&*cc.project, &cc.corner.buf2) {
                        ui.label(egui::RichText::new(ph::WARNING).color(ui.visuals().warn_fg_color)).on_hover_text(why);
                    }
                    if r2.has_focus() {
                        if let Some(v) = parse_num(cc.project, &cc.corner.buf2).filter(|v| *v > 1e-6) {
                            cc.tool_prefs.chamfer_second = v;
                        }
                    } else if bar_typed("sk_chamfer2") && !enter && !r2.lost_focus() && parse_num(cc.project, &cc.corner.buf2).is_none_or(|v| (v - cc.tool_prefs.chamfer_second).abs() > 1e-12) {
                        cc.corner.buf2 = qymcad_core::expr::fmt_num(cc.tool_prefs.chamfer_second);
                    }
                    if enter && (r2.lost_focus() || r2.has_focus()) {
                        apply = true;
                    }
                }
                if ui.button(ph::CHECK).clicked() {
                    apply = true;
                }
                if ui.button(ph::X).clicked() {
                    cancel = true;
                }
            });
            // HOW MUCH IS IN HAND, said beside the field: a person typing one value for four corners has to know
            // that it is four, and the one answer that cuts them is one Enter.
            if !standing.is_empty() {
                ui.label(qymcad_i18n::tr1("sk-corner-set-n", "n", &standing.len().to_string()));
            }
            // WHY A VALUE IS REFUSED IS SAID IN THE BOX AND NOT ONLY UNDER THE POINTER. The reason belongs to the
            // value being typed, and a person reading it back has their eyes on the field rather than hunting the
            // sheet for a warning glyph: the words were a tooltip, and the tooltip only existed while the pointer
            // rested on the glyph beside it.
            if let Some(why) = &said {
                ui.label(egui::RichText::new(format!("{} {why}", ph::WARNING)).color(ui.visuals().warn_fg_color));
            }
        });
    });
    cc.corner.buf = buf;
    let refused = judge(&*cc.project, &cc.corner.buf).or_else(|| second_used.then(|| judge2(&*cc.project, &cc.corner.buf2)).flatten()); // taken as zero, it closed the field and did nothing
    if apply && refused.is_some() {
        *cc.status = format!("{} {}", ph::WARNING, refused.clone().unwrap_or_default());
        cc.corner.why = refused;
        apply = false;
    }
    if apply {
        let r = parse_num(cc.project, &cc.corner.buf.clone()).unwrap_or(0.0);
        // THE BOUNDARY OF AN OPERATION: one step of undo, named after the tool. A person naming four corners of a
        // shape meant one act, and undo taking back one corner of it would leave the drawing in a state nobody
        // asked for.
        // the lines cut at a crossing to name the corner are the start of this step: the cut joins it, and one undo
        // takes back the whole of it
        let joins = cc.corner.crossing == qymcad_ui_state::CrossingCut::Cut;
        if !joins {
            qymcad_ui_state::begin_edit(&mut *cc.edits, &*cc.project, qymcad_i18n::tr(if chamfer { "tool-chamfer" } else { "tool-fillet" }));
        }
        if r > 1e-6 {
            cc.tool_prefs.fillet = r; // sticky: the next corner offers the same value
                                      // THE SIZE AS IT WAS GIVEN, not only as a radius: a fillet taken by the length of its arc or by the chord
                                      // between its ends is a different drawing, and a chamfer of two legs is two numbers.
            let size = qymcad_core::model::FilletSize { by: cc.tool_prefs.fillet_by, value: r };
            if chamfer && second_used {
                cc.tool_prefs.chamfer_second = parse_num(cc.project, &cc.corner.buf2.clone()).unwrap_or(cc.tool_prefs.chamfer_second);
            }
            let cut: usize = if pid == 0 {
                // "ROUND EVERY CORNER" OF A SHAPE, which is one command about a whole set of lines rather than about
                // the corners the set happens to name: it is the only corner cut left to the drawing to find.
                let only = cc.corner.only.take().or_else(|| {
                    let sel: std::collections::HashSet<Id> = cc.sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
                    (!sel.is_empty()).then_some(sel)
                });
                cc.project.fillet_all_corners_by(si, size, only.as_ref())
            } else {
                // ONE ANSWER CUTS THE WHOLE SET, OR NOTHING: the core cuts every corner by the size as the bar gives it,
                // the same cut the preview drew, and leaves the sketch as it was where one corner does not take it
                let corners: Vec<qymcad_core::model::CornerAt> = standing.iter().map(|&(point, pair)| qymcad_core::model::CornerAt { point, pair }).collect();
                let cut = corner_cut(cc, chamfer, r);
                cc.project.cut_corner_set(si, &corners, cut, cc.corner.near)
            };
            if cut > 0 {
                // THE CHOSEN LINES THAT THE KNIFE HAS CONSUMED GO OUT OF THE SELECTION: a line that is no longer
                // there cannot stand lit, and the selection would show a piece of the drawing that is not in it.
                // What is left is lit as it is - a corner that was cut away took its lines with it, and the ones
                // that remain are what a further click would work on.
                cc.sel_sk.items.retain(|&(_, id)| qymcad_ui_state::pick_still_stands(&*cc.project, si, id));
                qymcad_ui_state::invalidate(cc.regen);
                *cc.status = if pid != 0 && cut > 1 {
                    // MORE THAN ONE CORNER CUT AT ONCE IS SAID AS MANY: "done" would read as the one corner the
                    // pointer was on, and the set is not visible in a word that size.
                    qymcad_i18n::tr1("sk-filleted-n", "n", &cut.to_string())
                } else {
                    qymcad_i18n::tr("sk-done")
                };
            } else {
                *cc.status = format!("{} {}", ph::WARNING, qymcad_i18n::tr("sk-fillet-too-big"));
                cc.corner.why = Some(qymcad_i18n::tr("sk-fillet-too-big"));
            }
            if joins {
                qymcad_ui_state::fold_into_last_step(&mut *cc.edits, &*cc.project);
            } else {
                qymcad_ui_state::close_edit(&mut *cc.edits, &*cc.project);
            }
            if cut > 0 {
                cc.corner.clear(); // ALL of the input, so nothing of this set travels into the next call
            }
            // A VALUE THE CORNERS CANNOT TAKE keeps the field open with it, the reason said: closed, it left a
            // person no field to put a smaller one in, and no words beside it.
        } else {
            // a size of nothing cuts nothing: the lines cut at a crossing for it are whole again
            if joins {
                qymcad_ui_state::take_back_last_step(&mut *cc.edits, &mut *cc.project, &mut *cc.regen);
            } else {
                qymcad_ui_state::close_edit(&mut *cc.edits, &*cc.project);
            }
            cc.corner.clear();
        }
    }
    if cancel || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        // the lines cut at a crossing for a corner never cut are whole again
        if cc.corner.crossing != qymcad_ui_state::CrossingCut::None {
            qymcad_ui_state::take_back_last_step(&mut *cc.edits, &mut *cc.project, &mut *cc.regen);
        }
        cc.corner.clear();
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
            let at = (qymcad_ui_state::Sheet { view: *pl.view, rect }).at(Point2::new(click.x + rx, click.y + ry));
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

/// A rectangle: width by height. From two corners it is rebuilt from the anchor corner so the signs
/// are kept; from the centre it grows about the centre, so the centre never travels while the numbers
/// are typed.
pub fn rect_input_popup(pl: &mut qymcad_ui_state::PlaceCtx, ctx: &egui::Context, rect: Rect, si: usize) {
    // WHAT STAYS PUT while the numbers are typed: the first corner, or the centre.
    enum Anchor {
        Corner { fixed: Point2, signs: Point2 },
        Center { at: Point2 },
    }
    // The numbers the fields open on, where the popup stands, and what is being rebuilt.
    struct Active {
        anchor: Anchor,
        ids: Vec<Id>,
        w0: f64,
        h0: f64,
        at: Point2,
    }
    let active = pl
        .place
        .rect()
        .map(|(a, b, ids)| {
            let signs = Point2::new((b.x - a.x).signum(), (b.y - a.y).signum());
            Active { anchor: Anchor::Corner { fixed: a, signs }, ids, w0: (b.x - a.x).abs(), h0: (b.y - a.y).abs(), at: Point2::new(a.x.max(b.x), a.y.max(b.y)) }
        })
        .or_else(|| {
            pl.place.rect_center().map(|(center, corner, ids)| Active {
                anchor: Anchor::Center { at: center },
                ids,
                w0: 2.0 * (corner.x - center.x).abs(),
                h0: 2.0 * (corner.y - center.y).abs(),
                at: Point2::new(center.x.max(corner.x), center.y.max(corner.y)),
            })
        });
    // a rectangle: width by height (text fields, with auto-focus, Tab and Enter)
    if let Some(active) = active {
        let Active { anchor, ids, w0, h0, at } = active;
        let want_focus = std::mem::take(&mut pl.place.focus);
        if want_focus {
            pl.place.buf[0] = format!("{}", (w0 * 1000.0).round() / 1000.0);
            pl.place.buf[1] = format!("{}", (h0 * 1000.0).round() / 1000.0);
        }
        let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
        let (mut chg, mut close, mut got_focus) = (false, false, false);
        let mut buf = [std::mem::take(&mut pl.place.buf[0]), std::mem::take(&mut pl.place.buf[1])];
        egui::Area::new(egui::Id::new(("rectinput", si)))
            .fixed_pos(qymcad_ui_state::clamp_popup((qymcad_ui_state::Sheet { view: *pl.view, rect }).at(at), rect) + egui::vec2(10.0, -10.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
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
        pl.place.typed |= chg;
        if got_focus {
            pl.place.focus = false;
        }
        if chg {
            let nw = parse_num(pl.project, &buf[0]).unwrap_or(w0).max(0.01);
            let nh = parse_num(pl.project, &buf[1]).unwrap_or(h0).max(0.01);
            match anchor {
                Anchor::Corner { fixed, signs } => {
                    let nb = Point2::new(fixed.x + if signs.x < 0.0 { -nw } else { nw }, fixed.y + if signs.y < 0.0 { -nh } else { nh });
                    pl.project.delete_entities(si, &ids);
                    let nids = pl.project.add_rect_entity(si, fixed.x, fixed.y, nb.x, nb.y, qymcad_core::feature::Purpose::Real);
                    pl.place.set(qymcad_ui_state::PlacingShape::Rect { a: fixed, b: nb, ids: nids });
                }
                Anchor::Center { at: center } => {
                    let nb = Point2::new(center.x + nw / 2.0, center.y + nh / 2.0);
                    pl.project.delete_entities(si, &ids);
                    let nids = pl.project.add_rect_from_centre(si, center, nb, qymcad_core::feature::Purpose::Real);
                    pl.place.set(qymcad_ui_state::PlacingShape::RectCenter { center, corner: nb, ids: nids });
                }
            }
            qymcad_ui_state::invalidate(pl.regen);
        }
        // A WIDTH OR A HEIGHT TYPED IS LAID AS DIMENSIONS when the fields are closed with Enter or the tick, as the
        // diameter of a circle is: a size typed by hand is a size meant. Closed untouched, or with Esc, the rectangle is
        // left free.
        if close && pl.place.typed {
            if let Some(side) = pl.place.rect().or_else(|| pl.place.rect_center()).and_then(|(_, _, ids)| ids.first().copied()) {
                pl.project.dimension_rect(si, side);
                pl.project.solve_sketch(si);
                qymcad_ui_state::invalidate(pl.regen);
            }
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
            let at = (qymcad_ui_state::Sheet { view: *pl.view, rect }).at(Point2::new(cx + r, cy));
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

/// A POINT OF A NEW LINE where it was put: the place, the point, and the ends of the new line itself.
#[derive(Clone, Copy)]
struct LandedOn {
    at: Point2,
    point: Id,
    own: (Id, Id),
}

/// THE POINT TIED TO THE LINE IT LANDED ON (not its own): at its middle - within the 2 px a point lands on a line by - a
/// midpoint, anywhere else along it a point on the line. Reported behaviour: "nothing snaps to projected geometry" - the
/// pointer stuck to the middle of a line and along it, and the point put there was held by nothing, on a projected
/// line as on a line drawn by hand.
fn tie_to_the_line_under(project: &mut Project, view: qymcad_ui_state::View2d, si: usize, landed: LandedOn) {
    use qymcad_core::model::Constraint;
    let Some((la, lb)) = qymcad_ui_state::line_under_point(project, &view, si, landed.at, landed.own.0, landed.own.1) else { return };
    let at = |id: Id| project.sketches[si].points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
    let middle = at(la).zip(at(lb)).is_some_and(|(u, v)| ((u.0 + v.0) / 2.0 - landed.at.x).hypot((u.1 + v.1) / 2.0 - landed.at.y) * f64::from(view.scale) <= 2.0);
    let tie = if middle { Constraint::Midpoint { p: landed.point, a: la, b: lb } } else { Constraint::PointOnLine { p: landed.point, a: la, b: lb } };
    project.add_constraint_if_independent(si, tie);
}

/// THE POINTS OF A SHAPE JUST PUT WHERE IT WAS CLICKED, TIED TO WHAT THEY LANDED ON, as the ends of a line are: of the
/// points `own` the shape stands on, the one at each of `clicks` - put on a point of the sketch (a corner of a
/// projection, a grey corner of the face `auto_project_under` took in) is made coincident with it, put on a line is held
/// on it (`tie_to_the_line_under`). Reported behaviour: "even without projecting, tie to the edges of the face (the thin
/// grey ones) - at a rebuild everything falls apart" - a rectangle drawn from a grey corner and a circle around one
/// stood where they were drawn when the body changed.
fn tie_the_clicked_points(project: &mut Project, view: qymcad_ui_state::View2d, si: usize, clicks: &[Point2], own: &[Id]) {
    use qymcad_core::model::Constraint;
    for at in clicks {
        let Some(s) = project.sketches.get(si) else { return };
        let near = |q: &qymcad_core::model::SketchPoint| (q.x - at.x).hypot(q.y - at.y) < 1e-6;
        let Some(point) = s.points.iter().find(|q| own.contains(&q.id) && near(q)).map(|q| q.id) else { continue };
        let used: std::collections::HashSet<Id> = s.entities.iter().flat_map(qymcad_core::model::entity_points).collect();
        match s.points.iter().find(|q| !own.contains(&q.id) && used.contains(&q.id) && near(q)).map(|q| q.id) {
            Some(other) => {
                project.add_constraint_if_independent(si, Constraint::Coincident { a: other, b: point });
            }
            None => tie_to_the_line_under(project, view, si, LandedOn { at: *at, point, own: (point, point) }),
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
    // 3) a point on an edge: the new end landed on an existing line (not its own), so it is tied to it - and so is the start
    // of the first segment of a chain; the start of a next segment is the end of the one before, tied already
    tie_to_the_line_under(project, view, si, LandedOn { at: p2, point: b, own: (a, b) });
    if prev.is_none() {
        tie_to_the_line_under(project, view, si, LandedOn { at: p1, point: a, own: (a, b) });
    }
    // (a coincidence with a vertex happens by itself: `sketch_point_at(1e-6)` already shares the point)
    project.solve_sketch(si);
}

/// The pop-up entry of sizes right after a rectangle or a polygon is built.
/// THE POPUP OF THE CORNER TOOL (a fillet radius or a chamfer leg): Enter applies, Esc cancels.
///
/// `armed` is the tool in hand, and it is here rather than in `Editing` because the preview on the sheet is drawn
/// whether or not the field is up: before the first click there is no field, and the amber corner under the cursor
/// is drawn as the chamfer or the fillet the tool in hand would cut it as.
pub fn place_input_popup(ed: qymcad_ui_state::Editing, mut tools: qymcad_ui_state::PopupTools, looks: qymcad_ui_state::PopupLooks, ctx: &egui::Context, rect: Rect) {
    let qymcad_ui_state::PopupTools { corner, place, sel_sk, armed } = &mut tools;
    corner_input_popup(
        &mut qymcad_ui_state::CornerCtx {
            corner,
            project: &mut *ed.project,
            edits: &mut *ed.edits,
            sel_sk,
            view: &*ed.view,
            regen: &mut *ed.regen,
            status: ed.status,
            tool_prefs: looks.tool_prefs,
            scheme: looks.scheme,
            set: looks.set,
            sel: ed.sel,
            armed,
        },
        ctx,
        rect,
    );
    let qymcad_ui_state::Sel::Sketch(si) = *ed.sel else {
        place.clear(); // everything unfinished in the drawing at once (otherwise one of the three is forgotten)
        return;
    };
    offset_box(OffsetCtx { project: &mut *ed.project, edits: &mut *ed.edits, regen: &mut *ed.regen, sel_sk, status: &mut *ed.status, tool_prefs: &mut *looks.tool_prefs }, &*ed.view, si, ctx, rect);
    let pl = &mut qymcad_ui_state::PlaceCtx { place, project: ed.project, view: &*ed.view, regen: ed.regen };
    ellipse_input_popup(pl, ctx, rect, si);
    rect_input_popup(pl, ctx, rect, si);
    rect_dims_popup(pl, ctx, rect, si);
    poly_input_popup(pl, ctx, rect, si);
}

/// A RECTANGLE REOPENED BY A DOUBLE CLICK: its width and its height, typed into the dimensions it holds them by, as the
/// diameter of a circle is. A value typed is the dimension's at once and the rectangle follows - from the corner it was
/// drawn from, or about its centre; Enter, the tick or Esc closes the fields.
pub fn rect_dims_popup(pl: &mut qymcad_ui_state::PlaceCtx, ctx: &egui::Context, rect: Rect, si: usize) {
    let Some(dims) = pl.place.rect_dims() else { return };
    let dim = |p: &qymcad_core::model::Project, ci: usize| match p.sketches.get(si).and_then(|s| s.constraints.get(ci)) {
        Some(c @ qymcad_core::model::Constraint::Distance { .. }) => Some(c.clone()),
        _ => None,
    };
    let (Some(qymcad_core::model::Constraint::Distance { d: w0, a: at, .. }), Some(qymcad_core::model::Constraint::Distance { d: h0, .. })) =
        (dim(pl.project, dims.width), dim(pl.project, dims.height))
    else {
        pl.place.clear(); // the dimensions went from under the fields
        return;
    };
    let want_focus = std::mem::take(&mut pl.place.focus);
    if want_focus {
        pl.place.buf = [qymcad_core::expr::fmt_num(w0), qymcad_core::expr::fmt_num(h0)];
    }
    let Some(corner) = qymcad_ui_state::sketch_pt(pl.project, si, at) else { return };
    let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
    let (mut chg, mut close) = (false, false);
    let mut buf = std::mem::take(&mut pl.place.buf);
    egui::Area::new(egui::Id::new(("rectdims", si)))
        .fixed_pos(qymcad_ui_state::clamp_popup((qymcad_ui_state::Sheet { view: *pl.view, rect }).at(corner), rect) + egui::vec2(10.0, -10.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(qymcad_i18n::tr("sk-width-short"));
                    let r0 = qymcad_ui_state::focus_edit(ui, &mut buf[0], 60.0, "", want_focus);
                    ui.label(qymcad_i18n::tr("sk-height-short"));
                    let r1 = qymcad_ui_state::focus_edit(ui, &mut buf[1], 60.0, "", false);
                    chg = r0.changed() || r1.changed();
                    if ((r0.lost_focus() || r1.lost_focus()) && enter) || ui.button(ph::CHECK).clicked() {
                        close = true;
                    }
                });
            });
        });
    if chg {
        let mut set = |ci: usize, text: &str| {
            if let Some(v) = parse_num(pl.project, text).filter(|v| *v > 0.01) {
                if let Some(qymcad_core::model::Constraint::Distance { d, .. }) = pl.project.sketches[si].constraints.get_mut(ci) {
                    *d = v;
                }
            }
        };
        set(dims.width, &buf[0]);
        set(dims.height, &buf[1]);
        pl.project.solve_sketch(si);
        qymcad_ui_state::invalidate(pl.regen);
    }
    pl.place.buf = buf;
    if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        pl.place.clear();
    }
}

/// A DOUBLE CLICK ON A SHAPE OPENS ITS SIZE: a side of a rectangle its width and height, a circle its diameter (laid
/// as a dimension when it has none), an arc its radius, the circle a polygon hangs on - or the polygon itself - its
/// radius and turn. Answers whether a shape was there.
pub fn open_shape_size(sk: &mut qymcad_ui_state::SketchCtx, rect: Rect, pos: Pos2, si: usize) -> bool {
    use qymcad_ui_state::{InlineEdit, PlacingShape};
    if let Some((1, eid)) = sketch_hit(&sk.pick(), rect, pos, si) {
        if let Some(dims) = sk.project.rect_dims(si, eid) {
            sk.project.solve_sketch(si);
            sk.place.set(PlacingShape::RectDims(dims));
            sk.place.focus = true;
            return true;
        }
    }
    if let Some(eid) = qymcad_pick::nearest_circle_entity(&sk.pick(), rect, pos, si) {
        // a circle has its diameter edited, or the field of its size opened; an arc the field of its radius
        let center = sk.project.sketches[si].entities.iter().find(|e| e.id == eid).and_then(|e| match e.kind {
            qymcad_core::model::EntityKind::Circle { center, .. } => Some(center),
            _ => None,
        });
        // the circumscribed circle of a polygon (the vertices hang on it) opens the polygon popup (the radius plus the
        // angle), while an ordinary circle gets a diameter
        let is_poly_rim = center.is_some_and(|c| sk.project.sketches[si].constraints.iter().any(|x| matches!(x, qymcad_core::model::Constraint::PointOnCircle { c: cc, .. } if *cc == c)));
        if let (true, Some(c)) = (is_poly_rim, center) {
            sk.place.set(PlacingShape::Poly(c));
            sk.place.focus = true;
        } else if let Some(ci) = center.and_then(|c| sk.project.sketches[si].constraints.iter().position(|x| matches!(x, qymcad_core::model::Constraint::Diameter { c: cc, .. } if *cc == c))) {
            // a circle with its diameter laid: the dimension is edited
            *sk.inline = InlineEdit::Dim(ci);
            sk.dim.focus = true;
        } else {
            // A CIRCLE OR AN ARC WITH NO SIZE OF ITS OWN opens the field of its size, and a size is laid only when a value
            // is typed there - as at a circle just drawn. Reported behaviour: "a double click, if the size was not
            // changed, must not put a dimension line on the circle".
            *sk.inline = InlineEdit::Circle(eid);
            sk.dim.focus = true;
        }
        return true;
    }
    if let Some(cid) = qymcad_ui_state::polygon_under(&*sk.project, &*sk.view, rect, pos, si) {
        sk.place.set(PlacingShape::Poly(cid)); // editing the radius of the construction circle
        sk.place.focus = true;
        return true;
    }
    false
}

/// FINISH THE SPLINE - what a double click does.
///
/// Split out of the event handling for the same reason as everything else: a test must finish the shape
/// through THE SAME code rather than a copy of its own. Esc cancels a spline - that is a different
/// action, and substituting it for finishing would mean checking the wrong thing.
pub fn finish_spline(sk: &mut qymcad_ui_state::SketchCtx) {
    if let qymcad_ui_state::Sel::Sketch(si) = *sk.sel {
        if sk.tool.pts.len() >= 2 {
            // THE BOUNDARY OF AN OPERATION: the spline enters the document here, not at its clicks, so this is the
            // step of undo named after it.
            qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, qymcad_i18n::tr("sk-spline"));
            let pts = std::mem::take(&mut sk.tool.pts);
            sk.project.add_spline(si, pts, qymcad_core::feature::Ends::Open, qymcad_core::feature::Purpose::of(sk.tool.construction));
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

/// The ANGLE popup for the rotate tool (`move_op == 3`), placed at the picked centre. Enter or the tick
/// applies `rotate_entities`; editing the value gives a live preview (the ghost in `draw_move_preview`).
pub fn sketch_rotate_popup(sk: &mut qymcad_ui_state::SketchCtx, ctx: &egui::Context, rect: Rect) {
    if sk.armed.move_op() != 3 {
        return;
    }
    let qymcad_ui_state::Sel::Sketch(si) = *sk.sel else { return };
    let Some(base) = sk.tool.move_base else { return };
    let eids: Vec<Id> = sk.sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
    let texts = qymcad_ui_state::sel_text_indices(&*sk.project, &*sk.sel_sk, si);
    if eids.is_empty() && texts.is_empty() {
        return;
    }
    let rot = &mut *sk.rot;
    let at = (qymcad_ui_state::Sheet { view: *sk.view, rect }).at(base);
    let want_focus = std::mem::take(&mut rot.focus);
    let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
    let (mut chg, mut apply, mut got_focus) = (false, false, false);
    let mut buf = std::mem::take(&mut rot.buf);
    // AN ANGLE THAT CANNOT BE TAKEN is refused where it is typed: the reason on the mark beside the field, the tool
    // and what was typed staying. It used to be applied as 0 or taken away with the popup.
    let checked = angle_of(sk.project, &buf);
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
                if let Err(msg) = &checked {
                    ui.colored_label(sk.scheme.pal.error_mild(), ph::X).on_hover_text(msg);
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
        rot.angle = parse_num(sk.project, &rot.buf.clone()).unwrap_or(0.0); // the live preview
    }
    if let (true, Err(msg)) = (apply, angle_of(sk.project, &sk.rot.buf.clone())) {
        *sk.status = msg;
        sk.rot.focus = true;
        return;
    }
    if apply {
        if let Some(v) = parse_num(sk.project, &sk.rot.buf.clone()) {
            // THE BOUNDARY OF AN OPERATION: applying the angle is the act, so it makes one step of undo named
            // after the tool, as the target click of a move does.
            qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, qymcad_i18n::tr("tool-rotate"));
            // WHERE A FREE TURN WOULD HAVE PUT THE POINTS, and where the constraints let them go.
            //
            // A rectangle carries two horizontal and two vertical constraints, so it CANNOT turn: the solver
            // brings it back to upright, merely relocated - measured, a 90 degree turn about zero left the
            // corner at (-5, 5) instead of (0, 0). Saying "Rotated by 90 deg" over that is the same lie the
            // move tool used to tell, and it is told out loud here instead.
            let ids = sk.project.entity_point_ids(si, &eids);
            let at = |p: &qymcad_core::model::Project| -> Vec<(f64, f64)> {
                p.sketches.get(si).map(|s| ids.iter().filter_map(|id| s.points.iter().find(|q| q.id == *id).map(|q| (q.x, q.y))).collect()).unwrap_or_default()
            };
            let before = at(sk.project);
            let (sn, cs) = (v.to_radians().sin(), v.to_radians().cos());
            let want: Vec<(f64, f64)> = before
                .iter()
                .map(|(x, y)| {
                    let (dx, dy) = (x - base.x, y - base.y);
                    (base.x + dx * cs - dy * sn, base.y + dx * sn + dy * cs)
                })
                .collect();
            if !eids.is_empty() {
                sk.project.rotate_entities(si, &eids, base.x, base.y, v);
            }
            for &ti in &texts {
                sk.project.rotate_sketch_text(si, ti, base.x, base.y, v);
            }
            let after = at(sk.project);
            let mean = |a: &[(f64, f64)], b: &[(f64, f64)]| -> f64 {
                if a.len() != b.len() || a.is_empty() {
                    return 0.0;
                }
                a.iter().zip(b).map(|((x0, y0), (x1, y1))| (x1 - x0).hypot(y1 - y0)).sum::<f64>() / a.len() as f64
            };
            let asked = mean(&before, &want); // how far the turn alone would have carried the points
            let off = mean(&after, &want); // how far the result stands from that
                                           // a tenth of what was asked, and at least a hundredth of a millimetre: below that it did turn
            *sk.status = if asked > 1e-9 && off > (asked * 0.1).max(1e-2) {
                format!("{} {}", ph::WARNING, qymcad_i18n::tr("sk-turn-held"))
            } else {
                qymcad_i18n::tr1("sk-rotated-by", "a", &v.to_string())
            };
            qymcad_ui_state::commit_edit(&mut sk.rebuild());
        }
        *sk.armed = qymcad_ui_state::Armed::None;
        sk.tool.move_base = None;
        qymcad_ui_state::invalidate(&mut *sk.regen);
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
enum Level {
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

pub fn try_constraint_inner(
    project: &mut Project,
    regen: &mut qymcad_ui_state::Rebuilding,
    sel: qymcad_ui_state::Sel,
    sel_sk: &mut qymcad_ui_state::SketchSelection,
    status: &mut String,
    code: u8,
) -> bool {
    use qymcad_core::model::Constraint;
    let qymcad_ui_state::Sel::Sketch(si) = sel else { return false };
    middles_for(project, si, sel_sk, code);
    let pts = qymcad_ui_state::sel_point_ids(sel_sk);
    let mut lines = qymcad_ui_state::sel_line_pts(project, sel_sk, si);
    // the lines drawn, before the axes join them: Vertical and Horizontal go on these, an axis already stands so
    let drawn = lines.clone();
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
        // EVERY SELECTED LINE GETS IT, not the first alone; with no line selected, every point lines up with the first.
        // Reported behaviour (issue #34): with three lines selected Vertical turned one of them.
        1 if !drawn.is_empty() => new.extend(drawn.iter().map(|&(a, b)| Constraint::Horizontal { a, b })),
        1 if pts.len() >= 2 => new.extend(pts[1..].iter().map(|&b| Constraint::Horizontal { a: pts[0], b })),
        2 if !drawn.is_empty() => new.extend(drawn.iter().map(|&(a, b)| Constraint::Vertical { a, b })),
        2 if pts.len() >= 2 => new.extend(pts[1..].iter().map(|&b| Constraint::Vertical { a: pts[0], b })),
        // between lines: every further line is tied to the first one picked
        3 if lines.len() >= 2 => new.extend(lines[1..].iter().map(|&(c, d)| Constraint::Parallel { a: lines[0].0, b: lines[0].1, c, d })),
        4 if lines.len() >= 2 => new.push(Constraint::Perpendicular { a: lines[0].0, b: lines[0].1, c: lines[1].0, d: lines[1].1 }),
        5 if lines.len() >= 2 => new.extend(lines[1..].iter().map(|&(c, d)| Constraint::Equal { a: lines[0].0, b: lines[0].1, c, d })),
        5 => {
            // equal radii of two circles
            let cs = qymcad_ui_state::sel_circle_centers(project, sel_sk, si);
            if cs.len() >= 2 {
                new.push(Constraint::EqualRadius { c1: cs[0], c2: cs[1] });
            }
        }
        6 if !pts.is_empty() => new.extend(pts.iter().map(|p| Constraint::Fixed { p: *p })),
        7 if lines.len() >= 2 => new.extend(lines[1..].iter().map(|&(c, d)| Constraint::Collinear { a: lines[0].0, b: lines[0].1, c, d })),
        8 => {
            // concentricity is A REAL kind with a glyph of its own, not a fake made of coincident centres
            let centers = qymcad_ui_state::sel_circle_centers(project, sel_sk, si);
            if centers.len() >= 2 {
                new.push(Constraint::Concentric { c1: centers[0], c2: centers[1] });
            }
        }
        9 => {
            // tangency: a line plus a circle, the line touching it at a point of its own (`touch_point`)
            if let (Some((la, lb)), Some((cc, rr))) = (lines.first().copied(), qymcad_ui_state::sel_circle_cr(project, sel_sk, si)) {
                new.push(Constraint::Tangent { a: la, b: lb, c: cc, r: rr });
                if let Some(p) = touch_point(project, si, sel_sk, (la, lb), cc) {
                    new.push(Constraint::PointOnCircle { p, c: cc });
                }
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
    // WHAT IS HELD ALREADY IS NOT LAID AGAIN: a second Vertical on a line that has one turned both redundant (found
    // checking issue #34). With nothing left to add, the sketch says so instead of "The constraint is added".
    let new = project.independent_of(si, new);
    if new.is_empty() {
        sel_sk.clear();
        *status = qymcad_i18n::tr("sk-constraint-already");
        return true;
    }
    // the points of what was selected: the relations among them that the new ones make redundant are lifted below
    let among: std::collections::HashSet<Id> = pts.iter().copied().chain(lines.iter().flat_map(|&(a, b)| [a, b])).collect();
    let (had, old_pts): (usize, Vec<(f64, f64)>) = (project.sketches[si].constraints.len(), project.sketches[si].points.iter().map(|p| (p.x, p.y)).collect());
    // A LINE MADE HORIZONTAL OR VERTICAL IS TURNED TO IT FIRST, about its middle (about an end that is held), keeping its
    // length: a line given a direction is turned to it, not squashed onto it. The solve takes the least movement of the points, and bringing both
    // ends to one height is less than turning a steep line: a line at 80 deg given Horizontal came out 5 mm long of 30,
    // and a vertical one, its Vertical deleted, was refused as pulled into a point. Taken back with the rest if the
    // constraint is not kept.
    for c in &new {
        let (line, level) = match *c {
            Constraint::Horizontal { a, b } if drawn.contains(&(a, b)) => ((a, b), Level::Horizontal),
            Constraint::Vertical { a, b } if drawn.contains(&(a, b)) => ((a, b), Level::Vertical),
            _ => continue,
        };
        turn_to_level(project, si, line, level);
    }
    // EQUAL ON TWO CIRCLES THAT EACH CARRY THEIR RADIUS: the second one's radius becomes a reference and follows the
    // first - one radius, not a contradiction. Reported behaviour: the circle tool gives each circle its dimension, and
    // Equal on two of them left the sketch over-defined (residual 2.89).
    let referenced: Vec<usize> = new
        .iter()
        .filter_map(|c| match c {
            Constraint::EqualRadius { c1, c2 } => {
                let driving = |cc: Id| project.sketches[si].constraints.iter().position(|k| matches!(k, Constraint::Diameter { c, driven: false, .. } if *c == cc));
                driving(*c1).and(driving(*c2))
            }
            _ => None,
        })
        .collect();
    for &k in &referenced {
        if let Constraint::Diameter { driven, .. } = &mut project.sketches[si].constraints[k] {
            *driven = true;
        }
    }
    let sizes_before = shape_sizes(project, si);
    project.sketches[si].constraints.extend(new);
    let resid = project.solve_sketch(si);
    // A CONSTRAINT MET BY SHRINKING A SHAPE TO NOTHING IS NOT MET. Horizontal over Vertical on one line is satisfied
    // exactly by a line of no length - residual 0 - and both stood green while the line was gone. Reported behaviour
    // (found checking issue #34): three lines made Vertical, then Horizontal, shrank to points.
    let shrunk = shape_sizes(project, si).iter().zip(&sizes_before).any(|(after, before)| *before > 1e-6 && *after < 1e-6);
    sel_sk.clear(); // the selection and whatever was waiting for it
    qymcad_ui_state::invalidate(regen);
    // A CONSTRAINT THE SKETCH CANNOT MEET IS NOT KEPT, as a dimension that does not fit is not: parallel on two lines an
    // angle of 45 deg holds apart left both in the sketch, unsolved (residual 0.79), with nothing marked. It is taken
    // back, the points return where they were, and the status line says why. 1e-2 mm is the threshold the dimension
    // editor rolls back at: above the solver's noise, below any real conflict.
    if resid > 1e-2 || shrunk {
        project.sketches[si].constraints.truncate(had);
        for &k in &referenced {
            if let Constraint::Diameter { driven, .. } = &mut project.sketches[si].constraints[k] {
                *driven = false; // taken back with the constraint
            }
        }
        for (p, (x, y)) in project.sketches[si].points.iter_mut().zip(old_pts) {
            p.x = x;
            p.y = y;
        }
        project.solve_sketch(si);
        *status = if shrunk { qymcad_i18n::tr("sk-constraint-shrinks") } else { qymcad_i18n::tr1("sk-constraint-conflict", "r", &qymcad_i18n::num(resid, 2)) };
        return true;
    }
    let lifted = project.drop_implied_relations(si, had, &among);
    *status = if resid >= 1e-3 {
        qymcad_i18n::tr1("sk-constraint-added-resid", "r", &qymcad_i18n::num(resid, 2))
    } else if lifted > 0 {
        qymcad_i18n::tr1("sk-constraint-added-implied", "n", &lifted.to_string())
    } else {
        qymcad_i18n::tr("sk-constraint-added")
    };
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

/// The index of the dimension (a distance or an angle) whose caption is nearest to a screen point.
pub fn dim_at(sk: &mut qymcad_ui_state::SketchCtx, rect: Rect, pos: Pos2, si: usize) -> Option<usize> {
    let sh = qymcad_ui_state::Sheet { view: *sk.view, rect };
    use qymcad_core::model::Constraint;
    // THE CONSTRAINTS ARE COPIED OUT FIRST. The loop calls back into the context (`dim_label_pos`), and a
    // reference into the document held across that call borrows the same record twice.
    let cs = sk.project.sketches.get(si)?.constraints.clone();
    let mut best: Option<(f32, usize)> = None;
    for ci in 0..cs.len() {
        // the distance to the caption: taken anywhere on its text, however large the settings make it and however
        // it is turned
        let mut d = dim_label_box(sk, rect, si, ci).map(|(at, size, angle)| qymcad_ui_state::label_reach_turned(at, size, angle, pos)).unwrap_or(f32::INFINITY);
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
    dim_label_box(sk, rect, si, ci).map(|(at, _, _)| at)
}

/// WHERE THE TEXT OF A DIMENSION STANDS AND HOW MUCH ROOM IT TAKES - its middle, its size and its turn - from the same
/// geometry the drawing uses, so the text is taken where the eye sees it.
pub fn dim_label_box(sk: &mut qymcad_ui_state::SketchCtx, rect: Rect, si: usize, ci: usize) -> Option<(Pos2, egui::Vec2, f32)> {
    let sh = qymcad_ui_state::Sheet { view: *sk.view, rect };
    use qymcad_core::model::Constraint;
    let c = sk.project.sketches.get(si)?.constraints.get(ci)?.clone();
    let size = qymcad_ui_state::dim_text_size(&qymcad_ui_state::dim_caption(sk.project, si, &c, sk.set)?, sk.set.dim_font);
    match c {
        Constraint::Distance { .. } | Constraint::DistancePL { .. } | Constraint::EdgeDistance { .. } => {
            qymcad_ui_state::linear_text_of(sk.project, si, ci, &sh, sk.set).map(|(place, size)| (place.center, size, place.angle))
        }
        // both angles: where the drawing puts the label, from the one geometry of an angular dimension
        Constraint::Angle { .. } | Constraint::AngleLines { .. } => qymcad_ui_state::angle_dim_geom(sk.project, si, ci, &sh, sk.set).map(|g| (g.label, size, 0.0)),
        Constraint::Diameter { .. } => qymcad_ui_state::radial_dim_geom(sk.project, si, ci, &sh, sk.set).map(|g| (g.text, g.size, g.angle)),
        // an arc length: the number over the middle of its dimension arc, from the one geometry it is drawn by - otherwise
        // `dim_at` does not find it, and the dimension can be neither picked, nor edited, nor dragged
        Constraint::ArcLength { .. } => qymcad_ui_state::arc_length_dim_geom(sk.project, si, ci, &sh, sk.set).map(|g| (g.text, g.size, 0.0)),
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
    let picked = !sk.sel_sk.items.is_empty();
    if try_constraint(sk, code) {
        sk.sel_sk.constraint = None;
    } else {
        // it could not be applied to the current selection, so the picking starts AFRESH, without any
        // elements left stuck: otherwise they would go into the constraint as `pts[0]` or `lines[0]` and
        // tie the wrong things together.
        sk.sel_sk.clear(); // the selection and whatever was waiting for it
        sk.sel_sk.constraint = Some(code);
        // WHAT WAS PICKED DOES NOT TAKE IT, and that is said: two points are not parallel, two lines have no centre.
        // Asking for the geometry again as if nothing had been picked left the person guessing what was wrong.
        *sk.status = if picked {
            qymcad_i18n::tr1("sk-constraint-not-for-pick", "takes", &qymcad_i18n::tr(constraint_hint_key(code)))
        } else {
            qymcad_i18n::tr("sk-pick-for-constraint")
        };
    }
}

/// The words of a constraint's button - its name and what it ties - by its code.
pub fn constraint_hint_key(code: u8) -> &'static str {
    match code {
        0 => "con-coincident-hint",
        1 => "con-horizontal-hint",
        2 => "con-vertical-hint",
        3 => "con-parallel-hint",
        4 => "con-perpendicular-hint",
        5 => "con-equal",
        6 => "con-fix",
        7 => "con-collinear-hint",
        8 => "con-concentric-hint",
        9 => "con-tangent-hint",
        10 => "con-symmetric-hint",
        _ => "con-midpoint-hint",
    }
}

/// Put the label of angular dimension `ci` where `angle_dim_follow` says: the radius of its arc and its share of the angle.
fn put_angle_label(project: &mut Project, si: usize, ci: usize, (r, t): (f64, Option<f64>)) {
    use qymcad_core::model::Constraint;
    if let Some(Constraint::Angle { off, at, .. } | Constraint::AngleLines { off, at, .. }) = project.sketches.get_mut(si).and_then(|s| s.constraints.get_mut(ci)) {
        (*off, *at) = (r, t);
    }
}

/// WHAT FOLLOWS THE POINTER ON THE SHEET THIS FRAME: a dimension being placed, led by the pointer as it is, and the hint
/// of a snap, gone once the pointer has left the sheet.
pub fn follow_pointer(sk: &mut qymcad_ui_state::SketchCtx, rect: Rect, pointer: Option<Pos2>) {
    update_placing_dim(sk, rect, pointer);
    if pointer.is_none() {
        *sk.snap_hint = None;
    }
}

/// While the dimension follows the pointer, its offset (`off`) is updated to match.
fn update_placing_dim(sk: &mut qymcad_ui_state::SketchCtx, rect: Rect, pointer: Option<Pos2>) {
    let sh = qymcad_ui_state::Sheet { view: *sk.view, rect };
    use qymcad_core::model::Constraint;
    let Some(ci) = sk.place.dim else { return };
    let qymcad_ui_state::Sel::Sketch(si) = *sk.sel else {
        sk.place.dim = None;
        return;
    };
    // THE POINTER AS IT IS, NOT THE SNAPPED CURSOR: snapping is for the points a dimension is put between, not for where
    // its line is laid. Led by the snapped cursor, the dimension stuck on the line it measures - its middle and the
    // points on it caught the cursor - and further off it jumped from node to node of the grid, 3 px behind the hand.
    let Some(sc) = pointer else { return };
    match sk.project.sketches[si].constraints.get(ci).cloned() {
        Some(Constraint::Distance { a, b, .. }) => {
            if let (Some(pa), Some(pb)) = (qymcad_ui_state::sketch_pt(sk.project, si, a), qymcad_ui_state::sketch_pt(sk.project, si, b)) {
                let (sa, sb) = (sh.at(pa), sh.at(pb));
                let mid = ((sa.to_vec2() + sb.to_vec2()) / 2.0).to_pos2();
                // THE ORIENTATION follows the cursor: to the side gives a vertical dimension (dy), above or
                // below a horizontal one (dx), anything else an aligned one.
                let (cx, cy) = (sc.x - mid.x, sc.y - mid.y);
                let wanted = if cx.abs() > cy.abs() * 1.7 {
                    2u8
                } else if cy.abs() > cx.abs() * 1.7 {
                    1u8
                } else {
                    0u8
                };
                // A PROJECTION THAT MEASURES NOTHING IS NOT OFFERED: led to the side of a horizontal line the dimension
                // turned vertical and measured the line's height, 0.0, drawn red. Under 0.1 % of the length a projection
                // gives way to the other axis, and that to the length along the line.
                let along = |axis: u8| match axis {
                    1 => (pa.x - pb.x).abs(),
                    2 => (pa.y - pb.y).abs(),
                    _ => ((pa.x - pb.x).powi(2) + (pa.y - pb.y).powi(2)).sqrt(),
                };
                let nothing = 1e-3 * along(0);
                let new_axis = match wanted {
                    0 => 0,
                    w if along(w) > nothing => w,
                    w if along(3 - w) > nothing => 3 - w,
                    _ => 0,
                };
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
                let measured = along(new_axis);
                if let Some(Constraint::Distance { off: o, axis, d, .. }) = sk.project.sketches[si].constraints.get_mut(ci) {
                    *o = off;
                    *axis = new_axis;
                    *d = measured; // while it flies it measures; the value is typed when it is placed
                }
            }
        }
        Some(Constraint::DistancePL { p, a, b, .. }) => {
            if let (Some(pp), Some(pa), Some(ab)) =
                (qymcad_ui_state::sketch_pt(sk.project, si, p), qymcad_ui_state::sketch_pt(sk.project, si, a), qymcad_ui_state::line_screen_dir(sk.project, sk.view, si, a, b, rect))
            {
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
        // THE LABEL OF AN ANGLE FOLLOWS THE CURSOR - the radius of its arc and its place along it - and the next click
        // places it and opens its value, as a length's does
        Some(Constraint::AngleLines { .. }) | Some(Constraint::Angle { .. }) => {
            if let Some(placed) = qymcad_ui_state::angle_dim_follow(sk.project, si, ci, &sh, sk.set, sc) {
                put_angle_label(sk.project, si, ci, placed);
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
    *sk.status = if id == 0 { qymcad_i18n::tr("sk-not-projectable") } else { qymcad_i18n::tr("sk-projected-hint") };
    sk.project.solve_sketch(si);
    qymcad_ui_state::commit_edit(&mut sk.rebuild());
    qymcad_ui_state::invalidate(sk.regen);
}

/// A POINT PUT ON AN EDGE OF THE BODY UNDER THE SKETCH TAKES THE EDGE INTO IT: the edge is projected (`ProjSource::Edge`)
/// as construction geometry - it holds what is drawn and makes no contour - so the point put on its end is that end, and
/// a point put along it or at its middle is tied there (`tie_to_the_line_under`). The edge then moves with the body, and
/// what is drawn from it follows. Nothing when a curve of the sketch lies there already. Reported behaviour: "even
/// without projecting, tie to the edges of the face (the thin grey ones) - at a rebuild everything falls apart" - a point
/// put on a grey edge was held by nothing, and the body moved away from under it.
fn auto_project_under(sk: &mut qymcad_ui_state::SketchCtx, rect: Rect, si: usize, at: Point2) {
    use qymcad_core::model::ProjSource;
    let pos = (qymcad_ui_state::Sheet { view: *sk.view, rect }).at(at);
    if qymcad_pick::nearest_line_eid(&sk.pick(), rect, pos, si).is_some() || qymcad_pick::nearest_circle_entity(&sk.pick(), rect, pos, si).is_some() {
        return;
    }
    let Some((body, edge)) = nearest_ref_edge(sk, si, rect, pos) else { return };
    let id = qymcad_ui_state::with_kernel(&mut sk.rebuild(), |project, k| project.add_sketch_projection(si, body, ProjSource::Edge(edge), k));
    let Some(proj) = sk.project.sketches[si].projections.iter().find(|p| p.id == id) else { return };
    let curves = proj.entities.clone();
    for e in sk.project.sketches[si].entities.iter_mut().filter(|e| curves.contains(&e.id)) {
        e.construction = true;
    }
}

/// WHERE A CLICK OR AN ENTER OF EXTEND LANDS: the canvas, the pointer on it, the sketch, and the line under the pointer.
#[derive(Clone, Copy)]
struct ExtendAt {
    rect: Rect,
    pos: Pos2,
    si: usize,
    line: Option<Id>,
}

/// WHAT EXTEND ASKS OF ITS LINE with the pointer at `pos`: the pointer in the sketch, the curve under it, and the ends
/// "Both sides" gives.
fn extend_ask(sk: &mut qymcad_ui_state::SketchCtx, rect: Rect, pos: Pos2, si: usize) -> qymcad_core::model::ExtendAsk {
    let w = qymcad_ui_state::to_world(&*sk.view, rect, pos);
    let held = sk.tool.extend;
    let over = qymcad_pick::nearest_line_eid(&sk.pick(), rect, pos, si).filter(|e| Some(*e) != held).or_else(|| qymcad_pick::nearest_circle_entity(&sk.pick(), rect, pos, si));
    let sides = if sk.tool_prefs.extend_both { qymcad_core::model::ExtendSides::Both } else { qymcad_core::model::ExtendSides::Nearer };
    qymcad_core::model::ExtendAsk { pointer: Point2::new(w.x, w.y), over, sides }
}

/// A CLICK OF EXTEND: with no line held, the line clicked is taken and its extension previewed; with one held, the
/// extension the preview shows is made and the line let go. A line meeting nothing on the side asked says so.
fn extend_click(sk: &mut qymcad_ui_state::SketchCtx, at: ExtendAt) {
    let Some(held) = sk.tool.extend else {
        sk.tool.extend = at.line;
        *sk.status = qymcad_i18n::tr("opt-extend-to-hint");
        return;
    };
    extend_apply(sk, at, held);
}

/// THE EXTENSION OF LINE `held` MADE as the pointer at `at` asks, and the line let go.
fn extend_apply(sk: &mut qymcad_ui_state::SketchCtx, at: ExtendAt, held: Id) {
    let ask = extend_ask(sk, at.rect, at.pos, at.si);
    let ext = sk.project.line_extension(at.si, held, &ask);
    if sk.project.extend_line_by(at.si, held, ext) {
        sk.tool.extend = None;
        sk.sel_sk.clear(); // the selection and whatever was waiting for it
        qymcad_ui_state::invalidate(&mut *sk.regen);
        *sk.status = qymcad_i18n::tr("sk-done");
    } else if ext.kept == qymcad_core::model::EndKept::Joined {
        *sk.status = qymcad_i18n::tr("sk-extend-end-joined");
    } else {
        *sk.status = qymcad_i18n::tr("sk-op-failed-no-intersection");
    }
}

/// THE KEYS OF THE SKETCH TOOLS IN HAND, each frame: Enter makes the extension Extend previews, as a click does; and
/// the field of the rotation angle at its centre (`sketch_rotate_popup`).
pub fn sketch_tool_keys(sk: &mut qymcad_ui_state::SketchCtx, ctx: &egui::Context, rect: Rect) {
    offset_on_enter(sk, ctx, rect);
    // LINES A CORNER TOOL CUT AT A CROSSING, the tool put down with no corner cut, are whole again
    if sk.corner.crossing == qymcad_ui_state::CrossingCut::Left {
        qymcad_ui_state::take_back_last_step(&mut *sk.edits, &mut *sk.project, &mut *sk.regen);
        sk.corner.crossing = qymcad_ui_state::CrossingCut::None;
    }
    sketch_rotate_popup(sk, ctx, rect);
    let (Some(held), qymcad_ui_state::Sel::Sketch(si)) = (sk.tool.extend, *sk.sel) else { return };
    if sk.armed.click_op() != 2 || ctx.egui_wants_keyboard_input() || !ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
        return;
    }
    let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) else { return };
    qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, qymcad_i18n::tr("tool-extend"));
    extend_apply(sk, ExtendAt { rect, pos, si, line: None }, held);
    qymcad_ui_state::commit_edit_if_changed(&mut sk.rebuild());
}

/// THE MENU OF THE RIGHT BUTTON ON A SKETCH OPEN FOR EDITING, over what is selected: delete, construction, a projection
/// made ordinary geometry, the dimensions an arc and two round edges take, the constraints, select all.
pub fn sketch_context_menu(sk: &mut qymcad_ui_state::SketchCtx, ui: &mut egui::Ui, si: usize) {
    let has_sel = !sk.sel_sk.items.is_empty();
    let has_ent = sk.sel_sk.items.iter().any(|(k, _)| *k == 1);
    if ui.add_enabled(has_sel, egui::Button::new(format!("{} {}", ph::TRASH, qymcad_i18n::tr("props-delete")))).clicked() {
        qymcad_ui_state::delete_sketch_sel(&mut *sk.project, &mut *sk.regen, &mut *sk.sel_sk, &mut *sk.status, si);
        ui.close();
    }
    if ui.add_enabled(has_ent, egui::Button::new(qymcad_i18n::tr("sk-construction-toggle"))).clicked() {
        let eids: Vec<Id> = sk.sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
        sk.project.toggle_construction(si, &eids);
        sk.project.solve_sketch(si);
        qymcad_ui_state::invalidate(&mut *sk.regen);
        ui.close();
    }
    // THE CURVES LYING HERE: a curve picked that others lie over - a side drawn over another - lists them all, each by
    // its name and length, and the one clicked becomes the selection
    let picked: Vec<Id> = sk.sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
    if let [one] = picked.as_slice() {
        let here = sk.project.overlapping_curves(si, *one);
        if here.len() > 1 {
            ui.menu_button(qymcad_i18n::tr1("sk-overlapping", "n", &here.len().to_string()), |ui| {
                for id in here {
                    let name = sketch_entity_name(&*sk.project, si, id);
                    let len = qymcad_i18n::num(curve_length(&*sk.project, si, id), 1);
                    // the rectangle the curve is a side of, by its number in the sketch: two sides of one length are told
                    // apart by the shape they belong to
                    let rect = sk.project.sketches[si].rects.iter().position(|r| r.sides.contains(&id));
                    let label = match rect {
                        Some(k) => qymcad_i18n::trn("sk-overlapping-side", &[("name", &name), ("rect", &(k + 1).to_string()), ("len", &len)]),
                        None => qymcad_i18n::tr2("sk-overlapping-item", "name", &name, "len", &len),
                    };
                    if ui.button(label).clicked() {
                        sk.sel_sk.items = vec![(1, id)];
                        *sk.status = qymcad_i18n::tr1("sk-overlapping-picked", "name", &name);
                        ui.close();
                    }
                }
            });
        }
    }
    // A PROJECTION MADE ORDINARY: the curves selected that a body drives are let go of it, and stay where they are
    let projected: Vec<Id> = sk.sel_sk.items.iter().filter(|(k, id)| *k == 1 && sk.project.sketches[si].projected_entities().contains(id)).map(|(_, id)| *id).collect();
    if !projected.is_empty() && ui.button(qymcad_i18n::tr("sk-make-ordinary")).on_hover_text(qymcad_i18n::tr("sk-make-ordinary-hint")).clicked() {
        qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, qymcad_i18n::tr("sk-make-ordinary"));
        sk.project.release_projections_of(si, &projected);
        qymcad_ui_state::invalidate(&mut *sk.regen);
        qymcad_ui_state::commit_edit_if_changed(&mut sk.rebuild());
        *sk.status = qymcad_i18n::tr("sk-made-ordinary");
        ui.close();
    }
    // an arc length dimension, for the picked arc
    let arc_eid = sk.sel_sk.items.iter().filter(|(k, _)| *k == 1).find_map(|(_, id)| {
        sk.project.sketches.get(si).and_then(|s| s.entities.iter().find(|e| e.id == *id)).and_then(|e| matches!(e.kind, qymcad_core::model::EntityKind::Arc { .. }).then_some(*id))
    });
    if let Some(aid) = arc_eid {
        if ui.button(qymcad_i18n::tr("sk-arc-length-dim")).clicked() {
            if let Some(ci) = sk.project.ensure_arc_length(si, aid) {
                qymcad_ui_state::finish_dim(&mut *sk.project, &mut *sk.regen, si, ci);
                sk.place.dim = Some(ci);
            }
            ui.close();
        }
    }
    // a tangent (edge-to-edge) dimension: two references are picked, at least one of
    // them a circle or an arc
    let edge_refs: Vec<(Id, i8)> = sk
        .project
        .sketches
        .get(si)
        .map(|s| {
            sk.sel_sk
                .items
                .iter()
                .filter_map(|&(k, id)| {
                    if k == 1 {
                        s.entities.iter().find(|e| e.id == id).and_then(|e| match e.kind {
                            qymcad_core::model::EntityKind::Circle { center, .. } | qymcad_core::model::EntityKind::Arc { center, .. } => Some((center, -1i8)),
                            _ => None,
                        })
                    } else if k == 0 {
                        Some((id, 0i8))
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    let can_edge = edge_refs.len() == 2 && edge_refs.iter().any(|(_, m)| *m != 0);
    if ui.add_enabled(can_edge, egui::Button::new(qymcad_i18n::tr("sk-tangent-dim"))).on_hover_text(qymcad_i18n::tr("sk-gap-hint")).clicked() {
        let ((c1, m1), (c2, m2)) = (edge_refs[0], edge_refs[1]);
        let d = sk.project.measure_edge_distance(si, c1, m1, c2, m2);
        let ci = sk.project.sketches[si].constraints.len();
        sk.project.sketches[si].constraints.push(qymcad_core::model::Constraint::EdgeDistance { c1, c2, d, m1, m2, off: 0.0, expr: String::new(), driven: false, at: None });
        qymcad_ui_state::finish_dim(&mut *sk.project, &mut *sk.regen, si, ci);
        sk.gsel.constraint = Some(ci);
        qymcad_ui_state::invalidate(&mut *sk.regen);
        ui.close();
    }
    ui.separator();
    ui.menu_button(qymcad_i18n::tr("sk-constraint"), |ui| {
        for (label, code) in [
            (&qymcad_i18n::tr("sk-coincident"), 0u8),
            (&qymcad_i18n::tr("sk-horizontal"), 1),
            (&qymcad_i18n::tr("sk-vertical"), 2),
            (&qymcad_i18n::tr("sk-parallel"), 3),
            (&qymcad_i18n::tr("sk-perpendicular"), 4),
            (&qymcad_i18n::tr("sk-equal"), 5),
            (&qymcad_i18n::tr("sk-tangency"), 9),
            (&qymcad_i18n::tr("sk-midpoint"), 11),
            (&qymcad_i18n::tr("sk-fix"), 6),
        ] {
            if ui.button(label).clicked() {
                constraint_button(sk, code);
                ui.close();
            }
        }
    });
    ui.separator();
    if ui.button(qymcad_i18n::tr("sk-select-all")).clicked() {
        qymcad_ui_state::select_all_sketch(&mut *sk.annot, &mut *sk.gsel, &*sk.project, &mut *sk.sel_sk, &mut *sk.status, si);
        ui.close();
    }
}

/// A click with the dimension tool. Returns true when the click was handled.
/// A CIRCLE OR AN ARC DRAWN OPENS THE FIELD OF ITS SIZE, and lays no dimension of its own. The size is laid as a
/// dimension when a value is typed and the field closed with Enter or the tick - a diameter for a circle, a radius for an
/// arc; closed untouched, or with Esc, the shape is left free, as the rectangle is (`rect_input_popup`).
///
/// Reported behaviour: "on circles a radius is always put ... if no value was typed and nothing was edited, do not put
/// it, and let it be deleted - it is nailed down. Sometimes a mess comes of it." A diameter laid on every circle drawn
/// over-defined a sketch the moment a tangent or an equal radius was laid on it.
///
/// Construction geometry carries no dimensions and opens no field.
fn open_the_size_of_the_new_shape(sk: &mut qymcad_ui_state::SketchCtx, eid: Id, purpose: qymcad_core::feature::Purpose) {
    if purpose == qymcad_core::feature::Purpose::Construction {
        return;
    }
    *sk.inline = qymcad_ui_state::InlineEdit::Circle(eid);
    sk.dim.focus = true;
}

/// The arc the tool has just added, the last entity of the sketch, has the field of its radius opened.
fn open_the_size_of_the_new_arc(sk: &mut qymcad_ui_state::SketchCtx, si: usize, purpose: qymcad_core::feature::Purpose) {
    let last = sk.project.sketches[si].entities.last().filter(|e| matches!(e.kind, qymcad_core::model::EntityKind::Arc { .. })).map(|e| e.id);
    if let Some(eid) = last {
        open_the_size_of_the_new_shape(sk, eid, purpose);
    }
}

/// A click with the dimension tool. Returns true when the click was handled.
pub fn dim_click(sk: &mut qymcad_ui_state::SketchCtx, rect: Rect, pos: Pos2) -> bool {
    // THE BOUNDARY OF AN OPERATION: placing a dimension is a deliberate act and makes one undo step.
    qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, qymcad_i18n::tr("sk-dim"));
    let r = dim_click_inner(sk, rect, pos);
    qymcad_ui_state::commit_edit_if_changed(&mut sk.rebuild()); // picking the first line changes nothing yet
    r
}

pub fn dim_click_inner(sk: &mut qymcad_ui_state::SketchCtx, rect: Rect, pos: Pos2) -> bool {
    use qymcad_core::model::Constraint;
    let qymcad_ui_state::Sel::Sketch(si) = *sk.sel else {
        *sk.status = qymcad_i18n::tr("sk-pick-sketch-first");
        return true;
    };
    // A GREY EDGE OF THE BODY CLICKED IS TAKEN INTO THE SKETCH FIRST, as construction, and the dimension is laid on it:
    // a dimension from the edge of the face the sketch sits on holds what it measures to the body. The click went to
    // nothing, and the value typed after it was laid as the length of the line clicked next.
    if sk.place.dim.is_none() && sk.armed.dim_kind() != 3 {
        let w = qymcad_ui_state::to_world(&*sk.view, rect, pos);
        auto_project_under(sk, rect, si, Point2::new(w.x, w.y));
    }
    // the dimension follows the cursor: a click places it, OR (when it is a provisional length of a line
    // and a SECOND element was hit) switches it to a distance between the two.
    if let Some(ci) = sk.place.dim {
        if let Some(qymcad_ui_state::DimRef::Line(la, lb)) = sk.dim.first {
            // It switches to a distance ONLY on a click exactly on A VERTEX (not an end of this line) or
            // on AN AXIS. A click on the body of another line does NOT switch it - otherwise placing the
            // dimension line near the edge of a shape (a square, say) broke.
            let r2 = qymcad_pick::nearest_sketch_point(&sk.pick(), rect, pos, si)
                .filter(|p| *p != la && *p != lb)
                .map(qymcad_ui_state::DimRef::Point)
                .or_else(|| {
                    let o = (qymcad_ui_state::Sheet { view: *sk.view, rect }).at(Point2::new(0.0, 0.0));
                    if (pos.y - o.y).abs() <= qymcad_ui_state::grab::grab(&*sk.set, Grab::Guide) {
                        let (a, b) = sk.project.ensure_axis(si, 0);
                        Some(qymcad_ui_state::DimRef::Line(a, b))
                    } else if (pos.x - o.x).abs() <= qymcad_ui_state::grab::grab(&*sk.set, Grab::Guide) {
                        let (a, b) = sk.project.ensure_axis(si, 1);
                        Some(qymcad_ui_state::DimRef::Line(a, b))
                    } else {
                        None
                    }
                })
                .or_else(|| {
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
                sk.dim.before = None; // a distance to the second reference is made instead
                make_between_dim(sk, si, qymcad_ui_state::DimRef::Line(la, lb), r2);
                return true;
            }
        }
        sk.place.dim = None;
        sk.dim.first = None;
        sk.dim.before = None; // placed: the length is the person's now
        sk.dim.fresh = Some((ci, sk.edits.undo.len()));
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
                let laid = sk.project.sketches[si].constraints.len();
                if let Some(ci) = sk.project.ensure_diameter(si, c, true) {
                    // A DIAMETER MADE BY THIS CLICK AND ITS VALUE TYPED ARE ONE STEP OF UNDO, as a linear dimension and
                    // its value are: a circle drawn carries no diameter of its own, so the tool makes it. The click is one
                    // operation (`dim_click`), closed after this - the step it adds is the one the value joins.
                    if ci >= laid {
                        sk.dim.fresh = Some((ci, sk.edits.undo.len() + 1));
                    }
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
        // the sketch before this pick: resolving it may make an axis of the sketch on demand, and Esc takes that back too
        let before = sk.dim.first.is_none().then(|| Box::new(sk.project.sketches[si].clone()));
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
                        sk.dim.before = before;
                        let d = ((pa.x - pb.x).powi(2) + (pa.y - pb.y).powi(2)).sqrt();
                        sk.project.sketches[si].constraints.push(Constraint::Distance { a, b, d, off: 0.0, expr: String::new(), driven: false, axis: 0, at: None });
                        let ci = sk.project.sketches[si].constraints.len() - 1;
                        let (_, conflict) = qymcad_ui_state::finish_dim(&mut *sk.project, &mut *sk.regen, si, ci);
                        sk.place.dim = Some(ci);
                        sk.dim.first = Some(r);
                        *sk.status = if conflict { format!("{} {}", ph::WARNING, qymcad_i18n::tr("sk-length-conflict")) } else { qymcad_i18n::tr("sk-length-hint") };
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
            sk.project.sketches[si].constraints.push(Constraint::Angle { a, b, c, deg, expr: String::new(), driven: false, off: 0.0, at: None });
            let ci = sk.project.sketches[si].constraints.len() - 1;
            let (redundant, conflict) = qymcad_ui_state::finish_dim(&mut *sk.project, &mut *sk.regen, si, ci);
            sk.place.dim = Some(ci); // the next click places it and opens its value
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

/// A PROVISIONAL LENGTH LEFT BEHIND by taking another tool (every way of taking one lets go of the dimension without
/// the document in hand): the sketch it was put on comes back as it was before the pick, as on Esc.
fn drop_abandoned_dim(sk: &mut qymcad_ui_state::SketchCtx) {
    if sk.place.dim.is_some() || sk.dim.first.is_some() {
        return;
    }
    let Some(before) = sk.dim.before.take() else { return };
    if let Some(si) = sk.project.sketches.iter().position(|s| s.id == before.id) {
        sk.project.sketches[si] = *before;
        sk.project.solve_sketch(si);
        qymcad_ui_state::invalidate(sk.regen);
    }
}

/// Esc WHILE A DIMENSION IS BEING PLACED: a provisional length on a picked line (the second reference still awaited)
/// goes without a trace - the sketch as it was before the pick comes back whole, the axis the length made on demand
/// too; a dimension being moved stays where it is. Reported behaviour: after the pick and Esc a click where the
/// dimension would have stood still placed one, and the sketch kept a fixed point it did not have.
pub fn cancel_placing_dim(sk: &mut qymcad_ui_state::SketchCtx) {
    if let (Some(_), qymcad_ui_state::Sel::Sketch(si), Some(before)) = (sk.dim.first, *sk.sel, sk.dim.before.take()) {
        if let Some(s) = sk.project.sketches.get_mut(si) {
            *s = *before;
            sk.project.solve_sketch(si);
            qymcad_ui_state::invalidate(sk.regen);
        }
    }
    sk.dim.before = None;
    sk.place.dim = None;
    sk.dim.first = None;
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

pub fn sketch_select_click(sk: &mut qymcad_ui_state::SketchCtx, rect: Rect, pos: Pos2, additive: bool) {
    let sh = qymcad_ui_state::Sheet { view: *sk.view, rect };
    let qymcad_ui_state::Sel::Sketch(si) = *sk.sel else { return };
    // a click on a text object picks it (Del removes it, a double click edits the string or the height,
    // dragging moves it)
    if let Some(ti) = qymcad_ui_state::text_at(&*sk.project, &*sk.view, rect, pos, si) {
        sk.annot.text = Some(ti);
        sk.annot.note = None;
        sk.gsel.constraint = None;
        // THE TEXT JOINS THE SELECTION, so the move tools take it - alone, or with Shift beside the geometry already
        // picked (issue #32)
        if !additive {
            sk.sel_sk.clear(); // the selection and whatever was waiting for it
        }
        if let Some(id) = sk.project.sketches.get(si).and_then(|s| s.texts.get(ti)).map(|t| t.id) {
            if !sk.sel_sk.items.contains(&(qymcad_ui_state::SEL_TEXT, id)) {
                sk.sel_sk.items.push((qymcad_ui_state::SEL_TEXT, id));
            }
        }
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
    // A LINE CLICKED AT ITS MIDDLE, WHERE ITS TRIANGLE SHOWS, IS PICKED AS THE LINE and remembered as clicked there:
    // the constraint pressed after it decides whether the line or its middle is meant (`middles_for`). Picked as a
    // point at once, the middle took every click a person makes in the middle of a line, and constraints between lines
    // were laid between their midpoints.
    let middle = match hit {
        Some((1, eid)) => sk
            .snap_hint
            .filter(|&(_, kind)| kind == 3)
            .and_then(|(at, _)| line_with_middle_at(&*sk.project, si, at))
            .and_then(|ends| (line_ends_of(&*sk.project, si, eid) == Some(ends)).then_some(eid)),
        _ => None,
    };
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
    // THE MIRROR'S SECOND STEP: the click NAMES THE AXIS instead of adding to a selection.
    //
    // It stands before the ordinary selection on purpose. In this state the person has already said what to
    // reflect; letting the click fall through to the selection would quietly replace the answer to the first
    // question with an answer to the second.
    if !sk.sel_sk.mirror_of.is_empty() {
        match hit {
            Some((3, w)) => qymcad_ui_state::mirror_about_axis(qymcad_ui_state::editing_in!(sk), &mut *sk.sel_sk, w as usize),
            Some((1, eid)) => match sk.project.sketches.get(si).and_then(|s| s.entities.iter().find(|e| e.id == eid)).map(|e| e.kind) {
                // CONSTRUCTION GEOMETRY SERVES AS WELL: what matters is that two points make a line, and a
                // construction line is exactly what people draw to mirror about.
                Some(qymcad_core::model::EntityKind::Line { a, b }) => qymcad_ui_state::mirror_about_line(qymcad_ui_state::editing_in!(sk), &mut *sk.sel_sk, a, b),
                _ => *sk.status = qymcad_i18n::tr("sk-mirror-pick-axis"),
            },
            _ => *sk.status = qymcad_i18n::tr("sk-mirror-pick-axis"),
        }
        return;
    }
    if offset_side_click(sk, hit, qymcad_ui_state::to_world(&*sk.view, rect, pos)) {
        return;
    }
    match hit {
        Some(refr) => {
            if !additive {
                sk.sel_sk.clear(); // the selection and whatever was waiting for it
            }
            if let Some(p) = sk.sel_sk.items.iter().position(|r| *r == refr) {
                sk.sel_sk.items.remove(p); // a second click deselects it
                sk.sel_sk.at_middle.retain(|&e| e != refr.1);
            } else {
                sk.sel_sk.items.push(refr);
                if let Some(eid) = middle {
                    sk.sel_sk.at_middle.push(eid);
                    *sk.status = qymcad_i18n::tr("sk-midpoint-picked");
                }
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
    // THE OFFSET GATHERS ITS CURVES: a click adds to what it will offset, and Enter offsets them (`sketch_tool_keys`)
    if sk.sel_sk.modify == Some(qymcad_ui_state::EditTool::Offset) {
        if sk.sel_sk.items.iter().any(|(k, _)| *k == 1) {
            *sk.status = qymcad_i18n::tr("sk-offset-side");
        }
    } else if let Some(op) = sk.sel_sk.modify {
        if qymcad_ui_state::try_modify(qymcad_ui_state::editing_in!(sk), &mut *sk.sel_sk, *sk.sk_pat, &*sk.tool_prefs, op) {
            sk.sel_sk.modify = None;
        }
    }
}

/// THE SIDE OF AN OFFSET FIXED BY A CLICK: with curves chosen, a click that names no curve fixes the side of the copy
/// where it is, and the box of the distance opens beside the copy (`offset_box`). A click on a curve adds it, as before.
fn offset_side_click(sk: &mut qymcad_ui_state::SketchCtx, hit: Option<(u8, Id)>, at: Point2) -> bool {
    if sk.sel_sk.modify != Some(qymcad_ui_state::EditTool::Offset) || matches!(hit, Some((1, _))) || !sk.sel_sk.items.iter().any(|(k, _)| *k == 1) {
        return false;
    }
    sk.sel_sk.offset = qymcad_ui_state::OffsetAim { at: Some(at), buf: qymcad_core::expr::fmt_num(sk.tool_prefs.offset), focus: true };
    *sk.status = qymcad_i18n::tr("sk-offset-type");
    true
}

/// WHAT THE OFFSET MAKES ITS COPY WITH: the document, its undo, the curves chosen and the tool's state, the distance.
pub struct OffsetCtx<'a> {
    pub project: &'a mut qymcad_core::model::Project,
    pub edits: &'a mut qymcad_ui_state::Edits,
    pub regen: &'a mut qymcad_ui_state::Rebuilding,
    pub sel_sk: &'a mut qymcad_ui_state::SketchSelection,
    pub status: &'a mut String,
    pub tool_prefs: &'a mut qymcad_ui_state::SketchToolPrefs,
}

/// THE CURVES THE OFFSET HOLDS: the lines, arcs and circles chosen.
fn offset_curves(sel_sk: &qymcad_ui_state::SketchSelection) -> Vec<Id> {
    sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect()
}

/// MAKE THE COPY: the curves chosen offset by the distance `dist` of the tool on the side of `toward`, one step of undo.
/// The tool stays in hand for the next curves, its choice and its side let go; nothing it can offset says what it takes.
/// It used to put the curves down and keep the tool's name on the bar - the bar said Offset, a click chose as the arrow
/// does, and Enter did nothing: "the tool works every other time".
fn offset_make(oc: OffsetCtx, si: usize, dist: f64, toward: Option<Point2>) {
    let eids = offset_curves(oc.sel_sk);
    let dist = toward.map_or(dist, |w| oc.project.offset_toward(si, &eids, dist, w));
    qymcad_ui_state::begin_edit(&mut *oc.edits, &*oc.project, qymcad_i18n::tr("tool-offset"));
    let made = oc.project.offset_entities(si, &eids, dist);
    qymcad_ui_state::close_edit(&mut *oc.edits, &*oc.project);
    if made > 0 {
        qymcad_ui_state::invalidate(&mut *oc.regen);
        oc.sel_sk.items.clear();
        oc.sel_sk.at_middle.clear();
        oc.sel_sk.offset = qymcad_ui_state::OffsetAim::default();
        *oc.status = qymcad_i18n::tr("sk-offset-made");
    } else {
        *oc.status = qymcad_i18n::tr("sk-offset-takes");
    }
}

/// THE OFFSET MADE BY ENTER before a click has fixed the side: on the side of the pointer, by the distance of the bar,
/// Enter in the field of the bar included.
fn offset_on_enter(sk: &mut qymcad_ui_state::SketchCtx, ctx: &egui::Context, rect: Rect) {
    let qymcad_ui_state::Sel::Sketch(si) = *sk.sel else { return };
    if sk.sel_sk.modify != Some(qymcad_ui_state::EditTool::Offset) || sk.sel_sk.offset.at.is_some() || offset_curves(sk.sel_sk).is_empty() {
        return;
    }
    let pressed = !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Enter));
    if !pressed && !qymcad_ui_state::bar_enter_take(ctx) {
        return;
    }
    if qymcad_ui_state::bar_field_bad("sk_offset") {
        *sk.status = qymcad_i18n::tr("sk-offset-field-bad");
        return;
    }
    let pointer = ctx.input(|i| i.pointer.hover_pos()).filter(|p| rect.contains(*p)).map(|p| qymcad_ui_state::to_world(&*sk.view, rect, p));
    let dist = sk.tool_prefs.offset;
    offset_make(
        OffsetCtx { project: &mut *sk.project, edits: &mut *sk.edits, regen: &mut *sk.regen, sel_sk: &mut *sk.sel_sk, status: &mut *sk.status, tool_prefs: &mut *sk.tool_prefs },
        si,
        dist,
        pointer,
    );
}

/// THE BOX OF THE DISTANCE OF AN OFFSET whose side a click has fixed: beside the copy, the distance of the bar in it and
/// the keyboard given to it. What is typed is the bar's distance too, and the preview follows it; Enter or the tick
/// makes the copy, Esc or the cross frees the side and the copy follows the pointer again.
pub fn offset_box(oc: OffsetCtx, view: &qymcad_ui_state::View2d, si: usize, ctx: &egui::Context, rect: Rect) {
    let Some(at) = oc.sel_sk.offset.at.filter(|_| oc.sel_sk.modify == Some(qymcad_ui_state::EditTool::Offset)) else { return };
    let eids = offset_curves(oc.sel_sk);
    if eids.is_empty() {
        oc.sel_sk.offset = qymcad_ui_state::OffsetAim::default();
        return;
    }
    // the box stands off the point of the copy nearest the click, on the side away from the source
    let dist = oc.project.offset_toward(si, &eids, oc.tool_prefs.offset, at);
    let sh = qymcad_ui_state::Sheet { view: *view, rect };
    let near = oc.project.offset_preview(si, &eids, dist).into_iter().flatten().min_by(|a, b| a.dist(at).total_cmp(&b.dist(at))).unwrap_or(at);
    let pos = qymcad_ui_state::clamp_popup(sh.at(near) + egui::vec2(OFFSET_BOX_GAP, OFFSET_BOX_GAP), rect);
    let want_focus = std::mem::take(&mut oc.sel_sk.offset.focus);
    let mut buf = std::mem::take(&mut oc.sel_sk.offset.buf);
    let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
    let (mut apply, mut cancel) = (false, false);
    let why = |project: &qymcad_core::model::Project, text: &str| match qymcad_core::expr::eval(text.trim(), &project.param_map()) {
        Err(e) => Some(qymcad_i18n::error_words::expr_error_text(&e)),
        Ok(v) if !v.is_finite() || v.abs() <= 1e-6 => Some(qymcad_i18n::tr("cmd-value-zero")),
        Ok(_) => None,
    };
    egui::Area::new(egui::Id::new(("offsetbox", si))).fixed_pos(pos).order(egui::Order::Foreground).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(qymcad_i18n::tr("opt-distance"));
                let r = qymcad_ui_state::focus_edit(ui, &mut buf, 64.0, "", want_focus);
                let said = why(&*oc.project, &buf);
                if r.changed() && said.is_none() {
                    oc.tool_prefs.offset = parse_num(&*oc.project, &buf).unwrap_or(oc.tool_prefs.offset);
                }
                if let Some(w) = &said {
                    ui.label(egui::RichText::new(ph::WARNING).color(ui.visuals().warn_fg_color)).on_hover_text(w);
                }
                apply |= enter && (r.has_focus() || r.lost_focus());
                apply |= ui.button(ph::CHECK).clicked();
                cancel |= ui.button(ph::X).clicked();
            });
        });
    });
    let refused = why(&*oc.project, &buf);
    oc.sel_sk.offset.buf = buf;
    if apply && refused.is_some() {
        *oc.status = format!("{} {}", ph::WARNING, refused.unwrap_or_default());
    } else if apply {
        let d = oc.tool_prefs.offset;
        offset_make(oc, si, d, Some(at));
    } else if cancel || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        oc.sel_sk.offset = qymcad_ui_state::OffsetAim::default();
        *oc.status = qymcad_i18n::tr("sk-offset-side");
    }
}

/// How far the box of the offset distance stands off the copy, in pixels, right and down.
const OFFSET_BOX_GAP: f32 = 16.0;

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
            EntityKind::Arc { center, a, .. } => qymcad_ui_state::sketch_pt(&*sk.project, si, a)
                .zip(qymcad_ui_state::sketch_pt(&*sk.project, si, center))
                .map(|(pa, c)| (center, ((pa.x - c.x).powi(2) + (pa.y - c.y).powi(2)).sqrt(), true)),
            _ => None,
        });
        if let (Some((_center, r, is_arc)), Some(cp)) = (info, info.and_then(|(c, ..)| qymcad_ui_state::sketch_pt(&*sk.project, si, c))) {
            let at = (qymcad_ui_state::Sheet { view: *sk.view, rect }).at(Point2::new(cp.x + r, cp.y));
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
                buf = qymcad_core::expr::fmt_num(if is_arc { r } else { 2.0 * r });
                // a radius for an arc, a diameter for a circle
                sk.dim.typed = false;
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
            // APPLIED ON COMMIT, NOT ON EVERY LETTER, exactly as a linear dimension is.
            //
            // Reported behaviour: "you place a dimension, try to type it in, and while you are erasing, 100
            // becomes 1 - and at that moment the geometry gets thrown about". The linear field had already
            // learnt this; the field one over, the radius and diameter of a circle, still rebuilt the
            // circle on every keystroke, so erasing "100" really did build it at 10 and at 1 on the way.
            //
            // The value is evaluated when an expression is typed, and stored in the arc's radius dimension - put
            // there by the tool when the arc had none.
            // the model is not touched while the text is being typed - see below; only a value typed is laid
            sk.dim.typed |= buf_changed;
            if close && sk.dim.typed {
                match parse_num(&*sk.project, &buf) {
                    // A radius of zero or less is not a radius. It used to be clamped to 0.01 silently,
                    // which answers a typing slip with a circle nobody can see.
                    Some(v) if (if is_arc { v } else { v * 0.5 }) > 0.0 => {
                        rr = if is_arc { v } else { v * 0.5 }; // Ø -> r
                        chg = true;
                    }
                    Some(_) => {
                        *sk.status = qymcad_i18n::tr("sk-dim-must-be-positive");
                        close = false; // the field stays open on the text that was refused
                    }
                    None => {}
                }
            }
            if chg {
                // THE SIZE TYPED IS ONE STEP OF UNDO, named as a dimension is
                qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, qymcad_i18n::tr("sk-dim"));
                if is_arc {
                    // a fillet arc has its radius edited through the dimension constraint (parametrically);
                    // failing that, the fillet is recomputed geometrically; failing that, it is a plain arc
                    if !sk.project.set_fillet_radius_dim(si, eid, rr) && !sk.project.set_fillet_radius(si, eid, rr) {
                        sk.project.put_arc_radius_dim(si, eid, rr); // a plain arc gets its dimension, as a circle has
                    }
                } else {
                    // A DIAMETER TYPED IS LAID AS A DIMENSION, of the value typed
                    let centre = sk.project.sketches.get_mut(si).and_then(|s| s.entities.iter_mut().find(|e| e.id == eid)).and_then(|e| match &mut e.kind {
                        EntityKind::Circle { r, center } => {
                            *r = rr;
                            Some(*center)
                        }
                        _ => None,
                    });
                    if let Some(ci) = centre.and_then(|c| sk.project.ensure_diameter(si, c, true)) {
                        if let Some(Constraint::Diameter { d, .. }) = sk.project.sketches[si].constraints.get_mut(ci) {
                            *d = 2.0 * rr;
                        }
                    }
                    // THE SOLVER, not a rebuild. The radius of a circle is a variable of the solve (it is
                    // keyed by the centre), so a radius written straight into the entity is a value the
                    // solver has not seen: a tangency or an equality holding this circle would go on being
                    // drawn as satisfied while it no longer is.
                    sk.project.solve_sketch(si);
                }
                qymcad_ui_state::invalidate(&mut *sk.regen);
                qymcad_ui_state::commit_edit_if_changed(&mut sk.rebuild());
            }
            if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                sk.inline.clear();
                sk.dim.buf.clear();
                sk.dim.focus = false;
                sk.dim.typed = false;
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
        Some(Constraint::Distance { d, expr, driven, .. }) | Some(Constraint::Diameter { d, expr, driven, .. }) | Some(Constraint::EdgeDistance { d, expr, driven, .. }) => {
            (*d, expr.clone(), driven, false)
        }
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
    let label = if is_angle {
        qymcad_i18n::tr("sk-angle")
    } else if is_diameter {
        if diam_mode { "Ø" } else { "R" }.to_string()
    } else if is_edge {
        qymcad_i18n::tr("sk-tangent-short")
    } else {
        qymcad_i18n::tr("sk-dim")
    };
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
                let ok = |nm: &str| nm.is_empty() || (qymcad_core::drivers::check_ident(nm).is_ok() && !sk.project.driver_name_taken(nm, sid, &key_refs));
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
    // WHAT THE FIELD DOES TO THE DOCUMENT IS ONE OPERATION, named: the value of a dimension just made joins the step
    // that made it; any other change of a dimension is a step of its own rather than an unnamed one the frame notices
    let fold = sk.dim.fresh.is_some_and(|(fc, steps)| fc == ci && steps == sk.edits.undo.len());
    // THE TEXT DIFFERS FROM WHAT IS STORED, so there is something to apply on commit. TEXTS are compared
    // rather than the number parsed again: parsing a value in the sketcher must go through one door
    // (`parse_num`), and a second `parse::<f64>()` is caught at once by the ratchet in `expr_fields.rs`.
    let shown_before = if cur_expr.trim().is_empty() { qymcad_core::expr::fmt_num(cur_val) } else { cur_expr.clone() };
    let value_differs = buf.trim() != shown_before.trim();
    // A VALUE THAT CANNOT BE TAKEN KEEPS THE FIELD OPEN with what was typed in it, and the status line says why: an
    // empty field, or a formula that does not evaluate - an unknown name included, since a name that is not there
    // yet would hold the dimension at a value nobody typed
    if close && !is_driven && value_differs {
        let why = match &eval_res {
            None => Some(qymcad_i18n::tr("sk-dim-value-empty")),
            Some(Err(e)) => Some(qymcad_i18n::error_words::expr_error_text(e)),
            Some(Ok(_)) => None,
        };
        if let Some(why) = why {
            close = false;
            *sk.status = why;
        }
    }
    let apply_value = close && !is_driven && value_differs;
    let acting = !fold && (apply_value || toggle_driven || toggle_diam || toggle_edge);
    if acting {
        qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, qymcad_i18n::tr("sk-dim"));
    }
    // apply the changes to the constraint
    let mut new_buf: Option<String> = None; // refresh the field after a value edit (a diameter/radius swap, say)
    if let Some(c) = sk.project.sketches[si].constraints.get_mut(ci) {
        if toggle_driven {
            match c {
                Constraint::Distance { driven, .. }
                | Constraint::DistancePL { driven, .. }
                | Constraint::Angle { driven, .. }
                | Constraint::Diameter { driven, .. }
                | Constraint::AngleLines { driven, .. }
                | Constraint::ArcLength { driven, .. }
                | Constraint::EdgeDistance { driven, .. } => *driven = !*driven,
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
    }
    // THE VALUE IS APPLIED ON COMMIT, NOT ON EVERY LETTER.
    //
    // Every keystroke used to edit the constraint, solve the sketch again and mark the document for a
    // rebuild: typing "125" meant three rebuilds, and on the intermediate "1" and "12" the sketch honestly
    // rebuilt itself to a different size. The answer is visible anyway - the line "= 42.500" lives under
    // the field, and it costs the document nothing.
    //
    // AND A LENGTH THAT CANNOT EXIST IS REFUSED rather than handed over - see `set_dim_value`. A typed
    // EXPRESSION is judged by what it evaluates to now: it may change later with its parameter, and that is
    // the parameter's business, but a formula that reads -5 today is refused today.
    if apply_value {
        let t = buf.trim();
        // THE ONE BARE PARSE OF THE SKETCHER, and it is asked once: is this a number or a formula? The
        // answer decides both what value goes in and whether the formula is kept beside it.
        let as_number = t.parse::<f64>().ok();
        match as_number.or_else(|| parse_num(&*sk.project, t)) {
            Some(v) => {
                let took = set_dim_value(&mut *sk.project, si, ci, v);
                if took.is_ok() {
                    if as_number.is_none() {
                        // a formula keeps its text beside the value it evaluates to now
                        if let Some(e) = sk.project.sketches[si].constraints.get_mut(ci).and_then(Constraint::expr_mut) {
                            *e = buf.clone();
                        }
                    }
                    changed = true;
                } else if let Err(key) = took {
                    // refused: the field stays open with what was typed, and the status line says why
                    *sk.status = qymcad_i18n::tr1(key, "max", &qymcad_i18n::num(MAX_SKETCH_LENGTH, 0));
                    close = false;
                }
            }
            // not a number and not a formula that evaluates yet - the text is kept, and it will be read
            // again when the parameter it names appears
            None => {
                if let Some(e) = sk.project.sketches[si].constraints.get_mut(ci).and_then(Constraint::expr_mut) {
                    *e = buf.clone();
                    changed = true;
                }
            }
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
    // the value typed into a dimension just made is part of making it
    if changed && fold {
        qymcad_ui_state::fold_into_last_step(&mut *sk.edits, &*sk.project);
    }
    if acting {
        qymcad_ui_state::commit_edit_if_changed(&mut sk.rebuild());
    }
    if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        sk.inline.clear();
        sk.dim.buf.clear();
        sk.dim.focus = false;
        sk.dim.fresh = None;
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
        7 => &qymcad_i18n::tr("sk-slot"),
        8 => &qymcad_i18n::tr("sk-ellipse"),
        9 => &qymcad_i18n::tr("sk-spline"),
        10 => &qymcad_i18n::tr("sk-circle"), // a circle through three points is a circle
        11 => &qymcad_i18n::tr("sk-text"),
        _ => &qymcad_i18n::tr("sk-drawing"),
    };
    qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, name);
    sketch_tool_click_inner(sk, rect, pos);
    qymcad_ui_state::commit_edit_if_changed(&mut sk.rebuild());
}

/// WHICH SIDE OF PUTTING A RECTANGLE `tie_the_rect_to_what_is_under` STANDS ON.
#[derive(Clone, Copy, PartialEq)]
enum RectPut {
    /// before it is made: the edges of the body under the clicks taken in
    Before,
    /// after: its points at the clicks tied to what they landed on
    After,
}

/// A RECTANGLE CLICKED AT `clicks` TIED TO WHAT IS UNDER THEM, as the ends of a line are, when the automatic constraints
/// are on and it is not construction.
fn tie_the_rect_to_what_is_under(sk: &mut qymcad_ui_state::SketchCtx, rect: Rect, si: usize, clicks: &[Point2], put: RectPut) {
    if !tied(sk) {
        return;
    }
    match put {
        RectPut::Before => {
            for at in clicks {
                auto_project_under(sk, rect, si, *at);
            }
        }
        RectPut::After => {
            let own: Vec<Id> = sk.project.sketches[si].rects.last().map(|r| r.corners.iter().copied().chain([r.centre]).collect()).unwrap_or_default();
            tie_the_clicked_points(&mut *sk.project, *sk.view, si, clicks, &own);
        }
    }
}

/// WHETHER WHAT IS DRAWN NOW IS TIED TO WHAT IT LANDS ON (`qymcad_ui_state::ties_laid`).
fn tied(sk: &qymcad_ui_state::SketchCtx) -> bool {
    qymcad_ui_state::ties_laid(sk.tool, sk.set)
}

/// Where the two ends of line `seg` of sketch `si` stand, first end first.
fn segment_ends(project: &Project, si: usize, seg: Id) -> Option<[Point2; 2]> {
    let s = project.sketches.get(si)?;
    let qymcad_core::model::EntityKind::Line { a, b } = s.entities.iter().find(|e| e.id == seg)?.kind else { return None };
    let at = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| Point2::new(q.x, q.y));
    Some([at(a)?, at(b)?])
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
            if sk.tool.pts.last().is_some_and(|last| same_place(sk, *last, w)) {
                return; // the second click of the double click that ends the chain: no segment of no length
            }
            if let Some(&last) = sk.tool.pts.last() {
                let prev = (sk.tool.pts.len() >= 2).then(|| sk.tool.pts[sk.tool.pts.len() - 2]);
                // an end put on an edge of the body under the sketch takes the edge in first, so the end is tied to it
                if tied(sk) {
                    if prev.is_none() {
                        auto_project_under(sk, rect, si, last);
                    }
                    auto_project_under(sk, rect, si, w);
                }
                let seg = sk.project.add_line_entity(si, last.x, last.y, w.x, w.y, qymcad_core::feature::Purpose::of(con));
                if tied(sk) {
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
                if sk.tool.ties == qymcad_ui_state::Ties::Laid {
                    sk.project.merge_close_points(si, tol);
                }
                qymcad_ui_state::invalidate(&mut *sk.regen);
                // THE CHAIN GOES ON FROM ITS CORNERS AS THEY STAND: an automatic constraint and the solve after it move
                // the ends of the segment just laid (a corner squared by a Perpendicular), and kept at the clicks, the
                // dots of the chain stood a few pixels beside its corners and the next segment was led from a click
                // rather than from the corner.
                if let Some([a, b]) = segment_ends(sk.project, si, seg) {
                    if let Some(l) = sk.tool.pts.last_mut() {
                        *l = a;
                    }
                    sk.tool.pts.push(b);
                    return;
                }
            }
            sk.tool.pts.push(w);
        }
        2 => {
            sk.tool.pts.push(w);
            let need = if sk.tool_prefs.rect_mode == 2 { 3 } else { 2 };
            if sk.tool.pts.len() == need {
                // Centre plus a corner keeps the centre while the width and height are typed; two
                // corners keep the first corner. A rotated rectangle (three points) offers no typing.
                match sk.tool_prefs.rect_mode {
                    1 => {
                        // centre plus a corner: the opposite corner is its mirror through the centre, and the rectangle keeps the centre
                        let (c, cr) = (sk.tool.pts[0], sk.tool.pts[1]);
                        tie_the_rect_to_what_is_under(sk, rect, si, &[c, cr], RectPut::Before);
                        let ids = sk.project.add_rect_from_centre(si, c, cr, qymcad_core::feature::Purpose::of(con));
                        tie_the_rect_to_what_is_under(sk, rect, si, &[c, cr], RectPut::After);
                        sk.tool.pts.clear();
                        qymcad_ui_state::invalidate(&mut *sk.regen);
                        sk.place.set(qymcad_ui_state::PlacingShape::RectCenter { center: c, corner: cr, ids }); // typing the width and height about the centre
                        sk.place.focus = true;
                    }
                    2 => {
                        // three points give a rotated rectangle
                        let (p1, p2, p3) = (sk.tool.pts[0], sk.tool.pts[1], sk.tool.pts[2]);
                        sk.project.add_rect3_entity(si, Point2::new(p1.x, p1.y), Point2::new(p2.x, p2.y), Point2::new(p3.x, p3.y), qymcad_core::feature::Purpose::of(con));
                        sk.tool.pts.clear();
                        qymcad_ui_state::invalidate(&mut *sk.regen);
                    }
                    _ => {
                        let (a, b) = (sk.tool.pts[0], sk.tool.pts[1]);
                        tie_the_rect_to_what_is_under(sk, rect, si, &[a, b], RectPut::Before);
                        let ids = sk.project.add_rect_entity(si, a.x, a.y, b.x, b.y, qymcad_core::feature::Purpose::of(con));
                        tie_the_rect_to_what_is_under(sk, rect, si, &[a, b], RectPut::After);
                        sk.tool.pts.clear();
                        qymcad_ui_state::invalidate(&mut *sk.regen);
                        sk.place.set(qymcad_ui_state::PlacingShape::Rect { a, b, ids }); // typing the width and height from the first corner
                        sk.place.focus = true;
                    }
                };
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
                    open_the_size_of_the_new_shape(sk, eid, qymcad_core::feature::Purpose::of(con));
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
                // the centre clicked on an edge of the body under the sketch takes the edge in, and is tied to it
                let centre_clicked = sk.tool_prefs.circ_mode != 1 && tied(sk);
                if centre_clicked {
                    auto_project_under(sk, rect, si, p0);
                }
                let eid = sk.project.add_circle_entity(si, cx, cy, r, qymcad_core::feature::Purpose::of(con));
                if centre_clicked {
                    let centre: Vec<Id> = sk.project.sketches[si].entities.iter().find(|e| e.id == eid).map(qymcad_core::model::entity_points).unwrap_or_default();
                    tie_the_clicked_points(&mut *sk.project, *sk.view, si, &[p0], &centre);
                }
                sk.tool.pts.clear();
                qymcad_ui_state::invalidate(&mut *sk.regen);
                open_the_size_of_the_new_shape(sk, eid, qymcad_core::feature::Purpose::of(con));
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
                        open_the_size_of_the_new_arc(sk, si, qymcad_core::feature::Purpose::of(con));
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
                        open_the_size_of_the_new_arc(sk, si, qymcad_core::feature::Purpose::of(con));
                    } else {
                        *sk.status = qymcad_i18n::tr("sk-points-collinear-arc");
                    }
                } else {
                    // the centre, the start and the end
                    let (c, a, b) = (p0, p1, p2);
                    let ccw = (a.x - c.x) * (b.y - c.y) - (a.y - c.y) * (b.x - c.x) > 0.0;
                    sk.project.add_arc_entity(si, Point2::new(c.x, c.y), Point2::new(a.x, a.y), Point2::new(b.x, b.y), winding(ccw), qymcad_core::feature::Purpose::of(con));
                    open_the_size_of_the_new_arc(sk, si, qymcad_core::feature::Purpose::of(con));
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
            if sk.tool.pts.len() == 2 && sk.tool.pts[0].dist(sk.tool.pts[1]) < 1e-3 {
                // A CORNER AT THE CENTRE is no polygon: refused in words, the centre kept for the next click. The core
                // stretched such a radius to 0.01 and laid twelve corners on one spot, and finishing it ran the memory
                // past 1.5 GB (a second polygon centred on the corner of the first).
                sk.tool.pts.pop();
                *sk.status = qymcad_i18n::tr("sk-polygon-no-size");
                return;
            }
            if sk.tool.pts.len() == 2 {
                let n = sk.tool_prefs.poly_n.max(3);
                let (c, v) = qymcad_ui_state::polygon_from_clicks(sk.tool.pts[0], sk.tool.pts[1], n, sk.tool_prefs.poly_mode);
                // A PARAMETRIC regular polygon: the circle through its vertices plus constraints
                let (center, _sides) = sk.project.add_polygon_param(si, c, v, n, qymcad_core::feature::Purpose::of(con));
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
            // an ellipse: the centre, the end of the major semi-axis (its length and direction), then a point that
            // sets the minor semi-axis by its distance from the major axis. Two clicks (centre and a corner of the
            // bounding box) could only draw an axis-aligned ellipse, and the entity was laid on the second click,
            // so a third click started a new one.
            sk.tool.pts.push(w);
            if sk.tool.pts.len() == 3 {
                let (cc, a, e) = (sk.tool.pts[0], sk.tool.pts[1], sk.tool.pts[2]);
                let (major, rot, minor) = qymcad_ui_state::ellipse_from_clicks(cc, a, e);
                let center = sk.project.add_ellipse_entity(si, Point2::new(cc.x, cc.y), major.max(0.5), minor.max(0.5), rot, qymcad_core::feature::Purpose::of(con));
                sk.tool.pts.clear();
                qymcad_ui_state::invalidate(&mut *sk.regen);
                sk.place.set(qymcad_ui_state::PlacingShape::Ellipse(center, cc)); // typing the semi-axes through the centre handle
                sk.place.focus = true;
            }
        }
        9 => {
            // a spline: a set of nodes (a double click or Esc finishes it). The second click of the double click
            // lands on the node just put down and adds nothing.
            if !sk.tool.pts.last().is_some_and(|last| same_place(sk, *last, w)) {
                sk.tool.pts.push(w);
            }
        }
        10 => {
            // a circle through three points
            sk.tool.pts.push(w);
            if sk.tool.pts.len() == 3 {
                if let Some((cx, cy, r)) = qymcad_ui_state::circumcircle(sk.tool.pts[0], sk.tool.pts[1], sk.tool.pts[2]) {
                    let eid = sk.project.add_circle_entity(si, cx, cy, r, qymcad_core::feature::Purpose::of(con));
                    qymcad_ui_state::invalidate(&mut *sk.regen);
                    open_the_size_of_the_new_shape(sk, eid, qymcad_core::feature::Purpose::of(con));
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
                *sk.armed = qymcad_ui_state::Armed::None; // placed once, as a label is
                sk.tool_prefs.text.clear();
            }
        }
        11 if sk.tool_prefs.text.trim().is_empty() => {
            // AN EMPTY STRING IS NO REASON TO STAY SILENT. The tool was armed, the click went through, and
            // nothing happened: there was no telling whether the program was broken or something had been
            // left undone. Emptied by the font, which could not write the letters typed, it is the font that is
            // named.
            *sk.status = if sk.tool.text_refused {
                qymcad_i18n::tr1("sk-text-no-letters", "name", &qymcad_ui_state::font_label(&sk.tool_prefs.font, "opt-font"))
            } else {
                qymcad_i18n::tr("sk-text-needs-string")
            };
        }
        11 => {
            // A CLICK ON A LABEL THAT IS ALREADY THERE PLACES NOTHING. The tool is in hand while a label is edited, and
            // the next thing a person does to fix a typo is double-click the label: each click of that double click
            // would place a label of its own on top of it - three labels where there was one.
            // The double click itself opens the editor, in the frame of its second click.
            //
            // Not opened here, on the first click: the second click of the double click lands outside the field
            // that has just taken the focus, egui takes the focus away, and the editor opens with no caret in it.
            if qymcad_ui_state::text_at(&*sk.project, &*sk.view, rect, pos, si).is_some() {
                *sk.status = qymcad_i18n::tr("sk-text-on-label");
                return;
            }
            // text AS GEOMETRY: a parametric object (the outlines of the glyphs) that can be selected,
            // moved, scaled and retyped - not the loose contours it used to be
            // THE FONT TRAVELS WITH THE LABEL. Without it a later edit re-bakes in whatever the application
            // has in hand, which after a reopen is the system font: the label changes typeface with nobody
            // asked and nobody told.
            let font = sk.font_cache.for_tool(&mut sk.tool_prefs.font).map(|(f, _)| f).unwrap_or_default();
            let glyphs = qymcad_ui_state::bake_text_glyphs(&mut *sk.font_cache, &font, qymcad_ui_state::GlyphPlace { at: w, height: sk.tool_prefs.text_h, angle: 0.0 }, &sk.tool_prefs.text.clone());
            let n = glyphs.len();
            if n > 0 {
                let id = sk.project.add_sketch_text(
                    si,
                    qymcad_core::model::TextSpec { at: w, height: sk.tool_prefs.text_h, angle: 0.0, text: sk.tool_prefs.text.clone(), glyphs, font },
                    qymcad_core::feature::Purpose::of(sk.tool.construction),
                );
                sk.annot.text = sk.project.sketches[si].texts.iter().position(|t| t.id == id);
                qymcad_ui_state::invalidate(&mut *sk.regen);
                sk.view.initialized = false;
                *sk.status = qymcad_i18n::tr1("sk-text-placed", "n", &n.to_string());
                // A LABEL IS PLACED ONCE: the tool is put down, as in the CAD programs people know. Kept in hand it laid
                // a copy of the label on every stray click until Esc (found checking issue #32). Its string goes too: the
                // tool taken again offered it, and a click placed the same label once more; the height and the font
                // stay as they were set.
                *sk.armed = qymcad_ui_state::Armed::None;
                sk.tool_prefs.text.clear();
            } else if !qymcad_ui_state::any_font_at_all(&mut *sk.font_cache) {
                *sk.status = qymcad_i18n::tr("sk-font-not-found");
            } else if !qymcad_ui_state::font_can_write(&mut *sk.font_cache, &font, &sk.tool_prefs.text) {
                // THE FONT IS NAMED, because the string is not the one at fault. The list marks a face that
                // cannot write its own name, which is a different question: a font with Latin and no Cyrillic
                // looks good there and writes nothing at all in Russian. Here the string is known.
                *sk.status = qymcad_i18n::tr1("sk-text-no-letters", "name", &qymcad_ui_state::font_label(&font, "opt-font"));
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
    // A CHORD OF THE LEFT BUTTON WITH ANOTHER moves or zooms the sheet by the layout: the sketch starts nothing of its own
    if qymcad_ui_state::Gesture::chord_held(ctx) {
        return;
    }
    // A DRAWING TOOL IN HAND TAKES NO DRAG: picking up a point, a dimension or the selection, and the selection box, are
    // the work of selection mode. Reported behaviour (issue #33): with Line in hand a drag over empty space selected
    // what lay in the band, and a drag from a point moved it.
    if sk.armed.draw_kind() != 0 {
        return;
    }
    let sh = qymcad_ui_state::Sheet { view: *sk.view, rect };
    if resp.drag_started() && ctx.input(|i| i.pointer.button_down(egui::PointerButton::Primary)) {
        // WHAT IS TAKEN IS WHAT WAS PRESSED ON. egui decides that a press is a drag once the pointer is
        // 6 px away from it, and a point is taken within 8 px: looked for under the pointer at that
        // moment, the grab depended on the speed of the hand - at 3 px a frame the drag was decided at
        // 8.5 px and the view panned instead of the point moving.
        let pp = ctx.input(|i| i.pointer.press_origin()).or(resp.interact_pointer_pos());
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
                    if matches!(
                        sk.project.sketches[si].constraints.get(ci),
                        Some(qymcad_core::model::Constraint::Distance { .. })
                            | Some(qymcad_core::model::Constraint::DistancePL { .. })
                            | Some(qymcad_core::model::Constraint::Diameter { .. })
                            | Some(qymcad_core::model::Constraint::ArcLength { .. })
                            | Some(qymcad_core::model::Constraint::EdgeDistance { .. })
                            | Some(qymcad_core::model::Constraint::Angle { .. })
                            | Some(qymcad_core::model::Constraint::AngleLines { .. })
                    ) {
                        // THE DRAG OF A LABEL IS ONE STEP OF UNDO, closed on the release, as the drag of a point
                        // is: without the boundary Ctrl+Z after moving a label took back the operation before it
                        qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, qymcad_i18n::tr("sk-move-dim-label"));
                        *sk.drag = qymcad_ui_state::Dragging::Dim(ci);
                        sk.gsel.constraint = Some(ci); // grabbing a dimension selects it, even on a tiny drag
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
                    let mut arc_pts: std::collections::HashSet<Id> = sk.project.sketches[si]
                        .entities
                        .iter()
                        .flat_map(|e| match e.kind {
                            qymcad_core::model::EntityKind::Arc { center, a, b, .. } => vec![center, a, b],
                            _ => vec![],
                        })
                        .collect();
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
                    // THE CENTRE OF A RECTANGLE IS DRAGGED, and the rectangle goes with it as a circle goes with its centre: it
                    // stands on the middle of the diagonal, and was refused with the materialised midpoints above - the
                    // rectangle could not be taken by its centre. A centre pinned stays refused.
                    let pinned: std::collections::HashSet<Id> =
                        sk.project.sketches[si].constraints.iter().filter_map(|c| if let qymcad_core::model::Constraint::Fixed { p } = c { Some(*p) } else { None }).collect();
                    // AND SO IS THE CENTRE OF ANY SHAPE IT CARRIES (`carried_with`): an arc of its own, the ends of a slot.
                    // The centre of a fillet carries nothing and stays refused - the lines it touches hold it.
                    let s = &sk.project.sketches[si];
                    let centres: Vec<Id> = s
                        .rects
                        .iter()
                        .map(|r| r.centre)
                        .chain(s.entities.iter().filter_map(|e| if let qymcad_core::model::EntityKind::Arc { center, .. } = e.kind { Some(center) } else { None }))
                        .collect();
                    for c in centres {
                        if !pinned.contains(&c) && !s.carried_with(c).is_empty() {
                            arc_pts.remove(&c);
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
    // THE LEFT BUTTON ONLY: the sheet is moved with the middle one (or the gesture of the scheme), and a move of the
    // sheet with the trim in hand cut everything the pointer crossed. Reported behaviour: after a new project, where
    // the point had to be brought into view first, a click on the lower arc cut the line in two and failed.
    if resp.drag_started_by(egui::PointerButton::Primary) {
        sk.trim.path.clear();
        sk.trim.done.clear();
    }
    if !resp.dragged_by(egui::PointerButton::Primary) || qymcad_ui_state::Gesture::chord_held(&resp.ctx) {
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
        qymcad_ui_state::Sel::Mesh(i) if i < sk.project.bodies.len() => {
            qymcad_ui_state::move_body_at(&mut sk.rebuild(), i, qymcad_ui_state::mat_translate(dx, dy, 0.0));
            // a B-rep gets a Move feature; a raw mesh is simply shifted
        }
        _ => {}
    }
}

/// The reference object under the cursor: a point or centre > the midpoint of a line > the origin.
/// Used for dimensions and constraints between any geometry.
pub fn resolve_sketch_ref(sk: &mut qymcad_ui_state::SketchCtx, rect: Rect, pos: Pos2, si: usize) -> Option<qymcad_ui_state::SketchRef> {
    let sh = qymcad_ui_state::Sheet { view: *sk.view, rect };
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
    let o = (qymcad_ui_state::Sheet { view: *sk.view, rect }).at(Point2::new(0.0, 0.0));
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
    let sh = qymcad_ui_state::Sheet { view: *sk.view, rect };
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
                // AN ANGLE'S LABEL sticks to the pointer: the radius of the arc and the place along it, past a side if
                // it is led there. Reported behaviour: an angle could be placed but not moved afterwards, as a
                // length or a radius can.
                let angle = matches!(sk.project.sketches[si].constraints.get(ci), Some(Constraint::Angle { .. } | Constraint::AngleLines { .. }));
                if angle {
                    if let Some(placed) = resp.interact_pointer_pos().and_then(|pp| qymcad_ui_state::angle_dim_follow(&*sk.project, si, ci, &sh, &*sk.set, pp)) {
                        put_angle_label(&mut *sk.project, si, ci, placed);
                    }
                    qymcad_ui_state::invalidate(&mut *sk.regen);
                } else if let Some(Constraint::Diameter { .. }) = sk.project.sketches[si].constraints.get(ci) {
                    // the line turns to the pointer, and the text lies ON the line within the circle, on the
                    // shelf past it
                    if let Some((ang, t)) = resp.interact_pointer_pos().and_then(|pp| qymcad_ui_state::radial_text_follow(&*sk.project, si, ci, &sh, pp)) {
                        if let Some(Constraint::Diameter { off, at, .. }) = sk.project.sketches[si].constraints.get_mut(ci) {
                            (*off, *at) = (ang, t);
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
                        // an arc length: `off` runs along the radius through the middle of the arc, outwards
                        Some(Constraint::ArcLength { c, a, b, ccw, .. }) => {
                            match (qymcad_ui_state::sketch_pt(&*sk.project, si, c), qymcad_ui_state::sketch_pt(&*sk.project, si, a), qymcad_ui_state::sketch_pt(&*sk.project, si, b)) {
                                (Some(cp), Some(pa), Some(pb)) => {
                                    let (a0, a1) = ((pa.y - cp.y).atan2(pa.x - cp.x), (pb.y - cp.y).atan2(pb.x - cp.x));
                                    let tau = std::f64::consts::TAU;
                                    let mid = if ccw { a0 + (a1 - a0).rem_euclid(tau) / 2.0 } else { a0 - (a0 - a1).rem_euclid(tau) / 2.0 };
                                    let out = egui::vec2(mid.cos() as f32, -(mid.sin() as f32)); // the screen's y runs down
                                    (dl.x * out.x + dl.y * out.y) as f64
                                }
                                _ => 0.0,
                            }
                        }
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
                                                            // THE TEXT GOES ALONG THE LINE TOO: the part of the move along the dimension line carries the text
                                                            // there, past an arrow onto a shelf, while the part across it moves the line
                    let led = qymcad_ui_state::linear_text_led(&*sk.project, si, ci, &sh, sk.set, dl);
                    if let Some(Constraint::Distance { off, at, .. }) | Some(Constraint::DistancePL { off, at, .. }) | Some(Constraint::EdgeDistance { off, at, .. }) =
                        sk.project.sketches[si].constraints.get_mut(ci)
                    {
                        *off += dadd;
                        if led.is_some() {
                            *at = led;
                        }
                    } else if let Some(Constraint::ArcLength { off, .. }) = sk.project.sketches[si].constraints.get_mut(ci) {
                        *off += dadd;
                    }
                }
            }
        }
        if resp.drag_stopped() {
            sk.drag.clear();
            qymcad_ui_state::commit_edit_if_changed(&mut sk.rebuild());
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
                // without Shift the box REPLACES the selection; with Shift it adds to it. A TOOL WAITING FOR ITS CURVES is
                // given them by the box as by its clicks - added, the tool left in hand. Cleared with the selection, the
                // tool's mode went and its name stayed on the bar: Enter did nothing. Reported behaviour: "the tool is put
                // down when the geometry is chosen all at once with the left button held".
                let waiting = sk.sel_sk.constraint.is_some() || sk.sel_sk.modify.is_some();
                if !ctx.input(|i| i.modifiers.shift) && !waiting && matches!(*sk.sel, qymcad_ui_state::Sel::Sketch(_)) {
                    sk.sel_sk.clear(); // the selection and whatever was waiting for it
                }
                qymcad_ui_state::box_select(qymcad_ui_state::editing_in!(sk), &mut *sk.sel_sk, *sk.sketch_ses, rect, a, b);
                if sk.sel_sk.modify == Some(qymcad_ui_state::EditTool::Offset) && sk.sel_sk.items.iter().any(|(k, _)| *k == 1) {
                    *sk.status = qymcad_i18n::tr("sk-offset-side"); // what to do next, not how many were chosen
                }
            }
            sk.tree_sel.box_start = None;
        }
    } else if resp.dragged_by(egui::PointerButton::Primary) && sk.armed.draw_kind() == 0 {
        // the fallback pan is for the left button only, and not with a drawing tool in hand - the drawing would slide
        // away under the tool - a middle drag is served by the EXPLICIT
        // handler below (otherwise the middle button would pan twice, at double speed)
        let d = resp.drag_delta();
        sk.view.center.x -= d.x / sk.view.scale;
        sk.view.center.y += d.y / sk.view.scale;
    }
    // the points of a sketch being dragged hold its diagnostics to its shape until the release (`Caches::sk_dragged`)
    sk.cache.sk_dragged.set(match *sk.drag {
        qymcad_ui_state::Dragging::Point(si, _) | qymcad_ui_state::Dragging::Move(si, _) => Some(si),
        _ => None,
    });
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
    drop_abandoned_dim(sk);
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
                // a click on a shape follows the connected chain and opens the radius popup. While the popup is
                // open a click does not take the shape again: the popup stands at the shape, the click into its
                // field reached the canvas too, and taking the shape anew put the old radius back over what was
                // typed
                if sk.corner.at.is_some() {
                    return;
                }
                if let qymcad_ui_state::Sel::Sketch(si) = *sk.sel {
                    if let Some(eid) = qymcad_ui_state::entity_near(&sk.pick(), rect, pos, si) {
                        let comp = sk.project.connected_entities(si, eid);
                        sk.corner.at = Some((si, 0, false));
                        sk.corner.only = Some(comp.clone());
                        // THE CHOSEN LINES GO IN AS THE WHOLE SET, so the corners the shape is made of are drawn on it
                        // while its field is being answered: this command is the same act as naming them with Shift,
                        // said in one word. THE LINE CLICKED GOES FIRST, the rest in the order they were drawn: the
                        // box stands beside the first corner of the set, and taken in the order of a hash set it stood
                        // at the bottom of a rectangle one time and at the top of the sheet under the bar the next.
                        let mut lines: Vec<Id> = comp.iter().copied().filter(|&id| id != eid).collect();
                        lines.sort_unstable();
                        lines.insert(0, eid);
                        sk.corner.set.follow(&*sk.project, si, &lines);
                        sk.corner.pos = Some(pos);
                        sk.corner.shifted = false;
                        sk.corner.buf = qymcad_core::expr::fmt_num(sk.tool_prefs.fillet);
                        sk.corner.focus = true; // the tool stays held: after Enter it waits for the next shape
                    } else {
                        *sk.status = qymcad_i18n::tr("sk-click-shape-line");
                    }
                }
            } else if sk.armed.draw_kind() != 0 && !qymcad_ui_state::bar_fields_valid(ctx) {
                // A FIELD OF THE BAR THAT CANNOT BE TAKEN stops the click as it stops Enter: the shape would
                // be made with the last value the field held, not with what is written in it
                *sk.status = qymcad_i18n::tr("sk-fix-bar-field");
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
                // THE FILLET AND THE CHAMFER CUT A SET OF CORNERS, and the set is what the chosen lines
                // make - not a list of corners a person points at one after another. The field stands open
                // for the whole set and one answer cuts all of it; Shift is what lets a set be more than
                // one line, and any left click without it leaves the multi-selection behind.
                if let qymcad_ui_state::Sel::Sketch(si) = *sk.sel {
                    // A CORNER FOLLOWING THE POINTER IS FIXED BY A CLICK, on the side the click is, and the click does
                    // nothing else: the box of its size opens beside it and stays.
                    if sk.corner.aim == qymcad_ui_state::Aim::Following {
                        let w = qymcad_ui_state::to_world(sk.view, rect, pos);
                        sk.corner.set.steer(&*sk.project, si, (w.x, w.y));
                        sk.corner.near = Some(Point2::new(w.x, w.y));
                        sk.corner.aim = qymcad_ui_state::Aim::Fixed;
                        sk.corner.pos = Some(pos);
                        sk.corner.focus = true;
                        *sk.status = corner_set_says(sk);
                        return;
                    }
                    sk.corner.track_px = qymcad_ui_state::corner_reach(sk.set);
                    let chamfer = sk.armed.click_op() == 5;
                    let shift = ctx.input(|i| i.modifiers.shift);
                    // WAS THE FIELD ALREADY STANDING? The click that opens it is acted upon as a click
                    // as well - see `corner_click` - so what it lands on is named either way, and this
                    // only decides whether the value beside it still has to be put there.
                    let was_open = sk.corner.at.is_some();
                    // WHAT THE CURSOR IS OVER: a POINT OF THE ZONE outranks a line under it, because a
                    // point is the end of the lines meeting there and a person aims at the point when they
                    // mean the corner. The zone is three times the reach of a pick of a point. A point that is
                    // no corner - a line going on from a fillet along its tangent - does not: the click on the
                    // line beside it named that joint and nothing, and the line aimed at was not taken.
                    let over = {
                        let zone = qymcad_ui_state::corner_zone_px(sk.set);
                        let is_corner = |id: Id| sk.project.vertex_pairs(si, id).into_iter().any(|(a, b)| sk.project.corner_of_pair(si, a, b).is_some());
                        let point = qymcad_pick::nearest_point_within(&sk.pick(), rect, pos, si, zone).filter(|id| is_corner(*id)).map(|id| {
                            let w = qymcad_ui_state::to_world(sk.view, rect, pos);
                            qymcad_ui_state::CornerOver::Point { id, at: (w.x, w.y) }
                        });
                        // A CROSSING OF TWO LINES IN THE ZONE, where no corner stands: the two are cut there, a step of
                        // undo of its own that the corner cut joins, and the click names the corner of the quarter the
                        // pointer is in, at the point the cut made. Reported behaviour: "a sketch fillet or chamfer
                        // cannot be put where two lines cross - a click at the crossing picks no corner".
                        let point = point.or_else(|| {
                            let w = qymcad_ui_state::to_world(sk.view, rect, pos);
                            let reach = f64::from(zone) / f64::from(sk.view.scale);
                            let crossing = sk.project.crossing_near(si, Point2::new(w.x, w.y), reach)?;
                            qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, qymcad_i18n::tr(if chamfer { "tool-chamfer" } else { "tool-fillet" }));
                            let id = sk.project.cut_at_crossing(si, crossing);
                            qymcad_ui_state::close_edit(&mut *sk.edits, &*sk.project);
                            qymcad_ui_state::invalidate(&mut *sk.regen);
                            sk.corner.crossing = qymcad_ui_state::CrossingCut::Cut;
                            id.map(|id| qymcad_ui_state::CornerOver::Point { id, at: (w.x, w.y) })
                        });
                        point.or_else(|| qymcad_pick::nearest_line_eid(&sk.pick(), rect, pos, si).map(qymcad_ui_state::CornerOver::Line)).unwrap_or(qymcad_ui_state::CornerOver::Nothing)
                    };
                    let mut act = qymcad_ui_state::corner_click(sk.project, si, sk.corner, sk.sel_sk, over, shift);
                    // TWO CHOSEN LINES THAT CROSS make their corners where they cross: the two are cut there - a step of
                    // undo the corner cut joins - and the corner is named at that point, on the side the pointer is.
                    // Reported behaviour: "it must work as well with two lines chosen with Shift".
                    let chosen: Vec<Id> = sk.sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
                    let crossing = chosen.iter().enumerate().find_map(|(i, &one)| chosen[i + 1..].iter().find_map(|&other| sk.project.crossing_of(si, one, other)));
                    if let Some(crossing) = crossing {
                        qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, qymcad_i18n::tr(if chamfer { "tool-chamfer" } else { "tool-fillet" }));
                        let cut = sk.project.cut_at_crossing(si, crossing);
                        qymcad_ui_state::close_edit(&mut *sk.edits, &*sk.project);
                        qymcad_ui_state::invalidate(&mut *sk.regen);
                        sk.corner.crossing = qymcad_ui_state::CrossingCut::Cut;
                        sk.sel_sk.items.retain(|&(k, id)| !(k == 1 && (id == crossing.one || id == crossing.other)));
                        let rest: Vec<Id> = sk.sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
                        sk.corner.set.follow(&*sk.project, si, &rest);
                        if let Some(pid) = cut {
                            let w = qymcad_ui_state::to_world(sk.view, rect, pos);
                            let read = sk.corner.set.read_at_point(&*sk.project, si, pid, Some((w.x, w.y)));
                            act = qymcad_ui_state::CornerClick::AtPoint(sk.corner.set.click_at_point(&*sk.project, si, pid, read));
                        }
                    }
                    // A POINT WITH MORE THAN ONE CORNER TO GIVE IS NAMED, NOT A SIDE OF IT: the corner follows the pointer
                    // until a click on its side fixes it - for a crossing clicked or chosen by its lines, and for a point
                    // several shapes meet at alike
                    if matches!(act, qymcad_ui_state::CornerClick::AtPoint(qymcad_ui_state::PointAct::PointAdded)) && sk.corner.set.choice(&*sk.project, si) == qymcad_ui_state::Choice::Many {
                        sk.corner.aim = qymcad_ui_state::Aim::Following;
                    }
                    // WHERE THE CORNER WAS POINTED AT, on the sheet: the line of the corner the pointer stands
                    // nearer to is the one the FIRST leg of a chamfer of two is measured from. It is written
                    // down on every click of the corner tool, because a person may say which of the two lines by
                    // where they aim and no other way of saying it is open to them.
                    sk.corner.near = Some(qymcad_ui_state::to_world(sk.view, rect, pos));
                    // THE FIELD STANDS FOR THE TOOL IN HAND, so a chamfer is answered as a chamfer and not
                    // as a fillet left over from before.
                    sk.corner.at = Some((si, qymcad_ui_state::CORNER_SET, chamfer));
                    if !was_open {
                        // A FIELD THAT HAS JUST OPENED carries the value of the bar: a person who takes the
                        // tool and points at a corner means the radius of the bar, and can type over it.
                        // THE FIELD STANDS WHERE THE PERSON POINTED, as it did before the set was one field. Standing it at the centre
                        // of the canvas read as tidier - the set is not at one place - and laid it over the middle of
                        // the drawing: the tick and the cross of the box sit right of the field, and on a part
                        // whose lines run through the middle of the sheet the next click on a line landed on them
                        // instead. The set is whole wherever its field stands, and a field over the drawing
                        // takes clicks away from it.
                        sk.corner.pos = Some(pos);
                        sk.corner.buf = qymcad_core::expr::fmt_num(sk.tool_prefs.fillet);
                        sk.corner.buf2 = qymcad_core::expr::fmt_num(sk.tool_prefs.chamfer_second);
                        sk.corner.focus = true;
                    }
                    // **THE FIELD KEEPS THE CARET WHILE THE SET IS NAMED.** The focus is asked for again with
                    // every click that changes the set: egui gives it up to a click anywhere else, so without
                    // this a person naming a third corner had to go and click the field before typing - the
                    // one answer that cuts them all is the one act that must not need a hunt for the caret.
                    if !matches!(act, qymcad_ui_state::CornerClick::SingleMode | qymcad_ui_state::CornerClick::Opened) {
                        sk.corner.focus = true;
                    }
                    match act {
                        qymcad_ui_state::CornerClick::LineChosen(_) | qymcad_ui_state::CornerClick::LineLetGo(_) => *sk.status = corner_set_says(sk),
                        // A THIRD LINE AT A STRAIGHT JOINT IS NOT TAKEN, and the point it would have stood
                        // at is named: there is no corner to cut there, and saying so is better than a
                        // line that was not taken for no visible reason.
                        qymcad_ui_state::CornerClick::StraightJoint(pid) => {
                            let name = corner_point_name(sk, si, pid);
                            *sk.status = qymcad_i18n::tr1("sk-corner-straight", "p", &name);
                        }
                        qymcad_ui_state::CornerClick::AtPoint(act) => *sk.status = corner_point_says(sk, act),
                        // A CLICK ON NOTHING CHANGES NOTHING AND SAYS NOTHING. It used to speak, and what it said was a complaint
                        // about the click - that there is no line or point under it - which is the one thing a
                        // person who clicked empty sheet already knows, and which covered the status line of a
                        // tool that was working. A click that names nothing is not a mistake to be answered.
                        qymcad_ui_state::CornerClick::SingleMode => {}
                        // THE FIELD WAS NOT STANDING AND THIS CLICK NAMED NOTHING: the tool is in hand and
                        // says what it waits for, which is the one thing a person who has just taken it needs.
                        // With the field already up it says nothing: the click opened nothing, and the tool is
                        // already saying what it waits for in the hint on its own button.
                        qymcad_ui_state::CornerClick::Opened => *sk.status = qymcad_i18n::tr(if chamfer { "tb-chamfer-sketch-hint" } else { "tb-fillet-sketch-hint" }),
                    }
                }
            } else if sk.armed.click_op() != 0 {
                // trimming, extending or breaking by click: a line first, then a circle or an arc
                if let qymcad_ui_state::Sel::Sketch(si) = *sk.sel {
                    let w = qymcad_ui_state::to_world(&*sk.view, rect, pos);
                    // THE BOUNDARY OF AN OPERATION: the click is the act, one step of undo named after the tool
                    // - left for the frame to notice, the change was a step called "Edit"
                    let tool = match sk.armed.click_op() {
                        1 => "tool-trim",
                        2 => "tool-extend",
                        _ => "tool-break",
                    };
                    qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, qymcad_i18n::tr(tool));
                    let line_eid = qymcad_pick::nearest_line_eid(&sk.pick(), rect, pos, si);
                    // EXTEND HOLDS A LINE, or takes the line clicked: the click applies what the preview shows
                    if sk.armed.click_op() == 2 && (sk.tool.extend.is_some() || line_eid.is_some()) {
                        extend_click(sk, ExtendAt { rect, pos, si, line: line_eid });
                        qymcad_ui_state::commit_edit_if_changed(&mut sk.rebuild()); // taking the line changes nothing
                        return;
                    }
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
                    qymcad_ui_state::commit_edit_if_changed(&mut sk.rebuild()); // a click that changed nothing lays no step
                }
            } else if sk.clip.geom_pending.is_some() && matches!(*sk.sel, qymcad_ui_state::Sel::Sketch(_)) {
                // a click on THE ANCHOR point takes the geometry into the buffer (on a cut, the
                // source is removed)
                let qymcad_ui_state::Sel::Sketch(si) = *sk.sel else { return }; // the selection may have changed between frames - do not crash
                let w = snap_world(sk, rect, pos);
                // it should not be empty, but the program must not crash over that
                let Some((eids, cut)) = sk.clip.geom_pending.take() else { return };
                // a cut removes the source here, so it is a step of its own; a copy changes nothing until placed
                if cut {
                    qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, qymcad_i18n::tr("menu-cut"));
                }
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
                // A MARKER INTO THE CLIPBOARD OF THE SYSTEM, as a copy of the tree puts one: a live window hands Ctrl+V
                // over only while that clipboard holds some text. Reported behaviour: "Ctrl+V does not paste (Edit ->
                // Paste works), and no preview comes under the cursor".
                sk.clip.os_ping = true;
                *sk.status = qymcad_i18n::tr2("sk-clipboard", "what", &if cut { qymcad_i18n::tr("sk-cut-done") } else { qymcad_i18n::tr("sk-copied") }, "n", &n.to_string());
                // A COPY GOES TO THE CLIPBOARD AND NO FURTHER: the base point is the point the copy is held by when it is
                // pasted - here or in another sketch, as many times as wanted. Reported behaviour: "a copy meant for the
                // clipboard becomes a move-copy on the spot". Placing at once is the Copy tool of the panel.
                if cut {
                    qymcad_ui_state::commit_edit_if_changed(&mut sk.rebuild());
                }
            } else if let (true, qymcad_ui_state::Sel::Sketch(_)) = (sk.clip.geom_place, *sk.sel) {
                // a placement click pastes the buffer so that the anchor lands on the clicked point
                let qymcad_ui_state::Sel::Sketch(si) = *sk.sel else { return }; // the selection may have changed between frames - do not crash
                let w = snap_world(sk, rect, pos);
                if let Some(clip) = sk.clip.geom.clone() {
                    qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, qymcad_i18n::tr("win-insert"));
                    let ids = sk.project.paste_sketch_geometry(si, &clip, w.x, w.y);
                    sk.project.solve_sketch(si);
                    sk.sel_sk.items = ids.into_iter().map(|id| (1u8, id)).collect();
                    qymcad_ui_state::invalidate(&mut *sk.regen);
                    *sk.status = qymcad_i18n::tr("sk-pasted");
                    qymcad_ui_state::commit_edit_if_changed(&mut sk.rebuild());
                }
                sk.clip.geom_place = false;
            } else if sk.armed.pat_op() != 0 && matches!(*sk.sel, qymcad_ui_state::Sel::Sketch(_)) {
                // an array: pick the entities, then (for a circular one) click THE CENTRE of
                // rotation, then Enter
                let shift = ctx.input(|i| i.modifiers.shift);
                if let qymcad_ui_state::Sel::Sketch(si) = *sk.sel {
                    qymcad_ui_state::take_whole_rects(&*sk.project, si, &mut *sk.sel_sk);
                    // and what was selected before the tool
                }
                let has_sel = sk.sel_sk.items.iter().any(|(k, _)| *k == 1);
                if !has_sel {
                    sketch_select_click(sk, rect, pos, shift);
                    if let qymcad_ui_state::Sel::Sketch(si) = *sk.sel {
                        qymcad_ui_state::take_whole_rects(&*sk.project, si, &mut *sk.sel_sk);
                    }
                    *sk.status = if sk.sel_sk.items.iter().any(|(k, _)| *k == 1) {
                        if sk.armed.pat_op() == 2 {
                            qymcad_i18n::tr("sk-click-rot-centre")
                        } else {
                            qymcad_i18n::tr("sk-params-above")
                        }
                    } else {
                        qymcad_i18n::tr("sk-click-for-array")
                    };
                } else if shift {
                    // Shift continues picking the source
                    sketch_select_click(sk, rect, pos, true);
                    if let qymcad_ui_state::Sel::Sketch(si) = *sk.sel {
                        qymcad_ui_state::take_whole_rects(&*sk.project, si, &mut *sk.sel_sk);
                    }
                } else if sk.armed.pat_op() == 2 {
                    // circular: a click sets or moves the centre, snapping to an intersection or a vertex
                    sk.pat.center = Some(snap_world(sk, rect, pos));
                    *sk.status = qymcad_i18n::tr("sk-centre-set-params");
                }
            } else if sk.armed.move_op() != 0 && matches!(*sk.sel, qymcad_ui_state::Sel::Sketch(_)) {
                // an interactive move or copy: the selection, then the base point, then the target
                let qymcad_ui_state::Sel::Sketch(si) = *sk.sel else { return }; // the selection may have changed between frames - do not crash
                let w = snap_world(sk, rect, pos);
                // what was selected before the tool, too: one side of a rectangle carries the whole rectangle
                qymcad_ui_state::take_whole_rects(&*sk.project, si, &mut *sk.sel_sk);
                let eids: Vec<Id> = sk.sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
                let texts = qymcad_ui_state::sel_text_indices(&*sk.project, &*sk.sel_sk, si);
                if eids.is_empty() && texts.is_empty() {
                    let shift = ctx.input(|i| i.modifiers.shift);
                    sketch_select_click(sk, rect, pos, shift);
                    qymcad_ui_state::take_whole_rects(&*sk.project, si, &mut *sk.sel_sk);
                    if !sk.sel_sk.items.iter().any(|(k, _)| *k == 1 || *k == qymcad_ui_state::SEL_TEXT) {
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
                    // THE BOUNDARY OF AN OPERATION: the target click is the act, so it makes one step of
                    // undo named after the tool. A change left for the frame to notice is called "Edit".
                    qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, qymcad_i18n::tr(if sk.armed.move_op() == 2 { "tool-copy" } else { "tool-move" }));
                    if sk.armed.move_op() == 2 {
                        let ids = if eids.is_empty() { Vec::new() } else { sk.project.copy_entities(si, &eids, dx, dy) };
                        let text_ids: Vec<Id> = texts.iter().filter_map(|&ti| sk.project.copy_sketch_text(si, ti, dx, dy)).collect();
                        sk.project.solve_sketch(si);
                        // select the copies
                        sk.sel_sk.items = ids.into_iter().map(|id| (1u8, id)).chain(text_ids.into_iter().map(|id| (qymcad_ui_state::SEL_TEXT, id))).collect();
                        *sk.status = qymcad_i18n::tr("sk-copied");
                    } else {
                        // WHERE IT WAS ASKED TO GO, and where the constraints let it go.
                        //
                        // Reported behaviour: "I took the move tool, moved the circle far away, and
                        // the coincidence with the origin stayed - and stayed GREEN. Reopen the
                        // project and the circle is back at the centre; draw anything and it jumps
                        // back too."
                        //
                        // The move shifted the points and never asked the solver, so the screen
                        // showed a shape standing where nothing allows it to stand. The lie held
                        // until the next edit, and the jump back then looked like a defect of its
                        // own. THE SOLVER NOW RUNS INSIDE `move_entities`, where no caller can
                        // forget it; what is left here is telling the person that the drawing did
                        // not go where it was asked.
                        let before = qymcad_ui_state::entities_centroid(&*sk.project, si, &eids);
                        if !eids.is_empty() {
                            sk.project.move_entities(si, &eids, dx, dy);
                        }
                        for &ti in &texts {
                            sk.project.move_sketch_text(si, ti, dx, dy);
                        }
                        let after = qymcad_ui_state::entities_centroid(&*sk.project, si, &eids);
                        let asked = (dx * dx + dy * dy).sqrt();
                        let went = match (before, after) {
                            (Some(b), Some(a)) => ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt(),
                            _ => asked,
                        };
                        // a tenth of what was asked, and at least a hundredth of a millimetre: below
                        // that the drawing did not move at all
                        *sk.status = if asked > 1e-9 && went < (asked * 0.1).max(1e-2) { format!("{} {}", ph::WARNING, qymcad_i18n::tr("sk-move-held")) } else { qymcad_i18n::tr("sk-moved") };
                    }
                    *sk.armed = qymcad_ui_state::Armed::None;
                    qymcad_ui_state::invalidate(&mut *sk.regen);
                    qymcad_ui_state::commit_edit(&mut sk.rebuild());
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
    // "No ties" on the bar of a drawing tool: the point goes where the pointer is, and the preview snaps to nothing
    if sk.armed.draw_kind() != 0 && sk.tool.ties == qymcad_ui_state::Ties::Free {
        *sk.snap_hint = None;
        return Point2::new(w.x, w.y);
    }
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
    // A POINT BEING DRAGGED DOES NOT SNAP TO ITSELF. Its own place, and whatever stands on it (a coincident end, the
    // vertex of the contour drawn through it), is a vertex within reach until the hand is 9 px away: a point led
    // at 3 px a frame stuck, moved in jerks and was let go short - measured, 1 mm short of a 7 mm drag.
    //
    // NOR TO WHAT GOES WITH IT: the corners of a rectangle dragged by its centre, and every line ending at a point that
    // moves. They stand where the drag was a frame ago, and the middle of the rectangle's own diagonal - its centre
    // a frame back - caught the centre: a rectangle led 12 mm by its centre came 10.7.
    let moving: Vec<Point2> = sk
        .drag
        .pt()
        .and_then(|(dsi, pi)| {
            let s = sk.project.sketches.get(dsi)?;
            let q = s.points.get(pi)?;
            let at = |id: Id| s.points.iter().find(|p| p.id == id).map(|p| Point2::new(p.x, p.y));
            Some(std::iter::once(Point2::new(q.x, q.y)).chain(s.rects.iter().filter(|r| r.centre == q.id).flat_map(|r| r.corners.into_iter().filter_map(at))).collect())
        })
        .unwrap_or_default();
    let own = |p: Point2| moving.iter().any(|q| (p.x - q.x).abs() < 1e-9 && (p.y - q.y).abs() < 1e-9);
    if let Some(asi) = qymcad_ui_state::edit_si(&*sk.project, &*sk.sketch_ses) {
        if let Some(s) = sk.project.sketches.get(asi) {
            for sp in &s.points {
                let p = Point2::new(sp.x, sp.y);
                if own(p) {
                    continue;
                }
                let d = sd(p, &*sk.view);
                let ty = if centers.contains(&sp.id) { 4 } else { 0 };
                if best.is_none_or(|(bd, _, _)| d < bd) {
                    best = Some((d, p, ty));
                }
            }
            for cid in &s.contour_ids {
                let Some(ci) = sk.project.contour_index(*cid) else { continue };
                // NOR TO THE CURVE IT DRAGS ALONG: a spline drawn through a dragged node runs under the cursor, and
                // the points it is cut into caught the node - the middle node of a spline led to (20, 25) stopped at
                // (18.7, 24.8). The true corners of such a contour are points of the sketch, snapped above.
                if sk.project.contours[ci].points.iter().any(|p| own(*p)) {
                    continue;
                }
                for p in sk.project.contours[ci].points.iter() {
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
    let ref_edges =
        qymcad_ui_state::edit_si(&*sk.project, &*sk.sketch_ses).map(|si| qymcad_pick::sketch_ref_edges_2d(&*sk.cache, &*sk.cmd, &*sk.live, &*sk.project, &*sk.regen, si)).unwrap_or_default();
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
        // ONLY WHAT RUNS NEAR THE POINTER: every point offered below - a midpoint, an intersection, a point on an
        // edge - lies on its segments or circles and is taken within the reach of a snap, so a curve that passes
        // further than that from the pointer offers nothing. Every pair of 70 000 segments, intersected each frame,
        // took 15 s.
        let reach = qymcad_ui_state::grab::grab(sk.set, Grab::Snap) + 1.0;
        let near_seg = |a: Point2, b: Point2| qymcad_ui_state::screen_dist_seg(screen, sh.at(a), sh.at(b)) <= reach;
        let lines: Vec<(Point2, Point2)> = lines.into_iter().filter(|(a, b)| !own(*a) && !own(*b) && near_seg(*a, *b)).collect(); // what moves with the drag is no target
        let circs: Vec<_> = circs.into_iter().filter(|rim| (sh.at(rim.centre).distance(screen) - rim.radius as f32 * sk.view.scale).abs() <= reach).collect();
        // the segments of the projected outlines of the reference body, used for INTERSECTIONS with the
        // sketch lines and for points on an edge. That is how the intersection of a construction line with
        // a face or the outline of a part becomes snappable.
        let ref_segs: Vec<(Point2, Point2)> = ref_edges.iter().flat_map(|poly| poly.windows(2).map(|s| (s[0], s[1]))).filter(|(a, b)| near_seg(*a, *b)).collect();
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
            for rim in &circs {
                for p in qymcad_ui_state::seg_circle_intersect(lines[i].0, lines[i].1, rim.centre, rim.radius).into_iter().filter(|p| rim.holds(*p)) {
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
        // 2b) THE AXES X = 0 AND Y = 0 CROSS THINGS TOO.
        //
        // Reported behaviour: "a circle of construction geometry has a snap, but at the same time the
        // cursor is not pulled to the X and Y axes, which makes it awkward to put the centre of a new
        // circle on the construction circle". The axes were looked at only in the SECOND half of the
        // snapping, after this block has already returned an answer - so the moment the cursor came near a
        // circle, the circle answered "a point on my edge" and the axes never got a turn. And they were
        // nowhere among the things that get intersected either, so the one point being aimed at - where
        // the construction circle crosses the axis - was offered by neither half.
        for axis in [0u8, 1] {
            for (a, b) in lines.iter().chain(ref_segs.iter()) {
                if let Some(p) = qymcad_ui_state::seg_axis_intersect(*a, *b, axis) {
                    let d = sd(p, &*sk.view);
                    if d <= qymcad_ui_state::grab::grab(sk.set, Grab::Snap) && cand.is_none_or(|(bd, _, bty)| (5u8, d) < (bty, bd)) {
                        cand = Some((d, p, 5));
                    }
                }
            }
            for rim in &circs {
                for p in qymcad_ui_state::circle_axis_intersect(rim.centre, rim.radius, axis).into_iter().filter(|p| rim.holds(*p)) {
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
        for rim in &circs {
            let (c, r) = (rim.centre, rim.radius);
            let dir = Point2::new(w.x - c.x, w.y - c.y);
            let len = (dir.x * dir.x + dir.y * dir.y).sqrt();
            if len > 1e-9 {
                let p = Point2::new(c.x + dir.x / len * r, c.y + dir.y / len * r);
                if !rim.holds(p) {
                    continue; // the part of an arc's circle the arc does not run over is no edge
                }
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
        let ax = if near_x0 {
            0.0
        } else if (sh.at(Point2::new(gx, w.y)).x - screen.x).abs() <= qymcad_ui_state::grab::grab(sk.set, Grab::Snap) {
            gx
        } else {
            w.x
        };
        let ay = if near_y0 {
            0.0
        } else if (sh.at(Point2::new(w.x, gy)).y - screen.y).abs() <= qymcad_ui_state::grab::grab(sk.set, Grab::Snap) {
            gy
        } else {
            w.y
        };
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
            Some(Constraint::Distance { a: p1, b: p2, d, off: 0.0, expr: String::new(), driven: false, axis: 0, at: None })
        }
        (qymcad_ui_state::DimRef::Point(p), qymcad_ui_state::DimRef::Line(a, b)) | (qymcad_ui_state::DimRef::Line(a, b), qymcad_ui_state::DimRef::Point(p)) => {
            let d = perp(&*sk.project, p, a, b);
            Some(Constraint::DistancePL { p, a, b, d, off: 0.0, expr: String::new(), driven: false, at: None })
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
                Some(Constraint::DistancePL { p, a: axa, b: axb, d, off: 0.0, expr: String::new(), driven: false, at: None })
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
                    Some(Constraint::DistancePL { p: a1, a: a2, b: b2, d, off: 0.0, expr: String::new(), driven: false, at: None })
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
                    Some(Constraint::AngleLines { a: na1, b: nb1, c: na2, d: nb2, deg, expr: String::new(), driven: false, off: 0.0, at: None })
                } else {
                    None
                }
            }
        }
    };
    if let Some(c) = c {
        sk.project.give_rect_turn_to(si, &c); // an angle on a side of a rectangle turns it
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
            *sk.status = qymcad_i18n::tr("in-text-deleted");
        }
        return;
    }
    if let Some(ni) = sk.annot.note.take() {
        if ni < sk.project.sketches[si].notes.len() {
            sk.project.sketches[si].notes.remove(ni);
            *sk.status = qymcad_i18n::tr("in-note-deleted");
        }
        return;
    }
    if let Some(ci) = sk.gsel.constraint.take() {
        if ci < sk.project.sketches[si].constraints.len() {
            sk.project.delete_sketch_constraint(si, ci); // also cleans up an orphaned midpoint
            qymcad_ui_state::invalidate(&mut *sk.regen);
            *sk.status = qymcad_i18n::tr("in-constraint-deleted");
        }
        return;
    }
    let eids: Vec<Id> = sk.sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
    // system points (the origin and the axes) and DRIVEN ones (projections of a body) are not deleted one
    // by one
    let sys: std::collections::HashSet<Id> = sk.project.sketches[si].immovable_points();
    let pids: Vec<Id> = sk.sel_sk.items.iter().filter(|(k, id)| *k == 0 && !sys.contains(id)).map(|(_, id)| *id).collect();
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
    *sk.status = qymcad_i18n::tr("in-deleted");
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
    *sk.status = qymcad_i18n::tr("g-insert-click");
    true
}
