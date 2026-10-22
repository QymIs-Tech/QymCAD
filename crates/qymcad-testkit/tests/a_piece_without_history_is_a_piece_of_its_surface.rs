//! A PIECE THE HISTORY DOES NOT TELL IS A PIECE OF THE FACE IT LIES ON. Measured on the filter: a window cut through a
//! tube leaves each of its walls in two halves; widened from 22 to 24.2, the operation told the kernel's history of only
//! the upper half of a wall, the lower half - the one keeping the name of the whole - came with no history, was named as
//! a seam after the tube beside it, and the upper half took the name of the whole. The two halves lie on one surface of
//! the kernel; a face with no history on the surface of named faces all of one face is a piece of it
//! (`crates/qymcad-kernel/NAMING.md`, "A face split in pieces").
//!
//! The filter as it ships, the width of the window edited the way the matrix of names edits it and undone: the half
//! keeping the name of the whole keeps it. Live OCCT, headless.
use qymcad_core::names::{GeoName, Role};
use qymcad_testkit::real_documents::{self, Edit};

#[test]
fn the_half_keeping_the_name_of_the_whole_keeps_it_when_the_window_widens() {
    let doc = real_documents::documents().into_iter().find(|d| d.name == "Filter-v2.qcad").expect("the filter ships with the program");
    let mut o = real_documents::open(&doc);
    let wall = GeoName::new(206, Role::Wall, 170);
    let whole = (0..o.project.names.faces().len())
        .map(|i| qymcad_core::names::NAMED | i as u32)
        .find(|d| o.project.names.get(*d) == Some(wall))
        .expect("setup: the wall of the window, line 170 of node 206, is named");
    let widen = real_documents::edits(&o.project)
        .into_iter()
        .find(|e| matches!(e, Edit::Sketch { sketch: 53, refs, .. } if *refs == [163, 164]))
        .expect("setup: the width of the window, 163 to 164 of sketch 53");
    let faults = real_documents::edit_and_back(&mut o, &widen, 1.1);
    let renamed: Vec<&String> = faults.iter().filter(|f| f.contains(&format!("was {whole} ="))).collect();
    assert!(renamed.is_empty(), "the half keeping the name of the whole was renamed when the window widened:\n  {renamed:?}");
}
