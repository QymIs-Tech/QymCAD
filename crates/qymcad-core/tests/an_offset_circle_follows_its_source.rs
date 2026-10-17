//! AN OFFSET OF A CIRCLE IS HELD TO ITS SOURCE: concentric with it, its radius the source's plus the distance - the
//! pair has the freedoms of the source alone, and resizing the source resizes the copy.
//!
//! Reported behaviour: the copy of a circle at a distance of 3 was a circle of its own, with a centre of its own - five
//! freedoms instead of two, and moving or changing the source left the copy where it was.
use qymcad_core::feature::Purpose;
use qymcad_core::model::{Constraint, EntityKind, Project};

fn radii(p: &Project, si: usize) -> Vec<f64> {
    let mut r: Vec<f64> = p.sketches[si]
        .entities
        .iter()
        .filter_map(|e| match e.kind {
            EntityKind::Circle { r, .. } => Some(r),
            _ => None,
        })
        .collect();
    r.sort_by(|a, b| a.total_cmp(b));
    r
}

#[test]
fn an_offset_circle_is_concentric_and_follows_the_source() {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    let c = p.add_circle_entity(si, 5.0, 5.0, 10.0, Purpose::Real);
    let centre = match p.sketches[si].entities.iter().find(|e| e.id == c).map(|e| e.kind) {
        Some(EntityKind::Circle { center, .. }) => center,
        _ => panic!("the circle"),
    };
    p.sketches[si].constraints.push(Constraint::Diameter { dimension: Default::default(), c: centre, d: 20.0, off: 0.0, expr: String::new(), driven: false, diam: true, at: None });
    p.regen_sketch(si);
    assert_eq!(p.offset_entities(si, &[c], 3.0), 1, "one copy");
    p.solve_sketch(si);
    let (dof, _) = p.sketch_dof(si);
    assert_eq!(dof, 2, "the source with its diameter and the copy held to it have the two freedoms of the centre");
    for con in p.sketches[si].constraints.iter_mut() {
        if let Constraint::Diameter { d, .. } = con {
            *d = 24.0;
        }
    }
    p.solve_sketch(si);
    let r = radii(&p, si);
    assert!((r[0] - 12.0).abs() < 1e-6 && (r[1] - 15.0).abs() < 1e-6, "the source made R12, the copy stands at {r:?}, not 15");
}
