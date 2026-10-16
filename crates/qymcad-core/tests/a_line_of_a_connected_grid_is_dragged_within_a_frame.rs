//! A LINE OF A CONNECTED GRID IS DRAGGED WITHIN A FRAME: a horizontal line and a vertical one, each patterned 200 times
//! 20 mm apart, make one grid of 40 000 cells where every line crosses every line across it. A frame of a drag of one
//! line makes again the cells round that line, not the 40 000, and the loops after every frame are those of a whole
//! rebuild.
//!
//! Reported behaviour (#95): in such a grid every drag and every edit waited. The loops of what moved were made again
//! with every curve whose box met one of theirs, and so on - in a connected grid that is every curve: 0.7 s a frame
//! in a release build.
use qymcad_core::feature::Purpose;
use qymcad_core::model::{EntityKind, PatternKind, Project};
use std::time::{Duration, Instant};

/// The grid of the report, `n` lines each way.
fn grid(n: u32) -> Project {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    let len = 20.0 * n as f64;
    let h = p.add_line_entity(si, 0.0, -10.0, len, -10.0, Purpose::Real);
    let v = p.add_line_entity(si, -10.0, 0.0, -10.0, len, Purpose::Real);
    p.add_pattern(si, &[h], PatternKind::Linear { dx: 0.0, dy: 20.0, count: n, dx2: 0.0, dy2: 0.0, count2: 0 });
    p.add_pattern(si, &[v], PatternKind::Linear { dx: 20.0, dy: 0.0, count: n, dx2: 0.0, dy2: 0.0, count2: 0 });
    p.solve_sketch(si);
    p
}

/// The loops of a sketch as a sorted list of what each is: closed, its entities, its centroid and area to 1e-6.
fn loops(p: &Project) -> Vec<String> {
    let mut out: Vec<String> = p.sketches[0]
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

/// Which lines of the grid: those across (horizontal), counted up, or those up (vertical), counted right.
#[derive(Clone, Copy, Debug)]
enum Way {
    Across,
    Up,
}

/// A line of the grid, and its second end where it stands.
#[derive(Clone, Copy, Debug)]
struct Line {
    id: u64,
    end: u64,
    x: f64,
    y: f64,
}

/// The line `k` of the grid going `way`.
fn line(p: &Project, way: Way, k: usize) -> Line {
    let s = &p.sketches[0];
    let at = |id: u64| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y)).unwrap_or_default();
    let mut lines: Vec<Line> = s
        .entities
        .iter()
        .filter(|e| !e.construction)
        .filter_map(|e| match e.kind {
            EntityKind::Line { a, b } => {
                let (pa, pb) = (at(a), at(b));
                let across = pa.1 == pb.1;
                matches!((way, across), (Way::Across, true) | (Way::Up, false)).then_some(Line { id: e.id, end: b, x: pb.0, y: pb.1 })
            }
            _ => None,
        })
        .collect();
    lines.sort_by(|a, b| match way {
        Way::Across => a.y.total_cmp(&b.y),
        Way::Up => a.x.total_cmp(&b.x),
    });
    lines[k]
}

/// Whether the loops of `p` are those of a whole rebuild; the difference put into `failures` under `what`.
fn as_rebuilt(p: &Project, what: &str, failures: &mut Vec<String>) {
    let mut whole = p.clone();
    whole.regen_sketch_whole(0);
    let (made, rebuilt) = (loops(p), loops(&whole));
    if made != rebuilt {
        failures.push(format!("{what}: {} loops against {} of a rebuild; first apart: {:?}", made.len(), rebuilt.len(), made.iter().zip(&rebuilt).find(|(a, b)| a != b)));
    }
}

