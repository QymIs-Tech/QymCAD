//! A parameter is never hidden behind a constant of formulas.
//!
//! Reported behaviour: a parameter `e = 5` was accepted, and a dimension `= e * 2` came out 5.436, not 10 - a
//! formula reads `e`, `pi` and `tau` as the constants, so the parameter sat in the table and nothing could reach
//! it. A name a formula cannot reach is refused where it is given, as a name starting with a digit is.
use qymcad_core::drivers::{check_ident, IdentError, RenameError};
use qymcad_core::model::{Param, Project};

/// A parameter as the report wrote it.
struct Given {
    name: &'static str,
    value: f64,
}

/// The probe of the report: whatever the check lets through, a formula naming it reads the parameter.
#[test]
fn a_parameter_named_like_a_constant_is_either_refused_or_used() {
    for Given { name, value } in [Given { name: "e", value: 5.0 }, Given { name: "pi", value: 7.0 }, Given { name: "tau", value: 9.0 }] {
        let accepted = check_ident(name).is_ok();
        let mut p = Project::default();
        p.parameters.push(Param { name: name.into(), expr: value.to_string(), value });
        assert!(p.eval_parameters().is_empty());
        let got = p.eval_expr(name).unwrap();
        assert!(!accepted || got == value, "parameter `{name}` = {value} is accepted, but the expression `{name}` gives {got}");
    }
}

/// The three names are refused with their own reason; the same letters in another case are a parameter of their
/// own, since a formula finds `E` or `Pi` under its own name before it folds the case.
#[test]
fn the_names_of_the_constants_are_refused_and_other_cases_are_not() {
    for name in ["e", "pi", "tau", " pi "] {
        assert_eq!(check_ident(name), Err(IdentError::Constant), "`{name}` is read as a constant in a formula");
    }
    for name in ["E", "Pi", "TAU", "e1", "pi_2", "taux"] {
        assert_eq!(check_ident(name), Ok(()), "`{name}` is no constant and a formula reads it as a parameter");
        let mut p = Project::default();
        p.parameters.push(Param { name: name.into(), expr: "5".into(), value: 5.0 });
        assert!(p.eval_parameters().is_empty());
        assert_eq!(p.eval_expr(&format!("{name} * 2")).unwrap(), 10.0, "a formula did not reach the parameter `{name}`");
    }
}

/// A rename into a constant does not go through, and the formulas keep the old name.
#[test]
fn a_rename_into_a_constant_is_refused() {
    let mut p = Project::default();
    p.parameters.push(Param { name: "w".into(), expr: "5".into(), value: 5.0 });
    p.parameters.push(Param { name: "h".into(), expr: "w * 2".into(), value: 10.0 });
    assert_eq!(p.rename_driver("w", "e"), Err(RenameError::Bad(IdentError::Constant)));
    assert_eq!(p.parameters[0].name, "w", "the parameter took the name of a constant");
    assert_eq!(p.parameters[1].expr, "w * 2", "the formula followed a rename that did not go through");
}

/// A dimension is not named after a constant either: the driver would sit in the table and a formula would read
/// the constant.
#[test]
fn a_dimension_is_not_named_after_a_constant() {
    let mut p = Project::default();
    let node: qymcad_core::model::Id = 7;
    assert!(!p.add_named_feat_dim("e".into(), node, "height"), "a feature number was named `e`");
    assert!(p.named_dims.is_empty(), "the refused name stayed in the document: {:?}", p.named_dims.iter().map(|n| &n.name).collect::<Vec<_>>());
    assert!(p.add_named_feat_dim("E".into(), node, "height"), "`E` is no constant and names the number");
}
