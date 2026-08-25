//! THE TOOL-TO-ARTICLE LINK AND F1 BY CONTEXT.
//!
//! The help rots silently: a tool is renamed, split in two, removed — and the article stays and goes
//! on lying with confidence. The only cure is a link the build checks.
//!
//! There are three guards here, different in meaning:
//! 1. **every tool of the panels is in the table** — otherwise F1 will not answer;
//! 2. **every row of the table points at a tool that exists** — otherwise it is an article about
//!    something the program does not have;
//! 3. **THE RATCHET**: how many promised articles are still unwritten. It is lowered step by step down
//!    to zero. Demanding all the articles at once would keep the build red for weeks — that is,
//!    switch the guard off; the same device is already at work on the counter of untranslated
//!    lines.
#[cfg(test)]
mod tests {
    use crate::{help, help_map};

    /// The numbers of the tools actually wired into the panels of the workbench.
    fn tools_in_panels() -> (Vec<u8>, Vec<(String, u8)>) {
        // TWO FILES, not one: the workbench panel lives in `panels.rs`, while the shared creation panel
        // (datums, sketch) lives in `gui.rs`. A guard looking at one place missed the datums entirely —
        // three commands with no article, and F1 on them would answer with a section instead of the
        // matter at hand.
        let joined = format!("{}\n{}", crate::gui::panels_source::PANELS, include_str!("../gui.rs"));
        let src: &str = &joined;
        let mut part = Vec::new();
        let mut sketch = Vec::new();
        let mut rest = src;
        while let Some(i) = rest.find("start_") {
            let tail = &rest[i..];
            rest = &tail[6..];
            for (pat, _) in [("start_feat_cmd(", 0), ("start_prim_cmd(", 0)] {
                if let Some(t) = tail.strip_prefix(pat) {
                    if let Some(end) = t.find(')') {
                        if let Ok(n) = t[..end].parse::<u8>() {
                            if !part.contains(&n) {
                                part.push(n);
                            }
                        }
                    }
                }
            }
        }
        for (pat, handle) in [("set_sk_tool(", "sk"), ("set_dim_tool(", "dim"), ("set_click_op(", "click"), ("modify_button(", "mod")] {
            let mut rest = src;
            while let Some(i) = rest.find(pat) {
                let t = &rest[i + pat.len()..];
                rest = t;
                if let Some(end) = t.find(')') {
                    if let Ok(n) = t[..end].parse::<u8>() {
                        let key = (handle.to_string(), n);
                        if !sketch.contains(&key) {
                            sketch.push(key);
                        }
                    }
                }
            }
        }
        part.sort();
        sketch.sort();
        (part, sketch)
    }

    /// EVERY TOOL OF THE PANELS HAS AN ARTICLE IN THE TABLE. Otherwise F1 on it stays silent.
    #[test]
    fn every_tool_in_the_panels_has_an_article() {
        let (part, sketch) = tools_in_panels();
        assert!(part.len() > 20, "suspiciously few Part commands were found: {}", part.len());
        assert!(sketch.len() > 15, "suspiciously few Sketch tools were found: {}", sketch.len());
        for n in &part {
            assert!(help_map::part_article(*n).is_some(), "the Part command {n} is in the panel and there is no article for it in the table — F1 on it will stay silent");
        }
        for (h, n) in &sketch {
            assert!(help_map::sketch_article(h, *n).is_some(), "the Sketch tool {h}({n}) is in the panel and there is no article for it in the table");
        }
    }

    /// AND THE OTHER WAY ROUND: the table promises no articles about what the program does not have.
    #[test]
    fn the_table_promises_nothing_that_does_not_exist() {
        let (part, sketch) = tools_in_panels();
        for (n, a) in help_map::PART {
            assert!(part.contains(n), "the table holds command {n} (\"{a}\") and the panels do not — an article about a tool that does not exist");
        }
        for (h, n, a) in help_map::SKETCH {
            assert!(sketch.contains(&(h.to_string(), *n)), "the table holds the tool {h}({n}) (\"{a}\") and the panels do not");
        }
    }

