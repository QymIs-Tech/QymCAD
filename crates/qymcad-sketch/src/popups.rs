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

/// WHAT THE BOX OF A SHAPE HOLDS AS IT OPENS, and where it stands on the sheet: its fields as the shape stands, what
/// each of them sizes, the point it stands by and the corners of the shape.
pub(crate) struct ShapeNow {
    pub(crate) fields: Vec<qymcad_ui_state::CmdParam>,
    pub(crate) sizes: Vec<Size>,
    pub(crate) at: Point2,
    pub(crate) around: Vec<Point2>,
}

/// ONE SIZE OF A SHAPE IN ITS BOX: the key of its field, its kind, and whether a dimension holds it.
pub(crate) struct Size {
    pub(crate) key: &'static str,
    pub(crate) kind: Kind,
    pub(crate) held: Held,
}

/// IS A SIZE OF A SHAPE HELD BY A DIMENSION once laid - so its row is a dimension's, with "ref." and a name - or free:
/// the turn of a polygon is a freedom of the solve, not a dimension, and the axis of an ellipse is held by its half.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Held {
    Dimension,
    Free,
}

/// A size of a shape held by a dimension.
fn dimension(key: &'static str, kind: Kind) -> Size {
    Size { key, kind, held: Held::Dimension }
}

/// The field of a size of a shape, written as a dimension writes it - "40" and not "40.00".
fn as_dimension(mut p: qymcad_ui_state::CmdParam) -> qymcad_ui_state::CmdParam {
    p.txt = qymcad_core::expr::fmt_num(p.val);
    p
}

/// The four corners of box `b`.
fn corners(b: qymcad_core::geom::Bbox) -> Vec<Point2> {
    vec![b.min, Point2::new(b.max.x, b.min.y), b.max, Point2::new(b.min.x, b.max.y)]
}

/// The corners of the square of half side `r` about `c`.
fn square(c: Point2, r: f64) -> Vec<Point2> {
    corners(qymcad_core::geom::Bbox { min: Point2::new(c.x - r, c.y - r), max: Point2::new(c.x + r, c.y + r) })
}

/// HOW A ROUND SIZE IS SHOWN in its box: as asked, or as its shape lays it - a circle by its diameter, an arc and a polygon
/// by their radius.
fn shown_of(place: &qymcad_ui_state::Placing, key: &str, natural: qymcad_ui_state::Shown) -> qymcad_ui_state::Shown {
    place.asks.of(key).shown.unwrap_or(natural)
}

/// The caption key of a round size shown as `shown`.
fn round_label(shown: qymcad_ui_state::Shown) -> &'static str {
    match shown {
        qymcad_ui_state::Shown::Diameter => "sk-diameter",
        qymcad_ui_state::Shown::Radius => "sk-radius",
    }
}

/// A radius `r` as a round size shown as `shown` writes it.
fn round_value(r: f64, shown: qymcad_ui_state::Shown) -> f64 {
    if shown == qymcad_ui_state::Shown::Diameter {
        2.0 * r
    } else {
        r
    }
}

