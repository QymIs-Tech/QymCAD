//! SIZE AND TIME: projects of real size opened and rebuilt, and meshes of real size brought in, within the time a
//! person waits for in a professional system - a project open in seconds, rebuilt in under half a minute, a mesh of
//! two hundred thousand triangles in under half a minute, and no frame of the window stalled past the budget of every
//! step. The set is a debug build, several times slower than what people run: the budgets are set for it.
//!
//! The private samples (`samples/`, not published) close the letter as a cross-check: what they show must already be
//! caught by the checks built on files anyone has.
use std::time::{Duration, Instant};

use qymcad::Session;
use qymcad_acceptance::{build, probe};

/// How long opening a project may take, and rebuilding everything in it.
const OPEN: Duration = Duration::from_secs(10);
const REBUILD: Duration = Duration::from_secs(30);
/// How long bringing in a mesh of two hundred thousand triangles may take.
const MESH: Duration = Duration::from_secs(30);

/// A BUDGET ON THIS MACHINE: the budget times `QYMCAD_TIME_SCALE` (1 when unset). The budgets are set for the
/// developer's machine; a CI runner of four cores rebuilt the robot sample in 39.4 s against the 26.4 s of the whole
/// probe here, so CI runs these probes alone and at twice the budget - a run twice as slow as it should be still goes
/// red there, and the budgets here stay as they are.
fn scaled(budget: Duration) -> Duration {
    let k = std::env::var("QYMCAD_TIME_SCALE").ok().and_then(|v| v.parse::<f64>().ok()).filter(|k| *k >= 1.0).unwrap_or(1.0);
    budget.mul_f64(k)
}

/// Open `path`, time it, rebuild everything, time that: the problems, in words.
fn open_and_rebuild(path: &str) -> Vec<String> {
    let mut s = Session::start();
    s.budget(Duration::from_secs(600));
    let t = Instant::now();
    build::open_project(&mut s, path);
    let doc = s.document();
    let opened = t.elapsed();
    let t = Instant::now();
    let (edit, rebuild) = (s.word("menu-edit"), s.word("menu-rebuild"));
    s.menu(&[&edit, &rebuild]);
    let _ = s.document();
    let rebuilt = t.elapsed();
    let mut problems = Vec::new();
    let (open, rebuild) = (scaled(OPEN), scaled(REBUILD));
    if opened > open {
        problems.push(format!("opening took {opened:?}, over {open:?} ({} nodes, {} faces)", doc.features.len(), doc.bodies.iter().map(|b| b.faces).sum::<usize>()));
    }
    if rebuilt > rebuild {
        problems.push(format!("rebuilding everything took {rebuilt:?}, over {rebuild:?}"));
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

probe! {
    budget = 900;
    /// THE SAMPLE PROJECT OF A ROBOT opens whole - every part one body, no two parts of one name - and in time, and
    /// rebuilds in time.
    fn the_robot_sample_opens_and_rebuilds_in_time() {
        let p = open_and_rebuild(&sample("robotic_2.qcad"));
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
        let t = Instant::now();
        build::import(&mut s, &path);
        let doc = s.document();
        let took = t.elapsed();
        let frame = s.worst_frame();
        let volume: f64 = doc.bodies.iter().filter(|b| !b.consumed).map(|b| b.volume).sum();
        let sphere = 4.0 / 3.0 * std::f64::consts::PI * 125000.0;
        assert!((volume - sphere).abs() < sphere * 1e-3, "the mesh of the sphere holds {volume}, not {sphere}");
        assert!(took <= scaled(MESH), "bringing in 200000 triangles took {took:?}, over {:?}", scaled(MESH));
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
        let t = Instant::now();
        build::import(&mut s, &grip);
        let doc = s.document();
        let took = t.elapsed();
        let frame = s.worst_frame();
        assert!(doc.bodies.iter().any(|b| !b.consumed), "the grip brought in nothing; the program says {:?}", s.status());
        assert!(took <= scaled(MESH), "bringing in the grip took {took:?}, over {:?}", scaled(MESH));
        assert!(frame <= scaled(qymcad_acceptance::oracles::STEP_FRAME_BUDGET), "a frame after the grip came in took {frame:?}");
    }
}
