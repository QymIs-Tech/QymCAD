//! WHERE THE BOX OF A TOOL STANDS - the one place for every box at the geometry of a part and of an assembly. A tool
//! says only what its box stands by, in the world (`By`); here it is found on the screen, and the box is laid beside it
//! by one rule, clear of what has to stay in sight: what the box sizes, every handle and gizmo drawn in the frame
//! (`keep_in_sight`) and, where there is room, the model itself (`keep_model_in_sight`). It goes with them as the view
//! moves. Moved by a person, it stands where it was put on the screen until its tool is put down.
//!
//! Each box laid itself by a rule of its own before: a datum's beside its handles, a feature's 10 px off a point, a
//! section's and a gizmo's 12 px off theirs, a pattern of parts' left of the source, a joint's by the middle of its
//! anchors, each pressed back against the edge of the view. A box by a mate near the right edge was pressed over it, the
//! rings of a datum's gizmo stood under its box, and nothing could be moved out of the way.

use super::*;

/// WHAT A BOX STANDS BY, in the world: what its tool sizes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum By {
    /// A point: a datum point, the origin of a gizmo or of a section.
    Point([f64; 3]),
    /// An arrow from its base to its tip: the box stands by the tip, clear of the whole arrow.
    Arrow { base: [f64; 3], tip: [f64; 3] },
    /// A body the tool works on, as it is drawn in the context worked in: the box stands clear of all of it.
    Body(Id),
    /// A part - a component - with every body of its own.
    Part(Id),
    /// Two points sized together - the anchors of a mate: the box stands by their middle, clear of both.
    Pair { a: [f64; 3], b: [f64; 3] },
    /// Nothing picked yet: the top left corner of the view, clear of everything to be picked.
    ViewCorner,
}

/// WHAT A BOX STANDS BY, whether it may change sides, and whether it stands off the model.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    pub by: By,
    pub sides: Sides,
    pub off: Off,
}

/// WHAT A BOX STANDS OFF besides what it sizes and the handles and gizmos.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Off {
    /// The model as well, where the view has room.
    Model,
    /// Nothing more: a field at a point of its own - a radius at one corner of six - stands by its corner, over the model
    /// if it must; laid off the model it would stand by no corner at all.
    Nothing,
}

/// MAY A BOX CHANGE THE SIDE IT STANDS ON.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sides {
    /// By the rule, every frame.
    Free,
    /// It keeps the side it stood on and stands by its point, clear of nothing else: the box of a mate while the mate
    /// runs, whose gizmo sweeps past it - a box changing sides then carried the button that stops the run from under the
    /// pointer.
    Held,
}

impl Place {
    /// By `by`, by the rule every frame.
    pub fn by(by: By) -> Place {
        Place { by, sides: Sides::Free, off: Off::Model }
    }

    /// By `by` and nothing more: over the model if it must.
    pub fn close_by(by: By) -> Place {
        Place { by, sides: Sides::Free, off: Off::Nothing }
    }

    /// The same place, the side of the box held.
    pub fn held(self) -> Place {
        Place { sides: Sides::Held, ..self }
    }
}

/// WHAT A BOX STANDS BY, ON THE SCREEN: the point it stands by, and the place of what it sizes - `Rect::NOTHING` where the
/// point is all there is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spot {
    pub at: egui::Pos2,
    pub subject: Rect,
}

/// WHERE BODY `b` STANDS ON THE SCREEN of `project`: the box of its projected bounds through its placement in context `here` (an
/// imported part stands where it was put). None when it has no mesh.
pub fn body_on_screen(b: Id, project: &Project, screen: &Screen, here: Id) -> Option<Rect> {
    let mi = project.mesh_index(b)?;
    let bb = project.bodies[mi].mesh.bounds()?;
    let wt = project.body_display_transform(b, here);
    let mut body = Rect::NOTHING;
    for x in [bb.min.x, bb.max.x] {
        for y in [bb.min.y, bb.max.y] {
            for z in [bb.min.z, bb.max.z] {
                body.extend_with(screen.at(qymcad_core::feature::apply12(&wt, [x, y, z])).0);
            }
        }
    }
    body.is_positive().then_some(body)
}

