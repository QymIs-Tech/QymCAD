//! A RECTANGLE DRAWN FROM ITS CENTRE KEEPS THE CENTRE WHILE THE WIDTH AND HEIGHT ARE TYPED.
//!
//! Reported behaviour: a "centre + corner" rectangle is drawn by a click in the centre and a click in
//! the corner, the little width-by-height window opens, "25" is typed as the width - and the centre
//! binding is gone, the point has travelled away.
//!
//! WHERE IT WENT WRONG. The width-by-height popup remembered two CORNERS: the mirrored opposite corner
//! and the corner clicked. Typing a new width rebuilt the rectangle FROM THE MIRRORED CORNER, so the
//! centre - the very point the rectangle was drawn from - travelled with every keystroke. The first
//! click (the centre) was never stored anywhere: the popup had nothing to hold on to.
//!
//! THE COMPANION. A rectangle drawn from two corners must keep behaving as before: the first corner
//! stays, the corner under the pointer follows the numbers. Without this the fix could be "always grow
//! about the centre", which would move corner-drawn rectangles people have already placed.

use qymcad_core::geom::Point2;
use qymcad_core::model::{Id, Project};
use qymcad_ui_state::Bench;

/// A sketch with a centre-drawn rectangle awaiting its width and height: the centre click at
/// (10, 20), the corner click at (30, 50), so the fields open on 40 by 60.
///
/// Built exactly the way the rectangle tool builds it (`add_rect_from_centre`): four sides held about the centre, and
/// the unfinished drawing kept as centre plus corner.
fn a_centre_drawn_rectangle() -> (Bench, usize, Point2) {
    let mut b = Bench::default();
    b.project.new_document();
    let si = b.project.new_sketch("S");
    let (center, corner) = (Point2::new(10.0, 20.0), Point2::new(30.0, 50.0));
    let ids = b.project.add_rect_from_centre(si, center, corner, qymcad_core::feature::Purpose::Real);
    b.project.regen_sketch(si);
    b.view = qymcad_ui_state::View2d { center: egui::Vec2::ZERO, scale: 4.0, initialized: true, fit: 4.0 };
    b.place.set(qymcad_ui_state::PlacingShape::RectCenter { center, corner, ids });
    b.place.focus = true;
    // the rectangle tool in hand on the sketch open for editing, as a person has it right after drawing
    b.armed = qymcad_ui_state::Armed::Draw(qymcad_ui_state::SketchShape::Rect);
    b.sel = qymcad_ui_state::Sel::Sketch(si);
    b.sketch_ses.editing = Some(b.project.sketches[si].id);
    (b, si, center)
}

/// The values the fields of the box of sizes hold: the width, the height.
fn held(b: &Bench) -> Vec<f64> {
    b.place.fields.iter().map(|p| p.val).collect()
}

/// The width-by-height popup, driven through real frames: what typing does.
fn frame(b: &mut Bench, ctx: &egui::Context, rect: egui::Rect, events: Vec<egui::Event>) {
    let raw = egui::RawInput { screen_rect: Some(rect), events, ..Default::default() };
    let _ = ctx.run_ui(raw, |ui| qymcad_ui_state::tool_in_hand(ui, rect, qymcad_sketch::in_hand(&mut b.sketch_ctx())));
}

/// The corners of the rectangle as it actually stands, over the rectangle's own points rather than
/// the sketch's system ones (the origin and the axes stand at zero and would win every minimum).
struct RectBounds {
    min_x: f64,
    max_x: f64,
    min_y: f64,
    max_y: f64,
}

impl RectBounds {
    fn width(&self) -> f64 {
        self.max_x - self.min_x
    }

    fn height(&self) -> f64 {
        self.max_y - self.min_y
    }

    fn centre(&self) -> Point2 {
        Point2::new((self.min_x + self.max_x) / 2.0, (self.min_y + self.max_y) / 2.0)
    }
}

fn rect_corners(p: &Project, si: usize) -> RectBounds {
    let sys: std::collections::HashSet<Id> = p.sketches[si].system_ids().into_iter().collect();
    let mut xs: Vec<f64> = Vec::new();
    let mut ys: Vec<f64> = Vec::new();
    for q in &p.sketches[si].points {
        if sys.contains(&q.id) {
            continue;
        }
        xs.push(q.x);
        ys.push(q.y);
    }
    assert!(!xs.is_empty(), "GUARD: the rectangle left no points of its own behind");
    RectBounds {
        min_x: xs.iter().cloned().fold(f64::INFINITY, f64::min),
        max_x: xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        min_y: ys.iter().cloned().fold(f64::INFINITY, f64::min),
        max_y: ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
    }
}

