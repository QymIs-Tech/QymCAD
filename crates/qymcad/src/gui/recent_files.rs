//! RECENT FILES.
//!
//! There were none, neither in the menu nor anywhere else — even though the program had long been
//! storing the path to the previous project (`last_project`). That is, half the mechanism stood there
//! and a basic expectation went unmet.
//!
//! THE MAIN DECISION IS A SINGLE ENTRY POINT. "Opened a file" and "remembered a file" are one action
//! by a person, so a path becomes the current one only through `set_project_path`, and that same call
//! puts it into the list. Split them across call sites and forgetting the second would be a matter of
//! time: the compiler stays silent, no test goes red, and one day a person notices the list is a file
//! behind.
#[cfg(test)]
mod tests {
    use super::super::App;

    /// THE FRESHEST FIRST, WITH NO DUPLICATES. The list is read from the top.
    #[test]
    fn the_freshest_file_comes_first_and_never_twice() {
        let mut app = App::default();
        app.remember_recent("/a.qcad".into());
        app.remember_recent("/b.qcad".into());
        app.remember_recent("/a.qcad".into()); // back to the first one
        assert_eq!(app.recent_for_test(), vec!["/a.qcad", "/b.qcad"], "opening again must raise the file to the top instead of doubling it");
    }

    /// THE LENGTH OF THE LIST IS A SETTING, AND IT IS OBEYED.
    #[test]
    fn the_list_is_capped_by_the_setting() {
        let mut app = App::default();
        app.set_recent_limit_for_test(3);
        for i in 0..10 {
            app.remember_recent(format!("/f{i}.qcad"));
        }
        let got = app.recent_for_test();
        assert_eq!(got.len(), 3, "the list must be trimmed by the setting, and it came out {}", got.len());
        assert_eq!(got[0], "/f9.qcad", "the last one opened must be at the top");
    }

    /// AN EMPTY PATH DOES NOT GET INTO THE LIST — otherwise a row about nothing appears in the menu.
    #[test]
    fn an_empty_path_is_not_remembered() {
        let mut app = App::default();
        app.remember_recent(String::new());
        app.remember_recent("   ".into());
        assert!(app.recent_for_test().is_empty(), "an empty path leaked into the recent files");
    }

    /// A VANISHED FILE LEAVES THE LIST, and a person is told about it.
    ///
    /// Silence is not allowed: the list exists so that one gets there on the first try, and a row that
    /// does nothing when clicked reads as a broken program.
    #[test]
    fn a_vanished_file_leaves_the_list_and_says_so() {
        let mut app = App::default();
        let ghost = "/proc/qymcad-no-such/path/proj.qcad".to_string();
        app.remember_recent(ghost.clone());
        app.remember_recent("/tmp/alive.qcad".into());

        app.open_recent(ghost.clone());
        assert!(!app.recent_for_test().contains(&ghost), "a dead path must leave the list");
        assert!(!app.status_for_test().is_empty(), "a vanished file must be spoken about");
        assert_eq!(app.recent_for_test(), vec!["/tmp/alive.qcad"], "the other rows must not be touched");
    }

    /// THE CURRENT PATH IS SET THROUGH A SINGLE DOOR.
    ///
    /// A guard over the source: assigning `project_path` directly in the working code bypasses the
    /// remembering, and the list silently falls a file behind. The only legitimate place is the body
    /// of `set_project_path` itself; it is cut out of the text and no further ASSIGNMENTS of the path
    /// may remain. Clearing it (`= None`, "New project") is legitimate and not forbidden: it opens
    /// nothing, so there is nothing to remember. Tests may assign it: they set the scene rather than
    /// act out what a person does.
    #[test]
    fn the_current_path_is_set_through_a_single_door() {
        let mut sins = Vec::new();
        for (name, src) in [
            ("gui.rs", include_str!("../gui.rs")),
            ("io_jobs.rs", include_str!("io_jobs.rs")),
            ("panels.rs", crate::gui::panels_source::PANELS),
            ("commands.rs", include_str!("commands.rs")),
        ] {
            let code = src.split("#[cfg(test)]\nmod ").next().expect("the working part");
            // cut out the body of the setter — that is where the assignment belongs
            let cleaned = match code.find("fn set_project_path") {
                Some(a) => {
                    let end = code[a..].find("\n    }\n").map(|e| a + e).unwrap_or(code.len());
                    format!("{}{}", &code[..a], &code[end..])
                }
                None => code.to_string(),
            };
            for (i, line) in cleaned.lines().enumerate() {
                if line.contains("self.project_path = Some(") {
                    sins.push(format!("{name}: {}", line.trim()));
                    let _ = i;
                }
            }
        }
        assert!(
            sins.is_empty(),
            "the project path is assigned past `set_project_path` ({}) — the file will not reach the recent list:\n{}",
            sins.len(),
            sins.join("\n")
        );
    }
}
