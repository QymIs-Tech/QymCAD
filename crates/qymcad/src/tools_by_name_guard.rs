//! Guard on THE TOOL IN HAND BY NAME: a tool is held, launched, asked for and compared by its name - `PartTool`,
//! `SketchShape`, `ClickTool`, `MoveKind`, `PatternKind`, `DimKind`, `ConstraintKind` - never by a number, in the code and its checks alike. Zero, with no marks and no list of
//! old places.
//!
//! A number says nothing at the place it is compared, and a 10 written where 11 was meant still compiles. Behaviour
//! shared by several tools is a question asked of the tool by its name (`picks_profiles`, `takes_one_face`), not a list
//! of numbers spread over the window, the picking and the drawing.
//!
//! WHAT IS COUNTED: a tool in hand, a launch of a button, an ask of a bar or a wait for a sketch built from a number
//! (`Armed::Command(4)`, `Launch::Feat(1)`), a reader of a tool as a number (`cmd_kind()`), a tool of a part started
//! or pressed by a number (`start_feat_cmd(4)`, `.tool(4)`), the pattern of components by a number, and what is picked
//! in a sketch as a pair of a number and an id.

#[cfg(test)]
mod tests {
    /// THE FORMS that build or read a tool as a number: each is followed by a digit where it is counted, or counted
    /// wherever it stands.
    struct Form {
        text: &'static str,
        before_a_digit: Digit,
    }

    /// Is a form counted only before a digit?
    #[derive(Clone, Copy, PartialEq)]
    enum Digit {
        Only,
        Anywhere,
    }

    const FORMS: [Form; 33] = [
        Form { text: "Armed::Command(", before_a_digit: Digit::Only },
        Form { text: "Armed::Draw(", before_a_digit: Digit::Only },
        Form { text: "Armed::ClickOp(", before_a_digit: Digit::Only },
        Form { text: "Launch::Feat(", before_a_digit: Digit::Only },
        Form { text: "Launch::Prim(", before_a_digit: Digit::Only },
        Form { text: "Launch::SkTool(", before_a_digit: Digit::Only },
        Form { text: "Launch::ClickOp(", before_a_digit: Digit::Only },
        Form { text: "BarAsk::FeatCmd(", before_a_digit: Digit::Only },
        Form { text: "BarAsk::PrimCmd(", before_a_digit: Digit::Only },
        Form { text: "SketchFor(", before_a_digit: Digit::Only },
        Form { text: "start_feat_cmd(", before_a_digit: Digit::Only },
        Form { text: ".tool(", before_a_digit: Digit::Only },
        Form { text: "Armed::Move(", before_a_digit: Digit::Only },
        Form { text: "Armed::Pattern(", before_a_digit: Digit::Only },
        Form { text: "Armed::Dimension(", before_a_digit: Digit::Only },
        Form { text: "Launch::SkMove(", before_a_digit: Digit::Only },
        Form { text: "Launch::SkPattern(", before_a_digit: Digit::Only },
        Form { text: "Launch::Dim(", before_a_digit: Digit::Only },
        Form { text: "Launch::Constraint(", before_a_digit: Digit::Only },
        Form { text: "BarAsk::Constraint(", before_a_digit: Digit::Only },
        Form { text: ".constraint(", before_a_digit: Digit::Only },
        Form { text: "sk_move_tool(", before_a_digit: Digit::Only },
        Form { text: "cmd_kind()", before_a_digit: Digit::Anywhere },
        Form { text: "draw_kind()", before_a_digit: Digit::Anywhere },
        Form { text: "Launch::CompArray(", before_a_digit: Digit::Only },
        Form { text: "BarAsk::CompArray(", before_a_digit: Digit::Only },
        Form { text: "carr.mode == ", before_a_digit: Digit::Only },
        Form { text: "(u8, Id)", before_a_digit: Digit::Anywhere },
        Form { text: "(u8, u64)", before_a_digit: Digit::Anywhere },
        Form { text: "SEL_TEXT", before_a_digit: Digit::Anywhere },
        Form { text: "move_op()", before_a_digit: Digit::Anywhere },
        Form { text: "pat_op()", before_a_digit: Digit::Anywhere },
        Form { text: "dim_kind()", before_a_digit: Digit::Anywhere },
    ];