/// WHERE `by` STANDS ON THE SCREEN in view `screen` of `project`, in context `here`. A body or a part with no mesh
/// stands at the corner of the view.
pub fn spot_of(by: By, project: &Project, screen: &Screen, here: Id) -> Spot {
    let at = |p: [f64; 3]| screen.at(p).0;
    let corner = Spot { at: screen.rect.min + egui::vec2(16.0, 48.0), subject: Rect::NOTHING };
    // a body is sized by its top right corner: the box reads as the body's, beside it
    let beside = |r: Rect| Spot { at: r.right_top(), subject: r };
    match by {
        By::Point(p) => Spot { at: at(p), subject: Rect::NOTHING },
        By::Arrow { base, tip } => Spot { at: at(tip), subject: Rect::from_two_pos(at(base), at(tip)).expand(8.0) },
        By::Pair { a, b } => {
            let (a, b) = (at(a), at(b));
            Spot { at: egui::pos2((a.x + b.x) / 2.0, (a.y + b.y) / 2.0), subject: Rect::from_two_pos(a, b).expand(12.0) }
        }
        By::Body(b) => body_on_screen(b, project, screen, here).map_or(corner, beside),
        By::Part(c) => {
            let bodies = project.bodies.iter().filter(|b| project.body_owner(b.id) == Some(c));
            let all = bodies.filter_map(|b| body_on_screen(b.id, project, screen, here)).fold(Rect::NOTHING, |all, r| all.union(r));
            if all.is_positive() {
                beside(all)
            } else {
                corner
            }
        }
        By::ViewCorner => corner,
    }
}

/// THE SIDE OF WHAT IT SIZES A BOX STANDS ON. The side it stood on the frame before is asked first, so a box does not
/// jump from side to side while the view moves and both fit.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    Right,
    Left,
    Below,
    Above,
}

/// How far a box stands from what it keeps clear of, on the screen.
pub const CLEARANCE: f32 = 16.0;

/// The size a box is laid by before it was ever drawn: a caption, a field or four, and the two buttons.
const FIRST_SIZE: egui::Vec2 = egui::vec2(240.0, 140.0);

/// WHAT WAS KEPT IN SIGHT IN A FRAME: the places handles and gizmos took on the screen, and where the model stood.
#[derive(Clone, Default)]
struct Seen {
    gizmos: Vec<Rect>,
    model: Option<Rect>,
}

/// WHAT IS KEPT IN SIGHT, as the frame `frame` drew it.
#[derive(Clone, Default)]
struct InSight {
    frame: u64,
    now: Seen,
}

fn in_sight_id() -> egui::Id {
    egui::Id::new("kept in sight by the boxes of the tools")
}

/// What is kept in sight in this frame, `change` done to it: what an earlier frame kept goes.
fn keep(ctx: &egui::Context, change: impl FnOnce(&mut Seen)) {
    let frame = ctx.cumulative_frame_nr();
    ctx.data_mut(|d| {
        let s = d.get_temp_mut_or_default::<InSight>(in_sight_id());
        if s.frame != frame {
            *s = InSight { frame, now: Seen::default() };
        }
        change(&mut s.now);
    });
}

/// A HANDLE OR A GIZMO TOOK `rect` ON THE SCREEN in this frame: no box of a tool covers it.
pub fn keep_in_sight(ctx: &egui::Context, rect: Rect) {
    if rect.is_positive() {
        keep(ctx, |s| s.gizmos.push(rect));
    }
}

/// THE MODEL DRAWN IN THIS FRAME took `rect` on the screen: a box stands off it where the view has room.
pub fn keep_model_in_sight(ctx: &egui::Context, rect: Rect) {
    if rect.is_positive() {
        keep(ctx, |s| s.model = Some(rect));
    }
}

/// WHERE THE MODEL OF `pn` STANDS ON THE SCREEN `rect`: the bodies drawn in the context worked in, ghosts left out.
pub fn model_on_screen(pn: &Painting, rect: Rect) -> Option<Rect> {
    let basis = pn.cam.basis();
    let screen = Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis };
    let mut all = Rect::NOTHING;
    for SceneMesh { index, ghost, .. } in visible_mesh_items(pn) {
        if ghost {
            continue;
        }
        let Some(b) = mesh_world_bounds(pn, index) else { continue };
        for x in [b.min[0], b.max[0]] {
            for y in [b.min[1], b.max[1]] {
                for z in [b.min[2], b.max[2]] {
                    all.extend_with(screen.at([x, y, z]).0);
                }
            }
        }
    }
    all.is_positive().then_some(all)
}

