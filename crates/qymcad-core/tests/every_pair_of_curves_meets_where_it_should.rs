//! THE CURVE OF A SKETCH, ONE FOR EVERY KIND (`model::curve`): every pair of kinds meets where the geometry says - the
//! points worked out by hand where there is a formula, and for a spline as many points as it crosses the other curve,
//! counted by the sign of the other's own equation along 200 000 points of the spline - each on both curves to 1e-7 mm; a touch is one point; every kind is laid out within 0.005 mm of itself at a quarter, a half and
//! three quarters of every piece; the nearest place to a point lies on the curve square to it.
use qymcad_core::feature::{Ends, Purpose, Winding};
use qymcad_core::geom::Point2;
use qymcad_core::model::curve::{Curve, SAG};
use qymcad_core::model::{EllipseArcSpec, Project};

/// A point of the sheet by its two coordinates.
fn p2(at: [f64; 2]) -> Point2 {
    Point2::new(at[0], at[1])
}

/// A KIND OF THE TABLE: its name and the curve of the entity a sketch lays for it.
struct Kind {
    name: &'static str,
    curve: Curve,
}

/// THE EQUATION OF AN ANALYTIC KIND: its sign tells the sides of it, and the stretch of it the curve takes. `None` for a
/// spline, which has none.
fn equation(name: &str) -> Option<Equation> {
    Some(match name {
        "line" => Equation { side: |p| p.y - 1.0, takes: |p| p.x.abs() <= 12.0 },
        "circle" => Equation { side: |p| p.x * p.x + p.y * p.y - 25.0, takes: |_| true },
        "arc" => Equation { side: |p| p.x * p.x + p.y * p.y - 25.0, takes: |p| p.y >= 0.0 },
        "ellipse" => Equation { side: |p| (p.x / 8.0).powi(2) + (p.y / 4.0).powi(2) - 1.0, takes: |_| true },
        "arc of an ellipse" => Equation { side: |p| (p.x / 8.0).powi(2) + (p.y / 4.0).powi(2) - 1.0, takes: |p| p.y >= 0.0 },
        _ => return None,
    })
}

/// The sign of a curve's equation, and the stretch of it taken.
struct Equation {
    side: fn(Point2) -> f64,
    takes: fn(Point2) -> bool,
}

/// HOW MANY TIMES `curve` CROSSES the kind of `eq`: the changes of sign of its equation along 200 000 points of the
/// curve, where the stretch of it is taken.
fn crossings(curve: &Curve, eq: &Equation) -> usize {
    let n = 200_000;
    let pts: Vec<Point2> = (0..=n).map(|k| curve.at(k as f64 / n as f64)).collect();
    pts.windows(2).filter(|w| (eq.side)(w[0]) * (eq.side)(w[1]) < 0.0 && (eq.takes)(w[0])).count()
}

/// The curve of the last entity laid into sketch `si`.
fn last(p: &Project, si: usize) -> Curve {
    let s = &p.sketches[si];
    Curve::of(s, s.entities.last().expect("an entity")).expect("its curve")
}

