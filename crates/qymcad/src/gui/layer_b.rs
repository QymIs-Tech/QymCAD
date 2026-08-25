//! LAYER B: ONE CONTRACT ACROSS THE WHOLE COMMAND CATALOGUE.
//!
//! Tool coverage was written by hand and unevenly: some commands have their own `*_flow.rs` and some
//! do not, and nothing stopped a new tool from being forgotten — that is how the preview went missing
//! from "copy face" and the edge preparation from the patch. One sweep over the catalogue closes it: a
//! new command falls under the contract automatically, and an exception has to be named out loud.
#[cfg(test)]
mod tests {
    
    use crate::command_catalog::{Launch, COMMANDS};

    /// The timeline feature commands from the catalogue.
    fn feat_kinds() -> Vec<(&'static str, u8)> {
        COMMANDS
            .iter()
            .filter_map(|c| match c.launch {
                Launch::Feat(k) => Some((c.code, k)),
                _ => None,
            })
            .collect()
    }

    /// EVERY COMMAND IN THE CATALOGUE HAS A BUTTON. A command without a button exists only for
    /// search: a person looking for it with their eyes in the toolbar will not find it.
    #[test]
    fn every_command_has_a_button() {
        // THE TOOLBAR IS NOT ONE FILE: the shared creation bar lives in `gui.rs` (that is where the
        // datum buttons are) while the workbench ones are in `panels.rs`. Checking one file would
        // declare missing what actually exists.
        let panels = [crate::gui::panels_source::PANELS, include_str!("../gui.rs")].concat();
        let missing: Vec<&str> = feat_kinds()
            .into_iter()
            .filter(|(_, k)| !panels.contains(&format!("start_feat_cmd({k})")))
            .map(|(code, _)| code)
            .collect();
        assert!(missing.is_empty(), "the toolbar has no buttons for these commands: {}", missing.join(", "));
    }

    /// EVERY COMMAND HAS A PREVIEW OF ITS OWN, or a named reason why it does not.
    #[test]
    fn every_command_draws_what_it_is_doing() {
        let excused: &[(u8, &str)] = &[
            (1, "extrude draws a height gizmo"),
            (3, "revolve draws an angle gizmo"),
            (8, "sweep highlights its sketches in the tree"),
            (9, "loft highlights its sections in the tree"),
            (10, "primitives: the shape is drawn by the prim branch"),
            (11, "primitives: the same"),
            (12, "primitives: the same"),
            (13, "primitives: the same"),
            (14, "primitives: the same"),
            (15, "primitives: the same"),
            (20, "datums draw their own branch"),
            (21, "datums: the same"),
            (22, "datums: the same"),
        ];
        let render = crate::gui::render_source::RENDER;
        let drawn: std::collections::HashSet<u8> = render
            .lines()
            .filter(|l| l.contains("cmd.kind"))
            .flat_map(|l| l.split(|c: char| !c.is_ascii_digit()).filter(|t| !t.is_empty()).filter_map(|t| t.parse::<u8>().ok()).collect::<Vec<u8>>())
            .collect();
        let missing: Vec<String> = feat_kinds()
            .into_iter()
            .filter(|(_, k)| !excused.iter().any(|(e, _)| e == k))
            .filter(|(_, k)| !drawn.contains(k))
            .map(|(code, k)| format!("{code} (kind {k})"))
            .collect();
        assert!(missing.is_empty(), "these commands draw nothing, so to a person they \"do not work\": {}", missing.join(", "));
    }
}