/// TYPING THE WIDTH KEEPS THE CENTRE WHERE THE FIRST CLICK PUT IT.
///
/// Driven through the popup in real frames, because the whole complaint is about what happens WHILE
/// typing: a check that rebuilds the rectangle by hand would never see the anchor the popup uses.
#[test]
fn typing_the_width_keeps_the_centre() {
    let (mut b, si, center) = a_centre_drawn_rectangle();
    let ctx = egui::Context::default();
    let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0));

    // A COUPLE OF FRAMES: an `Area` places itself a frame late, and the field asks for the focus
    // until it gets it.
    for _ in 0..3 {
        frame(&mut b, &ctx, rect, Vec::new());
    }
    assert_eq!(held(&b), [40.0, 60.0], "GUARD: the fields must open on the full 40 by 60, or what follows is about nothing");
    assert!(ctx.memory(|m| m.focused()).is_some(), "GUARD: the width field must hold the keyboard, or nothing below is really typed");

    // TYPED FOR REAL. On auto-focus the whole text is selected, so the keystroke replaces it - which
    // is exactly what a person gets.
    frame(&mut b, &ctx, rect, vec![egui::Event::Text("25".into())]);
    for _ in 0..3 {
        frame(&mut b, &ctx, rect, Vec::new());
    }
    assert_eq!(held(&b)[0], 25.0, "GUARD: \"25\" must really be in the width field: {:?}", held(&b));

    let bounds = rect_corners(&b.project, si);
    assert!((bounds.width() - 25.0).abs() < 1e-6, "the width is {:.3} instead of 25", bounds.width());
    assert!((bounds.height() - 60.0).abs() < 1e-6, "the height is {:.3} instead of 60", bounds.height());
    let mid = bounds.centre();
    assert!(
        (mid.x - center.x).abs() < 1e-6 && (mid.y - center.y).abs() < 1e-6,
        "the centre travelled to ({:.3}, {:.3}) instead of staying at ({:.3}, {:.3}) - the popup rebuilt the rectangle from the corner",
        mid.x,
        mid.y,
        center.x,
        center.y
    );
    // AND THE UNFINISHED DRAWING STILL KNOWS ITS CENTRE: the next keystroke must grow about the
    // same point rather than about a corner of the rebuilt rectangle.
    match b.place.rect_center() {
        Some((kept, _, _)) => assert!((kept.x - center.x).abs() < 1e-9 && (kept.y - center.y).abs() < 1e-9, "the popup kept ({:.3}, {:.3}) instead of the centre", kept.x, kept.y),
        None => panic!("the unfinished rectangle stopped being a centre-drawn one after the rebuild"),
    }
}

/// A CORNER-DRAWN RECTANGLE STILL GROWS FROM ITS FIRST CORNER.
///
/// The same popup, the other anchor: typing the width must not drag the corner the rectangle was
/// drawn from.
#[test]
fn typing_the_width_keeps_the_first_corner() {
    let mut b = Bench::default();
    b.project.new_document();
    let si = b.project.new_sketch("S");
    let (a, corner) = (Point2::new(10.0, 20.0), Point2::new(50.0, 80.0));
    let ids = b.project.add_rect_entity(si, a.x, a.y, corner.x, corner.y, qymcad_core::feature::Purpose::Real);
    b.project.regen_sketch(si);
    b.view = qymcad_ui_state::View2d { center: egui::Vec2::ZERO, scale: 4.0, initialized: true, fit: 4.0 };
    b.place.set(qymcad_ui_state::PlacingShape::Rect { a, b: corner, ids });
    b.place.focus = true;
    // the rectangle tool in hand on the sketch open for editing, as a person has it right after drawing
    b.armed = qymcad_ui_state::Armed::Draw(qymcad_ui_state::SketchShape::Rect);
    b.sel = qymcad_ui_state::Sel::Sketch(si);
    b.sketch_ses.editing = Some(b.project.sketches[si].id);
    let ctx = egui::Context::default();
    let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0));

    for _ in 0..3 {
        frame(&mut b, &ctx, rect, Vec::new());
    }
    assert_eq!(held(&b), [40.0, 60.0], "GUARD: the fields must open on 40 by 60: {:?}", held(&b));

    frame(&mut b, &ctx, rect, vec![egui::Event::Text("25".into())]);
    for _ in 0..3 {
        frame(&mut b, &ctx, rect, Vec::new());
    }

    let bounds = rect_corners(&b.project, si);
    assert!(
        (bounds.min_x - a.x).abs() < 1e-6 && (bounds.min_y - a.y).abs() < 1e-6,
        "the first corner travelled to ({:.3}, {:.3}) instead of staying at ({:.3}, {:.3})",
        bounds.min_x,
        bounds.min_y,
        a.x,
        a.y
    );
    assert!((bounds.width() - 25.0).abs() < 1e-6 && (bounds.height() - 60.0).abs() < 1e-6, "the rectangle is {:.3} by {:.3} instead of 25 by 60", bounds.width(), bounds.height());
}
