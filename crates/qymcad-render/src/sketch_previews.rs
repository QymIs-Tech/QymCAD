//! THE PREVIEWS OF THE TOOLS OF A SKETCH: the shape being drawn, extend, trim, pattern, move.

use super::*;

pub fn draw_sketch_preview(pn: &Painting, painter: &egui::Painter, rect: Rect) {
    let sh = qymcad_ui_state::Sheet { view: pn.view, rect };
    draw_offset_preview(pn, painter, &sh);
    if pn.armed.draw_kind() == 0 {
        return;
    }
    let col = if pn.tool.construction { pn.scheme.pal.sketch_construction() } else { pn.scheme.pal.sketch_line() };
    let stroke = Stroke::new(1.3, col);
    // THE CLICKS THAT ARE NOTHING YET ARE MARKED, and of a chain of lines only the corner the next segment goes from: the
    // corners already laid are points of the sketch and drawn as such, and the solve after an automatic constraint moves
    // them - marked again where they were clicked, a second dot stood beside every corner of the chain.
    let marked = if pn.armed.draw_kind() == 1 { pn.tool.pts.len().saturating_sub(1) } else { 0 };
    for p in &pn.tool.pts[marked..] {
        painter.circle_filled(sh.at(*p), 3.0, col);
    }
    let Some(cur) = pn.cursor else { return };
    let sc = sh.at(cur);
    match pn.armed.draw_kind() {
        // THE LABEL WHERE THE CLICK WILL PUT IT: its outlines follow the pointer (a note, its words)
        11 if pn.tool_prefs.text_note => {
            painter.text(sc, egui::Align2::LEFT_BOTTOM, &pn.tool_prefs.text, egui::FontId::proportional(14.0), col);
        }
        11 if qymcad_ui_state::text_ghost_shown(pn.armed, &pn.inline) => {
            for glyph in pn.tool.text_ghost.iter().flat_map(|(.., g)| g) {
                let pts: Vec<Pos2> = glyph.iter().chain(glyph.first()).map(|p| sh.at(Point2::new(cur.x + p.x, cur.y + p.y))).collect();
                painter.add(egui::Shape::line(pts, stroke));
            }
        }
        1 => {
            if let Some(&last) = pn.tool.pts.last() {
                painter.line_segment([sh.at(last), sc], stroke);
                // a live preview of the automatic constraints: what will be attached if a point is placed here - none
                // where none will be ("No ties", the auto constraints off)
                if let Some(si) = qymcad_ui_state::edit_si(pn.project, &pn.sketch_ses).filter(|_| qymcad_ui_state::ties_laid(pn.tool, pn.set)) {
                    let prev = (pn.tool.pts.len() >= 2).then(|| pn.tool.pts[pn.tool.pts.len() - 2]);
                    for (g, at) in qymcad_pick::infer_hints(&DrawCtx { cam: &pn.cam, set: pn.set, scheme: pn.scheme, project: pn.project, active_path: pn.active_path }, si, prev, last, cur) {
                        let sat = sh.at(at) + egui::vec2(11.0, -11.0);
                        painter.rect_filled(Rect::from_center_size(sat, egui::vec2(14.0, 14.0)), 2.0, pn.scheme.pal.constraint_ok());
                        paint_gly(painter, sat, 4.0, g, pn.scheme.pal.glyph_text());
                    }
                }
            }
        }
        2 => match pn.tool_prefs.rect_mode {
            1 => {
                // centre plus corner: the extent is mirrored through the centre
                if let Some(&c) = pn.tool.pts.first() {
                    let opp = sh.at(Point2::new(2.0 * c.x - cur.x, 2.0 * c.y - cur.y));
                    painter.rect_stroke(Rect::from_two_pos(opp, sc), 0.0, stroke, egui::StrokeKind::Middle);
                }
            }
            2 => {
                // rotated, by three points: the first point to the cursor is one side; after the second, the frame follows the height
                if pn.tool.pts.len() == 1 {
                    painter.line_segment([sh.at(pn.tool.pts[0]), sc], stroke);
                } else if pn.tool.pts.len() == 2 {
                    let (p1, p2) = (pn.tool.pts[0], pn.tool.pts[1]);
                    let (dx, dy) = (p2.x - p1.x, p2.y - p1.y);
                    let len = (dx * dx + dy * dy).sqrt().max(1e-9);
                    let (nx, ny) = (-dy / len, dx / len);
                    let h = (cur.x - p2.x) * nx + (cur.y - p2.y) * ny;
                    let p3 = Point2::new(p2.x + nx * h, p2.y + ny * h);
                    let p4 = Point2::new(p1.x + nx * h, p1.y + ny * h);
                    let poly: Vec<Pos2> = [p1, p2, p3, p4, p1].iter().map(|p| sh.at(*p)).collect();
                    painter.add(egui::Shape::line(poly, stroke));
                }
            }
            _ => {
                if let Some(&a) = pn.tool.pts.first() {
                    painter.rect_stroke(Rect::from_two_pos(sh.at(a), sc), 0.0, stroke, egui::StrokeKind::Middle);
                }
            }
        },
        3 => {
            if pn.tool_prefs.circ_mode == 2 {
                // a tangent circle: once the base is chosen, a circle at the cursor with the tangency radius
                if let (Some(eref), Some(si)) = (pn.tool.circ_tan, qymcad_ui_state::edit_si(pn.project, &pn.sketch_ses)) {
                    let r = qymcad_ui_state::tangent_radius_to_edge(&DrawCtx { cam: &pn.cam, set: pn.set, scheme: pn.scheme, project: pn.project, active_path: pn.active_path }, si, eref, cur);
                    if r > 1e-6 {
                        let scn = sh.at(cur);
                        let rp = (sh.at(Point2::new(cur.x + r, cur.y)).x - scn.x).abs();
                        painter.circle_stroke(scn, rp, stroke);
                    }
                }
            } else if let Some(&c) = pn.tool.pts.first() {
                if pn.tool_prefs.circ_mode == 1 {
                    // by two points: the diameter runs from c to the cursor
                    let mid = sh.at(Point2::new((c.x + cur.x) / 2.0, (c.y + cur.y) / 2.0));
                    painter.circle_stroke(mid, mid.distance(sh.at(c)), stroke);
                } else {
                    let scn = sh.at(c);
                    painter.circle_stroke(scn, scn.distance(sc), stroke);
                }
            }
        }
        4 if pn.tool_prefs.arc_mode == 2 => {
            // a tangent arc: once started (at the end of a curve), an arc from s to the cursor, smooth into the base
            if pn.tool.pts.len() == 1 {
                let s = pn.tool.pts[0];
                if let Some((t, _)) = qymcad_ui_state::edit_si(pn.project, &pn.sketch_ses)
                    .and_then(|si| qymcad_pick::arc_tangent_ref(&DrawCtx { cam: &pn.cam, set: pn.set, scheme: pn.scheme, project: pn.project, active_path: pn.active_path }, si, s))
                {
                    if let Some((cx, cy, r, ccw)) = qymcad_ui_state::tangent_arc(s, t, cur) {
                        let a0 = (s.y - cy).atan2(s.x - cx);
                        let mut a1 = (cur.y - cy).atan2(cur.x - cx);
                        if ccw && a1 < a0 {
                            a1 += std::f64::consts::TAU;
                        } else if !ccw && a1 > a0 {
                            a1 -= std::f64::consts::TAU;
                        }
                        let pts: Vec<Pos2> = (0..=40)
                            .map(|k| {
                                let a = a0 + (a1 - a0) * k as f64 / 40.0;
                                sh.at(Point2::new(cx + r * a.cos(), cy + r * a.sin()))
                            })
                            .collect();
                        painter.add(egui::Shape::line(pts, stroke));
                    } else {
                        painter.line_segment([sh.at(s), sc], stroke);
                    }
                } else {
                    painter.line_segment([sh.at(s), sc], Stroke::new(0.6, col));
                }
            }
        }
        4 => {
            if pn.tool_prefs.arc_mode == 1 {
                // by three points: the start, the end, and the cursor as a point on the arc
                if pn.tool.pts.len() == 1 {
                    painter.line_segment([sh.at(pn.tool.pts[0]), sc], Stroke::new(0.6, col));
                } else if pn.tool.pts.len() == 2 {
                    let (s, e) = (pn.tool.pts[0], pn.tool.pts[1]);
                    if let Some((cx, cy, _r)) = qymcad_ui_state::circumcircle(s, e, cur) {
                        let cen = Point2::new(cx, cy);
                        let a0 = (s.y - cy).atan2(s.x - cx);
                        let mut a1 = (e.y - cy).atan2(e.x - cx);
                        let ccw = (cur.x - s.x) * (e.y - s.y) - (cur.y - s.y) * (e.x - s.x) > 0.0;
                        if ccw && a1 < a0 {
                            a1 += std::f64::consts::TAU;
                        } else if !ccw && a1 > a0 {
                            a1 -= std::f64::consts::TAU;
                        }
                        let r = ((s.x - cx).powi(2) + (s.y - cy).powi(2)).sqrt();
                        let pts: Vec<Pos2> = (0..=40)
                            .map(|k| {
                                let t = a0 + (a1 - a0) * k as f64 / 40.0;
                                sh.at(Point2::new(cen.x + r * t.cos(), cen.y + r * t.sin()))
                            })
                            .collect();
                        painter.add(egui::Shape::line(pts, stroke));
                    } else {
                        painter.line_segment([sh.at(s), sc], stroke);
                    }
                }
            } else if let Some(&c) = pn.tool.pts.first() {
                let scn = sh.at(c);
                painter.line_segment([scn, sc], Stroke::new(0.6, col));
                painter.circle_stroke(scn, scn.distance(sc), Stroke::new(0.6, col));
            }
        }
        6 => {
            if let Some(&first) = pn.tool.pts.first() {
                let n = pn.tool_prefs.poly_n.max(3);
                // the same polygon the click will make, from the one computation of it
                let (c, v) = qymcad_ui_state::polygon_from_clicks(first, cur, n, pn.tool_prefs.poly_mode);
                let a0 = (v.y - c.y).atan2(v.x - c.x);
                let r = ((v.x - c.x).powi(2) + (v.y - c.y).powi(2)).sqrt();
                let pts: Vec<Pos2> = (0..=n)
                    .map(|k| {
                        let a = a0 + std::f64::consts::TAU * k as f64 / n as f64;
                        sh.at(Point2::new(c.x + r * a.cos(), c.y + r * a.sin()))
                    })
                    .collect();
                painter.add(egui::Shape::line(pts, stroke));
            }
        }
        7 => {
            if pn.tool.pts.len() == 1 {
                painter.line_segment([sh.at(pn.tool.pts[0]), sc], stroke);
            } else if pn.tool.pts.len() == 2 {
                let (a, b) = (pn.tool.pts[0], pn.tool.pts[1]);
                let (sa, sb) = (sh.at(a), sh.at(b));
                painter.line_segment([sa, sb], stroke);
                let r = (sc - sb).length().min((sc - sa).length());
                painter.circle_stroke(sa, r, Stroke::new(0.6, col));
                painter.circle_stroke(sb, r, Stroke::new(0.6, col));
            }
        }
        8 => {
            // the major axis follows the cursor after the centre, the whole ellipse after the end of the axis
            if pn.tool.pts.len() == 1 {
                painter.line_segment([sh.at(pn.tool.pts[0]), sc], stroke);
            } else if pn.tool.pts.len() == 2 {
                let (cc, a) = (pn.tool.pts[0], pn.tool.pts[1]);
                let (major, rot, minor) = qymcad_ui_state::ellipse_from_clicks(cc, a, cur);
                let (ux, uy) = (rot.cos(), rot.sin());
                let pts: Vec<Pos2> = (0..=48)
                    .map(|i| {
                        let t = std::f64::consts::TAU * i as f64 / 48.0;
                        let (x, y) = (major * t.cos(), minor * t.sin());
                        sh.at(Point2::new(cc.x + x * ux - y * uy, cc.y + x * uy + y * ux))
                    })
                    .collect();
                painter.add(egui::Shape::line(pts, stroke));
            }
        }
        9 => {
            // a polyline preview of the knots + a tail to the cursor
            let mut sp: Vec<Pos2> = pn.tool.pts.iter().map(|p| sh.at(*p)).collect();
            sp.push(sc);
            if sp.len() >= 2 {
                painter.add(egui::Shape::line(sp, stroke));
            }
        }
        10 if pn.tool.pts.len() == 2 => {
            if let Some((cx, cy, r)) = qymcad_ui_state::circumcircle(pn.tool.pts[0], pn.tool.pts[1], cur) {
                let scn = sh.at(Point2::new(cx, cy));
                let rp = (sh.at(Point2::new(cx + r, cy)).x - scn.x).abs();
                painter.circle_stroke(scn, rp, stroke);
            }
        }
        _ => {}
    }
    // under ANY tool, show the automatic constraint that the cursor's snap implies (coincident, on-edge):
    // a click snapped to geometry will attach it through the shared point. The badge sits at the snap point.
    if let Some((g, at)) = qymcad_pick::snap_infer_glyph(pn) {
        let sat = sh.at(at) + egui::vec2(11.0, -11.0);
        painter.rect_filled(Rect::from_center_size(sat, egui::vec2(14.0, 14.0)), 2.0, pn.scheme.pal.constraint_ok());
        paint_gly(painter, sat, 4.0, g, pn.scheme.pal.glyph_text());
    }
}

