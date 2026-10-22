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

/// The texts painted once the box has settled: two frames running with every text where it stood. Measured: the grid of
/// a box learns the widths of its columns from the frames before, and the box of a circle stood at x 457, 417, 398 and
/// 371 over its first four frames.
fn settled(ctx: &egui::Context, b: &mut qymcad_ui_state::Bench) -> Vec<Painted> {
    let mut painted: Vec<Painted> = Vec::new();
    let mut still = 0;
    for _ in 0..20 {
        let now = frame(ctx, b, Vec::new());
        let same = now.len() == painted.len() && now.iter().zip(&painted).all(|(a, b)| a.text == b.text && a.rect == b.rect);
        still = if same { still + 1 } else { 0 };
        painted = now;
        if still >= 2 {
            break;
        }
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

/// The box says what the box of a dimension says: the unit, the switch of the radius, the reference switch and the
/// driver.
#[test]
fn the_box_of_the_size_of_a_circle_says_what_the_box_of_its_dimension_says() {
    let Drawn { mut b, .. } = a_circle_in_its_box();
    let ctx = egui::Context::default();
    let painted = settled(&ctx, &mut b);
    let mut words = ["sk-mm", "sk-ref-short", "sk-driver-label"].map(qymcad_i18n::tr).to_vec();
    words.push("\u{d8}".to_string());
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

/// What a value typed into a box leaves behind, frame by frame: the size it reads on the geometry and the steps of undo.
struct Seen {
    size: f64,
    steps: usize,
}

/// Typing `text` into the box that holds the keyboard - its value selected, so the text takes its place.
fn type_in(ctx: &egui::Context, b: &mut qymcad_ui_state::Bench, text: &str) {
    frame(ctx, b, vec![egui::Event::Text(text.to_string())]);
    frame(ctx, b, Vec::new());
}

/// THE GEOMETRY GOES WITH THE VALUE TYPED, AND THE DOCUMENT ONLY WITH Enter: 40 typed is 40 on the geometry before Enter
/// with no step of undo; Esc puts it back as it was with no step; typed again and kept by Enter it is exactly one step.
fn goes_with_the_value(open: fn() -> Drawn, size: fn(&qymcad_ui_state::Bench, usize) -> f64, what: &str) {
    for keep in [egui::Key::Escape, egui::Key::Enter] {
        let Drawn { mut b, si } = open();
        let ctx = egui::Context::default();
        settled(&ctx, &mut b);
        let at_first = Seen { size: size(&b, si), steps: b.edits.undo.len() };
        type_in(&ctx, &mut b, "40");
        let typed = Seen { size: size(&b, si), steps: b.edits.undo.len() };
        assert!((typed.size - 40.0).abs() < 1e-6, "{what}: 40 typed is {} on the geometry before Enter", typed.size);
        assert_eq!(typed.steps, at_first.steps, "{what}: a value typed made a step of undo before Enter");
        key(&ctx, &mut b, keep);
        let after = Seen { size: size(&b, si), steps: b.edits.undo.len() };
        if keep == egui::Key::Escape {
            assert!((after.size - at_first.size).abs() < 1e-6, "{what}: Esc left the geometry at {}, not {}", after.size, at_first.size);
            assert_eq!(after.steps, at_first.steps, "{what}: Esc left a step of undo");
        } else {
            assert!((after.size - 40.0).abs() < 1e-6, "{what}: Enter left the geometry at {}, not 40", after.size);
            assert_eq!(after.steps, at_first.steps + 1, "{what}: Enter made {} steps of undo, not one", after.steps - at_first.steps);
        }
        assert!(b.edits.open.is_none(), "{what}: an edit stays open after {keep:?}");
    }
}

/// The width of the rectangle of the sketch, its first side.
fn width(b: &qymcad_ui_state::Bench, si: usize) -> f64 {
    let side = b.project.sketches[si].entities[0].id;
    b.project.rect_corners(si, side).map_or(f64::NAN, |[c0, c1, ..]| c0.dist(c1))
}

/// The diameter of the circle of the sketch.
fn diameter(b: &qymcad_ui_state::Bench, si: usize) -> f64 {
    b.project.sketches[si].entities[0].kind.as_circle().map_or(f64::NAN, |c| 2.0 * c.r)
}

/// A rectangle 40 x 30 opened again by a double click on its side, no dimension laid on it.
fn a_rectangle_reopened() -> Drawn {
    let Drawn { mut b, si } = a_sketch();
    b.project.add_rect_entity(si, 0.0, 0.0, 25.0, 30.0, qymcad_core::feature::Purpose::Real);
    b.project.regen_sketch(si);
    let side = b.project.sketches[si].entities[0].id;
    b.place.reopen(qymcad_ui_state::PlacingShape::RectSide(side));
    b.place.focus = true;
    Drawn { b, si }
}

/// A line 25 long with its length dimension open for editing.
fn a_length_in_its_box() -> Drawn {
    let Drawn { mut b, si } = a_sketch();
    b.project.add_line_entity(si, 0.0, 0.0, 25.0, 0.0, qymcad_core::feature::Purpose::Real);
    b.project.regen_sketch(si);
    let line = b.project.sketches[si].entities[0].kind.as_line().expect("the line");
    b.project.sketches[si].constraints.push(Constraint::Distance { a: line.a, b: line.b, d: 25.0, off: 5.0, expr: String::new(), driven: false, axis: 0, at: None });
    b.project.solve_sketch(si);
    b.inline = qymcad_ui_state::InlineEdit::Dim(b.project.sketches[si].constraints.len() - 1);
    b.dim.focus = true;
    Drawn { b, si }
}

/// The length of the line of the sketch.
fn length(b: &qymcad_ui_state::Bench, si: usize) -> f64 {
    let s = &b.project.sketches[si];
    let l = s.entities[0].kind.as_line().expect("the line");
    let at = |id| s.points.iter().find(|q| q.id == id).map(|q| qymcad_core::geom::Point2::new(q.x, q.y)).expect("a point");
    at(l.a).dist(at(l.b))
}

#[test]
fn a_rectangle_reopened_goes_with_its_width() {
    goes_with_the_value(a_rectangle_reopened, width, "the width of a rectangle reopened");
}

#[test]
fn a_circle_goes_with_its_diameter() {
    goes_with_the_value(a_circle_in_its_box, diameter, "the diameter of a circle");
}

#[test]
fn a_line_goes_with_its_length_dimension() {
    goes_with_the_value(a_length_in_its_box, length, "the length dimension of a line");
}
