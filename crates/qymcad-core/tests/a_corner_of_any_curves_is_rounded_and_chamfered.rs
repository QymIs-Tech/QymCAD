//! A CORNER OF ANY TWO CURVES IS ROUNDED AND CHAMFERED: a line, an arc, an open spline and an arc of an ellipse, each
//! arriving at a vertex and each leaving it, every pair. A fillet of radius 2 is an arc of that radius touching both
//! curves where it meets them; a chamfer cuts 2 long with its ends as far from the corner, or 2 and 3 from it; the sides
//! left are pieces of the curves they were, cut back to the fillet or the chamfer.
//!
//! Reported behaviour: the fillet and the chamfer took lines and arcs only - a corner of a spline or of an arc of an
//! ellipse was no corner to them.
use qymcad_core::feature::{ChamferMode, Ends, Purpose, Winding};
use qymcad_core::geom::Point2;
use qymcad_core::model::curve::Curve;
use qymcad_core::model::{entity_ends, ChamferLegs, Constraint, EllipseArcSpec, Id, Project};

fn p2(at: [f64; 2]) -> Point2 {
    Point2::new(at[0], at[1])
}

/// The angle on an ellipse 10 by 5 where a line 6 from its centre across its long axis meets it: cos = -0.6, sin = 0.8.
const CAP: f64 = 2.214_297_435_588_181;

/// A CURVE ARRIVING AT THE VERTEX (0, 0) from the left.
fn arrive(p: &mut Project, si: usize, kind: &str) {
    match kind {
        "line" => {
            p.add_line_entity(si, -12.0, 0.0, 0.0, 0.0, Purpose::Real);
        }
        "arc" => p.add_arc_entity(si, p2([-6.0, 0.0]), p2([-6.0, -6.0]), p2([0.0, 0.0]), Winding::Ccw, Purpose::Real),
        "open spline" => {
            p.add_spline(si, vec![p2([-12.0, -3.0]), p2([-8.0, 1.0]), p2([-4.0, -2.0]), p2([0.0, 0.0])], Ends::Open, Purpose::Real);
        }
        // the left end of an ellipse 10 by 5 centred at (6, -4), from the vertex round to (0, -8)
        _ => {
            p.add_ellipse_arc_entity(
                si,
                EllipseArcSpec { centre: p2([6.0, -4.0]), radii: p2([10.0, 5.0]), rot: 0.0, angles: [CAP, std::f64::consts::TAU - CAP], winding: Winding::Ccw },
                Purpose::Real,
            );
        }
    }
}

/// A CURVE LEAVING THE VERTEX to the right.
fn leave(p: &mut Project, si: usize, kind: &str) {
    match kind {
        "line" => {
            p.add_line_entity(si, 0.0, 0.0, 8.0, 10.0, Purpose::Real);
        }
        "arc" => p.add_arc_entity(si, p2([-4.68, 3.72]), p2([0.0, 0.0]), p2([0.9, 5.92]), Winding::Ccw, Purpose::Real),
        "open spline" => {
            p.add_spline(si, vec![p2([0.0, 0.0]), p2([3.0, 5.0]), p2([7.0, 6.0]), p2([10.0, 11.0])], Ends::Open, Purpose::Real);
        }
        // the lower end of an upright ellipse 10 by 5 centred at (4, 6), from the vertex down round to (8, 0)
        _ => {
            p.add_ellipse_arc_entity(
                si,
                EllipseArcSpec { centre: p2([4.0, 6.0]), radii: p2([10.0, 5.0]), rot: std::f64::consts::FRAC_PI_2, angles: [CAP, std::f64::consts::TAU - CAP], winding: Winding::Ccw },
                Purpose::Real,
            );
        }
    }
}

