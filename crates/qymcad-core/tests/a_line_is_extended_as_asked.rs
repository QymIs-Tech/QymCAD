//! A LINE IS EXTENDED AS THE EXTEND TOOL ASKS: the nearer end to the nearest curve across its axis, or to the curve the
//! pointer is over; both ends with "Both sides", each to its own; an end with nothing on its side stays.
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
    let qymcad_core::model::EntityKind::Line { a, b } = e.kind else { panic!("not a line") };
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
        let ext = p.line_extension(si, line, &ask);
        p.extend_line_by(si, line, ext);
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
    use qymcad_core::model::{Constraint, EndKept, EntityKind};
    let mut failures = Vec::new();
    for joined in ["another line ends there", "a dimension holds it"] {
        let (mut p, si, line, _) = lines(&[-15.0, 20.0]);
        let EntityKind::Line { a, b } = p.sketches[si].entities.iter().find(|e| e.id == line).expect("the line").kind else { panic!("a line") };
        if joined == "another line ends there" {
            let free = p.sketch_point_at(si, 10.0, 8.0, 1e-9);
            let id = p.alloc_id();
            p.sketches[si].entities.push(qymcad_core::model::SketchEntity { id, kind: EntityKind::Line { a: b, b: free }, construction: false });
        } else {
            p.sketches[si].constraints.push(Constraint::Distance { a, b, d: 10.0, off: 3.0, expr: String::new(), driven: false, axis: 0, at: None });
        }
        let places = |p: &Project| p.sketches[si].points.iter().map(|q| (q.id, q.x.to_bits(), q.y.to_bits())).collect::<Vec<_>>();
        let before = places(&p);
        let ask = ExtendAsk { pointer: Point2::new(9.0, 0.0), over: None, sides: ExtendSides::Nearer };
        let ext = p.line_extension(si, line, &ask);
        if ext.b.is_some() || ext.kept != EndKept::Joined {
            failures.push(format!("{joined}: the joined end is asked to go to {:?}, kept {:?}", ext.b, ext.kept));
        }
        if p.extend_line_by(si, line, ext) || places(&p) != before {
            failures.push(format!("{joined}: extending the joined end moved the sketch"));
        }
        let free = p.line_extension(si, line, &ExtendAsk { pointer: Point2::new(1.0, 0.0), over: None, sides: ExtendSides::Nearer });
        if joined == "another line ends there" && free.a != Some(-1.5) {
            failures.push(format!("{joined}: the free end (0, 0) is not extended to the line at -15: {:?}", free.a));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
