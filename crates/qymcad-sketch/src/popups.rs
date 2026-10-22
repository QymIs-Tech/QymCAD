//! THE FIELDS AT THE GEOMETRY of a sketch: the sizes of a new shape, of a corner, of a turn, the editor of a dimension.

use qymcad_ui_state::Notice;
use super::*;

/// THE CORNER UNDER THE POINTER, on the sheet: the side of a corner with more than one to give follows the pointer while
/// the corner is not fixed, and the preview of the cut is drawn. The box of the set is a tool of the frame (`CornerEdit`).
pub fn corner_preview(cc: &mut qymcad_ui_state::CornerCtx, ctx: &egui::Context, rect: Rect) {
    // THE SIDE OF A CORNER WITH MORE THAN ONE TO GIVE IS WHERE THE POINTER IS, while the field is open: the corner of a
    // crossing follows it into the quarter it stands in, and the first leg of a chamfer is the line it is nearer to.
    // Reported behaviour: "the side of the fillet or the chamfer is chosen by where the cursor is, with the preview
    // drawn there".
    let following = cc.corner.aim == qymcad_ui_state::Aim::Following;
    let pointer = ctx.input(|i| i.pointer.hover_pos()).filter(|p| rect.contains(*p));
    if let Some((si, _, _)) = cc.corner.at.filter(|_| following) {
        if let Some(pos) = pointer {
            let w = qymcad_ui_state::to_world(cc.view, rect, pos);
            if cc.corner.set.steer(&*cc.project, si, (w.x, w.y)) == qymcad_ui_state::Steered::Yes {
                cc.corner.near = Some(Point2::new(w.x, w.y));
            }
        }
    }
    // THE PREVIEW IS DRAWN FIRST, AND ALWAYS: it belongs to the sheet and not to the field, and a person deciding
    // where to click cannot see it if the field has to be open before it appears.
    draw_the_corner_preview(cc, ctx, rect);
    // A CORNER FOLLOWING THE POINTER HAS NO BOX YET: the box would stand beside a corner that moves, and run from the
    // pointer that goes to it. A click fixes the corner, and the box opens then.
    if following && cc.corner.at.is_some() {
        *cc.status = Notice::info(qymcad_i18n::tr("sk-corner-click-to-fix"));
    }
}

/// WHAT THE BOX OF A SHAPE HOLDS AS IT OPENS, and where it stands on the sheet: its fields as the shape stands, the point
/// it stands by and the corners of the shape.
pub(crate) struct ShapeNow {
    pub(crate) fields: Vec<qymcad_ui_state::CmdParam>,
    pub(crate) at: Point2,
    pub(crate) around: Vec<Point2>,
}

/// The four corners of box `b`.
fn corners(b: qymcad_core::geom::Bbox) -> Vec<Point2> {
    vec![b.min, Point2::new(b.max.x, b.min.y), b.max, Point2::new(b.min.x, b.max.y)]
}

