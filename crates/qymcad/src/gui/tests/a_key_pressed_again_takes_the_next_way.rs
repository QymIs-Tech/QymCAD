//! A KEY PRESSED AGAIN TAKES THE NEXT WAY OF ITS TOOL, when Settings -> Keyboard says so: C, C, C, C, C walks the circle
//! through its four ways and back to the first. A tool taken anew starts at its first way, a way picked on the bar holds
//! and the next press goes on from it, the points clicked for one way are dropped when the way changes, and a tool with
//! one way is put down whatever is chosen. With the choice left at "puts the tool down" nothing changes.

use super::super::hand::Hand;
use super::super::App;
use crate::i18n::tr;
use qymcad_ui_state::{Again, CircleWay, RectWay, SketchShape};

/// SETTINGS -> KEYBOARD SET TO `again` through the window, opened and closed from the Windows menu, as a person sets it.
fn choose(hand: &mut Hand, again: Again) {
    let choice = match again {
        Again::PutsDown => "settings-again-puts-down",
        Again::Cycles => "settings-again-cycles",
    };
    let (windows, settings) = (tr("menu-windows"), tr("menu-settings"));
    let set = hand.press_word(&windows, egui::Pos2::ZERO)
        && hand.press_word(&settings, egui::Pos2::ZERO)
        && hand.press_word(&tr("settings-sec-keyboard"), egui::Pos2::ZERO)
        && hand.press_word(&tr(choice), egui::Pos2::ZERO)
        && hand.press_word(&windows, egui::Pos2::ZERO)
        && hand.press_word(&settings, egui::Pos2::ZERO);
    assert!(set, "setup: Settings -> Keyboard could not be set to {again:?} through the window");
    assert_eq!(hand.app.set.again, again, "setup: the window was gone through and the choice is not {again:?}");
}

/// A NEW SKETCH OPEN FOR DRAWING, the key pressed again answering as `again` says: the start screen closed, the sketch
/// begun on the XY plane by its word, the choice made in Settings -> Keyboard.
fn sketch(hand: &mut Hand, again: Again) {
    hand.press_word(&tr("start-close"), egui::Pos2::ZERO);
    assert!(hand.press_word(&tr("plane-xy-table"), egui::Pos2::ZERO), "setup: no XY plane to begin a sketch on");
    choose(hand, again);
}

/// What the status line says the way of the circle in force is: the name of the tool and the word of the way.
fn said_of(way: CircleWay) -> String {
    let word = match way {
        CircleWay::CentreRadius => "opt-circle-centre-radius",
        CircleWay::TwoPoints => "opt-circle-2pt",
        CircleWay::Tangent => "opt-tangent-m",
        CircleWay::ThreePoints => "opt-rect-3pt",
    };
    crate::i18n::tr2("opt-way-in-force", "tool", &tr("opt-circle-mode"), "way", &tr(word))
}

