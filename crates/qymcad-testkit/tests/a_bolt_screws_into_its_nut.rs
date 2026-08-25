//! A BOLT MADE HERE SCREWS INTO A NUT MADE HERE.
//!
//! Reported behaviour: the run-out at the entry and the exit spoils the turn by thickening it, and the
//! thread will not screw on.
//!
//! A thread is a PAIR, not a picture. Two parts built from the same standard, the same pitch and the same
//! fit have to go together with the clearance that was asked for and nothing more. Measuring one part alone
//! says nothing about that: an external thread can be perfectly formed and still not enter a perfectly
//! formed nut.
//!
//! What is measured is the volume the two bodies share once put on one axis. For a pair that fits, that is
//! zero or a rounding crumb; for one that binds, it is cubic millimetres of metal in the same place twice.
use qymcad_core::model::Project;
use qymcad_core::thread::{ThreadSpec, ThreadStandard};

/// HOW MUCH METAL TWO BODIES SHARE, insisting on an answer.
///
/// The kernel is allowed to refuse: it says "could not measure" rather than reporting a zero, because a zero
/// already means "clear of each other". In a check a refusal is not something to work around quietly - every
/// number below rests on it, so it is shown as a failure.
trait SharedMetal {
    fn shared_metal(&self, other: &qymcad_kernel::Shape) -> f64;
}
impl SharedMetal for qymcad_kernel::Shape {
    fn shared_metal(&self, other: &qymcad_kernel::Shape) -> f64 {
        self.interference_volume(other).expect("the kernel measured the shared metal")
    }
}

/// M`d` with a pitch of `p`, external or internal, with the given per-side clearance.
fn m(d: f64, pitch: f64, internal: bool, fit: f64) -> ThreadSpec {
    ThreadSpec { standard: ThreadStandard::MetricIso, nominal_d: d, pitch, internal, fit, ..Default::default() }
}

/// The round edge of radius about `r` — what a person clicks to place a thread.
fn rim(p: &mut Project, body: u64, r: f64) -> u32 {
    let (_report, _shapes) = qymcad_testkit::regenerate(p);
    let e = p.regen_edges.get(&body).cloned().unwrap_or_default();
    e.iter()
        .filter(|e| e.radius > 1e-9 && (e.radius - r).abs() < 0.05)
        .map(|e| e.id)
        .next()
        .unwrap_or_else(|| panic!("body {body} has no round edge of radius {r}: {:?}", e.iter().map(|x| x.radius).collect::<Vec<_>>()))
}

/// THE BOLT: a shaft at the major diameter with an external thread over its whole length.
fn bolt(d: f64, pitch: f64, len: f64, lead: f64, fit: f64) -> qymcad_kernel::Shape {
    let mut p = Project::default();
    p.new_document();
    let blank = p.add_cylinder(d * 0.5, len);
    let e = rim(&mut p, blank, d * 0.5);
    let t = p.add_thread(blank, e, m(d, pitch, false, fit), len, lead, lead);
    let last = p.finish_base_body(t, 1);
    let (report, mut shapes) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "the bolt did not build: {:?}", report.errors);
    shapes.remove(&last).expect("the bolt's shape")
}

/// THE NUT: a sleeve bored to the thread's MINOR diameter, with an internal thread in the bore.
///
/// The bore is the tap drill: bored to the major diameter there would be nothing left to cut into, and the
/// pair would go together by virtue of holding no thread at all.
fn nut(d: f64, pitch: f64, len: f64, lead: f64, fit: f64) -> qymcad_kernel::Shape {
    let minor = m(d, pitch, true, fit).geometry().minor_d;
    let mut p = Project::default();
    p.new_document();
    let outer = p.add_cylinder(d, len);
    let hole = p.add_cylinder(minor * 0.5, len + 10.0);
    let blank = p.add_body_boolean(outer, hole, 0);
    let e = rim(&mut p, blank, minor * 0.5);
    let t = p.add_thread(blank, e, m(d, pitch, true, fit), len, lead, lead);
    let last = p.finish_base_body(t, 1);
    let (report, mut shapes) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "the nut did not build: {:?}", report.errors);
    shapes.remove(&last).expect("the nut's shape")
}

