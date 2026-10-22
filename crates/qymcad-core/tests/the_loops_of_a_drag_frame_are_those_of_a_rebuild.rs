//! THE LOOPS OF A DRAG FRAME ARE THOSE OF A REBUILD: a frame of a drag rebuilds only the loops of what it moved
//! (`regen_sketch_moved`); after every frame the loops of the sketch are the loops a whole rebuild gives - the same
//! entities, closed or open, at the same place and size - and a loop far from the drag keeps its id.
mod sketch_kinds;

use qymcad_core::model::{Project, SketchEntity, SketchPoint};
use sketch_kinds::{build, Kind};

/// The loops of a sketch as a sorted list of what each is: closed, its entities, its centroid and area to 1e-6.
fn loops(p: &Project, si: usize) -> Vec<String> {
    let mut out: Vec<String> = p.sketches[si]
        .contour_ids
        .iter()
        .filter_map(|cid| {
            let c = &p.contours[p.contour_index(*cid)?];
            let mut src = c.edge_src.clone();
            src.sort_unstable();
            src.dedup();
            let n = c.points.len().max(1) as f64;
            let (sx, sy) = c.points.iter().fold((0.0, 0.0), |(a, b), q| (a + q.x, b + q.y));
            Some(format!("{} {:?} ({:.6}, {:.6}) {:.6}", c.closed, src, sx / n, sy / n, c.signed_area().abs()))
        })
        .collect();
    out.sort();
    out
}

/// Drag the point `id` along `path`, a frame at each place, and after each frame compare the loops with a whole rebuild.
fn dragged_as_rebuilt(mut p: Project, id: u64, path: &[(f64, f64)], what: &str, failures: &mut Vec<String>) -> Project {
    for (k, &(x, y)) in path.iter().enumerate() {
        let _ = p.solve_sketch_drag_fast(0, Some((id, x, y)));
        let mut whole = p.clone();
        whole.regen_sketch_whole(0);
        let (framed, rebuilt) = (loops(&p, 0), loops(&whole, 0));
        if framed != rebuilt {
            failures.push(format!("{what}, frame {k}: {} loops against {} of a rebuild; first apart: {:?}", framed.len(), rebuilt.len(), framed.iter().zip(&rebuilt).find(|(a, b)| a != b)));
        }
    }
    p
}

#[test]
fn every_frame_of_a_drag_leaves_the_loops_of_a_rebuild() {
    let mut failures = Vec::new();
    for kind in [Kind::Lines, Kind::Rectangles, Kind::Circles, Kind::Hexagons, Kind::Tangents, Kind::Array, Kind::Mixed] {
        let mut p = build(kind, 40);
        p.solve_sketch(0);
        let far = p.sketches[0].contour_ids.last().copied();
        let q = p.sketches[0].points[0];
        let path: Vec<(f64, f64)> = (1..=5).map(|k| (q.x + k as f64 * 3.0, q.y + k as f64 * 2.0)).collect();
        let p = dragged_as_rebuilt(p, q.id, &path, &format!("{kind:?}"), &mut failures);
        if let Some(far) = far.filter(|_| kind != Kind::Array) {
            if !p.sketches[0].contour_ids.contains(&far) {
                failures.push(format!("{kind:?}: the loop farthest from the drag lost its id"));
            }
        }
    }
    assert!(failures.is_empty(), "the loops of a drag frame and of a rebuild differ:\n{}", failures.join("\n"));
}

#[test]
fn a_segment_dragged_across_a_rectangle_cuts_it_and_leaves_it_whole_again() {
    let mut p = build(Kind::Rectangles, 4);
    // a segment beside the first rectangle (10 x 6 from (0, 0)), its end then led across it and out again
    let s = &mut p.sketches[0];
    let next = s.points.iter().map(|q| q.id).chain(s.entities.iter().map(|e| e.id)).max().unwrap_or(0) + 1;
    let (a, b) = (next, next + 1);
    s.points.push(SketchPoint { id: a, x: 5.0, y: -5.0 });
    s.points.push(SketchPoint { id: b, x: 5.0, y: -2.0 });
    s.entities.push(SketchEntity { id: next + 2, kind: qymcad_core::model::LineParts { a, b }.into(), construction: false });
    p.regen_sketch(0);
    let mut failures = Vec::new();
    let path = [(5.0, 3.0), (5.0, 12.0), (5.0, 3.0), (5.0, -2.0)];
    let p = dragged_as_rebuilt(p, b, &path, "a segment across a rectangle", &mut failures);
    let closed = loops(&p, 0).iter().filter(|l| l.starts_with("true")).count();
    assert!(failures.is_empty(), "the loops of a drag frame and of a rebuild differ:\n{}", failures.join("\n"));
    assert_eq!(closed, 4, "after the segment went out again, four whole rectangles");
}

#[test]
fn a_side_dragged_off_a_square_of_separate_lines_leaves_an_open_chain() {
    // four lines each with ends of its own standing on the next one's: one closed loop; the end of one side led away,
    // the four are an open chain - found from the side dragged along the ends that stand together - and the loops of
    // every frame are those of a rebuild
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    // the points laid by hand, two at each corner, as a drawing brought in lays them
    let c = [(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)];
    let s = &mut p.sketches[si];
    let mut end = 0;
    for i in 0..4 {
        let (a, b, e) = (1000 + 3 * i as u64, 1001 + 3 * i as u64, 1002 + 3 * i as u64);
        s.points.push(SketchPoint { id: a, x: c[i].0, y: c[i].1 });
        s.points.push(SketchPoint { id: b, x: c[(i + 1) % 4].0, y: c[(i + 1) % 4].1 });
        s.entities.push(SketchEntity { id: e, kind: qymcad_core::model::LineParts { a, b }.into(), construction: false });
        if i == 0 {
            end = b;
        }
    }
    p.regen_sketch(si);
    assert_eq!(loops(&p, 0).iter().filter(|l| l.starts_with("true")).count(), 1, "the four separate lines make one closed loop");
    let mut failures = Vec::new();
    let p = dragged_as_rebuilt(p, end, &[(22.0, -3.0), (30.0, -10.0)], "a side dragged off a square", &mut failures);
    let open = loops(&p, 0).iter().filter(|l| l.starts_with("false")).count();
    assert!(failures.is_empty(), "the loops of a drag frame and of a rebuild differ:\n{}", failures.join("\n"));
    assert!(open >= 1, "the three sides left are no open chain");
}
