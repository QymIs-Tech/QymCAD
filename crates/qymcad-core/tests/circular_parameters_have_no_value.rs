//! PARAMETERS THAT READ EACH OTHER IN A CIRCLE HAVE NO VALUE, and no evaluation makes their numbers grow. The values are
//! worked out in the order the formulas read each other, from the formulas alone, so evaluating twice gives what
//! evaluating once gave.
//!
//! Reported behaviour: `a = b+1` was taken with `b = a+1`, and every evaluation started from the values before it and
//! went round eight times - 28 / 29, then 60 / 61, then 76 / 77; `s = s*2` doubled eight times a pass.
use qymcad_core::errors::ExprError;
use qymcad_core::model::{Param, Project};

/// A parameter written as the table shows it, `a = b+1`, holding `value` from before.
fn param(row: &str, value: f64) -> Param {
    let (name, expr) = row.split_once('=').expect("a row is `name = formula`");
    Param { name: name.trim().into(), expr: expr.trim().into(), value }
}

fn document(params: Vec<Param>) -> Project {
    let mut p = Project::default();
    p.new_document();
    p.parameters = params;
    p
}

fn value_of(p: &Project, name: &str) -> f64 {
    p.parameters.iter().find(|q| q.name == name).map(|q| q.value).expect("the parameter")
}

/// The fault `eval_parameters` gave for a name, out of its list of what was evaluated and why it has no value.
macro_rules! fault_of {
    ($errs:expr, $name:expr) => {
        $errs.iter().find(|f| f.0 == $name).map(|f| f.1.clone())
    };
}

#[test]
fn two_parameters_reading_each_other_have_no_value_and_do_not_grow() {
    // a document saved with the circle in it, as the window used to take it: a = b+1 with b = a+1, at 10 and 11
    let mut p = document(vec![param("a = b+1", 10.0), param("b = a+1", 11.0)]);
    let first = p.eval_parameters();
    let (a, b) = (value_of(&p, "a"), value_of(&p, "b"));
    for _ in 0..3 {
        p.eval_parameters();
    }
    assert_eq!((value_of(&p, "a"), value_of(&p, "b")), (a, b), "the circle a = b+1, b = a+1 grows on every evaluation");
    assert_eq!(fault_of!(first, "a"), Some(ExprError::LeansOnItself("a -> b -> a".into())), "a in the circle is not said with its chain");
    assert_eq!(fault_of!(first, "b"), Some(ExprError::LeansOnItself("b -> a -> b".into())), "b in the circle is not said with its chain");
    let reading = qymcad_core::model::NamedFormula { name: "x", text: "a*2" };
    assert_eq!(p.check_formula(&reading), Err(ExprError::NameWithoutValue("a".into())), "a formula reading a name in a circle has a value");
}

#[test]
fn a_parameter_reading_itself_has_no_value_and_does_not_grow() {
    let mut p = document(vec![param("s = s*2", 3.0)]);
    let errs = p.eval_parameters();
    p.eval_parameters();
    assert_eq!(value_of(&p, "s"), 3.0, "s = s*2 doubles on every evaluation");
    assert_eq!(fault_of!(errs, "s"), Some(ExprError::LeansOnItself("s -> s".into())));
}

/// A NAME THAT READS A NAME IN A CIRCLE has no value either, and says which name it lacks.
#[test]
fn a_parameter_reading_a_circle_has_no_value() {
    let mut p = document(vec![param("a = b+1", 10.0), param("b = a+1", 11.0), param("c = a*2", 20.0)]);
    let errs = p.eval_parameters();
    assert_eq!(fault_of!(errs, "c"), Some(ExprError::NameWithoutValue("a".into())), "c reading a in a circle is not said");
    let reading = qymcad_core::model::NamedFormula { name: "x", text: "c+1" };
    assert_eq!(p.check_formula(&reading), Err(ExprError::NameWithoutValue("c".into())), "a formula reading c, which reads a circle, has a value");
}

/// THE ORDER IS THE ORDER THE FORMULAS READ EACH OTHER IN, whatever order the rows stand in, and a chain longer than
/// eight is worked out whole.
#[test]
fn values_are_worked_out_in_the_order_the_formulas_read_each_other() {
    let mut rows: Vec<Param> = (1..12).rev().map(|k| param(&format!("p{k} = p{}+1", k - 1), 0.0)).collect();
    rows.push(param("p0 = 1", 0.0));
    let mut p = document(rows);
    assert!(p.eval_parameters().is_empty());
    assert_eq!(value_of(&p, "p11"), 12.0, "a chain of twelve standing in reverse order is not worked out");
}

/// A DIVISION BY ZERO STAYS A FORMULA WITH NO NUMBER, as it was.
#[test]
fn a_division_by_zero_is_still_no_number() {
    let mut p = document(vec![param("d = 1/0", 5.0)]);
    let errs = p.eval_parameters();
    assert_eq!(fault_of!(errs, "d"), Some(ExprError::NotANumber));
}

/// THE WINDOW'S DOOR REFUSES A FORMULA THAT WOULD CLOSE A CIRCLE, with the chain.
#[test]
fn a_formula_closing_a_circle_is_refused() {
    let mut p = document(vec![param("a = 10", 10.0), param("b = a+1", 11.0)]);
    p.eval_parameters();
    assert_eq!(p.check_param_text(0, "b+1"), Err(ExprError::LeansOnItself("a -> b -> a".into())));
    assert_eq!(p.check_param_text(0, "a*2"), Err(ExprError::LeansOnItself("a -> a".into())));
    assert_eq!(p.check_param_text(0, "12"), Ok(12.0));
}

/// A RENAME THAT WOULD CLOSE A CIRCLE IS REFUSED: `a = b+1` reads a name nobody holds, and `c = a*2` renamed to `b` would
/// make the two read each other.
#[test]
fn a_rename_closing_a_circle_is_refused() {
    let mut p = document(vec![param("a = b+1", 0.0), param("c = a*2", 0.0)]);
    assert!(matches!(p.rename_driver("c", "b"), Err(qymcad_core::drivers::RenameError::LeansOnItself(chain)) if chain == "b -> a -> b"), "renaming c to b closed the circle");
    assert_eq!(p.parameters[1].name, "c", "a refused rename renamed");
}
