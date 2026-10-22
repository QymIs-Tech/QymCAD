//! THE BOX OF THE SIZES OF A SHAPE IS THE BOX OF ITS DIMENSIONS: a row of a dimension for each size, as the box of the
//! dimension once laid says it.
//!
//! Reported behaviour: the box of a dimension of a circle said the unit, radius or diameter, a reference switch and a
//! driver name, and the box opened on a circle with no dimension yet said only "Diameter" and a number - two boxes of
//! one size, one with its switches and one without.

use qymcad_core::model::Constraint;

/// A shape drawn in a sketch, with the bench it is edited on.
struct Drawn {
    b: qymcad_ui_state::Bench,
    si: usize,
}

/// A sketch being edited, empty.
fn a_sketch() -> Drawn {
    let mut b = qymcad_ui_state::Bench::default();
    b.project.new_document();
    let si = b.project.new_sketch("S");
    b.sketch_ses.editing = Some(b.project.sketches[si].id);
    b.sel = qymcad_ui_state::Sel::Sketch(si);
    b.view = qymcad_ui_state::View2d { center: egui::Vec2::ZERO, scale: 4.0, initialized: true, fit: 4.0 };
    Drawn { b, si }
}

/// A circle of radius 50 drawn in a sketch being edited, its box of size open.
fn a_circle_in_its_box() -> Drawn {
    let Drawn { mut b, si } = a_sketch();
    b.project.add_circle_entity(si, 0.0, 0.0, 50.0, qymcad_core::feature::Purpose::Real);
    b.project.regen_sketch(si);
    let eid = b.project.sketches[si].entities[0].id;
    b.place.reopen(qymcad_ui_state::PlacingShape::Round(eid));
    b.place.focus = true;
    Drawn { b, si }
}

/// A rectangle 40 x 30 drawn in a sketch being edited, its box of sizes open.
fn a_rectangle_in_its_box() -> Drawn {
    let Drawn { mut b, si } = a_sketch();
    let a = qymcad_core::geom::Point2::new(0.0, 0.0);
    let corner = qymcad_core::geom::Point2::new(40.0, 30.0);
    let ids = b.project.add_rect_entity(si, a.x, a.y, corner.x, corner.y, qymcad_core::feature::Purpose::Real);
    b.project.regen_sketch(si);
    b.place.reopen(qymcad_ui_state::PlacingShape::Rect { a, b: corner, ids });
    b.place.focus = true;
    Drawn { b, si }
}

/// One painted text and where it stands.
struct Painted {
    text: String,
    rect: egui::Rect,
}

/// One frame of the tool in hand with `events`, and the texts it painted.
fn frame(ctx: &egui::Context, b: &mut qymcad_ui_state::Bench, events: Vec<egui::Event>) -> Vec<Painted> {
    fn walk(s: &egui::epaint::Shape, out: &mut Vec<Painted>) {
        match s {
            egui::epaint::Shape::Text(t) => out.push(Painted { text: t.galley.text().to_string(), rect: egui::Rect::from_min_size(t.pos, t.galley.size()) }),
            egui::epaint::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0));
    let raw = egui::RawInput { screen_rect: Some(rect), events, ..Default::default() };
    let out = ctx.run_ui(raw, |ui| qymcad_ui_state::tool_in_hand(ui, rect, qymcad_sketch::in_hand(&mut b.sketch_ctx())));
    let mut texts = Vec::new();
    for cs in &out.shapes {
        walk(&cs.shape, &mut texts);
    }
    texts
}

/// The texts painted once the box has settled.
fn settled(ctx: &egui::Context, b: &mut qymcad_ui_state::Bench) -> Vec<Painted> {
    let mut painted = Vec::new();
    for _ in 0..3 {
        painted = frame(ctx, b, Vec::new());
    }
    painted
}

/// A click on the text `word` the box painted.
fn click(ctx: &egui::Context, b: &mut qymcad_ui_state::Bench, painted: &[Painted], word: &str) {
    let at = painted.iter().find(|p| p.text == word).map(|p| p.rect.center()).unwrap_or_else(|| panic!("no {word:?} in the box"));
    let button = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    frame(ctx, b, vec![egui::Event::PointerMoved(at)]);
    frame(ctx, b, vec![button(true)]);
    frame(ctx, b, vec![button(false)]);
}

