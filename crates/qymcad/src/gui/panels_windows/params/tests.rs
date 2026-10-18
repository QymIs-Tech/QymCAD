//! THE TABLE OF PARAMETERS UNDER A PERSON'S HAND: what is typed into the row of a named dimension, read off the screen
//! of whole frames.
use crate::gui::App;
use crate::gui::hand::Hand;
use qymcad_core::geom::Point2;
use qymcad_core::model::{Constraint, Id};

/// A rectangle 40 x 20 in a sketch of its own, its width named `a_len` and its height `b_len`. Answers the sketch's
/// index.
fn rectangle_with_two_drivers(app: &mut App) -> usize {
    app.project.new_document();
    let corners = vec![Point2::new(0.0, 0.0), Point2::new(40.0, 0.0), Point2::new(40.0, 20.0), Point2::new(0.0, 20.0)];
    let sid = app.project.add_line_sketch("Profile", corners, true);
    app.project.add_sketch_node(sid, "Profile");
    let si = app.project.sketch_index(sid).expect("the sketch just added");
    let pts: Vec<Id> = app.project.sketches[si].points.iter().map(|q| q.id).collect();
    let width = Constraint::Distance { a: pts[0], b: pts[1], d: 40.0, off: 0.0, expr: String::new(), driven: false, axis: 0, at: None };
    let height = Constraint::Distance { a: pts[1], b: pts[2], d: 20.0, off: 0.0, expr: String::new(), driven: false, axis: 0, at: None };
    app.project.sketches[si].constraints.extend([width, height]);
    assert!(app.project.add_named_dim("a_len".into(), sid, vec![pts[0], pts[1]]), "setup: the width is named");
    assert!(app.project.add_named_dim("b_len".into(), sid, vec![pts[1], pts[2]]), "setup: the height is named");
    si
}

/// The height of the rectangle as the sketch stands: the distance between its second and third corners. A distance, not a
/// difference of y: the sketch holds no horizontal or vertical, and its sides may lean after a solve.
fn height(app: &App, si: usize) -> f64 {
    let p = &app.project.sketches[si].points;
    (p[2].x - p[1].x).hypot(p[2].y - p[1].y)
}

/// A FORMULA TYPED INTO THE ROW OF A NAMED DIMENSION STAYS A FORMULA: the height written as `a_len/4` follows the width
/// when the width is changed in the same table, and the row shows the formula, not the number it gave once.
///
/// Reported behaviour: the table evaluated what was typed and kept the number - a formula of a named dimension became a
/// constant, and the name it read changed without it.
#[test]
fn a_formula_typed_for_a_named_dimension_follows_its_names() {
    let mut app = App::default();
    let si = rectangle_with_two_drivers(&mut app);
    let mut hand = Hand::new(&mut app);
    assert!(hand.press_word(&crate::i18n::tr("wb-params"), egui::pos2(0.0, 0.0)), "the button of the parameters is not on screen");
    assert!(hand.type_after_word("b_len", "a_len/4"), "the value of b_len is not on screen");
    hand.key(egui::Key::Enter).settle();
    assert!(hand.type_after_word("a_len", "60"), "the value of a_len is not on screen");
    hand.key(egui::Key::Enter).settle();
    assert!((height(hand.app, si) - 15.0).abs() < 1e-6, "the width went to 60 and the height written as a_len/4 stands at {}", height(hand.app, si));
    assert!(hand.written_at("a_len/4").is_some(), "the row of b_len does not show the formula typed into it");
}

/// A NUMBER TYPED OVER THE FORMULA REPLACES IT: the height becomes a constant and no longer follows the width.
#[test]
fn a_number_typed_over_a_formula_makes_the_dimension_a_constant() {
    let mut app = App::default();
    let si = rectangle_with_two_drivers(&mut app);
    let mut hand = Hand::new(&mut app);
    assert!(hand.press_word(&crate::i18n::tr("wb-params"), egui::pos2(0.0, 0.0)), "the button of the parameters is not on screen");
    assert!(hand.type_after_word("b_len", "a_len/4"), "the value of b_len is not on screen");
    hand.key(egui::Key::Enter).settle();
    assert!(hand.type_after_word("b_len", "25"), "the value of b_len is not on screen");
    hand.key(egui::Key::Enter).settle();
    assert!(hand.written_at("a_len/4").is_none(), "the formula stays in the row of b_len after a number was typed over it");
    assert!(hand.type_after_word("a_len", "60"), "the value of a_len is not on screen");
    hand.key(egui::Key::Enter).settle();
    assert!((height(hand.app, si) - 25.0).abs() < 1e-6, "the height typed as 25 followed the width to {}", height(hand.app, si));
}

