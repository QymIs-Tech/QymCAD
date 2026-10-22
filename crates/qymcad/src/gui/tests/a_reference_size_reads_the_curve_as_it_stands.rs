//! A REFERENCE SIZE READS THE CURVE AS IT STANDS, on the path of a person: an arc whose radius and sweep other
//! constraints already hold - its centre fixed, its ends level and upright of it, the distance from the centre to an end
//! a dimension of 20 - takes a radius dimension, and the dimension comes as a reference by itself; so does its length. The
//! dimension of 20 changed by a double click to 26: the reference radius says 26 and the reference length 26 x pi / 2. A
//! circle held so by a line from its centre to its rim does the same.
//!
//! Reported behaviour (#167): a reference radius on an arc kept its old value after the arc's radius changed; on a circle
//! the same dimension followed.
use crate::gui::hand::Hand;
use crate::gui::App;
use qymcad_core::geom::Point2;
use qymcad_core::model::Constraint;

/// A sketch open for editing in a new window.
fn sketch() -> App {
    let mut app = App::default();
    let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    app.enter_sketch_edit(si);
    app
}

/// Points `a` then `b` picked, constraint `code` pressed, and the pick let go by a click on nothing.
fn tie(hand: &mut Hand, points: [Point2; 2], code: qymcad_ui_state::ConstraintKind) {
    let [a, b] = points;
    hand.sk_select().shift_click2d(a.x, a.y).shift_click2d(b.x, b.y);
    hand.constraint(code);
    hand.key(egui::Key::Escape);
    hand.sk_select().click2d(90.0, 90.0);
}

/// The distance from point `a` to point `b` dimensioned by hand and set to 20.
fn twenty(hand: &mut Hand, points: [Point2; 2], place: Point2) {
    let [a, b] = points;
    hand.sk_select();
    assert!(hand.press_hint(&crate::i18n::tr("tb-dim-hint")), "the dimension tool");
    hand.close_window();
    hand.click2d(a.x, a.y).click2d(b.x, b.y).click2d(place.x, place.y);
    hand.frame(Vec::new());
    hand.chord(egui::Modifiers::COMMAND, egui::Key::A).type_text("20").key(egui::Key::Enter);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
}

/// The size of a curve dimensioned by hand by a click on it and one beside it.
fn size_of(hand: &mut Hand, clicks: [Point2; 2]) {
    let [at, place] = clicks;
    hand.sk_select();
    assert!(hand.press_hint(&crate::i18n::tr("tb-dim-hint")), "the dimension tool");
    hand.close_window();
    hand.click2d(at.x, at.y).click2d(place.x, place.y);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
}

/// The dimension reading 20 at `place` opened by a double click on its value and set to 26.
fn twenty_six(hand: &mut Hand, place: Point2) -> bool {
    hand.sk_select();
    let near = hand.on_screen2d((place.x, place.y));
    let Some(value) = hand.written_near("20.0", near) else { return false };
    hand.double_click_screen(value.center());
    hand.frame(Vec::new());
    hand.chord(egui::Modifiers::COMMAND, egui::Key::A).type_text("26").key(egui::Key::Enter);
    hand.key(egui::Key::Escape);
    true
}

/// The reference radius (or half the reference diameter) and the reference arc length of the sketch, by kind.
struct References {
    radius: Option<f64>,
    length: Option<f64>,
}

fn references(app: &App) -> References {
    let s = &app.project.sketches[0];
    References {
        radius: s.constraints.iter().find_map(|c| match c {
            Constraint::Diameter { d, driven: true, diam, .. } => Some(if *diam { d / 2.0 } else { *d }),
            _ => None,
        }),
        length: s.constraints.iter().find_map(|c| match c {
            Constraint::ArcLength { len, driven: true, .. } => Some(*len),
            _ => None,
        }),
    }
}