/// THE POINTS STANDING AT THE VERTEX made one, as a click on a point there makes them: the id that stays.
fn weld(p: &mut Project, si: usize) -> Id {
    let s = &mut p.sketches[si];
    let there: Vec<Id> = s.points.iter().filter(|q| q.x.hypot(q.y) < 1e-6).map(|q| q.id).collect();
    let keep = there[0];
    for gone in &there[1..] {
        for e in s.entities.iter_mut() {
            e.kind.each_point_mut(|id| {
                if id == gone {
                    *id = keep;
                }
            });
        }
    }
    s.points.retain(|q| q.id == keep || !there.contains(&q.id));
    keep
}

/// THE CUT OF THE CORNER.
#[derive(Clone, Copy, Debug)]
enum Cut {
    Fillet,
    Chamfer,
    ChamferTwo,
}

/// A NEW SKETCH with the pair laid and their ends at the vertex made one: the project, the sketch, the vertex and the two
/// curves as they were.
struct Laid {
    p: Project,
    si: usize,
    vertex: Id,
    sides: Vec<Curve>,
}

fn laid(pair: [&str; 2]) -> Laid {
    let mut p = Project::default();
    let si = p.new_sketch("corner");
    arrive(&mut p, si, pair[0]);
    leave(&mut p, si, pair[1]);
    let vertex = weld(&mut p, si);
    let s = &p.sketches[si];
    let sides = s.entities.iter().filter_map(|e| Curve::of(s, e)).collect();
    Laid { p, si, vertex, sides }
}

/// WHAT THE CUT LEFT, against what it should.
fn made(cut: Cut, before: &Laid) -> Result<(), String> {
    let Laid { p, si, sides, .. } = before;
    let s = &p.sketches[*si];
    let on_a_side = |q: Point2| sides.iter().map(|c| c.nearest(q).off).fold(f64::MAX, f64::min);
    // the new curve: the last entity laid
    let new = s.entities.last().ok_or("no entity")?;
    let new_curve = Curve::of(s, new).ok_or("no curve of the cut")?;
    let [a, b] = entity_ends(new).ok_or("the cut has no ends")?;
    let ends = new_curve.ends();
    let wrong = !match cut {
        Cut::Fillet => new.kind.as_arc().is_some(),
        Cut::Chamfer | Cut::ChamferTwo => new.kind.as_line().is_some(),
    };
    if wrong || s.entities.len() != 3 {
        return Err(format!("the sketch holds {} entities, the last {:?}", s.entities.len(), new.kind));
    }
    for q in ends {
        if on_a_side(q) > 1e-6 {
            return Err(format!("an end of the cut stands {:.2e} off the curves", on_a_side(q)));
        }
    }
    // the sides left are on the curves they were, and each ends where the cut does
    for e in &s.entities[..2] {
        let c = Curve::of(s, e).ok_or("no curve of a side")?;
        let worst = (0..=50).map(|k| on_a_side(c.at(k as f64 / 50.0))).fold(0.0, f64::max);
        if worst > 1e-6 {
            return Err(format!("a side strays {worst:.2e} off the curve it was"));
        }
        let side_ends = entity_ends(e).ok_or("a side has no ends")?;
        if !side_ends.iter().any(|id| *id == a || *id == b) {
            return Err("a side does not end where the cut does".into());
        }
    }
    let corner = Point2::new(0.0, 0.0);
    match cut {
        Cut::Fillet => {
            let Curve::Round { centre, r, .. } = new_curve else { return Err("the fillet is no arc".into()) };
            if (r - 2.0).abs() > 1e-6 {
                return Err(format!("the fillet has a radius of {r:.6}"));
            }
            // at each end the side runs square to the radius
            for (q, id) in ends.iter().zip([a, b]) {
                let side = s.entities[..2].iter().find(|e| entity_ends(e).is_some_and(|x| x.contains(&id))).ok_or("no side at an end")?;
                let c = Curve::of(s, side).ok_or("no curve of a side")?;
                let t = if c.at(0.0).dist(*q) < c.at(1.0).dist(*q) { 0.0 } else { 1.0 };
                let (u, radial) = (c.tangent(t), *q - centre);
                let lean = (u.x * radial.x + u.y * radial.y).abs() / radial.x.hypot(radial.y);
                if lean > 1e-6 {
                    return Err(format!("the fillet meets a side leaning {lean:.2e} off touching it"));
                }
            }
        }
        Cut::Chamfer => {
            let legs = [ends[0].dist(corner), ends[1].dist(corner)];
            if (ends[0].dist(ends[1]) - 2.0).abs() > 1e-6 || (legs[0] - legs[1]).abs() > 1e-6 {
                return Err(format!("the chamfer is {:.6} long with its ends {:.6} and {:.6} from the corner", ends[0].dist(ends[1]), legs[0], legs[1]));
            }
        }
        Cut::ChamferTwo => {
            let mut legs = [ends[0].dist(corner), ends[1].dist(corner)];
            legs.sort_by(f64::total_cmp);
            if (legs[0] - 2.0).abs() > 1e-6 || (legs[1] - 3.0).abs() > 1e-6 {
                return Err(format!("the chamfer cuts {:.6} and {:.6} from the corner", legs[0], legs[1]));
            }
        }
    }
    Ok(())
}