/// SCREW the body in: turn it by `turn` radians about the axis AND move it along by the pitch that goes
/// with that turn.
///
/// The two go TOGETHER, and that is the whole point. Sliding a nut along a bolt without turning it is not
/// screwing but shoving: the turns run into each other whatever the thread is like, and a check built that
/// way calls every thread in the world broken. A thread is a helix; the only motion it allows is this one.
fn screwed(s: &qymcad_kernel::Shape, turn: f64, pitch: f64, base: f64) -> qymcad_kernel::Shape {
    let (c, si) = (turn.cos(), turn.sin());
    let dz = base + pitch * turn / (2.0 * std::f64::consts::PI);
    s.transformed(&[c, -si, 0.0, 0.0, si, c, 0.0, 0.0, 0.0, 0.0, 1.0, dz]).expect("the screw motion")
}

/// HOW MUCH METAL THE TWO SHARE at the worst point of screwing in one full turn.
///
/// One position proves nothing: the pair may pass at one angle and bind a quarter of a turn further on, so
/// a whole revolution is stepped through.
fn worst_bind(bolt: &qymcad_kernel::Shape, nut: &qymcad_kernel::Shape, pitch: f64, len: f64) -> (f64, f64) {
    let mut worst = (0.0, 0.0);
    let steps = 12;
    for i in 0..steps {
        let turn = 2.0 * std::f64::consts::PI * (i as f64) / (steps as f64);
        let v = bolt.shared_metal(&screwed(nut, turn, pitch, -len * 0.5));
        if v > worst.0 {
            worst = (v, turn.to_degrees());
        }
    }
    worst
}

/// THE PAIR GOES TOGETHER. This is the whole complaint, stated as one number.
#[test]
fn a_bolt_and_its_nut_do_not_bind() {
    let (d, pitch, len, lead, fit) = (10.0, 1.5, 20.0, 1.5, 0.2);
    let b = bolt(d, pitch, len, lead, fit);
    let n = nut(d, pitch, len, lead, fit);
    let (bind, at) = worst_bind(&b, &n, pitch, len);

    // THE SCALE OF THE ANSWER. A clearance of 0.2 mm per side over about 6 turns leaves room measured in
    // tens of cubic millimetres; a bind that eats a whole turn is hundreds. The bar is set at the volume of
    // ONE turn's worth of metal at the thread's depth - past that the pair is not a pair.
    let g = m(d, pitch, false, fit).geometry();
    let one_turn = std::f64::consts::PI * ((d * 0.5).powi(2) - (d * 0.5 - g.depth).powi(2)) * pitch;
    eprintln!("M{d}x{pitch}, run-out {lead}: the pair shares {bind:.1} mm^3 at {at:.0} deg (one turn is about {one_turn:.1} mm^3)");
    assert!(bind < one_turn, "the bolt does not screw into its nut: they share {bind:.1} mm^3, about {:.1} turns of metal", bind / one_turn);
}

/// WITHOUT A RUN-OUT the same pair goes together — that is what pins the blame on the run-out.
///
/// Without this the check would only say "the pair binds" and leave open whether the run-out or the profile
/// itself is at fault.
#[test]
fn without_a_run_out_the_same_pair_goes_together() {
    let (d, pitch, len, fit) = (10.0, 1.5, 20.0, 0.2);
    let b = bolt(d, pitch, len, 0.0, fit);
    let n = nut(d, pitch, len, 0.0, fit);
    let (bind, at) = worst_bind(&b, &n, pitch, len);

    let g = m(d, pitch, false, fit).geometry();
    let one_turn = std::f64::consts::PI * ((d * 0.5).powi(2) - (d * 0.5 - g.depth).powi(2)) * pitch;
    eprintln!("the same pair with no run-out: {bind:.1} mm^3 at {at:.0} deg (one turn is about {one_turn:.1} mm^3)");
    assert!(bind < one_turn, "even without a run-out the pair binds by {bind:.1} mm^3 - then the profile itself is at fault");
}