/// The hover preview for trim, extend and break: what will happen if the entity under the cursor is
/// clicked. Trim lights the span that will be removed in red; break puts a marker at the point; extend
/// lights in green the end that will be pulled.
/// THE RING WHERE AN EXTENDED END LANDS, in px.
pub const EXTEND_RING: f32 = 4.0;

/// THE LINE EXTEND HOLDS and where the pointer stands on the canvas.
#[derive(Clone, Copy)]
pub(crate) struct ExtendHeld {
    pub(crate) si: usize,
    pub(crate) line: Id,
    pub(crate) pos: Pos2,
}

/// THE PREVIEW OF EXTEND: the line it holds lit, and from each end that goes, a dashed line to where it goes - the
/// curve under the pointer when it lies on that side of the axis, else the nearest there. A side meeting nothing draws
/// nothing.
pub(crate) fn draw_extend_preview(pn: &Painting, painter: &egui::Painter, rect: Rect, held: ExtendHeld) {
    let sh = qymcad_ui_state::Sheet { view: pn.view, rect };
    let pick = PickCtx { project: pn.project, set: pn.set, view: &pn.view };
    let Some(s) = pn.project.sketches.get(held.si) else { return };
    let Some(qymcad_core::model::EntityKind::Line { a, b }) = s.entities.iter().find(|e| e.id == held.line).map(|e| e.kind) else { return };
    let at = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| Point2::new(q.x, q.y));
    let (Some(pa), Some(pb)) = (at(a), at(b)) else { return };
    let green = pn.scheme.pal.add();
    painter.line_segment([sh.at(pa), sh.at(pb)], Stroke::new(3.0, green));
    let over = qymcad_pick::nearest_line_eid(&pick, rect, held.pos, held.si).filter(|e| *e != held.line).or_else(|| qymcad_pick::nearest_circle_entity(&pick, rect, held.pos, held.si));
    let sides = if pn.tool_prefs.extend_both { qymcad_core::model::ExtendSides::Both } else { qymcad_core::model::ExtendSides::Nearer };
    let Some(pointer) = pn.cursor else { return };
    let ext = pn.project.line_extension(held.si, held.line, &qymcad_core::model::ExtendAsk { pointer, over, sides });
    let along = |t: f64| Point2::new(pa.x + (pb.x - pa.x) * t, pa.y + (pb.y - pa.y) * t);
    for (end, t) in [(pa, ext.a), (pb, ext.b)] {
        if let Some(t) = t {
            // the dashes, and a ring where the end lands: the last dash stops short of it by up to a gap
            painter.add(egui::Shape::dashed_line(&[sh.at(end), sh.at(along(t))], Stroke::new(2.0, green), 6.0, 4.0));
            painter.circle_stroke(sh.at(along(t)), EXTEND_RING, Stroke::new(2.0, green));
        }
    }
}

