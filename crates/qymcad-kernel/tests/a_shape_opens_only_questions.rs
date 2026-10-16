//! A BODY OF THE BRIDGE OPENS ONLY QUESTIONS. What makes a body, a mesh, a face or a name - a body placed elsewhere, a
//! piece of it, its triangles, a body read from bytes, the raw records the names are made from - is closed to every crate
//! but the bridge, and the compiler holds it: the window had placed bodies itself to ask a boolean of them beside a
//! running rebuild, meshed them for a file outside the lock, and scaled an import into the cache. Outside the bridge a
//! body is asked about (`qymcad_kernel::ask`), brought in (`qymcad_kernel::imports`) or built by an operation of the
//! kernel through its door; a probe has `qymcad_kernel::specimen`. A method opened again is a door reopened, and this
//! names it.

/// The methods of `Shape` open outside the bridge: questions that make nothing.
const QUESTIONS: [&str; 26] = [
    "to_brep_bytes", "volume", "sliver_faces", "is_sheet", "is_empty_shape", "shell_count", "shares_geometry", "solid_count", "solids_info", "face_kinds", "is_valid", "edges", "edges_with_ids",
    "edges_full", "edges_full_smooth", "edges_info", "smooth_edge_ids", "sharp_edge_ids", "min_round_radius", "face_axis", "face_cylinder", "face_edge_ids", "face_areas", "bbox", "bbox_diag",
    "of_extension",
];

#[test]
fn a_shape_opens_nothing_but_its_questions() {
    // the shape and its modules: the methods of `Shape` stand in `shape/` as well
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src/shape");
    let mut text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/shape.rs")).expect("the shape reads");
    for e in std::fs::read_dir(dir).expect("the modules of the shape read").flatten() {
        text.push_str(&std::fs::read_to_string(e.path()).expect("a module of the shape reads"));
    }
    let open: Vec<&str> = text.lines().filter_map(|l| l.strip_prefix("    pub fn ")).filter_map(|rest| rest.split('(').next()).collect();
    assert!(open.len() >= 20, "only {} open methods found: the walk does not see the shape", open.len());
    let opened: Vec<&&str> = open.iter().filter(|name| !QUESTIONS.contains(name)).collect();
    assert!(opened.is_empty(), "methods of the shape open outside the bridge that are no question: {opened:?} - a body, a mesh or a name made past the door");
}