#[test]
fn every_pair_of_kinds_at_a_vertex_is_rounded_and_chamfered() {
    let kinds = ["line", "arc", "open spline", "arc of an ellipse"];
    let mut failures = Vec::new();
    for first in kinds {
        for second in kinds {
            for cut in [Cut::Fillet, Cut::Chamfer, Cut::ChamferTwo] {
                let mut corner = laid([first, second]);
                let (si, pid) = (corner.si, corner.vertex);
                let done = match cut {
                    Cut::Fillet => corner.p.fillet_at_vertex(si, pid, 2.0),
                    Cut::Chamfer => corner.p.chamfer_at_vertex(si, pid, ChamferLegs::equal(2.0), None),
                    Cut::ChamferTwo => corner.p.chamfer_at_vertex(si, pid, ChamferLegs { mode: ChamferMode::TwoDist, first: 2.0, second: 3.0 }, None),
                };
                let result = if done { made(cut, &corner) } else { Err("refused".into()) };
                if let Err(why) = result {
                    failures.push(format!("{cut:?} of {first} to {second}: {why}"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{} of 48:\n{}", failures.len(), failures.join("\n"));
}

/// HOW FAR THE FILLET OF THE SKETCH STRAYS FROM TOUCHING ITS SIDES: the arc laid last, each of its ends on a side and
/// that side running square to its radius there; how far apart its ends stand, and its radius.
struct Held {
    off: f64,
    lean: f64,
    apart: f64,
    radius: f64,
}

fn held(p: &Project, si: usize) -> Option<Held> {
    let s = &p.sketches[si];
    // the fillet: the arc laid last
    let arc = s.entities.iter().filter(|e| e.kind.as_arc().is_some()).max_by_key(|e| e.id)?;
    let curve = Curve::of(s, arc)?;
    let Curve::Round { centre, r, .. } = curve else { return None };
    let [a, b] = entity_ends(arc)?;
    let mut out = Held { off: 0.0, lean: 0.0, apart: curve.at(0.0).dist(curve.at(1.0)), radius: r };
    for (q, id) in curve.ends().into_iter().zip([a, b]) {
        let side = s.entities.iter().find(|e| e.id != arc.id && entity_ends(e).is_some_and(|x| x.contains(&id)))?;
        let c = Curve::of(s, side)?;
        let t = if c.at(0.0).dist(q) < c.at(1.0).dist(q) { 0.0 } else { 1.0 };
        let (u, radial) = (c.tangent(t), q - centre);
        out.off = out.off.max(c.at(t).dist(q));
        out.lean = out.lean.max((u.x * radial.x + u.y * radial.y).abs() / radial.x.hypot(radial.y));
    }
    Some(out)
}

/// A FILLET OF ANY TWO CURVES HOLDS ITS CORNER WHEN A SIDE IS DRAGGED: every pair rounded R2, the far end of the first
/// side led 1 right and 1.5 down in six frames as the window drags it and let go - the arc still touches both sides
/// where it ends on them, keeps its radius of 2, and its ends stay apart. How far apart is the turn of the corner the
/// drag leaves: an open spline running nearly along a line after the drag turns some 10 deg, and its fillet of 2 has
/// ends about 0.35 apart.
///
/// Reported behaviour: a spline and a line rounded, the end of the line dragged - the arc shrank to nothing and was
/// drawn as a circle, its ends one point: the sharp of the corner was held on the spline itself, not on its way past its
/// end, and the arc was not held touching the spline.
#[test]
fn every_fillet_holds_its_corner_when_a_side_is_dragged() {
    let kinds = ["line", "arc", "open spline", "arc of an ellipse"];
    let mut failures = Vec::new();
    for first in kinds {
        for second in kinds {
            let mut corner = laid([first, second]);
            let (si, pid) = (corner.si, corner.vertex);
            if !corner.p.fillet_at_vertex(si, pid, 2.0) {
                failures.push(format!("{first} to {second}: refused"));
                continue;
            }
            // the far end of the first side: the end of the entity laid first that the arc does not end at
            let s = &corner.p.sketches[si];
            let arc_ends: Vec<Id> = s.entities.iter().filter(|e| e.kind.as_arc().is_some()).max_by_key(|e| e.id).and_then(entity_ends).map(|e| e.to_vec()).unwrap_or_default();
            let far = s.entities.first().and_then(entity_ends).and_then(|ends| ends.into_iter().find(|id| !arc_ends.contains(id)));
            let Some((far, at)) = far.and_then(|id| s.point(id).map(|q| (id, p2([q.x, q.y])))) else {
                failures.push(format!("{first} to {second}: no far end"));
                continue;
            };
            for k in 1..=6 {
                let f = k as f64 / 6.0;
                corner.p.solve_sketch_drag_fast(si, Some((far, at.x + f, at.y - 1.5 * f)));
            }
            corner.p.solve_sketch(si);
            match held(&corner.p, si) {
                Some(Held { off, lean, apart, radius }) if off < 1e-4 && lean < 1e-3 && apart > 1e-3 && (radius - 2.0).abs() < 1e-6 => {}
                Some(Held { off, lean, apart, radius }) => {
                    failures.push(format!("{first} to {second}: the arc of radius {radius:.6} ends {off:.2e} off its sides, leans {lean:.2e} off touching, its ends {apart:.3} apart"))
                }
                None => failures.push(format!("{first} to {second}: no fillet left")),
            }
        }
    }
    assert!(failures.is_empty(), "{} of 16:\n{}", failures.len(), failures.join("\n"));
}

/// A FILLET TOUCHES EVERY CURVE IT ENDS ON AT THAT END: the touching of the arc with a spline or an arc of an ellipse is
/// held where the two meet, the way of each read at the shared end - not sought along the curves, where the place of
/// touching could slide off the end the arc stands on.
#[test]
fn a_fillet_touches_a_curve_of_any_kind_at_the_end_it_shares_with_it() {
    let kinds = ["line", "arc", "open spline", "arc of an ellipse"];
    let mut failures = Vec::new();
    for first in kinds {
        for second in ["open spline", "arc of an ellipse"] {
            let mut corner = laid([first, second]);
            let si = corner.si;
            if !corner.p.fillet_at_vertex(si, corner.vertex, 2.0) {
                failures.push(format!("{first} to {second}: refused"));
                continue;
            }
            let along: Vec<String> = corner.p.sketches[si]
                .constraints
                .iter()
                .filter_map(|c| match c {
                    Constraint::TangentCurves { end: None, .. } => Some(format!("{c:?}")),
                    _ => None,
                })
                .collect();
            if !along.is_empty() {
                failures.push(format!("{first} to {second}: a touching sought along the curves, not held at the end: {}", along.join("; ")));
            }
        }
    }
    assert!(failures.is_empty(), "{} of 8:\n{}", failures.len(), failures.join("\n"));
}
