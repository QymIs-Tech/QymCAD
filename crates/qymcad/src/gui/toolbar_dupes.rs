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
    use crate::command_catalog::{all, of_bench};
    use qymcad_ui_state::catalogue::Shelf;

    /// NO ICON REPEATS WITHIN A WORKBENCH.
    ///
    /// Two identical icons in one bar are either a duplicate or, worse, two different tools that the
    /// eye cannot tell apart. A person reads both as a breakage.
    #[test]
    fn no_workbench_shows_the_same_icon_twice() {
        let mut sins: Vec<String> = Vec::new();
        for wb in ["sketch", "part", "assembly"] {
            let rows = of_bench(wb);
            assert!(rows.len() > 3, "only {} buttons are on the bar of {wb}", rows.len());
            for (i, c) in rows.iter().enumerate() {
                if let Some(twin) = rows[..i].iter().find(|o| o.icon == c.icon) {
                    sins.push(format!("{wb}: {:?} on {} and {}", c.icon, twin.code, c.code));
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

    /// NO TOOL REUSES AN ICON ACROSS WORKBENCHES.
    ///
    /// Every tool has its own dedicated icon. Reusing icons across workbenches confuses the eye when switching between
    /// contexts. A row shown on two bars - a datum on the bars of the part and of the assembly - is one tool.
    #[test]
    fn no_tool_reuses_an_icon_across_workbenches() {
        let rows: Vec<_> = all().filter(|c| c.shelf == Shelf::Bar).collect();
        let mut sins: Vec<String> = Vec::new();
        for (i, c) in rows.iter().enumerate() {
            if let Some(twin) = rows[..i].iter().find(|o| o.icon == c.icon) {
                sins.push(format!("{:?} is used by {} and {}", c.icon, twin.code, c.code));
            }
        }
        assert!(sins.is_empty(), "tools reuse icons ({}):\n{}\nEvery tool must have a unique dedicated icon.", sins.len(), sins.join("\n"));
    }
}