/// THE PLACES OF THE HANDLES AND GIZMOS KEPT IN SIGHT NOW: what they took this frame, or the one before when none drew yet.
pub fn in_sight(ctx: &egui::Context) -> Vec<Rect> {
    let s = seen(ctx);
    s.gizmos
}

/// What is kept in sight now: what this frame drew, or - nothing drawn in it yet - what the frame before drew. Not both:
/// the window draws the gizmos of a frame before its boxes, and the handles of a pick let go, kept from the frame before,
/// turned the box of a datum plane to a side it then held.
fn seen(ctx: &egui::Context) -> Seen {
    let frame = ctx.cumulative_frame_nr();
    let s = ctx.data(|d| d.get_temp::<InSight>(in_sight_id())).unwrap_or_default();
    if frame.saturating_sub(s.frame) <= 1 {
        s.now
    } else {
        Seen::default()
    }
}

/// WHERE A BOX STOOD, kept between frames: the frame it was drawn last, what it stood by, the side it stood on, and where
/// a person put it.
#[derive(Clone, Copy)]
struct Stood {
    frame: u64,
    by: By,
    side: Side,
    moved: Option<egui::Pos2>,
}

/// The size box `id` was drawn at the frame before, or the size a box is first laid by.
fn size_of(ctx: &egui::Context, id: egui::Id) -> egui::Vec2 {
    ctx.memory(|m| m.area_rect(id)).map_or(FIRST_SIZE, |r| r.size())
}

/// THE CORNER OF A BOX `id` STANDING BY `place` IN THIS FRAME: where a person put it, if they did since its tool was
/// taken; otherwise by the rule (`place_by_the_rule`) in the view `look` draws.
pub fn box_corner(ctx: &egui::Context, id: egui::Id, place: Place, look: &BoxLook) -> egui::Pos2 {
    let frame = ctx.cumulative_frame_nr();
    let stood_id = id.with("where the box stood");
    // a box not drawn the frame before is a box opened anew: where it was put before goes with the tool put down
    let stood = ctx.data(|d| d.get_temp::<Stood>(stood_id)).filter(|s| s.frame + 1 >= frame);
    if let Some(at) = stood.and_then(|s| s.moved) {
        ctx.data_mut(|d| d.insert_temp(stood_id, Stood { frame, by: place.by, side: stood.map_or(Side::Right, |s| s.side), moved: Some(at) }));
        return at;
    }
    // the side of the frame before is asked first only by the same kind of thing: a pick let go puts the box back where
    // it stood before the pick, not on the side it took by the arrow of the pick
    let was = stood.filter(|s| std::mem::discriminant(&s.by) == std::mem::discriminant(&place.by)).map(|s| s.side);
    let seen = seen(ctx);
    let laid = place_by_the_rule(&Laying {
        spot: spot_of(place.by, look.project, &look.screen, look.here),
        sides: place.sides,
        size: size_of(ctx, id),
        view: look.screen.rect,
        kept: &seen.gizmos,
        model: seen.model.filter(|_| place.off == Off::Model),
        was,
    });
    ctx.data_mut(|d| d.insert_temp(stood_id, Stood { frame, by: place.by, side: laid.side, moved: None }));
    laid.at
}

/// BOX `id` WAS DRAGGED BY A PERSON to `at`: there it stands until its tool is put down.
pub fn box_moved(ctx: &egui::Context, id: egui::Id, at: egui::Pos2) {
    let frame = ctx.cumulative_frame_nr();
    let stood_id = id.with("where the box stood");
    if let Some(s) = ctx.data(|d| d.get_temp::<Stood>(stood_id)) {
        ctx.data_mut(|d| d.insert_temp(stood_id, Stood { frame, moved: Some(at), ..s }));
    }
}

/// WHERE THE RULE LAID A BOX: its corner and the side it stands on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Laid {
    pub at: egui::Pos2,
    pub side: Side,
}

/// WHAT LAYING A BOX BY THE RULE TAKES: what it stands by on the screen and whether it may change sides, its size, the
/// view, the handles and gizmos kept in sight, where the model stands, and the side it stood on the frame before.
pub struct Laying<'a> {
    pub spot: Spot,
    pub sides: Sides,
    pub size: egui::Vec2,
    pub view: Rect,
    pub kept: &'a [Rect],
    pub model: Option<Rect>,
    pub was: Option<Side>,
}

/// How much more covering what the box sizes or a gizmo weighs than covering the model, where no side is clear.
const NEAR_WEIGHT: f32 = 1000.0;

