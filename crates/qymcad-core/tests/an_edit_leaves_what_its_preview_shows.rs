//! AN EDIT LEAVES WHAT ITS PREVIEW SHOWS, through one door for every kind of curve: Trim (`trim_curve`) takes away the
//! piece `trim_span` drew and keeps the rest, Break (`break_curve`) cuts where `break_at` said, Extend (`extend_curve`)
//! leaves the curve `extended` drew - for a line, an arc, a circle, an arc of an ellipse, an ellipse and a spline, the
//! end of the curve free, placed on a line by a tie, or joined to another curve. A tie that places the end is let go and
//! the end moves; an end that would move stays where it is joined, in the preview and in the edit alike (a spline grows
//! by a node past its end, which does not move). A line keeps what it is held by through the door as well: its pieces
//! stay on one straight, its halves stay level.
//!
//! Reported behaviour: the preview was drawn by one function and the edit made by another for each kind; a line went
//! through the door of the other kinds as plain pieces, its level and its straight lost; the end of a line or of an arc of
//! an ellipse placed on a line came back under the solve after Extend had moved it; the end of an arc or of a line
//! joined to another curve was moved and pulled it along.
use qymcad_core::feature::{Ends, Purpose, Winding};
use qymcad_core::geom::Point2;
use qymcad_core::model::curve::Curve;
use qymcad_core::model::{Constraint, EllipseArcSpec, Project};

fn p2(at: [f64; 2]) -> Point2 {
    Point2::new(at[0], at[1])
}

/// A CURVE OF THE MATRIX in a sketch fenced by two upright lines at x = -4 and x = 4 and a target upright at x = 12, all
/// from y -20 to 20: the curve crosses the fence twice and an open one meets the target on its way past its right end.
struct Fenced {
    p: Project,
    si: usize,
    curve: u64,
}

fn fenced(lay: fn(&mut Project, usize) -> u64) -> Fenced {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    for x in [-4.0, 4.0, 12.0] {
        p.add_line_entity(si, x, -20.0, x, 20.0, Purpose::Real);
    }
    let curve = lay(&mut p, si);
    p.regen_sketch(si);
    Fenced { p, si, curve }
}

/// A level line from (-10, 1) to (8, 1), held level.
fn line(p: &mut Project, si: usize) -> u64 {
    let id = p.add_line_entity(si, -10.0, 1.0, 8.0, 1.0, Purpose::Real);
    let ends = p.sketches[si].entity(id).and_then(|e| e.kind.as_line()).expect("the line");
    p.sketches[si].constraints.push(Constraint::Horizontal { a: ends.a, b: ends.b });
    id
}

/// An arc of radius 8 about the origin, from -150 deg round the bottom to 30 deg.
fn arc(p: &mut Project, si: usize) -> u64 {
    let at = |deg: f64| p2([8.0 * deg.to_radians().cos(), 8.0 * deg.to_radians().sin()]);
    p.add_arc_entity(si, p2([0.0, 0.0]), at(-150.0), at(30.0), Winding::Ccw, Purpose::Real);
    p.sketches[si].entities.last().expect("the arc").id
}

fn circle(p: &mut Project, si: usize) -> u64 {
    p.add_circle_entity(si, 0.0, 0.0, 8.0, Purpose::Real)
}

/// An arc of the ellipse 9 by 5 about the origin, from -150 deg round the bottom to 30 deg.
fn ellipse_arc(p: &mut Project, si: usize) -> u64 {
    let spec = EllipseArcSpec { centre: p2([0.0, 0.0]), radii: p2([9.0, 5.0]), rot: 0.0, angles: [-150f64.to_radians(), 30f64.to_radians()], winding: Winding::Ccw };
    p.add_ellipse_arc_entity(si, spec, Purpose::Real)
}

/// The ellipse 9 by 5 about the origin (laying one answers its centre: the curve is the one laid last).
fn ellipse(p: &mut Project, si: usize) -> u64 {
    p.add_ellipse_entity(si, p2([0.0, 0.0]), 9.0, 5.0, 0.0, Purpose::Real);
    p.sketches[si].entities.last().expect("the ellipse").id
}

fn spline(p: &mut Project, si: usize) -> u64 {
    p.add_spline(si, vec![p2([-10.0, -2.0]), p2([0.0, 3.0]), p2([9.0, 0.0])], Ends::Open, Purpose::Real).expect("the spline")
}