/// THE SHAPE IN THE BOX, as it stands: an ellipse its width and height (the full axes, twice the semi-axes); a
/// rectangle its width and height; a polygon the radius of its circle and its turn; a rectangle reopened the values of
/// the dimensions it holds them by; a slot its length between the centres of its ends and its width. None when no shape
/// is placed, or the shape went from under the box.
pub(crate) fn shape_now(project: &Project, place: &qymcad_ui_state::Placing, si: usize) -> Option<ShapeNow> {
    use qymcad_ui_state::{CmdParam, PlacingShape};
    match &place.shape {
        PlacingShape::None => None,
        PlacingShape::Ellipse(handle, centre) => {
            let (rx, ry) = project.ellipse_axes(si, *handle)?;
            let fields = vec![CmdParam::new("sk-width-short", "w", 2.0 * rx, 0.02, 1e5), CmdParam::new("sk-height-short", "h", 2.0 * ry, 0.02, 1e5)];
            let hi = Point2::new(centre.x + rx, centre.y + ry);
            Some(ShapeNow { fields, at: hi, around: corners(qymcad_core::geom::Bbox { min: Point2::new(centre.x - rx, centre.y - ry), max: hi }) })
        }
        PlacingShape::Rect { a, b, .. } => Some(rect_now([*a, *b])),
        PlacingShape::RectCenter { center, corner, .. } => Some(rect_now([Point2::new(2.0 * center.x - corner.x, 2.0 * center.y - corner.y), *corner])),
        PlacingShape::Poly(cid) => {
            let (cx, cy, r) = project.polygon_circle(si, *cid)?;
            let ang = project.polygon_angle(si, *cid)?;
            let fields = vec![CmdParam::new("sk-radius", "r", r, 0.01, 1e5), CmdParam::new("sk-angle-deg", "a", ang.to_degrees(), -360.0, 360.0)];
            Some(ShapeNow { fields, at: Point2::new(cx + r, cy), around: corners(qymcad_core::geom::Bbox { min: Point2::new(cx - r, cy - r), max: Point2::new(cx + r, cy + r) }) })
        }
        PlacingShape::RectDims(dims) => {
            let dim = |ci: usize| project.sketches.get(si).and_then(|s| s.constraints.get(ci));
            let Some(qymcad_core::model::Constraint::Distance { d: w, a: at, .. }) = dim(dims.width) else { return None };
            let Some(qymcad_core::model::Constraint::Distance { d: h, .. }) = dim(dims.height) else { return None };
            let corner = qymcad_ui_state::sketch_pt(project, si, *at)?;
            let fields = vec![CmdParam::new("sk-width-short", "w", *w, 0.01, 1e5), CmdParam::new("sk-height-short", "h", *h, 0.01, 1e5)];
            Some(ShapeNow { fields, at: corner, around: vec![corner] })
        }
        PlacingShape::Slot(curve) => {
            use qymcad_core::model::SlotSize;
            let length = project.slot_size(si, *curve, SlotSize::Length)?;
            let width = project.slot_size(si, *curve, SlotSize::Width)?;
            let ends: Vec<Point2> = project
                .sketches
                .get(si)
                .and_then(|s| s.entity(*curve))
                .map(qymcad_core::model::entity_points)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|p| qymcad_ui_state::sketch_pt(project, si, p))
                .collect();
            let fields = vec![CmdParam::new("sk-length-short", "l", length, 0.01, 1e5), CmdParam::new("sk-width-short", "w", width, 0.01, 1e5)];
            Some(ShapeNow { fields, at: *ends.first()?, around: ends })
        }
    }
}

/// A rectangle between the two corners of its diagonal, in its box.
fn rect_now(diagonal: [Point2; 2]) -> ShapeNow {
    let [a, b] = diagonal;
    let fields = vec![
        qymcad_ui_state::CmdParam::new("sk-width-short", "w", (b.x - a.x).abs(), 0.01, 1e5),
        qymcad_ui_state::CmdParam::new("sk-height-short", "h", (b.y - a.y).abs(), 0.01, 1e5),
    ];
    let hi = Point2::new(a.x.max(b.x), a.y.max(b.y));
    ShapeNow { fields, at: hi, around: corners(qymcad_core::geom::Bbox { min: Point2::new(a.x.min(b.x), a.y.min(b.y)), max: hi }) }
}

/// The value field `key` of the box of a shape holds.
fn field(place: &qymcad_ui_state::Placing, key: &str) -> Option<f64> {
    place.fields.iter().find(|p| p.key == key).map(|p| p.val)
}