/// A FORMULA THAT COMES BACK TO ITS OWN NAME IS REFUSED, said on the status line in the tone of a refusal, and the document
/// is left as it was with no step of undo. Taken, `b_len + 1` would add one to the height on every evaluation.
#[test]
fn a_formula_that_comes_back_to_its_own_name_is_refused() {
    let mut app = App::default();
    rectangle_with_two_drivers(&mut app);
    let pal = app.scheme.pal.clone();
    let undo_before = app.disk.edits.undo.len();
    let mut hand = Hand::new(&mut app);
    assert!(hand.press_word(&crate::i18n::tr("wb-params"), egui::pos2(0.0, 0.0)), "the button of the parameters is not on screen");
    assert!(hand.type_after_word("b_len", "b_len + 1"), "the value of b_len is not on screen");
    hand.key(egui::Key::Enter).settle();
    let why = crate::gui::error_words::expr_error_text(&qymcad_core::errors::ExprError::LeansOnItself("b_len -> b_len".into()));
    let said = hand.status_painted().into_iter().find(|w| w.text.contains(&why));
    assert!(said.as_ref().is_some_and(|w| w.colour == pal.error_mild()), "the status line does not refuse b_len + 1 in the tone of a refusal: {:?}", said.map(|w| (w.text, w.colour)));
    assert_eq!(hand.app.project.param_map().get("b_len"), Some(&20.0), "a formula leaning on itself changed b_len");
    assert_eq!(hand.app.disk.edits.undo.len(), undo_before, "a refused formula left a step of undo");
}

/// A NUMBER AT THE END OF A FORMULA IS NO NAME, and Enter after it applies the formula. A name does not begin with a digit,
/// so the list of names has nothing to offer for `2`.
///
/// Measured: `a_len*2` typed and Enter pressed applied nothing, and the field held `a_len*a_len` when it was left. The `2`
/// opened the list - the list looks in the paths too, and the path of both names of this sketch holds a 2 - and Enter put
/// the first name of the list in its place.
#[test]
fn a_number_ending_a_formula_is_not_taken_for_a_name() {
    let mut app = App::default();
    rectangle_with_two_drivers(&mut app);
    let mut hand = Hand::new(&mut app);
    assert!(hand.press_word(&crate::i18n::tr("wb-params"), egui::pos2(0.0, 0.0)), "the button of the parameters is not on screen");
    assert!(hand.type_after_word("b_len", "a_len*2"), "the value of b_len is not on screen");
    hand.key(egui::Key::Enter).settle();
    assert_eq!(hand.app.project.param_map().get("b_len"), Some(&80.0), "Enter after a_len*2 did not make b_len 80");
}

/// THE LIST OF NAMES UNDER A FORMULA SAYS WHERE EACH NAME STANDS IN WORDS, as the table beside it does: the part by its
/// name, not by the key of the catalogue the document stores it under.
///
/// Measured: `a_` typed into a formula opened the list with `name-part-n#2.Profile` beside `a_len`. The table writes the
/// path of both rows of this sketch, so the list is the third place it stands.
#[test]
fn the_list_of_names_says_the_path_in_words() {
    let mut app = App::default();
    rectangle_with_two_drivers(&mut app);
    let target = app.project.named_dims[0].target.clone();
    let path = qymcad_ui_state::path_in_words(&app.project.driver_path(&target));
    let mut hand = Hand::new(&mut app);
    assert!(hand.press_word(&crate::i18n::tr("wb-params"), egui::pos2(0.0, 0.0)), "the button of the parameters is not on screen");
    assert!(hand.type_after_word("b_len", "a_"), "the value of b_len is not on screen");
    hand.settle();
    let in_list = hand.words_drawn().iter().filter(|(t, _)| *t == path).count();
    assert!(in_list >= 3, "the list under the formula does not say where a_len stands in words ({path:?}): {:?}", hand.words_drawn().iter().map(|(t, _)| t).collect::<Vec<_>>());
}