/// THE SHAPE IN THE BOX, as it stands: an ellipse its width and height (the full axes, twice the semi-axes); a
/// rectangle its width and height; a polygon the radius of its circle and its turn; a rectangle reopened the values of
/// the dimensions it holds them by; a slot its length between the centres of its ends and its width; a circle its
/// diameter, an arc its radius. None when no shape is placed, or the shape went from under the box.
pub(crate) fn shape_now(project: &Project, place: &qymcad_ui_state::Placing, si: usize) -> Option<ShapeNow> {
    use qymcad_ui_state::{PlacingShape, Shown};
    let length = |key| dimension(key, Kind::Length);
    match &place.shape {
        PlacingShape::None => None,
        PlacingShape::Ellipse(handle, centre) => {
            let (rx, ry) = project.ellipse_axes(si, *handle)?;
            // its turn as the polygon's: the direction of its major axis, written to a hundredth of a degree
            let deg = project.kept_number(*handle, "turn").unwrap_or(0.0);
            let mut turn = qymcad_ui_state::CmdParam::new("sk-angle", "turn", deg, -360.0, 360.0).unit("unit-deg-suffix");
            turn.txt = qymcad_core::expr::fmt_num((deg * 100.0).round() / 100.0);
            let fields = vec![
                as_dimension(qymcad_ui_state::CmdParam::new("sk-width", "w", 2.0 * rx, 0.02, MAX_SKETCH_LENGTH)),
                as_dimension(qymcad_ui_state::CmdParam::new("sk-height", "h", 2.0 * ry, 0.02, MAX_SKETCH_LENGTH)),
                turn,
            ];
            let free = |key, kind| Size { key, kind, held: Held::Free };
            let hi = Point2::new(centre.x + rx, centre.y + ry);
            let sizes = vec![free("w", Kind::Length), free("h", Kind::Length), free("turn", Kind::Angle)];
            Some(ShapeNow { fields, sizes, at: hi, around: corners(qymcad_core::geom::Bbox { min: Point2::new(centre.x - rx, centre.y - ry), max: hi }) })
        }
        PlacingShape::Rect { a, b, .. } => Some(rect_now([*a, *b])),
        PlacingShape::RectCenter { center, corner, .. } => Some(rect_now([Point2::new(2.0 * center.x - corner.x, 2.0 * center.y - corner.y), *corner])),
        PlacingShape::Poly(cid) => {
            let (cx, cy, r) = project.polygon_circle(si, *cid)?;
            let ang = project.polygon_angle(si, *cid)?;
            let shown = shown_of(place, "r", Shown::Radius);
            let mut turn = qymcad_ui_state::CmdParam::new("sk-angle", "a", ang.to_degrees(), -360.0, 360.0).unit("unit-deg-suffix");
            // written to a hundredth of a degree: "-28.1701" read as noise; the value itself stays as the shape holds it
            turn.txt = qymcad_core::expr::fmt_num((ang.to_degrees() * 100.0).round() / 100.0);
            let fields = vec![as_dimension(qymcad_ui_state::CmdParam::new(round_label(shown), "r", round_value(r, shown), 0.01, MAX_SKETCH_LENGTH)), turn];
            let sizes = vec![dimension("r", Kind::Round(shown)), Size { key: "a", kind: Kind::Angle, held: Held::Free }];
            Some(ShapeNow { fields, sizes, at: Point2::new(cx + r, cy), around: square(Point2::new(cx, cy), r) })
        }
        PlacingShape::RectSide(side) => {
            let [c0, c1, c2, c3] = project.rect_corners(si, *side)?;
            let fields = vec![
                as_dimension(qymcad_ui_state::CmdParam::new("sk-width", "w", c0.dist(c1), 0.01, MAX_SKETCH_LENGTH)),
                as_dimension(qymcad_ui_state::CmdParam::new("sk-height", "h", c1.dist(c2), 0.01, MAX_SKETCH_LENGTH)),
            ];
            Some(ShapeNow { fields, sizes: vec![length("w"), length("h")], at: c2, around: vec![c0, c1, c2, c3] })
        }
        PlacingShape::Slot(curve) => {
            use qymcad_core::model::SlotSize;
            let length_now = project.slot_size(si, *curve, SlotSize::Length)?;
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
            let fields = vec![
                as_dimension(qymcad_ui_state::CmdParam::new("sk-length", "l", length_now, 0.01, MAX_SKETCH_LENGTH)),
                as_dimension(qymcad_ui_state::CmdParam::new("sk-width", "w", width, 0.01, MAX_SKETCH_LENGTH)),
            ];
            Some(ShapeNow { fields, sizes: vec![length("l"), length("w")], at: *ends.first()?, around: ends })
        }
        PlacingShape::Round(eid) => {
            let s = project.sketches.get(si)?;
            let e = s.entity(*eid)?;
            let k = qymcad_core::model::circle_of(s, e)?;
            let c = qymcad_ui_state::sketch_pt(project, si, k.center)?;
            let natural = if e.kind.as_arc().is_some() { Shown::Radius } else { Shown::Diameter };
            let shown = shown_of(place, "size", natural);
            let fields = vec![as_dimension(qymcad_ui_state::CmdParam::new(round_label(shown), "size", round_value(k.r, shown), 0.0, MAX_SKETCH_LENGTH).nonzero())];
            Some(ShapeNow { fields, sizes: vec![dimension("size", Kind::Round(shown))], at: Point2::new(c.x + k.r, c.y), around: square(c, k.r) })
        }
    }
}

