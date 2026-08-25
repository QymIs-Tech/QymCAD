//! ONE QUESTION BEFORE A DELETION — NO MATTER WHERE IT WAS PRESSED FROM.
//!
//! Only the tree asked, on Del and on "delete the part". The very same feature, sketch, plane, axis,
//! point, contour and body were removed SILENTLY by the button in the properties panel. That is,
//! whether you were asked at all depended on which route you took to ONE AND THE SAME action — the
//! same defect the editors in the right panel already had: what was available to you was decided by
//! the route rather than by the meaning.
//!
//! Now there is one entry (`ask_delete`), one executor (`execute_delete`), one question. And it
//! finally answers "what am I going to lose": the cascade is listed by name, from the same kernel
//! query (`Project::dependents_of`) that feeds the lineage in the properties card.
#[cfg(test)]
mod tests {
    use super::super::{App, Sel};

    /// A plate with a fillet: the fillet has a source, the extrude has a dependent.
    fn plate_with_a_fillet() -> App {
        let mut app = super::super::screen_keys::tests::plate();
        let body = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the body of the plate");
        let edges: Vec<u32> = app.project.regen_edges.get(&body).map(|es| es.iter().take(3).map(|e| e.id).collect()).unwrap_or_default();
        app.project.add_fillet(body, 1.0, edges);
        app.rebuild_if_dirty();
        app
    }

    /// NO DELETION GOES PAST THE QUESTION.
    ///
    /// Over the source deliberately: a behavioural test catches only the paths it was told about,
    /// while the trouble here is precisely a FORGOTTEN path — a button in a panel, a menu item, a
    /// hotkey. The guard looks at every place at once and therefore finds what nobody remembered.
    #[test]
    fn no_delete_button_bypasses_the_question() {
        let files: [(&str, &str); 3] = [
            ("panels.rs", crate::gui::panels_source::PANELS),
            ("gui.rs", include_str!("../gui.rs")),
            ("sketching.rs", include_str!("sketching.rs")),
        ];
        // the destroyers of the document: from the interface they may only be called through `execute_delete`
        let killers = ["delete_feature(", "delete_contour(", "delete_sketch_full(", "delete_body_mesh(", "delete_plane(", "delete_datum_axis(", "delete_datum_point(", "delete_component("];
        let mut leaks: Vec<String> = Vec::new();
        for (fname, src) in &files {
            for (ln, line) in src.lines().enumerate() {
                let code = line.split("//").next().unwrap_or("");
                if !killers.iter().any(|k| code.contains(k)) {
                    continue;
                }
                // declarations are not entries; and there are places where the question is beside the
                // point: those are marked right in the code with a REASON, otherwise an exception
                // would turn into a quiet way round the guard
                if code.contains("fn ") || line.contains("ask_delete-exempt:") {
                    continue;
                }
                leaks.push(format!("{fname}:{}: {}", ln + 1, code.trim()));
            }
        }
        assert!(
            leaks.is_empty(),
            "a deletion is called from the interface directly, past the question ({}):\n{}\nThe single entry is `ask_delete`, the removal is `execute_delete`.",
            leaks.len(),
            leaks.join("\n")
        );
    }

    /// THE QUESTION IS ASKED, AND THE DOCUMENT IS NOT TOUCHED UNTIL THE ANSWER.
    #[test]
    fn asking_changes_nothing_until_the_answer() {
        let mut app = plate_with_a_fillet();
        let before = app.project.timeline.len();
        app.ask_delete(Sel::Feature(0));
        assert_eq!(app.project.timeline.len(), before, "the question has not been asked yet and the timeline is already shorter — the deletion went through without an answer");
        assert!(app.deferred_delete_for_test(), "the question was not queued — the button did nothing");
    }

    /// THE QUESTION NAMES WHAT GOES WITH IT. For an extrude that carries a fillet the cascade is
    /// not empty.
    #[test]
    fn the_question_names_what_goes_with_it() {
        let app = plate_with_a_fillet();
        let extrude = app.project.timeline.iter().position(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::Extrude { .. })).expect("the extrude in the timeline");
        let names = app.delete_cascade_names_for_test(Sel::Feature(extrude));
        assert!(!names.is_empty(), "for an extrude that carries a fillet the cascade must not be empty");
        // and the topmost node has nothing to lose
        let last = app.project.timeline.len() - 1;
        assert!(app.delete_cascade_names_for_test(Sel::Feature(last)).is_empty(), "the last node of the timeline cannot have dependents");
    }

    /// AN ANSWER OF "YES" CARRIES IT THROUGH — by the same executor Del in the tree uses.
    #[test]
    fn answering_yes_deletes_through_the_single_executor() {
        let mut app = plate_with_a_fillet();
        let before = app.project.timeline.len();
        app.ask_delete(Sel::Feature(before - 1)); // the last one is the fillet
        app.execute_deferred_delete_for_test();
        assert!(app.project.timeline.len() < before, "the answer of yes deleted nothing: it was {before}, it became {}", app.project.timeline.len());
        assert!(!app.deferred_delete_for_test(), "the question must go away after the answer");
    }
}
