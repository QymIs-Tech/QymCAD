//! WHAT IS TYPED FOR A NAMED NUMBER IN THE TABLE OF PARAMETERS STAYS WHAT WAS TYPED: a formula stays a formula and follows
//! the names it reads, a number makes the dimension a constant, a formula that comes back to its own name is refused.
//! And a feature's named number written as a formula is read in the same scope the rebuild reads it in.
use qymcad_core::errors::ExprError;
use qymcad_core::geom::Point2;
use qymcad_core::model::{Constraint, DimTarget, Id, Project};

/// The document of the checks: a rectangle 40 x 20 with its width named `a_len` and its height `b_len`, extruded 25.
struct Rectangle {
    p: Project,
    si: usize,
    width: DimTarget,
    height: DimTarget,
    extrude: Id,
}

fn rectangle() -> Rectangle {
    let mut p = Project::default();
    p.new_document();
    let corners = vec![Point2::new(0.0, 0.0), Point2::new(40.0, 0.0), Point2::new(40.0, 20.0), Point2::new(0.0, 20.0)];
    let sid = p.add_line_sketch("Profile", corners, true);
    p.add_sketch_node(sid, "Profile");
    let si = p.sketch_index(sid).expect("the sketch just added");
    let pts: Vec<Id> = p.sketches[si].points.iter().map(|q| q.id).collect();
    let width = Constraint::Distance { a: pts[0], b: pts[1], d: 40.0, off: 0.0, expr: String::new(), driven: false, axis: 0, at: None };
    let height = Constraint::Distance { a: pts[1], b: pts[2], d: 20.0, off: 0.0, expr: String::new(), driven: false, axis: 0, at: None };
    p.sketches[si].constraints.extend([width, height]);
    assert!(p.add_named_dim("a_len".into(), sid, vec![pts[0], pts[1]]));
    assert!(p.add_named_dim("b_len".into(), sid, vec![pts[1], pts[2]]));
    let extrude = p.add_extrude_on(sid, 0, 25.0, qymcad_core::feature::Reach::Forward, 0.0);
    let width = DimTarget::Sketch { sketch: sid, refs: Project::dim_key_pub(&[pts[0], pts[1]]) };
    let height = DimTarget::Sketch { sketch: sid, refs: Project::dim_key_pub(&[pts[1], pts[2]]) };
    Rectangle { p, si, width, height, extrude }
}

/// The height of the rectangle as the sketch stands after a solve: the distance between its second and third corners, the
/// sketch holding no horizontal or vertical.
fn solved_height(r: &mut Rectangle) -> f64 {
    r.p.solve_sketch(r.si);
    let pts = &r.p.sketches[r.si].points;
    (pts[2].x - pts[1].x).hypot(pts[2].y - pts[1].y)
}

#[test]
fn a_formula_typed_for_a_dimension_stays_and_follows() {
    let mut r = rectangle();
    assert_eq!(r.p.set_dim_target_text(&r.height.clone(), "a_len/2"), Ok(true));
    assert_eq!(r.p.dim_target_expr(&r.height).as_deref(), Some("a_len/2"), "the formula typed for b_len was not kept");
    assert!((solved_height(&mut r) - 20.0).abs() < 1e-6);
    assert_eq!(r.p.set_dim_target_text(&r.width.clone(), "60"), Ok(true));
    assert!((solved_height(&mut r) - 30.0).abs() < 1e-6, "the width went to 60 and b_len = a_len/2 stayed at {}", solved_height(&mut r));
}

#[test]
fn a_number_typed_over_a_formula_makes_a_constant() {
    let mut r = rectangle();
    assert_eq!(r.p.set_dim_target_text(&r.height.clone(), "a_len/2"), Ok(true));
    assert_eq!(r.p.set_dim_target_text(&r.height.clone(), "25"), Ok(true));
    assert_eq!(r.p.dim_target_expr(&r.height), None, "the formula stayed after a number was typed over it");
    assert_eq!(r.p.set_dim_target_text(&r.width.clone(), "60"), Ok(true));
    assert!((solved_height(&mut r) - 25.0).abs() < 1e-6, "b_len typed as 25 followed the width to {}", solved_height(&mut r));
}

