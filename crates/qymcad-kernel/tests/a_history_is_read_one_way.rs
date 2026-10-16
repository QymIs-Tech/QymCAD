//! THE HISTORY OF THE LOW-LEVEL ALGORITHMS IS READ THROUGH ONE TRANSLATION.
//!
//! OCCT says what became of an element in three ways: a builder of shapes answers `Modified`/`Generated` itself, the
//! low-level booleans and the merging of a monolith hand a `BRepTools_History`, a repair keeps a record of its own. The
//! carrying of names asks one question (`QymHistory`) and each way has its translation in `occt_common.hpp`. A history
//! handle read in place - `hist->Modified(s)` - is a carrying of its own beside the one the bridge holds, and the fixes
//! of that one (a name beats a number, a piece yields another's number) do not reach it.

/// EVERY C++ FILE OF THE WHOLE TREE, as text, from the crates directory: a history read in a new file, or in another
/// crate, is looked at too.
fn sources() -> Vec<(String, String)> {
    let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("the crates directory");
    let mut stack: Vec<std::path::PathBuf> = vec![crates.to_path_buf()];
    let mut out: Vec<(String, String)> = Vec::new();
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).expect("the tree reads").flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            if p.extension().is_none_or(|x| x != "cpp" && x != "hpp") {
                continue;
            }
            let name = p.strip_prefix(crates).unwrap_or(&p).to_string_lossy().replace('\\', "/");
            out.push((name, std::fs::read_to_string(&p).unwrap_or_default()));
        }
    }
    assert!(out.len() >= 4, "the walk found {} C++ files, which is fewer than the tree has - it is looking in the wrong place", out.len());
    assert!(out.iter().any(|(n, _)| n.ends_with("occt_common.hpp")), "the translations themselves were not found by the walk");
    out
}

/// The file that holds the translations.
const TRANSLATIONS: &str = "occt_common.hpp";

/// What a history handle is asked through its arrow, and the record of a repair named at all: it is taken straight
/// into its translation (`QymRepairHistory(fx.Context(), ...)`), never held and asked in place.
const READS: [&str; 4] = ["->Modified(", "->Generated(", "->IsRemoved(", "ShapeBuild_ReShape"];

#[test]
fn a_history_handle_is_read_only_by_its_translation() {
    let mut caught: Vec<String> = Vec::new();
    for (name, text) in sources() {
        if name.ends_with(TRANSLATIONS) {
            continue;
        }
        for (n, line) in text.lines().enumerate() {
            let t = line.trim_start();
            if t.starts_with("//") || t.starts_with('*') {
                continue;
            }
            if READS.iter().any(|r| line.contains(r)) {
                caught.push(format!("{name}:{}: {}", n + 1, t));
            }
        }
    }
    assert!(caught.is_empty(), "a history is read in place, past its translation:\n{}", caught.join("\n"));
}
