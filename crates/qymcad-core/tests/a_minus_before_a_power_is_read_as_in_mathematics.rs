//! A MINUS BEFORE A POWER IS READ AS IN MATHEMATICS. Reported behaviour (issue #220): `-w^2` with `w` = 3 gave 9 and
//! `-2^2` gave 4 - the sign was taken with the base before the power - while mathematics, and the languages engineers
//! write formulas in, give -9 and -4. A formula meant one way silently got the other's sign. And an old document whose
//! formulas change their value under the new reading is told which, rather than left to find out.
use qymcad_core::expr::eval;
use qymcad_core::model::{Constraint, Param, Project, SignRuleChange};
use std::collections::HashMap;

/// A formula and the value it must give with `w` = 3.
struct Reading {
    src: &'static str,
    want: f64,
}

#[test]
fn the_power_comes_before_the_sign() {
    let mut v = HashMap::new();
    v.insert("w".to_string(), 3.0);
    for Reading { src, want } in [
        Reading { src: "-w^2", want: -9.0 },
        Reading { src: "-2^2", want: -4.0 },
        Reading { src: "0-w^2", want: -9.0 },
        Reading { src: "(-w)^2", want: 9.0 },
        Reading { src: "-(w^2)", want: -9.0 },
        Reading { src: "--w^2", want: 9.0 },
        Reading { src: "+w^2", want: 9.0 },
        Reading { src: "2*-w^2", want: -18.0 },
        Reading { src: "-w^2*2", want: -18.0 },
        Reading { src: "12/-w^2", want: -12.0 / 9.0 },
        Reading { src: "2^-1", want: 0.5 },
        Reading { src: "-2^-1", want: -0.5 },
        Reading { src: "2^-2^2", want: 0.0625 },
        Reading { src: "2^3^2", want: 512.0 },
        Reading { src: "-2^3^2", want: -512.0 },
        Reading { src: "-sin(30)^2", want: -0.25 },
        Reading { src: "abs(-2)^2", want: 4.0 },
        Reading { src: "w^2-w^2", want: 0.0 },
    ] {
        let got = eval(src, &v).expect(src);
        assert!((got - want).abs() < 1e-12, "{src}: {got}, not {want}");
    }
}

/// A parameter of the document as it is written: its name and its formula.
struct Written {
    name: &'static str,
    expr: &'static str,
}

fn document(params: &[Written]) -> Project {
    let mut p = Project::default();
    p.new_document();
    for w in params {
        p.parameters.push(Param { name: w.name.into(), expr: w.expr.into(), value: 0.0 });
    }
    p
}

/// The parameters whose value the new reading changes are named with their formula and their value before and now -
/// and only they: an odd power, `0-w^2` and parenthesised formulas mean the same either way.
#[test]
fn a_document_names_the_parameters_that_changed() {
    let p = document(&[
        Written { name: "w", expr: "3" },
        Written { name: "h", expr: "-w^2" },
        Written { name: "k", expr: "w^2" },
        Written { name: "c", expr: "-w^3" },
        Written { name: "z", expr: "0-w^2" },
        Written { name: "q", expr: "(-w)^2" },
    ]);
    let want = SignRuleChange { place: "h".into(), formula: "-w^2".into(), was: Some(9.0), now: Some(-9.0) };
    assert_eq!(p.formulas_changed_by_sign_rule(), vec![want]);
}

/// A parameter that changes only because one it uses changed is named too: each reading is followed through the
/// document. And a formula that evaluates under one reading only is a change as well.
#[test]
fn a_change_is_followed_through_the_parameters_that_use_it() {
    let p = document(&[Written { name: "w", expr: "3" }, Written { name: "h", expr: "-w^2" }, Written { name: "k", expr: "h+1" }, Written { name: "r", expr: "sqrt(h)" }]);
    let want = vec![
        SignRuleChange { place: "h".into(), formula: "-w^2".into(), was: Some(9.0), now: Some(-9.0) },
        SignRuleChange { place: "k".into(), formula: "h+1".into(), was: Some(10.0), now: Some(-8.0) },
        SignRuleChange { place: "r".into(), formula: "sqrt(h)".into(), was: Some(3.0), now: None },
    ];
    assert_eq!(p.formulas_changed_by_sign_rule(), want);
}

/// A feature's formula and a sketch dimension's are named by their owner.
#[test]
fn a_feature_formula_and_a_sketch_dimension_are_named() {
    let mut p = document(&[Written { name: "w", expr: "3" }]);
    p.feat_dims.entry(4242).or_default().insert("height".into(), "-w^2+20".into());
    let si = p.new_sketch("Profile");
    p.sketches[si].constraints.push(Constraint::Distance { a: 1, b: 2, d: 0.0, off: 0.0, expr: "-w^2+30".into(), driven: false, axis: 0, at: None });
    let want = vec![
        SignRuleChange { place: "4242: height".into(), formula: "-w^2+20".into(), was: Some(29.0), now: Some(11.0) },
        SignRuleChange { place: "Profile".into(), formula: "-w^2+30".into(), was: Some(39.0), now: Some(21.0) },
    ];
    assert_eq!(p.formulas_changed_by_sign_rule(), want);
}
