//! EVERY ENTRY OF THE BRIDGE THAT TOUCHES OCCT CATCHES EVERYTHING. An exception thrown by OCCT and not caught inside a
//! bridge function leaves through `extern "C"`, and Rust aborts the whole program ("Rust cannot catch foreign
//! exceptions"): measured on `qym_shape_edges` with a stored body whose edge lies on an offset curve over a degenerate
//! B-spline - opening the project closed the program the moment its edges were drawn.
//!
//! The check reads the sources. An entry whose body touches OCCT - its types, its maps, the shape a `QymShape` holds -
//! must hold that work inside `try` with a catch of everything (`catch (...)` or `QYM_WHY_CATCH`), and do none of it
//! before the `try`. An entry that touches no OCCT - a reader of a filled vector, a `delete` - cannot meet an OCCT
//! exception and is let be; it is told by what it does, not by a list, so the day it starts touching OCCT it goes red.

/// EVERY C++ FILE OF THE TREE, as text, walked from the crates directory (a guard rooted in one crate would miss a
/// bridge file written in another).
fn sources() -> Vec<Source> {
    let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("the crates directory");
    let mut stack: Vec<std::path::PathBuf> = vec![crates.to_path_buf()];
    let mut out: Vec<Source> = Vec::new();
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).expect("the tree reads").flatten() {
            let p = e.path();
            if p.is_dir() {
                if p.file_name().is_some_and(|n| n != "target") {
                    stack.push(p);
                }
                continue;
            }
            if p.extension().is_some_and(|x| x == "cpp") {
                let name = p.strip_prefix(crates).unwrap_or(&p).to_string_lossy().replace('\\', "/");
                out.push(Source { name, text: std::fs::read_to_string(&p).unwrap_or_default() });
            }
        }
    }
    assert!(out.iter().any(|s| s.name.ends_with("occt_bridge.cpp")), "the bridge itself was not found by the walk");
    out
}

/// A C++ file of the tree.
struct Source {
    name: String,
    text: String,
}

/// An `extern "C"` function defined in a file: its name, the line it starts on, its body with the comments taken out.
struct Entry {
    name: String,
    line: usize,
    body: String,
}

/// The text with `//` comments taken out: the comments talk about OCCT a great deal and do nothing.
fn without_comments(text: &str) -> String {
    text.lines().map(|l| l.find("//").map_or(l, |at| &l[..at])).collect::<Vec<_>>().join("\n")
}

/// The index just past the bracket that closes the one opened right before `from`.
fn closing(text: &[u8], from: usize, open: u8, close: u8) -> Option<usize> {
    let mut depth = 1usize;
    for (i, c) in text.iter().enumerate().skip(from) {
        if *c == open {
            depth += 1;
        } else if *c == close {
            depth -= 1;
            if depth == 0 {
                return Some(i + 1);
            }
        }
    }
    None
}

/// Every `extern "C"` DEFINITION of the file; a declaration (ended by `;`) is not one.
fn entries(source: &Source) -> Vec<Entry> {
    let text = without_comments(&source.text);
    let bytes = text.as_bytes();
    let mut out: Vec<Entry> = Vec::new();
    let mut at = 0usize;
    while let Some(found) = text[at..].find("extern \"C\"") {
        let start = at + found;
        at = start + 1;
        let Some(open) = text[start..].find('(').map(|o| start + o) else { break };
        let head = &text[start..open];
        let Some(name) = head.split(|c: char| !c.is_alphanumeric() && c != '_').rfind(|w| w.starts_with("qym_")) else { continue };
        let Some(params_end) = closing(bytes, open + 1, b'(', b')') else { continue };
        let rest = text[params_end..].trim_start();
        if !rest.starts_with('{') {
            continue;
        }
        let body_start = text.len() - rest.len();
        let Some(body_end) = closing(bytes, body_start + 1, b'{', b'}') else { continue };
        out.push(Entry { name: name.to_string(), line: text[..start].matches('\n').count() + 1, body: text[body_start..body_end].to_string() });
    }
    out
}

