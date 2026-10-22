//! WHAT BECOMES OF THE POINT OF A CORNER, kept in one place.
//!
//! It stays, after a fillet as after a chamfer, as the VIRTUAL SHARP on the extensions of both shortened lines - drawn
//! and picked like any point, so a dimension or a constraint can be measured to the corner at any time. A dimension
//! measured to it before the cut stays on it.
//!
//! And where the rest of the drawing still needs the point - a third line at a T, or the two lines of the far square
//! at the point two squares share - it stays as it is.
use qymcad_core::feature::Purpose;
use qymcad_core::model::{entity_points, ChamferLegs, Constraint, Project};

/// An L of two lines meeting at (20, 0), with the point of the corner and the far end of the bottom line.
fn an_angle() -> (Project, usize, u64, u64, u64) {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    p.add_line_entity(si, 0.0, 0.0, 20.0, 0.0, Purpose::Real);
    p.add_line_entity(si, 20.0, 0.0, 20.0, 20.0, Purpose::Real);
    p.regen_sketch(si);
    let corner = p.sketches[si].points.iter().find(|q| (q.x - 20.0).abs() < 1e-6 && q.y.abs() < 1e-6).map(|q| q.id).expect("the corner");
    let far = p.sketches[si].points.iter().find(|q| q.x.abs() < 1e-6 && q.y.abs() < 1e-6).map(|q| q.id).expect("the far end");
    (p, si, corner, far, 0)
}

/// The points of the sketch that no entity and no spline stands on.
fn loose_points(p: &Project, si: usize) -> Vec<(f64, f64)> {
    let used: std::collections::HashSet<u64> = p.sketches[si].entities.iter().flat_map(entity_points).collect();
    p.sketches[si].points.iter().filter(|q| !used.contains(&q.id)).map(|q| (q.x, q.y)).collect()
}

#[test]
fn a_rounded_corner_keeps_its_point_as_the_virtual_sharp() {
    // a fillet keeps the corner as a chamfer does: on the extensions of both shortened lines
    let (mut p, si, corner, _far, _) = an_angle();
    assert!(p.fillet_at_vertex(si, corner, 5.0), "the corner was not rounded");
    assert!(p.sketches[si].points.iter().any(|q| q.id == corner), "the point of a rounded corner went with the corner");
    assert_eq!(p.sketches[si].constraints.iter().filter(|c| matches!(c, Constraint::PointOnLine { p, .. } if *p == corner)).count(), 2, "the virtual sharp of a fillet is not held on both lines");
    assert!(!p.sketches[si].unseen_points().contains(&corner), "the virtual sharp of a fillet is hidden from the drawing");
}

#[test]
fn a_chamfered_corner_keeps_the_sharp_its_legs_are_measured_from() {
    // a chamfer measures its legs from the sharp corner, so the point stays as the virtual sharp: taken away, the
    // legs would have nothing to be measured from and the size could not be changed afterwards
    let (mut p, si, corner, _far, _) = an_angle();
    assert!(p.chamfer_at_vertex(si, corner, ChamferLegs::equal(5.0), None), "the corner was not cut");
    assert!(p.sketches[si].points.iter().any(|q| q.id == corner), "the point the chamfer measures its legs from was taken away");
    assert!(p.constraints_on_point(si, corner) > 0, "the sharp is there but nothing is stated about it any more");
    // and it stands on the extensions of both shortened lines rather than on any of them
    assert!(
        p.sketches[si].constraints.iter().filter(|c| matches!(c, Constraint::PointOnLine { p, .. } if *p == corner)).count() == 2,
        "the virtual sharp is not held on both lines: {:?}",
        p.sketches[si].constraints.iter().filter(|c| c.points().contains(&corner)).collect::<Vec<_>>()
    );
}

#[test]
fn a_dimension_on_the_rounded_corner_keeps_the_point_as_the_virtual_sharp() {
    let (mut p, si, corner, far, _) = an_angle();
    p.sketches[si].constraints.push(Constraint::Distance { a: corner, b: far, d: 20.0, off: 0.0, expr: String::new(), driven: false, axis: 0, at: None });
    p.solve_sketch(si);
    assert!(p.fillet_at_vertex(si, corner, 5.0), "the corner was not rounded");
    assert!(p.sketches[si].points.iter().any(|q| q.id == corner), "the point the dimension is measured to went with the corner");
    assert!(p.sketches[si].constraints.iter().any(|c| matches!(c, Constraint::Distance { a, b, .. } if *a == corner && *b == far)), "the dimension measured to the corner was taken away with it");
    assert_eq!(p.sketches[si].constraints.iter().filter(|c| matches!(c, Constraint::PointOnLine { p, .. } if *p == corner)).count(), 2, "the virtual sharp is not held on both lines");
    // the dimension still says 20 from the far end to the sharp, at the radius it was rounded with and at another
    let at = |p: &Project, id: u64| p.sketches[si].points.iter().find(|q| q.id == id).map(|q| (q.x, q.y)).expect("the point");
    let span = |p: &Project| (at(p, corner).0 - at(p, far).0).hypot(at(p, corner).1 - at(p, far).1);
    assert!((span(&p) - 20.0).abs() < 1e-6, "the sharp stands {:.4} from the far end, not 20", span(&p));
    for c in &mut p.sketches[si].constraints {
        if let Constraint::Diameter { d, .. } = c {
            *d = 8.0;
        }
    }
    let r = p.solve_sketch(si);
    assert!(r < 1e-6 && (span(&p) - 20.0).abs() < 1e-6, "after the radius went to 8 the sharp stands {:.4} from the far end, residual {r:.2e}", span(&p));
}

#[test]
fn the_point_stays_while_a_line_still_comes_to_it() {
    // a T: three lines meet at (20, 0), and the corner is taken off two of them
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    p.add_line_entity(si, 0.0, 0.0, 20.0, 0.0, Purpose::Real);
    p.add_line_entity(si, 20.0, 0.0, 20.0, 20.0, Purpose::Real);
    p.add_line_entity(si, 20.0, 0.0, 40.0, 0.0, Purpose::Real);
    p.regen_sketch(si);
    let corner = p.sketches[si].points.iter().find(|q| (q.x - 20.0).abs() < 1e-6 && q.y.abs() < 1e-6).map(|q| q.id).expect("the point of the T");
    let towards = |p: &Project, to: (f64, f64)| -> u64 {
        p.vertex_edges(si, corner)
            .into_iter()
            .find(|&e| {
                let Some(qymcad_core::model::LineParts { a, b }) = p.sketches[si].entities.iter().find(|x| x.id == e).and_then(|x| x.kind.as_line()) else { return false };
                let other = if a == corner { b } else { a };
                p.sketches[si].points.iter().any(|q| q.id == other && (q.x - to.0).abs() < 1e-6 && (q.y - to.1).abs() < 1e-6)
            })
            .unwrap_or_else(|| panic!("no line at the point of the T runs to {to:?}"))
    };
    let (up, left) = (towards(&p, (20.0, 20.0)), towards(&p, (0.0, 0.0)));
    assert!(p.chamfer_lines_of_pair(si, (up, left), ChamferLegs::equal(5.0), None), "the corner of the T was not cut");
    assert!(p.sketches[si].points.iter().any(|q| q.id == corner), "the point of the T went although the third line still comes to it");
    assert!(loose_points(&p, si).is_empty(), "a point drawing nothing is left behind: {:?}", loose_points(&p, si));
}