/// A CURVE OF THE MATRIX: its name, how it is laid, and whether it is open (Extend takes an open curve alone).
struct Case {
    name: &'static str,
    lay: fn(&mut Project, usize) -> u64,
    open: bool,
}

const CASES: [Case; 6] = [
    Case { name: "a line", lay: line, open: true },
    Case { name: "an arc", lay: arc, open: true },
    Case { name: "a circle", lay: circle, open: false },
    Case { name: "an arc of an ellipse", lay: ellipse_arc, open: true },
    Case { name: "an ellipse", lay: ellipse, open: false },
    Case { name: "a spline", lay: spline, open: true },
];

/// The curve `id` of the fenced sketch, `None` where it is gone.
fn curve_of(f: &Fenced, id: u64) -> Option<Curve> {
    let s = &f.p.sketches[f.si];
    s.entity(id).and_then(|e| Curve::of(s, e))
}

/// How far `q` stands from the nearest curve of the sketch.
fn off_every_curve(f: &Fenced, q: Point2) -> f64 {
    let s = &f.p.sketches[f.si];
    s.entities.iter().filter_map(|e| Curve::of(s, e)).map(|c| c.nearest(q).off).fold(f64::MAX, f64::min)
}

/// The place of the curve nearest the middle of the fence, below it - where a person clicks the piece between the fence.
fn middle_below(c: &Curve) -> Point2 {
    (0..=400).map(|k| c.at(k as f64 / 400.0)).filter(|q| q.y < 3.5).min_by(|a, b| a.x.abs().total_cmp(&b.x.abs())).expect("a place of the curve")
}

fn trim(case: &Case) -> Result<(), String> {
    let mut f = fenced(case.lay);
    let before = curve_of(&f, f.curve).ok_or("no curve laid")?;
    let click = middle_below(&before);
    let span = f.p.trim_span(f.si, f.curve, click).ok_or("no preview")?;
    let line_ends = f.p.sketches[f.si].entity(f.curve).and_then(|e| e.kind.as_line()).map(|l| [l.a, l.b]);
    if !f.p.trim_curve(f.si, f.curve, click.x, click.y) {
        return Err("the trim did nothing".into());
    }
    let (from, to) = (span.at(0.0), span.at(1.0));
    for k in 0..=200 {
        let q = before.at(k as f64 / 200.0);
        let on_span = span.nearest(q).off < 1e-9;
        let near_its_ends = q.dist(from) < 0.05 || q.dist(to) < 0.05;
        let left = off_every_curve(&f, q);
        if on_span && !near_its_ends && left < 1e-3 {
            return Err(format!("({:.3}, {:.3}) of the piece the preview took away is still drawn", q.x, q.y));
        }
        if span.nearest(q).off > 0.05 && left > 1e-6 {
            return Err(format!("({:.3}, {:.3}) off the piece the preview took away is gone, {left:.2e} from every curve", q.x, q.y));
        }
    }
    if let Some(ends) = line_ends {
        let on_its_ends = |a: u64, b: u64| (a == ends[0] && b == ends[1]) || (a == ends[1] && b == ends[0]);
        let straight = f.p.sketches[f.si].constraints.iter().filter(|c| matches!(c, Constraint::PointOnLine { a, b, .. } if on_its_ends(*a, *b))).count();
        if straight != 2 {
            return Err(format!("{straight} cut points held on the straight of the line, not 2: the pieces may part"));
        }
    }
    f.p.solve_sketch(f.si);
    Ok(())
}

fn break_case(case: &Case) -> Result<(), String> {
    let mut f = fenced(case.lay);
    let before = curve_of(&f, f.curve).ok_or("no curve laid")?;
    let click = middle_below(&before);
    let places = f.p.break_at(f.si, f.curve, click);
    if places.is_empty() || !f.p.break_curve(f.si, f.curve, click.x, click.y) {
        return Err("the break did nothing".into());
    }
    let s = &f.p.sketches[f.si];
    let ends: Vec<Point2> = s.entities.iter().filter_map(|e| Curve::of(s, e)).filter(|c| !c.closed()).flat_map(|c| c.ends()).collect();
    for t in places {
        let q = before.at(t);
        if !ends.iter().any(|e| e.dist(q) < 1e-6) {
            return Err(format!("no piece ends at ({:.3}, {:.3}), where the preview cut", q.x, q.y));
        }
    }
    if let Some(k) = (0..=200).map(|k| before.at(k as f64 / 200.0)).find(|q| off_every_curve(&f, *q) > 1e-6) {
        return Err(format!("({:.3}, {:.3}) of the curve is gone after the break", k.x, k.y));
    }
    if case.name == "a line" {
        let level = s.constraints.iter().filter(|c| matches!(c, Constraint::Horizontal { .. })).count();
        if level != 2 {
            return Err(format!("{level} halves held level, not 2"));
        }
    }
    Ok(())
}

