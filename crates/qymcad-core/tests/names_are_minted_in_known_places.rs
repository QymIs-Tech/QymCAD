//! A NAME OF A FACE, AN EDGE OR A VERTEX IS MINTED IN THE PLACES THAT NAME BY RECIPE, and nowhere else. A name minted
//! inline by a new operation is a scheme of its own: it is not the one rule for what the history is silent about, it is
//! not covered by the matrices of names, and the next edit above it finds what nobody checked.
//!
//! The check reads the sources of every crate. A call that mints - `intern_name`, `intern_face`, `intern_edge`,
//! `intern_vertex` - stands only in a function of the list below; code under `#[cfg(test)]` and in `tests/` is let be.
//! A new place goes on the list on purpose, with its row in `every_operation_names_what_it_makes`.

/// THE PLACES THAT MINT NAMES, by file and function, each with why it may: the door of the bridge and what the bridge
/// names where the history of an operation is silent - the rules of `crates/qymcad-kernel/NAMING.md` - and the table.
const PLACES: [Place; 8] = [
    Place { file: "qymcad-kernel/src/naming.rs", function: "land_roles", why: "the roles an operation of the bridge gave, named when its result lands" },
    Place { file: "qymcad-kernel/src/naming.rs", function: "pieces", why: "a piece of a split face, told by what cut it and what stands beside it" },
    Place { file: "qymcad-kernel/src/naming.rs", function: "seams", why: "a face the history was silent about, named after a named neighbour" },
    Place { file: "qymcad-kernel/src/naming.rs", function: "placed", why: "a face with no history and no named neighbour - a body brought in from a file or made of a mesh - by where it lies" },
    Place { file: "qymcad-kernel/src/naming.rs", function: "edges", why: "an edge, after the pair of faces it runs between" },
    Place { file: "qymcad-core/src/model/refs.rs", function: "vertex_pool", why: "a vertex the bridge named (`MeshEdge::ends`), taken into the table when first referenced" },
    Place { file: "qymcad-core/src/model/timeline/clone.rs", function: "clone_subtree_into", why: "a part copied into another document takes the names it has into that document's table" },
    Place { file: "qymcad-core/src/names.rs", function: "*", why: "the table of names itself" },
];

/// A place allowed to mint names: a file of the tree, a function of it (`*` - all of the file), and why it may.
struct Place {
    file: &'static str,
    function: &'static str,
    why: &'static str,
}

/// A call that mints a name, met in the sources.
struct Mint {
    file: String,
    line: usize,
    function: String,
}

const MINTING: [&str; 4] = ["intern_name(", "intern_face(", "intern_edge(", "intern_vertex("];

/// Every `.rs` file under `src/` of every crate, by its path from the crates directory.
fn sources() -> Vec<(String, String)> {
    let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("the crates directory");
    let mut stack: Vec<std::path::PathBuf> = vec![crates.to_path_buf()];
    let mut out: Vec<(String, String)> = Vec::new();
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).expect("the tree reads").flatten() {
            let p = e.path();
            let name = p.strip_prefix(crates).unwrap_or(&p).to_string_lossy().replace('\\', "/");
            if p.is_dir() {
                if !name.ends_with("/tests") && !name.ends_with("target") {
                    stack.push(p);
                }
                continue;
            }
            if name.contains("/src/") && p.extension().is_some_and(|x| x == "rs") && !name.ends_with("tests.rs") {
                out.push((name, std::fs::read_to_string(&p).unwrap_or_default()));
            }
        }
    }
    assert!(out.iter().any(|(n, _)| n.ends_with("qymcad-core/src/model/regen.rs")), "the walk did not find the rebuild");
    out
}

/// The calls that mint a name in one file, outside `#[cfg(test)]` items, with the function each stands in.
fn mints(file: &str, text: &str) -> Vec<Mint> {
    let mut out: Vec<Mint> = Vec::new();
    let mut function = String::new();
    let mut skip_depth: Option<i64> = None;
    let mut depth: i64 = 0;
    let mut test_next = false;
    for (n, raw) in text.lines().enumerate() {
        let line = raw.split("//").next().unwrap_or("");
        if raw.trim_start().starts_with("#[cfg(test)]") {
            test_next = true;
        }
        let opens = line.matches('{').count() as i64;
        let closes = line.matches('}').count() as i64;
        if test_next && opens > 0 {
            skip_depth = Some(depth);
            test_next = false;
        }
        if skip_depth.is_none() {
            let t = line.trim_start();
            let head = t.trim_start_matches("pub(crate) ").trim_start_matches("pub(super) ").trim_start_matches("pub ");
            if let Some(rest) = head.strip_prefix("fn ") {
                function = rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            } else if MINTING.iter().any(|m| line.contains(m)) {
                out.push(Mint { file: file.to_string(), line: n + 1, function: function.clone() });
            }
        }
        depth += opens - closes;
        if skip_depth.is_some_and(|d| depth <= d) && closes > 0 {
            skip_depth = None;
        }
    }
    out
}

#[test]
fn names_are_minted_in_the_places_that_name_by_recipe() {
    let mut caught: Vec<String> = Vec::new();
    let mut seen = 0usize;
    for (file, text) in sources() {
        for m in mints(&file, &text) {
            seen += 1;
            let allowed = PLACES.iter().any(|p| p.file == m.file && (p.function == "*" || p.function == m.function));
            if !allowed {
                caught.push(format!("{}:{} in `{}`", m.file, m.line, m.function));
            }
        }
    }
    // 37 such calls on 12.10.2026; 11 on 13.10, once the roles of every operation moved to the bridge and the model
    // minted no face names of its own; a parse that found none would pass all
    assert!(seen >= 9, "the walk found {seen} calls that mint a name, fewer than the tree has - it is reading them wrong");
    assert!(PLACES.iter().all(|p| !p.why.is_empty()), "a place on the list says not why it may mint");
    assert!(caught.is_empty(), "a name is minted outside the places that name by recipe:\n{}", caught.join("\n"));
}

/// THE CHECK SEES WHAT IT IS FOR: a mint in a function of its own is caught, one under `#[cfg(test)]` is not.
#[test]
fn the_check_tells_a_new_place_from_a_test() {
    let text = [
        "fn lay_my_tool(&mut self) {",
        "    let d = self.intern_name(f, Role::Wall, 1);",
        "}",
        "#[cfg(test)]",
        "mod tests {",
        "    fn t() { p.names.intern_face(g); }",
        "}",
    ]
    .join("\n");
    let found: Vec<String> = mints("x.rs", &text).into_iter().map(|m| m.function).collect();
    assert_eq!(found, ["lay_my_tool"], "the mints are not told apart");
}
