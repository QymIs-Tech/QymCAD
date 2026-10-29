use super::super::hand::Hand;
use super::super::App;
use crate::i18n::tr;
use qymcad_ui_state::{Again, CircleWay, RectWay, SketchShape};

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

fn sketch(hand: &mut Hand, again: Again) {
    hand.press_word(&tr("start-close"), egui::Pos2::ZERO);
    assert!(hand.press_word(&tr("plane-xy-table"), egui::Pos2::ZERO), "setup: no XY plane to begin a sketch on");
    choose(hand, again);
}

fn said_of(way: CircleWay) -> String {
    let word = match way {
        CircleWay::CentreRadius => "opt-circle-centre-radius",
        CircleWay::TwoPoints => "opt-circle-2pt",
        CircleWay::Tangent => "opt-tangent-m",
        CircleWay::ThreePoints => "opt-rect-3pt",
    };
    crate::i18n::tr2("opt-way-in-force", "tool", &tr("opt-circle-mode"), "way", &tr(word))
}

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
    hand.key(egui::Key::C).frame(Vec::new());
    let said = said_of(CircleWay::TwoPoints);
    if !hand.words_drawn().iter().any(|(w, _)| w.contains(&said)) {
        wrong.push(format!("the status line does not say {said:?} after the next way was taken"));
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

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

struct WithWays {
    key: egui::Key,
    words: &'static [&'static str],
}

const WITH_WAYS: [WithWays; 4] = [
    WithWays { key: egui::Key::R, words: &["opt-rect-2corners", "opt-rect-centre", "opt-rect-3pt"] },
    WithWays { key: egui::Key::C, words: &["opt-circle-centre-radius", "opt-circle-2pt", "opt-tangent-m", "opt-rect-3pt"] },
    WithWays { key: egui::Key::A, words: &["opt-arc-cse", "opt-rect-3pt", "opt-tangent"] },
    WithWays { key: egui::Key::G, words: &["opt-polygon-inscribed", "opt-polygon-circumscribed", "opt-by-edge"] },
];

#[derive(Debug, PartialEq)]
struct Left {
    ways: String,
    armed: String,
    pts: usize,
    tangents: usize,
}

#[derive(Clone, Copy)]
enum Next {
    Key,
    Bar,
}

fn next_way_from(tool: &WithWays, from: usize, by: Next) -> Left {
    let mut app = App::default();
    let mut hand = Hand::canvas(&mut app);
    sketch(&mut hand, Again::Cycles);
    for _ in 0..=from {
        hand.key(tool.key);
    }
    hand.click2d(0.0, 0.0);
    match by {
        Next::Key => {
            hand.key(tool.key);
        }
        Next::Bar => {
            let word = tr(tool.words[(from + 1) % tool.words.len()]);
            assert!(hand.press_word(&word, egui::pos2(400.0, 0.0)), "{:?} from way {from}: no word {word:?} on the bar", tool.key);
        }
    }
    let p = &hand.app.tool_prefs;
    let ways = format!("{:?} {:?} {:?} {:?}", p.rect_mode, p.circ_mode, p.arc_mode, p.poly_mode);
    Left { ways, armed: format!("{:?}", hand.app.tools.armed), pts: hand.app.tools.tool.pts.len(), tangents: hand.app.tools.tool.circ_tan.len() }
}

#[test]
fn the_key_pressed_again_does_what_the_bar_does() {
    let mut wrong = Vec::new();
    for tool in &WITH_WAYS {
        for from in 0..tool.words.len() {
            let (by_key, by_bar) = (next_way_from(tool, from, Next::Key), next_way_from(tool, from, Next::Bar));
            if by_key != by_bar {
                wrong.push(format!("{:?} from way {from}: by the key {by_key:?}, by the bar {by_bar:?}", tool.key));
            }
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}
