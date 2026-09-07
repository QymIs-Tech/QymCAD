//! A REQUEST PUT OFF, AND A WRITE MADE AT ONCE: the order that quietly undoes the click.
//!
//! A panel does not act on the document itself. It puts a named request into `ask`, and the frame carries
//! it out AFTER the drawing - so a panel never redraws a tree it has just changed. That is the rule, and
//! it has a trap in it: anything the panel writes DIRECTLY happens FIRST, and the deferred request then
//! runs over it.
//!
//! Measured, on a live button. The ruler in the Part toolbar set `measure.on = true` and pushed
//! `BarAsk::SketchSelectMode`; that request calls `exit_draw_tools`, which clears the measuring tool along
//! with the rest. On, then off, in the same frame - the button did nothing at all, and nobody reported it,
//! because a button that does nothing looks like a button one has misunderstood.
//!
//! THE SIGNAL. Inside one click block: a request is pushed, and after it the block writes a field that
//! THE REQUEST ITSELF CLEARS. What a request clears is not a list written here by hand - it is read out
//! of the request's own handler, and one call deep, because a handler usually delegates
//! (`BarAsk::SketchSelectMode` clears nothing itself; `exit_draw_tools`, which it calls, clears eleven
//! tools).
//!
//! A hand-written list was the first shape of this check, and it was wrong twice over: it named eleven
//! tool records, so it missed the case that a request clears the SELECTION (entering a component resets
//! `sel`, and a click that entered and then selected arrived nowhere), and it would have gone stale the
//! day a request began clearing something new.
//!
//! The cure is never "move the write above the push". It is a DOOR: one function that releases the others
//! and takes this one, in that order, the way `set_measure` and `set_dim_tool` do.
#[cfg(test)]
mod tests {
    /// The panels that put requests: the same list the source-reading guards use.
    const PANELS: [(&str, &str); 7] = [
        ("Part", include_str!("../../../qymcad-part/src/lib.rs")),
        ("Sketch", include_str!("../../../qymcad-sketch/src/lib.rs")),
        ("Assembly", include_str!("../../../qymcad-assembly/src/lib.rs")),
        ("bars", include_str!("panels_bars.rs")),
        ("tree", include_str!("panels_tree.rs")),
        ("properties", include_str!("panels_props.rs")),
        ("windows", include_str!("panels_windows.rs")),
    ];

    /// Where the requests are carried out, and where the functions they delegate to live.
    const HANDLERS: [&str; 14] = [
        include_str!("../gui.rs"),
        include_str!("commands.rs"),
        include_str!("sketching.rs"),
        include_str!("joints.rs"),
        include_str!("io_jobs.rs"),
        include_str!("pick.rs"),
        include_str!("panels_tree.rs"),
        include_str!("file_ask.rs"),
        include_str!("help_window.rs"),
        include_str!("measure3d.rs"),
        include_str!("../../../qymcad-ui-state/src/lib.rs"),
        include_str!("../../../qymcad-sketch/src/lib.rs"),
        include_str!("../../../qymcad-part/src/lib.rs"),
        include_str!("../../../qymcad-assembly/src/lib.rs"),
    ];

    /// EVERY NAME A PATH GOES THROUGH, past the thing it starts from.
    ///
    /// The two sides spell the same state differently: a panel writes through a context (`bc.armed.measuring()`,
    /// `pr.sel`), a handler writes through the application (`self.tools.armed.measuring()`, `self.chosen.sel`).
    /// Only the middle names are shared, and WHICH middle name it is differs from case to case - the
    /// record is the leaf in `self.chosen.sel` and the one before it in `self.side.section.pick`. So both
    /// sides give every name they pass through and a shared one is enough.
    ///
    /// Clearing is written two ways in this code, and both count: `x.y = None` and `x.clear()`. Missing
    /// the second would have missed `exit_draw_tools`, which releases eleven tools by calling `clear` on
    /// each.
    fn assigned_names(body: &str) -> std::collections::HashSet<String> {
        fn segments(lhs: &str, skip_root: bool) -> Vec<String> {
            let lhs = lhs.trim().trim_start_matches('*').trim_start_matches("&mut ").trim();
            if lhs.contains('(') || lhs.contains('[') || lhs.contains(' ') {
                return Vec::new();
            }
            lhs.split('.')
                .skip(usize::from(skip_root))
                .filter(|s| !s.is_empty() && *s != "self" && s.chars().all(|c| c.is_alphanumeric() || c == '_'))
                .map(str::to_string)
                .collect()
        }
        let mut out = std::collections::HashSet::new();
        for line in body.lines() {
            let code = line.split("//").next().unwrap_or("");
            // a plain write
            if let Some(eq) = code.find(" = ") {
                if !code[..eq].ends_with(['=', '<', '>', '!']) {
                    out.extend(segments(code[..eq].trim(), true));
                }
            }
            // a release: `measure.clear()`, `cmd.close()`
            for call in [".clear()", ".close()"] {
                if let Some(k) = code.find(call) {
                    let head = code[..k].trim();
                    let start = head.rfind(|c: char| !(c.is_alphanumeric() || c == '_' || c == '.')).map(|i| i + 1).unwrap_or(0);
                    // `measure.clear()` names the record itself, so the first segment counts too
                    out.extend(segments(&head[start..], false));
                }
            }
        }
        out
    }

