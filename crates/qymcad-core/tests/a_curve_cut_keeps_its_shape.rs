//! A CURVE OF A SKETCH CUT INTO PIECES KEEPS ITS SHAPE (`Project::cut_curve`): a line becomes lines, a circle and an arc
//! arcs of the same centre, an ellipse and an arc of one arcs of the same ellipse, a spline splines - every piece on the
//! curve it was cut from, the pieces covering it end to end.
//!
//! Reported behaviour: an ellipse could not be cut at all, and a spline cut lost its shape - a zigzag cut a third of the
//! way into a span bowed away from where it was drawn.
use qymcad_core::feature::{Ends, Purpose, Winding};
use qymcad_core::geom::Point2;
use qymcad_core::model::curve::Curve;
use qymcad_core::model::{EllipseArcSpec, EntityKind, Project};

fn p2(at: [f64; 2]) -> Point2 {
    Point2::new(at[0], at[1])
}

/// The kind of an entity in words.
fn kind(k: &EntityKind) -> &'static str {
    match (k.name_key(), k.as_ellipse().map(|e| e.span.is_some())) {
        (_, Some(false)) => "ellipse",
        (_, Some(true)) => "arc of an ellipse",
        ("ent-line", _) => "line",
        ("ent-arc", _) => "arc",
        ("ent-circle", _) => "circle",
        _ => "spline",
    }
}

#[test]
fn every_kind_cut_is_pieces_of_its_own_kind_on_it() {
    // each kind laid alone in a sketch of its own, the kind its pieces must be, and where it is cut
    struct Case {
        name: &'static str,
        lay: fn(&mut Project, usize),
        pieces: &'static str,
        places: &'static [f64],
    }
    let cases = [
        Case {
            name: "line",
            lay: |p, si| {
                p.add_line_entity(si, 0.0, 0.0, 20.0, 10.0, Purpose::Real);
            },
            pieces: "line",
            places: &[0.3, 0.6],
        },
        Case {
            name: "circle",
            lay: |p, si| {
                p.add_circle_entity(si, 0.0, 0.0, 6.0, Purpose::Real);
            },
            pieces: "arc",
            places: &[0.1, 0.45],
        },
        Case { name: "arc", lay: |p, si| p.add_arc_entity(si, p2([0.0, 0.0]), p2([6.0, 0.0]), p2([-6.0, 0.0]), Winding::Ccw, Purpose::Real), pieces: "arc", places: &[0.4] },
        Case {
            name: "ellipse",
            lay: |p, si| {
                p.add_ellipse_entity(si, p2([0.0, 0.0]), 8.0, 4.0, 0.3, Purpose::Real);
            },
            pieces: "arc of an ellipse",
            places: &[0.2, 0.7],
        },
        Case {
            name: "arc of an ellipse",
            lay: |p, si| {
                p.add_ellipse_arc_entity(si, EllipseArcSpec { centre: p2([0.0, 0.0]), radii: p2([8.0, 4.0]), rot: 0.0, angles: [0.0, 2.0], winding: Winding::Ccw }, Purpose::Real);
            },
            pieces: "arc of an ellipse",
            places: &[0.5],
        },
        // the zigzag of the report, cut a third of the way into its middle span
        Case {
            name: "open spline",
            lay: |p, si| {
                p.add_spline(si, vec![p2([0.0, 0.0]), p2([10.0, 8.0]), p2([20.0, -8.0]), p2([30.0, 0.0])], Ends::Open, Purpose::Real);
            },
            pieces: "spline",
            places: &[(1.0 + 1.0 / 3.0) / 3.0],
        },
        Case {
            name: "closed spline",
            lay: |p, si| {
                p.add_spline(si, vec![p2([-9.0, 0.0]), p2([0.0, 5.0]), p2([9.0, 0.0]), p2([0.0, -5.0])], Ends::Closed, Purpose::Real);
            },
            pieces: "spline",
            places: &[0.15, 0.6],
        },
    ];
    let mut failures = Vec::new();
    for case in &cases {
        let mut p = Project::default();
        let si = p.new_sketch(case.name);
        (case.lay)(&mut p, si);
        let s = &p.sketches[si];
        let whole = s.entities.last().map(|e| (e.id, Curve::of(s, e).expect("its curve"))).expect("the curve");
        let ids = p.cut_curve(si, whole.0, case.places);
        let s = &p.sketches[si];
        let want = if whole.1.closed() { case.places.len() } else { case.places.len() + 1 };
        if ids.len() != want || s.entities.len() != want {
            failures.push(format!("{}: {} pieces and {} entities, not {want}", case.name, ids.len(), s.entities.len()));
            continue;
        }
        let mut covered = 0.0;
        for e in &s.entities {
            if kind(&e.kind) != case.pieces {
                failures.push(format!("{}: a piece is a {}, not a {}", case.name, kind(&e.kind), case.pieces));
            }
            let piece = Curve::of(s, e).expect("the piece's curve");
            let worst = (0..=200).map(|k| whole.1.nearest(piece.at(k as f64 / 200.0)).off).fold(0.0, f64::max);
            if worst > 0.01 {
                failures.push(format!("{}: a piece strays {worst:.4} mm off the curve it was cut from", case.name));
            }
            covered += piece.length();
        }
        if (covered - whole.1.length()).abs() > 0.01 {
            failures.push(format!("{}: the pieces are {covered:.4} long, the curve {:.4}", case.name, whole.1.length()));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