/// HOW THE END OF THE CURVE STANDS before Extend.
#[derive(Clone, Copy, Debug)]
enum EndHeld {
    Free,
    /// placed by a tie on a short line across it, laid through the end
    PlacedOnALine,
    /// the start of a short line across it: a corner
    Joined,
}

fn extend(case: &Case, held: EndHeld) -> Result<(), String> {
    let mut f = fenced(case.lay);
    let before = curve_of(&f, f.curve).ok_or("no curve laid")?;
    let [_, finish] = before.ends();
    let end = if before.at(0.0).x > finish.x { before.at(0.0) } else { finish };
    let way = before.tangent(before.nearest(end).t);
    let across = [end.x - way.y * 3.0, end.y + way.x * 3.0];
    match held {
        EndHeld::Free => {}
        EndHeld::PlacedOnALine => {
            let back = [end.x + way.y * 3.0, end.y - way.x * 3.0];
            let l = f.p.add_line_entity(f.si, across[0], across[1], back[0], back[1], Purpose::Real);
            let ends = f.p.sketches[f.si].entity(l).and_then(|e| e.kind.as_line()).expect("the line across");
            let s = &f.p.sketches[f.si];
            let p = s.points.iter().filter(|q| Point2::new(q.x, q.y).dist(end) < 1e-9).map(|q| q.id).find(|id| *id != ends.a && *id != ends.b).ok_or("no point at the end")?;
            f.p.sketches[f.si].constraints.push(Constraint::PointOnLine { p, a: ends.a, b: ends.b });
        }
        EndHeld::Joined => {
            f.p.add_line_entity(f.si, end.x, end.y, across[0], across[1], Purpose::Real);
        }
    }
    f.p.regen_sketch(f.si);
    let click = Point2::new(end.x - 0.01, end.y);
    let preview = f.p.extended(f.si, f.curve, click);
    let did = f.p.extend_curve(f.si, f.curve, click.x, click.y);
    let got = curve_of(&f, f.curve).ok_or("the curve is gone after the extension")?;
    // an end that would move stays where it is joined: a spline grows by a node past its end, which does not move
    let stays = matches!(held, EndHeld::Joined) && case.name != "a spline";
    let Some(want) = preview else {
        let moved = (0..=40).map(|k| got.nearest(before.at(k as f64 / 40.0)).off).fold(0.0, f64::max) + got.ends()[0].dist(before.ends()[0]) + got.ends()[1].dist(before.ends()[1]);
        return match (stays, did || moved > 1e-9) {
            (true, false) => Ok(()),
            (true, true) => Err(format!("the preview keeps the end joined, and the edit moved the curve by {moved:.2e}")),
            (false, _) => Err("no preview".into()),
        };
    };
    if stays {
        return Err("the preview stretches an end joined to another curve".into());
    }
    if !did {
        return Err("the extension did nothing".into());
    }
    let worst = (0..=40).map(|k| got.nearest(want.at(k as f64 / 40.0)).off).fold(0.0, f64::max);
    let [w0, w1] = want.ends();
    let [g0, g1] = got.ends();
    let ends_apart = (w0.dist(g0) + w1.dist(g1)).min(w0.dist(g1) + w1.dist(g0));
    if worst > 1e-6 || ends_apart > 1e-6 {
        return Err(format!("the curve stands {worst:.2e} off the preview, its ends {ends_apart:.2e} from the preview's"));
    }
    Ok(())
}

#[test]
fn every_edit_of_every_curve_leaves_what_its_preview_shows() {
    let mut failures = Vec::new();
    for case in &CASES {
        if let Err(why) = trim(case) {
            failures.push(format!("Trim of {}: {why}", case.name));
        }
        if let Err(why) = break_case(case) {
            failures.push(format!("Break of {}: {why}", case.name));
        }
        for held in [EndHeld::Free, EndHeld::PlacedOnALine, EndHeld::Joined] {
            if case.open {
                if let Err(why) = extend(case, held) {
                    failures.push(format!("Extend of {} with its end {held:?}: {why}", case.name));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