/// THE RULE: the box stands beside all it keeps clear of - the point it stands by, what it sizes, the handles and gizmos -
/// right of them, left, under, over, the side of the frame before asked first: the first side where the whole box
/// stands in the view and covers none of them nor the model. Where none is, the same sides beside the model as well.
/// Where no side is clear at all, the one covering the least, what it sizes and the gizmos counted far above the model,
/// pulled into the view.
pub fn place_by_the_rule(l: &Laying) -> Laid {
    let view = Rect::from_min_max(l.view.min + egui::vec2(4.0, 32.0), l.view.max - egui::vec2(4.0, 4.0));
    let by = l.spot.at;
    let point = Rect::from_center_size(by, egui::vec2(12.0, 12.0));
    let held = l.was.filter(|_| l.sides == Sides::Held);
    // a box that holds its side stands by its point and what it sizes only: what sweeps past it does not push it about
    let mut near: Vec<Rect> = if held.is_some() { Vec::new() } else { l.kept.iter().copied().filter(|r| r.is_positive()).collect() };
    if l.spot.subject.is_positive() {
        near.push(l.spot.subject);
    }
    near.push(point);
    let model = l.model.filter(|m| m.is_positive() && held.is_none());
    let around = |all: Rect, side: Side| {
        let all = all.expand(CLEARANCE);
        let c = match side {
            Side::Right => egui::pos2(all.right(), by.y - 12.0),
            Side::Left => egui::pos2(all.left() - l.size.x, by.y - 12.0),
            Side::Below => egui::pos2(by.x - l.size.x / 2.0, all.bottom()),
            Side::Above => egui::pos2(by.x - l.size.x / 2.0, all.top() - l.size.y),
        };
        // pulled into the view along the side it stands on, not across: a box right of a mate stays right of it
        let (top, left) = ((view.bottom() - l.size.y).max(view.top()), (view.right() - l.size.x).max(view.left()));
        let at = match side {
            Side::Right | Side::Left => egui::pos2(c.x, c.y.clamp(view.top(), top)),
            Side::Below | Side::Above => egui::pos2(c.x.clamp(view.left(), left), c.y),
        };
        Laid { at, side }
    };
    let near_all = near.iter().fold(point, |u, r| u.union(*r));
    if let Some(side) = held {
        return around(near_all, side);
    }
    let overlap = |rects: &mut dyn Iterator<Item = Rect>, at: egui::Pos2| {
        let the_box = Rect::from_min_size(at, l.size);
        rects.map(|r| r.expand(4.0).intersect(the_box)).filter(|r| r.is_positive()).map(|r| r.area()).sum::<f32>()
    };
    let cost = |at: egui::Pos2| NEAR_WEIGHT * overlap(&mut near.iter().copied(), at) + overlap(&mut model.into_iter(), at);
    let sides: Vec<Side> = l.was.into_iter().chain([Side::Right, Side::Left, Side::Below, Side::Above]).collect();
    let mut tried: Vec<Laid> = sides.iter().map(|s| around(near_all, *s)).collect();
    if let Some(m) = model {
        tried.extend(sides.iter().map(|s| around(near_all.union(m), *s)));
    }
    let fits = |l2: &Laid| view.contains_rect(Rect::from_min_size(l2.at, l.size));
    if let Some(clear) = tried.iter().find(|l2| fits(l2) && cost(l2.at) == 0.0) {
        return *clear;
    }
    let inside = |l2: Laid| Laid {
        at: egui::pos2(l2.at.x.clamp(view.left(), (view.right() - l.size.x).max(view.left())), l2.at.y.clamp(view.top(), (view.bottom() - l.size.y).max(view.top()))),
        side: l2.side,
    };
    tried.into_iter().map(inside).min_by(|a, b| cost(a.at).total_cmp(&cost(b.at))).unwrap_or_else(|| inside(around(near_all, Side::Right)))
}

/// WHERE A BOX OF `size` STANDS BY `spot` IN `view` BY THE RULE ALONE, with nothing kept in sight but what it sizes: for a
/// check of where a tool's box goes.
pub fn laid_by_the_rule(spot: Spot, size: egui::Vec2, view: Rect) -> Rect {
    let laid = place_by_the_rule(&Laying { spot, sides: Sides::Free, size, view, kept: &[], model: None, was: None });
    Rect::from_min_size(laid.at, size)
}
