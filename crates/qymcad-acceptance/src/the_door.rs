//! THE CHECKS HAVE ONE DOOR INTO THE PROGRAM, and this holds it.
//!
//! The compiler already refuses most of a way around it - the application is private to its crate. What it does
//! not refuse is a second dependency, a door the program opens wider than its session, or a name of the inside
//! written where a check can reach it; those are read here.
use std::path::{Path, PathBuf};

/// The root of this crate.
fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Every Rust file under `dir`, recursively.
fn sources(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return out };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(sources(&p));
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
    out
}

/// The code of a file without its comments: a word of the inside may be explained, not used.
fn code_of(text: &str) -> String {
    text.lines().map(|l| l.split("//").next().unwrap_or("")).collect::<Vec<_>>().join("\n")
}

/// Does `word` stand in `code` as a whole word?
fn has_word(code: &str, word: &str) -> bool {
    let inner = |c: char| c.is_alphanumeric() || c == '_';
    code.match_indices(word).any(|(i, _)| {
        let before = code[..i].chars().next_back().is_none_or(|c| !inner(c));
        let after = code[i + word.len()..].chars().next().is_none_or(|c| !inner(c));
        before && after
    })
}

/// THE WAYS AROUND THE WINDOW that `code` takes, named.
fn ways_around(code: &str) -> Vec<String> {
    // the application object, and the crates of the inside a check would read or drive it through
    const WORDS: [&str; 15] = [
        "App", "unsafe", "egui", "eframe", "qymcad_core", "qymcad_kernel", "qymcad_io", "qymcad_ui_state", "qymcad_sketch", "qymcad_part", "qymcad_assembly", "qymcad_i18n", "qymcad_testkit",
        "qymcad_paths", "qymcad_render",
    ];
    // handlers the window calls itself, rebuilds it starts itself, and facades made for checks
    const PIECES: [&str; 10] = [
        "_for_test",
        "start_feat_cmd",
        "apply_feat_cmd",
        "viewport_3d_click_at",
        "regenerate",
        "rebuild_if_dirty",
        "resync_after",
        "drain_bg",
        "wait_bg",
        "std::process::Command",
    ];
    let mut out: Vec<String> = WORDS.iter().filter(|w| has_word(code, w)).map(|w| format!("the word `{w}`")).collect();
    out.extend(PIECES.iter().filter(|p| code.contains(*p)).map(|p| format!("`{p}`")));
    out
}

/// THE DEPENDENCIES a manifest declares, of every kind and for every target.
fn dependencies(manifest: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_deps = false;
    for line in manifest.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_deps = t.trim_matches(|c| c == '[' || c == ']').ends_with("dependencies");
            continue;
        }
        if in_deps && !t.is_empty() && !t.starts_with('#') {
            out.push(t.split(['=', '.']).next().unwrap_or("").trim().to_string());
        }
    }
    out
}

/// THE CHECKS REACH THE PROGRAM THROUGH ITS SESSION ALONE: no second dependency, no word of the inside.
#[test]
fn the_checks_reach_the_program_through_its_session_alone() {
    let manifest = std::fs::read_to_string(here().join("Cargo.toml")).expect("the manifest reads");
    let deps = dependencies(&manifest);
    assert_eq!(deps, vec!["qymcad".to_string()], "the acceptance checks depend on more than the program");

    let files: Vec<PathBuf> = [here().join("src"), here().join("tests")].iter().flat_map(|d| sources(d)).collect();
    assert!(files.iter().any(|f| f.starts_with(here().join("tests"))), "the walk found no checks at all - it reads the wrong place");
    let mut found = Vec::new();
    for f in files.iter().filter(|f| f.file_name().is_some_and(|n| n != "the_door.rs")) {
        let text = std::fs::read_to_string(f).expect("a source reads");
        // THE ONE FILE THAT STARTS A PROCESS starts the test binary itself, to give each check a process of its own
        let allowed: &[&str] = if f.ends_with("src/isolation.rs") { &["`std::process::Command`"] } else { &[] };
        for way in ways_around(&code_of(&text)).into_iter().filter(|w| !allowed.contains(&w.as_str())) {
            found.push(format!("{}: {way}", f.strip_prefix(here()).unwrap_or(f).display()));
        }
    }
    assert!(found.is_empty(), "a check goes around the window:\n{}", found.join("\n"));
}

