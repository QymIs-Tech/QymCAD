//! Renaming a driver carries the formulas with it.
//!
//! The measurement that started the rework: the name of a parameter was edited straight in the model on every
//! keystroke, and the core had no reference updating at all — no `rename_*` function existed. Renaming `w` to
//! `shirina` therefore left expressions such as `w*2+5` all over the project, pointing at a name that no longer
//! existed, and it broke on the very first letter.
//!
//! The bar is what a professional CAD does: renaming a parameter there corrects every expression referring to
//! it, and does so as one action.
use qymcad_core::drivers::{check_ident, IdentError, RenameError};
use qymcad_core::expr;
use qymcad_core::geom::Point2;
use qymcad_core::model::{Constraint, Id, Param, Project};

fn param(p: &mut Project, name: &str, e: &str) {
    let v = p.eval_expr(e).unwrap_or(0.0);
    p.parameters.push(Param { name: name.into(), expr: e.into(), value: v });
}

/// A sketch with one dimension named as a driver. Returns the sketch.
fn sketch_with_driver(p: &mut Project, name: &str, driver: &str, len: f64) -> Id {
    let sid = p.add_line_sketch(
        name,
        vec![Point2::new(0.0, 0.0), Point2::new(len, 0.0), Point2::new(len, 10.0), Point2::new(0.0, 10.0)],
        true,
    );
    let si = p.sketch_index(sid).unwrap();
    let pts: Vec<Id> = p.sketches[si].points.iter().map(|q| q.id).collect();
    p.sketches[si].constraints.push(Constraint::Distance { a: pts[0], b: pts[1], d: len, off: 0.0, expr: String::new(), driven: false, axis: 0 });
    assert!(p.add_named_dim(driver.into(), sid, vec![pts[0], pts[1]]), "the dimension is named as a driver");
    sid
}

// ── name boundaries inside an expression ─────────────────────────────────────────────────────────

/// Other words are left alone. A `w` inside `wall`, `pow` or `w2` is a letter in another word rather than a
/// name, and a substring replacement would turn the formula into nonsense silently.
#[test]
fn only_whole_names_are_replaced() {
    assert_eq!(expr::rename_ident("w*2+5", "w", "shirina"), "shirina*2+5");
    assert_eq!(expr::rename_ident("wall+w", "w", "shirina"), "wall+shirina");
    assert_eq!(expr::rename_ident("pow(w,2)", "w", "shirina"), "pow(shirina,2)");
    assert_eq!(expr::rename_ident("w2+w_1+w", "w", "s"), "w2+w_1+s");
    assert_eq!(expr::rename_ident("(w+h)/w", "w", "s"), "(s+h)/s");
    assert_eq!(expr::rename_ident("h*2", "w", "s"), "h*2", "an unrelated expression must not change");
}

/// A multi-byte letter is never split in the middle: names are written in the language of the author, and
/// working byte by byte ends in a panic.
#[test]
fn cyrillic_names_survive() {
    assert_eq!(expr::rename_ident("ширина*2", "ширина", "длина"), "длина*2");
    assert_eq!(expr::rename_ident("ширина_общая+ширина", "ширина", "w"), "ширина_общая+w");

    // The search for a name has the same problem, and it lived in the project before the rework: `mentions`
    // stepped byte by byte, so a multi-byte name whose start matched another name crashed the parse. It only
    // surfaced when the first match was rejected by a boundary check, which is why it went unnoticed.
    assert!(!expr::mentions("ширина_общая", "ширина"), "a name inside a longer name is not a name of its own");
    assert!(expr::mentions("ширина_общая+ширина", "ширина"));
    assert!(expr::mentions("длина", "длина"));
}

// ── what makes a valid name ──────────────────────────────────────────────────────────────────────

