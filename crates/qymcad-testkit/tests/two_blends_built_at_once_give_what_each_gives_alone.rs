//! TWO BLENDS BUILT AT ONCE GIVE WHAT EACH GIVES ALONE. The rebuild builds the nodes of one wave on threads of their own,
//! and two chamfers of OCCT built on two threads at once clash, each on a body of its own: on the case of the examples
//! the chamfer 11963 came out invalid and was built a hair short (240 edges instead of 238, the ends of eight edges
//! renamed) in 3 of 40 rebuilds, every time while the chamfer 5128 of another part was being built beside it, and in
//! none of 20 rebuilds on one thread.
//!
//! The case is rebuilt whole once; then the two chamfers are marked and rebuilt together, again and again, and the
//! chamfer 11963 is to come out every time as it came out the first time. Live OCCT, headless.
use qymcad_core::model::Id;

/// THE CASE OF THE EXAMPLES, opened as the window opens it.
const CASE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/QymBoxPc-Case.qcad");

/// The chamfer that came out short, and the chamfer of another part built beside it in the same wave.
const SHORT: Id = 11963;
const BESIDE: Id = 5128;

/// How many times the two are rebuilt together: a clash measured once in about ten waves where they overlap.
const ROUNDS: usize = 50;

/// What the chamfer came out as: its warning and how many edges its body has.
fn came_out(p: &qymcad_core::model::Project) -> String {
    format!("{:?}, {} edges", p.regen_warnings.get(&SHORT), p.regen_edges.get(&SHORT).map_or(0, Vec::len))
}

#[test]
fn the_chamfer_11963_comes_out_the_same_with_the_chamfer_5128_built_beside_it() {
    let mut p = qymcad_io::load_project(CASE).expect("the case opens");
    let shapes = qymcad_testkit::restore_import_shapes(&p);
    let (report, mut shapes) = qymcad_testkit::regenerate_with_shapes(&mut p, shapes);
    assert!(report.errors.is_empty(), "setup: the case is built whole: {:?}", report.errors);
    let first = came_out(&p);
    assert!(!p.regen_warnings.contains_key(&SHORT), "setup: the chamfer {SHORT} is built short on the first rebuild: {first}");
    let mut unlike = Vec::new();
    for round in 1..=ROUNDS {
        for n in p.timeline.iter_mut().filter(|n| n.id == SHORT || n.id == BESIDE) {
            n.dirty = true;
        }
        let (_, again) = qymcad_testkit::regenerate_dirty_with_shapes(&mut p, shapes);
        shapes = again;
        let now = came_out(&p);
        if now != first {
            unlike.push(format!("round {round}: {now}"));
        }
    }
    assert!(unlike.is_empty(), "the chamfer {SHORT} came out as {first} first and otherwise in {} of {ROUNDS} rounds:\n  {}", unlike.len(), unlike.join("\n  "));
}