/// A rectangle between the two corners of its diagonal, in its box.
fn rect_now(diagonal: [Point2; 2]) -> ShapeNow {
    let [a, b] = diagonal;
    let fields = vec![
        as_dimension(qymcad_ui_state::CmdParam::new("sk-width", "w", (b.x - a.x).abs(), 0.01, MAX_SKETCH_LENGTH)),
        as_dimension(qymcad_ui_state::CmdParam::new("sk-height", "h", (b.y - a.y).abs(), 0.01, MAX_SKETCH_LENGTH)),
    ];
    let hi = Point2::new(a.x.max(b.x), a.y.max(b.y));
    let sizes = vec![dimension("w", Kind::Length), dimension("h", Kind::Length)];
    ShapeNow { fields, sizes, at: hi, around: corners(qymcad_core::geom::Bbox { min: Point2::new(a.x.min(b.x), a.y.min(b.y)), max: hi }) }
}

/// THE BOX OF A SHAPE AS ITS ROWS SAY IT: the layout, and the names and references of its dimensions.
pub(crate) struct ShapeBox {
    pub(crate) layout: qymcad_ui_state::Layout,
    pub(crate) names: Vec<qymcad_ui_state::FieldName>,
}

/// THE BOX OF THE SIZES OF A SHAPE IS THE BOX OF ITS DIMENSIONS, a row of a dimension for each size a dimension will hold
/// - the same rows as the box of the dimension once laid (`dim_rows`) - and a plain row for a size no dimension holds.
pub(crate) fn shape_box(place: &qymcad_ui_state::Placing, now: &ShapeNow) -> ShapeBox {
    let mut rows = Vec::new();
    let mut names = Vec::new();
    for (at, size) in now.sizes.iter().enumerate() {
        let Some(p) = place.fields.iter().find(|p| p.key == size.key) else { continue };
        let caption = match size.kind {
            Kind::Round(_) => row_caption(size.kind),
            _ => qymcad_ui_state::field_caption(p),
        };
        if size.held == Held::Free {
            // the unit of a free size is its field's own (`CmdParam::unit`), written after it by the frame
            // its driver is the item's own, named by the tool in hand (`InSketch::name_kept_values`): no dimension holds the
            // axes of an ellipse or the turn of a polygon
            rows.push(qymcad_ui_state::Row::new(caption).field(size.key));
            continue;
        }
        // its name and its reference are the frame's, kept as typed and switched until Enter lays the dimension
        names.push(qymcad_ui_state::FieldName::of(size.key, String::new(), None).with_driven(qymcad_ui_state::Lit::Off));
        rows.extend(dim_rows(DimRows { caption, field: size.key, at, kind: size.kind }));
    }
    ShapeBox { layout: dim_layout(rows), names }
}

