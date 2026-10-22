//! THE FIRST PIECE OF A BROKEN CURVE IS THE CURVE IT WAS, of every kind: an arc, a circle, an ellipse and a spline broken
//! by a click are two pieces, and the piece from the start of the curve - for a closed one, the piece from the place of
//! the click on - keeps the id of the curve, so what was built from it goes on naming it, as a line broken in two does.
use qymcad_core::feature::{Ends, Purpose, Winding};
use qymcad_core::geom::Point2;
use qymcad_core::model::curve::Curve;
use qymcad_core::model::{Id, Project};

/// THE CURVE BROKEN.
#[derive(Clone, Copy, Debug)]
enum Kind {
    Arc,
    Circle,
    Ellipse,
    Spline,
}

/// A sketch with one curve of `kind`, its id, and where it is clicked to break it.
struct Drawn {
    p: Project,
    eid: Id,
    click: Point2,
}

fn drawn(kind: Kind) -> Drawn {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    let (eid, click) = match kind {
        Kind::Arc => {
            p.add_arc_entity(si, Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), Point2::new(-10.0, 0.0), Winding::Ccw, Purpose::Real);
            (p.sketches[si].entities.iter().find(|e| e.kind.as_arc().is_some()).map(|e| e.id).expect("the arc"), Point2::new(0.0, 10.0))
        }
        Kind::Circle => (p.add_circle_entity(si, 0.0, 0.0, 10.0, Purpose::Real), Point2::new(0.0, 10.0)),
        Kind::Ellipse => {
            p.add_ellipse_entity(si, Point2::new(0.0, 0.0), 12.0, 6.0, 0.0, Purpose::Real);
            (p.sketches[si].entities.iter().find(|e| e.kind.as_ellipse().is_some()).map(|e| e.id).expect("the ellipse"), Point2::new(0.0, 6.0))
        }
        Kind::Spline => {
            let id = p.add_spline(si, vec![Point2::new(-10.0, 0.0), Point2::new(0.0, 5.0), Point2::new(10.0, 0.0)], Ends::Open, Purpose::Real).expect("the spline");
            (id, Point2::new(0.0, 5.0))
        }
    };
    Drawn { p, eid, click }
}

#[test]
fn the_first_piece_of_a_broken_curve_keeps_its_id() {
    let mut failures = Vec::new();
    for kind in [Kind::Arc, Kind::Circle, Kind::Ellipse, Kind::Spline] {
        let Drawn { mut p, eid, click } = drawn(kind);
        // where the piece keeping the id starts: the start of an open curve, the place of the click on a closed one
        let start = {
            let s = &p.sketches[0];
            let c = s.entity(eid).and_then(|e| Curve::of(s, e)).unwrap_or_else(|| panic!("{kind:?}: the curve {eid} in {:?}", s.entities.iter().map(|e| e.id).collect::<Vec<_>>()));
            if c.closed() {
                click
            } else {
                c.at(0.0)
            }
        };
        if !p.break_curve(0, eid, click.x, click.y) {
            failures.push(format!("{kind:?}: not broken"));
            continue;
        }
        let s = &p.sketches[0];
        let pieces = s.entities.len();
        let kept = s.entity(eid).and_then(|e| Curve::of(s, e));
        let from_start = kept.as_ref().is_some_and(|c| c.at(0.0).dist(start) < 1e-6);
        if pieces != 2 || kept.is_none() || !from_start {
            failures.push(format!("{kind:?}: {pieces} pieces, the id of the curve on {kept:?}, from its start {from_start}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