/// THE SHAPE FOLLOWS A VALUE TYPED into its box: an ellipse takes its axes; a rectangle is rebuilt from the corner it was
/// drawn from, so the signs are kept, or about its centre, so the centre never travels while the numbers are typed; a
/// polygon takes its radius and turn; a rectangle reopened has the values put into its dimensions and is solved. A slot
/// is not touched while it is typed: Enter lays its sizes (`shape_apply`).
pub(crate) fn follow_typed(sk: &mut qymcad_ui_state::SketchCtx, si: usize) {
    use qymcad_ui_state::PlacingShape;
    let w = field(sk.place, "w");
    let h = field(sk.place, "h");
    match sk.place.shape.clone() {
        PlacingShape::Ellipse(handle, _) => {
            if let Some(w) = w {
                if let Some(h) = h {
                    sk.project.set_ellipse_axes(si, handle, w / 2.0, h / 2.0);
                }
            }
        }
        PlacingShape::Rect { a, b, ids } => {
            let nw = w.unwrap_or(1.0);
            let nh = h.unwrap_or(1.0);
            let nb = Point2::new(a.x + if b.x < a.x { -nw } else { nw }, a.y + if b.y < a.y { -nh } else { nh });
            sk.project.delete_entities(si, &ids);
            let nids = sk.project.add_rect_entity(si, a.x, a.y, nb.x, nb.y, qymcad_core::feature::Purpose::Real);
            sk.place.shape = PlacingShape::Rect { a, b: nb, ids: nids };
        }
        PlacingShape::RectCenter { center, ids, .. } => {
            let nb = Point2::new(center.x + w.unwrap_or(1.0) / 2.0, center.y + h.unwrap_or(1.0) / 2.0);
            sk.project.delete_entities(si, &ids);
            let nids = sk.project.add_rect_from_centre(si, center, nb, qymcad_core::feature::Purpose::Real);
            sk.place.shape = PlacingShape::RectCenter { center, corner: nb, ids: nids };
        }
        PlacingShape::Poly(cid) => {
            if let Some(r) = field(sk.place, "r") {
                sk.project.set_polygon_radius(si, cid, r);
            }
            if let Some(a) = field(sk.place, "a") {
                sk.project.set_polygon_angle(si, cid, a.to_radians());
            }
        }
        PlacingShape::RectDims(dims) => {
            let set = |c: Option<&mut qymcad_core::model::Constraint>, v: Option<f64>| {
                if let Some(qymcad_core::model::Constraint::Distance { d, .. }) = c {
                    if let Some(v) = v {
                        *d = v;
                    }
                }
            };
            set(sk.project.sketches[si].constraints.get_mut(dims.width), w);
            set(sk.project.sketches[si].constraints.get_mut(dims.height), h);
            sk.project.solve_sketch(si);
        }
        PlacingShape::Slot(_) | PlacingShape::None => return,
    }
    qymcad_ui_state::invalidate(&mut *sk.regen);
}

/// ENTER IN THE BOX OF A SHAPE: a width or a height typed into a rectangle just drawn is laid as its dimensions, as the
/// diameter of a circle is - a size typed by hand is a size meant; a slot lays the size of each field whose value was
/// changed, the slot not having been touched while it was typed. Either way the box goes.
pub(crate) fn shape_apply(sk: &mut qymcad_ui_state::SketchCtx, si: usize) {
    use qymcad_core::model::SlotSize;
    use qymcad_ui_state::PlacingShape;
    match sk.place.shape.clone() {
        PlacingShape::Rect { ids, .. } | PlacingShape::RectCenter { ids, .. } if sk.place.typed => {
            if let Some(&side) = ids.first() {
                sk.project.dimension_rect(si, side);
                sk.project.solve_sketch(si);
                qymcad_ui_state::invalidate(&mut *sk.regen);
            }
        }
        PlacingShape::Slot(curve) => {
            let mut lay = |key: &str, size: SlotSize| {
                let now = sk.project.slot_size(si, curve, size);
                let typed = field(sk.place, key).filter(|v| Some(*v) != now);
                if let Some(v) = typed {
                    sk.project.set_slot_size(si, curve, size, v);
                }
                typed.is_some()
            };
            let laid = lay("l", SlotSize::Length) | lay("w", SlotSize::Width);
            if laid {
                qymcad_ui_state::invalidate(&mut *sk.regen);
            }
        }
        _ => {}
    }
    sk.place.clear();
}

