//! A field of the document that is missing from the file schema is lost silently.
//!
//! The format on disk is a separate type (`doc_file::DocumentFile`), and rightly so: the in-memory model is free
//! to change. The price is that the connection between them rests on attention alone — add a field to `Project`,
//! forget it in the schema, and the program builds, the tests stay green, and document properties are filled in
//! and lost on save without an error or a hint.
//!
//! That is exactly what happened to the document properties (author, title, version, comment): they were absent
//! from the schema entirely, while the tests nearby checked the creation date and whether anything leaked into
//! the settings — but not the file. This guard closes the whole class: every field of the model is either in the
//! schema or named here with a reason.

/// The field names of a struct, read from the source. A crude parse, but exact for the declaration style used
/// here.
fn fields_of(src: &str, decl: &str) -> Vec<String> {
    let a = src.find(decl).unwrap_or_else(|| panic!("the struct {decl} was not found"));
    let b = src[a..].find("\n}\n").map(|i| a + i).unwrap_or(src.len());
    let mut out = Vec::new();
    for line in src[a..b].lines() {
        let t = line.trim();
        if !t.starts_with("pub ") || t.starts_with("pub fn") {
            continue;
        }
        if let Some(name) = t.trim_start_matches("pub ").split(':').next() {
            let n = name.trim();
            if !n.is_empty() && n.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') && !out.contains(&n.to_string()) {
                out.push(n.to_string());
            }
        }
    }
    out
}

#[test]
fn every_document_field_is_either_in_the_file_or_named_here() {
    let model = include_str!("../src/model.rs");
    let schema = include_str!("../src/doc_file.rs");
    let in_model = fields_of(model, "pub struct Project {");
    let in_file = fields_of(schema, "pub(crate) struct DocumentFile {");
    assert!(in_model.len() > 20, "suspiciously few fields were collected from the model: {}", in_model.len());
    assert!(in_file.len() > 20, "suspiciously few fields were collected from the schema: {}", in_file.len());

    // Not stored, each for its own reason. The list is deliberately short: it is the one place that shows what
    // the program keeps in memory rather than in the file.
    let derived: &[(&str, &str)] = &[
        ("regen_faces", "derived: the faces come from the B-rep during a rebuild"),
        ("regen_edges", "derived: the edges come from the B-rep during a rebuild"),
        ("regen_errors", "derived: the errors of the last rebuild"),
        ("mates_conflict", "derived: diagnostics of the assembly solver"),
        ("mates_violated", "derived: which joints conflict; filled in by the solve so the panel can tell the truth about each"),
        ("source_data", "the bytes of an import live as separate files in the bundle rather than in document.ron"),
        ("dof", "derived: the degrees of freedom are computed by the solver"),
        ("solver_note", "derived: a message from the solver"),
        ("snap_rebinds", "derived: a counter of geometric fallback hits, living in memory"),
        ("drag_pull", "the pull of a drag: it lives only while a part is being dragged and has no business in a file"),
    ];

    let mut lost: Vec<String> = Vec::new();
    for f in &in_model {
        if in_file.contains(f) || derived.iter().any(|(n, _)| n == f) {
            continue;
        }
        lost.push(f.clone());
    }
    assert!(
        lost.is_empty(),
        "fields of the document are missing from the file schema ({}): {}\n\
         They will be lost silently on save. Either add them to `DocumentFile` (and to `from_model` and \
         `into_model`), or list them among the derived fields here, with a reason.",
        lost.len(),
        lost.join(", ")
    );
}