    /// The places in `text` where a tool is built or read as a number, as `line: form`.
    fn numbers_for_tools(text: &str) -> Vec<String> {
        let mut out = Vec::new();
        for (n, line) in text.lines().enumerate() {
            for form in &FORMS {
                let counted = line.match_indices(form.text).any(|(i, _)| form.before_a_digit == Digit::Anywhere || line[i + form.text.len()..].starts_with(|c: char| c.is_ascii_digit()));
                if counted {
                    out.push(format!("{}: {}", n + 1, form.text));
                }
            }
        }
        out
    }

    /// Every `.rs` file under `dir`.
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(dir).expect("a folder of the sources").flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }

    #[test]
    fn no_tool_is_held_or_compared_by_a_number() {
        let root = qymcad_i18n::ratchet::crates_root();
        let mut files = Vec::new();
        for src in qymcad_i18n::ratchet::every_crate_src() {
            walk(&src, &mut files);
            let tests = src.with_file_name("tests");
            if tests.is_dir() {
                walk(&tests, &mut files);
            }
        }
        assert!(files.len() > 500, "the sources were not found: {} files", files.len());
        let mut found = Vec::new();
        for p in &files {
            let name = p.strip_prefix(&root).unwrap_or(p).to_string_lossy().replace('\\', "/");
            if name == "qymcad/src/tools_by_name_guard.rs" {
                continue; // this file names the forms in order to forbid them
            }
            let text = std::fs::read_to_string(p).expect("a source");
            found.extend(numbers_for_tools(&text).into_iter().map(|at| format!("{name}:{at}")));
        }
        assert!(found.is_empty(), "a tool held or compared by a number - give it its name:\n{}", found.join("\n"));
    }

    /// The places in the code of `text` that name a tool of a part - `PartTool::` and a variant - as `line: column`. A
    /// question asked of the tool (`PartTool::picks_faces`) and the list of them (`PartTool::ALL`) are not names.
    fn part_tools_named(text: &str) -> Vec<String> {
        let mut out = Vec::new();
        for (n, line) in text.lines().enumerate() {
            // the code of the line, not its comment
            let code = line.split("//").next().unwrap_or("");
            for (i, _) in code.match_indices("PartTool::") {
                // a variant: a capital, then small letters - `ALL` is the list of them, not a tool
                let word: String = code[i + "PartTool::".len()..].chars().take_while(|c| c.is_alphanumeric()).collect();
                if word.starts_with(|c: char| c.is_ascii_uppercase()) && word.chars().any(|c| c.is_ascii_lowercase()) {
                    out.push(format!("{}:{}", n + 1, i + 1));
                }
            }
        }
        out
    }

    /// THE WINDOW NEVER ASKS WHICH TOOL OF A PART IS IN HAND: what a tool takes under the pointer and how it shows it in
    /// the view are asked of the tool (`aim`, `shows`, `picks_faces`) or done by the door of its workbench. The window
    /// and the drawing of the pickers name no tool of a part, outside their checks.
    #[test]
    fn the_window_names_no_tool_of_a_part() {
        let root = qymcad_i18n::ratchet::crates_root();
        let mut files = vec![root.join("qymcad/src/gui.rs"), root.join("qymcad-render/src/pickers.rs")];
        walk(&root.join("qymcad/src/gui"), &mut files);
        let mut found = Vec::new();
        for p in &files {
            let text = std::fs::read_to_string(p).expect("a source");
            let name = p.strip_prefix(&root).unwrap_or(p).to_string_lossy().replace('\\', "/");
            if qymcad_i18n::ratchet::is_test_module(p) {
                continue;
            }
            found.extend(part_tools_named(&qymcad_i18n::ratchet::working_part(&text)).into_iter().map(|at| format!("{name}:{at}")));
        }
        assert!(found.is_empty(), "the window names a tool of a part - ask the tool, or hand it to the door of its workbench:\n{}", found.join("\n"));
    }

    /// THE GUARD SEES A NAME: a tool named, and not a question asked of it.
    #[test]
    fn the_guard_sees_a_tool_named() {
        assert_eq!(part_tools_named("if t == PartTool::Shell {}\nif t.is_some_and(PartTool::picks_faces) {}\nfor t in PartTool::ALL {} // PartTool::Shell").len(), 1);
    }

    /// THE GUARD SEES A NUMBER: a tool built from one, and a reader of one.
    #[test]
    fn the_guard_sees_a_number() {
        assert_eq!(numbers_for_tools("let a = Armed::Command(4);\nif x.cmd_kind() == 4 {}").len(), 2);
        assert!(numbers_for_tools("let a = Armed::Command(PartTool::Fillet);").is_empty());
    }
}
