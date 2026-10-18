//! THE KEYS OF THE STATUS CACHE ARE QUICK, AND TELL A SHAPE FROM A PLACE: the keys a frame takes several times over a
//! sketch of 70 000 lines cost a little of a frame, a point moved changes where the sketch stands and not its shape, a
//! constraint laid changes its shape.
use qymcad_core::model::{Constraint, EntityKind, Project, SketchEntity, SketchPoint};
use std::time::Duration;

/// A sketch of `n` separate lines, laid straight into its records.
fn lines(n: usize) -> Project {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    let mut next = p.alloc_id() + 1;
    let s = &mut p.sketches[si];
    for k in 0..n {
        let (a, b, e) = (next, next + 1, next + 2);
        next += 3;
        let (x, y) = ((k % 300) as f64 * 20.0, (k / 300) as f64 * 20.0);
        s.points.push(SketchPoint { id: a, x, y });
        s.points.push(SketchPoint { id: b, x: x + 10.0, y });
        s.entities.push(SketchEntity { id: e, kind: EntityKind::Line { a, b }, construction: false });
    }
    p
}

#[test]
fn the_keys_of_seventy_thousand_lines_cost_a_little_of_a_frame() {
    // A MEASURE OF A RELEASE BUILD: in a test build the walk over the sketch outweighs the hashing itself (71 ms with the
    // hasher of a map, 53 ms with this one), and a threshold between them would be noise. In a release build: 5.1 ms
    // against 1.05 ms a key pair, taken some seven times a frame.
    if cfg!(debug_assertions) {
        eprintln!("PASSED OVER: a measure of a release build (cargo test --release)");
        return;
    }
    let p = lines(70_000);
    let si = p.sketches.len() - 1;
    // the work of the keys, not the share of the machine it was given (`time_budget::Cost`), the best of three
    let t = (0..3)
        .map(|_| qymcad_core::time_budget::cost_of(qymcad_core::time_budget::Doer::Thread, || (qymcad_ui_state::sketch_shape_key(&p, si), qymcad_ui_state::sketch_place_key(&p, si))).cost)
        .min_by_key(|c| c.work)
        .unwrap_or_default();
    eprintln!("the keys of 70 000 lines: {t}");
    let budget = Duration::from_millis(3);
    assert!(t.work < budget, "{} in a release build", qymcad_core::time_budget::over("the keys of 70 000 lines", t, budget));
}

#[test]
fn a_point_moved_changes_the_place_and_a_constraint_the_shape() {
    let mut p = lines(100);
    let si = p.sketches.len() - 1;
    let (shape, place) = (qymcad_ui_state::sketch_shape_key(&p, si), qymcad_ui_state::sketch_place_key(&p, si));
    p.sketches[si].points[7].x += 1e-9;
    assert_eq!(qymcad_ui_state::sketch_shape_key(&p, si), shape, "a point moved changed the shape");
    assert_ne!(qymcad_ui_state::sketch_place_key(&p, si), place, "a point moved by 1e-9 left the place as it was");
    let (a, b) = (p.sketches[si].points[0].id, p.sketches[si].points[1].id);
    p.sketches[si].constraints.push(Constraint::Horizontal { a, b });
    assert_ne!(qymcad_ui_state::sketch_shape_key(&p, si), shape, "a constraint laid left the shape as it was");
}
