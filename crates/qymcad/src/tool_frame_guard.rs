//! Guard on THE FRAME OF A TOOL: outside the frame (`qymcad_ui_state::tool_frame`), the crates of the workbenches lay no
//! area or window of their own for the fields of a tool and read no Enter or Esc of their own.
//!
//! Measured on 2026-10-17 before the frame, in the working halves of the files: the part laid 4 areas of its own and
//! read Enter or Esc in 8 places, the assembly laid 1 and read the keys in 10, the sketch laid 9 and read them in 19 -
//! each its own Enter, its own Esc, its own place for its words. A tool of the frame says what it needs and the frame draws it and answers the keys, so a
//! tool that lays a box of its own is where the tools drift apart again.
//!
//! The drawing tools of the sketch keep boxes of their own until the sketch comes onto the frame; each is named below
//! with the reason, and the list may only shrink.

#[cfg(test)]
mod tests {
    /// THE CRATES OF THE WORKBENCHES whose tools stand in the frame.
    const WORKBENCHES: [&str; 3] = ["qymcad-part/src", "qymcad-assembly/src", "qymcad-sketch/src"];

    /// What a tool of the frame does not do itself: lay an area or a window, read Enter or Esc.
    const FORBIDDEN: [&str; 4] = ["Area::new(", "Window::new(", "Key::Enter", "Key::Escape"];

    /// A function allowed to lay a box of its own or read a key of its own, and why.
    struct Allowed {
        path: &'static str,
        function: &'static str,
        why: &'static str,
    }

    /// Why each drawing tool of the sketch is allowed its own box.
    const NOT_YET: &str = "a box of a drawing tool of the sketch, with its own Enter and Esc, kept until the sketch comes onto the frame of a tool";

    const ALLOWED: [Allowed; 10] = [
        Allowed { path: "qymcad-sketch/src/popups.rs", function: "corner_input_popup", why: NOT_YET },
        Allowed { path: "qymcad-sketch/src/popups.rs", function: "dim_editor", why: NOT_YET },
        Allowed { path: "qymcad-sketch/src/popups.rs", function: "ellipse_input_popup", why: NOT_YET },
        Allowed { path: "qymcad-sketch/src/popups.rs", function: "poly_input_popup", why: NOT_YET },
        Allowed { path: "qymcad-sketch/src/popups.rs", function: "rect_dims_popup", why: NOT_YET },
        Allowed { path: "qymcad-sketch/src/popups.rs", function: "rect_input_popup", why: NOT_YET },
        Allowed { path: "qymcad-sketch/src/popups.rs", function: "sketch_rotate_popup", why: NOT_YET },
        Allowed { path: "qymcad-sketch/src/offset.rs", function: "offset_box", why: NOT_YET },
        Allowed { path: "qymcad-sketch/src/offset.rs", function: "offset_on_enter", why: NOT_YET },
        Allowed {
            path: "qymcad-sketch/src/menu.rs",
            function: "sketch_tool_keys",
            why: "Enter that ends the modification of a sketch in hand - a drawing tool of the sketch, kept until the sketch comes onto the frame of a tool",
        },
    ];

    /// One place a tool does what the frame does: the file, the function it stands in and what it does.
    struct Found {
        path: String,
        function: String,
        what: &'static str,
        line: usize,
    }

    /// EVERY PLACE IN THE CRATES OF THE WORKBENCHES where a box is laid or a key read outside the frame: the working
    /// half of each file, a module of checks below it left out.
    fn found() -> Vec<Found> {
        let root = qymcad_i18n::ratchet::crates_root();
        let mut stack: Vec<std::path::PathBuf> = WORKBENCHES.iter().map(|w| root.join(w)).collect();
        let mut out = Vec::new();
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).expect("the sources read").flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                if p.extension().is_none_or(|x| x != "rs") || qymcad_i18n::ratchet::is_test_module(&p) {
                    continue;
                }
                let path = p.strip_prefix(&root).unwrap_or(&p).to_string_lossy().replace('\\', "/");
                let text = std::fs::read_to_string(&p).unwrap_or_default();
                let work = ["#[cfg(test)]\nmod ", "#[cfg(test)]\npub(crate) mod ", "#[cfg(test)]\npub(in crate::gui) mod ", "#[cfg(test)]\npub(super) mod "]
                    .iter()
                    .fold(text.as_str(), |t, cut| t.split(cut).next().unwrap_or(""));
                let mut function = String::new();
                for (i, line) in work.lines().enumerate() {
                    let code = line.split("//").next().unwrap_or("");
                    if let Some(at) = code.find("fn ") {
                        let name: String = code[at + 3..].chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
                        if !name.is_empty() {
                            function = name;
                        }
                    }
                    for what in FORBIDDEN.iter().filter(|f| code.contains(**f)) {
                        out.push(Found { path: path.clone(), function: function.clone(), what, line: i + 1 });
                    }
                }
            }
        }
        out
    }

    #[test]
    fn every_exception_says_why() {
        for a in ALLOWED {
            assert!(a.why.split_whitespace().count() >= 8, "the exception of {} in {} gives no reason", a.function, a.path);
        }
    }

    /// NO TOOL OF THE PART OR THE ASSEMBLY LAYS A BOX OR READS A KEY OF ITS OWN: it describes itself, and the frame draws
    /// it and answers Enter and Esc.
    #[test]
    fn no_tool_lays_a_box_or_reads_a_key_outside_the_frame() {
        let outside: Vec<String> =
            found().into_iter().filter(|f| !ALLOWED.iter().any(|a| a.path == f.path && a.function == f.function)).map(|f| format!("{}:{} in {}: {}", f.path, f.line, f.function, f.what)).collect();
        assert!(
            outside.is_empty(),
            "a tool lays a box or reads Enter or Esc of its own instead of describing itself to the frame of a tool (`tool_bar`, `tool_box`); {} of them:\n{}",
            outside.len(),
            outside.join("\n")
        );
    }

    /// AN EXCEPTION NOT NEEDED ANY MORE GOES: a drawing tool of the sketch moved onto the frame leaves the list with it.
    #[test]
    fn every_exception_still_stands() {
        let all = found();
        let stale: Vec<String> = ALLOWED.iter().filter(|a| !all.iter().any(|f| f.path == a.path && f.function == a.function)).map(|a| format!("{} in {}", a.function, a.path)).collect();
        assert!(stale.is_empty(), "the exception lays no box and reads no key any more - take it off the list: {stale:?}");
    }
}