/// A bare cylinder of radius `r` and height `h`, standing at the origin — a probe for measuring how much
/// metal the shaft keeps inside a given slab.
fn probe(r: f64, h: f64) -> qymcad_kernel::Shape {
    let mut p = Project::default();
    p.new_document();
    let c = p.add_cylinder(r, h);
    let last = p.finish_base_body(c, 1);
    let (_report, mut shapes) = qymcad_testkit::regenerate(&mut p);
    shapes.remove(&last).expect("the probe's shape")
}

/// THE RUN-OUT TAKES THE CREST DOWN, IT DOES NOT FILL THE ROOT.
///
/// Reported behaviour: the run-out spoils the turn by thickening it. On a picture of the far end the last
/// turn is not a fading ridge but a WIDE FLAT BAND: the groove disappears and a plain cylinder at the MAJOR
/// diameter is left. Nothing mates with that — the nut's bore is smaller than the shaft there.
///
/// What is measured is the metal the shaft keeps inside the last `lead` millimetres. With the groove still
/// cut it is well under a solid cylinder; with the groove filled in it is the solid cylinder itself.
#[test]
fn the_run_out_does_not_leave_a_solid_cylinder() {
    let (d, pitch, len, lead, fit) = (10.0, 1.5, 20.0, 1.5, 0.2);
    let b = bolt(d, pitch, len, lead, fit);

    // The slab is the run-out zone at the far end: z from 0 to `lead`, the shaft starting at the origin.
    let slab = probe(d * 0.5, lead);
    let solid = std::f64::consts::PI * (d * 0.5).powi(2) * lead;
    let kept = b.shared_metal(&slab);

    // WHAT A LIVE THREAD LEAVES THERE. A metric groove takes away about half the ring between the major and
    // the minor diameter; over the run-out the depth fades, so more than half stays - but not ALL of it. The
    // bar is set where the difference is unmistakable: nine tenths of a solid cylinder means there is no
    // groove left worth speaking of.
    let share = kept / solid;
    eprintln!("M{d}x{pitch}: the last {lead} mm keep {kept:.1} mm^3 of {solid:.1} - {:.0}% of a solid cylinder", share * 100.0);
    assert!(
        share < 0.9,
        "the run-out left a solid cylinder at the MAJOR diameter ({:.0}% of it): the thread ends in a plug no nut can pass",
        share * 100.0
    );
}

/// HOW THE METAL GROWS TOWARDS THE END — a diagnostic, not a verdict.
///
/// Run by hand while the run-out is being worked on: it prints the share of a solid cylinder the shaft
/// keeps inside slabs of growing thickness at the far end. A groove that fades honestly gives a share that
/// rises smoothly; one that fills the root gives a share pinned at 100% near the very end.
#[test]
#[ignore]
fn how_the_run_out_fills_towards_the_end() {
    let (d, pitch, len, fit) = (10.0, 1.5, 20.0, 0.2);
    for lead in [0.0, 1.5] {
        let b = bolt(d, pitch, len, lead, fit);
        eprintln!("--- run-out {lead} mm");
        for h in [0.1, 0.2, 0.3, 0.5, 0.75, 1.0, 1.5, 2.0, 3.0] {
            let slab = probe(d * 0.5, h);
            let solid = std::f64::consts::PI * (d * 0.5).powi(2) * h;
            let kept = b.shared_metal(&slab);
            eprintln!("  last {h:>4} mm: {:>5.1}% of a solid cylinder", kept / solid * 100.0);
        }
    }
    let _ = (len, pitch);

    // WHERE THE METAL SITS ACROSS THE RADIUS in the last tenth of a millimetre. A crest still standing at
    // the major diameter shows up as metal in the outermost ring; a crest taken down leaves that ring empty.
    for lead in [0.0, 1.5] {
        let b = bolt(d, pitch, len, lead, fit);
        eprintln!("--- run-out {lead} mm, the last 0.1 mm across the radius");
        let mut prev = 0.0;
        for r in [4.2, 4.4, 4.6, 4.8, 5.0] {
            let kept = b.shared_metal(&probe(r, 0.1));
            eprintln!("  up to r={r}: {kept:>7.3} mm^3 (the ring adds {:>7.3})", kept - prev);
            prev = kept;
        }
    }
}