/// EVERY CHECK RUNS IN A PROCESS OF ITS OWN, and every file of checks holds some.
///
/// A bare `#[test]` runs in the process of all the others, where its kernel meets theirs. And a file of checks
/// that lost them compiles and passes: measured once, when a file of fifteen checks was written back empty and the
/// run went green on the rest.
#[test]
fn every_check_runs_in_a_process_of_its_own() {
    let checks = here().join("tests");
    let mut bare = Vec::new();
    let mut empty = Vec::new();
    for f in sources(&checks) {
        let code = code_of(&std::fs::read_to_string(&f).expect("a source reads"));
        let name = f.strip_prefix(here()).unwrap_or(&f).display().to_string();
        if code.contains("#[test]") {
            bare.push(name.clone());
        }
        // a check is written with `probe!`, or with `contract!`, which writes every point of a contract with `probe!`
        if f.file_name().is_some_and(|n| n != "main.rs") && !code.contains("probe! {") && !code.contains("contract!(") {
            empty.push(name);
        }
    }
    assert!(bare.is_empty(), "a check is written with a bare #[test] and runs beside the others, kernel and all: {bare:?}");
    assert!(empty.is_empty(), "a file of checks holds no check: {empty:?}");
}

/// THE PROGRAM OPENS NO DOOR BUT ITS SESSION: everything its library makes public is the start of the program
/// and the session.
#[test]
fn the_program_opens_no_door_but_its_session() {
    let program = here().join("../qymcad/src");
    let root = std::fs::read_to_string(program.join("lib.rs")).expect("the program's library reads");
    let allowed = ["pub fn run(", "pub use gui::session::{"];
    let root_code = code_of(&root);
    let open: Vec<&str> = root_code.lines().map(str::trim).filter(|l| l.starts_with("pub ") && !allowed.iter().any(|a| l.starts_with(a))).collect();
    assert!(open.is_empty(), "the program's library opens a door beside its session:\n{}", open.join("\n"));
    // and the session hands out nothing of the inside: no application, no window, no context
    // the session and its modules: its methods stand in `gui/session/` as well
    let mut whole = std::fs::read_to_string(program.join("gui/session.rs")).expect("the session reads");
    for e in std::fs::read_dir(program.join("gui/session")).expect("the modules of the session read").flatten() {
        whole.push_str(&std::fs::read_to_string(e.path()).expect("a module of the session reads"));
    }
    let session = code_of(&whole);
    let leaks: Vec<&str> = session.lines().filter(|l| l.trim_start().starts_with("pub fn") && ["App", "Window", "Context"].iter().any(|w| has_word(l, w))).collect();
    assert!(leaks.is_empty(), "the session hands the inside of the program out:\n{}", leaks.join("\n"));
}

/// THE DOOR'S GUARD IS NOT BLIND: every way around the window, written into a check, is caught by name.
#[test]
fn every_way_around_the_window_is_caught() {
    let sample = "let a: App = x; unsafe {} egui::Key; eframe::run; qymcad_core::X; qymcad_kernel::X; qymcad_io::X; qymcad_ui_state::X; qymcad_sketch::X; \
                  qymcad_part::X; qymcad_assembly::X; qymcad_i18n::tr; qymcad_testkit::X; qymcad_paths::X; qymcad_render::X; s.drain_bg_for_test(); \
                  s.start_feat_cmd(1); s.apply_feat_cmd(); s.viewport_3d_click_at(p); regenerate_all(c); rebuild_if_dirty(c); resync_after_topology_change(); \
                  wait_bg(); std::process::Command::new(\"x\");";
    let found = ways_around(sample);
    assert_eq!(found.len(), 15 + 10, "the guard missed a way around the window; it found only:\n{}", found.join("\n"));
    assert!(ways_around(&code_of("let happy = Apple; // App")).is_empty(), "the guard takes a word inside another word, or a comment, for a way around");
    let manifest = "[dependencies]\nqymcad = { path = \"../qymcad\" }\n[dev-dependencies]\negui = \"0.35\"\n[target.'cfg(unix)'.dependencies]\nlibc.workspace = true\n";
    assert_eq!(dependencies(manifest), ["qymcad", "egui", "libc"], "the guard misses a dependency declared under another heading");
}