/// THE WORDS OF OCCT: its types by their packages, and what a `QymShape` holds of it - the shape and its maps of names.
const OCCT: [&str; 22] = [
    "TopExp", "TopoDS", "TopTools", "TopAbs", "BRep", "Geom", "gp_", "BOPAlgo", "ShapeFix", "ShapeUpgrade", "ShapeAnalysis", "Handle(", "STEP", "IGES", "XCAF", "TDocStd", "TDF_", "Interface_",
    "Standard_", "->shape", "->fids", "->eids",
];
/// The maps of a `QymShape` that are OCCT's as well, by their field.
const OCCT_FIELDS: [&str; 2] = ["->fsplit_of", "->fsplit_idx"];

/// Does the code say a word of OCCT? A field is a whole word: `->shapes`, a vector of a list, is not `->shape`.
fn touches_occt(code: &str) -> bool {
    OCCT.iter().chain(OCCT_FIELDS.iter()).any(|w| {
        if !w.starts_with("->") {
            return code.contains(w);
        }
        code.match_indices(w).any(|(at, _)| code[at + w.len()..].chars().next().is_none_or(|c| !c.is_alphanumeric() && c != '_'))
    })
}

#[test]
fn every_entry_that_touches_occt_catches_everything() {
    let mut caught: Vec<String> = Vec::new();
    let mut seen = 0usize;
    for source in sources() {
        for e in entries(&source) {
            seen += 1;
            if !touches_occt(&e.body) {
                continue;
            }
            let Some(at) = e.body.find("try") else {
                caught.push(format!("{}:{} {} - touches OCCT with no try", source.name, e.line, e.name));
                continue;
            };
            if !(e.body.contains("catch (...)") || e.body.contains("QYM_WHY_CATCH")) {
                caught.push(format!("{}:{} {} - its catch does not catch everything", source.name, e.line, e.name));
            }
            if touches_occt(&e.body[..at]) {
                caught.push(format!("{}:{} {} - touches OCCT before its try", source.name, e.line, e.name));
            }
        }
    }
    // the walk finds what the bridge has: 135 entries on 12.10.2026, and a parse that found a handful would pass all
    assert!(seen >= 130, "the parse found {seen} extern \"C\" definitions, fewer than the bridge has - it is reading them wrong");
    assert!(caught.is_empty(), "an exception of OCCT can leave the bridge and abort the program ({}):\n{}", caught.len(), caught.join("\n"));
}

/// THE CHECK SEES WHAT IT IS FOR: an entry walking a shape with no try is caught, one walking it inside a try with a
/// catch of everything is not, and a reader of a filled vector is let be.
#[test]
fn the_check_tells_a_guarded_entry_from_a_bare_one() {
    let source = Source {
        name: "sample.cpp".into(),
        text: [
            "extern \"C\" int qym_bare(const QymShape* s) { int n = 0; for (TopExp_Explorer ex(s->shape, TopAbs_FACE); ex.More(); ex.Next()) ++n; return n; }",
            "extern \"C\" int qym_guarded(const QymShape* s) { if (!s) return 0; try { return s->shape.IsNull(); } catch (...) { return 0; } }",
            "extern \"C\" int qym_early(const QymShape* s) { TopExp_Explorer ex(s->shape, TopAbs_FACE); try { return 1; } catch (...) { return 0; } }",
            "extern \"C\" size_t qym_reader(const QymShapeList* l) { return l ? l->shapes.size() : 0; }",
            "extern \"C\" int qym_declared(const QymShape* s);",
        ]
        .join("\n"),
    };
    let found = entries(&source);
    let names: Vec<&str> = found.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["qym_bare", "qym_guarded", "qym_early", "qym_reader"], "the definitions are not told from the declaration");
    let bare: Vec<&str> = found.iter().filter(|e| touches_occt(&e.body) && e.body.find("try").is_none_or(|at| touches_occt(&e.body[..at]))).map(|e| e.name.as_str()).collect();
    assert_eq!(bare, ["qym_bare", "qym_early"], "the entries open to an exception are not the ones caught");
}