/// THE NUT'S SIDE OF THE SAME QUESTION — a diagnostic, not a verdict.
///
/// An internal thread has its crest at the SMALL diameter, so a run-out that goes the wrong way leaves a
/// ridge poking into the bore, and the bolt runs into it. What is printed is how much metal the nut keeps
/// inside a growing radius, at the run-out end against the middle: metal below the bore's radius is a
/// ridge, and there must be none.
#[test]
#[ignore]
fn what_the_nut_keeps_near_its_end() {
    let (d, pitch, len, fit) = (10.0, 1.5, 20.0, 0.2);
    let g_int = m(d, pitch, true, fit).geometry();
    let g_ext = m(d, pitch, false, fit).geometry();
    eprintln!("internal: major {:.3}, pitch {:.3}, minor {:.3}, depth {:.3}", g_int.major_d, g_int.pitch_d, g_int.minor_d, g_int.depth);
    eprintln!("external: major {:.3}, pitch {:.3}, minor {:.3}, depth {:.3}", g_ext.major_d, g_ext.pitch_d, g_ext.minor_d, g_ext.depth);

    for lead in [0.0, 1.5] {
        let n = nut(d, pitch, len, lead, fit);
        eprintln!("--- nut, run-out {lead} mm; metal inside r, in the last 0.1 mm");
        let mut prev = 0.0;
        for r in [3.8, 4.0, 4.2, 4.4, 4.6, 4.8, 5.0, 5.2] {
            let kept = n.shared_metal(&probe(r, 0.1));
            eprintln!("  up to r={r}: {kept:>7.3} mm^3 (the ring adds {:>7.3})", kept - prev);
            prev = kept;
        }
    }
}

/// BOTH ENDS AT ONCE — a diagnostic, not a verdict.
///
/// Every measurement so far looked at the far end, where the sweep starts. The entry, at the other end, is
/// built differently: there the thread is carried PAST the face so that the groove's end cap lands in the
/// air. What is printed is the metal inside a growing radius in the last tenth of a millimetre at BOTH
/// ends, so that the two can be told apart.
#[test]
#[ignore]
fn both_ends_of_the_bolt() {
    let (d, pitch, len, fit) = (10.0, 1.5, 20.0, 0.2);
    for lead in [0.0, 1.5] {
        let b = bolt(d, pitch, len, lead, fit);
        for (what, base) in [("the far end (z=0)", 0.0), ("the entry (z=len)", len - 0.1)] {
            eprintln!("--- run-out {lead} mm, {what}");
            let mut prev = 0.0;
            for r in [4.0, 4.2, 4.4, 4.6, 4.8, 5.0] {
                let p = screwed(&probe(r, 0.1), 0.0, pitch, base);
                let kept = b.shared_metal(&p);
                eprintln!("  up to r={r}: {kept:>7.3} mm^3 (the ring adds {:>7.3})", kept - prev);
                prev = kept;
            }
        }
    }
}