#[test]
fn a_name_must_be_usable_in_a_formula() {
    assert_eq!(check_ident("len"), Ok(()));
    assert_eq!(check_ident("_len2"), Ok(()));
    assert_eq!(check_ident("ширина"), Ok(()), "non-Latin letters are allowed");
    assert_eq!(check_ident("  len  "), Ok(()), "surrounding spaces are trimmed");

    assert_eq!(check_ident(""), Err(IdentError::Empty));
    assert_eq!(check_ident("2w"), Err(IdentError::BadStart('2')), "a name cannot start with a digit");
    assert_eq!(check_ident("моя длина"), Err(IdentError::BadChar(' ')), "a space would break the name inside a formula");
    assert_eq!(check_ident("a.b"), Err(IdentError::BadChar('.')));
    assert_eq!(check_ident("a-b"), Err(IdentError::BadChar('-')), "an operator inside a name is not allowed");
}

// ── renaming ─────────────────────────────────────────────────────────────────────────────────────

/// The point: references follow the name while the values stay put.
#[test]
fn renaming_a_parameter_updates_every_reference() {
    let mut p = Project::default();
    p.new_document();
    param(&mut p, "w", "50");
    param(&mut p, "h", "w*2+5");
    let sid = sketch_with_driver(&mut p, "Profile", "len", 20.0);
    let si = p.sketch_index(sid).unwrap();
    if let Some(e) = p.sketches[si].constraints[0].expr_mut() {
        *e = "w/2".into();
    }
    let node = p.timeline.first().map(|n| n.id).unwrap_or(sid);
    p.set_feat_dim(node, "h", "w+3".into());

    let before = p.eval_expr("w*2+5").unwrap();
    let fixed = p.rename_driver("w", "shirina").expect("the rename has to go through");

    assert_eq!(p.parameters[0].name, "shirina", "the name itself did not change");
    assert_eq!(p.parameters[1].expr, "shirina*2+5", "the formula of the parameter still points at the vanished name");
    assert_eq!(p.sketches[si].constraints[0].expr(), Some("shirina/2"), "the expression of the sketch dimension was not updated");
    assert_eq!(p.feat_dim(node, "h"), Some("shirina+3"), "the feature parameter was not updated");
    assert_eq!(fixed, 3, "the wrong number of expressions was corrected: {fixed}");
    assert_eq!(p.eval_expr("shirina*2+5").unwrap(), before, "the value changed as a result of the rename");
}

/// A driving dimension renames the same way, being in the same scope.
#[test]
fn renaming_a_sketch_driver_updates_references() {
    let mut p = Project::default();
    p.new_document();
    sketch_with_driver(&mut p, "Profile", "len", 20.0);
    param(&mut p, "zazor", "len/4");

    let fixed = p.rename_driver("len", "dlina").expect("the driving dimension is renamed");
    assert_eq!(fixed, 1);
    assert_eq!(p.named_dims[0].name, "dlina");
    assert_eq!(p.parameters[0].expr, "dlina/4");
    assert_eq!(p.param_map().get("dlina"), Some(&20.0), "the driver dropped out of scope");
    assert!(p.param_map().get("len").is_none(), "the old name is still reachable");
}

/// A rename onto a taken name is refused, and the document is left untouched by the refusal.
#[test]
fn renaming_onto_a_taken_name_is_refused_and_changes_nothing() {
    let mut p = Project::default();
    p.new_document();
    param(&mut p, "w", "50");
    param(&mut p, "h", "w*2");

    assert_eq!(p.rename_driver("w", "h"), Err(RenameError::Taken));
    assert_eq!(p.parameters[0].name, "w", "the name changed despite the refusal");
    assert_eq!(p.parameters[1].expr, "w*2", "the formulas were touched despite the refusal");
}

#[test]
fn a_bad_name_is_refused_with_a_reason() {
    let mut p = Project::default();
    p.new_document();
    param(&mut p, "w", "50");

    assert_eq!(p.rename_driver("w", "2w"), Err(RenameError::Bad(IdentError::BadStart('2'))));
    assert_eq!(p.rename_driver("w", "моя ширина"), Err(RenameError::Bad(IdentError::BadChar(' '))));
    assert_eq!(p.rename_driver("w", ""), Err(RenameError::Bad(IdentError::Empty)));
    assert_eq!(p.parameters[0].name, "w", "the name changed despite the refusal");
}