pub fn draw_trim_preview(pn: &Painting, painter: &egui::Painter, rect: Rect) {
    let sh = qymcad_ui_state::Sheet { view: pn.view, rect };
    use qymcad_core::model::EntityKind;
    if pn.armed.click_op() == 0 {
        return;
    }
    let Some(cur) = pn.cursor else { return };
    let Sel::Sketch(si) = pn.sel else { return };
    let pos = sh.at(cur);
    if let (2, Some(held)) = (pn.armed.click_op(), pn.tool.extend) {
        draw_extend_preview(pn, painter, rect, ExtendHeld { si, line: held, pos });
        return;
    }
    let Some(eid) = qymcad_pick::nearest_line_eid(&PickCtx { project: pn.project, set: pn.set, view: &pn.view }, rect, pos, si)
        .or_else(|| qymcad_pick::nearest_circle_entity(&PickCtx { project: pn.project, set: pn.set, view: &pn.view }, rect, pos, si))
    else {
        return;
    };
    let Some(s) = pn.project.sketches.get(si) else { return };
    let Some(kind) = s.entities.iter().find(|e| e.id == eid).map(|e| e.kind) else { return };
    let points_by_id: std::collections::HashMap<Id, &qymcad_core::model::SketchPoint> = s.points.iter().map(|p| (p.id, p)).collect(); // a table: these are looked up for every entity or constraint
    let pt = |id: Id| points_by_id.get(&id).copied().map(|q| Point2::new(q.x, q.y));
    let red = pn.scheme.pal.error();
    let green = pn.scheme.pal.add();
    let inter = pn.project.entity_intersections(si, eid);
    match pn.armed.click_op() {
        1 => {
            // TRIM: light the span that will be removed in red
            match kind {
                EntityKind::Line { a, b } => {
                    let (Some(pa), Some(pb)) = (pt(a), pt(b)) else { return };
                    let (dx, dy) = (pb.x - pa.x, pb.y - pa.y);
                    let len2 = dx * dx + dy * dy;
                    if len2 < 1e-9 {
                        return;
                    }
                    let param = |x: f64, y: f64| ((x - pa.x) * dx + (y - pa.y) * dy) / len2;
                    let mut ts: Vec<f64> = inter.iter().map(|&(x, y)| param(x, y)).filter(|t| *t > 1e-6 && *t < 1.0 - 1e-6).collect();
                    ts.push(0.0);
                    ts.push(1.0);
                    ts.sort_by(|x, y| x.total_cmp(y));
                    ts.dedup_by(|x, y| (*x - *y).abs() < 1e-6);
                    let tc = param(cur.x, cur.y).clamp(0.0, 1.0);
                    for w in ts.windows(2) {
                        if tc >= w[0] - 1e-9 && tc <= w[1] + 1e-9 {
                            let p0 = sh.at(Point2::new(pa.x + dx * w[0], pa.y + dy * w[0]));
                            let p1 = sh.at(Point2::new(pa.x + dx * w[1], pa.y + dy * w[1]));
                            painter.line_segment([p0, p1], Stroke::new(3.0, red));
                            break;
                        }
                    }
                }
                EntityKind::Circle { center, r } => {
                    let Some(c) = pt(center) else { return };
                    let mut angs: Vec<f64> = inter.iter().map(|&(x, y)| (y - c.y).atan2(x - c.x)).collect();
                    draw_curve_trim_span(&PickCtx { project: pn.project, set: pn.set, view: &pn.view }, painter, rect, TrimArc { c, r, span: None }, &mut angs, cur, red);
                }
                EntityKind::Arc { center, a, b, ccw } => {
                    let (Some(c), Some(pa), Some(pb)) = (pt(center), pt(a), pt(b)) else { return };
                    let r = ((pa.x - c.x).powi(2) + (pa.y - c.y).powi(2)).sqrt();
                    let a0 = (pa.y - c.y).atan2(pa.x - c.x);
                    let a1 = (pb.y - c.y).atan2(pb.x - c.x);
                    let mut angs: Vec<f64> = inter.iter().map(|&(x, y)| (y - c.y).atan2(x - c.x)).collect();
                    draw_curve_trim_span(&PickCtx { project: pn.project, set: pn.set, view: &pn.view }, painter, rect, TrimArc { c, r, span: Some((a0, a1, ccw)) }, &mut angs, cur, red);
                }
                EntityKind::Ellipse { .. } => {}
            }
        }
        2 => {
            // EXTEND: in green, the end that will be pulled (the one nearest the cursor)
            let ends = match kind {
                EntityKind::Line { a, b } => vec![pt(a), pt(b)],
                EntityKind::Arc { a, b, .. } => vec![pt(a), pt(b)],
                _ => vec![],
            };
            if let Some(end) = ends.into_iter().flatten().min_by(|p, q| {
                let dp = (p.x - cur.x).powi(2) + (p.y - cur.y).powi(2);
                let dq = (q.x - cur.x).powi(2) + (q.y - cur.y).powi(2);
                dp.total_cmp(&dq)
            }) {
                painter.circle_stroke(sh.at(end), 6.0, Stroke::new(2.0, green));
            }
        }
        3 => {
            // BREAK: a marker at the cut point (the cursor projected onto the entity)
            let split = match kind {
                EntityKind::Line { a, b } => {
                    if let (Some(pa), Some(pb)) = (pt(a), pt(b)) {
                        let (dx, dy) = (pb.x - pa.x, pb.y - pa.y);
                        let len2 = (dx * dx + dy * dy).max(1e-9);
                        let t = (((cur.x - pa.x) * dx + (cur.y - pa.y) * dy) / len2).clamp(0.05, 0.95);
                        Some(Point2::new(pa.x + dx * t, pa.y + dy * t))
                    } else {
                        None
                    }
                }
                EntityKind::Circle { center, r } => pt(center).map(|c| {
                    let ang = (cur.y - c.y).atan2(cur.x - c.x);
                    Point2::new(c.x + r * ang.cos(), c.y + r * ang.sin())
                }),
                EntityKind::Arc { center, a, .. } => match (pt(center), pt(a)) {
                    (Some(c), Some(pa)) => {
                        let r = ((pa.x - c.x).powi(2) + (pa.y - c.y).powi(2)).sqrt();
                        let ang = (cur.y - c.y).atan2(cur.x - c.x);
                        Some(Point2::new(c.x + r * ang.cos(), c.y + r * ang.sin()))
                    }
                    _ => None,
                },
                EntityKind::Ellipse { .. } => None,
            };
            if let Some(p) = split {
                let sp = sh.at(p);
                painter.circle_filled(sp, 4.0, green);
                painter.circle_stroke(sp, 7.0, Stroke::new(1.5, green));
            }
        }
        _ => {}
    }
}

