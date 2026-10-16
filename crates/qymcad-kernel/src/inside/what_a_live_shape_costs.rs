//! WHAT A LIVE BODY COSTS IN MEMORY - a measurement, not a check.
//!
//! Reported behaviour: with the reference V8 engine open (a STEP of 374 MB, 1170 imported solids) the program
//! held 5.24 GB and the window went sluggish. Part of that is the scene on the card, and part is the LIVE
//! B-REP: for an imported body the kernel shape cannot be rebuilt from a recipe, only by parsing the embedded
//! STEP again, so it is kept for as long as the document is open.
//!
//! Before deciding whether to let those shapes go and restore them on demand, the price has to be known. The
//! number here is the resident memory of the process, because there is no way to ask a kernel shape how much
//! it occupies - it is a tree of C++ objects behind a handle.
//!
//! `QYM_STEP` points the measurement at any file; with none given it makes its own from a body of 6 faces
//! written `QYM_STEP_COPIES` times over (200 by default).

/// The resident size of this process in bytes, or None where the system does not tell.
fn rss() -> Option<u64> {
    let statm = std::fs::read_to_string("/proc/self/statm").ok()?;
    let pages: u64 = statm.split_whitespace().nth(1)?.parse().ok()?;
    Some(pages * 4096)
}

#[test]
#[ignore = "a measurement, not a check: prints numbers"]
fn what_a_live_shape_costs() {
    let copies: usize = std::env::var("QYM_STEP_COPIES").ok().and_then(|s| s.parse().ok()).unwrap_or(200);
    let path = match std::env::var("QYM_STEP") {
        Ok(p) => p,
        Err(_) => {
            // a body of our own, written as many times as asked - each copy is a solid of its own in the file
            let cube = crate::Shape::extrude(&[0.0, 0.0, 20.0, 0.0, 20.0, 20.0, 0.0, 20.0], 20.0).expect("the body");
            let mut at: Vec<[f64; 12]> = Vec::new();
            for i in 0..copies {
                let mut p = qymcad_core::feature::PLACE_IDENTITY;
                p[3] = i as f64 * 30.0;
                at.push(p);
            }
            let bodies: Vec<(&crate::Shape, [f64; 12])> = at.iter().map(|p| (&cube, *p)).collect();
            let out = std::env::temp_dir().join("qym_shape_cost.step");
            crate::write_step(&bodies, out.to_str().expect("the path")).expect("the file is written");
            out.to_string_lossy().into_owned()
        }
    };

    // THE COST IS MEASURED AS A DIFFERENCE, not as a level. The resident size of a process says as much
    // about the allocator's spare room as about what is held: writing the file above left tens of megabytes
    // of arena behind, and the first reading warms it. So the shapes are read TWICE and the answer is what
    // the second reading ADDED - the same work on an already warm allocator.
    let warm = crate::step_solids(&path).expect("the file is read");
    let n = warm.len();
    // HOW MANY FACES THE SOLIDS CARRY - the number that scales to another file. A kernel shape is a tree of
    // faces, edges and curves, so the memory follows the faces rather than the count of solids: a bolt and an
    // engine block are both one solid.
    let faces: usize =
        warm.iter().map(|s| s.tessellate_auto(qymcad_core::model::GeomQuality::Normal.deflection_k()).first().map(|qymcad_core::geom::Built { faces: f, .. }| f.len()).unwrap_or(0)).sum();
    let one = rss();

    let t0 = std::time::Instant::now();
    let again = crate::step_solids(&path).expect("the file is read a second time");
    let read = t0.elapsed();
    let two = rss();
    assert_eq!(again.len(), n, "the two readings of the same file gave different numbers of solids");

    let grew = match (one, two) {
        (Some(a), Some(b)) => b.saturating_sub(a),
        _ => 0,
    };
    drop(again);
    let after_one = rss();
    drop(warm);
    let after_none = rss();

    let mb = |v: Option<u64>| v.map(|b| format!("{:.1}", b as f64 / 1048576.0)).unwrap_or_else(|| "-".into());
    let per = |d: usize| if d > 0 { format!("{:.1} KB", grew as f64 / d as f64 / 1024.0) } else { "-".into() };
    eprintln!(
        "MEASURED {path}\n  solids {n}, faces {faces}, a reading takes {:.0} ms\n  \
         resident: one copy {} MB, two copies {} MB (the second added {:.1} MB), \
         after letting one go {} MB, after letting both go {} MB\n  \
         about {} per solid, {} per face",
        read.as_secs_f64() * 1000.0,
        mb(one),
        mb(two),
        grew as f64 / 1048576.0,
        mb(after_one),
        mb(after_none),
        per(n),
        per(faces),
    );
}
