//! SIZE AND TIME: projects of real size opened and rebuilt, and meshes of real size brought in, within the time a
//! person waits for in a professional system - a project open in seconds, rebuilt in under half a minute, a mesh of
//! two hundred thousand triangles in under half a minute, and no frame of the window stalled past the budget of every
//! step. The set is a debug build, several times slower than what people run: the budgets are set for it.
//!
//! The private samples (`samples/`, not published) close the letter as a cross-check: what they show must already be
//! caught by the checks built on files anyone has.
//!
//! A budget holds what the program spent, not the share of the machine it was given: a probe is alone in its process,
//! so the processor time of the process is the work of the program - its threads of rebuilding and reading included -
//! and the time it waited for a processor is the machine's (`time_budget::Cost`). Timed by the wall, the rebuild of the
//! case sample read 30.4 s under a load of 35-42 on 20 threads, against 12.4-13.8 s alone.
//! Every measure is printed, green or red.
use std::time::Duration;

use qymcad::Session;
use qymcad_acceptance::{build, probe};

/// How long opening a project may take, and rebuilding everything in it.
const OPEN: Duration = Duration::from_secs(10);
const REBUILD: Duration = Duration::from_secs(30);
/// How long bringing in a mesh of two hundred thousand triangles may take.
const MESH: Duration = Duration::from_secs(30);

// A BUDGET ON THIS MACHINE is the budget times `QYMCAD_TIME_SCALE` (1 when unset, `time_budget::scaled`). The budgets
// are set for the developer's machine; a CI runner of four cores rebuilt a sample project of 3 MB in 39.4 s against
// the 26.4 s of the whole probe here, so CI runs these probes alone and at twice the budget - a run twice as slow as it
// should be still goes red there, and the budgets here stay as they are.
use qymcad::time_budget::{holds, over, scaled, tell, Cost, Doer, Meter};

/// The meter of a step of a probe: the process is the probe's own.
fn meter() -> Meter {
    Meter::start(Doer::Process)
}

/// What the step since `meter` started cost, printed as a measure.
fn spent(what: &str, meter: &Meter) -> Cost {
    let cost = meter.cost();
    tell(what, cost);
    cost
}

/// Open `path`, time it, rebuild everything, time that: the problems, in words.
fn open_and_rebuild(path: &str) -> Vec<String> {
    let mut s = Session::start();
    s.budget(Duration::from_secs(600));
    let m = meter();
    build::open_project(&mut s, path);
    let doc = s.document();
    let opened = spent("opening", &m);
    let m = meter();
    let (edit, rebuild) = (s.word("menu-edit"), s.word("menu-rebuild"));
    s.menu(&[&edit, &rebuild]);
    let _ = s.document();
    let rebuilt = spent("rebuilding everything", &m);
    let mut problems = Vec::new();
    if !holds(opened, OPEN) {
        problems.push(format!("{} ({} nodes, {} faces)", over("opening", opened, scaled(OPEN)), doc.features.len(), doc.bodies.iter().map(|b| b.faces).sum::<usize>()));
    }
    if !holds(rebuilt, REBUILD) {
        problems.push(over("rebuilding everything", rebuilt, scaled(REBUILD)));
    }
    problems
}

