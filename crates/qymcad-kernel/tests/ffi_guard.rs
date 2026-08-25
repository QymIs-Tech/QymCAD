//! The C-ABI boundary with the kernel has to catch everything.
//!
//! An exception escaping through `extern "C"` is not an error but an abort of the process, seen from outside as
//! the program simply closing. The most likely sources are checked here — a damaged file and plainly impossible
//! geometry — and an honest refusal is expected rather than a crash. The test is itself the check of
//! survivability: if the process dies, the harness shows it.

#[test]
fn broken_step_returns_none_not_abort() {
    let dir = std::env::temp_dir();
    // rubbish in place of a STEP file
    let junk = dir.join("qym_ffi_junk.step");
    std::fs::write(&junk, b"\x00\x01\x02 not a step file at all \xff\xfe").unwrap();
    assert!(qymcad_kernel::step_solids(junk.to_string_lossy().as_ref()).map(|v| v.is_empty()).unwrap_or(true), "rubbish yields no solids");
    assert!(qymcad_kernel::import_step(junk.to_string_lossy().as_ref(), 0.5).is_err(), "rubbish gives an honest error");

    // a truncated but genuine STEP file, with a header and a cut-off body: the most treacherous case
    let cube = qymcad_kernel::Shape::extrude(&[0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0], 10.0).expect("the cube");
    let full = dir.join("qym_ffi_full.step");
    qymcad_kernel::write_step(&[(&cube, qymcad_core::feature::PLACE_IDENTITY)], full.to_string_lossy().as_ref()).expect("step");
    let bytes = std::fs::read(&full).unwrap();
    let cut = dir.join("qym_ffi_cut.step");
    std::fs::write(&cut, &bytes[..bytes.len() / 2]).unwrap();
    let _ = qymcad_kernel::step_solids(cut.to_string_lossy().as_ref()); // what matters is the fact that the process survives
    let _ = qymcad_kernel::import_step(cut.to_string_lossy().as_ref(), 0.5);

    // a path that does not exist
    assert!(qymcad_kernel::step_solids("/no/such/file.step").map(|v| v.is_empty()).unwrap_or(true));
    for f in [junk, full, cut] {
        let _ = std::fs::remove_file(f);
    }
}

#[test]
fn degenerate_geometry_returns_none_not_abort() {
    // a degenerate profile of zero area and a zero height: the kernel throws on these rather than returning
    let degenerate = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    assert!(qymcad_kernel::Shape::extrude(&degenerate, 10.0).is_none(), "a degenerate profile gives None");
    let square = [0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0];
    assert!(qymcad_kernel::Shape::extrude(&square, 0.0).is_none(), "a zero height gives None");
    assert!(qymcad_kernel::Shape::revolve(&degenerate, 1, 360.0).is_none(), "a degenerate revolution gives None");

    // a boolean of two disjoint bodies, and operations with an empty result
    let a = qymcad_kernel::Shape::extrude(&square, 10.0).expect("a");
    let far = [100.0, 100.0, 110.0, 100.0, 110.0, 110.0, 100.0, 110.0];
    let b = qymcad_kernel::Shape::extrude(&far, 10.0).expect("b");
    let common = a.boolean(&b, 2); // there is no intersection
    assert!(common.is_none() || common.unwrap().volume() < 1e-9, "an empty intersection is not a crash");

    // tessellation with an absurd deflection
    let s = qymcad_kernel::Shape::extrude(&square, 10.0).expect("s");
    let _ = s.tessellate(-5.0);
    let _ = s.tessellate(f64::MAX);
    assert!(!s.tessellate(0.5).is_empty(), "after the absurd calls the kernel is alive and computes normally");
}
