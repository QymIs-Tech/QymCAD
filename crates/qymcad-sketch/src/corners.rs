//! THE CORNER TOOLS on the canvas: the preview of a rounding or a chamfer, its box and what it says.

use super::*;

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
pub(crate) fn draw_corner_preview(cc: &qymcad_ui_state::CornerCtx, ctx: &egui::Context, rect: Rect, si: usize, corners: &[Shown], chamfer: bool, value: f64) {
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
pub(crate) fn corner_cut(cc: &qymcad_ui_state::CornerCtx, chamfer: bool, value: f64) -> qymcad_core::model::CornerCut {
    if !chamfer {
        return qymcad_core::model::CornerCut::Fillet(qymcad_core::model::FilletSize { by: cc.tool_prefs.fillet_by, value });
    }
    let second = parse_num(cc.project, &cc.corner.buf2).unwrap_or(cc.tool_prefs.chamfer_second);
    qymcad_core::model::CornerCut::Chamfer(qymcad_core::model::ChamferLegs { mode: cc.tool_prefs.chamfer_mode, first: value, second })
}

/// THE SET SAID IN WORDS: how many corners one answer will cut.
pub(crate) fn corner_set_says(sk: &qymcad_ui_state::SketchCtx) -> String {
    let n = sk.corner.set.standing().len();
    if n == 0 {
        return qymcad_i18n::tr("sk-click-corner");
    }
    qymcad_i18n::tr1("sk-corner-set-n", "n", &n.to_string())
}

/// WHAT A CLICK AT A POINT DID, said in words rather than left to be read off the drawing.
pub(crate) fn corner_point_says(sk: &qymcad_ui_state::SketchCtx, act: qymcad_ui_state::PointAct) -> String {
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
pub(crate) fn corner_point_name(sk: &qymcad_ui_state::SketchCtx, si: usize, pid: Id) -> String {
    sk.project.point_xy(si, pid).map(|(x, y)| format!("({}, {})", qymcad_i18n::num(x, 2), qymcad_i18n::num(y, 2))).unwrap_or_default()
}

/// THE YELLOW PREVIEW OF A CORNER THAT IS NOT NAMED YET: the corner a click at the point under the cursor WOULD
/// name. It is drawn only while the cursor stands inside the zone of that point, and it is amber because it is not
/// in the set - a click there makes it violet. A point that already has a corner standing in it shows nothing at
/// all: it is one place, and there is nothing to add there.
pub(crate) fn corner_hover_under(cc: &qymcad_ui_state::CornerCtx, ctx: &egui::Context, rect: Rect, si: usize) -> Option<(Id, (Id, Id))> {
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
pub(crate) fn draw_the_corner_preview(cc: &qymcad_ui_state::CornerCtx, ctx: &egui::Context, rect: Rect) {
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
pub(crate) fn value_for_preview(cc: &qymcad_ui_state::CornerCtx) -> f64 {
    parse_num(cc.project, &cc.corner.buf).unwrap_or(cc.tool_prefs.fillet)
}

/// WHERE THE BOX OF THE SET STANDS: beside its first corner, on the outside of it - away from the middle of the cut
/// towards the corner and on past it by `CORNER_BOX_GAP` - and growing that way, so the corner, the two ends the lines
/// are cut at and the arc between them stay in sight. It stood on the place clicked, which is on the corner or on one
/// of its lines: measured on a rectangle 40 x 30, the box covered the corner, both cut ends and the middle of the arc.
///
/// The corner of the box nearest the corner is placed by the size the box had the frame before (`size`). An area given
/// a pivot instead is measured on its first frame and drawn on none: the box did not show at all in the frame it opened.
pub(crate) fn corner_box_place(cc: &qymcad_ui_state::CornerCtx, rect: Rect, si: usize, first: Option<(Id, (Id, Id))>, chamfer: bool, size: egui::Vec2) -> Option<Pos2> {
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
pub(crate) const CORNER_BOX_GUESS: egui::Vec2 = egui::vec2(320.0, 56.0);

/// How far the box of the corner tools stands off the corner, in pixels.
pub(crate) const CORNER_BOX_GAP: f32 = 16.0;

/// The size a corner is read at for the side the box stands on, where the value typed draws no preview, in mm.
pub(crate) const CORNER_BOX_PROBE: f64 = 1e-3;
