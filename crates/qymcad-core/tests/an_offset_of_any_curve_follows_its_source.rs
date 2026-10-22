//! AN OFFSET OF ANY CURVE FOLLOWS ITS SOURCE: an ellipse, an arc of one, an open and a closed spline offset by 3, then
//! the source edited - a point of it dragged - and the offset stands 3 from it again; its dimension set to 5, and it
//! stands 5 from it - every place of the copy at the distance within the tolerance of an offset.
//!
//! Reported behaviour: the offset of an ellipse or a spline was a free spline - the source edited, the offset stayed
//! where it was.
use qymcad_core::feature::{Ends, Purpose, Winding};
use qymcad_core::geom::Point2;
use qymcad_core::model::curve::{Curve, OFFSET_TOLERANCE};
use qymcad_core::model::{Constraint, EllipseArcSpec, Id, Project};

fn p2(at: [f64; 2]) -> Point2 {
    Point2::new(at[0], at[1])
}

/// A SOURCE OF AN OFFSET: what it is, how it is laid, and the point of it the edit drags with where it goes.
struct Source {
    name: &'static str,
    lay: fn(&mut Project, usize) -> Id,
    drag: fn(&Project, usize, Id) -> Option<Dragged>,
}

/// A POINT DRAGGED and where it is taken.
struct Dragged {
    point: Id,
    to: Point2,
}

fn ellipse(p: &mut Project, si: usize) -> Id {
    // the call answers the centre of the ellipse; the curve is the one laid
    p.add_ellipse_entity(si, p2([0.0, 0.0]), 12.0, 6.0, 0.0, Purpose::Real);
    p.sketches[si].entities.iter().find(|e| e.kind.as_ellipse().is_some()).map(|e| e.id).expect("the ellipse")
}
fn ellipse_arc(p: &mut Project, si: usize) -> Id {
    p.add_ellipse_arc_entity(si, EllipseArcSpec { centre: p2([0.0, 0.0]), radii: p2([12.0, 6.0]), rot: 0.0, angles: [0.3, 2.6], winding: Winding::Ccw }, Purpose::Real)
}
fn open_spline(p: &mut Project, si: usize) -> Id {
    p.add_spline(si, vec![p2([0.0, 0.0]), p2([10.0, 4.0]), p2([20.0, 2.0]), p2([30.0, 6.0])], Ends::Open, Purpose::Real).expect("the spline")
}
fn closed_spline(p: &mut Project, si: usize) -> Id {
    p.add_spline(si, vec![p2([0.0, 0.0]), p2([20.0, -2.0]), p2([24.0, 14.0]), p2([2.0, 16.0])], Ends::Closed, Purpose::Real).expect("the spline")
}

/// The end of the major axis of the ellipse of `eid`, taken 4 further out.
fn major_out(p: &Project, si: usize, eid: Id) -> Option<Dragged> {
    let s = &p.sketches[si];
    let ma = s.entity(eid)?.kind.as_ellipse()?.ma;
    let q = s.point(ma)?;
    Some(Dragged { point: ma, to: p2([q.x + 4.0, q.y]) })
}
/// The second node of the spline of `eid`, taken 3 up.
fn node_up(p: &Project, si: usize, eid: Id) -> Option<Dragged> {
    let s = &p.sketches[si];
    let node = s.entity(eid)?.kind.as_spline()?.nodes[1];
    let q = s.point(node)?;
    Some(Dragged { point: node, to: p2([q.x, q.y + 3.0]) })
}

/// How far from `d` the copies of the sketch's offset stand off the source, the worst of 400 places of each; `None`
/// where there is no offset.
fn worst_off(p: &Project, si: usize, source: Id, d: f64) -> Option<f64> {
    let s = &p.sketches[si];
    let curve = Curve::of(s, s.entity(source)?)?;
    let copies: Vec<Curve> = s.entities.iter().filter(|e| e.id != source && e.kind.as_spline().is_some()).filter_map(|e| Curve::of(s, e)).collect();
    copies.iter().flat_map(|c| (0..=400).map(|k| c.at(k as f64 / 400.0))).map(|q| (curve.nearest(q).off - d).abs()).reduce(f64::max)
}

#[test]
fn an_offset_of_any_curve_follows_its_source() {
    let sources = [
        Source { name: "an ellipse", lay: ellipse, drag: major_out },
        Source { name: "an arc of an ellipse", lay: ellipse_arc, drag: major_out },
        Source { name: "an open spline", lay: open_spline, drag: node_up },
        Source { name: "a closed spline", lay: closed_spline, drag: node_up },
    ];
    let mut failures = Vec::new();
    for Source { name, lay, drag } in sources {
        let mut p = Project::default();
        p.new_document();
        let si = p.new_sketch(name);
        let eid = lay(&mut p, si);
        p.solve_sketch(si);
        if p.offset_entities(si, &[eid], 3.0) == 0 {
            failures.push(format!("{name}: not offset"));
            continue;
        }
        // the source edited: a point of it dragged and let go
        let Some(Dragged { point, to }) = drag(&p, si, eid) else {
            failures.push(format!("{name}: nothing to drag"));
            continue;
        };
        p.solve_sketch_drag(si, Some((point, to.x, to.y)));
        p.solve_sketch(si);
        match worst_off(&p, si, eid, 3.0) {
            Some(off) if off <= OFFSET_TOLERANCE => {}
            off => failures.push(format!("{name}: the source edited, the offset stands {off:?} off 3")),
        }
        // the dimension of the offset set to 5
        let dim = p.sketches[si].constraints.iter().position(|c| matches!(c, Constraint::CurveDistance { .. }));
        match dim.map(|ci| &mut p.sketches[si].constraints[ci]) {
            Some(Constraint::CurveDistance { d, .. }) => *d = 5.0,
            _ => {
                failures.push(format!("{name}: the offset has no dimension"));
                continue;
            }
        }
        p.solve_sketch(si);
        match worst_off(&p, si, eid, 5.0) {
            Some(off) if off <= OFFSET_TOLERANCE => {}
            off => failures.push(format!("{name}: its dimension set to 5, the offset stands {off:?} off 5")),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
