//! A DIMENSION IS APPLIED WHEN IT IS FINISHED, AND IT IS NEVER NEGATIVE.
//!
//! Reported behaviour: "negative dimensions in sketches. It happens that you place a dimension, try to
//! type it in, and because of the constraints already placed, while you are erasing, 100 becomes 1 - and at
//! that moment the geometry gets thrown into the negative, and since negative dimensions do not exist the
//! whole part goes crooked and you cannot get it back."
//!
//! TWO HALVES, and each was measured before being fixed.
//!
//! FIRST: erasing passes through values nobody asked for. For a LINEAR dimension this lesson had already
//! been learnt - "the value is applied on commit, not on every letter" - but the field one over, the radius
//! and diameter of a circle, still edited the model on every keystroke. Erasing "100" there means the
//! circle is really rebuilt at 10 and at 1 on the way.
//!
//! SECOND: a length of zero or less reaches the solver. Measured on a rectangle 100 x 40 with a distance
//! dimension on its lower edge: setting that dimension to -100 collapses the edge to nothing - both of its
//! ends land on (50, 0) - and leaves a residual of 100, an inconsistent system. `DistancePL` already took
//! the magnitude and kept its own sign, so the rule was known; it was simply applied to one kind of
//! dimension out of six.
//!
//! WHY REFUSING BEATS ROLLING BACK. The editor does roll a conflicting value back afterwards, and that
//! saved the sketch here. But a rollback is a cure applied after the damage: it depends on the residual
//! rising above a threshold, and a value that quietly solves to a mirrored answer never trips it. A length
//! that cannot exist should not be handed to the solver at all.

use qymcad_core::model::{Constraint, Project};

/// A rectangle 100 x 40 with a distance dimension of 100 on its lower edge, and THE INDEX OF THAT
/// DIMENSION.
///
/// The index is returned rather than assumed to be zero: a rectangle brings its own constraints
/// (horizontals, verticals, coincidences) and the dimension lands after them. The first edition of the
/// check passed 0, `set_dim_value` refused it because a horizontal is not a dimension, the edge stayed at
/// 100 - and the check went green over a value that had never been offered. Its companion, "an ordinary
/// value is still taken", is what caught it.
fn a_rectangle_with_a_width_dimension() -> (Project, usize, usize) {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    p.add_rect_entity(si, 0.0, 0.0, 100.0, 40.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    let mut ids: Vec<(u64, f64, f64)> = p.sketches[si].points.iter().map(|q| (q.id, q.x, q.y)).collect();
    ids.sort_by(|a, b| a.2.total_cmp(&b.2).then(a.1.total_cmp(&b.1)));
    let (a, b) = (ids[0].0, ids[1].0);
    p.sketches[si].constraints.push(Constraint::Distance { a, b, d: 100.0, off: 0.0, expr: String::new(), driven: false, axis: 0 });
    let ci = p.sketches[si].constraints.len() - 1;
    p.solve_sketch(si);
    (p, si, ci)
}

/// The length of the sketch's lower edge as it actually stands.
fn lower_edge(p: &Project, si: usize) -> f64 {
    let mut q: Vec<(f64, f64)> = p.sketches[si].points.iter().map(|k| (k.x, k.y)).collect();
    q.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.total_cmp(&b.0)));
    (q[0].0 - q[1].0).hypot(q[0].1 - q[1].1)
}

/// A LENGTH OF ZERO OR LESS IS REFUSED, and the sketch keeps its shape.
#[test]
fn a_length_that_cannot_exist_is_not_handed_to_the_solver() {
    let mut bad = Vec::new();
    for v in [-100.0f64, -1.0, 0.0] {
        let (mut p, si, ci) = a_rectangle_with_a_width_dimension();
        let took = qymcad_sketch::set_dim_value(&mut p, si, ci, v);
        p.solve_sketch(si);
        let edge = lower_edge(&p, si);
        if took || (edge - 100.0).abs() > 1e-3 {
            bad.push(format!("{v} was taken={took}, the edge became {edge:.3} instead of 100"));
        }
    }
    assert!(
        bad.is_empty(),
        "a length of zero or less reached the solver, and it collapses the edge to nothing:\n{}",
        bad.join("\n")
    );
}

/// AND AN ORDINARY VALUE STILL GOES IN.
///
/// Without this the fix could be "refuse everything", which would be a sketcher that takes no dimensions.
#[test]
fn an_ordinary_value_is_still_taken() {
    let (mut p, si, ci) = a_rectangle_with_a_width_dimension();
    assert!(qymcad_sketch::set_dim_value(&mut p, si, ci, 60.0), "60 mm is an ordinary width and it was refused");
    p.solve_sketch(si);
    assert!((lower_edge(&p, si) - 60.0).abs() < 1e-3, "the edge is {:.3} instead of 60", lower_edge(&p, si));
}