/// The pattern preview: ghosts of the copies from the row's current parameters. The source is either the
/// selection (for a new pattern) or the source of the pattern being edited.
pub fn draw_pattern_preview(pn: &Painting, painter: &egui::Painter, rect: Rect) {
    use qymcad_core::model::PatternKind;
    if pn.armed.pat_op() == 0 {
        return;
    }
    let Sel::Sketch(si) = pn.sel else { return };
    let eids: Vec<Id> = if let Some(pi) = pn.pat.edit {
        pn.project.sketches.get(si).and_then(|s| s.patterns.get(pi)).map(|p| p.source.clone()).unwrap_or_default()
    } else {
        pn.sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect()
    };
    if eids.is_empty() {
        return;
    }
    let kind = qymcad_ui_state::current_pattern_kind(pn.armed, pn.pat, pn.project, pn.sk_pat, si, &eids);
    let stroke = Stroke::new(1.2, pn.scheme.pal.highlight());
    // the centre of rotation of a circular pattern as a cross (so that what it is built around is visible)
    if let qymcad_core::model::PatternKind::Circular { cx, cy, .. } = kind {
        let c = (qymcad_ui_state::Sheet { view: pn.view, rect }).at(Point2::new(cx, cy));
        let m = pn.scheme.pal.pattern_center();
        painter.line_segment([c + egui::vec2(-7.0, 0.0), c + egui::vec2(7.0, 0.0)], Stroke::new(1.5, m));
        painter.line_segment([c + egui::vec2(0.0, -7.0), c + egui::vec2(0.0, 7.0)], Stroke::new(1.5, m));
        painter.circle_stroke(c, 4.0, Stroke::new(1.2, m));
    }
    let transforms: Vec<PatternCopy> = match kind {
        PatternKind::Linear { dx, dy, count, dx2, dy2, count2 } => {
            let mut v: Vec<PatternCopy> = Vec::new();
            for i in 0..count.max(1) {
                for j in 0..count2.max(1) {
                    if i == 0 && j == 0 {
                        continue;
                    }
                    let (ox, oy) = (dx * i as f64 + dx2 * j as f64, dy * i as f64 + dy2 * j as f64);
                    v.push(PatternCopy { cos: 1.0, sin: 0.0, about: (0.0, 0.0), shift: (ox, oy) });
                }
            }
            v
        }
        PatternKind::Circular { cx, cy, count: c, total_deg } => {
            let step = if (total_deg - 360.0).abs() < 1e-6 { total_deg / c as f64 } else { total_deg / c.max(2) as f64 };
            (1..c.max(1))
                .map(|k| {
                    let ang = (step * k as f64).to_radians();
                    PatternCopy { cos: ang.cos(), sin: ang.sin(), about: (cx, cy), shift: (0.0, 0.0) }
                })
                .collect()
        }
    };
    for xf in &transforms {
        for &eid in &eids {
            draw_entity_xform(&PickCtx { project: pn.project, set: pn.set, view: &pn.view }, painter, rect, si, eid, &|x, y| xf.at(x, y), stroke);
        }
    }
}