/// THE RADIUS OR THE DIAMETER OF A ROUND SIZE SWITCHED in the box of a shape, the value in the field written anew in it;
/// kept until Enter lays the dimension. Answers whether `did` was it.
pub(crate) fn shape_took(sk: &mut qymcad_ui_state::SketchCtx, si: usize, did: &qymcad_ui_state::Did) -> bool {
    use qymcad_ui_state::{Did, Shown};
    let Some(now) = shape_now(sk.project, sk.place, si) else { return false };
    let Did::Pressed { key } = did else { return false };
    if key.name != ROUND {
        return false;
    }
    let Some(Size { key: field, kind: Kind::Round(was), .. }) = now.sizes.get(key.at) else { return false };
    let shown = if *was == Shown::Diameter { Shown::Radius } else { Shown::Diameter };
    sk.place.asks.at(field).shown = Some(shown);
    if let Some(p) = sk.place.fields.iter_mut().find(|p| p.key == *field) {
        p.set_label(round_label(shown));
        // a number is written anew as the other size; a formula stays as it was typed
        if !is_formula(&p.txt) {
            p.val = if shown == Shown::Radius { p.val * 0.5 } else { p.val * 2.0 };
            p.txt = qymcad_core::expr::fmt_num(p.val);
        }
    }
    true
}

/// The value field `key` of the box of a shape holds.
fn field(place: &qymcad_ui_state::Placing, key: &str) -> Option<f64> {
    place.fields.iter().find(|p| p.key == key).map(|p| p.val)
}

/// A radius out of round field `key` of the box of a shape, shown as `natural` unless asked otherwise.
fn radius_of(place: &qymcad_ui_state::Placing, key: &str, natural: qymcad_ui_state::Shown) -> Option<f64> {
    let v = field(place, key)?;
    Some(if shown_of(place, key, natural) == qymcad_ui_state::Shown::Diameter { v * 0.5 } else { v })
}

/// THE SHAPE FOLLOWS A VALUE TYPED into its box: an ellipse takes its axes; a rectangle is rebuilt from the corner it was
/// drawn from, so the signs are kept, or about its centre, so the centre never travels while the numbers are typed; a
/// polygon takes its radius and turn. A rectangle reopened, a slot, a circle and an arc are not touched while they are
/// typed: Enter lays their sizes (`shape_apply`).
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
            // the turn typed turns it, the axes kept
            if let Some(t) = field(sk.place, "turn").filter(|t| sk.project.kept_number(handle, "turn").is_some_and(|now| (now - t).abs() > 1e-9)) {
                sk.project.set_ellipse_turn(si, handle, t.to_radians());
            }
        }
        // A RECTANGLE GOES WITH ITS SIZES BY ITS CORNERS MOVED, from the corner it was drawn from or about its centre: the
        // same lines and points, so the ties a snap gave them hold
        PlacingShape::Rect { a, b, ids } => {
            let size = qymcad_core::model::RectSize { width: w.unwrap_or(1.0), height: h.unwrap_or(1.0) };
            let nb = Point2::new(a.x + if b.x < a.x { -size.width } else { size.width }, a.y + if b.y < a.y { -size.height } else { size.height });
            if let Some(&side) = ids.first() {
                sk.project.size_rect(si, side, size);
            }
            sk.place.shape = PlacingShape::Rect { a, b: nb, ids };
        }
        PlacingShape::RectCenter { center, ids, .. } => {
            let size = qymcad_core::model::RectSize { width: w.unwrap_or(1.0), height: h.unwrap_or(1.0) };
            let nb = Point2::new(center.x + size.width / 2.0, center.y + size.height / 2.0);
            if let Some(&side) = ids.first() {
                sk.project.size_rect(si, side, size);
            }
            sk.place.shape = PlacingShape::RectCenter { center, corner: nb, ids };
        }
        PlacingShape::Poly(cid) => {
            if let Some(r) = radius_of(sk.place, "r", qymcad_ui_state::Shown::Radius) {
                sk.project.set_polygon_radius(si, cid, r);
            }
            if let Some(a) = field(sk.place, "a") {
                sk.project.set_polygon_angle(si, cid, a.to_radians());
            }
        }
        PlacingShape::Slot(_) | PlacingShape::Round(_) | PlacingShape::RectSide(_) | PlacingShape::None => return,
    }
    qymcad_ui_state::invalidate(&mut *sk.regen);
}

