//! THE SPECIMENS OF THE BRIDGE ARE ASKED FOR BY THE PROBES ALONE.
//!
//! `qymcad_kernel::specimen` builds bodies past the door: right for a body a probe measures, wrong for a document. It
//! exists only under the `specimens` feature, and the build of the application (`tools/gate.py`, "the application
//! builds as it ships") is the proof that no code of the window builds with it - as long as no crate asks for the
//! feature outside its dev-dependencies. This reads every manifest of the tree and holds that.

/// Every `Cargo.toml` of the crates, as text.
fn manifests() -> Vec<(String, String)> {
    let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("the crates directory");
    let mut out = Vec::new();
    for e in std::fs::read_dir(crates).expect("the crates read").flatten() {
        let p = e.path().join("Cargo.toml");
        if let Ok(text) = std::fs::read_to_string(&p) {
            out.push((e.file_name().to_string_lossy().to_string(), text));
        }
    }
    assert!(out.len() >= 8, "the walk found {} manifests, fewer than the tree has", out.len());
    out
}

#[test]
fn only_a_dev_dependency_asks_for_the_specimens() {
    let mut caught = Vec::new();
    let mut asked = 0usize;
    for (name, text) in manifests() {
        let mut section = String::new();
        for line in text.lines() {
            let t = line.trim();
            if t.starts_with('[') {
                section = t.to_string();
                continue;
            }
            if t.starts_with('#') || !t.contains("\"specimens\"") {
                continue;
            }
            if section.ends_with("dev-dependencies]") {
                asked += 1;
            } else if !(name == "qymcad-kernel" && section == "[features]") {
                caught.push(format!("{name}: {section} {t}"));
            }
        }
    }
    assert!(asked >= 1, "no probe asks for the specimens - the check reads the wrong place");
    assert!(caught.is_empty(), "the specimens are asked for outside the probes:\n{}", caught.join("\n"));
}