/// A SWEEP OVER SIZES AND RUN-OUT LENGTHS — a diagnostic, not a verdict.
///
/// One size proves nothing: the complaint may live where the run-out eats most of the thread (a short
/// thread with a long run-out), on a coarse pitch or on a small diameter. What is printed is the worst
/// bind of a full turn of screwing for each combination, against the metal in one turn.
#[test]
#[ignore]
fn a_sweep_over_sizes_and_run_outs() {
    // (nominal, pitch, thread length, run-out at each end)
    let cases: &[(f64, f64, f64, f64)] = &[
        (10.0, 1.5, 20.0, 0.0),
        (10.0, 1.5, 20.0, 1.5),
        (10.0, 1.5, 6.0, 1.5),
        (10.0, 1.5, 6.0, 2.5),
        (6.0, 1.0, 12.0, 2.0),
        (20.0, 2.5, 30.0, 2.5),
    ];
    let fit = 0.2;
    for (d, pitch, len, lead) in cases.iter().copied() {
        let b = bolt(d, pitch, len, lead, fit);
        let n = nut(d, pitch, len, lead, fit);
        let (bind, at) = worst_bind(&b, &n, pitch, len);
        let g = m(d, pitch, false, fit).geometry();
        let one_turn = std::f64::consts::PI * ((d * 0.5).powi(2) - (d * 0.5 - g.depth).powi(2)) * pitch;
        eprintln!(
            "M{d}x{pitch}, length {len}, run-out {lead}: worst {bind:>7.1} mm^3 at {at:>3.0} deg = {:>5.2} turns of metal",
            bind / one_turn
        );
    }
}

/// WHICH WAY THE NUT ACTUALLY TURNS — a diagnostic, not a verdict.
///
/// Every measurement so far found its worst point at the LAST step of the revolution, all six of them. A
/// bind that grows steadily with the angle is the mark of turning the wrong way: a right-hand pair screwed
/// left drives metal into metal, and no thread on earth would pass such a check. The two directions are
/// measured side by side, and the pair is judged by the better one - that is the way it actually goes on.
#[test]
#[ignore]
fn which_way_the_nut_turns() {
    let fit = 0.2;
    for (d, pitch, len, lead) in [(10.0, 1.5, 20.0, 1.5), (20.0, 2.5, 30.0, 2.5)] {
        let b = bolt(d, pitch, len, lead, fit);
        let n = nut(d, pitch, len, lead, fit);
        let g = m(d, pitch, false, fit).geometry();
        let one_turn = std::f64::consts::PI * ((d * 0.5).powi(2) - (d * 0.5 - g.depth).powi(2)) * pitch;
        for sign in [1.0, -1.0] {
            let (mut worst, mut at) = (0.0f64, 0.0f64);
            for i in 0..12 {
                let turn = 2.0 * std::f64::consts::PI * (i as f64) / 12.0;
                let dz = -len * 0.5 + sign * pitch * turn / (2.0 * std::f64::consts::PI);
                let (c, si) = (turn.cos(), turn.sin());
                let moved = n.transformed(&[c, -si, 0.0, 0.0, si, c, 0.0, 0.0, 0.0, 0.0, 1.0, dz]).expect("the screw motion");
                let v = b.shared_metal(&moved);
                if v > worst {
                    worst = v;
                    at = turn.to_degrees();
                }
            }
            let way = if sign > 0.0 { "with the helix" } else { "against it" };
            eprintln!("M{d}x{pitch} run-out {lead}, turning {way:>13}: worst {worst:>7.1} mm^3 at {at:>3.0} deg = {:>5.2} turns", worst / one_turn);
        }
    }
}

/// WHY M20x2.5 BINDS — a diagnostic, not a verdict.
///
/// The sweep found one size that really does not go together: M20x2.5 shares over a turn's worth of metal,
/// and it does so at ZERO degrees, before any turning at all - a standing overlap rather than something the
/// screwing motion produces. Two things could cause it: the run-out, or a clearance that does not keep up
/// with a coarse pitch. They are separated here by varying one at a time.
#[test]
#[ignore]
fn why_the_coarse_pair_binds() {
    let (d, pitch, len) = (20.0, 2.5, 30.0);
    for lead in [0.0, 2.5] {
        for fit in [0.2, 0.4] {
            let b = bolt(d, pitch, len, lead, fit);
            let n = nut(d, pitch, len, lead, fit);
            let g = m(d, pitch, false, fit).geometry();
            let one_turn = std::f64::consts::PI * ((d * 0.5).powi(2) - (d * 0.5 - g.depth).powi(2)) * pitch;
            // AT REST: the two put on one axis with no turning at all. A pair that fits shares nothing here.
            let at_rest = b.shared_metal(&screwed(&n, 0.0, pitch, -len * 0.5));
            let (worst, at) = worst_bind(&b, &n, pitch, len);
            eprintln!(
                "M{d}x{pitch} run-out {lead}, fit {fit}: at rest {at_rest:>7.1}, worst {worst:>7.1} mm^3 at {at:>3.0} deg = {:>5.2} turns",
                worst / one_turn
            );
        }
    }
}