/// A key pressed and let go.
fn key(ctx: &egui::Context, b: &mut qymcad_ui_state::Bench, k: egui::Key) {
    let ev = |pressed| egui::Event::Key { key: k, physical_key: None, pressed, repeat: false, modifiers: Default::default() };
    frame(ctx, b, vec![ev(true)]);
    frame(ctx, b, vec![ev(false)]);
}

/// How many times `word` is painted.
fn times(painted: &[Painted], word: &str) -> usize {
    painted.iter().filter(|p| p.text == word).count()
}

/// The box says what the box of a dimension says: the unit, the switch of the radius, the reference switch, the driver
/// and how Enter takes it.
#[test]
fn the_box_of_the_size_of_a_circle_says_what_the_box_of_its_dimension_says() {
    let Drawn { mut b, .. } = a_circle_in_its_box();
    let ctx = egui::Context::default();
    let painted = settled(&ctx, &mut b);
    let mut words = ["sk-mm", "sk-ref-short", "sk-driver-label", "sk-enter-apply-expr"].map(qymcad_i18n::tr).to_vec();
    words.push("->R".to_string());
    let missing: Vec<&String> = words.iter().filter(|w| times(&painted, w) == 0).collect();
    assert!(missing.is_empty(), "the box of the size of a circle does not say {missing:?}; it says {:?}", painted.iter().map(|p| &p.text).collect::<Vec<_>>());
}

/// THE REFERENCE SWITCH, clicked on a circle with no dimension, lays nothing yet; Enter lays its dimension as a reference
/// one, and the box goes.
#[test]
fn enter_lays_the_dimension_as_a_reference_one_when_asked() {
    let Drawn { mut b, si } = a_circle_in_its_box();
    let ctx = egui::Context::default();
    let painted = settled(&ctx, &mut b);
    click(&ctx, &mut b, &painted, &qymcad_i18n::tr("sk-ref-short"));
    let laid = |b: &qymcad_ui_state::Bench| b.project.sketches[si].constraints.iter().filter(|c| matches!(c, Constraint::Diameter { .. })).cloned().collect::<Vec<Constraint>>();
    assert!(laid(&b).is_empty(), "the switch laid a dimension before Enter: {:?}", laid(&b));
    key(&ctx, &mut b, egui::Key::Enter);
    assert!(matches!(laid(&b).as_slice(), [Constraint::Diameter { driven: true, .. }]), "Enter did not lay one reference dimension of the circle: {:?}", laid(&b));
    assert!(!b.place.active(), "the box stays after Enter");
}

/// A RECTANGLE SAYS EACH OF ITS SIZES AS A DIMENSION: a reference switch and a driver for its width and for its height.
#[test]
fn the_box_of_a_rectangle_has_the_rows_of_a_dimension_for_each_size() {
    let Drawn { mut b, .. } = a_rectangle_in_its_box();
    let ctx = egui::Context::default();
    let painted = settled(&ctx, &mut b);
    for w in ["sk-ref-short", "sk-driver-label"].map(qymcad_i18n::tr) {
        assert_eq!(times(&painted, &w), 2, "the box of a rectangle says {w:?} not once a size; it says {:?}", painted.iter().map(|p| &p.text).collect::<Vec<_>>());
    }
}

/// Esc LEAVES THE RECTANGLE WITHOUT THE SIZES ASKED: the reference switched on, Esc lays nothing.
#[test]
fn esc_leaves_the_rectangle_without_its_dimensions() {
    let Drawn { mut b, si } = a_rectangle_in_its_box();
    let ctx = egui::Context::default();
    let painted = settled(&ctx, &mut b);
    click(&ctx, &mut b, &painted, &qymcad_i18n::tr("sk-ref-short"));
    key(&ctx, &mut b, egui::Key::Escape);
    key(&ctx, &mut b, egui::Key::Escape);
    let dims = b.project.sketches[si].constraints.iter().filter(|c| matches!(c, Constraint::Distance { .. })).count();
    assert_eq!(dims, 0, "Esc laid {dims} dimensions on the rectangle");
    assert!(!b.place.active(), "the box stays after Esc");
}
