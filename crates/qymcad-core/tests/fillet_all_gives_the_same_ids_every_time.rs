//! "FILLET ALL CORNERS" GIVES THE SAME IDS EVERY TIME.
//!
//! The corners were counted in a hash map and filleted in its order, which is chosen at random for every map; each
//! corner takes its ids (two tangency points, a centre, an arc) in the order it is reached, so the same edit gave the
//! arcs other ids from one run to the next - eight different assignments in twelve runs on a 100 x 4 rectangle - and
//! the document saved another file. The corners are now taken by id.
use qymcad_core::geom::Point2;
use qymcad_core::model::{EntityKind, Project};

/// One rounded corner: where its arc's centre stands, in whole micrometres so that it compares exactly, and the id the
/// arc took.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct RoundedCorner {
    centre_x_um: i64,
    centre_y_um: i64,
    arc: u64,
}

/// The corners of a 100 x 4 rectangle with every corner rounded at `r`, in the order of their centres.
fn arcs_by_corner(r: f64) -> Vec<RoundedCorner> {
    let mut p = Project::default();
    let sid = p.add_line_sketch("thin", vec![Point2::new(0.0, 0.0), Point2::new(100.0, 0.0), Point2::new(100.0, 4.0), Point2::new(0.0, 4.0)], true);
    let si = p.sketch_index(sid).expect("the sketch");
    assert_eq!(p.fillet_all_corners_of(si, r, None), 4, "every corner of the rectangle is rounded");
    let s = &p.sketches[si];
    let mut arcs: Vec<RoundedCorner> = s
        .entities
        .iter()
        .filter_map(|e| match e.kind {
            EntityKind::Arc { center, .. } => {
                let q = s.points.iter().find(|q| q.id == center).expect("the centre");
                Some(RoundedCorner { centre_x_um: (q.x * 1e3).round() as i64, centre_y_um: (q.y * 1e3).round() as i64, arc: e.id })
            }
            _ => None,
        })
        .collect();
    arcs.sort();
    arcs
}

#[test]
fn the_same_edit_gives_the_arcs_the_same_ids() {
    let first = arcs_by_corner(1.5);
    for run in 1..12 {
        assert_eq!(arcs_by_corner(1.5), first, "run {run} gave the arcs other ids than the first");
    }
}
