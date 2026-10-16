//! A NAME WHOSE OCCURRENCES OVERLAP IS RENAMED, not a reason for the program to end.
//!
//! The search for a name went on one character after each place it was found, so the next place could begin inside
//! the last one: `a+a` is found in `a+a+a` at 0 and again at 2. Renaming then cut the text from the end of the first
//! (3) to the start of the second (2) and panicked. A name of letters, digits and `_` cannot overlap itself - the
//! check of the characters around a match refuses the second - but a project file can hold any name, and a rename
//! checks only the new one. Places found now never overlap: the search goes on after the end of each.
use qymcad_core::expr;
use qymcad_core::model::{Param, Project};

#[test]
fn overlapping_places_are_renamed_once() {
    assert_eq!(expr::rename_ident("a+a+a", "a+a", "x"), "x+a");
    assert_eq!(expr::rename_ident("a+a+a+a", "a+a", "x"), "x+x");
    assert!(expr::mentions("a+a+a", "a+a"), "the name is still found");
}

/// The way the program meets it: a parameter named `a+a` - as a project file can hold it - and a formula using it,
/// renamed from the parameters window.
#[test]
fn a_parameter_named_against_the_rules_is_renamed_in_its_formulas() {
    let mut p = Project::default();
    p.parameters.push(Param { name: "a+a".into(), expr: "1".into(), value: 1.0 });
    p.parameters.push(Param { name: "b".into(), expr: "a+a+a".into(), value: 0.0 });
    assert_eq!(p.rename_driver("a+a", "x"), Ok(1), "one formula is corrected");
    assert_eq!(p.parameters[1].expr, "x+a");
}