/// A BLIND END GETS A RELIEF GROOVE, NOT A WALL.
///
/// Reported: the exit of the thread at the bottom is broken off by a flat wall perpendicular to the profile.
/// The end there is BLIND - the thread runs into a flange - and that is the case a free-standing shaft never
/// exercises: both of its ends are open.
///
/// What a lathe does at a blind end is cut a relief groove down to the root and let the thread run into it.
/// So just above the shoulder there must be no metal outside the root radius: a plain cylinder, and the last
/// turn ends inside it with a full face.
///
/// The profile is deliberately a sane one (60 degrees, 2.5 deep at a 5 pitch) — a groove wider than the pitch
/// would eat the turns and the measurement would be about that instead.
#[test]
#[ignore]
fn a_blind_end_is_relieved_not_walled() {
    // THE GUARD AGAINST A VACUOUS CHECK: with no run-out asked for there is no relief, and the metal stands
    // right up to the shoulder. Without this the check could pass on geometry that never had a relief at all.
    let bare = metal_above_the_shoulder(0.0) * 100.0;
    let relieved = metal_above_the_shoulder(2.0) * 100.0;
    eprintln!("above the shoulder: {bare:.0}% of the ring with no run-out, {relieved:.0}% with one");
    assert!(bare > 30.0, "GUARD: with no run-out the metal must stand up to the shoulder, and only {bare:.0}% of the ring does - the check would prove nothing");
    assert!(relieved < 15.0, "asking for a run-out left {relieved:.0}% of the ring standing: the thread runs into the shoulder and the last turn ends against a wall");
    assert!(bare > relieved * 2.0, "the run-out barely changed anything: {bare:.0}% against {relieved:.0}%");
}

/// The share of a full ring (from the root radius out to the boss) that stands just above the shoulder.
fn metal_above_the_shoulder(lead: f64) -> f64 {
    let (r_boss, h_flange, h_boss, len) = (20.0, 6.0, 26.0, 20.0);
    let spec = ThreadSpec {
        standard: ThreadStandard::Custom,
        nominal_d: r_boss * 2.0,
        pitch: 5.0,
        custom_angle: 60.0,
        custom_depth: 2.5,
        crest_r: Some(0.0),
        root_r: Some(0.0),
        ..Default::default()
    };
    let mut p = Project::default();
    p.new_document();
    let flange = p.add_cylinder(35.0, h_flange);
    let boss = p.add_cylinder(r_boss, h_boss);
    let blank = p.add_body_boolean(flange, boss, 1);
    // THE RIM A PERSON CLICKS is the one they can see — the top of the boss. There are two of that radius.
    let (_r0, _s0) = qymcad_testkit::regenerate(&mut p);
    let e = p.regen_edges.get(&blank).cloned().unwrap_or_default();
    let rim = e
        .iter()
        .filter(|x| (x.radius - r_boss).abs() < 0.05)
        .max_by(|x, y| x.mid[2].total_cmp(&y.mid[2]))
        .map(|x| x.id)
        .expect("the top rim of the boss");
    let t = p.add_thread(blank, rim, spec.clone(), len, lead, lead);
    let last = p.finish_base_body(t, 1);
    let (report, mut shapes) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "the part did not build: {:?}", report.errors);
    let body = shapes.remove(&last).expect("the part");

    let root_r = r_boss - spec.geometry().depth;
    // A THIN SLAB JUST ABOVE THE SHOULDER, inside the run-out zone.
    let slab_h = 0.3;
    let base = h_flange + 0.05;
    let outside = body.shared_metal(&screwed(&probe(r_boss, slab_h), 0.0, 0.0, base))
        - body.shared_metal(&screwed(&probe(root_r, slab_h), 0.0, 0.0, base));
    let ring = std::f64::consts::PI * (r_boss.powi(2) - root_r.powi(2)) * slab_h;
    outside / ring
}

