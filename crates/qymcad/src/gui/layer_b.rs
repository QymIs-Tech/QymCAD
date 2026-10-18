//! LAYER B: ONE CONTRACT ACROSS THE WHOLE COMMAND CATALOGUE.
//!
//! Tool coverage was written by hand and unevenly: some commands have their own `*_flow.rs` and some
//! do not, and nothing stopped a new tool from being forgotten — that is how the preview went missing
//! from "copy face" and the edge preparation from the patch. One sweep over the catalogue closes it: a
//! new command falls under the contract automatically, and an exception has to be named out loud.
#[cfg(test)]
mod tests {

    use crate::command_catalog::{all, Launch};
    use qymcad_ui_state::catalogue::Shelf;

    /// The timeline feature commands from the catalogue.
    fn feat_kinds() -> Vec<(&'static str, u8)> {
        all()
            .filter_map(|c| match c.launch {
                Launch::Feat(k) => Some((c.code, k)),
                _ => None,
            })
            .collect()
    }

    /// EVERY COMMAND OF THE TIMELINE HAS A BUTTON. A command without a button exists only for the search: a person
    /// looking for it with their eyes on the bar will not find it. The bar is drawn from the rows, so a button is a row
    /// on the shelf of the bar of a workbench.
    #[test]
    fn every_command_has_a_button() {
        let missing: Vec<&str> = all().filter(|c| matches!(c.launch, Launch::Feat(_)) && (c.shelf != Shelf::Bar || c.benches.is_empty())).map(|c| c.code).collect();
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
            .filter(|l| l.contains("armed.cmd_kind()"))
            .flat_map(|l| l.split(|c: char| !c.is_ascii_digit()).filter(|t| !t.is_empty()).filter_map(|t| t.parse::<u8>().ok()).collect::<Vec<u8>>())
            .collect();
        let missing: Vec<String> =
            feat_kinds().into_iter().filter(|(_, k)| !excused.iter().any(|(e, _)| e == k)).filter(|(_, k)| !drawn.contains(k)).map(|(code, k)| format!("{code} (kind {k})")).collect();
        assert!(missing.is_empty(), "these commands draw nothing, so to a person they \"do not work\": {}", missing.join(", "));
    }
}
