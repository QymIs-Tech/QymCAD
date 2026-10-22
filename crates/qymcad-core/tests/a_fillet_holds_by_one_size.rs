//! A FILLET HOLDS BY ONE SIZE, and its arc says so: sized by its radius, by its chord or by the length of its arc, it
//! carries that size and no other, and `Sketch::rim_sized` knows it - so the sheet labels no passive radius beside a
//! fillet held by its chord or its arc length. A bare arc carries none.
//!
//! Reported behaviour: a fillet taken by its chord or by its arc length showed an R beside it as well.
use qymcad_core::feature::Purpose;
use qymcad_core::model::{FilletBy, FilletSize, Project};

/// An L of two lines meeting at (20, 0), its corner rounded by `size`. Answers the project, the sketch and the centre
/// of the arc.
fn rounded(size: FilletSize) -> (Project, usize, u64) {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    p.add_line_entity(si, 0.0, 0.0, 20.0, 0.0, Purpose::Real);
    p.add_line_entity(si, 20.0, 0.0, 20.0, 20.0, Purpose::Real);
    p.regen_sketch(si);
    let corner = p.sketches[si].points.iter().find(|q| (q.x - 20.0).abs() < 1e-9 && q.y.abs() < 1e-9).map(|q| q.id).expect("the corner");
    assert!(p.fillet_at_vertex_by(si, corner, size), "the corner was not rounded by {size:?}");
    let centre = p.sketches[si].entities.iter().find_map(|e| match e.kind.as_arc() {
        Some(qymcad_core::model::ArcParts { center, .. }) => Some(center),
        _ => None,
    });
    (p, si, centre.expect("the arc of the fillet"))
}

#[test]
fn a_fillet_by_any_size_is_sized_and_a_bare_arc_is_not() {
    for by in [FilletBy::Radius, FilletBy::Chord, FilletBy::ArcLength] {
        let (p, si, centre) = rounded(FilletSize { by, value: 3.0 });
        assert!(p.sketches[si].rim_sized(centre), "a fillet by {by:?} is not known to carry its size");
    }
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    let pt = qymcad_core::geom::Point2::new;
    p.add_arc_entity(si, pt(0.0, 0.0), pt(10.0, 0.0), pt(0.0, 10.0), qymcad_core::feature::Winding::Ccw, Purpose::Real);
    let centre = p.sketches[si].entities.iter().find_map(|e| match e.kind.as_arc() {
        Some(qymcad_core::model::ArcParts { center, .. }) => Some(center),
        _ => None,
    });
    assert!(!p.sketches[si].rim_sized(centre.expect("the arc")), "a bare arc is taken for one that carries a size");
}
