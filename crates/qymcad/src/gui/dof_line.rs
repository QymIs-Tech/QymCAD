//! SKETCH DEFINEDNESS — ONE DEFINITION FOR THE WHOLE APPLICATION.
//!
//! The "Degrees of freedom: N — ..." line was assembled in two places by two copies, and they had
//! already drifted apart: one said "CONSTRAINT CONFLICT" and the other "DIMENSION CONFLICT" about the
//! same state. Copies of an interface drift silently — a person sees one thing in one place and
//! another elsewhere, with no way to tell which is true.
#[cfg(test)]
mod tests {
    /// Exactly one place assembles the line.
    #[test]
    fn the_dof_line_is_built_in_exactly_one_place() {
        let files = [
            ("gui.rs", include_str!("../gui.rs")),
            ("sketching.rs", include_str!("sketching.rs")),
            ("panels.rs", crate::gui::panels_source::PANELS),
        ];
        // look for the CALL that assembles the line rather than for the phrase itself: the phrase
        // moved into the language catalogue, and holding on to it would mean checking text instead of
        // substance again
        let n: usize = files.iter().map(|(_, s)| s.matches("\"dof-line\"").count()).sum();
        assert_eq!(
            n, 1,
            "the definedness line must be assembled in one place (`sketch_dof_line`), and {n} places do it — \
             the copies will drift silently, as \"CONSTRAINT CONFLICT\" and \"DIMENSION CONFLICT\" already did"
        );
    }
}