/// A FORMULA THAT CLOSES A CIRCLE OF PARAMETERS IS REFUSED in words with its chain, and the values stay what they were
/// through every step after it: a parameter added, another one changed, Ctrl+Z.
///
/// Reported behaviour: with `a = 10` and `b = a+1`, `a = b+1` was taken without a word and gave 28 / 29; adding `c = 5`
/// made them 60 / 61; `c` changed to 6 made them 76 / 77; Undo gave `c` back and left `a` / `b` at 76 / 77.
#[test]
fn a_circle_of_parameters_is_refused_and_nothing_grows() {
    let mut app = App::default();
    app.project.new_document();
    app.project.parameters.push(qymcad_core::model::Param { name: "a".into(), expr: "10".into(), value: 10.0 });
    app.project.parameters.push(qymcad_core::model::Param { name: "b".into(), expr: "a+1".into(), value: 11.0 });
    app.project.eval_parameters();
    let pal = app.scheme.pal.clone();
    let mut hand = Hand::new(&mut app);
    let value = |hand: &Hand, name: &str| hand.app.project.param_map().get(name).copied();
    assert!(hand.press_word(&crate::i18n::tr("wb-params"), egui::pos2(0.0, 0.0)), "the button of the parameters is not on screen");

    assert!(hand.type_after_word("a", "b+1"), "the formula of a is not on screen");
    hand.key(egui::Key::Enter).settle();
    assert_eq!((value(&hand, "a"), value(&hand, "b")), (Some(10.0), Some(11.0)), "a = b+1 with b = a+1 was taken");
    let why = crate::gui::error_words::expr_error_text(&qymcad_core::errors::ExprError::LeansOnItself("a -> b -> a".into()));
    let said = hand.status_painted().into_iter().find(|w| w.text.contains(&why));
    assert!(said.as_ref().is_some_and(|w| w.colour == pal.error_mild()), "the circle a -> b -> a is not refused in words: {:?}", said.map(|w| (w.text, w.colour)));

    // a parameter added by its button, named in the empty row (its field shows the example `w`) and given 5
    assert!(hand.press_word(&crate::i18n::tr("win-add-param"), egui::pos2(0.0, 0.0)), "the button that adds a parameter is not on screen");
    let empty = hand.written_at("w").expect("the empty row of the new parameter is not on screen");
    hand.click_screen(empty.center()).type_text("c").key(egui::Key::Enter).settle();
    assert!(hand.type_after_word("c", "5"), "the formula of c is not on screen");
    hand.key(egui::Key::Enter).settle();
    assert_eq!(value(&hand, "c"), Some(5.0), "setup: c = 5 was not added");
    assert_eq!((value(&hand, "a"), value(&hand, "b")), (Some(10.0), Some(11.0)), "adding c = 5 moved a and b");

    assert!(hand.type_after_word("c", "6"), "the formula of c is not on screen");
    hand.key(egui::Key::Enter).settle();
    assert_eq!((value(&hand, "a"), value(&hand, "b")), (Some(10.0), Some(11.0)), "c changed to 6 moved a and b");

    hand.undo().settle();
    assert_eq!(value(&hand, "c"), Some(5.0), "Ctrl+Z did not give c back");
    assert_eq!((value(&hand, "a"), value(&hand, "b")), (Some(10.0), Some(11.0)), "Ctrl+Z left a and b moved");
}

/// A RENAME THAT WOULD CLOSE A CIRCLE IS REFUSED in the table, the chain said under it: `a = b+1` reads a name nobody holds,
/// and `c = a*2` renamed to `b` would make the two read each other.
#[test]
fn a_rename_closing_a_circle_is_refused_in_the_table() {
    let mut app = App::default();
    app.project.new_document();
    app.project.parameters.push(qymcad_core::model::Param { name: "a".into(), expr: "b+1".into(), value: 0.0 });
    app.project.parameters.push(qymcad_core::model::Param { name: "c".into(), expr: "a*2".into(), value: 0.0 });
    app.project.eval_parameters();
    let mut hand = Hand::new(&mut app);
    assert!(hand.press_word(&crate::i18n::tr("wb-params"), egui::pos2(0.0, 0.0)), "the button of the parameters is not on screen");
    let name = hand.written_at("c").expect("the name c is not on screen");
    hand.click_screen(name.center()).chord(egui::Modifiers::COMMAND, egui::Key::A).type_text("b").key(egui::Key::Enter).settle();
    assert_eq!(hand.app.project.parameters[1].name, "c", "c renamed to b closed the circle a -> b");
    let why = crate::gui::error_words::expr_error_text(&qymcad_core::errors::ExprError::LeansOnItself("b -> a -> b".into()));
    assert!(hand.shows(&why), "the table does not say the chain b -> a -> b: {:?}", hand.words_drawn());
}
