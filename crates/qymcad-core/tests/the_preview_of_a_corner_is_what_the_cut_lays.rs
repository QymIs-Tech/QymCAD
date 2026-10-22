//! THE PREVIEW OF A CORNER IS WHAT THE CUT LAYS: the arc of a fillet and the ends of a chamfer shown before Enter stand
//! where the operation puts them, for a corner of two lines as for any other - a radius that takes a side whole
//! included, which the cut lays and the preview must show.
use qymcad_core::feature::{ChamferMode, Purpose};
use qymcad_core::geom::Point2;
use qymcad_core::model::curve::Curve;
use qymcad_core::model::{ChamferLegs, CornerAt, CornerCut, FilletSize, Project};

/// A CORNER OF TWO LINES at the origin: the drawing, its sketch and the corner.
struct Drawn {
    p: Project,
    si: usize,
    corner: CornerAt,
}

/// The line to (6, 0) and the line to `far`, their ends at the origin one point.
fn drawn(far: [f64; 2]) -> Drawn {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    let a = p.add_line_entity(si, 6.0, 0.0, 0.0, 0.0, Purpose::Real);
    let b = p.add_line_entity(si, 0.0, 0.0, far[0], far[1], Purpose::Real);
    p.merge_close_points(si, 1e-9);
    let s = &p.sketches[si];
    let point = s.points.iter().find(|q| q.x.hypot(q.y) < 1e-9).map(|q| q.id).expect("the corner");
    Drawn { p, si, corner: CornerAt::of(point, [a, b]) }
}

fn p2(at: [f64; 2]) -> Point2 {
    Point2::new(at[0], at[1])
}

/// The newest curve of the sketch that `pick` takes: the one the cut laid.
fn laid(p: &Project, si: usize, pick: fn(&Curve) -> bool) -> Option<Curve> {
    let s = &p.sketches[si];
    s.entities.iter().filter(|e| Curve::of(s, e).is_some_and(|c| pick(&c))).max_by_key(|e| e.id).and_then(|e| Curve::of(s, e))
}

/// Whether the two ends stand at the same two places, in either order.
fn same_ends([a, b]: [[Point2; 2]; 2]) -> bool {
    let near = |x: Point2, y: Point2| x.dist(y) < 1e-6;
    (near(a[0], b[0]) && near(a[1], b[1])) || (near(a[0], b[1]) && near(a[1], b[0]))
}

/// A FILLET TO SHOW AND LAY: what it is, the far end of the second line, the radius.
struct Fillet {
    name: &'static str,
    far: [f64; 2],
    r: f64,
}

#[test]
fn the_preview_of_a_corner_of_two_lines_is_what_the_cut_lays() {
    let mut failures = Vec::new();
    let sixty = [3.0, 3.0 * 3f64.sqrt()];
    // a fillet of 2 at 60 deg, and one of 4 on a square corner whose upright side is 4 long - the arc takes it whole
    let fillets = [Fillet { name: "a fillet of 2 at 60 deg", far: sixty, r: 2.0 }, Fillet { name: "a fillet of 4 taking the side of 4 whole", far: [0.0, 4.0], r: 4.0 }];
    for Fillet { name, far, r } in fillets {
        let Drawn { mut p, si, corner } = drawn(far);
        let cut = CornerCut::Fillet(FilletSize::radius(r));
        let Some(shown) = p.corner_blend(si, corner.point, corner.pair, cut) else {
            failures.push(format!("{name}: no preview"));
            continue;
        };
        if p.cut_corner_set(si, &[corner], cut, None) != 1 {
            failures.push(format!("{name}: not rounded"));
            continue;
        }
        let arc = laid(&p, si, |c| matches!(c, Curve::Round { .. }));
        match (arc, shown.arc) {
            (Some(Curve::Round { centre, r: laid_r, .. }), Some((c, shown_r))) if centre.dist(p2(c)) < 1e-6 && (laid_r - shown_r).abs() < 1e-6 => {}
            (arc, shown_arc) => failures.push(format!("{name}: the arc laid {arc:?}, the arc shown {shown_arc:?}")),
        }
        let ends = laid(&p, si, |c| matches!(c, Curve::Round { .. })).map(|c| c.ends());
        if !ends.is_some_and(|e| same_ends([e, shown.ends.map(p2)])) {
            failures.push(format!("{name}: the arc laid ends at {ends:?}, the preview at {:?}", shown.ends));
        }
    }
    // a chamfer of two legs, 2 and 3, at 60 deg: the cut laid between the ends shown
    let Drawn { mut p, si, corner } = drawn(sixty);
    let cut = CornerCut::Chamfer(ChamferLegs { mode: ChamferMode::TwoDist, first: 2.0, second: 3.0 });
    match p.corner_blend(si, corner.point, corner.pair, cut) {
        None => failures.push("a chamfer of 2 and 3: no preview".to_string()),
        Some(shown) => {
            let made = p.cut_corner_set(si, &[corner], cut, None) == 1;
            let line = laid(&p, si, |c| matches!(c, Curve::Line { .. })).map(|c| c.ends());
            if !made || !line.is_some_and(|e| same_ends([e, shown.ends.map(p2)])) {
                failures.push(format!("a chamfer of 2 and 3: the cut laid {line:?}, the preview {:?}", shown.ends));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