/// AN ANGLE MAY BE NEGATIVE, and refusing it would be a new defect in place of the old one.
///
/// A negative angle means the other way round; there is no such thing as the other way round for a length.
#[test]
fn a_negative_angle_is_still_allowed() {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    p.add_line_entity(si, 0.0, 0.0, 40.0, 0.0, qymcad_core::feature::Purpose::Real);
    p.add_line_entity(si, 0.0, 0.0, 30.0, 30.0, qymcad_core::feature::Purpose::Real);
    p.regen_sketch(si);
    let q: Vec<u64> = p.sketches[si].points.iter().map(|x| x.id).collect();
    p.sketches[si].constraints.push(Constraint::AngleLines { a: q[0], b: q[1], c: q[0], d: q[2], deg: 45.0, expr: String::new(), driven: false });
    let ci = p.sketches[si].constraints.len() - 1;
    assert!(qymcad_sketch::set_dim_value(&mut p, si, ci, -30.0), "a negative angle means the other way round and must be allowed");
}

/// THE RADIUS FIELD DOES NOT REBUILD THE CIRCLE ON EVERY LETTER.
///
/// Driven through the editor in real frames, because the whole complaint is about what happens BETWEEN
/// keystrokes: a check that set the value and asked for the result would never see the states the report is
/// about.
///
/// CONFIRMED BY THE TICK, NOT BY ENTER. In a headless frame the text field never takes the keyboard - the
/// same wall the F2 check ran into - so Enter reaches nothing and a check built on it would be measuring an
/// empty room. The tick button beside the field is the other door a person has, it needs no focus, and it
/// is clicked here for real. What is typed is put into the field's own buffer, which is what typing does.
#[test]
fn erasing_the_diameter_does_not_resize_the_circle_until_it_is_finished() {
    let mut b = qymcad_ui_state::Bench::default();
    b.project.new_document();
    let si = b.project.new_sketch("S");
    let sid = b.project.sketches[si].id;
    b.project.add_circle_entity(si, 0.0, 0.0, 50.0, qymcad_core::feature::Purpose::Real);
    b.project.regen_sketch(si);
    b.sketch_ses.editing = Some(sid);
    b.sel = qymcad_ui_state::Sel::Sketch(si);
    let eid = b.project.sketches[si].entities[0].id;
    b.inline = qymcad_ui_state::InlineEdit::Circle(eid);
    b.dim.focus = true;
    b.view = qymcad_ui_state::View2d { center: egui::Vec2::ZERO, scale: 4.0, initialized: true };

    let radius = |b: &qymcad_ui_state::Bench| match b.project.sketches[si].entities[0].kind {
        qymcad_core::model::EntityKind::Circle { r, .. } => r,
        _ => f64::NAN,
    };
    let ctx = egui::Context::default();
    let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0));
    let frame = |b: &mut qymcad_ui_state::Bench, events: Vec<egui::Event>| -> Vec<(String, egui::Rect)> {
        fn walk(s: &egui::epaint::Shape, out: &mut Vec<(String, egui::Rect)>) {
            match s {
                egui::epaint::Shape::Text(t) => out.push((t.galley.text().to_string(), egui::Rect::from_min_size(t.pos, t.galley.size()))),
                egui::epaint::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let raw = egui::RawInput { screen_rect: Some(rect), events, ..Default::default() };
        let out = ctx.run_ui(raw, |ui| qymcad_sketch::dim_editor(&mut b.sketch_ctx(), ui.ctx(), rect));
        let mut texts = Vec::new();
        for cs in &out.shapes {
            walk(&cs.shape, &mut texts);
        }
        texts
    };

    // A COUPLE OF FRAMES: an `Area` places itself a frame late, and the field asks for the focus until it
    // gets it.
    for _ in 0..3 {
        frame(&mut b, Vec::new());
    }
    assert_eq!(b.dim.buf.trim(), "100", "GUARD: the field must open on the diameter, or what follows is about nothing");
    assert!(ctx.memory(|m| m.focused()).is_some(), "GUARD: the field must hold the keyboard, or nothing below is really typed");

    // TYPED FOR REAL. On auto-focus the whole text is selected, so the first keystroke replaces it - which
    // is exactly what a person gets - and the erasing after that is one character at a time.
    frame(&mut b, vec![egui::Event::Text("100".into())]);
    let mut seen = Vec::new();
    for _ in 0..3 {
        frame(&mut b, vec![egui::Event::Key { key: egui::Key::Backspace, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }]);
        seen.push((b.dim.buf.clone(), radius(&b)));
    }
    assert_eq!(seen.iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>(), vec!["10", "1", ""], "GUARD: the erasing must really go through 10 and 1: {seen:?}");
    let moved: Vec<String> = seen.iter().filter(|(_, r)| (r - 50.0).abs() > 1e-9).map(|(t, r)| format!("with \"{t}\" in the field the circle was already rebuilt to r = {r}")).collect();
    assert!(moved.is_empty(), "the circle is rebuilt while the number is still being erased:\n{}", moved.join("\n"));

    // AND WHAT IS FINISHED DOES GO IN.
    frame(&mut b, vec![egui::Event::Text("30".into())]);
    frame(&mut b, vec![egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }]);
    assert!((radius(&b) - 15.0).abs() < 1e-9, "Enter must apply what is in the field: the radius is {} and 30 as a diameter is 15", radius(&b));
}