#[test]
fn a_line_of_a_grid_is_dragged_and_its_loops_are_those_of_a_rebuild() {
    // the loops checked against a rebuild on a grid small enough to rebuild after every frame; every case: a line in
    // the middle, a line at the edge, a vertical one, the end led across the next line and back
    let mut failures = Vec::new();
    for (what, way, k) in [("a middle row", Way::Across, 5), ("the first row", Way::Across, 0), ("the last row", Way::Across, 11), ("a middle column", Way::Up, 6)] {
        let mut p = grid(12);
        let l = line(&p, way, k);
        let (dx, dy) = match way {
            Way::Across => (0.0, 1.0),
            Way::Up => (1.0, 0.0),
        };
        for (f, step) in [3.0, 9.0, 25.0, 9.0, -30.0, 0.0].into_iter().enumerate() {
            let _ = p.solve_sketch_drag_fast(0, Some((l.end, l.x + dx * step, l.y + dy * step)));
            as_rebuilt(&p, &format!("{what}, frame {f}"), &mut failures);
        }
    }
    assert!(failures.is_empty(), "the loops of a drag frame and of a rebuild differ:\n{}", failures.join("\n"));
}

#[test]
fn an_edit_of_a_grid_leaves_the_loops_of_a_rebuild() {
    // each edit through the door of the project the window calls, one after another on one grid
    let mut failures = Vec::new();
    let mut p = grid(12);
    let middle = line(&p, Way::Across, 5).id;
    p.move_entities(0, &[middle], 0.0, 7.0);
    as_rebuilt(&p, "a middle row moved up", &mut failures);
    p.move_entities(0, &[middle], 0.0, 25.0);
    as_rebuilt(&p, "the row moved across the next", &mut failures);
    let last = line(&p, Way::Up, 11).id;
    p.move_entities(0, &[last], 40.0, 0.0);
    as_rebuilt(&p, "the last column moved off the grid", &mut failures);
    let column = line(&p, Way::Up, 4).id;
    p.delete_entities(0, &[column]);
    as_rebuilt(&p, "a column deleted", &mut failures);
    let _ = p.add_line_entity(0, 5.0, 5.0, 115.0, 115.0, Purpose::Real);
    as_rebuilt(&p, "a line drawn across the cells", &mut failures);
    let row = line(&p, Way::Across, 2).id;
    let _ = p.toggle_construction(0, &[row]);
    as_rebuilt(&p, "a row made construction", &mut failures);
    let _ = p.toggle_construction(0, &[row]);
    as_rebuilt(&p, "the row drawn again", &mut failures);
    assert!(failures.is_empty(), "the loops of an edit and of a rebuild differ:\n{}", failures.join("\n"));
}

#[test]
fn a_frame_of_a_drag_in_a_grid_of_200_takes_a_frame() {
    let mut p = grid(200);
    let l = line(&p, Way::Across, 100);
    let mut frames = Vec::new();
    for step in [1.0, 2.0, 3.0, 4.0, 5.0] {
        let started = Instant::now();
        let _ = p.solve_sketch_drag_fast(0, Some((l.end, l.x, l.y + step)));
        frames.push(started.elapsed());
    }
    eprintln!("a line of a grid of 200 by 200 dragged: frames {frames:?}");
    // measured: 0.7 s a frame while every curve of the grid was taken with the line, 31 ms with the cells round it alone
    // and 14 ms with the loops of the drag on a grid of their own size
    let budget = Duration::from_millis(50);
    let after_first = &frames[1..];
    assert!(after_first.iter().all(|t| qymcad_core::time_budget::within(*t, budget)), "frames of a drag in a grid of 200 by 200 took {frames:?}, budget {budget:?} after the first");
}

#[test]
fn an_edit_of_a_grid_of_200_takes_a_frame() {
    let mut p = grid(200);
    let mut edits = Vec::new();
    for k in [100, 101, 102] {
        let id = line(&p, Way::Across, k).id;
        let started = Instant::now();
        p.move_entities(0, &[id], 0.0, 3.0);
        edits.push(started.elapsed());
    }
    let started = Instant::now();
    p.regen_sketch_whole(0);
    let whole = started.elapsed();
    eprintln!("a line of a grid of 200 by 200 moved: {edits:?}, the grid rebuilt whole {whole:?}");
    // measured in a release build: 0.27 s with the whole grid rebuilt for every edit, 37 ms with the cells round the line
    let budget = Duration::from_millis(60);
    assert!(edits.iter().all(|t| qymcad_core::time_budget::within(*t, budget)), "a line of a grid of 200 by 200 moved in {edits:?}, budget {budget:?}");
}
