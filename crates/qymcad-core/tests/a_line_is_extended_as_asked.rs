//! A CURVE IS EXTENDED AS THE EXTEND TOOL ASKS: the nearer end to the nearest curve on its way, or to the curve the
//! pointer is over; both ends with "Both sides", each to its own; an end with nothing on its way stays - a line along
//! its axis, an arc and an arc of an ellipse round their own curve.
//!
//! Reported behaviour: Extend stretched the end nearer the click to the nearest crossing at once - no choice of which
//! curve to stop at, and no way to extend both ends.
use qymcad_core::feature::Purpose;
use qymcad_core::geom::Point2;
use qymcad_core::model::{ExtendAsk, ExtendSides, Project};

/// The line (0, 0) - (10, 0) and vertical lines across its axis at each of `across`. Answers the project, the sketch, the
/// line and the vertical lines in the order given.
fn lines(across: &[f64]) -> (Project, usize, u64, Vec<u64>) {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    p.add_line_entity(si, 0.0, 0.0, 10.0, 0.0, Purpose::Real);
    for &x in across {
        p.add_line_entity(si, x, -5.0, x, 5.0, Purpose::Real);
    }
    let ids: Vec<u64> = p.sketches[si].entities.iter().map(|e| e.id).collect();
    (p, si, ids[0], ids[1..].to_vec())
}

/// Where the ends of line `eid` stand, x of the start and of the end.
fn ends_x(p: &Project, si: usize, eid: u64) -> (f64, f64) {
    let e = p.sketches[si].entities.iter().find(|e| e.id == eid).expect("the line");
    let Some(qymcad_core::model::LineParts { a, b }) = e.kind.as_line() else { panic!("not a line") };
    let x = |id| p.sketches[si].points.iter().find(|q| q.id == id).map(|q| q.x).expect("the point");
    (x(a), x(b))
}

/// THE ASK: the pointer at x on the axis, over the vertical line `over` of those made, which ends.
struct Case {
    name: &'static str,
    across: &'static [f64],
    pointer_x: f64,
    over: Option<usize>,
    sides: ExtendSides,
    ends: (f64, f64),
}

#[test]
fn a_line_is_extended_to_where_it_is_asked() {
    let cases = [
        Case { name: "the nearer end to the nearest", across: &[-15.0, 20.0, 30.0], pointer_x: 9.0, over: None, sides: ExtendSides::Nearer, ends: (0.0, 20.0) },
        Case { name: "the start, the pointer nearer it", across: &[-15.0, 20.0, 30.0], pointer_x: 1.0, over: None, sides: ExtendSides::Nearer, ends: (-15.0, 10.0) },
        Case { name: "to the farther line the pointer is over", across: &[-15.0, 20.0, 30.0], pointer_x: 30.0, over: Some(2), sides: ExtendSides::Nearer, ends: (0.0, 30.0) },
        Case { name: "both sides, each to its nearest", across: &[-15.0, 20.0, 30.0], pointer_x: 5.0, over: None, sides: ExtendSides::Both, ends: (-15.0, 20.0) },
        Case { name: "both sides, the far one taken on its side", across: &[-15.0, 20.0, 30.0], pointer_x: 30.0, over: Some(2), sides: ExtendSides::Both, ends: (-15.0, 30.0) },
        Case { name: "both sides, nothing before the start", across: &[20.0, 30.0], pointer_x: 5.0, over: None, sides: ExtendSides::Both, ends: (0.0, 20.0) },
        Case { name: "the start, nothing on its side", across: &[20.0], pointer_x: 1.0, over: None, sides: ExtendSides::Nearer, ends: (0.0, 10.0) },
    ];
    let mut failures = Vec::new();
    for case in cases {
        let (mut p, si, line, across) = lines(case.across);
        let ask = ExtendAsk { pointer: Point2::new(case.pointer_x, 0.0), over: case.over.map(|k| across[k]), sides: case.sides };
        if let Some(ext) = p.curve_extension(si, line, &ask) {
            p.extend_curve_by(si, line, &ext);
        }
        let got = ends_x(&p, si, line);
        if (got.0 - case.ends.0).abs() > 1e-6 || (got.1 - case.ends.1).abs() > 1e-6 {
            failures.push(format!("{}: the line runs {got:?}, wanted {:?}", case.name, case.ends));
        }
    }
    assert!(failures.is_empty(), "a line extended as asked:\n{}", failures.join("\n"));
}