/// HAS A SHAPE TAKEN THE NUMBERS OF ITS BOX as they were typed, or does Enter give them to its dimensions.
#[derive(Clone, Copy)]
enum Sized {
    Followed,
    ByValue,
}

/// A DIMENSION THE BOX OF A SHAPE LAID, by the key of the field that sizes it.
struct Laid {
    key: &'static str,
    dim: Dim,
}

/// THE SIZES OF A SHAPE LAID AS DIMENSIONS - a size typed by hand is a size meant - with what was asked
/// of them in their rows: a reference, a name, the radius shown for a diameter, the formula typed. A rectangle lays its
/// width and height once a size of it is typed or touched; a slot each size changed or touched; a circle or an arc its
/// size, a polygon its radius, when typed or touched. A shape closed untouched is left free.
pub(crate) fn shape_lay(sk: &mut qymcad_ui_state::SketchCtx, si: usize, rows: &[qymcad_ui_state::FieldAsk]) {
    use qymcad_core::model::SlotSize;
    use qymcad_ui_state::{PlacingShape, Shown};
    let typed = sk.place.typed;
    let asks = sk.place.asks.clone();
    let fields = sk.place.fields.clone();
    let row = |key: &str| rows.iter().find(|a| a.field == key);
    // asked in the row of a size: its radius or diameter, a reference, a name
    let asked = |key: &str| asks.touched(key) || row(key).is_some_and(|a| a.driven == qymcad_ui_state::Lit::On || !a.name.trim().is_empty());
    let wanted = |key: &str| typed || asked(key);
    let mut laid: Vec<Laid> = Vec::new();
    // a shape that followed its fields as they were typed holds the numbers already, and a dimension laid on it is given a
    // formula only; a rectangle reopened was not touched, and its dimensions are given the values typed
    let mut sized = Sized::Followed;
    match sk.place.shape.clone() {
        PlacingShape::Rect { ids, .. } | PlacingShape::RectCenter { ids, .. } if wanted("w") || wanted("h") => {
            if let Some(dims) = ids.first().and_then(|side| sk.project.rect_dims(si, *side)) {
                sk.project.solve_sketch(si);
                laid = vec![Laid { key: "w", dim: Dim { si, ci: dims.width } }, Laid { key: "h", dim: Dim { si, ci: dims.height } }];
            }
        }
        PlacingShape::RectSide(side) if wanted("w") || wanted("h") => {
            if let Some(dims) = sk.project.rect_dims(si, side) {
                laid = vec![Laid { key: "w", dim: Dim { si, ci: dims.width } }, Laid { key: "h", dim: Dim { si, ci: dims.height } }];
                sized = Sized::ByValue;
            }
        }
        PlacingShape::Slot(curve) => {
            for (key, size) in ["l", "w"].into_iter().zip([SlotSize::Length, SlotSize::Width]) {
                let now = sk.project.slot_size(si, curve, size);
                let v = field(sk.place, key);
                let changed = v.is_some_and(|v| Some(v) != now);
                if !changed && !asked(key) {
                    continue;
                }
                if let Some(v) = v.or(now) {
                    sk.project.set_slot_size(si, curve, size, v);
                }
                if let Some(ci) = sk.project.slot_size_dim(si, curve, size) {
                    laid.push(Laid { key, dim: Dim { si, ci } });
                }
            }
        }
        PlacingShape::Poly(cid) if wanted("r") => {
            if let Some(ci) = sk.project.ensure_diameter(si, cid, false) {
                laid.push(Laid { key: "r", dim: Dim { si, ci } });
            }
        }
        PlacingShape::Round(eid) if wanted("size") => {
            let s = &sk.project.sketches[si];
            let natural = if s.entity(eid).is_some_and(|e| e.kind.as_arc().is_some()) { Shown::Radius } else { Shown::Diameter };
            if let Some(centre) = s.entity(eid).and_then(|e| qymcad_core::model::circle_of(s, e)).map(|k| k.center) {
                if let Some(r) = radius_of(sk.place, "size", natural) {
                    set_round_size(sk, si, eid, r);
                }
                if let Some(ci) = sk.project.sketches[si].constraints.iter().position(|c| matches!(c, qymcad_core::model::Constraint::Diameter { c, .. } if *c == centre)) {
                    laid.push(Laid { key: "size", dim: Dim { si, ci } });
                }
            }
        }
        _ => {}
    }
    if !laid.is_empty() {
        qymcad_ui_state::invalidate(&mut *sk.regen);
    }
    for l in &laid {
        let p = fields.iter().find(|p| p.key == l.key).filter(|_| typed);
        let value = match sized {
            Sized::Followed => p.and_then(formula_of),
            Sized::ByValue => p.map(|p| Value { number: p.val, text: p.txt.trim().to_string() }),
        };
        ask_laid(sk, l.dim, AskOf { shown: asks.of(l.key).shown, row: row(l.key) }, value);
    }
}

