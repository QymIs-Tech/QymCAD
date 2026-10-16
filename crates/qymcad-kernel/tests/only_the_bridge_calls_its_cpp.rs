//! ONLY THE BRIDGE CALLS ITS C++, AND ONLY NAMED ENTRIES EDIT A BODY IN PLACE.
//!
//! The symbols of the bridge's C++ go into the one program with every crate: a crate that declares `extern "C" { fn
//! qym_... }` itself calls OCCT past `Shape`, past the door and past the lock, and the compiler cannot see it. The
//! declarations live in `qymcad-kernel/src/shape.rs`, private, and nowhere else.
//!
//! An entry of C++ that takes a body to change in place (`QymShape*`, not `const`) changes it past `qym_result`: the
//! removal of strips did so and carried the names by place, renaming the ends of a part with every blend. Such entries
//! are a named list; a new one is red until it is named here with what it does.

/// The only file that declares the C++ of the bridge.
const DECLARED_IN: &str = "qymcad-kernel/src/shape.rs";

/// The entries of C++ that take a body to change in place, and why each may.
const IN_PLACE: [InPlace; 6] = [
    InPlace { name: "qym_shape_free", why: "gives the body back" },
    InPlace { name: "qym_shape_drop_slivers", why: "a repair of the door, its names carried by `qym_result` through the record of the fixing" },
    InPlace { name: "qym_shape_heal_pinched_faces", why: "a repair of the door, its names carried by `qym_result` through the copy and the reshape" },
    InPlace { name: "qym_shape_clear_face_splits", why: "the records of pieces cleared once the names of the pieces are given" },
    InPlace { name: "qym_shape_rename_faces", why: "the names the table of the document gave, landed by the bridge" },
    InPlace { name: "qym_shape_rename_edges", why: "the names the table of the document gave, landed by the bridge" },
];

/// An entry allowed to change a body in place.
struct InPlace {
    name: &'static str,
    why: &'static str,
}

/// Every file of the crates with one of the extensions, by its path from the crates directory; the generators of data
/// for the probes left out.
fn files(ext: &[&str]) -> Vec<(String, String)> {
    let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("the crates directory");
    let mut stack: Vec<std::path::PathBuf> = vec![crates.to_path_buf()];
    let mut out: Vec<(String, String)> = Vec::new();
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).expect("the tree reads").flatten() {
            let p = e.path();
            let name = p.strip_prefix(crates).unwrap_or(&p).to_string_lossy().replace('\\', "/");
            if p.is_dir() {
                if !name.ends_with("target") && !name.ends_with("tests/data") {
                    stack.push(p);
                }
            } else if p.extension().is_some_and(|x| ext.iter().any(|e| x == *e)) {
                out.push((name, std::fs::read_to_string(&p).unwrap_or_default()));
            }
        }
    }
    out
}

#[test]
fn no_crate_but_the_bridge_declares_its_cpp() {
    let all = files(&["rs"]);
    assert!(all.iter().any(|(n, _)| n == DECLARED_IN), "the declarations of the bridge were not found");
    let mut past: Vec<String> = Vec::new();
    for (name, text) in &all {
        if name == DECLARED_IN {
            continue;
        }
        let mut inside = false;
        for (i, line) in text.lines().enumerate() {
            let t = line.trim_start();
            if t.starts_with("//") {
                continue;
            }
            if t.starts_with("extern \"C\" {") || t.starts_with("unsafe extern \"C\" {") {
                inside = true;
            } else if inside && t.starts_with('}') {
                inside = false;
            } else if inside && t.contains("fn qym_") {
                past.push(format!("{name}:{}: {}", i + 1, t));
            }
        }
    }
    assert!(past.is_empty(), "C++ of the bridge declared outside {DECLARED_IN} - a call past `Shape`, the door and the lock:\n{}", past.join("\n"));
}

#[test]
fn only_named_entries_change_a_body_in_place() {
    let mut found: Vec<String> = Vec::new();
    for (_, text) in files(&["cpp"]) {
        for line in text.lines() {
            let t = line.trim_start();
            let Some(rest) = t.strip_prefix("extern \"C\" ") else { continue };
            let Some(open) = rest.find('(') else { continue };
            let (head, args) = rest.split_at(open);
            if args.starts_with("(QymShape* ") || args.starts_with("(QymShape *") {
                found.push(head.split_whitespace().last().unwrap_or_default().trim_start_matches('*').to_string());
            }
        }
    }
    assert!(found.len() >= IN_PLACE.len(), "only {} entries changing a body found: the walk does not see the bridge", found.len());
    let unnamed: Vec<&String> = found.iter().filter(|f| IN_PLACE.iter().all(|p| p.name != f.as_str())).collect();
    assert!(unnamed.is_empty(), "entries of C++ change a body in place past `qym_result` and are not named here: {unnamed:?}");
    let gone: Vec<&str> = IN_PLACE.iter().filter(|p| !found.iter().any(|f| f == p.name)).map(|p| p.name).collect();
    assert!(gone.is_empty(), "entries named as changing a body in place are not in the bridge any more - take them off the list: {gone:?}");
    assert!(IN_PLACE.iter().all(|p| !p.why.is_empty()), "an entry is named without saying why it may");
}