/// AN END JOINED TO OTHER GEOMETRY STAYS, and Extend says why: the line (0, 0) - (10, 0) with a line across at 20, its
/// end (10, 0) the end of another line too, or tied by a dimension. Asked to extend that end, the line keeps it -
/// `EndKept::Joined` - and nothing in the sketch moves; its free end still extends. Reported behaviour: Extend on a side
/// whose end is the end of a fillet moved the drawing and extended nothing.
#[test]
fn an_end_joined_to_other_geometry_is_not_extended() {
    use qymcad_core::model::{Constraint, EndKept};
    let mut failures = Vec::new();
    for joined in ["another line ends there", "a dimension holds it"] {
        let (mut p, si, line, _) = lines(&[-15.0, 20.0]);
        let Some(qymcad_core::model::LineParts { a, b }) = p.sketches[si].entities.iter().find(|e| e.id == line).expect("the line").kind.as_line() else { panic!("a line") };
        if joined == "another line ends there" {
            let free = p.sketch_point_at(si, 10.0, 8.0, 1e-9);
            let id = p.alloc_id();
            p.sketches[si].entities.push(qymcad_core::model::SketchEntity { id, kind: qymcad_core::model::LineParts { a: b, b: free }.into(), construction: false });
        } else {
            p.sketches[si].constraints.push(Constraint::Distance { a, b, d: 10.0, off: 3.0, expr: String::new(), driven: false, axis: 0, at: None });
        }
        let places = |p: &Project| p.sketches[si].points.iter().map(|q| (q.id, q.x.to_bits(), q.y.to_bits())).collect::<Vec<_>>();
        let before = places(&p);
        let ask = ExtendAsk { pointer: Point2::new(9.0, 0.0), over: None, sides: ExtendSides::Nearer };
        let ext = p.curve_extension(si, line, &ask).expect("the line is open");
        if !ext.goes.is_empty() || ext.kept != EndKept::Joined {
            failures.push(format!("{joined}: the joined end is asked to go {:?}, kept {:?}", ext.goes, ext.kept));
        }
        if p.extend_curve_by(si, line, &ext) || places(&p) != before {
            failures.push(format!("{joined}: extending the joined end moved the sketch"));
        }
        let free = p.curve_extension(si, line, &ExtendAsk { pointer: Point2::new(1.0, 0.0), over: None, sides: ExtendSides::Nearer }).expect("the line is open");
        let start = free.curve.at(0.0);
        if joined == "another line ends there" && ((start.x + 15.0).abs() > 1e-9 || start.y.abs() > 1e-9) {
            failures.push(format!("{joined}: the free end (0, 0) is not extended to the line at -15: it starts at {start:?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// AN ARC IS EXTENDED AS A LINE IS: the arc of radius 10 about the origin from 0 to 90 deg, and upright lines at x = -5
/// and x = -8 and a level line at y = -6 across its circle. Both sides take each end to its nearest - the finish round
/// to x = -5, the start down to y = -6; the pointer over the farther upright takes the finish there and leaves the start.
/// The same for an arc of an ellipse 10 by 6.
#[test]
fn an_arc_and_an_arc_of_an_ellipse_are_extended_as_asked() {
    use qymcad_core::feature::Winding;
    use qymcad_core::model::curve::Curve;
    use qymcad_core::model::EllipseArcSpec;
    /// A CURVE OF THE CASE: its name and how it is laid, answering its id.
    struct Kind {
        name: &'static str,
        lay: fn(&mut Project, usize) -> u64,
    }
    fn arc(p: &mut Project, si: usize) -> u64 {
        p.add_arc_entity(si, Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), Point2::new(0.0, 10.0), Winding::Ccw, Purpose::Real);
        p.sketches[si].entities.last().expect("the arc").id
    }
    fn ellipse_arc(p: &mut Project, si: usize) -> u64 {
        let spec = EllipseArcSpec { centre: Point2::new(0.0, 0.0), radii: Point2::new(10.0, 6.0), rot: 0.0, angles: [0.0, std::f64::consts::FRAC_PI_2], winding: Winding::Ccw };
        p.add_ellipse_arc_entity(si, spec, Purpose::Real)
    }
    /// WHAT IS ASKED: which ends, and whether the pointer is over the farther upright.
    #[derive(Clone, Copy, Debug)]
    enum Asked {
        BothSides,
        OverTheFartherLine,
    }
    let mut failures = Vec::new();
    for kind in [Kind { name: "an arc", lay: arc }, Kind { name: "an arc of an ellipse", lay: ellipse_arc }] {
        for asked in [Asked::BothSides, Asked::OverTheFartherLine] {
            let mut p = Project::default();
            p.new_document();
            let si = p.new_sketch("S");
            let curve = (kind.lay)(&mut p, si);
            p.add_line_entity(si, -5.0, -20.0, -5.0, 20.0, Purpose::Real);
            let far = p.add_line_entity(si, -8.0, -20.0, -8.0, 20.0, Purpose::Real);
            p.add_line_entity(si, -20.0, -6.0, 20.0, -6.0, Purpose::Real);
            let ask = match asked {
                Asked::BothSides => ExtendAsk { pointer: Point2::new(5.0, 5.0), over: None, sides: ExtendSides::Both },
                Asked::OverTheFartherLine => ExtendAsk { pointer: Point2::new(-8.0, 5.0), over: Some(far), sides: ExtendSides::Nearer },
            };
            let Some(ext) = p.curve_extension(si, curve, &ask) else {
                failures.push(format!("{} {asked:?}: no extension", kind.name));
                continue;
            };
            p.extend_curve_by(si, curve, &ext);
            let s = &p.sketches[si];
            let [start, finish] = Curve::of(s, s.entity(curve).expect("the curve")).expect("its curve").ends();
            let (finish_x, start_ok) = match asked {
                Asked::BothSides => (-5.0, (start.y + 6.0).abs() < 1e-6),
                Asked::OverTheFartherLine => (-8.0, start.dist(Point2::new(10.0, 0.0)) < 1e-6),
            };
            if (finish.x - finish_x).abs() > 1e-6 || !start_ok {
                failures.push(format!("{} {asked:?}: it runs from {start:?} to {finish:?}", kind.name));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// AN ARC WHOSE END IS JOINED TO OTHER GEOMETRY IS NOT EXTENDED from that end, as a line is not: a line (0, 0) - (20, 0)
/// held at 20 long, an arc about (5, -15) starting on the line's left end, and a line at y = 0.5 across the way of the
/// arc. Reported behaviour: Extend near the joined end answered "Done", moved the line to (0.936, 0.260) - (20.936,
/// 0.260) and the centre to (5.006, -15.019), and the end of the arc stopped at y = 0.260, short of the line it was
/// extended to.
#[test]
fn an_arc_whose_end_is_joined_to_a_line_is_not_extended() {
    use qymcad_core::feature::Winding;
    use qymcad_core::model::{Constraint, EndKept};
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    let line = p.add_line_entity(si, 0.0, 0.0, 20.0, 0.0, Purpose::Real);
    let Some(qymcad_core::model::LineParts { a, b }) = p.sketches[si].entities.iter().find(|e| e.id == line).expect("the line").kind.as_line() else { panic!("a line") };
    p.sketches[si].constraints.push(Constraint::Distance { a, b, d: 20.0, off: 3.0, expr: String::new(), driven: false, axis: 0, at: None });
    p.add_line_entity(si, -30.0, 0.5, 30.0, 0.5, Purpose::Real);
    let r = 250f64.sqrt();
    p.add_arc_entity(si, Point2::new(5.0, -15.0), Point2::new(0.0, 0.0), Point2::new(5.0 - r, -15.0), Winding::Ccw, Purpose::Real);
    let arc = p.sketches[si].entities.last().expect("the arc").id;
    let Some(parts) = p.sketches[si].entities.iter().find(|e| e.id == arc).expect("the arc").kind.as_arc() else { panic!("an arc") };
    assert!(parts.a == a || parts.b == a, "the arc does not start on the end of the line: the case is not joined");
    let places = |p: &Project| p.sketches[si].points.iter().map(|q| (q.id, q.x.to_bits(), q.y.to_bits())).collect::<Vec<_>>();
    let before = places(&p);
    let ask = ExtendAsk { pointer: Point2::new(-0.5, -0.3), over: None, sides: ExtendSides::Nearer };
    let ext = p.curve_extension(si, arc, &ask).expect("the arc is open");
    let mut failures = Vec::new();
    if !ext.goes.is_empty() || ext.kept != EndKept::Joined {
        failures.push(format!("the joined end is asked to go {:?}, kept {:?}", ext.goes, ext.kept));
    }
    if p.extend_curve_by(si, arc, &ext) || places(&p) != before {
        failures.push("extending the joined end of the arc moved the sketch".to_string());
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
