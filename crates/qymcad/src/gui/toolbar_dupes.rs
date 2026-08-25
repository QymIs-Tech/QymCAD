//! NO WORKBENCH SHOWS TWO BUTTONS THAT LOOK THE SAME.
//!
//! Reported behaviour: the pattern buttons appear twice, in both the Body and the Pattern category.
//! It was in fact worse than duplication: "Body" held COMPONENT patterns (an assembly tool) while
//! "Pattern" held BODY patterns. The icons were the same, the actions were different, and the only
//! way to tell them apart was to hover.
//!
//! Component patterns moved to Assembly, where they belong (the Part workbench has no components at
//! all), and mirror plus body patterns are gathered into one "Body" group.
#[cfg(test)]
mod tests {
    /// The slice of source for one workbench, from its `match` arm to the next.
    fn workbench_block<'a>(code: &'a str, name: &str) -> &'a str {
        let at = code.find(&format!("Workbench::{name} => {{")).unwrap_or_else(|| panic!("the {name} workbench is there"));
        let rest = &code[at + 10..];
        let end = rest.find("Workbench::").map(|i| at + 10 + i).unwrap_or(code.len());
        &code[at..end]
    }

    /// The icons of the tool buttons within a slice of source.
    fn tool_icons(block: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut rest = block;
        while let Some(i) = rest.find("Self::icon_tool(ui, ph::") {
            let after = &rest[i + 24..];
            let end = after.find(|c: char| !(c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')).unwrap_or(after.len());
            out.push(after[..end].to_string());
            rest = &after[end..];
        }
        out
    }

    /// NO ICON REPEATS WITHIN A WORKBENCH.
    ///
    /// Two identical icons in one bar are either a duplicate or, worse, two different tools that the
    /// eye cannot tell apart. A person reads both as a breakage.
    #[test]
    fn no_workbench_shows_the_same_icon_twice() {
        let src = crate::gui::panels_source::PANELS;
        let code = src.split("#[cfg(test)]\nmod ").next().expect("the working part");
        let mut sins: Vec<String> = Vec::new();
        for wb in ["Sketch", "Part", "Assembly"] {
            let icons = tool_icons(workbench_block(code, wb));
            assert!(icons.len() > 3, "only {} buttons were found in the {wb} workbench: the bar parser has drifted from the code", icons.len());
            let mut seen: Vec<&String> = Vec::new();
            for ic in &icons {
                if seen.contains(&ic) {
                    sins.push(format!("{wb}: ph::{ic}"));
                } else {
                    seen.push(ic);
                }
            }
        }
        assert!(
            sins.is_empty(),
            "one workbench has two buttons with the same icon ({}):\n{}\n\
             Either it is a duplicate or two DIFFERENT tools the eye cannot tell apart; the second is worse.",
            sins.len(),
            sins.join("\n")
        );
    }
}
