//! THE ONE VOICE ON SCREEN: a panel, a popup and a table say a fault in the theme's colour of its tone, with its icon,
//! in a dark and in a light theme alike - the colour follows the theme, not a constant.
use crate::gui::hand::{Hand, PaintedWords};
use crate::gui::{App, Sel};

/// The themes a person picks between: the colour of a tone must follow each.
const THEMES: [&str; 2] = ["dark", "light"];

/// Put the theme on, as the settings do.
fn wear(app: &mut App, theme: &str) {
    app.set.scheme = theme.into();
    crate::gui::apply_theme(&mut app.scheme, &app.set, &egui::Context::default());
}

/// The painted run that starts with the icon of `tone` and holds `words`.
fn said<'a>(painted: &'a [PaintedWords], tone: qymcad_ui_state::Tone, words: &str) -> Option<&'a PaintedWords> {
    let icon = qymcad_ui_state::tone_icon(tone).expect("the tone of a fault has an icon");
    painted.iter().find(|w| w.text.starts_with(icon) && w.text.contains(words))
}

/// THE PROPERTIES OF A NODE THAT DID NOT BUILD say why in red, with the error's icon - the red of the theme worn.
#[test]
fn the_properties_of_a_broken_node_say_why_in_the_red_of_the_theme() {
    let mut reds = Vec::new();
    for theme in THEMES {
        let mut app = crate::gui::screen_keys::tests::plate();
        wear(&mut app, theme);
        let block = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the plate");
        // a rounding of an edge the plate never had: it cannot build at all
        let fillet = app.project.add_fillet_ref(block, 1.0, qymcad_core::refs::Ref::picks(&[0x6000_7777]));
        let mut hand = Hand::new(&mut app);
        hand.settle();
        let ti = hand.app.project.timeline.iter().position(|n| n.id == fillet || n.kind.body() == Some(fillet)).expect("the rounding");
        let node = hand.app.project.timeline[ti].id;
        let why = hand.app.project.regen_errors.get(&node).map(crate::gui::error_words::error_text).unwrap_or_else(|| panic!("setup: the rounding of no edge goes red in {theme}"));
        hand.app.chosen.sel = Sel::Feature(ti);
        // the panel of properties along the right of the window - the status line below says the same reason, and is
        // not what is looked at here
        let painted: Vec<PaintedWords> = hand.painted_words().into_iter().filter(|w| w.rect.min.x > 1100.0 && w.rect.max.y < 860.0).collect();
        let words = said(&painted, qymcad_ui_state::Tone::Error, &why).unwrap_or_else(|| panic!("{theme}: the properties do not say {why:?} with the error's icon"));
        assert_eq!(words.colour, hand.app.scheme.pal.error(), "{theme}: the reason is not in the theme's red");
        reds.push(words.colour);
    }
    assert_ne!(reds[0], reds[1], "the red of the properties is the same in a dark and a light theme: a constant, not the theme");
}

/// A SIZE THE CORNERS CANNOT TAKE is refused in the box of the corners, in the soft red of the theme and with the
/// refusal's icon, as a value refused in the popup of a part is.
#[test]
fn the_box_of_the_corners_refuses_a_size_in_the_soft_red_of_the_theme() {
    let mut softs = Vec::new();
    for theme in THEMES {
        let mut app = App::default();
        wear(&mut app, theme);
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.chosen.sel = Sel::Sketch(si);
        let mut hand = Hand::new(&mut app);
        hand.sk_tool(1);
        for (x, y) in [(0.0, 0.0), (40.0, 0.0), (40.0, 30.0)] {
            hand.click2d(x, y);
        }
        hand.key(egui::Key::Escape);
        let s = &hand.app.project.sketches[si];
        let corner = s.entities.iter().filter(|e| matches!(e.kind, qymcad_core::model::EntityKind::Line { .. })).map(|e| (1u8, e.id)).collect::<Vec<_>>();
        assert_eq!(corner.len(), 2, "setup: two lines meet at a corner");
        assert!(hand.sk_corner_typed("tb-fillet-sketch-hint", None, &corner, 0.0), "{theme}: no place to click on the two lines");
        let why = crate::i18n::tr("cmd-value-zero");
        let painted = hand.painted_words();
        let words =
            said(&painted, qymcad_ui_state::Tone::Refusal, &why).unwrap_or_else(|| panic!("{theme}: the box does not refuse with {why:?}: {:?}", painted.iter().map(|w| &w.text).collect::<Vec<_>>()));
        assert_eq!(words.colour, hand.app.scheme.pal.error_mild(), "{theme}: the refusal is not in the theme's soft red");
        softs.push(words.colour);
        hand.key(egui::Key::Escape);
    }
    assert_ne!(softs[0], softs[1], "the soft red of a refusal is the same in a dark and a light theme");
}

/// A PARAMETER WHOSE FORMULA GIVES NO VALUE: the table says the name and the reason under itself in the theme's red,
/// with the error's icon.
#[test]
fn the_table_says_a_formula_without_a_value_in_the_red_of_the_theme() {
    let mut reds = Vec::new();
    for theme in THEMES {
        let mut app = App::default();
        wear(&mut app, theme);
        app.project.parameters.push(qymcad_core::model::Param { name: "w".into(), expr: "60 +".into(), value: 0.0 });
        let why = crate::gui::error_words::expr_error_text(&app.project.eval_expr("60 +").expect_err("the formula is broken"));
        let mut hand = Hand::new(&mut app);
        assert!(hand.press_word(&crate::i18n::tr("wb-params"), egui::pos2(0.0, 0.0)), "the button of the parameters is not on screen");
        let painted = hand.painted_words();
        let words = said(&painted, qymcad_ui_state::Tone::Error, &format!("w: {why}")).unwrap_or_else(|| panic!("{theme}: the table does not say why w has no value"));
        assert_eq!(words.colour, hand.app.scheme.pal.error(), "{theme}: the reason is not in the theme's red");
        // AND THE ROW ITSELF SAYS IT IN WORDS, in the cell of the value: a sign alone there is read by nobody
        let none = crate::i18n::tr("par-no-value");
        let cell = said(&painted, qymcad_ui_state::Tone::Error, &none).unwrap_or_else(|| panic!("{theme}: the cell of the value of w does not say {none:?}"));
        assert_eq!(cell.colour, hand.app.scheme.pal.error(), "{theme}: the cell says it is without a value, not in the theme's red");
        reds.push(words.colour);
    }
    assert_ne!(reds[0], reds[1], "the red of the table is the same in a dark and a light theme");
}
