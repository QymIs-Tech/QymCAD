//! A CIRCLE IS DRAWN IN EACH OF ITS MODES, on the path of a person: the circle tool taken, its mode chosen in the bar.
//! Touching two curves - two lines, a line and a circle, a spline and a line - a click on each curve and one where the
//! circle should stand lay a circle of the radius of the bar touching both. By three points - three clicks lay the
//! circle through them. There is no tool of its own for a circle through three points: it is a mode of the circle.
//!
//! Reported behaviour: the circle through three points was a button of its own beside the circle, and the tangent mode
//! touched one curve only, its radius fitted rather than given.
use crate::gui::hand::Hand;
use crate::gui::{App, Sel};
use qymcad_core::geom::Point2;
use qymcad_core::model::curve::Curve;
use qymcad_core::model::Id;

/// THE TWO CURVES A CIRCLE TOUCHES.
#[derive(Clone, Copy, Debug)]
enum Two {
    Lines,
    LineAndCircle,
    SplineAndLine,
}

/// WHERE A PERSON CLICKS: on the first curve, on the second, and where the circle should stand.
struct Clicks {
    first: Point2,
    second: Point2,
    stand: Point2,
}

/// A new sketch, open for drawing.
fn sketch() -> App {
    let mut app = App::default();
    let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    app.chosen.sel = Sel::Sketch(si);
    app
}

/// The two curves drawn by hand, and where a person clicks for a circle of radius 5 touching them.
fn drawn(hand: &mut Hand, two: Two) -> Clicks {
    match two {
        Two::Lines => {
            hand.sk_tool(qymcad_ui_state::SketchShape::Line).click2d(10.0, 10.0).double_click2d(60.0, 10.0);
            hand.click2d(10.0, 10.0).double_click2d(10.0, 60.0);
            hand.key(egui::Key::Escape).key(egui::Key::Escape);
            Clicks { first: Point2::new(40.0, 10.0), second: Point2::new(10.0, 40.0), stand: Point2::new(16.0, 16.0) }
        }
        Two::LineAndCircle => {
            hand.sk_tool(qymcad_ui_state::SketchShape::Line).click2d(10.0, 10.0).double_click2d(70.0, 10.0);
            hand.key(egui::Key::Escape).key(egui::Key::Escape);
            hand.sk_tool(qymcad_ui_state::SketchShape::Circle).click2d(40.0, 24.0).click2d(48.0, 24.0).key(egui::Key::Escape).key(egui::Key::Escape);
            Clicks { first: Point2::new(60.0, 10.0), second: Point2::new(40.0, 32.0), stand: Point2::new(31.0, 15.0) }
        }
        Two::SplineAndLine => {
            hand.sk_tool(qymcad_ui_state::SketchShape::Line).click2d(10.0, 10.0).double_click2d(70.0, 10.0);
            hand.key(egui::Key::Escape).key(egui::Key::Escape);
            hand.sk_tool(qymcad_ui_state::SketchShape::Spline).click2d(10.0, 18.0).click2d(25.0, 21.0);
            hand.double_click2d(40.0, 18.0);
            hand.key(egui::Key::Escape).key(egui::Key::Escape);
            Clicks { first: Point2::new(65.0, 10.0), second: Point2::new(25.0, 21.0), stand: Point2::new(18.0, 15.0) }
        }
    }
}

/// The circles of the sketch.
fn circles(app: &App) -> Vec<Curve> {
    let s = &app.project.sketches[0];
    s.entities.iter().filter(|e| e.kind.as_circle().is_some()).filter_map(|e| Curve::of(s, e)).collect()
}

/// The circle tool taken and its mode `word` pressed on the bar.
fn circle_in(hand: &mut Hand, word: &str) {
    hand.sk_tool(qymcad_ui_state::SketchShape::Circle);
    assert!(hand.press_word(&crate::i18n::tr(word), egui::pos2(400.0, 0.0)), "no mode {word:?} on the bar of the circle");
}

#[test]
fn a_circle_touching_two_curves_is_laid_where_it_was_clicked() {
    let mut failures = Vec::new();
    for two in [Two::Lines, Two::LineAndCircle, Two::SplineAndLine] {
        let mut app = sketch();
        let mut hand = Hand::new(&mut app);
        let Clicks { first, second, stand } = drawn(&mut hand, two);
        let before: Vec<Id> = hand.app.project.sketches[0].entities.iter().map(|e| e.id).collect();
        circle_in(&mut hand, "opt-tangent-m");
        let r = hand.app.tool_prefs.circ_r;
        hand.click2d(first.x, first.y).click2d(second.x, second.y);
        hand.hover2d(stand.x, stand.y);
        hand.click2d(stand.x, stand.y);
        hand.key(egui::Key::Escape).key(egui::Key::Escape);
        let s = &hand.app.project.sketches[0];
        let made: Vec<&qymcad_core::model::SketchEntity> = s.entities.iter().filter(|e| e.kind.as_circle().is_some() && !before.contains(&e.id)).collect();
        let Some(Curve::Round { centre, r: laid, .. }) = made.as_slice().first().filter(|_| made.len() == 1).and_then(|e| Curve::of(s, e)) else {
            failures.push(format!("{two:?}: {} circles laid by the clicks, want 1; the status line says {:?}", made.len(), hand.app.status));
            continue;
        };
        let touching = s.entities.iter().filter(|e| e.id != made[0].id).filter_map(|e| Curve::of(s, e)).filter(|c| (c.nearest(centre).off - laid).abs() < 1e-4).count();
        if (laid - r).abs() > 1e-6 || touching < 2 || centre.dist(stand) > 8.0 {
            failures.push(format!("{two:?}: a circle of radius {laid} (the bar says {r}) about {centre:?}, touching {touching} of the curves, clicked at {stand:?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_circle_by_three_points_is_a_mode_of_the_circle() {
    let mut app = sketch();
    let mut hand = Hand::new(&mut app);
    circle_in(&mut hand, "opt-rect-3pt");
    hand.click2d(40.0, 30.0).click2d(30.0, 40.0).click2d(20.0, 30.0);
    hand.key(egui::Key::Escape).key(egui::Key::Escape);
    let all = circles(hand.app);
    let through = matches!(all.as_slice(), [Curve::Round { centre, r, .. }] if centre.dist(Point2::new(30.0, 30.0)) < 1e-6 && (r - 10.0).abs() < 1e-6);
    assert!(through, "three clicks in the mode of three points laid {all:?}, not the circle of radius 10 about (30, 30)");
}
