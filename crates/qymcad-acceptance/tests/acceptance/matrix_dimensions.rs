//! THE RADIUS AND DIAMETER OF A SKETCH ON FIGURES AS THE TOOLS DRAW THEM - the auto constraints on, as a person has
//! them. A circle and an arc come with no size until one is typed: the tool puts a diameter on the circle and a radius
//! on the arc, one more constraint and one freedom less. A circle whose diameter was deleted gets it back. Whatever is
//! typed must be what the figure then measures.
use qymcad::{Key, Session, SketchPick};
use qymcad_acceptance::build::{circle, draw, into_the_first_part};
use qymcad_acceptance::probe;

/// A sketch on XY of the first part with the auto constraints as they come, drawn by `draw`.
fn sketched(s: &mut Session, draw: impl FnOnce(&mut Session)) {
    into_the_first_part(s);
    let xy = s.word("plane-xy-table");
    s.press_word(&xy);
    draw(s);
}

/// The radius tool clicked on the sheet at (x, y), `value` typed into the field it opens, Enter.
fn dimension(s: &mut Session, x: f64, y: f64, value: &str) {
    let hint = s.word("tb-dim-radius-hint");
    s.press_hint(&hint);
    s.click_on_sketch(x, y);
    // a dimension's field shows an example expression while empty; the arc's own radius opens a field holding its
    // number - either is the field a person types into
    let example = s.word("sk-expr-example");
    let field = s.widgets().into_iter().find(|w| w.placeholder == example || (w.kind == qymcad::Kind::TextField && w.value.trim().parse::<f64>().is_ok())).unwrap_or_else(|| {
        let fields: Vec<(String, String)> = s.widgets().into_iter().filter(|w| w.kind == qymcad::Kind::TextField).map(|w| (w.placeholder, w.value)).collect();
        panic!("the radius opens no field; the status line says {:?}; the fields on screen: {fields:?}", s.status())
    });
    s.click(field.rect.center()).chord(qymcad::Modifiers::COMMAND, Key::A).type_text(value).key(Key::Enter);
}

/// The radius of the circle or arc at (x, y) of the sheet.
fn radius_at(s: &mut Session, x: f64, y: f64) -> Option<f64> {
    match s.sketch_under(x, y) {
        Some(SketchPick::Circle { radius, .. }) => Some(radius),
        Some(SketchPick::Arc { centre, from, .. }) => Some((from.0 - centre.0).hypot(from.1 - centre.1)),
        _ => None,
    }
}

/// The count of constraints and the freedoms left of the open sketch.
fn held(s: &mut Session) -> (usize, i32, i32) {
    let sk = s.document().sketches[0].clone();
    (sk.constraints, sk.dof, sk.redundant)
}

probe! {
    /// A CIRCLE AS THE TOOL DRAWS IT, its field left with Esc, carries no size: the radius tool puts its diameter on -
    /// 30 typed makes the circle 30 across, one constraint more, one freedom less, nothing over-defined; the same tool
    /// again changes that diameter and puts no second one.
    fn the_radius_tool_puts_a_diameter_on_a_circle() {
        let mut s = Session::start();
        sketched(&mut s, |s| circle(s, (0.0, 0.0), (10.0, 0.0)));
        let before = held(&mut s);
        dimension(&mut s, 10.0, 0.0, "30");
        let after = held(&mut s);
        let r = radius_at(&mut s, 15.0, 0.0).or_else(|| radius_at(&mut s, 10.0, 0.0));
        assert!(r.is_some_and(|r| (r - 15.0).abs() < 1e-6), "the diameter typed as 30 left the circle of radius {r:?}");
        assert!(after.0 == before.0 + 1 && after.1 == before.1 - 1 && after.2 == 0, "the diameter did not go on the circle as a dimension: constraints, freedoms, redundant {before:?} became {after:?}");
        // the tool stays in hand after it is applied, and its button pressed again would put it down: Esc first
        s.key(Key::Escape);
        dimension(&mut s, 15.0, 0.0, "40");
        let again = held(&mut s);
        let r = radius_at(&mut s, 20.0, 0.0).or_else(|| radius_at(&mut s, 15.0, 0.0));
        assert!(r.is_some_and(|r| (r - 20.0).abs() < 1e-6) && again == after, "the diameter changed to 40: the circle of radius {r:?}, constraints, freedoms, redundant {after:?} became {again:?}");
    }
}

probe! {
    /// AN ARC AS THE TOOL DRAWS IT carries no radius: the radius tool puts one on it - the arc is 15 in radius after
    /// 15 is typed, the sketch holds one constraint more and one freedom less, and nothing is over-defined.
    fn the_radius_tool_puts_a_radius_on_an_arc() {
        let mut s = Session::start();
        sketched(&mut s, |s| draw(s, "tb-arc-hint", &[(0.0, 0.0), (10.0, 0.0), (0.0, 10.0)]));
        let before = held(&mut s);
        dimension(&mut s, 7.071, 7.071, "15");
        let after = held(&mut s);
        let r = radius_at(&mut s, 10.607, 10.607).or_else(|| radius_at(&mut s, 7.071, 7.071));
        assert!(r.is_some_and(|r| (r - 15.0).abs() < 1e-6), "the radius typed as 15 left the arc of radius {r:?}");
        assert!(after.0 == before.0 + 1 && after.1 == before.1 - 1 && after.2 == 0, "the radius did not go on the arc as a dimension: constraints, freedoms, redundant {before:?} became {after:?}");
    }
}