    /// EVERY TOOL HAS AN ARTICLE THAT IS WRITTEN. The mark reached zero, so the ratchet became an
    /// ordinary guard: a new tool must now arrive WITH AN ARTICLE rather than with a promise.
    ///
    /// While the articles were being written a ratchet with a falling mark stood here: demanding all of
    /// them at once would have kept the build red for weeks, and then the guard would simply have been
    /// switched off. The mark went 39 -> 19 -> 0, and no further leniency is needed.
    #[test]
    fn every_promised_article_is_written() {
        let missing: Vec<&str> = help_map::promised().into_iter().filter(|a| help::article(a).is_none()).collect();
        assert!(
            missing.is_empty(),
            "the tool is there and there is no article for it ({}): {missing:?}\nF1 on it will show that the article is not written yet.",
            missing.len()
        );
    }

    /// F1 ANSWERS ABOUT ASSEMBLY MODES TOO — they are not timeline commands, but they are what a
    /// person is busy with.
    #[test]
    fn f1_answers_about_assembly_modes() {
        let mut app = super::super::App::default();
        app.start_rigid_joint_pick_for_test();
        assert_eq!(app.help_for_context(), "assembly/02-joints", "while the faces of a mate are being picked, F1 must lead to the article about mates");
        app.cancel_all_tools();

        app.start_comp_array_mode_for_test(1);
        assert_eq!(app.help_for_context(), "assembly/04-arrays", "while a component array is being laid out, F1 must lead to the article about arrays");
    }

    /// AND EVERY WORKBENCH HAS A SECTION ARTICLE — that is where F1 leads when no command is active.
    #[test]
    fn every_workbench_has_a_section_article() {
        // "cam" too: the workbench exists, so F1 in it must answer. That the article is a single short
        // one is a separate conversation (the module is being reworked), but emptiness there must not be.
        for wb in ["sketch", "part", "assembly", "cam"] {
            let a = help_map::workbench_article(wb);
            assert!(help::article(a).is_some(), "the workbench \"{wb}\" has no section article \"{a}\" — F1 with no command will show emptiness");
        }
    }

    /// F1 ANSWERS ABOUT WHAT A PERSON IS BUSY WITH rather than opening the title page.
    #[test]
    fn f1_answers_about_the_active_tool() {
        let mut app = super::super::screen_keys::tests::plate();
        // with no command — the section of the workbench
        app.cancel_all_tools();
        assert_eq!(app.help_for_context(), help_map::workbench_article(app.workbench_code()), "with no active command F1 must lead to the section of the workbench");

        // with a command — its article
        let body = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the body");
        app.sel = super::super::Sel::Mesh(app.project.mesh_index(body).expect("the mesh"));
        app.start_feat_cmd(7); // the hole
        assert_eq!(app.help_for_context(), "part/08-hole", "F1 inside the open Hole command must lead to the article about holes, and it leads to \"{}\"", app.help_for_context());
    }

    /// AND THE KEY IS REALLY WIRED. A table without the key is a link nobody can reach.
    #[test]
    fn the_f1_key_is_wired() {
        let input = include_str!("input.rs");
        assert!(input.contains("egui::Key::F1"), "F1 is not handled at all");
        assert!(input.contains("self.help_for_context()"), "F1 opens the help past the context");
    }