/// DOES TURNING THE CREST DOWN ACTUALLY CURE THE BIND? — the remedy proved before it is built.
///
/// The clearance saturates against the pitch, and the remainder has to be taken radially:
/// `ThreadSpec::radial_relief` says how much. Cutting that ring inside the kernel is a piece of work, and it
/// would be foolish to build it before knowing it helps. So the shaft is turned down BY HAND here — the
/// blank is made smaller by that very amount — and the pair is screwed together as before.
///
/// If the bind falls, the arithmetic is right and the ring is worth building. If it does not, something else
/// is wrong and building the ring would have been a waste.
///
/// MEASURED (M20x2.5, length 20, run-out 2.5, fit 0.2): the crest goes down by 0.119 mm and the pair shares
/// 97.5 mm^3 against 119.3 before - 0.44 turns of metal against 0.54. So the arithmetic points the right way
/// and the ring does help, but it takes off only about a FIFTH of the bind and leaves the rest. On this
/// evidence the ring alone is not the cure, and whatever holds the other four fifths has not been named yet.
#[test]
#[ignore]
fn turning_the_crest_down_cures_the_bind() {
    // THE SIZE MATTERS, AND IT CHANGED UNDER THIS TEST. Written for M10x1.5, the check measured 0.0 against
    // 0.0 and failed on its own premise: that pair no longer binds at all (measured today, worst 0.0 mm^3).
    // The pair that still binds is the coarse one - M20x2.5 shares 226 mm^3, a full turn of metal - so the
    // remedy is proved where the trouble is. Asking a cure of a pair that is already well is asking nothing.
    let (d, pitch, len, lead, fit) = (20.0, 2.5, 20.0, 2.5, 0.2);
    let e = m(d, pitch, false, fit).radial_relief();
    assert!(e > 0.0, "setup: at this pitch the clearance overflows, so there is something to take off");

    let n = nut(d, pitch, len, lead, fit);
    let plain = bolt(d, pitch, len, lead, fit);
    let (bind_plain, _) = worst_bind(&plain, &n, pitch, len);

    // THE SAME BOLT ON A SHAFT TURNED DOWN by the radial relief. Everything else is untouched.
    let mut p = Project::default();
    p.new_document();
    let blank = p.add_cylinder(d * 0.5 - e, len);
    let rim_id = rim(&mut p, blank, d * 0.5 - e);
    let t = p.add_thread(blank, rim_id, m(d, pitch, false, fit), len, lead, lead);
    let last = p.finish_base_body(t, 1);
    let (report, mut shapes) = qymcad_testkit::regenerate(&mut p);
    assert!(report.errors.is_empty(), "the turned-down bolt did not build: {:?}", report.errors);
    let turned = shapes.remove(&last).expect("the turned-down bolt");
    let (bind_turned, _) = worst_bind(&turned, &n, pitch, len);

    let g = m(d, pitch, false, fit).geometry();
    let one_turn = std::f64::consts::PI * ((d * 0.5).powi(2) - (d * 0.5 - g.depth).powi(2)) * pitch;
    eprintln!(
        "M{d}x{pitch}: crest down by {e:.3} mm -> the pair shares {bind_turned:.1} mm^3 against {bind_plain:.1} before ({:.2} turns against {:.2})",
        bind_turned / one_turn,
        bind_plain / one_turn
    );
    assert!(bind_turned < bind_plain, "turning the crest down did not help at all: {bind_turned:.1} against {bind_plain:.1} - the arithmetic is wrong and the ring is not worth building");
}