#[test]
fn renaming_something_that_does_not_exist_is_refused() {
    let mut p = Project::default();
    p.new_document();
    assert_eq!(p.rename_driver("net_takogo", "novoe"), Err(RenameError::NotFound));
}

/// Changing the case of one's own name is not a conflict. Names are compared case-insensitively, and `len` to
/// `Len` would otherwise run into being taken by itself.
#[test]
fn changing_the_case_of_ones_own_name_is_allowed() {
    let mut p = Project::default();
    p.new_document();
    param(&mut p, "len", "10");
    param(&mut p, "h", "len*2");

    assert!(p.rename_driver("len", "Len").is_ok(), "a change of case has to go through");
    assert_eq!(p.parameters[0].name, "Len");
    assert_eq!(p.parameters[1].expr, "Len*2");
}

/// Renaming to the same thing is neither work nor an error.
#[test]
fn renaming_to_the_same_name_does_nothing() {
    let mut p = Project::default();
    p.new_document();
    param(&mut p, "w", "50");
    assert_eq!(p.rename_driver("w", "w"), Ok(0));
    assert_eq!(p.rename_driver(" w ", "w"), Ok(0), "surrounding spaces still mean the same name");
}

// ── who owns a name ──────────────────────────────────────────────────────────────────────────────

/// The interface has to say not merely "taken" but what exactly has taken it.
///
/// Either identical names are forbidden outright, or it has to be clear which sketch, body or assembly a name
/// comes from. A refusal without naming the owner leaves the namesake to be hunted down by hand across the
/// whole project.
#[test]
fn a_taken_name_says_who_holds_it() {
    let mut p = Project::default();
    p.new_document();
    let comp = p.add_component("Housing");
    p.set_active_component(Some(comp));
    let sid = sketch_with_driver(&mut p, "Profile", "len", 20.0);
    p.add_sketch_node(sid, "Profile");

    let owner = p.name_owner("len").expect("the owner of a name has to be found");
    assert_eq!(owner.path, "Housing.Profile", "an owner without a path explains nothing");
    assert_eq!(owner.value, Some(20.0));

    assert!(p.name_owner("LEN").is_some(), "case must not hide a namesake");
    assert!(p.name_owner("svobodno").is_none());
    assert!(p.name_owner("").is_none(), "an empty name belongs to nobody");
}

// ── numbers in a field ───────────────────────────────────────────────────────────────────────────

/// No more than four decimals, and no trailing zeros.
///
/// Automatically generated values in input fields carried far too many decimals; four are enough, and the rule
/// applies to every field. A bare `format!("{v}")` prints the whole truth about an `f64`, leaving the tail to
/// be cleaned up by hand.
#[test]
fn a_number_put_into_a_field_is_short_and_clean() {
    assert_eq!(expr::fmt_num(12.75), "12.75");
    assert_eq!(expr::fmt_num(40.0), "40", "trailing zeros are not printed");
    assert_eq!(expr::fmt_num(0.1 + 0.2), "0.3", "the whole truth about an f64 is not what the reader needs");
    assert_eq!(expr::fmt_num(12.750000000000002), "12.75");
    assert_eq!(expr::fmt_num(1.0 / 3.0), "0.3333", "exactly four decimals, no more");
    assert_eq!(expr::fmt_num(-2.5), "-2.5");
    assert_eq!(expr::fmt_num(0.00001), "0", "below the fourth decimal there is nothing to show");
    assert_eq!(expr::fmt_num(-0.00001), "0", "and a minus sign on zero is not printed");
    assert_eq!(expr::fmt_num(123456.789), "123456.789");
}