/// ONE COPY OF A SKETCH PATTERN: turned by the angle whose cosine and sine are given about `about`, then shifted.
pub(crate) struct PatternCopy {
    pub(crate) cos: f64,
    pub(crate) sin: f64,
    pub(crate) about: (f64, f64),
    pub(crate) shift: (f64, f64),
}

impl PatternCopy {
    pub(crate) fn at(&self, x: f64, y: f64) -> (f64, f64) {
        let (vx, vy) = (x - self.about.0, y - self.about.1);
        (self.about.0 + vx * self.cos - vy * self.sin + self.shift.0, self.about.1 + vx * self.sin + vy * self.cos + self.shift.1)
    }
}

/// Waiting for a base point after Ctrl+C/X: the selected geometry is lit green (a hint that this is what
/// will be copied) and a green crosshair is drawn under the cursor (a hint to click the base point).
pub fn draw_clip_pending(pn: &Painting, painter: &egui::Painter, rect: Rect) {
    let Some((eids, _cut)) = pn.clip.geom_pending.as_ref() else { return };
    let Some(si) = qymcad_ui_state::edit_si(pn.project, &pn.sketch_ses) else { return };
    let col = pn.scheme.pal.clip();
    let stroke = Stroke::new(2.2, col);
    let f = |x: f64, y: f64| (x, y);
    for &id in eids {
        draw_entity_xform(&PickCtx { project: pn.project, set: pn.set, view: &pn.view }, painter, rect, si, id, &f, stroke);
    }
    if let Some(cur) = pn.cursor {
        let c = (qymcad_ui_state::Sheet { view: pn.view, rect }).at(cur);
        let arm = 9.0;
        painter.line_segment([c + egui::vec2(-arm, 0.0), c + egui::vec2(arm, 0.0)], Stroke::new(1.6, col));
        painter.line_segment([c + egui::vec2(0.0, -arm), c + egui::vec2(0.0, arm)], Stroke::new(1.6, col));
        painter.circle_stroke(c, 4.0, Stroke::new(1.2, col));
    }
}