    /// AND A BUTTON THAT LAUNCHES NO NUMBERED COMMAND HAS AN ARTICLE TOO.
    ///
    /// THE BLIND SPOT THIS GUARD EXISTS FOR. The check above looks for tools BY NUMBER, and such
    /// buttons are 55 of the 91 in the panels: the rest call a handle of their own (`arm_body_boolean`,
    /// `start_move_tool`, `start_pattern` and so on). The rule did not touch them, and the holes piled
    /// up silently — a measurement found six: the boolean of bodies, move/copy/rotate in the sketch, the
    /// array in the sketch, measurement, the library and the auger. All six lay EXACTLY beyond the
    /// boundary of what was being checked.
    ///
    /// The contract is simple: every `tb-...` hint from a panel has either an article
    /// (`help_map::TOOLBAR`) or a reason stated out loud for why it is not a tool
    /// (`help_map::NOT_A_TOOL`).
    #[test]
    fn every_toolbar_button_has_an_article_or_a_stated_reason() {
        let joined = format!("{}\n{}", crate::gui::panels_source::PANELS, include_str!("../gui.rs"));
        let mut hints: Vec<String> = Vec::new();
        let mut rest: &str = &joined;
        while let Some(i) = rest.find("tr(\"tb-") {
            let t = &rest[i + 4..];
            rest = t;
            let Some(end) = t.find('"') else { break };
            let key = t[..end].to_string();
            if !hints.contains(&key) {
                hints.push(key);
            }
        }
        assert!(hints.len() > 60, "suspiciously few toolbar hints were found: {}", hints.len());
        let launchers = ["start_feat_cmd(", "start_prim_cmd(", "set_sk_tool(", "set_dim_tool(", "set_click_op(", "modify_button("];
        let mut bad: Vec<String> = Vec::new();
        for h in &hints {
            // the button of a numbered command: its article comes from PART/SKETCH and is checked above
            let numbered = joined.match_indices(&format!("tr(\"{h}\")")).any(|(i, _)| {
                // THE EDGE OF THE WINDOW IS BY CHARACTER, NOT BY BYTE: the panels carry non-ASCII
                // text, and a slice in the middle of a letter crashes the check instead of finding
                // trouble.
                let end = joined[i..].char_indices().map(|(o, _)| i + o).take_while(|o| *o - i <= 320).last().unwrap_or(i);
                let win = &joined[i..end];
                launchers.iter().any(|p| win.contains(p))
            });
            if numbered {
                continue;
            }
            match crate::help_map::toolbar_article(h) {
                Some(a) => {
                    if crate::help::article(a).is_none() {
                        bad.push(format!("{h}: the article \"{a}\" is promised and it is not there"));
                    }
                }
                None if crate::help_map::NOT_A_TOOL.contains(&h.as_str()) => {}
                None => bad.push(format!("{h}: the button is there and the article is not — F1 on it will answer with a section instead of the matter at hand")),
            }
        }
        assert!(bad.is_empty(), "toolbar buttons with no help ({}):\n{}", bad.len(), bad.join("\n"));
    }

    /// AND F1 REALLY REACHES THE ARTICLE rather than the table merely promising it.
    ///
    /// THE CHECK ABOVE TURNED OUT TO BE INSUFFICIENT, and that is worth writing down. It asked the
    /// table whether there is a row and whether the article is written — and it was green while
    /// `help_for_context` KNEW NOTHING about that table at all. Five new articles existed while F1 on
    /// the boolean of bodies opened the contents of the section. It was the compiler that said so
    /// (`toolbar_article` is never used), not the guard.
    ///
    /// So what a person sees is what gets asked: the hand is put into the same state the button itself
    /// shows as pressed, and F1 is pressed.
    #[test]
    fn f1_reaches_the_article_of_an_armed_toolbar_tool() {
        let cases: &[(&str, fn(&mut crate::gui::App))] = &[
            ("tb-bool-bodies-hint", |a| a.arm_boolean_for_test()),
            ("tb-move-hint", |a| a.start_move_tool_for_test(1)),
            ("tb-copy-hint", |a| a.start_move_tool_for_test(2)),
            ("tb-rotate-hint", |a| a.start_move_tool_for_test(3)),
            ("tb-lin-array-hint", |a| a.start_pattern_for_test(1)),
            ("tb-circ-array-hint", |a| a.start_pattern_for_test(2)),
            ("tb-measure3d-hint", |a| a.arm_measure3d_for_test()),
        ];
        for (hint, arm) in cases {
            let mut app = crate::gui::App::default();
            arm(&mut app);
            assert_eq!(app.armed_toolbar_hint(), Some(*hint), "the hand is occupied and F1 does not recognise it: {hint}");
            let want = help_map::toolbar_article(hint).expect("the article is promised by the table");
            assert_eq!(app.help_for_context(), want, "F1 with the hand occupied ({hint}) leads to somebody else's article");
            assert!(help::article(want).is_some(), "the article {want} is promised but not written");
        }
    }
}
