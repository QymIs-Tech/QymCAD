//! A CORNER OF ANY CURVES IS CUT BY ONE PATH, keeping what a corner of two lines keeps: a level line ending on a spline -
//! the fillet carries the level onto the line as it is cut back; a symmetric chamfer holds its two legs equal; a chamfer
//! of a leg and an angle keeps its angle as a dimension. The same for a line ending on an arc and on a line.
use qymcad_core::feature::{ChamferMode, Ends, Purpose, Winding};
use qymcad_core::geom::Point2;
use qymcad_core::model::{ChamferLegs, Constraint, CornerAt, CornerCut, FilletSize, Id, Project};

/// THE SIDE A LEVEL LINE FROM (0, 0) TO (20, 0) ENDS ON, at (20, 0).
#[derive(Clone, Copy, Debug)]
enum Other {
    Line,
    Arc,
    Spline,
}

/// A CORNER DRAWN: the drawing, the level line, and the corner at (20, 0).
struct Drawn {
    p: Project,
    line: Id,
    corner: CornerAt,
}

fn drawn(other: Other) -> Drawn {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    let line = p.add_line_entity(si, 0.0, 0.0, 20.0, 0.0, Purpose::Real);
    let side = match other {
        Other::Line => p.add_line_entity(si, 20.0, 0.0, 20.0, 20.0, Purpose::Real),
        Other::Arc => {
            p.add_arc_entity(si, Point2::new(30.0, 0.0), Point2::new(20.0, 0.0), Point2::new(30.0, 10.0), Winding::Cw, Purpose::Real);
            p.sketches[si].entities.iter().find(|e| e.kind.as_arc().is_some()).map(|e| e.id).expect("the arc")
        }
        Other::Spline => p.add_spline(si, vec![Point2::new(20.0, 0.0), Point2::new(24.0, 10.0), Point2::new(22.0, 20.0)], Ends::Open, Purpose::Real).expect("the spline"),
    };
    p.merge_close_points(si, 1e-9);
    let s = &p.sketches[si];
    let ends = s.entity(line).and_then(|e| e.kind.as_line()).expect("the line");
    p.sketches[si].constraints.push(Constraint::Horizontal { a: ends.a, b: ends.b });
    p.regen_sketch(si);
    let corner = CornerAt::of(ends.b, [line, side]);
    Drawn { p, line, corner }
}

#[test]
fn a_corner_of_a_line_and_any_curve_keeps_what_a_corner_of_two_lines_keeps() {
    let mut failures = Vec::new();
    for other in [Other::Line, Other::Arc, Other::Spline] {
        // the fillet: the level goes with the line, now from (0, 0) to where the arc touches it
        let Drawn { mut p, line, corner } = drawn(other);
        if p.cut_corner_set(0, &[corner], CornerCut::Fillet(FilletSize::radius(2.0)), None) != 1 {
            failures.push(format!("{other:?}: not rounded"));
        } else {
            let s = &p.sketches[0];
            let ends = s.entity(line).and_then(|e| e.kind.as_line()).expect("the line");
            let level = s.constraints.iter().any(|c| matches!(*c, Constraint::Horizontal { a, b } if (a == ends.a && b == ends.b) || (a == ends.b && b == ends.a)));
            if !level {
                failures.push(format!("{other:?}: the fillet left the level of the line on the corner it took off: {:?}", s.constraints));
            }
        }
        // a symmetric chamfer: its two legs from the sharp held equal
        let Drawn { mut p, corner, .. } = drawn(other);
        if p.cut_corner_set(0, &[corner], CornerCut::Chamfer(ChamferLegs::equal(3.0)), None) != 1 {
            failures.push(format!("{other:?}: no symmetric chamfer"));
        } else if !p.sketches[0].constraints.iter().any(|c| matches!(*c, Constraint::Equal { a, c: cc, .. } if a == corner.point && cc == corner.point)) {
            failures.push(format!("{other:?}: the symmetric chamfer does not hold its legs equal"));
        }
        // a chamfer of a leg and an angle: the angle kept as a dimension
        let Drawn { mut p, corner, .. } = drawn(other);
        let legs = ChamferLegs { mode: ChamferMode::DistAngle, first: 3.0, second: 40.0 };
        if p.cut_corner_set(0, &[corner], CornerCut::Chamfer(legs), None) != 1 {
            failures.push(format!("{other:?}: no chamfer of a leg and an angle"));
        } else if !p.sketches[0].constraints.iter().any(|c| matches!(*c, Constraint::AngleLines { deg, .. } if (deg - 40.0).abs() < 1e-9)) {
            failures.push(format!("{other:?}: the chamfer of a leg and an angle keeps no angle"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
