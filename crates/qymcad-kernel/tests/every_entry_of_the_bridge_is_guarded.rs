//! EVERY ENTRY OF THE BRIDGE IS GUARDED: it catches what OCCT throws, and it says what it does to the body it is handed.
//!
//! WHAT OCCT THROWS. An exception thrown by OCCT and not caught inside a bridge function leaves through `extern "C"`,
//! and Rust aborts the whole program ("Rust cannot catch foreign exceptions"): measured on `qym_shape_edges` with a
//! stored body whose edge lies on an offset curve over a degenerate B-spline - opening the project closed the program
//! the moment its edges were drawn. An entry whose body touches OCCT - its types, its maps, the shape a `QymShape`
//! holds - must hold that work inside `try` with a catch of everything (`catch (...)` or `QYM_WHY_CATCH`), and do none
//! of it before the `try`. An entry that touches no OCCT - a reader of a filled vector, a `delete` - cannot meet an OCCT
//! exception and is let be; it is told by what it does, not by a list, so the day it starts touching OCCT it goes red.
//!
//! THE BODY IT IS HANDED. A body of the cache shares its edges and vertices with the bodies before it and beside it,
//! and the rebuild works the nodes of one wave on threads of their own: an algorithm of OCCT that writes into what it
//! is given writes into bodies another thread reads. Every body an entry takes is named in one declaration of
//! `occt_common.hpp` - `QYM_OWN_COPY` (it works on a copy of its own) or `QYM_AS_HANDED` (it works on the body as handed
//! and says why that is safe) - so a new entry cannot hand a body of the cache to OCCT without saying which.
//!
//! The check reads the sources.

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

