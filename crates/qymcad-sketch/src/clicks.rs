//! A CLICK IN A SKETCH: picking, drawing with the tool in hand, the click as the canvas reads it.

use qymcad_ui_state::Notice;
use super::*;

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
        *sk.status = Notice::info(qymcad_i18n::tr("sk-text-selected"));
        return;
    }
    sk.annot.text = None;
    // a click on a note picks it (Del removes it, a double click edits it, dragging moves it)
    if let Some(ni) = qymcad_ui_state::note_at(&*sk.project, &*sk.view, rect, pos, si) {
        sk.annot.note = Some(ni);
        sk.gsel.constraint = None;
        sk.sel_sk.clear(); // the selection and whatever was waiting for it
        *sk.status = Notice::info(qymcad_i18n::tr("sk-note-selected"));
        return;
    }
    sk.annot.note = None;
    // a click on a constraint glyph or a dimension caption PICKS it (Del removes it) rather than
    // deleting it at once
    if let Some(ci) = constraint_glyph_at(sk, rect, pos, si).or_else(|| dim_at(sk, rect, pos, si)) {
        sk.gsel.constraint = Some(ci);
        sk.sel_sk.clear(); // the selection and whatever was waiting for it
        *sk.status = Notice::info(qymcad_i18n::tr("sk-constraint-selected"));
        return;
    }
    sk.gsel.constraint = None;
    // while a constraint or modify tool is active, the selection accumulates WITHOUT Shift: elements are
    // clicked one at a time, and the constraint applies as soon as there are enough of them.
    let shift = additive; // Shift itself: in the mirror's second step it adds, and a plain click names the axis
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
    // THE MIRROR'S SECOND STEP: a plain click NAMES WHAT IT REFLECTS ABOUT - a line, a point or an axis - and Shift adds
    // the curve under it to what is reflected.
    //
    // It stands before the ordinary selection on purpose. In this state the person has already said what to
    // reflect; letting the click fall through to the selection would quietly replace the answer to the first
    // question with an answer to the second.
    if !sk.sel_sk.mirror_of.is_empty() {
        if shift {
            if let Some(refr @ (1, _)) = hit {
                match sk.sel_sk.items.iter().position(|r| *r == refr) {
                    Some(k) => {
                        sk.sel_sk.items.remove(k);
                    }
                    None => sk.sel_sk.items.push(refr),
                }
                qymcad_ui_state::mirror_takes_the_choice(&mut *sk.sel_sk, &mut *sk.status);
            }
            return;
        }
        match hit {
            Some((3, w)) => qymcad_ui_state::mirror_about_axis(qymcad_ui_state::editing_in!(sk), &mut *sk.sel_sk, w as usize),
            Some((0, p)) => qymcad_ui_state::mirror_about_point(qymcad_ui_state::editing_in!(sk), &mut *sk.sel_sk, p),
            Some((1, eid)) => match sk.project.sketches.get(si).and_then(|s| s.entities.iter().find(|e| e.id == eid)).map(|e| e.kind) {
                // CONSTRUCTION GEOMETRY SERVES AS WELL: what matters is that two points make a line, and a
                // construction line is exactly what people draw to mirror about.
                Some(qymcad_core::model::EntityKind::Line { a, b }) => qymcad_ui_state::mirror_about_line(qymcad_ui_state::editing_in!(sk), &mut *sk.sel_sk, a, b),
                _ => *sk.status = Notice::info(qymcad_i18n::tr("sk-mirror-pick-axis")),
            },
            _ => *sk.status = Notice::info(qymcad_i18n::tr("sk-mirror-pick-axis")),
        }
        return;
    }
    // THE MIRROR'S FIRST STEP, the tool taken with nothing chosen: the first click on a curve chooses what it reflects
    // and the next plain click names what about. Reported behaviour: "with the tool first, the first click chooses the
    // geometry to mirror; then Shift + click or Shift + box adds, and a plain click mirrors about what it is on".
    if sk.sel_sk.modify == Some(qymcad_ui_state::EditTool::Mirror) {
        if let Some(refr @ (1, _)) = hit {
            sk.sel_sk.items.retain(|r| r.0 == 1);
            if !sk.sel_sk.items.contains(&refr) {
                sk.sel_sk.items.push(refr);
            }
            qymcad_ui_state::mirror_takes_the_choice(&mut *sk.sel_sk, &mut *sk.status);
        } else {
            *sk.status = Notice::info(qymcad_i18n::tr("sk-mirror-pick-what"));
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
                    *sk.status = Notice::info(qymcad_i18n::tr("sk-midpoint-picked"));
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
            *sk.status = Notice::info(qymcad_i18n::tr("sk-offset-side"));
        }
    } else if let Some(op) = sk.sel_sk.modify {
        if qymcad_ui_state::try_modify(qymcad_ui_state::editing_in!(sk), &mut *sk.sel_sk, *sk.sk_pat, &*sk.tool_prefs, op) {
            sk.sel_sk.modify = None;
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
    // the places clicked for the shape, this one with them, and the points standing before it: what the click puts down
    // is tied to what it landed on (`tie_what_landed`)
    let here = snap_world(sk, rect, pos);
    let clicks: Vec<Point2> = sk.tool.pts.iter().copied().chain([here]).collect();
    let si = qymcad_ui_state::edit_si(&*sk.project, &*sk.sketch_ses);
    let before: std::collections::HashSet<Id> = si.and_then(|si| sk.project.sketches.get(si)).map(|s| s.points.iter().map(|q| q.id).collect()).unwrap_or_default();
    sketch_tool_click_inner(sk, rect, pos);
    if let Some(si) = si.filter(|_| tied(sk)) {
        let made: std::collections::HashSet<Id> = sk.project.sketches.get(si).map(|s| s.points.iter().map(|q| q.id).filter(|id| !before.contains(id)).collect()).unwrap_or_default();
        if !made.is_empty() {
            tie_what_landed(&mut *sk.project, *sk.view, si, &clicks, &made);
            sk.project.solve_sketch(si);
            qymcad_ui_state::invalidate(&mut *sk.regen);
        }
    }
    qymcad_ui_state::commit_edit_if_changed(&mut sk.rebuild());
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
                    *sk.status = Notice::info(qymcad_i18n::tr("sk-tangent-base-picked"));
                } else {
                    *sk.status = Notice::info(qymcad_i18n::tr("sk-tangent-pick-base"));
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
                    *sk.status = Notice::refusal(qymcad_i18n::tr("sk-centre-on-edge"));
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
                        *sk.status = Notice::refusal(qymcad_i18n::tr("sk-arc-end-on-tangent"));
                    }
                } else {
                    *sk.status = Notice::info(qymcad_i18n::tr("sk-tangent-arc-hint"));
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
                        *sk.status = Notice::refusal(qymcad_i18n::tr("sk-points-collinear-arc"));
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
                *sk.status = Notice::refusal(qymcad_i18n::tr("sk-polygon-no-size"));
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
            // A CLICK ON THE FIRST NODE CLOSES IT: the spline becomes a loop through its nodes, not a curve ending on its
            // own start with a node lying over another - a loop dragged by a node stays closed.
            if sk.tool.pts.len() >= 3 && sk.tool.pts.first().is_some_and(|first| same_place(sk, *first, w)) {
                lay_the_spline(sk, qymcad_core::feature::Ends::Closed);
            } else if !sk.tool.pts.last().is_some_and(|last| same_place(sk, *last, w)) {
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
                    *sk.status = Notice::refusal(qymcad_i18n::tr("sk-points-collinear-circle"));
                }
                sk.tool.pts.clear();
            }
        }
        11 if sk.tool_prefs.text_note => {
            // a text note, which is not geometry
            if !sk.tool_prefs.text.trim().is_empty() {
                sk.project.add_note(si, w.x, w.y, sk.tool_prefs.text.clone());
                *sk.status = Notice::info(qymcad_i18n::tr("sk-note-added"));
                *sk.armed = qymcad_ui_state::Armed::None; // placed once, as a label is
                sk.tool_prefs.text.clear();
            }
        }
        11 if sk.tool_prefs.text.trim().is_empty() => {
            // AN EMPTY STRING IS NO REASON TO STAY SILENT. The tool was armed, the click went through, and
            // nothing happened: there was no telling whether the program was broken or something had been
            // left undone. Emptied by the font, which could not write the letters typed, it is the font that is
            // named.
            *sk.status = Notice::refusal(if sk.tool.text_refused {
                qymcad_i18n::tr1("sk-text-no-letters", "name", &qymcad_ui_state::font_label(&sk.tool_prefs.font, "opt-font"))
            } else {
                qymcad_i18n::tr("sk-text-needs-string")
            });
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
                *sk.status = Notice::info(qymcad_i18n::tr("sk-text-on-label"));
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
                *sk.status = Notice::info(qymcad_i18n::tr1("sk-text-placed", "n", &n.to_string()));
                // A LABEL IS PLACED ONCE: the tool is put down, as in the CAD programs people know. Kept in hand it laid
                // a copy of the label on every stray click until Esc (found checking issue #32). Its string goes too: the
                // tool taken again offered it, and a click placed the same label once more; the height and the font
                // stay as they were set.
                *sk.armed = qymcad_ui_state::Armed::None;
                sk.tool_prefs.text.clear();
            } else if !qymcad_ui_state::any_font_at_all(&mut *sk.font_cache) {
                *sk.status = Notice::refusal(qymcad_i18n::tr("sk-font-not-found"));
            } else if !qymcad_ui_state::font_can_write(&mut *sk.font_cache, &font, &sk.tool_prefs.text) {
                // THE FONT IS NAMED, because the string is not the one at fault. The list marks a face that
                // cannot write its own name, which is a different question: a font with Latin and no Cyrillic
                // looks good there and writes nothing at all in Russian. Here the string is known.
                *sk.status = Notice::refusal(qymcad_i18n::tr1("sk-text-no-letters", "name", &qymcad_ui_state::font_label(&font, "opt-font")));
            } else {
                *sk.status = Notice::refusal(qymcad_i18n::tr("sk-text-empty"));
            }
        }
        _ => {}
    }
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
                        *sk.status = Notice::refusal(qymcad_i18n::tr("sk-click-shape-line"));
                    }
                }
            } else if sk.armed.draw_kind() != 0 && !qymcad_ui_state::bar_fields_valid(ctx) {
                // A FIELD OF THE BAR THAT CANNOT BE TAKEN stops the click as it stops Enter: the shape would
                // be made with the last value the field held, not with what is written in it
                *sk.status = Notice::refusal(qymcad_i18n::tr("sk-fix-bar-field"));
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
                    *sk.status =
                        Notice::info(qymcad_i18n::trn("sk-distance-dxdy", &[("d", &qymcad_i18n::num(d, 3)), ("dx", &qymcad_i18n::num(b.x - a.x, 3)), ("dy", &qymcad_i18n::num(b.y - a.y, 3))]));
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
                        *sk.status = Notice::info(corner_set_says(sk));
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
                        qymcad_ui_state::CornerClick::LineChosen(_) | qymcad_ui_state::CornerClick::LineLetGo(_) => *sk.status = Notice::info(corner_set_says(sk)),
                        // A THIRD LINE AT A STRAIGHT JOINT IS NOT TAKEN, and the point it would have stood
                        // at is named: there is no corner to cut there, and saying so is better than a
                        // line that was not taken for no visible reason.
                        qymcad_ui_state::CornerClick::StraightJoint(pid) => {
                            let name = corner_point_name(sk, si, pid);
                            *sk.status = Notice::refusal(qymcad_i18n::tr1("sk-corner-straight", "p", &name));
                        }
                        qymcad_ui_state::CornerClick::AtPoint(act) => *sk.status = Notice::info(corner_point_says(sk, act)),
                        // A CLICK ON NOTHING CHANGES NOTHING AND SAYS NOTHING. It used to speak, and what it said was a complaint
                        // about the click - that there is no line or point under it - which is the one thing a
                        // person who clicked empty sheet already knows, and which covered the status line of a
                        // tool that was working. A click that names nothing is not a mistake to be answered.
                        qymcad_ui_state::CornerClick::SingleMode => {}
                        // THE FIELD WAS NOT STANDING AND THIS CLICK NAMED NOTHING: the tool is in hand and
                        // says what it waits for, which is the one thing a person who has just taken it needs.
                        // With the field already up it says nothing: the click opened nothing, and the tool is
                        // already saying what it waits for in the hint on its own button.
                        qymcad_ui_state::CornerClick::Opened => *sk.status = Notice::info(qymcad_i18n::tr(if chamfer { "tb-chamfer-sketch-hint" } else { "tb-fillet-sketch-hint" })),
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
                        *sk.status = Notice::info(qymcad_i18n::tr("sk-done"));
                    } else if line_eid.is_none() && qymcad_pick::nearest_circle_entity(&sk.pick(), rect, pos, si).is_none() {
                        *sk.status = Notice::info(qymcad_i18n::tr("sk-click-curve"));
                    } else {
                        *sk.status = Notice::refusal(qymcad_i18n::tr("sk-op-failed-no-intersection"));
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
                *sk.status = Notice::info(qymcad_i18n::tr2("sk-clipboard", "what", &if cut { qymcad_i18n::tr("sk-cut-done") } else { qymcad_i18n::tr("sk-copied") }, "n", &n.to_string()));
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
                    *sk.status = Notice::info(qymcad_i18n::tr("sk-pasted"));
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
                        Notice::info(if sk.armed.pat_op() == 2 { qymcad_i18n::tr("sk-click-rot-centre") } else { qymcad_i18n::tr("sk-params-above") })
                    } else {
                        Notice::info(qymcad_i18n::tr("sk-click-for-array"))
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
                    *sk.status = Notice::info(qymcad_i18n::tr("sk-centre-set-params"));
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
                        *sk.status = Notice::info(qymcad_i18n::tr("sk-click-for-move"));
                    } else {
                        *sk.status = Notice::info(qymcad_i18n::tr("sk-click-base-point"));
                    }
                } else if sk.tool.move_base.is_none() {
                    sk.tool.move_base = Some(w);
                    if sk.armed.move_op() == 3 {
                        // the centre is set, so the angle is typed in the popup - no target click is
                        // awaited
                        sk.rot.angle = 0.0;
                        sk.rot.buf = "0".into();
                        sk.rot.focus = true;
                        *sk.status = Notice::info(qymcad_i18n::tr("sk-centre-set-angle"));
                    } else {
                        *sk.status = Notice::info(qymcad_i18n::tr("sk-click-target-point"));
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
                        *sk.status = Notice::info(qymcad_i18n::tr("sk-copied"));
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
                        *sk.status = if asked > 1e-9 && went < (asked * 0.1).max(1e-2) { Notice::warning(qymcad_i18n::tr("sk-move-held")) } else { Notice::info(qymcad_i18n::tr("sk-moved")) };
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
                        *sk.status = Notice::info(format!("{} {}", ph::CHECK, qymcad_i18n::tr1("g-rev-axis", "what", &qymcad_ui_state::axis_line_label(&*sk.project, si, eid, n))));
                    }
                    None => *sk.status = Notice::refusal(qymcad_i18n::tr("sk-miss-line")),
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
                    *sk.status = Notice::info(qymcad_i18n::tr("sk-contour-picked"));
                } else {
                    *sk.status = Notice::refusal(qymcad_i18n::tr("sk-miss-contour"));
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
                        *sk.status = Notice::info(qymcad_i18n::tr1("sk-profiles-n", "n", &sk.gsel.profiles.len().to_string()));
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