/// C, C, C, C, C: the circle by its centre and radius, by two points, touching two curves, by three points, and by its
/// centre and radius again - the circle in hand all along, the status line naming each way.
#[test]
fn the_key_of_the_circle_walks_its_ways_and_comes_back() {
    let mut app = App::default();
    let mut hand = Hand::canvas(&mut app);
    sketch(&mut hand, Again::Cycles);
    let mut wrong = Vec::new();
    for want in [CircleWay::CentreRadius, CircleWay::TwoPoints, CircleWay::Tangent, CircleWay::ThreePoints, CircleWay::CentreRadius] {
        hand.key(egui::Key::C);
        let (held, way) = (hand.app.tools.armed.drawing(), hand.app.tool_prefs.circ_mode);
        if held != Some(SketchShape::Circle) || way != want {
            wrong.push(format!("in hand {held:?} by {way:?}, want the circle by {want:?}"));
        }
    }
    // one more press: the status line names the way it took
    hand.key(egui::Key::C).frame(Vec::new());
    let said = said_of(CircleWay::TwoPoints);
    if !hand.words_drawn().iter().any(|(w, _)| w.contains(&said)) {
        wrong.push(format!("the status line does not say {said:?} after the next way was taken"));
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// C, C, Esc, C: the circle taken anew starts by its centre and radius, not by the two points it was left at.
#[test]
fn a_tool_taken_anew_starts_at_its_first_way() {
    let mut app = App::default();
    let mut hand = Hand::canvas(&mut app);
    sketch(&mut hand, Again::Cycles);
    hand.key(egui::Key::C).key(egui::Key::C);
    assert_eq!(hand.app.tool_prefs.circ_mode, CircleWay::TwoPoints, "setup: C, C did not take the circle by two points");
    hand.key(egui::Key::Escape);
    assert!(qymcad_ui_state::in_select_mode(&hand.app.tools.armed), "setup: Esc left {:?} in hand", hand.app.tools.armed);
    hand.key(egui::Key::C);
    assert_eq!(hand.app.tool_prefs.circ_mode, CircleWay::CentreRadius, "the circle taken anew kept the way it was put down in");
}

/// C, "tangent" picked on the bar, C: the next press goes on from the way picked, to three points.
#[test]
fn the_next_press_goes_on_from_the_way_picked_on_the_bar() {
    let mut app = App::default();
    let mut hand = Hand::canvas(&mut app);
    sketch(&mut hand, Again::Cycles);
    hand.key(egui::Key::C);
    assert!(hand.press_word(&tr("opt-tangent-m"), egui::pos2(400.0, 0.0)), "no way \"tangent\" on the bar of the circle");
    assert_eq!(hand.app.tool_prefs.circ_mode, CircleWay::Tangent, "setup: the word on the bar did not pick the tangent way");
    hand.key(egui::Key::C);
    assert_eq!(hand.app.tool_prefs.circ_mode, CircleWay::ThreePoints, "C after the tangent way picked on the bar did not take three points");
    assert_eq!(hand.app.tools.armed.drawing(), Some(SketchShape::Circle), "C after a way picked on the bar put the circle down");
}

/// L, L puts the line down under either choice: a tool with one way answers a second press as it always did.
#[test]
fn a_tool_with_one_way_is_put_down_under_either_choice() {
    for again in [Again::PutsDown, Again::Cycles] {
        let mut app = App::default();
        let mut hand = Hand::canvas(&mut app);
        sketch(&mut hand, again);
        hand.key(egui::Key::L);
        assert_eq!(hand.app.tools.armed.drawing(), Some(SketchShape::Line), "{again:?}: setup: L did not take the line");
        hand.key(egui::Key::L);
        assert!(qymcad_ui_state::in_select_mode(&hand.app.tools.armed), "{again:?}: L pressed again left {:?} in hand, not the arrow", hand.app.tools.armed);
    }
}

/// E, E puts the extrusion of a part down under either choice: no command of a part has ways.
#[test]
fn a_command_of_a_part_is_put_down_under_either_choice() {
    for again in [Again::PutsDown, Again::Cycles] {
        let mut app = App::default();
        let mut hand = Hand::new(&mut app);
        hand.press_word(&tr("start-close"), egui::Pos2::ZERO);
        assert!(hand.press_hint(&tr("tb-new-part-hint")), "{again:?}: setup: no button of a new part");
        choose(&mut hand, again);
        let extrude = qymcad_ui_state::catalogue::Launch::Feat(qymcad_ui_state::PartTool::Extrude);
        hand.key(egui::Key::E);
        assert!(qymcad_ui_state::catalogue::pressed(extrude, &hand.app.bar_ctx(&mut Vec::new())), "{again:?}: setup: E did not take the extrusion");
        hand.key(egui::Key::E);
        assert!(!qymcad_ui_state::catalogue::pressed(extrude, &hand.app.bar_ctx(&mut Vec::new())), "{again:?}: E pressed again left the extrusion in hand");
    }
}

/// Under "puts the tool down" C, C puts the circle down, and the way left on the bar comes back with the next C.
#[test]
fn puts_down_keeps_the_way_left_on_the_bar() {
    let mut app = App::default();
    let mut hand = Hand::canvas(&mut app);
    sketch(&mut hand, Again::PutsDown);
    hand.key(egui::Key::C);
    assert!(hand.press_word(&tr("opt-circle-2pt"), egui::pos2(400.0, 0.0)), "no way \"by 2 points\" on the bar of the circle");
    hand.key(egui::Key::C);
    assert!(qymcad_ui_state::in_select_mode(&hand.app.tools.armed), "C pressed again left {:?} in hand, not the arrow", hand.app.tools.armed);
    hand.key(egui::Key::C);
    assert_eq!(hand.app.tool_prefs.circ_mode, CircleWay::TwoPoints, "the circle taken again lost the way left on its bar");
}

/// Two points of a turned rectangle clicked, R: the next way is taken, no rectangle is drawn and no point is left for
/// the way taken - the clicks of one way mean nothing to another.
#[test]
fn the_points_of_one_way_are_dropped_when_the_way_changes() {
    let mut app = App::default();
    let mut hand = Hand::canvas(&mut app);
    sketch(&mut hand, Again::Cycles);
    hand.key(egui::Key::R).key(egui::Key::R).key(egui::Key::R);
    assert_eq!(hand.app.tool_prefs.rect_mode, RectWay::ThreePoints, "setup: R, R, R did not take the rectangle by three points");
    let before = hand.app.project.sketches[0].entities.len();
    hand.click2d(0.0, 0.0).click2d(30.0, 0.0);
    assert_eq!(hand.app.tools.tool.pts.len(), 2, "setup: the two clicks of the turned rectangle were not kept");
    hand.key(egui::Key::R);
    let (way, left, drawn) = (hand.app.tool_prefs.rect_mode, hand.app.tools.tool.pts.len(), hand.app.project.sketches[0].entities.len() - before);
    assert_eq!(way, RectWay::TwoCorners, "R after the last way did not come back to the first");
    assert_eq!(left, 0, "the points of the turned rectangle were left for the rectangle by two corners");
    assert_eq!(drawn, 0, "the change of the way drew a shape");
}

/// THE PREVIEW AFTER C, C IS THE CIRCLE BY TWO POINTS: a click at (0, 0) and the pointer at (20, 0) draw the circle of
/// the diameter between them, about (10, 0) - not the circle about the click through the pointer.
#[test]
fn the_preview_after_the_key_pressed_again_is_the_circle_by_two_points() {
    let mut app = App::default();
    let mut hand = Hand::canvas(&mut app);
    sketch(&mut hand, Again::Cycles);
    hand.key(egui::Key::C).key(egui::Key::C);
    hand.click2d(0.0, 0.0).hover2d(20.0, 0.0);
    hand.frame(Vec::new());
    let (centre, end) = (hand.on_screen2d((10.0, 0.0)), hand.on_screen2d((20.0, 0.0)));
    let radius = centre.distance(end);
    let mut rings = Vec::new();
    fn walk(s: &egui::Shape, out: &mut Vec<egui::epaint::CircleShape>) {
        match s {
            egui::Shape::Circle(c) => out.push(*c),
            egui::Shape::Vec(v) => v.iter().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    hand.shapes_drawn().iter().for_each(|cs| walk(&cs.shape, &mut rings));
    let found = rings.iter().any(|c| c.center.distance(centre) < 1.0 && (c.radius - radius).abs() < 1.0);
    assert!(found, "no preview of radius {radius:.1} px about {centre:?}; the circles drawn: {:?}", rings.iter().map(|c| (c.center, c.radius)).collect::<Vec<_>>());
}

/// THE KEY PRESSED AGAIN DOES WHAT THE NEXT WORD OF THE BAR DOES: the centre of a polygon clicked, G takes the next way
/// and the centre stays, as it stays when "circum." is picked on the bar - both ways of the centre read the same click.
#[test]
fn the_key_pressed_again_does_what_the_bar_does() {
    let mut kept = Vec::new();
    for by_bar in [true, false] {
        let mut app = App::default();
        let mut hand = Hand::canvas(&mut app);
        sketch(&mut hand, Again::Cycles);
        hand.key(egui::Key::G).click2d(0.0, 0.0);
        if by_bar {
            assert!(hand.press_word(&tr("opt-polygon-circumscribed"), egui::pos2(400.0, 0.0)), "no way \"circum.\" on the bar of the polygon");
        } else {
            hand.key(egui::Key::G);
        }
        assert_eq!(hand.app.tool_prefs.poly_mode, qymcad_ui_state::PolygonWay::Circumscribed, "by bar {by_bar}: setup: the way did not change");
        kept.push(hand.app.tools.tool.pts.len());
    }
    assert_eq!(kept[1], kept[0], "the centre clicked: kept {} by the bar and {} by the key pressed again", kept[0], kept[1]);
}