/// A NUT CAN BE STARTED ON THE FIRST TURN.
///
/// Reported behaviour: the run-out at the entry spoils the turn and the thread will not screw on. Whether it
/// screws on DEEP is one question — the pair check answers that. Whether it can be STARTED is another, and
/// it is the one a person meets first: the nut is brought to the end face and given a turn by hand.
///
/// So the nut is placed to overlap the bolt by a single pitch, right at the mouth, and screwed through a
/// whole revolution. A first turn that breaks off at a wall, or one left fat by the run-out, shows up here as
/// metal in the same place twice.
#[test]
fn a_nut_starts_on_the_first_turn() {
    let (d, pitch, len, lead, fit) = (10.0, 1.5, 20.0, 1.5, 0.2);
    let b = bolt(d, pitch, len, lead, fit);
    let n = nut(d, pitch, len, lead, fit);

    // AT THE MOUTH, BUT PAST THE CHAMFER. The entry carries a countersink as long as the run-out, and inside
    // it there is deliberately nothing to catch — that is what lets a nut be offered up at all. A nut sunk by
    // a single pitch sits entirely in that cone and passes at any angle, which says nothing about the thread.
    // Three pitches reach the chamfer plus two whole turns: the first turns a person actually screws onto.
    let base = len - 3.0 * pitch;
    let screwed_in = |i: i32| {
        let turn = 2.0 * std::f64::consts::PI * (i as f64) / 12.0;
        b.shared_metal(&screwed(&n, turn, pitch, base))
    };
    let along: Vec<f64> = (0..12).map(screwed_in).collect();

    // OFF THE THREAD'S OWN PITCH: the nut is turned without the travel that goes with it, half a pitch out of
    // step. A real thread REFUSES that - the turns run into each other - and a first turn that has been erased
    // instead of shaped lets the nut pass at any angle at all. Without this the check could not tell a good
    // entry from a missing one: both slip on when correctly phased.
    let off_phase: Vec<f64> = (0..12)
        .map(|i| {
            let turn = 2.0 * std::f64::consts::PI * (i as f64) / 12.0;
            let (c, si) = (turn.cos(), turn.sin());
            let dz = base + pitch * 0.5;
            let moved = n.transformed(&[c, -si, 0.0, 0.0, si, c, 0.0, 0.0, 0.0, 0.0, 1.0, dz]).expect("the misphased nut");
            b.shared_metal(&moved)
        })
        .collect();

    let g = m(d, pitch, false, fit).geometry();
    let one_turn = std::f64::consts::PI * ((d * 0.5).powi(2) - (d * 0.5 - g.depth).powi(2)) * pitch;
    let worst_along = along.iter().cloned().fold(0.0, f64::max);
    let most_off = off_phase.iter().cloned().fold(0.0, f64::max);
    eprintln!(
        "starting the nut at the mouth: {worst_along:.1} mm^3 screwing in, {most_off:.1} mm^3 half a pitch out of step (one turn is about {one_turn:.1})"
    );

    assert!(
        worst_along < 0.25 * one_turn,
        "the nut cannot be started: screwing it on at the mouth shares {worst_along:.1} mm^3, and a quarter of a turn is {:.1}",
        0.25 * one_turn
    );
    // THE FIRST TURN IS THERE AT ALL. A thread that catches must refuse a nut half a pitch out of step; one
    // whose first turn was erased lets it through, and that is the entry the complaint was about.
    // THE BAR SITS BETWEEN NOTHING AND WHAT WAS MEASURED. Correctly phased the pair shares 0.0; half a pitch
    // out of step it shares 2.7 mm^3 over the two turns that engage, and with the first turn erased it shared
    // 0.0 there as well. Two per cent of a turn's metal is well clear of nothing and well under the reading,
    // so the check answers "does it catch" rather than "how much".
    assert!(
        most_off > 0.02 * one_turn,
        "the first turn is missing: half a pitch out of step the nut still passes, sharing only {most_off:.1} mm^3 — a thread that catches refuses a wrong phase"
    );
}
