//! THE DIMENSIONS of a sketch: placed, edited, driven or driving.

use qymcad_ui_state::Notice;
use super::*;

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

/// RESOLVE A CONFLICT IN ONE CLICK: the conflicting dimension becomes A DRIVEN one - it stops driving the
/// geometry but stays on the drawing and shows the actual value. That is the standard way out in a
/// professional CAD; without it the only option left is to delete the dimension and lose it from the
/// drawing.
///
/// It lives here rather than in the body of the panel: it is an operation on the document, and a test
/// drives it.
pub fn make_dim_driven(project: &mut Project, regen: &mut qymcad_ui_state::Rebuilding, status: &mut qymcad_ui_state::Notice, si: usize, ci: usize) -> bool {
    if project.auto_driven(si, ci) {
        project.solve_sketch(si);
        qymcad_ui_state::invalidate(regen);
        *status = Notice::info(qymcad_i18n::tr("sk-dim-now-driven"));
        true
    } else {
        *status = Notice::refusal(qymcad_i18n::tr("sk-cannot-be-driven"));
        false
    }
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

/// Put the label of angular dimension `ci` where `angle_dim_follow` says: the radius of its arc and its share of the angle.
pub(crate) fn put_angle_label(project: &mut Project, si: usize, ci: usize, (r, t): (f64, Option<f64>)) {
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
pub(crate) fn update_placing_dim(sk: &mut qymcad_ui_state::SketchCtx, rect: Rect, pointer: Option<Pos2>) {
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
        *sk.status = Notice::info(qymcad_i18n::tr("sk-pick-sketch-first"));
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
        *sk.status = Notice::info(qymcad_i18n::tr("sk-dim-placed"));
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
                    *sk.status = Notice::info(qymcad_i18n::tr("sk-enter-diameter"));
                }
            } else {
                *sk.inline = qymcad_ui_state::InlineEdit::Circle(eid); // an arc or a fillet
                sk.dim.focus = true;
                *sk.status = Notice::info(qymcad_i18n::tr("sk-enter-arc-radius"));
            }
            return true;
        }
        // the circumscribed circle of a polygon has its radius edited, through its centre handle
        if let Some(cid) = qymcad_ui_state::polygon_under(&*sk.project, &*sk.view, rect, pos, si) {
            sk.place.set(qymcad_ui_state::PlacingShape::Poly(cid));
            sk.place.focus = true;
            *sk.status = Notice::info(qymcad_i18n::tr("sk-enter-polygon-radius"));
            return true;
        }
        *sk.status = Notice::refusal(qymcad_i18n::tr("sk-click-nearer-circle"));
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
            *sk.status = Notice::info(if sk.dim.first.is_some() { qymcad_i18n::tr("sk-need-second") } else { qymcad_i18n::tr("sk-dim-pick-hint") });
            return true;
        };
        match sk.dim.first.take() {
            None => match r {
                // a coordinate axis as the first reference creates NO provisional length (an axis has no
                // length; this used to breed a service dimension of 1.0 on the axes). It simply waits for
                // the second element.
                qymcad_ui_state::DimRef::Line(a, b) if sk.project.sketches[si].axis_pts.contains(&a) || sk.project.sketches[si].axis_pts.contains(&b) => {
                    sk.dim.first = Some(r);
                    *sk.status = Notice::info(qymcad_i18n::tr("sk-axis-picked"));
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
                        *sk.status = if conflict { Notice::refusal(qymcad_i18n::tr("sk-length-conflict")) } else { Notice::info(qymcad_i18n::tr("sk-length-hint")) };
                    }
                }
                qymcad_ui_state::DimRef::Point(_) => {
                    sk.dim.first = Some(r);
                    *sk.status = Notice::info(qymcad_i18n::tr("sk-point-picked"));
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
                *sk.status = Notice::info(qymcad_i18n::tr("sk-line1-picked"));
            }
        }
        return true;
    }
    // otherwise it is the three-point mode (A, the vertex B, then C)
    let id = if let Some(r) = resolve_sketch_ref(sk, rect, pos, si) {
        qymcad_ui_state::materialize_ref(&mut *sk.project, si, r)
    } else {
        *sk.status = Notice::info(qymcad_i18n::tr("sk-angle-hint"));
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
                Notice::refusal(qymcad_i18n::tr("sk-angle-conflict"))
            } else {
                Notice::info(if redundant { qymcad_i18n::tr("sk-driven-angle") } else { qymcad_i18n::tr1("sk-angle-added", "a", &qymcad_i18n::num(deg, 1)) })
            };
        }
    }
    true
}

/// A PROVISIONAL LENGTH LEFT BEHIND by taking another tool (every way of taking one lets go of the dimension without
/// the document in hand): the sketch it was put on comes back as it was before the pick, as on Esc.
pub(crate) fn drop_abandoned_dim(sk: &mut qymcad_ui_state::SketchCtx) {
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
            Notice::refusal(qymcad_i18n::tr("sk-dim-conflict"))
        } else {
            Notice::info(if redundant { qymcad_i18n::tr("sk-driven-dim-hint") } else { qymcad_i18n::tr("sk-drag-dim-hint") })
        };
    }
}
