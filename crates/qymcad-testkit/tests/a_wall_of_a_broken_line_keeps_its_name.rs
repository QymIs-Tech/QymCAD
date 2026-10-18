//! A WALL OF A LINE BROKEN IN TWO KEEPS ITS NAME. A line of a sketch broken at its middle is two lines along one
//! segment: the contour draws the same outline, the extrusion makes the same wall, and the wall is the wall of the line
//! that was there. Measured on the case of the examples: breaking the first line of each of its 32 sketches renamed 59
//! faces that did not move, the wall named after the new half, and every rounding and sketch on it went with the name.
use qymcad_core::feature::{Purpose, Reach};
use qymcad_core::model::{EntityKind, Project};
use qymcad_core::names::Role;

#[test]
fn a_wall_of_a_line_broken_in_two_keeps_its_name() {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("s");
    let sketch = p.sketches[si].id;
    p.add_sketch_node(sketch, "s");
    p.add_rect_entity(si, 0.0, 0.0, 20.0, 10.0, Purpose::Real);
    p.regen_sketch(si);
    let contour = p.sketches[si].contour_ids.iter().copied().find(|c| p.contour_profile_xy(*c).is_some()).expect("a closed contour");
    let block = p.add_extrude_multi(sketch, vec![contour], 5.0, Reach::Forward, 0.0, vec![]);
    let (report, _) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "the block does not build: {:?}", report.errors);
    // the wall at y = 0, swept from the bottom line of the rectangle
    let bottom = |p: &Project| p.regen_faces[&block].iter().filter(|f| f.normal[1] < -0.99).map(|f| f.id).collect::<Vec<u32>>();
    let was = bottom(&p);
    assert_eq!(was.len(), 1, "the bottom wall is not one face: {}", was.len());
    let line = p.sketches[si]
        .entities
        .iter()
        .find(|e| match e.kind {
            EntityKind::Line { a, b } => [a, b].iter().all(|k| p.point_xy(si, *k).is_some_and(|q| q.1.abs() < 1e-9)),
            _ => false,
        })
        .map(|e| e.id)
        .expect("the bottom line");
    assert!(p.break_line(si, line, 10.0, 0.0), "the line did not break");
    p.mark_sketch_dirty(sketch);
    let (report, _) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "the block with a broken line does not build: {:?}", report.errors);
    let now = bottom(&p);
    let name = |d: u32| p.names.get(d);
    assert!(
        now.len() == 1 && name(now[0]).is_some_and(|n| n.role == Role::Wall && n.src == line) && now == was,
        "the bottom wall was {}, is {}",
        p.names.describe(was[0]),
        now.iter().map(|d| p.names.describe(*d)).collect::<Vec<_>>().join(", ")
    );
}