    /// Every function body in the handler sources, by name, built once.
    ///
    /// Built once because it is asked for thousands of times: scanning five large sources per lookup
    /// took the check past two minutes, and a check nobody waits for is a check nobody runs.
    fn bodies() -> &'static std::collections::HashMap<String, String> {
        static INDEX: std::sync::OnceLock<std::collections::HashMap<String, String>> = std::sync::OnceLock::new();
        INDEX.get_or_init(|| {
            let mut map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
            for src in HANDLERS {
                let b = src.as_bytes();
                let mut at = 0usize;
                while let Some(k) = src[at..].find("fn ") {
                    let s = at + k;
                    at = s + 3;
                    if s > 0 && (b[s - 1].is_ascii_alphanumeric() || b[s - 1] == b'_') {
                        continue;
                    }
                    let Some(paren) = src[s..].find('(') else { break };
                    let name = src[s + 3..s + paren].trim();
                    if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                        continue;
                    }
                    let Some(rel) = src[s + paren..].find('{') else { continue };
                    let open = s + paren + rel;
                    let mut depth = 0usize;
                    for (j, ch) in src[open..].char_indices() {
                        match ch {
                            '{' => depth += 1,
                            '}' => {
                                depth -= 1;
                                if depth == 0 {
                                    // the FIRST declaration wins; a name declared twice is read once
                                    map.entry(name.to_string()).or_insert_with(|| src[open + 1..open + j].to_string());
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            map
        })
    }

    /// Every function name called in `body`, whatever it is qualified with.
    fn calls_in(body: &str) -> Vec<String> {
        let mut out = Vec::new();
        let b = body.as_bytes();
        for (i, ch) in body.char_indices() {
            if ch != '(' || i == 0 {
                continue;
            }
            let mut j = i;
            while j > 0 && (b[j - 1].is_ascii_alphanumeric() || b[j - 1] == b'_') {
                j -= 1;
            }
            if j == i {
                continue;
            }
            let name = &body[j..i];
            if name.chars().next().is_some_and(|c| c.is_ascii_lowercase()) {
                out.push(name.to_string());
            }
        }
        out.sort();
        out.dedup();
        out
    }

    /// What one request clears: what its own arm assigns, plus what the functions it reaches assign.
    ///
    /// TWO CALLS DEEP. The chain that matters is exactly that long: `BarAsk::SketchSelectMode` calls
    /// `sketch_select_mode`, which calls `exit_draw_tools`, and only the last one clears anything. One
    /// hop would have missed the very case this check was written for; a deeper walk would start
    /// reporting whatever the document itself sets on the way.
    fn cleared_by(request: &str) -> std::collections::HashSet<String> {
        let mut out = std::collections::HashSet::new();
        for src in HANDLERS {
            let mut at = 0usize;
            while let Some(k) = src[at..].find(&format!("Ask::{request}")) {
                let s = at + k;
                at = s + 4;
                let Some(rel) = src[s..].find("=>") else { break };
                let arm_start = s + rel + 2;
                let line_end = src[arm_start..].find('\n').map(|i| arm_start + i).unwrap_or(src.len());
                let first = src[arm_start..line_end].trim();
                let arm = if first.starts_with('{') {
                    let open = arm_start + src[arm_start..].find('{').unwrap_or(0);
                    let mut depth = 0usize;
                    let mut end = line_end;
                    for (j, ch) in src[open..].char_indices() {
                        match ch {
                            '{' => depth += 1,
                            '}' => {
                                depth -= 1;
                                if depth == 0 {
                                    end = open + j;
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                    src[open + 1..end].to_string()
                } else {
                    first.to_string()
                };
                out.extend(assigned_names(&arm));
                for first in calls_in(&arm) {
                    let Some(b1) = bodies().get(&first) else { continue };
                    out.extend(assigned_names(b1));
                    for second in calls_in(b1) {
                        if let Some(b2) = bodies().get(&second) {
                            out.extend(assigned_names(b2));
                        }
                    }
                }
            }
        }
        out
    }

    /// The body of every `if ...clicked()/icon_tool/sym_button... {` block, with the line it starts on.
    fn click_blocks(src: &str) -> Vec<(usize, &str)> {
        let mut out = Vec::new();
        let bytes = src.as_bytes();
        let mut at = 0usize;
        // THE NEAREST OF THE THREE, not the first that happens to exist. `find(a).or_else(find(b))` picks by
        // WHICH PATTERN, not by WHERE: one `clicked()` far below hid every `icon_tool(` above it, and the
        // ruler - the button this whole check was written for - was among them. Caught by putting the
        // defect back into the tree and watching the guard stay green.
        while let Some(rel) = ["clicked()", "icon_tool(", "sym_button("].iter().filter_map(|p| src[at..].find(p)).min() {
            let hit = at + rel;
            // the line has to START the block: `if ... {`
            let line_start = src[..hit].rfind('\n').map(|i| i + 1).unwrap_or(0);
            let line_end = src[hit..].find('\n').map(|i| hit + i).unwrap_or(src.len());
            let line = &src[line_start..line_end];
            at = line_end.max(hit + 1);
            if !line.trim_start().starts_with("if ") || !line.trim_end().ends_with('{') {
                continue;
            }
            let mut depth = 0usize;
            let mut i = line_end;
            let open = line_end - 1;
            let mut end = None;
            let mut j = open;
            while j < bytes.len() {
                match bytes[j] {
                    b'{' => depth += 1,
                    b'}' => {
                        depth -= 1;
                        if depth == 0 {
                            end = Some(j);
                            break;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            i = i.max(open);
            let _ = i;
            if let Some(e) = end {
                out.push((src[..line_start].matches('\n').count() + 1, &src[open + 1..e]));
            }
        }
        out
    }

    /// What follows the push INSIDE THE SAME BRANCH.
    ///
    /// A write in a different arm of the same `match` cannot run in the same click as the push, and
    /// reporting it is noise: the mirror button pushes its request under `Some(comp)` and writes the
    /// status under `None`. The scan therefore stops when the branch that holds the push ends - either
    /// its braces close, or a new arm begins with `=>` at the same depth.
    fn same_branch_after(block: &str, push: usize) -> &str {
        let b = block.as_bytes();
        let mut depth = 0i32;
        let mut i = push;
        while i < b.len() {
            match b[i] {
                b'{' | b'(' | b'[' => depth += 1,
                b'}' | b')' | b']' => {
                    depth -= 1;
                    if depth < 0 {
                        break;
                    }
                }
                b'=' if depth == 0 && i + 1 < b.len() && b[i + 1] == b'>' => break,
                _ => {}
            }
            i += 1;
        }
        &block[push..i]
    }

    /// The request named in `.ask.push(SomeAsk::Variant...)`, if the push names one.
    fn pushed_request(after: &str) -> Option<String> {
        let k = after.find("Ask::")? + 5;
        let name: String = after[k..].chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
        if name.is_empty() { None } else { Some(name) }
    }

    /// Every place in one source that shows the shape.
    fn find_in(src: &str) -> Vec<String> {
        let mut caught = Vec::new();
        for (line, block) in click_blocks(src) {
            let Some(push) = block.find(".ask.push(") else { continue };
            let after = same_branch_after(block, push);
            let Some(request) = pushed_request(after) else { continue };
            let cleared = cleared_by(&request);
            for f in cleared.iter().map(|s| s.as_str()) {
                // `x.tool.on = ` / `x.dim = ` - a write to the tool's own record
                for pat in [format!(".{f} = "), format!(".{f}.")] {
                    let Some(k) = after.find(&pat) else { continue };
                    let tail = &after[k + pat.len()..];
                    let writes = pat.ends_with(' ') || tail.lines().next().map(|l| l.contains(" = ") && !l.contains("==")).unwrap_or(false);
                    if writes {
                        caught.push(format!("line {line}: `{request}` is pushed, and after it the click writes `{f}` - which `{request}` itself clears, so the write is undone"));
                        break;
                    }
                }
            }
        }
        caught.sort();
        caught.dedup();
        caught
    }


    /// THE TABLE MUST SAY WHEN IT CANNOT SEE.
    ///
    /// What a request clears is read out of the handler's own code, so a handler this file cannot read is
    /// a request that appears to clear NOTHING - and the sweep above then passes it in silence. That is
    /// the same fault the sweep is written against, one level up.
    ///
    /// This one was real: `HANDLERS` first listed five sources, and the arms call into
    /// `crate::gui::commands` and `crate::gui::sketching`, which were not among them. Written by hand an
    /// hour after the check that exists because hand-written lists go stale.
    #[test]
    fn every_function_a_request_calls_can_be_read() {
        let gui = HANDLERS[0];
        let mut blind: Vec<String> = Vec::new();
        let mut arms = 0usize;
        let mut at = 0usize;
        while let Some(k) = gui[at..].find("Ask::") {
            let s = at + k;
            at = s + 5;
            let Some(rel) = gui[s..].find("=>") else { break };
            let line_end = gui[s..].find('\n').map(|i| s + i).unwrap_or(gui.len());
            if s + rel > line_end {
                continue; // the `=>` belongs to a later line: this was a pattern, not an arm head
            }
            arms += 1;
            for name in calls_in(&gui[s + rel + 2..line_end]) {
                // the sugar of the frame itself, not a handler: these take no state and clear nothing
                if matches!(name.as_str(), "tr" | "tr1" | "tr2" | "trn" | "to_string" | "clone" | "push" | "some" | "from") {
                    continue;
                }
                if !bodies().contains_key(&name) {
                    blind.push(name);
                }
            }
        }
        blind.sort();
        blind.dedup();
        assert!(arms > 20, "the arms of the requests were not found at all: {arms}");
        assert!(
            blind.is_empty(),
            "a request calls something this check cannot read, so what it clears is unknown and the sweep passes it in silence. \
             Add the source to `HANDLERS`:\n{}",
            blind.join("\n")
        );
    }

    #[test]
    fn no_click_pushes_a_request_and_then_writes_a_tool() {
        let mut caught = Vec::new();
        for (where_, src) in PANELS {
            caught.extend(find_in(src).into_iter().map(|m| format!("{where_} {m}")));
        }
        assert!(
            caught.is_empty(),
            "a click puts a request off and writes a tool at once; the deferred one wins and the button does nothing:\n{}",
            caught.join("\n")
        );
    }

    /// THE SIGNAL CATCHES THE SHAPE IT IS FOR, on the ruler exactly as it was written.
    ///
    /// A guard over a tree that is already clean is green whether it works or not. This pins the detector
    /// itself: the sample below is the button that did nothing, copied from before the fix.
    #[test]
    fn the_signal_catches_the_shape_it_is_for() {
        let broken = concat!(
            "                if qymcad_ui_state::icon_tool(ui, ph::RULER, &t(\"tb-measure-hint\"), bc.armed.measuring()) {\n",
            "                    let on = !bc.armed.measuring();\n",
            "                    bc.ask.push(qymcad_ui_state::BarAsk::SketchSelectMode);\n",
            "                    bc.measure.on = on;\n",
            "                }\n",
        );
        assert_eq!(find_in(broken).len(), 1, "the detector did not name the ruler on the very sample it was written for");

        // THE CASE THE HAND-WRITTEN LIST COULD NOT SEE: a request that clears the SELECTION.
        //
        // Entering a component resets `sel`, so a click that asks to go somewhere and then selects
        // something arrives nowhere. The old check named eleven TOOL records and this is not one of them;
        // the table read out of the handler has it, because `WinAsk::GoTo` assigns `chosen.sel` itself.
        let goes_nowhere = concat!(
            "                if qymcad_ui_state::icon_tool(ui, ph::TREE, &t(\"go\"), false) {\n",
            "                    pr.ask.push(qymcad_ui_state::WinAsk::GoTo { owner: Some(o), sel });\n",
            "                    *pr.sel = qymcad_ui_state::Sel::Sketch(si);\n",
            "                }\n",
        );
        assert_eq!(find_in(goes_nowhere).len(), 1, "a request that clears the selection, followed by a write to it, went unnoticed");

        // AND THE OTHER HALF: the cure must NOT be named, or the guard would forbid the fix.
        let fixed = concat!(
            "                if qymcad_ui_state::icon_tool(ui, ph::RULER, &t(\"tb-measure-hint\"), bc.armed.measuring()) {\n",
            "                    let on = !bc.armed.measuring();\n",
            "                    qymcad_ui_state::set_measure(&mut qymcad_ui_state::tools_in!(bc), on);\n",
            "                }\n",
        );
        assert!(find_in(fixed).is_empty(), "the detector names the cure as a defect: {:?}", find_in(fixed));
    }
}
