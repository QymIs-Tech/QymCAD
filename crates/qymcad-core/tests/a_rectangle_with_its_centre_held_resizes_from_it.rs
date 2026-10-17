//! A RECTANGLE WITH ITS CENTRE HELD RESIZES FROM IT: drawn from a corner, its centre fixed or set by dimensions to the
//! origin, a corner dragged resizes it about the centre - the centre does not move, the corner across goes the other
//! way. With the centre free a corner dragged still stretches it from the corner across. Both ways a frame is solved:
//! through the drag session and alone.
//!
//! Reported behaviour: "a rectangle drawn from a corner, its centre fixed - none of its corners can be dragged to change
//! its size; with the centre let go it drags as it should".
use qymcad_core::feature::Purpose;
use qymcad_core::model::{Constraint, Project};

/// HOW THE CENTRE OF THE RECTANGLE IS HELD.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Hold {
    Fixed,
    DimensionsToTheOrigin,
    Free,
}

/// HOW A FRAME OF THE DRAG IS SOLVED.
#[derive(Clone, Copy, Debug)]
enum Frame {
    Session,
    Alone,
}

/// A rectangle drawn by its corners from (20, 10) to (80, 70) - its centre at (50, 40) - held as `hold`.
fn rectangle(hold: Hold) -> (Project, usize) {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    p.add_rect_entity(si, 20.0, 10.0, 80.0, 70.0, Purpose::Real);
    let centre = p.sketches[si].rects[0].centre;
    match hold {
        Hold::Fixed => p.sketches[si].constraints.push(Constraint::Fixed { p: centre }),
        Hold::DimensionsToTheOrigin => {
            let origin = p.ensure_frame(si);
            for (axis, d) in [(1, 50.0), (2, 40.0)] {
                p.sketches[si].constraints.push(Constraint::Distance { dimension: Default::default(), a: origin, b: centre, d, off: 5.0, expr: String::new(), driven: false, axis, at: None });
            }
        }
        Hold::Free => {}
    }
    p.solve_sketch(si);
    (p, si)
}

fn at(p: &Project, si: usize, id: u64) -> (f64, f64) {
    p.sketches[si].points.iter().find(|q| q.id == id).map(|q| (q.x, q.y)).expect("the point")
}

/// The upper right corner (80, 70) dragged 10 right and 6 up in five frames, as `frame` solves them, then let go.
fn dragged(hold: Hold, frame: Frame) -> (Project, usize) {
    let (mut p, si) = rectangle(hold);
    let r = p.sketches[si].rects[0].clone();
    let corner = *r.corners.iter().find(|&&k| at(&p, si, k) == (80.0, 70.0)).expect("the corner (80, 70)");
    for step in 1..=5 {
        let t = f64::from(step) / 5.0;
        let to = Some((corner, 80.0 + 10.0 * t, 70.0 + 6.0 * t));
        match frame {
            Frame::Session => p.solve_sketch_drag_fast(si, to),
            Frame::Alone => p.solve_sketch_drag_frame_alone(si, to),
        };
    }
    p.solve_sketch(si);
    (p, si)
}

fn near(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3
}

#[test]
fn a_corner_dragged_resizes_a_rectangle_about_its_held_centre() {
    let mut failures = Vec::new();
    for hold in [Hold::Fixed, Hold::DimensionsToTheOrigin, Hold::Free] {
        for frame in [Frame::Session, Frame::Alone] {
            let (p, si) = dragged(hold, frame);
            let r = &p.sketches[si].rects[0];
            let corners: Vec<(f64, f64)> = r.corners.iter().map(|&k| at(&p, si, k)).collect();
            let has = |want: (f64, f64)| corners.iter().any(|&c| near(c, want));
            let centre = at(&p, si, r.centre);
            // held: the centre stays and the corner across goes the other way; free: the corner across stays
            let (want_centre, across) = if hold == Hold::Free { ((55.0, 43.0), (20.0, 10.0)) } else { ((50.0, 40.0), (10.0, 4.0)) };
            if !near(centre, want_centre) || !has((90.0, 76.0)) || !has(across) {
                failures.push(format!(
                    "{hold:?}, a frame {frame:?}: the corners {corners:?} about {centre:?}; wanted the dragged one at (90, 76), the one across at {across:?}, the centre at {want_centre:?}"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "a corner of a rectangle drawn from a corner, dragged:\n{}", failures.join("\n"));
}