/// DOES A SHAPE FOLLOW ITS FIELDS STEP BY STEP as they are typed - rebuilt from the corner or the centre it was drawn from,
/// its axes or its circle set - or is it laid anew over the document as the box found it: a rectangle opened again, a
/// slot, a circle and an arc, whose sizes are dimensions.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Follows {
    StepByStep,
    LaidAnew,
}

/// How shape `shape` follows its fields.
pub(crate) fn follows(shape: &qymcad_ui_state::PlacingShape) -> Follows {
    use qymcad_ui_state::PlacingShape;
    match shape {
        PlacingShape::Ellipse(..) | PlacingShape::Rect { .. } | PlacingShape::RectCenter { .. } | PlacingShape::Poly(_) => Follows::StepByStep,
        _ => Follows::LaidAnew,
    }
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
        if sk.project.rect_corners(si, eid).is_some() {
            sk.place.reopen(PlacingShape::RectSide(eid));
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
        // AN ELLIPSE OPENS ITS AXES AND ITS TURN, as a polygon opens its radius and turn: it opened nothing
        let ellipse = sk.project.sketches[si].entity(eid).and_then(|e| e.kind.as_ellipse()).map(|el| el.c);
        if let Some(c) = ellipse {
            if let Some(at) = qymcad_ui_state::sketch_pt(&*sk.project, si, c) {
                sk.place.reopen(PlacingShape::Ellipse(c, at));
                sk.place.focus = true;
                return true;
            }
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
            sk.place.reopen(PlacingShape::Round(eid));
            sk.place.focus = true;
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

/// A DOUBLE CLICK ON DIMENSION `ci` OF THE SKETCH CHOSEN OPENS ITS BOX: the radius of the circle a polygon hangs on opens
/// the box of the polygon - its radius and turn - as a double click on the polygon does; any other dimension its own box.
pub fn open_dim_box(sk: &mut qymcad_ui_state::SketchCtx, ci: usize) {
    let qymcad_ui_state::Sel::Sketch(si) = *sk.sel else { return };
    let poly = match sk.project.sketches[si].constraints.get(ci) {
        Some(qymcad_core::model::Constraint::Diameter { c, .. }) => {
            let c = *c;
            sk.project.sketches[si].constraints.iter().any(|x| matches!(x, qymcad_core::model::Constraint::PointOnCircle { c: cc, .. } if *cc == c)).then_some(c)
        }
        _ => None,
    };
    match poly {
        Some(c) => {
            sk.place.reopen(qymcad_ui_state::PlacingShape::Poly(c));
            sk.place.focus = true;
        }
        None => {
            *sk.inline = qymcad_ui_state::InlineEdit::Dim(ci);
            sk.dim.focus = true;
        }
    }
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