/// Every kind, each laid by the sketch as a person's tool lays it, in a sketch of its own.
fn kinds() -> Vec<Kind> {
    let mut p = Project::default();
    let mut out = Vec::new();
    let mut lay = |name: &'static str, f: &dyn Fn(&mut Project, usize)| {
        let si = p.new_sketch(name);
        f(&mut p, si);
        out.push(Kind { name, curve: last(&p, si) });
    };
    lay("line", &|p, si| {
        p.add_line_entity(si, -12.0, 1.0, 12.0, 1.0, Purpose::Real);
    });
    lay("circle", &|p, si| {
        p.add_circle_entity(si, 0.0, 0.0, 5.0, Purpose::Real);
    });
    lay("arc", &|p, si| p.add_arc_entity(si, p2([0.0, 0.0]), p2([5.0, 0.0]), p2([-5.0, 0.0]), Winding::Ccw, Purpose::Real));
    lay("ellipse", &|p, si| {
        p.add_ellipse_entity(si, p2([0.0, 0.0]), 8.0, 4.0, 0.0, Purpose::Real);
    });
    lay("arc of an ellipse", &|p, si| {
        let spec = EllipseArcSpec { centre: p2([0.0, 0.0]), radii: p2([8.0, 4.0]), rot: 0.0, angles: [0.0, std::f64::consts::PI], winding: Winding::Ccw };
        p.add_ellipse_arc_entity(si, spec, Purpose::Real);
    });
    lay("open spline", &|p, si| {
        p.add_spline(si, vec![p2([-10.0, -3.0]), p2([-3.0, 4.5]), p2([3.0, -4.5]), p2([10.0, 3.0])], Ends::Open, Purpose::Real);
    });
    lay("closed spline", &|p, si| {
        p.add_spline(si, vec![p2([-9.0, 0.0]), p2([0.0, 3.0]), p2([9.0, 0.0]), p2([0.0, -3.0])], Ends::Closed, Purpose::Real);
    });
    out
}

/// A CASE OF THE TABLE: two curves and the points they share, worked out by hand; for a spline and an analytic kind the
/// points are counted (`crossings`).
struct Case {
    a: &'static str,
    b: &'static str,
    points: Vec<Point2>,
}

fn cases() -> Vec<Case> {
    let r24 = 24f64.sqrt();
    let x1 = 8.0 * (1.0f64 - 1.0 / 16.0).sqrt();
    let (cx, cy) = (12f64.sqrt(), 13f64.sqrt());
    let case = |a, b, points: Vec<Point2>| Case { a, b, points };
    vec![
        // the line y = 1 across every kind
        case("line", "circle", vec![p2([-r24, 1.0]), p2([r24, 1.0])]),
        case("line", "arc", vec![p2([-r24, 1.0]), p2([r24, 1.0])]),
        case("line", "ellipse", vec![p2([-x1, 1.0]), p2([x1, 1.0])]),
        case("line", "arc of an ellipse", vec![p2([-x1, 1.0]), p2([x1, 1.0])]),
        // the circle of radius 5 and the ellipse 8 by 4: x^2 = 12, y^2 = 13
        case("circle", "ellipse", vec![p2([-cx, -cy]), p2([-cx, cy]), p2([cx, -cy]), p2([cx, cy])]),
        case("arc", "arc of an ellipse", vec![p2([-cx, cy]), p2([cx, cy])]),
        case("circle", "arc of an ellipse", vec![p2([-cx, cy]), p2([cx, cy])]),
        case("arc", "ellipse", vec![p2([-cx, cy]), p2([cx, cy])]),
        // a spline across every analytic kind
        case("open spline", "line", Vec::new()),
        case("open spline", "circle", Vec::new()),
        case("open spline", "arc", Vec::new()),
        case("open spline", "ellipse", Vec::new()),
        case("open spline", "arc of an ellipse", Vec::new()),
        case("closed spline", "line", Vec::new()),
        case("closed spline", "circle", Vec::new()),
        case("closed spline", "arc", Vec::new()),
        case("closed spline", "ellipse", Vec::new()),
        case("closed spline", "arc of an ellipse", Vec::new()),
    ]
}

