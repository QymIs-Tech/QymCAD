//! THE PART HOLDS NO TOOL OF THE SKETCH AND NO BAR OF THE PATH. The rows of the sketch commands, their press, the bar
//! of a sketch tool and the words of the sketch live in the sketch workbench; the bar of the path lives in the shell.
//! What the part takes from a sketch - its contours as a profile - is data, read through the document.
//!
//! A workbench that holds the tools of another cannot be read, changed or checked on its own.

use std::path::Path;

/// THE MARKS OF A SKETCH TOOL in a source: the code of a row of a sketch command, a launch of a sketch tool, a tool
/// of the sketch by its name, a word of the sketch catalogue.
const MARKS: &[&str] = &[
    "\"sketch.",
    "Launch::SketchSelect",
    "Launch::SkTool",
    "Launch::ClickOp",
    "Launch::SkMove",
    "Launch::SkPattern",
    "Launch::Construction",
    "Launch::Measure2d",
    "Launch::FilletAll",
    "Launch::Dim(",
    "Launch::Constraint(",
    "Launch::Modify(",
    "SketchShape",
    "ClickTool",
    "EditTool",
    "\"sk-",
];

/// A SOURCE FILE: where it is and what it says.
struct Source {
    path: String,
    text: String,
}

/// Every `.rs` file under `dir`, with its text.
fn sources(dir: &Path, out: &mut Vec<Source>) {
    for entry in std::fs::read_dir(dir).expect("a folder of the sources") {
        let path = entry.expect("an entry of the folder").path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(Source { path: path.display().to_string(), text: std::fs::read_to_string(&path).expect("a source") });
        }
    }
}

/// THE MARKS OF THE BAR OF THE PATH in a source: the steps of the path, its button that leaves, its words. The bar is
/// one for every workbench and stands in the shell; a workbench only offers its switches to it.
const PATH_MARKS: &[&str] = &["ensure_active_path", "BarAsk::GotoPath", "BarAsk::ExitContext", "\"wb-"];

/// The places of the sources of the part where one of `marks` stands, as `file:line: mark`.
fn found_in_the_part(marks: &[&str]) -> Vec<String> {
    let mut files = Vec::new();
    sources(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
    assert!(files.len() > 10, "the sources of the part were not found: {} files", files.len());
    let mut found = Vec::new();
    for file in &files {
        for (n, line) in file.text.lines().enumerate() {
            for mark in marks.iter().filter(|m| line.contains(*m)) {
                found.push(format!("{}:{}: {mark}", file.path, n + 1));
            }
        }
    }
    found
}

#[test]
fn the_part_holds_no_tool_of_the_sketch() {
    let found = found_in_the_part(MARKS);
    assert!(found.is_empty(), "a tool of the sketch in the part crate - it belongs to the sketch workbench:\n{}", found.join("\n"));
}

#[test]
fn the_part_holds_no_bar_of_the_path() {
    let found = found_in_the_part(PATH_MARKS);
    assert!(found.is_empty(), "the bar of the path in the part crate - it belongs to the shell:\n{}", found.join("\n"));
}