/// THE BOXES OF THE CORNER TOOL (a fillet radius or a chamfer leg) AND OF THE DISTANCE OF AN OFFSET whose side a click
/// has fixed: Enter applies, Esc cancels.
///
/// `armed` is the tool in hand, and it is here rather than in `Editing` because the preview on the sheet is drawn
/// whether or not the field is up: before the first click there is no field, and the amber corner under the cursor
/// is drawn as the chamfer or the fillet the tool in hand would cut it as.
pub fn place_input_popup(ed: qymcad_ui_state::Editing, mut tools: qymcad_ui_state::PopupTools, looks: qymcad_ui_state::PopupLooks, ctx: &egui::Context, rect: Rect) {
    let qymcad_ui_state::PopupTools { corner, place, sel_sk, armed } = &mut tools;
    corner_preview(
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
    if !matches!(*ed.sel, qymcad_ui_state::Sel::Sketch(_)) {
        place.clear(); // everything unfinished in the drawing at once (otherwise one of the three is forgotten)
    }
}

/// A DOUBLE CLICK ON A SHAPE OPENS ITS SIZE: a side of a rectangle its width and height, a straight side of a slot its
/// length and width, a circle its diameter (laid
/// as a dimension when it has none), an arc its radius, the circle a polygon hangs on - or the polygon itself - its
/// radius and turn. Answers whether a shape was there.
pub fn open_shape_size(sk: &mut qymcad_ui_state::SketchCtx, rect: Rect, pos: Pos2, si: usize) -> bool {
    use qymcad_ui_state::{InlineEdit, PlacingShape};
    if let Some(qymcad_ui_state::SketchItem { kind: qymcad_ui_state::ItemKind::Curve, id: eid }) = sketch_hit(&sk.pick(), rect, pos, si) {
        if let Some(dims) = sk.project.rect_dims(si, eid) {
            sk.project.solve_sketch(si);
            sk.place.reopen(PlacingShape::RectDims(dims));
            sk.place.focus = true;
            return true;
        }
        // A SIDE OF A POLYGON OR A SLOT OPENS THE FIGURE'S SIZE, as a side of a rectangle does: the polygon its radius and
        // turn, the slot its length and width. A polygon opened only within 8 px of its circle, which the middle of a
        // side of a hexagon R50 stands 6.7 mm off.
        if let Some(centre) = qymcad_ui_state::polygon_of_curve(&*sk.project, si, eid) {
            sk.place.reopen(PlacingShape::Poly(centre));
            sk.place.focus = true;
            return true;
        }
        // a straight side of a slot: an end of it is an arc, which opens its own radius below
        let straight = sk.project.sketches[si].entity(eid).is_some_and(|e| e.kind.as_line().is_some());
        if straight && sk.project.slot_size(si, eid, qymcad_core::model::SlotSize::Length).is_some() {
            sk.place.reopen(PlacingShape::Slot(eid));
            sk.place.focus = true;
            return true;
        }
    }
    let round = |eid: &Id| sk.project.sketches[si].entity(*eid).is_some_and(|e| e.kind.as_circle().is_some() || e.kind.as_arc().is_some());
    if let Some(eid) = qymcad_pick::curve_or_caption_under(&sk.pick(), rect, pos, si).filter(round) {
        // a circle has its diameter edited, or the field of its size opened; an arc the field of its radius
        let center = sk.project.sketches[si].entity(eid).and_then(|e| e.kind.as_circle()).map(|k| k.center);
        // the circumscribed circle of a polygon (the vertices hang on it) opens the polygon popup (the radius plus the
        // angle), while an ordinary circle gets a diameter
        let is_poly_rim = center.is_some_and(|c| sk.project.sketches[si].constraints.iter().any(|x| matches!(x, qymcad_core::model::Constraint::PointOnCircle { c: cc, .. } if *cc == c)));
        if let (true, Some(c)) = (is_poly_rim, center) {
            sk.place.reopen(PlacingShape::Poly(c));
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
        sk.place.reopen(PlacingShape::Poly(cid)); // editing the radius of the construction circle
        sk.place.focus = true;
        return true;
    }
    false
}

/// WHAT A TURN OF THE ROTATE TOOL TURNS: the sketch, the centre, the shapes and the texts chosen.
pub(crate) struct Turn {
    si: usize,
    base: Point2,
    eids: Vec<Id>,
    texts: Vec<usize>,
}

/// The turn about `base` of what is chosen in sketch `si`; none with nothing chosen.
pub(crate) fn turn_of(sk: &qymcad_ui_state::SketchCtx, si: usize, base: Point2) -> Option<Turn> {
    let eids = qymcad_ui_state::shapes_chosen(&*sk.project, si, &*sk.sel_sk);
    let texts = qymcad_ui_state::sel_text_indices(&*sk.project, &*sk.sel_sk, si);
    (!eids.is_empty() || !texts.is_empty()).then_some(Turn { si, base, eids, texts })
}

/// TURN WHAT IS CHOSEN by `v` degrees about the centre, one step of undo named after the tool, as the target click of a
/// move is, and say whether it turned.
///
/// WHERE A FREE TURN WOULD HAVE PUT THE POINTS, and where the constraints let them go. A rectangle carries two horizontal
/// and two vertical constraints, so it CANNOT turn: the solver brings it back to upright, merely relocated - measured, a
/// 90 degree turn about zero left the corner at (-5, 5) instead of (0, 0). Saying "Rotated by 90 deg" over that is the
/// same lie the move tool used to tell, and it is told out loud here instead.
pub(crate) fn turn_by(sk: &mut qymcad_ui_state::SketchCtx, turn: &Turn, v: f64) {
    let Turn { si, base, eids, texts } = turn;
    let si = *si;
    qymcad_ui_state::begin_edit(&mut *sk.edits, &*sk.project, qymcad_i18n::tr("tool-rotate"));
    let ids = sk.project.entity_point_ids(si, eids);
    let at = |p: &qymcad_core::model::Project| -> Vec<Point2> {
        p.sketches.get(si).map(|s| ids.iter().filter_map(|id| s.points.iter().find(|q| q.id == *id).map(|q| Point2::new(q.x, q.y))).collect()).unwrap_or_default()
    };
    let before = at(sk.project);
    let sn = v.to_radians().sin();
    let cs = v.to_radians().cos();
    let want: Vec<Point2> = before
        .iter()
        .map(|p| {
            let d = Point2::new(p.x - base.x, p.y - base.y);
            Point2::new(base.x + d.x * cs - d.y * sn, base.y + d.x * sn + d.y * cs)
        })
        .collect();
    if !eids.is_empty() {
        sk.project.rotate_entities(si, eids, base.x, base.y, v);
    }
    for &ti in texts {
        sk.project.rotate_sketch_text(si, ti, base.x, base.y, v);
    }
    let after = at(sk.project);
    let mean = |a: &[Point2], b: &[Point2]| -> f64 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }
        a.iter().zip(b).map(|(p, q)| (q.x - p.x).hypot(q.y - p.y)).sum::<f64>() / a.len() as f64
    };
    let asked = mean(&before, &want); // how far the turn alone would have carried the points
    let off = mean(&after, &want); // how far the result stands from that
                                   // a tenth of what was asked, and at least a hundredth of a millimetre: below that it did turn
    *sk.status = if asked > 1e-9 && off > (asked * 0.1).max(1e-2) {
        Notice::warning(qymcad_i18n::tr("sk-turn-held"))
    } else {
        Notice::info(qymcad_i18n::tr1("sk-rotated-by", "a", &v.to_string()))
    };
    qymcad_ui_state::commit_edit(&mut sk.rebuild());
    qymcad_ui_state::invalidate(&mut *sk.regen);
}
