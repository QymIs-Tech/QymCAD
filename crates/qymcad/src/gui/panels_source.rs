//! THE PANELS AS ONE TEXT, for the checks that read them.
//!
//! Fifty-seven checks assert that a button, a tree row or a string exists by looking for it in the panels'
//! source. That is a strong guard - a tool nobody can reach does not exist for a person, and a text search is
//! the only thing that notices when the button is gone - and splitting the file must not weaken it.
//!
//! THE WORKBENCHES ARE CRATES NOW, and their bars went with them. The list below follows the code: a guard
//! that reads a file BY NAME guards that file and no other, and leaving the list behind would have left
//! twenty-five checks reading the remnants - green, and blind. It happened once already, with the drawing.

/// The whole of the panels, in the order the file used to have them.
#[cfg(test)]
pub(crate) const PANELS: &str = concat!(
    include_str!("../../../qymcad-part/src/lib.rs"),
    "\n",
    include_str!("../../../qymcad-sketch/src/lib.rs"),
    "\n",
    include_str!("../../../qymcad-assembly/src/lib.rs"),
    "\n",
    include_str!("panels_tree.rs"),
    "\n",
    include_str!("panels_props.rs"),
    "\n",
    include_str!("panels_bars.rs"),
    "\n",
    include_str!("panels_windows.rs"),
);

/// THE WINDOWS AND DIALOGUES OF THE APPLICATION, as one text.
///
/// Separate from `PANELS` on purpose: several checks over `PANELS` assert that something is ABSENT from
/// it, and widening that constant would change what those checks mean. A window is not a panel.
#[cfg(test)]
pub(crate) const WINDOWS: &str = concat!(
    include_str!("help_window.rs"),
    "\n",
    include_str!("hotkeys.rs"),
    "\n",
    include_str!("report_problem.rs"),
    "\n",
    include_str!("file_ask.rs"),
    "\n",
    include_str!("io_jobs.rs"),
    "\n",
    include_str!("start_screen.rs"),
    "\n",
    include_str!("command_search.rs"),
);

/// THE WINDOWS AS ONE TEXT: everywhere a window is opened. The panels plus the three files that open a window
/// without being a panel - the start screen, the command search and the menu that raises them.
#[cfg(test)]
const WINDOW_OPENERS: &str = concat!(
    include_str!("../../../qymcad-part/src/lib.rs"),
    "\n",
    include_str!("panels_bars.rs"),
    "\n",
    include_str!("panels_windows.rs"),
    "\n",
    include_str!("start_screen.rs"),
    "\n",
    include_str!("command_search.rs"),
    "\n",
    include_str!("../gui.rs"),
);

/// EVERYWHERE A CONTEXT IS USED: the three workbench crates and the panels of the application. `gui.rs` is
/// deliberately absent - there `self.viewing.mode_3d = true` is correct, and it is the contexts that are being checked.
#[cfg(test)]
const CTX_USERS: &str = concat!(
    include_str!("../../../qymcad-part/src/lib.rs"),
    "\n",
    include_str!("../../../qymcad-sketch/src/lib.rs"),
    "\n",
    include_str!("../../../qymcad-assembly/src/lib.rs"),
    "\n",
    include_str!("panels_tree.rs"),
    "\n",
    include_str!("panels_props.rs"),
    "\n",
    include_str!("panels_bars.rs"),
    "\n",
    include_str!("panels_windows.rs"),
);

/// The declarations of the contexts, for the two checks below.
#[cfg(test)]
const CTX_DECL: &str = include_str!("../../../qymcad-ui-state/src/lib.rs");

#[cfg(test)]
mod tests {
    use qymcad_ui_state::WinKind;