fn sample(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

probe! {
    budget = 900;
    /// THE SAMPLE PROJECT OF A FILTER opens whole and in time, and rebuilds in time.
    fn the_filter_sample_opens_and_rebuilds_in_time() {
        let p = open_and_rebuild(&sample("Filter-v2.qcad"));
        assert!(p.is_empty(), "{}", p.join("; "));
    }
}

probe! {
    budget = 900;
    /// THE SAMPLE PROJECT OF A PC CASE - 96 nodes, 63 bodies, some 5700 faces - opens whole and in time, and rebuilds in
    /// time.
    fn the_case_sample_opens_and_rebuilds_in_time() {
        let p = open_and_rebuild(&sample("QymBoxPc-Case.qcad"));
        assert!(p.is_empty(), "{}", p.join("; "));
    }
}

/// A binary STL of a sphere of radius `r` about the origin, `lat` bands by `lon` slices: 2 * lat * lon triangles.
fn sphere_stl(path: &str, r: f32, lat: usize, lon: usize) {
    let point = |i: usize, j: usize| {
        let (t, p) = (std::f32::consts::PI * i as f32 / lat as f32, std::f32::consts::TAU * j as f32 / lon as f32);
        [r * t.sin() * p.cos(), r * t.sin() * p.sin(), r * t.cos()]
    };
    let mut tris = Vec::with_capacity(2 * lat * lon);
    for i in 0..lat {
        for j in 0..lon {
            let (a, b, c, d) = (point(i, j), point(i + 1, j), point(i + 1, j + 1), point(i, j + 1));
            if i > 0 {
                tris.push([a, b, d]);
            }
            if i + 1 < lat {
                tris.push([b, c, d]);
            }
        }
    }
    let mut out = vec![0u8; 80];
    out.extend((tris.len() as u32).to_le_bytes());
    for t in &tris {
        out.extend([0u8; 12]);
        for p in t {
            for x in p {
                out.extend(x.to_le_bytes());
            }
        }
        out.extend([0u8; 2]);
    }
    std::fs::write(path, out).expect("the mesh of a sphere is written");
}

probe! {
    budget = 900;
    /// A MESH OF TWO HUNDRED THOUSAND TRIANGLES - a sphere of 50, 200 bands by 500 slices - comes into a part within
    /// its time, holding the sphere's volume, and no frame after it stalls.
    fn a_mesh_of_two_hundred_thousand_triangles_comes_in_in_time() {
        let path = format!("{}/qymcad-sphere-{}.stl", std::env::temp_dir().display(), std::process::id());
        sphere_stl(&path, 50.0, 200, 500);
        let mut s = Session::start();
        s.budget(Duration::from_secs(600));
        build::into_the_first_part(&mut s);
        let _ = s.worst_frame();
        let m = meter();
        build::import(&mut s, &path);
        let doc = s.document();
        let took = spent("bringing in 200000 triangles", &m);
        let frame = s.worst_frame();
        let volume: f64 = doc.bodies.iter().filter(|b| !b.consumed).map(|b| b.volume).sum();
        let sphere = 4.0 / 3.0 * std::f64::consts::PI * 125000.0;
        assert!((volume - sphere).abs() < sphere * 1e-3, "the mesh of the sphere holds {volume}, not {sphere}");
        assert!(holds(took, MESH), "{}", over("bringing in 200000 triangles", took, scaled(MESH)));
        assert!(frame <= scaled(qymcad_acceptance::oracles::STEP_FRAME_BUDGET), "a frame after the mesh came in took {frame:?}");
    }
}

probe! {
    budget = 1800;
    /// THE VACUUM CLEANER SAMPLE - a cross-check: it opens whole and in time and rebuilds in time, as the checks on the
    /// samples anyone has say a project does.
    fn the_vacuum_cleaner_sample_opens_and_rebuilds_in_time() {
        let Some(path) = qymcad_acceptance::private_sample("vacuumCleaner.qcad") else { return };
        let p = open_and_rebuild(&path);
        assert!(p.is_empty(), "{}", p.join("; "));
    }
}

probe! {
    budget = 1800;
    /// THE GRIP SAMPLE - a mesh of some 180000 triangles made in another program, a cross-check: it comes into a part
    /// in time, and no frame after it stalls.
    fn the_grip_sample_comes_in_in_time() {
        let Some(grip) = qymcad_acceptance::private_sample("grip.stl") else { return };
        let mut s = Session::start();
        s.budget(Duration::from_secs(1200));
        build::into_the_first_part(&mut s);
        let _ = s.worst_frame();
        let m = meter();
        build::import(&mut s, &grip);
        let doc = s.document();
        let took = spent("bringing in the grip", &m);
        let frame = s.worst_frame();
        assert!(doc.bodies.iter().any(|b| !b.consumed), "the grip brought in nothing; the program says {:?}", s.status());
        assert!(holds(took, MESH), "{}", over("bringing in the grip", took, scaled(MESH)));
        assert!(frame <= scaled(qymcad_acceptance::oracles::STEP_FRAME_BUDGET), "a frame after the grip came in took {frame:?}");
    }
}

/// HOW LONG A CLICK ON AN EDGE MAY TAKE TO BE ANSWERED BY THE TRIAL BUILD, and how long Enter may take to rebuild the
/// propeller below the fillet edited, on the first edit of a document just opened: the trial of the fillet is kept
/// and taken by Enter, the nodes below it are built afresh. Measured in a release build through the window, the machine
/// under a load of 4-10, two runs each: a click 0.98-1.24 s of work, Enter 12.0-12.1 s. A click's trial is OCCT's own
/// fillet of 8 edges against the spline blades, 0.3-0.5 s, and the frames of the window round it; Enter is the fillet
/// of 66 edges below, whose edges will not take together and are tried edge by edge (44 attempts of the kernel on a
/// body of 51 spline faces, 0.1-0.2 s each), the cut through the body and three blends on it. A trial queued behind
/// the trials of every state before, or one rebuilding the whole timeline, is read over the budget of a click (35-37 s
/// a step measured); the checks of a blend run three times over and the volumes of a cut counted exactly, over the
/// budget of Enter (17.5-20.2 s measured).
const TRIAL: Duration = Duration::from_millis(1_300);
const BLEND_ENTER: Duration = Duration::from_secs(14);

/// Is Apply of the tool in hand open?
fn apply_open(s: &mut Session) -> bool {
    let apply = s.word("cmd-apply-enter");
    s.widgets().into_iter().any(|w| w.label.contains(&apply) && w.enabled)
}

probe! {
    budget = 1800;
    /// THE PROPELLER SAMPLE, ITS FILLET EDITED AS A PERSON EDITS IT: the part entered, the fillet R10 at the top of its 11
    /// nodes reopened from the tree, two edges of the blades added by clicks, Enter. The trial of each step answers in time
    /// with Apply open and no Enter pressed, and Enter rebuilds in time. Reported behaviour: each step 35-37 s of work for
    /// its trial, and Enter 40 s more behind them. That a trial running is said at once is checked where the words are
    /// laid (`a_trial_running_is_said_before_enter`): here every step is followed by the checks of every step, which wait
    /// for the program to come to rest.
    fn the_propeller_sample_takes_a_fillet_in_time() {
        let Some(path) = qymcad_acceptance::private_sample("blend-speed/propeller_test_speed.qcad") else { return };
        let mut s = Session::start();
        s.budget(Duration::from_secs(600));
        build::open_project(&mut s, &path);
        let part = s.find("Part 1", qymcad::pos2(0.0, 300.0)).expect("the part is not in the tree");
        std::thread::sleep(Duration::from_millis(700));
        s.double_click(part.center());
        let row = s.words().into_iter().find(|w| w.contains("R10")).expect("the fillet R10 is not in the tree");
        let at = s.find(&row, qymcad::pos2(0.0, 300.0)).expect("the row of the fillet R10 is not on screen");
        std::thread::sleep(Duration::from_millis(700));
        let mut problems = Vec::new();
        let m = meter();
        s.double_click(at.center());
        s.settle();
        let took = spent("the fillet R10 reopened", &m);
        if !holds(took, TRIAL) {
            problems.push(over("the trial of the fillet reopened", took, scaled(TRIAL)));
        }
        // the long top edges of two blades, as the bodies of the sample stand
        for edge in [[21.2, 50.7, 14.0], [-50.7, 21.2, 14.0]] {
            let to = s.edge_at(edge);
            let m = meter();
            s.click(to);
            s.settle();
            let took = spent("an edge of a blade added", &m);
            if !holds(took, TRIAL) {
                problems.push(over("the trial of an edge added", took, scaled(TRIAL)));
            }
            assert!(apply_open(&mut s), "the trial answered and Apply stays shut; the corner says {:?}", s.tool_says());
        }
        let m = meter();
        s.key(qymcad::Key::Enter);
        s.settle();
        let took = spent("Enter on the fillet", &m);
        if !holds(took, BLEND_ENTER) {
            problems.push(over("rebuilding the propeller after Enter", took, scaled(BLEND_ENTER)));
        }
        // THE DOCUMENT AFTER ENTER, as the tree shows it: the fillet R10 of 8 edges with two left sharp (the two clicked
        // are among its eight), every node below it built - the fillet of 66 edges with the edges it could not take said
        // in yellow, not red - and the body
        let doc = s.document();
        let red: Vec<&str> = doc.features.iter().filter(|f| f.error.is_some()).map(|f| f.name.as_str()).collect();
        assert!(red.is_empty(), "nodes did not build after Enter: {red:?}");
        let fillets: Vec<&qymcad::Feature> = doc.features.iter().filter(|f| f.kind == "Fillet").collect();
        let chamfers = doc.features.iter().filter(|f| f.kind == "Chamfer").count();
        assert!(fillets.len() == 5 && chamfers == 1, "the tree holds {} fillets and {chamfers} chamfers after Enter, expecting 5 and 1", fillets.len());
        let r10 = fillets[0];
        assert!(r10.warning.as_deref().is_some_and(|w| w.contains('2') && w.contains('8')), "the fillet R10 does not say two of its 8 edges are left sharp: {:?}", r10.warning);
        assert!(fillets[2].warning.is_some(), "the fillet of 66 edges says nothing of the edges it could not take: {:?}", fillets[2].warning);
        assert!(s.words().iter().any(|w| w.contains("R10") && w.contains('8')), "the tree does not show the fillet R10 of 8 edges");
        assert!(doc.bodies.iter().any(|b| !b.consumed && b.faces > 50), "the propeller has no body of its faces after Enter: {:?}", doc.bodies.iter().map(|b| b.faces).collect::<Vec<_>>());
        assert!(problems.is_empty(), "{}", problems.join("; "));
    }
}