/// The glyphs of the selected texts, carried by `f`: the ghost of a text being moved or turned.
pub(crate) fn draw_texts_xform(pn: &Painting, painter: &egui::Painter, rect: Rect, si: usize, f: &dyn Fn(f64, f64) -> (f64, f64), stroke: Stroke) {
    let sh = qymcad_ui_state::Sheet { view: pn.view, rect };
    let Some(s) = pn.project.sketches.get(si) else { return };
    for ti in qymcad_ui_state::sel_text_indices(pn.project, pn.sel_sk, si) {
        for glyph in &s.texts[ti].glyphs {
            let pts: Vec<egui::Pos2> = glyph.iter().map(|p| f(p.x, p.y)).map(|(x, y)| sh.at(Point2::new(x, y))).collect();
            painter.add(egui::Shape::closed_line(pts, stroke));
        }
    }
}

pub fn draw_move_preview(pn: &Painting, painter: &egui::Painter, rect: Rect) {
    let sh = qymcad_ui_state::Sheet { view: pn.view, rect };
    if pn.armed.move_op() == 0 {
        return;
    }
    let Sel::Sketch(si) = pn.sel else { return };
    let Some(base) = pn.tool.move_base else { return };
    // rotation: the ghost is turned by rot_angle about the centre (the angle comes from the popup, not from the cursor)
    if pn.armed.move_op() == 3 {
        let (sn, cs) = (pn.rot.angle.to_radians().sin(), pn.rot.angle.to_radians().cos());
        let col = pn.scheme.pal.preview_axis();
        let stroke = Stroke::new(1.4, col);
        let f = move |x: f64, y: f64| {
            let (px, py) = (x - base.x, y - base.y);
            (base.x + px * cs - py * sn, base.y + px * sn + py * cs)
        };
        for id in pn.sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id) {
            draw_entity_xform(&PickCtx { project: pn.project, set: pn.set, view: &pn.view }, painter, rect, si, id, &f, stroke);
        }
        draw_texts_xform(pn, painter, rect, si, &f, stroke);
        let c = sh.at(base);
        painter.circle_filled(c, 4.0, col); // the centre of rotation
        painter.circle_stroke(c, 9.0, Stroke::new(0.8, col));
        return;
    }
    let Some(cur) = pn.cursor else { return };
    let (dx, dy) = (cur.x - base.x, cur.y - base.y);
    let col = if pn.armed.move_op() == 2 { pn.scheme.pal.clip() } else { pn.scheme.pal.highlight() };
    let stroke = Stroke::new(1.4, col);
    let f = move |x: f64, y: f64| (x + dx, y + dy);
    for (k, id) in pn.sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).enumerate() {
        let _ = k;
        draw_entity_xform(&PickCtx { project: pn.project, set: pn.set, view: &pn.view }, painter, rect, si, id, &f, stroke);
    }
    draw_texts_xform(pn, painter, rect, si, &f, stroke);
    painter.circle_filled(sh.at(base), 3.0, col);
    painter.line_segment([sh.at(base), sh.at(cur)], Stroke::new(0.8, col));
}
