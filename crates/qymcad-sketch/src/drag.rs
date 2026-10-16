//! DRAGGING IN A SKETCH: points, curves, the power trim, the references a drag reads.

use qymcad_ui_state::Notice;
use super::*;

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
                    for spi in qymcad_ui_state::splines_with_handles(&*sk.project, &*sk.sel_sk, si) {
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
        *sk.status = Notice::info(qymcad_i18n::tr1("sk-trimmed-n", "n", &sk.trim.done.len().to_string()));
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
            // a Move feature, a raw mesh's moved by the document
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
            *sk.status = Notice::info(qymcad_i18n::tr("g-dim-from-centre"));
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
                    *sk.status = Notice::info(qymcad_i18n::tr("sk-offset-side"));
                    // what to do next, not how many were chosen
                }
                // the box chooses what the mirror reflects, in either of its steps, as its Shift + click does
                if sk.sel_sk.modify == Some(qymcad_ui_state::EditTool::Mirror) {
                    qymcad_ui_state::mirror_takes_the_choice(&mut *sk.sel_sk, &mut *sk.status);
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