/// An `extern "C"` function defined in a file: its name, the line it starts on, its parameters and its body with the
/// comments taken out.
struct Entry {
    name: String,
    line: usize,
    params: String,
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
        out.push(Entry { name: name.to_string(), line: text[..start].matches('\n').count() + 1, params: text[open + 1..params_end - 1].to_string(), body: text[body_start..body_end].to_string() });
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

/// THE BODIES AN ENTRY TAKES: the names of its parameters that are a `QymShape` or a list of them, not a list of shapes
/// the bridge made (`QymShapeList`).
fn bodies_taken(params: &str) -> Vec<String> {
    params
        .split(',')
        .filter(|p| p.contains("QymShape") && !p.contains("QymShapeList"))
        .filter_map(|p| p.split(|c: char| !c.is_alphanumeric() && c != '_').rfind(|w| !w.is_empty()).map(str::to_string))
        .collect()
}

/// HOW MANY TIMES THE WORK OF AN ENTRY DECLARES A BODY: `QYM_OWN_COPY(s)`, `QYM_OWN_COPIES(parts, n)`,
/// `QYM_AS_HANDED(s, ...)` - the name whole, not the start of a longer one.
fn declared(entry: &Entry, name: &str) -> usize {
    let body = &entry.body;
    ["QYM_OWN_COPY(", "QYM_OWN_COPIES(", "QYM_AS_HANDED("]
        .iter()
        .flat_map(|d| {
            let said = format!("{d}{name}");
            body.match_indices(&said).map(|(at, _)| at + said.len()).collect::<Vec<_>>()
        })
        .filter(|end| body[*end..].starts_with([',', ')']))
        .count()
}

#[test]
fn every_entry_says_what_it_does_to_the_body_it_is_handed() {
    let mut caught: Vec<String> = Vec::new();
    let mut taking = 0usize;
    for source in sources() {
        for e in entries(&source) {
            for body in bodies_taken(&e.params) {
                taking += 1;
                match declared(&e, &body) {
                    1 => {}
                    0 => caught.push(format!("{}:{} {} - says nothing of what it does to `{body}`", source.name, e.line, e.name)),
                    n => caught.push(format!("{}:{} {} - says {n} times what it does to `{body}`", source.name, e.line, e.name)),
                }
            }
        }
    }
    // the entries of the bridge take 76 bodies; a parse that found a handful would pass all
    assert!(taking >= 70, "the parse found {taking} bodies taken by the entries, fewer than the bridge has - it is reading them wrong");
    assert!(caught.is_empty(), "an entry hands a body to OCCT without saying what it does to it ({}):\n{}", caught.len(), caught.join("\n"));
}

/// THE CHECK SEES WHAT IT IS FOR: a body declared once passes, a body not declared and a body declared twice are caught,
/// a list of bodies the bridge made is no body taken, and a name that begins another name is not its declaration.
#[test]
fn the_check_tells_a_declared_body_from_a_silent_one() {
    let source = Source {
        name: "sample.cpp".into(),
        text: [
            "extern \"C\" double qym_said(const QymShape* s) { try { QYM_AS_HANDED(s, \"it only reads the body\"); return 1.0; } catch (...) { return 0.0; } }",
            "extern \"C\" QymShape* qym_silent(const QymShape* a, const QymShape* b) { try { QYM_OWN_COPY(a); return nullptr; } catch (...) { return nullptr; } }",
            "extern \"C\" QymShape* qym_twice(const QymShape* s) { try { QYM_OWN_COPY(s); QYM_AS_HANDED(s, \"and as handed too\"); return nullptr; } catch (...) { return nullptr; } }",
            "extern \"C\" QymShape* qym_list(const QymShape* const* parts, size_t n, const QymShapeList* made) { try { QYM_OWN_COPIES(parts, n); return nullptr; } catch (...) { return nullptr; } }",
            "extern \"C\" QymShape* qym_prefix(const QymShape* s, const QymShape* s_tool) { try { QYM_OWN_COPY(s_tool); return nullptr; } catch (...) { return nullptr; } }",
        ]
        .join("\n"),
    };
    let mut told: Vec<String> = Vec::new();
    for e in entries(&source) {
        for body in bodies_taken(&e.params) {
            told.push(format!("{}:{body}:{}", e.name, declared(&e, &body)));
        }
    }
    let silent_or_twice: Vec<&String> = told.iter().filter(|t| !t.ends_with(":1")).collect();
    assert_eq!(silent_or_twice, ["qym_silent:b:0", "qym_twice:s:2", "qym_prefix:s:0"], "the bodies left unsaid are not the ones caught");
    assert!(told.iter().all(|t| !t.contains(":made:")), "a list of shapes the bridge made was taken for a body");
}

/// THE BLENDS OF OCCT AN ENTRY MAKES: the names of its `BRepFilletAPI_MakeFillet` and `BRepFilletAPI_MakeChamfer`.
fn blends_made(body: &str) -> Vec<String> {
    ["BRepFilletAPI_MakeFillet ", "BRepFilletAPI_MakeChamfer "]
        .iter()
        .flat_map(|kind| body.match_indices(kind).map(|(at, _)| body[at + kind.len()..].split(|c: char| !c.is_alphanumeric() && c != '_').next().unwrap_or_default().to_string()).collect::<Vec<_>>())
        .filter(|name| !name.is_empty())
        .collect()
}

/// EVERY FILLET AND CHAMFER OF OCCT IS BUILT ONE AT A TIME. Two built at once on two threads clash inside OCCT even on
/// bodies of their own (`QymShared::Blend` in `occt_common.hpp`), so the building goes through
/// `qym_build_alone(QymShared::Blend, ...)` and never through its own `Build()`.
#[test]
fn every_blend_of_occt_is_built_one_at_a_time() {
    let mut caught: Vec<String> = Vec::new();
    let mut seen = 0usize;
    for source in sources() {
        for e in entries(&source) {
            for name in blends_made(&e.body) {
                seen += 1;
                if e.body.contains(&format!("{name}.Build()")) || !e.body.contains(&format!("qym_build_alone(QymShared::Blend, {name})")) {
                    caught.push(format!("{}:{} {} - builds its `{name}` past the lock of blends", source.name, e.line, e.name));
                }
            }
        }
    }
    // seven entries of the bridge make a blend of OCCT; a parse that found none would pass all
    assert!(seen >= 7, "the parse found {seen} blends of OCCT made by the entries, fewer than the bridge has");
    assert!(caught.is_empty(), "a blend of OCCT is built while another may be ({}):\n{}", caught.len(), caught.join("\n"));
}