/// A FORMULA THAT COMES BACK TO ITS OWN NAME IS REFUSED, directly or through another name, and the document stays as it
/// was. Taken, `b_len = b_len + 1` would add one on every evaluation.
#[test]
fn a_formula_that_comes_back_to_its_own_name_is_refused() {
    let mut r = rectangle();
    let value_before = r.p.dim_target_value(&r.height);
    assert_eq!(r.p.set_dim_target_text(&r.height.clone(), "b_len + 1"), Err(ExprError::LeansOnItself("b_len -> b_len".into())));
    assert_eq!(r.p.set_dim_target_text(&r.width.clone(), "b_len*2"), Ok(true));
    assert_eq!(r.p.set_dim_target_text(&r.height.clone(), "a_len/2"), Err(ExprError::LeansOnItself("b_len -> a_len -> b_len".into())), "b_len = a_len/2 with a_len = b_len*2 was taken");
    assert_eq!(r.p.dim_target_value(&r.height), value_before, "a refused formula changed the value of b_len");
    assert_eq!(r.p.dim_target_expr(&r.height), None, "a refused formula was written into b_len");
    assert!(matches!(r.p.set_dim_target_text(&r.height.clone(), "no_such/2"), Err(ExprError::UnknownName(_))), "a formula with no value was taken");
}

/// A FEATURE'S NAMED NUMBER WRITTEN AS A FORMULA HAS A VALUE when the formula reads a sketch dimension: the rebuild reads
/// it so, and the table and other formulas read it the same way.
#[test]
fn a_feature_number_written_through_a_dimension_has_a_value() {
    let mut r = rectangle();
    r.p.set_feat_dim(r.extrude, "height", "a_len/4".into());
    assert!(r.p.add_named_feat_dim("depth".into(), r.extrude, "height"));
    let depth = DimTarget::Feature { node: r.extrude, key: "height".into() };
    assert_eq!(r.p.dim_target_value(&depth), Some(10.0), "depth = a_len/4 has no value in the table");
    assert_eq!(r.p.eval_expr("depth*2").ok(), Some(20.0), "depth = a_len/4 cannot be read by another formula");
}

/// A FORMULA TYPED FOR A FEATURE'S NUMBER STAYS A FORMULA as well.
#[test]
fn a_formula_typed_for_a_feature_number_stays() {
    let mut r = rectangle();
    assert!(r.p.add_named_feat_dim("depth".into(), r.extrude, "height"));
    let depth = DimTarget::Feature { node: r.extrude, key: "height".into() };
    assert_eq!(r.p.set_dim_target_text(&depth, "a_len/4"), Ok(true));
    assert_eq!(r.p.feat_dim(r.extrude, "height"), Some("a_len/4"), "the formula typed for depth became a number");
    assert_eq!(r.p.dim_target_value(&depth), Some(10.0));
}

/// A NAME THAT WOULD CLOSE A CIRCLE IS REFUSED to a dimension, renamed or named for the first time: its formula reads `q`,
/// and `q = x*2` reads the name it would take.
#[test]
fn naming_a_dimension_into_a_circle_is_refused() {
    let mut r = rectangle();
    r.p.parameters.push(qymcad_core::model::Param { name: "q".into(), expr: "x*2".into(), value: 0.0 });
    // the height written as q/2 while q reads a name nobody holds
    if let Some(e) = r.p.sketches[r.si].constraints.last_mut().and_then(Constraint::expr_mut) {
        *e = "q/2".into();
    }
    let renamed = r.p.rename_driver("b_len", "x");
    assert!(matches!(&renamed, Err(qymcad_core::drivers::RenameError::LeansOnItself(chain)) if chain == "x -> q -> x"), "renaming b_len to x closed the circle: {renamed:?}");
    r.p.remove_named_dim("b_len");
    assert_eq!(r.p.naming_closes_circle(&r.height, "x").as_deref(), Some("x -> q -> x"));
    assert!(!r.p.name_dim("x".into(), r.height.clone()), "naming the height x closed the circle");
}