#[test]
fn a_reference_radius_and_length_of_an_arc_read_it_after_its_size_changed() {
    let mut app = sketch();
    let mut hand = Hand::new(&mut app);
    // an arc about (30, 30) from (50, 30) round to (30, 50); its centre fixed, its ends level and upright of it, the
    // first 20 from it
    hand.sk_tool(qymcad_ui_state::SketchShape::Arc).click2d(30.0, 30.0).click2d(50.0, 30.0).click2d(30.0, 50.0).key(egui::Key::Escape).key(egui::Key::Escape);
    tie(&mut hand, [Point2::new(30.0, 30.0), Point2::new(50.0, 30.0)], qymcad_ui_state::ConstraintKind::Horizontal);
    tie(&mut hand, [Point2::new(30.0, 30.0), Point2::new(30.0, 50.0)], qymcad_ui_state::ConstraintKind::Vertical);
    hand.sk_select().click2d(30.0, 30.0);
    hand.constraint(qymcad_ui_state::ConstraintKind::Fix);
    hand.key(egui::Key::Escape);
    hand.sk_select().click2d(90.0, 90.0);
    twenty(&mut hand, [Point2::new(30.0, 30.0), Point2::new(50.0, 30.0)], Point2::new(40.0, 24.0));
    // its radius and its length dimensioned: both come as references, held as they are by the rest
    let r = 20.0 * std::f64::consts::FRAC_1_SQRT_2;
    size_of(&mut hand, [Point2::new(30.0 + r, 30.0 + r), Point2::new(52.0, 52.0)]);
    hand.sk_select().click2d(30.0 + r, 30.0 + r);
    hand.right_click2d(30.0 + r, 30.0 + r);
    assert!(hand.press_word(&crate::i18n::tr("sk-arc-length-dim"), egui::pos2(0.0, 0.0)), "no arc length in the menu of the arc");
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    let before = references(hand.app);
    let quarter = std::f64::consts::FRAC_PI_2;
    assert!(
        before.radius.is_some_and(|d| (d - 20.0).abs() < 1e-6) && before.length.is_some_and(|l| (l - 20.0 * quarter).abs() < 1e-6),
        "setup: the radius and the length of the arc held by the rest come as references of 20 and {:.4}: {:?}",
        20.0 * quarter,
        hand.app.project.sketches[0].constraints
    );
    assert!(twenty_six(&mut hand, Point2::new(40.0, 24.0)), "no dimension reading 20 to open");
    let after = references(hand.app);
    let status = hand.app.status.clone();
    assert!(
        after.radius.is_some_and(|d| (d - 26.0).abs() < 1e-6) && after.length.is_some_and(|l| (l - 26.0 * quarter).abs() < 1e-6),
        "the distance made 26: the reference radius says {:?}, the reference length {:?}, not 26 and {:.4}; the status line says {status:?}",
        after.radius,
        after.length,
        26.0 * quarter
    );
}

#[test]
fn a_reference_radius_of_a_circle_reads_it_after_its_size_changed() {
    let mut app = sketch();
    let mut hand = Hand::new(&mut app);
    // a circle about (30, 30) through (50, 30), its centre fixed, and a line from its centre to its rim, 20 long
    hand.sk_tool(qymcad_ui_state::SketchShape::Circle).click2d(30.0, 30.0).click2d(50.0, 30.0).key(egui::Key::Escape).key(egui::Key::Escape);
    hand.sk_select().click2d(30.0, 30.0);
    hand.constraint(qymcad_ui_state::ConstraintKind::Fix);
    hand.key(egui::Key::Escape);
    hand.sk_select().click2d(90.0, 90.0);
    hand.sk_tool(qymcad_ui_state::SketchShape::Line).click2d(30.0, 30.0).click2d(50.0, 30.0).key(egui::Key::Escape).key(egui::Key::Escape);
    twenty(&mut hand, [Point2::new(30.0, 30.0), Point2::new(50.0, 30.0)], Point2::new(40.0, 24.0));
    let r = 20.0 * std::f64::consts::FRAC_1_SQRT_2;
    size_of(&mut hand, [Point2::new(30.0 + r, 30.0 + r), Point2::new(52.0, 52.0)]);
    let before = references(hand.app).radius;
    assert!(before.is_some_and(|d| (d - 20.0).abs() < 1e-6), "setup: the size of the circle held by the line comes as a reference of 20: {:?}", hand.app.project.sketches[0].constraints);
    assert!(twenty_six(&mut hand, Point2::new(40.0, 24.0)), "no dimension reading 20 to open");
    let after = references(hand.app).radius;
    assert!(after.is_some_and(|d| (d - 26.0).abs() < 1e-6), "the line made 26: the reference radius of the circle says {after:?}");
}
