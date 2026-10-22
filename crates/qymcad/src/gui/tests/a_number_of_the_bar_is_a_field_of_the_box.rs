//! A NUMBER OF THE BAR IS A FIELD OF THE BOX, as a length is: the sides of a polygon and the height of a text are typed in
//! the box at the geometry, judged as they are typed - what a value comes to, or why it is refused, said under the field
//! - and a refused value is not taken by the tool.
//!
//! Reported behaviour: moved from the bar into the box, the sides of a polygon, the steps and the count of a pattern, the
//! height of a text had no line under the field, no unit, and their refusals said nothing or were missing.
use super::*;
use qymcad_core::feature::SketchPlane;

/// WHAT IS TYPED INTO A NUMBER, and the tool's value it must leave or give.
struct Case {
    typed: &'static str,
}

/// The refusal a field of `label` within `range` gives to `typed`, as the frame judges it.
fn refusal(label: &'static str, number: qymcad_ui_state::Number, range: std::ops::RangeInclusive<f64>, typed: &str, project: &qymcad_core::model::Project) -> Option<String> {
    let mut p = qymcad_ui_state::CmdParam::new(label, "x", 1.0, *range.start(), *range.end());
    p.number = number;
    p.txt = typed.to_string();
    qymcad_ui_state::field_value(&p, &project.param_map()).err()
}

/// THE SIDES OF A POLYGON: every refused text is said under the field and leaves the sides as they were; a good formula
/// is taken and said as what it comes to.
#[test]
fn the_sides_of_a_polygon_are_judged_in_the_box() {
    let mut app = App::default();
    let si = app.create_sketch_on(SketchPlane::default());
    app.chosen.sel = Sel::Sketch(si);
    let mut hand = super::super::hand::Hand::new(&mut app);
    hand.sk_tool(qymcad_ui_state::SketchShape::Polygon);
    let label = crate::i18n::tr("opt-sides");
    let was = hand.app.tool_prefs.poly_n;
    for case in [Case { typed: "abc" }, Case { typed: "-5" }, Case { typed: "w/0" }, Case { typed: "nobody*2" }, Case { typed: "500" }, Case { typed: "4.5" }] {
        assert!(hand.type_after_word(&label, case.typed), "the box has no field of {label:?}: {:?}", hand.words_drawn());
        hand.frame(Vec::new());
        let why = refusal("opt-sides", qymcad_ui_state::Number::Whole, 3.0..=64.0, case.typed, &hand.app.project).unwrap_or_else(|| panic!("{:?} is not refused by the field", case.typed));
        assert!(hand.words_drawn().iter().any(|(t, _)| t.contains(&why)), "{:?}: the box does not say {why:?} under the field: {:?}", case.typed, hand.words_drawn());
        hand.key(egui::Key::Enter);
        assert_eq!(hand.app.tool_prefs.poly_n, was, "{:?} was taken as the sides of the polygon", case.typed);
    }
    // a formula is said as what it comes to under the field; a plain number says itself
    assert!(hand.type_after_word(&label, "2+3"));
    hand.frame(Vec::new());
    let said = format!("= {}", crate::i18n::num(5.0, 3));
    assert!(hand.words_drawn().iter().any(|(t, _)| *t == said), "the box does not say {said:?} under the field: {:?}", hand.words_drawn());
    assert_eq!(hand.app.tool_prefs.poly_n, 5, "2+3 typed was not taken as the sides of the polygon");
}

/// THE HEIGHT OF A TEXT: a height of nothing or a broken formula is said under the field and left out; a good one taken.
#[test]
fn the_height_of_a_text_is_judged_in_the_box() {
    let mut app = App::default();
    let si = app.create_sketch_on(SketchPlane::default());
    app.chosen.sel = Sel::Sketch(si);
    let mut hand = super::super::hand::Hand::new(&mut app);
    hand.sk_tool(qymcad_ui_state::SketchShape::Text);
    let label = crate::i18n::tr("opt-height-short");
    let was = hand.app.tool_prefs.text_h;
    for case in [Case { typed: "abc" }, Case { typed: "-5" }, Case { typed: "w/0" }, Case { typed: "0" }] {
        assert!(hand.type_after_word(&label, case.typed), "the box has no field of {label:?}: {:?}", hand.words_drawn());
        hand.frame(Vec::new());
        let why = refusal("opt-height-short", qymcad_ui_state::Number::Real, 0.1..=1000.0, case.typed, &hand.app.project).unwrap_or_else(|| panic!("{:?} is not refused by the field", case.typed));
        assert!(hand.words_drawn().iter().any(|(t, _)| t.contains(&why)), "{:?}: the box does not say {why:?} under the field: {:?}", case.typed, hand.words_drawn());
        assert!((hand.app.tool_prefs.text_h - was).abs() < 1e-12, "{:?} was taken as the height of the text", case.typed);
    }
    assert!(hand.type_after_word(&label, "12"));
    hand.frame(Vec::new());
    assert!((hand.app.tool_prefs.text_h - 12.0).abs() < 1e-12, "12 typed was not taken as the height of the text");
}
