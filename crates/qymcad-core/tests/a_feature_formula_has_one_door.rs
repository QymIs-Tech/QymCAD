//! A FORMULA OF A FEATURE'S NUMBER GOES INTO THE DOCUMENT BY ONE DOOR, `Project::write_feat_formula`, which asks
//! `Project::check_formula` and keeps the rule of a plain number. Outside the core, no code but a check writes
//! `feat_dims` past it: a field that wrote around the door took `n+1` for the count of an array named `n`.
//!
//! The code of a check stands in a `tests` directory, in a file named `tests.rs`, or after the first `#[cfg(test)]` of
//! its file; a check plants a document as a file on disk holds it, and may.

fn rust_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            rust_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

#[test]
fn no_code_but_the_core_writes_a_feature_formula_past_its_door() {
    let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    for c in std::fs::read_dir(&crates).expect("the crates of the workspace").flatten() {
        if c.file_name() != "qymcad-core" {
            rust_files(&c.path().join("src"), &mut files);
        }
    }
    assert!(files.len() > 100, "the walk found {} files: the crates are not where they were", files.len());
    let mut past = Vec::new();
    for f in files {
        let path = f.to_string_lossy().replace('\\', "/");
        if path.contains("/tests/") || path.ends_with("tests.rs") {
            continue;
        }
        let text = std::fs::read_to_string(&f).unwrap_or_default();
        for (i, line) in text.lines().enumerate() {
            if line.trim() == "#[cfg(test)]" {
                break;
            }
            if line.contains(".set_feat_dim(") {
                past.push(format!("{path}:{}", i + 1));
            }
        }
    }
    assert!(past.is_empty(), "a feature's formula is written past `write_feat_formula`: {past:?}");
}