probe! {
    /// A CIRCLE WHOSE DIAMETER WAS DELETED gets it back from the radius tool: the dimension clicked and deleted, the
    /// radius tool on the circle, 24 typed - the circle 24 across, and the dimension there again.
    fn a_deleted_diameter_comes_back_with_the_radius_tool() {
        let mut s = Session::start();
        sketched(&mut s, |s| circle(s, (0.0, 0.0), (10.0, 0.0)));
        dimension(&mut s, 10.0, 0.0, "20");
        s.key(Key::Escape).key(Key::Escape);
        let with = held(&mut s);
        let canvas = s.canvas();
        let label = s.words_at().into_iter().find(|(w, r)| canvas.contains(r.center()) && w.contains("20")).map(|(_, r)| r.center());
        let label = label.unwrap_or_else(|| panic!("the diameter of 20 is not written on the sheet; on screen: {:?}", s.words()));
        s.click(label).key(Key::Delete);
        let without = held(&mut s);
        assert!(without.0 + 1 == with.0, "the diameter was not deleted: constraints {} became {}", with.0, without.0);
        dimension(&mut s, 10.0, 0.0, "24");
        let again = held(&mut s);
        let r = radius_at(&mut s, 12.0, 0.0).or_else(|| radius_at(&mut s, 10.0, 0.0));
        assert!(r.is_some_and(|r| (r - 12.0).abs() < 1e-6), "the diameter typed as 24 left the circle of radius {r:?}");
        assert!(again == with, "the diameter did not come back as it was: constraints, freedoms, redundant {with:?} before, {again:?} now");
    }
}

probe! {
    /// A SKETCH BROUGHT IN FROM A DXF FILE carries no dimensions: the plate of the examples - a rectangle of 50 by 30 and
    /// a circle of radius 6 in its middle - imported into the first part and put on the table by a click, which opens
    /// its sketch; the radius tool on the circle, 20 typed: the circle is 20 across, one constraint more, one freedom
    /// less.
    fn the_radius_tool_dimensions_a_circle_brought_in_from_a_file() {
        let mut s = Session::start();
        into_the_first_part(&mut s);
        let (file, import) = (s.word("menu-file"), s.word("file-import"));
        s.menu(&[&file, &import]);
        s.answer_file(format!("{}/../../examples/plate.dxf", env!("CARGO_MANIFEST_DIR")));
        let table = s.in_space([0.0, 0.0, 0.0]);
        s.click(table);
        let sk = s.document().sketches.last().cloned().unwrap_or_else(|| panic!("the drawing brought in no sketch; the program says {:?}", s.status()));
        assert!(s.document().editing.as_deref() == Some(sk.name.as_str()), "the sketch of the drawing did not open: {:?}", s.document().editing);
        // the rectangle's corner stands where the drawing was put; the circle is 25 and 15 from it
        let (x, y) = (sk.min[0] + 25.0, sk.min[1] + 15.0);
        let before = (sk.constraints, sk.dof, sk.redundant);
        dimension(&mut s, x + 6.0, y, "20");
        let sk = s.document().sketches.last().cloned().unwrap_or_else(|| panic!("no sketch"));
        let after = (sk.constraints, sk.dof, sk.redundant);
        let r = radius_at(&mut s, x + 10.0, y).or_else(|| radius_at(&mut s, x + 6.0, y));
        assert!(r.is_some_and(|r| (r - 10.0).abs() < 1e-6), "the diameter typed as 20 left the circle of radius {r:?}");
        assert!(after.0 == before.0 + 1 && after.1 == before.1 - 1 && after.2 == 0, "the diameter did not go on the circle as a dimension: constraints, freedoms, redundant {before:?} became {after:?}");
    }
}

probe! {
    /// A LENGTH ON A RECTANGLE AS THE TOOL DRAWS IT - its sides level and upright, no size of its own: the bottom
    /// clicked with the dimension tool, the dimension put below it, 50 typed - the bottom is 50 long, the top follows
    /// it, one constraint more and one freedom less, nothing over-defined.
    fn a_length_on_a_rectangle_as_drawn() {
        let mut s = Session::start();
        sketched(&mut s, |s| draw(s, "tb-rect-hint", &[(0.0, 0.0), (40.0, 30.0)]));
        let before = held(&mut s);
        let hint = s.word("tb-dim-hint");
        s.press_hint(&hint);
        s.click_on_sketch(20.0, 0.0).click_on_sketch(20.0, -10.0);
        let example = s.word("sk-expr-example");
        let field = s.widgets().into_iter().find(|w| w.placeholder == example).unwrap_or_else(|| panic!("the length opens no field; the status line says {:?}", s.status()));
        s.click(field.rect.center()).chord(qymcad::Modifiers::COMMAND, Key::A).type_text("50").key(Key::Enter);
        let after = held(&mut s);
        let sk = s.document().sketches[0].clone();
        let width = sk.max[0] - sk.min[0];
        assert!((width - 50.0).abs() < 1e-6, "the length typed as 50 left the rectangle {width} wide");
        assert!(after.0 == before.0 + 1 && after.1 == before.1 - 1 && after.2 == 0, "the length did not go on as a dimension: constraints, freedoms, redundant {before:?} became {after:?}");
    }
}
