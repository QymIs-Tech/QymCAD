//! A PART SITTING INSIDE ANOTHER PART IS NEVER MISSED QUIETLY.
//!
//! Reported behaviour: the metal shared by two bodies came out differently in different places on the same
//! rod - by tens of per cent, and once as a flat zero. A flat zero is the dangerous one: an assembly reads it
//! as "these two are clear of each other", so a shaft could sit inside a housing with nothing said.
//!
//! The instrument itself was tried on a clean rod in `qymcad-kernel/tests/interference_tells_the_truth.rs` and
//! held to 0.1%. So this file measures the OTHER suspect: the body. The bolt here is built the way a person
//! builds one - through a document, with a thread, with a run-out at both ends - and the same slab of it is
//! weighed at height after height. A thread is uniform along its length, so away from the ends every one of
//! those slabs holds the same metal. One that does not is either a defect in the bolt or a lie by the
//! instrument, and both are worth a red test.
use qymcad_kernel::Shape;
use qymcad_core::model::Project;
use qymcad_core::thread::{ThreadSpec, ThreadStandard};

const PI: f64 = std::f64::consts::PI;

/// M`d` with a pitch of `p`, external, with the given per-side clearance.
fn m(d: f64, pitch: f64, fit: f64) -> ThreadSpec {
    ThreadSpec { standard: ThreadStandard::MetricIso, nominal_d: d, pitch, internal: false, fit, ..Default::default() }
}

/// The round edge of radius about `r` — what a person clicks to place a thread.
fn rim(p: &mut Project, body: u64, r: f64) -> u32 {
    let (_report, _shapes) = qymcad_testkit::regenerate(p);
    let e = p.regen_edges.get(&body).cloned().unwrap_or_default();
    e.iter()
        .filter(|e| e.radius > 1e-9 && (e.radius - r).abs() < 0.05)
        .map(|e| e.id)
        .next()
        .unwrap_or_else(|| panic!("body {body} has no round edge of radius {r}"))
}

/// THE BOLT OF THE COMPLAINT, built through a document: a shaft threaded over its whole length, with a
/// run-out of `lead` at each end.
fn bolt(d: f64, pitch: f64, len: f64, lead: f64, fit: f64) -> Shape {
    let mut p = Project::default();
    p.new_document();
    let blank = p.add_cylinder(d * 0.5, len);
    let e = rim(&mut p, blank, d * 0.5);
    let t = p.add_thread(blank, e, m(d, pitch, fit), len, lead, lead);
    let last = p.finish_base_body(t, 1);
    let (report, mut shapes) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "the bolt did not build: {:?}", report.errors);
    shapes.remove(&last).expect("the bolt's shape")
}

/// A slab of space: a disc wide enough to swallow anything under test, `t` thick, its bottom at `z`.
fn slab(t: f64, z: f64) -> Shape {
    Shape::cylinder(50.0, t)
        .expect("the slab")
        .transformed(&[1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, z])
        .expect("the slab moved into place")
}

/// THE SAME SLAB OF THE SAME BOLT WEIGHS THE SAME AT EVERY HEIGHT.
///
/// Away from the run-outs a thread repeats itself, so a slab one pitch thick is the same piece of steel
/// wherever it is taken. The spread between the lightest and the heaviest is therefore not a property of the
/// bolt but the error of whatever produced the numbers.
#[test]
fn the_bolt_of_the_complaint_weighs_the_same_at_every_height() {
    let (d, pitch, len, lead, fit) = (10.0, 1.5, 20.0, 1.5, 0.2);
    let b = bolt(d, pitch, len, lead, fit);
    let g = m(d, pitch, fit).geometry();

    // Well clear of both run-outs: they change the turn, and this check is about the plain middle.
    let mut seen: Vec<(f64, f64)> = Vec::new();
    for i in 0..24 {
        let z = lead + 1.0 + 0.5 * i as f64;
        let got = b.interference_volume(&slab(pitch, z)).unwrap_or_else(|| panic!("the kernel refused to measure a slab at z={z:.2}: an assembly would have read that as 'clear'"));
        seen.push((z, got));
    }
    let lo = seen.iter().map(|s| s.1).fold(f64::INFINITY, f64::min);
    let hi = seen.iter().map(|s| s.1).fold(0.0, f64::max);
    let spread = (hi - lo) / hi * 100.0;
    for (z, v) in &seen {
        eprintln!("  z={z:.2}: {v:.3} mm^3");
    }
    eprintln!("a pitch-thick slab of the bolt: {lo:.3} to {hi:.3} mm^3, a spread of {spread:.1}%");

    // THE BOUNDS ARITHMETIC GIVES: not less than the bare core, not more than the untouched blank.
    let (floor, ceiling) = (PI * (g.minor_d * 0.5).powi(2) * pitch, PI * (g.stock_d * 0.5).powi(2) * pitch);
    for (z, v) in &seen {
        assert!(*v > floor && *v < ceiling, "a slab at z={z:.2} shares {v:.3} mm^3, outside {floor:.3}..{ceiling:.3} which is the bare core and the blank");
    }
    assert!(spread < 2.0, "the same slab of the same bolt weighs {lo:.3} in one place and {hi:.3} in another, a spread of {spread:.1}%");
}

/// A ROD DRIVEN STRAIGHT THROUGH A BLOCK IS CAUGHT AT EVERY DEPTH.
///
/// The plainest penetration there is, and the one an assembly must never miss. It is stepped through so that
/// no single lucky position can carry the check: the answer has to be right at all of them, and a refusal
/// counts as a failure rather than as a zero.
#[test]
fn a_rod_driven_through_a_block_is_caught_at_every_depth() {
    let (r, len, thick) = (4.0, 30.0, 6.0);
    let rod = Shape::cylinder(r, len).expect("the rod");
    let block = Shape::cylinder(20.0, thick).expect("the block");
    for i in 0..20 {
        let z = 1.0 + 1.5 * i as f64; // from well inside the rod to hanging off its far end
        let at = block.transformed(&[1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, z]).expect("the block moved");
        let got = rod.interference_volume(&at).unwrap_or_else(|| panic!("the kernel refused to measure the pair at z={z}"));
        let overlap = len.min(z + thick) - z.max(0.0); // how much of the block the rod actually crosses
        if overlap < 0.5 {
            continue; // a grazing contact is a different question, and not this one
        }
        let want = PI * r * r * overlap;
        let off = (got - want) / want * 100.0;
        assert!(off.abs() < 1.0, "a rod through a block at z={z} shares {got:.3} mm^3 against {want:.3} on paper, off by {off:.1}%");
    }
}