#[test]
fn every_pair_of_kinds_meets_where_the_geometry_says() {
    let all = kinds();
    let of = |name: &str| &all.iter().find(|k| k.name == name).expect("the kind").curve;
    let mut failures = Vec::new();
    for case in cases() {
        // how many: the points worked out, or the crossings of the spline counted against the analytic kind
        let count = match equation(case.b) {
            Some(eq) if case.points.is_empty() => crossings(of(case.a), &eq),
            _ => case.points.len(),
        };
        if count == 0 {
            failures.push(format!("{} and {}: the case meets nowhere - it checks nothing", case.a, case.b));
        }
        for [first, second] in [[case.a, case.b], [case.b, case.a]] {
            let (a, b) = (of(first), of(second));
            let found = a.meet(b);
            let label = format!("{first} and {second}");
            if found.len() != count {
                failures.push(format!("{label}: {} meetings, not {count}: {:?}", found.len(), found.iter().map(|m| m.at).collect::<Vec<_>>()));
                continue;
            }
            for m in &found {
                let gap = a.at(m.ta).dist(b.at(m.tb));
                if gap > 1e-7 || a.at(m.ta).dist(m.at) > 1e-7 {
                    failures.push(format!("{label}: the meeting at {:?} is {gap:.2e} apart on the two curves", m.at));
                }
            }
            for want in &case.points {
                let near = found.iter().map(|m| m.at.dist(*want)).fold(f64::MAX, f64::min);
                if near > 1e-6 {
                    failures.push(format!("{label}: no meeting at {want:?}, the nearest {near:.2e} off"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{} of the pairs do not meet as they should:\n  {}", failures.len(), failures.join("\n  "));
}

#[test]
fn a_line_touching_a_circle_meets_it_once() {
    let mut p = Project::default();
    let si = p.new_sketch("s");
    p.add_circle_entity(si, 0.0, 0.0, 5.0, Purpose::Real);
    let circle = last(&p, si);
    p.add_line_entity(si, -8.0, 5.0, 8.0, 5.0, Purpose::Real);
    let line = last(&p, si);
    let found = line.meet(&circle);
    assert!(found.len() == 1 && found[0].at.dist(p2([0.0, 5.0])) < 1e-9, "a line touching a circle meets it at {:?}", found.iter().map(|m| m.at).collect::<Vec<_>>());
}

#[test]
fn every_kind_is_laid_out_within_the_sag() {
    let mut failures = Vec::new();
    for k in kinds() {
        let places = k.curve.places();
        let mut worst: f64 = 0.0;
        for w in places.windows(2) {
            let (a, b) = (k.curve.at(w[0]), k.curve.at(w[1]));
            for share in [0.25, 0.5, 0.75] {
                let q = k.curve.at(w[0] + (w[1] - w[0]) * share);
                let l = a.dist(b);
                let off = if l < 1e-15 { q.dist(a) } else { ((b.x - a.x) * (q.y - a.y) - (b.y - a.y) * (q.x - a.x)).abs() / l };
                worst = worst.max(off);
            }
        }
        if worst > SAG + 1e-12 {
            failures.push(format!("{}: a piece strays {worst:.4} mm from the curve", k.name));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn the_nearest_place_of_every_kind_lies_square_to_the_point() {
    let mut failures = Vec::new();
    for k in kinds() {
        // a point off the middle of the curve, along its normal there
        let t = 0.4;
        let (q, d) = (k.curve.at(t), k.curve.tangent(t));
        let p = Point2::new(q.x - d.y * 0.7, q.y + d.x * 0.7);
        let at = k.curve.nearest(p);
        let along = k.curve.tangent(at.t);
        let lean = ((p.x - at.point.x) * along.x + (p.y - at.point.y) * along.y).abs();
        if (at.off - 0.7).abs() > 1e-6 || lean > 1e-6 {
            failures.push(format!("{}: the nearest place is {:.6} off, leaning {lean:.2e}, not 0.7 square", k.name, at.off));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The kind of a curve in words, as the table names it.
fn kind_of(c: &Curve) -> &'static str {
    match c {
        Curve::Line { .. } => "line",
        Curve::Round { .. } => "round",
        Curve::Oval { .. } => "oval",
        Curve::Spline { .. } => "spline",
    }
}

#[test]
fn a_piece_of_every_kind_is_the_same_curve_over_a_shorter_run() {
    let mut failures = Vec::new();
    for k in kinds() {
        let c = &k.curve;
        // a third of the way into a span and past the middle; round the start of a closed curve
        let runs: Vec<[f64; 2]> = if c.closed() { vec![[0.0, 0.37], [0.37, 1.0], [0.8, 1.3]] } else { vec![[0.0, 0.37], [0.37, 0.71], [0.71, 1.0]] };
        for [from, to] in runs {
            let piece = c.piece([from, to]);
            let label = format!("{} from {from} to {to}", k.name);
            if kind_of(&piece) != kind_of(c) {
                failures.push(format!("{label}: a {} came out of a {}", kind_of(&piece), kind_of(c)));
                continue;
            }
            let ends = [piece.at(0.0), piece.at(1.0)];
            let want = [c.at(from), c.at(to.rem_euclid(1.0 + 1e-15))];
            if ends[0].dist(want[0]) > 1e-9 || ends[1].dist(want[1]) > 1e-9 {
                failures.push(format!("{label}: the piece runs from {:?} to {:?}, not {:?} to {:?}", ends[0], ends[1], want[0], want[1]));
            }
            // every point of the piece lies on the curve, and it leaves its start the way the curve does - the places along
            // a piece of a spline are not those of the whole, its spans being others
            let worst = (0..=400).map(|i| c.nearest(piece.at(i as f64 / 400.0)).off).fold(0.0, f64::max);
            let (u, v) = (piece.tangent(0.0), c.tangent(from));
            let turn = (u.x * v.y - u.y * v.x).abs() + (1.0 - (u.x * v.x + u.y * v.y)).abs();
            if worst > 1e-6 || turn > 1e-9 {
                failures.push(format!("{label}: the piece strays {worst:.2e} mm off the curve, or leaves its start another way ({turn:.2e})"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn every_open_kind_stretches_along_its_own_way_to_a_curve() {
    use qymcad_core::model::curve::End;
    // the upright line x = 14 a line or a spline is stretched to from its finish, and x = -14 from its start; an arc of a
    // circle or of an ellipse, the upper half of its curve, goes on round below to the upright line x = 0 from either end
    let wall = |x: f64| Curve::Line { a: p2([x, -40.0]), b: p2([x, 40.0]) };
    let mut failures = Vec::new();
    for k in kinds().into_iter().filter(|k| !k.curve.closed()) {
        for end in [End::Finish, End::Start] {
            let round = matches!(k.curve, Curve::Round { .. } | Curve::Oval { .. });
            let x = if round {
                0.0
            } else if end == End::Finish {
                14.0
            } else {
                -14.0
            };
            let label = format!("{} from its {end:?}", k.name);
            let Some(longer) = k.curve.stretched(end, &wall(x)) else {
                failures.push(format!("{label}: it did not reach the line x = {x}"));
                continue;
            };
            if round && (if end == End::Finish { longer.at(1.0) } else { longer.at(0.0) }).y > -1.0 {
                failures.push(format!("{label}: it did not go on round below to the line x = 0"));
            }
            let reached = if end == End::Finish { longer.at(1.0) } else { longer.at(0.0) };
            if (reached.x - x).abs() > 1e-7 {
                failures.push(format!("{label}: it ends at {reached:?}, not on x = {x}"));
            }
            // the old curve is a part of the longer one, and the stretch goes on its own way
            let off = (0..=100).map(|i| longer.nearest(k.curve.at(i as f64 / 100.0)).off).fold(0.0, f64::max);
            if off > 1e-6 {
                failures.push(format!("{label}: the curve it was is {off:.2e} mm off the longer one"));
            }
            let kept_end = if end == End::Finish { k.curve.at(1.0) } else { k.curve.at(0.0) };
            let (t_old, t_new) = (k.curve.tangent(if end == End::Finish { 1.0 } else { 0.0 }), longer.tangent(longer.nearest(kept_end).t));
            if (t_old.x * t_new.y - t_old.y * t_new.x).abs() > 1e-6 {
                failures.push(format!("{label}: it turns where it was cut off"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn every_kind_moved_turned_and_mirrored_is_its_image() {
    use qymcad_core::model::curve::Map;
    let maps = [Map::shift(p2([7.0, -3.0])), Map::turn(p2([2.0, 1.0]), 0.7), Map::mirror([p2([-1.0, -4.0]), p2([3.0, 5.0])])];
    let mut failures = Vec::new();
    for k in kinds() {
        for (m, map) in maps.iter().enumerate() {
            let image = k.curve.mapped(map);
            if kind_of(&image) != kind_of(&k.curve) {
                failures.push(format!("{} through map {m}: a {} came out", k.name, kind_of(&image)));
                continue;
            }
            let worst = (0..=200).map(|i| i as f64 / 200.0).map(|t| image.at(t).dist(map.point(k.curve.at(t)))).fold(0.0, f64::max);
            if worst > 1e-9 {
                failures.push(format!("{} through map {m}: the image is {worst:.2e} mm off where the map takes the curve", k.name));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn every_kind_offset_stands_at_its_distance() {
    use qymcad_core::model::curve::OFFSET_TOLERANCE;
    let mut failures = Vec::new();
    for k in kinds() {
        // less than the least radius of curvature of any kind inwards - the tips of the flat closed spline bend at 0.25 mm
        // (its way 3 and its turn 36 by 6 there: 27 / 108): an offset past it folds into a loop, cut away into pieces
        for d in [0.4, -0.2] {
            let label = format!("{} offset {d}", k.name);
            let offsets = k.curve.offset(d);
            let [off] = offsets.as_slice() else {
                failures.push(format!("{label}: {} offsets, not one", offsets.len()));
                continue;
            };
            let exact = matches!(k.curve, Curve::Line { .. } | Curve::Round { .. });
            let want_kind = if exact { kind_of(&k.curve) } else { "spline" };
            if kind_of(off) != want_kind || off.closed() != k.curve.closed() {
                failures.push(format!("{label}: a {} (closed {}) came out, not a {want_kind}", kind_of(off), off.closed()));
            }
            // every point of the offset stands |d| from the curve, on the side asked for
            let tol = if exact { 1e-9 } else { OFFSET_TOLERANCE };
            let worst = (0..=300).map(|i| off.at(i as f64 / 300.0)).map(|p| (k.curve.nearest(p).off - d.abs()).abs()).fold(0.0, f64::max);
            let (q, u) = (k.curve.at(0.5), k.curve.tangent(0.5));
            let side = (off.nearest(q).point - q).x * -u.y + (off.nearest(q).point - q).y * u.x;
            if worst > tol || side * d <= 0.0 {
                failures.push(format!("{label}: a point of it stands {worst:.2e} off the distance, or it lies on the other side ({side:.3})"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_turned_ellipse_meets_its_axis_at_its_ends_and_misses_what_passes_by() {
    let mut p = Project::default();
    let si = p.new_sketch("s");
    // the major axis along (1, 1), its end at (3, 3); the minor semi-axis 1
    p.add_ellipse_entity(si, p2([0.0, 0.0]), 18f64.sqrt(), 1.0, std::f64::consts::FRAC_PI_4, Purpose::Real);
    let ellipse = last(&p, si);
    let axis = Curve::Line { a: p2([-4.0, -4.0]), b: p2([4.0, 4.0]) };
    let found = axis.meet(&ellipse);
    let hit = |at: Point2| found.iter().any(|m| m.at.dist(at) < 1e-9);
    assert!(found.len() == 2 && hit(p2([3.0, 3.0])) && hit(p2([-3.0, -3.0])), "the axis meets the turned ellipse at {:?}", found.iter().map(|m| m.at).collect::<Vec<_>>());
    // a line past it, and a circle inside it, meet it nowhere
    let past = Curve::Line { a: p2([-5.0, 5.0]), b: p2([5.0, 15.0]) };
    let inside = Curve::Round { centre: p2([0.0, 0.0]), r: 0.5, from: 0.0, sweep: std::f64::consts::TAU };
    assert!(past.meet(&ellipse).is_empty() && inside.meet(&ellipse).is_empty(), "a line past the ellipse or a circle inside it meets it");
}

#[test]
fn an_offset_past_the_bend_of_a_spline_has_its_loop_cut_away() {
    use qymcad_core::model::curve::OFFSET_TOLERANCE;
    let mut p = Project::default();
    let si = p.new_sketch("s");
    // the S of the matrix: its bends are tighter than 3 mm, where an offset of 3 folds into a loop
    p.add_spline(si, vec![p2([-8.0, 0.0]), p2([-3.0, 4.0]), p2([3.0, -4.0]), p2([8.0, 1.0])], Ends::Open, Purpose::Real);
    let s_curve = last(&p, si);
    for d in [3.0, -3.0] {
        let pieces = s_curve.offset(d);
        assert!(pieces.len() >= 2, "offset {d}: the loop was not cut - {} piece", pieces.len());
        // the pieces meet end to end at the corners, and no point of them stands nearer the curve than the distance
        for w in pieces.windows(2) {
            assert!(w[0].at(1.0).dist(w[1].at(0.0)) < 1e-9, "offset {d}: the pieces do not meet at their corner");
        }
        let nearest = pieces.iter().flat_map(|c| (0..=200).map(move |k| c.at(k as f64 / 200.0))).map(|q| s_curve.nearest(q).off).fold(f64::MAX, f64::min);
        assert!(nearest > d.abs() - OFFSET_TOLERANCE, "offset {d}: a point of it stands {nearest:.3} from the curve");
    }
}

/// THE OFFSET OF A CLOSED SPLINE IS A SPLINE OF FEW NODES: a closed blob of seven nodes, about 50 by 40, offset 1.5 out and
/// in, is one closed spline within `OFFSET_TOLERANCE` of the true offset, of tens of nodes - no more than eight for each
/// node of the blob - not hundreds; offset 4, past the bend of its notch, its loop cut away, the pieces left as few.
///
/// Reported behaviour: the offset of a closed spline came out a spline of hundreds of nodes - 1038 degrees of freedom -
/// and the program hung.
#[test]
fn the_offset_of_a_closed_spline_is_a_spline_of_few_nodes() {
    use qymcad_core::model::curve::OFFSET_TOLERANCE;
    let blob = [[-22.0, 6.0], [-14.0, 14.0], [-3.0, 7.0], [7.0, 25.0], [21.0, 9.0], [10.0, -9.0], [-17.0, -14.0]];
    let curve = Curve::spline(blob.iter().map(|q| p2(*q)).collect(), &[], qymcad_core::feature::Ends::Closed);
    let mut failures = Vec::new();
    // offset 4 the notch of the blob bends tighter than that: its loop is cut away, and what is left is in pieces
    let pieces: usize = curve.offset(4.0).iter().map(|c| c.nodes().len()).sum();
    let worst = curve.offset(4.0).iter().flat_map(|c| (0..=500).map(|i| c.at(i as f64 / 500.0)).collect::<Vec<_>>()).map(|p| (curve.nearest(p).off - 4.0).abs()).fold(0.0, f64::max);
    if pieces == 0 || pieces > 8 * blob.len() || worst > OFFSET_TOLERANCE {
        failures.push(format!("offset 4, its loop cut away: {pieces} nodes in all, {worst:.4} mm off the distance at worst"));
    }
    for d in [1.5, -1.5] {
        let offsets = curve.offset(d);
        let [off] = offsets.as_slice() else {
            failures.push(format!("offset {d}: {} pieces, not one", offsets.len()));
            continue;
        };
        let nodes = off.nodes().len();
        let worst = (0..=2000).map(|i| off.at(i as f64 / 2000.0)).map(|p| (curve.nearest(p).off - d.abs()).abs()).fold(0.0, f64::max);
        if nodes == 0 || nodes > 8 * blob.len() || worst > OFFSET_TOLERANCE || !off.closed() {
            failures.push(format!("offset {d}: a closed {} spline of {nodes} nodes, {worst:.4} mm off the distance at worst", off.closed()));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
