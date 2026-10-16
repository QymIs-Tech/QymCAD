//! A CONSTRAINT THE SKETCH CANNOT MEET IS TAKEN BACK AND NAMED.
//!
//! Reported behaviour: parallel put on two lines an angle of 45 deg holds apart went into the sketch beside the
//! angle; the sketch stayed unsolved (residual 0.79) and nothing was marked. A dimension that does not fit is rolled
//! back and said; a constraint is the other half of the same question.
use qymcad_ui_state::Notice;
use qymcad_core::model::{Constraint, Project};

#[test]
fn parallel_against_an_angle_of_45_is_taken_back() {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    let l1 = p.add_line_entity(si, 0.0, 0.0, 40.0, 0.0, qymcad_core::feature::Purpose::Real);
    let l2 = p.add_line_entity(si, 0.0, 0.0, 30.0, 30.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    let q: Vec<u64> = p.sketches[si].points.iter().map(|x| x.id).collect();
    p.sketches[si].constraints.push(Constraint::AngleLines { a: q[0], b: q[1], c: q[0], d: q[2], deg: 45.0, expr: String::new(), driven: false, off: 0.0, at: None });
    p.solve_sketch(si);
    let before: Vec<(f64, f64)> = p.sketches[si].points.iter().map(|x| (x.x, x.y)).collect();
    let mut regen = qymcad_ui_state::Rebuilding::default();
    let mut sel_sk = qymcad_ui_state::SketchSelection { items: vec![(1, l1), (1, l2)], ..Default::default() };
    let mut status = Notice::default();
    qymcad_sketch::try_constraint_inner(&mut p, &mut regen, qymcad_ui_state::Sel::Sketch(si), &mut sel_sk, &mut status, 3);

    assert!(!p.sketches[si].constraints.iter().any(|c| matches!(c, Constraint::Parallel { .. })), "parallel stayed beside the angle it contradicts");
    let after: Vec<(f64, f64)> = p.sketches[si].points.iter().map(|x| (x.x, x.y)).collect();
    assert!(before.iter().zip(&after).all(|(a, b)| (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6), "the points did not return: {before:?} became {after:?}");
    assert!(status.words.starts_with(&qymcad_i18n::tr1("sk-constraint-conflict", "r", "").split(" (").next().unwrap_or_default().to_string()), "the refusal is not said: {status:?}");
}

/// EQUAL ON TWO CIRCLES THAT EACH CARRY THEIR RADIUS gives them one radius: the second circle's radius becomes a
/// reference (driven) - it follows the first rather than contradicting it. Reported behaviour: the sketch went
/// over-defined, the constraint unsolved (residual 2.89).
#[test]
fn equal_on_two_dimensioned_circles_makes_the_second_radius_a_reference() {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    let e1 = p.add_circle_entity(si, 0.0, 0.0, 10.0, qymcad_core::feature::Purpose::Real);
    let e2 = p.add_circle_entity(si, 40.0, 0.0, 7.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    let centre = |p: &Project, x: f64| p.sketches[si].points.iter().find(|q| (q.x - x).abs() < 1e-6 && q.y.abs() < 1e-6).map(|q| q.id).expect("the centre");
    let (c1, c2) = (centre(&p, 0.0), centre(&p, 40.0));
    p.sketches[si].constraints.push(Constraint::Diameter { c: c1, d: 10.0, off: 0.0, expr: String::new(), driven: false, diam: false, at: None });
    p.sketches[si].constraints.push(Constraint::Diameter { c: c2, d: 7.0, off: 0.0, expr: String::new(), driven: false, diam: false, at: None });
    p.solve_sketch(si);
    let mut regen = qymcad_ui_state::Rebuilding::default();
    let mut sel_sk = qymcad_ui_state::SketchSelection { items: vec![(1, e1), (1, e2)], ..Default::default() };
    let mut status = Notice::default();
    qymcad_sketch::try_constraint_inner(&mut p, &mut regen, qymcad_ui_state::Sel::Sketch(si), &mut sel_sk, &mut status, 5);
    let cs = &p.sketches[si].constraints;
    assert!(cs.iter().any(|c| matches!(c, Constraint::EqualRadius { .. })), "equal was not put on the circles: {status:?}");
    assert!(cs.iter().any(|c| matches!(c, Constraint::Diameter { c, driven: true, .. } if *c == c2)), "the second circle's radius did not become a reference");
    assert!(cs.iter().any(|c| matches!(c, Constraint::Diameter { c, driven: false, .. } if *c == c1)), "the first circle's radius stopped driving");
    let r2 = p.sketches[si]
        .entities
        .iter()
        .find_map(|e| match e.kind {
            qymcad_core::model::EntityKind::Circle { center, r } if center == c2 => Some(r),
            _ => None,
        })
        .expect("the second radius");
    assert!((r2 - 10.0).abs() < 1e-6, "the second circle stands at radius {r2}, not the first's 10");
}
