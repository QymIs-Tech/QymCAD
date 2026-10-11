//! AN OFFSET OF A CHAIN OF LINES AND ARCS IS HELD TO ITS SOURCE, as an offset of a circle is: each line of the copy
//! parallel to its own at the distance, each arc concentric with its own, its radius the source's plus the distance,
//! and an arc the offset rounds a sharp corner with centred on that corner. The pair has the freedoms of the source
//! alone, and changing the source changes the copy.
//!
//! Reported behaviour: the copy of a rectangle at a distance of 3 was lines of their own - resizing the rectangle
//! left the copy where it was.
use qymcad_core::feature::Purpose;
use qymcad_core::model::{Constraint, EntityKind, Project};

/// A rectangle 30 x 20 with its width and height dimensioned: two freedoms left, where it stands. Answers the sketch,
/// its lines and the two points of its bottom edge (the width).
fn a_dimensioned_rectangle(p: &mut Project) -> (usize, Vec<u64>, (u64, u64)) {
    p.new_document();
    let si = p.new_sketch("S");
    let lines = p.add_rect_entity(si, 0.0, 0.0, 30.0, 20.0, Purpose::Real);
    let ends = |p: &Project, e: u64| match p.sketches[si].entities.iter().find(|x| x.id == e).map(|x| x.kind) {
        Some(EntityKind::Line { a, b }) => (a, b),
        _ => panic!("a line"),
    };
    let (a0, b0) = ends(p, lines[0]);
    let (a1, b1) = ends(p, lines[1]);
    let width = Constraint::Distance { dimension: Default::default(), a: a0, b: b0, d: 30.0, off: 0.0, expr: String::new(), driven: false, axis: 0, at: None };
    let height = Constraint::Distance { dimension: Default::default(), a: a1, b: b1, d: 20.0, off: 0.0, expr: String::new(), driven: false, axis: 0, at: None };
    p.sketches[si].constraints.push(width);
    p.sketches[si].constraints.push(height);
    p.regen_sketch(si);
    p.solve_sketch(si);
    (si, lines, (a0, b0))
}

/// The width of the copy: the span in x of the points of the lines not in `source` - and not the construction
/// diagonals of the source rectangle, which are no part of the copy.
fn copy_width(p: &Project, si: usize, source: &[u64]) -> f64 {
    let s = &p.sketches[si];
    let xs: Vec<f64> = s
        .entities
        .iter()
        .filter(|e| !source.contains(&e.id) && !e.construction)
        .filter_map(|e| match e.kind {
            EntityKind::Line { a, b } => Some([a, b]),
            _ => None,
        })
        .flatten()
        .filter_map(|id| s.points.iter().find(|q| q.id == id).map(|q| q.x))
        .collect();
    xs.iter().fold(f64::MIN, |m, x| m.max(*x)) - xs.iter().fold(f64::MAX, |m, x| m.min(*x))
}

fn set_width(p: &mut Project, si: usize, (a, b): (u64, u64), w: f64) {
    for c in p.sketches[si].constraints.iter_mut() {
        if let Constraint::Distance { a: ca, b: cb, d, .. } = c {
            if (*ca, *cb) == (a, b) {
                *d = w;
            }
        }
    }
    p.solve_sketch(si);
}

#[test]
fn an_offset_rectangle_follows_its_source() {
    for dist in [3.0, -3.0] {
        let mut p = Project::default();
        let (si, lines, width) = a_dimensioned_rectangle(&mut p);
        let (before, _) = p.sketch_dof(si);
        assert_eq!(before, 2, "setup: a dimensioned rectangle has the two freedoms of where it stands");
        assert_eq!(p.offset_entities(si, &lines, dist), 1, "one copy at {dist}");
        p.solve_sketch(si);
        let (after, _) = p.sketch_dof(si);
        assert_eq!(after, before, "the copy at {dist} brings freedoms of its own: {before} -> {after}");
        // the copy is 36 or 24 wide, as the side the offset went to: 6 either way from the source
        let was = copy_width(&p, si, &lines);
        assert!((was - 36.0).abs() < 1e-6 || (was - 24.0).abs() < 1e-6, "setup: the copy at {dist} is {was} wide");
        set_width(&mut p, si, width, 40.0);
        let now = copy_width(&p, si, &lines);
        assert!((now - was - 10.0).abs() < 1e-6, "the source made 10 wider, the copy at {dist} went from {was} to {now}");
    }
}

/// THE ROUNDED ONE: its arc's copy concentric and the radius apart by the offset, a sharp corner the offset rounds
/// centred on that corner.
///
/// The corner rounded here is the one NO dimension was measured to: the point of a corner goes with the corner, and a
/// dimension measured to it cannot be stated without its subject, so it goes too. Rounding the top left corner of a
/// dimensioned rectangle therefore leaves both of its edge dimensions standing, and the copy can be asked to follow
/// the source afterwards — which is the whole point of this chain.
#[test]
fn an_offset_rounded_rectangle_follows_its_source() {
    for dist in [3.0, -3.0] {
        let mut p = Project::default();
        let (si, lines, width) = a_dimensioned_rectangle(&mut p);
        assert!(p.fillet_lines(si, lines[2], lines[3], 4.0), "setup: the top left corner rounded");
        p.solve_sketch(si);
        let chain: Vec<u64> = p.sketches[si].entities.iter().filter(|e| !e.construction).map(|e| e.id).collect();
        assert_eq!(p.offset_entities(si, &chain, dist), 1, "one copy at {dist}");
        p.solve_sketch(si);
        let (after, redundant) = p.sketch_dof(si);
        assert_eq!((after, redundant), (2, 0), "the copy at {dist} of the rounded rectangle: {after} freedoms, {redundant} redundant, not the two of where the dimensioned rectangle stands");
        let was = copy_width(&p, si, &chain);
        set_width(&mut p, si, width, 40.0);
        let now = copy_width(&p, si, &chain);
        assert!((now - was - 10.0).abs() < 1e-6, "the source made 10 wider, the copy at {dist} went from {was} to {now}");
        let s = &p.sketches[si];
        let at = |id: u64| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y)).expect("a point");
        let arcs: Vec<((f64, f64), f64)> = s
            .entities
            .iter()
            .filter_map(|e| match e.kind {
                EntityKind::Arc { center, a, .. } => Some((at(center), (at(center).0 - at(a).0).hypot(at(center).1 - at(a).1))),
                _ => None,
            })
            .collect();
        let source = arcs.iter().find(|(_, r)| (r - 4.0).abs() < 1e-6).expect("the source arc is still R4");
        let copy = arcs.iter().find(|(c, r)| (c.0 - source.0 .0).hypot(c.1 - source.0 .1) < 1e-6 && (r - 4.0).abs() > 1e-6).expect("a copy arc concentric with the source");
        assert!((copy.1 - 1.0).abs() < 1e-6 || (copy.1 - 7.0).abs() < 1e-6, "the rounded corner R4 offset by 3 is R1 or R7 by the side, not {}: {arcs:?}", copy.1);
    }
}