    /// EVERY FILE THAT DRAWS IS READ BY SOMETHING, or is named here with a reason.
    ///
    /// Fifty-seven checks look for a button, a row or a string in the source of the panels. A guard that
    /// reads a file BY NAME guards that file and no other - that is [D17], and it cost four false greens in
    /// one day. The lists were then gathered into three places, which fixed the SPLITTING of a file but not
    /// the ADDING of one: a new window drawn in a new file is read by nothing, and nothing says so.
    ///
    /// So the coverage is checked rather than assumed. A file that draws and is in no list has to be named
    /// below with the reason, and the reason has to be a sentence rather than "not yet".
    #[test]
    fn every_file_that_draws_is_read_by_a_check() {
        // NOT PANELS, and each says why. A reason here is a decision; silence would be an oversight.
        //
        // KEYED BY PATH, not by file name. Every workbench crate is a single `lib.rs`, so an exception
        // written as "lib.rs" would have quietly excused the Part, the sketcher and the assembly along with
        // the one file it was meant for.
        const NOT_A_PANEL: [(&str, &str); 3] = [
            ("qymcad/src/gui/input.rs", "keyboard handling: it draws nothing, it only takes a frame's context to read keys from"),
            ("qymcad/src/gui/expr_field.rs", "a widget drawn INTO a panel, not a panel: it is reached through the panels that place it"),
            (
                "qymcad-shell/src/lib.rs",
                "the shell: it opens the CONTAINERS and hands each to whoever fills it, so it draws places rather than content. \
                 Being absent from every panel list is the rule it exists to hold, not an oversight",
            ),
        ];
        // EVERY LIST IN THE FILE, not the four panel ones. What is asserted below is "no source check reads
        // this file at all", so the four panel lists are the wrong measure of it: `gui.rs` is read by
        // `WINDOW_OPENERS` and `qymcad-ui-state` by `CTX_DECL`, and reporting them as unread would have been
        // the guard lying in the other direction. Membership rather than a count, so overlapping lists are
        // safe here - unlike the counting guards, where one list is passed for the reason in D17.
        let lists = [
            super::PANELS,
            super::WINDOWS,
            super::WINDOW_OPENERS,
            super::CTX_USERS,
            super::CTX_DECL,
            crate::gui::render_source::RENDER,
            crate::gui::sketch_source::SKETCH,
        ];
        // EVERY CRATE, not just the application's `gui`. The panels moved out into crates of their own, and
        // a walk rooted at one directory would from then on have reported full coverage of a shrinking part
        // of the interface - the same blindness recorded as D20 for the language guards. Found by widening:
        // `qymcad-shell` draws and no list read it.
        let crates = qymcad_i18n::ratchet::crates_root();
        let mut files: Vec<std::path::PathBuf> = Vec::new();
        let mut stack = qymcad_i18n::ratchet::every_crate_src();
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).expect("the sources read").flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    files.push(p);
                }
            }
        }
        let mut unread = Vec::new();
        let mut looked = 0;
        for p in files {
            let name = p.strip_prefix(&crates).unwrap_or(&p).to_string_lossy().replace('\\', "/");
            let text = std::fs::read_to_string(&p).unwrap_or_default();
            // WHAT THE PRODUCTION HALF OF THE FILE DOES; a test module that draws is not a panel.
            //
            // Cut at the test MODULE, not at the first `#[cfg(test)]`. That attribute also sits on single
            // functions in the middle of a file, and cutting there hid everything below it: `viewcube.rs`
            // drew from its first day and this check never saw it, because one test-only function stood
            // above the drawing. It became visible only when that function was moved out.
            let prod = text.split("#[cfg(test)]\nmod ").next().unwrap_or("");
            let draws = prod.contains("ui: &mut egui::Ui") || prod.contains("egui::Window::new") || prod.contains("ctx: &egui::Context");
            if !draws {
                continue;
            }
            looked += 1;
            if NOT_A_PANEL.iter().any(|(n, _)| *n == name) {
                continue;
            }
            // A file is read when a list carries a line only it has: its own module comment's first line.
            let first = prod.lines().find(|l| l.starts_with("//!")).unwrap_or("");
            if first.len() > 20 && !lists.iter().any(|l| l.contains(first)) {
                unread.push(name);
            }
        }
        assert!(looked > 5, "suspiciously few drawing files were found ({looked}) - the sweep looked at nothing");
        assert!(
            unread.is_empty(),
            "a file draws and no list reads it, so every source check is blind to it; put it in a list or name it in NOT_A_PANEL with a reason: {unread:?}"
        );
    }

    /// EVERY WINDOW HAS A WAY IN. A window that nothing opens does not exist for a person, and nothing else
    /// notices: it compiles, it draws when the flag is set, and the flag is never set.
    ///
    /// This became possible only when the flags became a set: ten booleans could not be walked over, so the
    /// question "is any of them unreachable" had to be asked ten times by hand, and never was.
    #[test]
    fn every_window_can_be_opened_from_the_interface() {
        let mut lost = Vec::new();
        for k in WinKind::ALL {
            let name = format!("{k:?}");
            let opened = ["open", "toggle", "set"]
                .iter()
                .any(|verb| super::WINDOW_OPENERS.contains(&format!(".{verb}(WinKind::{name}")));
            if !opened {
                lost.push(name);
            }
        }
        assert!(lost.is_empty(), "windows that nothing opens: {lost:?}");
    }

    /// THE LIST OF ALL KINDS MATCHES THE DECLARATION. `ALL` is written by hand, so a variant added later is
    /// left out of every walk unless something counts them.
    #[test]
    fn the_list_of_window_kinds_is_complete() {
        let decl = include_str!("../../../qymcad-ui-state/src/lib.rs");
        let body = decl.split("pub enum WinKind {").nth(1).expect("the declaration of WinKind");
        let body = body.split("\n}").next().expect("the end of the declaration");
        let declared = body.lines().filter(|l| l.trim().ends_with(',') && !l.trim().starts_with("//")).count();
        assert_eq!(declared, WinKind::ALL.len(), "WinKind::ALL must list every variant");
    }

    /// EVERY FIELD OF A CONTEXT THAT IS HELD BY VALUE, as (context, field).
    ///
    /// Parsed rather than listed, so it follows the code.
    fn fields_held_by_value() -> Vec<(String, String)> {
        let mut out = Vec::new();
        let mut ctx: Option<String> = None;
        for line in super::CTX_DECL.lines() {
            if let Some(rest) = line.strip_prefix("pub struct ") {
                let name = rest.split('<').next().unwrap_or("").trim().to_string();
                ctx = name.ends_with("Ctx").then_some(name);
                continue;
            }
            if line == "}" {
                ctx = None;
                continue;
            }
            let Some(c) = ctx.as_ref() else { continue };
            let Some(rest) = line.strip_prefix("    pub ") else { continue };
            let Some((field, ty)) = rest.split_once(": ") else { continue };
            if !ty.starts_with('&') {
                out.push((c.clone(), field.to_string()));
            }
        }
        out.sort();
        out
    }

    /// A FIELD OF A CONTEXT HELD BY VALUE IS NEVER ASSIGNED TO.
    ///
    /// THE ONE CLASS OF TRANSFER ERROR THE COMPILER SAYS NOTHING ABOUT. A method becomes a free function, the
    /// field it wrote to arrives in the context BY VALUE, and `self.viewing.mode_3d = true` becomes `pc.mode_3d = true` -
    /// an assignment to a copy that dies with the call. The build is green, there is no warning, the type is
    /// right, `dead_code` is silent. Three tools of the Part went quiet at once that way, and it was found by
    /// two behaviour tests rather than by anything structural.
    ///
    /// Found live when this check was written: the properties panel wrote `pr.mode_3d = true` for "pick the face
    /// again", and the switch to the viewport never happened. `PropsCtx::mode_3d` is `&mut bool` now.
    #[test]
    fn nothing_assigns_to_a_context_field_held_by_value() {
        let mut caught = Vec::new();
        for (ctx, field) in fields_held_by_value() {
            let needle = format!(".{field} = ");
            for (n, line) in super::CTX_USERS.lines().enumerate() {
                let Some(at) = line.find(&needle) else { continue };
                let head = &line[..at];
                let name_start = head.len() - head.chars().rev().take_while(|c| c.is_alphanumeric() || *c == '_').count();
                // `*pc.mode_3d = ...` writes THROUGH a reference and is correct; the bare form is the defect.
                if name_start > 0 && head.as_bytes()[name_start - 1] == b'*' {
                    continue;
                }
                caught.push(format!("{ctx}::{field} at line {n}: {}", line.trim()));
            }
        }
        assert!(caught.is_empty(), "assignment to a context field held by value (it writes to a copy):\n{}", caught.join("\n"));
    }

    /// THE LIST OF FIELDS HELD BY VALUE IS THE ONE THAT WAS LOOKED AT.
    ///
    /// On equality, so that a field added by value forces a look and a field that quietly stops being one is
    /// noticed too. By value is right for a fact the panel only READS - which workbench it is, whether the view
    /// is 3D while drawing. It is wrong for anything the panel changes, and the difference is invisible to the
    /// compiler, so it is written down here instead.
    #[test]
    fn the_context_fields_held_by_value_are_the_reviewed_ones() {
        let seen: Vec<String> = fields_held_by_value().into_iter().map(|(c, f)| format!("{c}::{f}")).collect();
        let reviewed = [
            "JointCtx::mode_3d",   // read: the glyphs are drawn only in 3D
            "JointCtx::workbench", // read: which workbench is current
            "PartCtx::workbench",  // read
            "PropsCtx::workbench", // read
            "StatusCtx::cursor",   // read: where the pointer is, printed as two numbers and changed by nobody
            "TreeCtx::workbench",  // read
            "WinCtx::file_ask_open", // read: a window does not open a second file dialogue over the first
            "WinCtx::workbench",     // read: the command search puts its own commands first
        ];
        assert_eq!(seen, reviewed, "the fields held by value have changed - look at each before changing this list");
    }
}
