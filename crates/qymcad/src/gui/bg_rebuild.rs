//! A BACKGROUND REBUILD DOES NOT LOSE EDITS.
//!
//! A rebuild clones the document, computes in a thread and replaces the live document with it
//! ENTIRELY. Input used to be muted across the whole screen for that time, so there was seemingly
//! nowhere to edit from. That is no longer so: a pinpoint rebuild runs QUIETLY, with no window and no
//! barrier (there is no reason to hold a person where the work takes a second), and under the window
//! of a full rebuild the canvas stays alive. That is, the "edit while it computes" path is no longer
//! hypothetical — it is the normal one.
//!
//! The pairing of "clone plus full replacement" is dangerous by construction: an edit that slipped
//! past the lock would vanish without a trace and without an error.
//!
//! So what is checked is a PROPERTY: a document that has changed since the rebuild started does not
//! let a stale result overwrite it.
#[cfg(test)]
mod tests {
    use super::super::App;

    /// The document moved on while the rebuild was computing, so its result is stale and must not be
    /// applied.
    #[test]
    fn an_edit_made_during_a_background_rebuild_is_not_lost() {
        let mut app = App::default();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        app.rebuild_if_dirty();

        // THE SNAPSHOT the background rebuild went off with
        let stale = app.project.clone_without_source_data();
        let stamp = app.regen_doc_stamp();

        // while it was computing, a person added a part
        super::super::joint_flow::tests::add_part_at(&mut app, 100.0);
        let after_edit = app.project.components.len();
        assert!(after_edit > stale.components.len(), "setup: the edit must change the document");

        // the result arrives — and must be rejected as stale
        app.finish_regen_checked(stamp, stale, Vec::new(), Vec::new(), Vec::new(), false);
        assert_eq!(
            app.project.components.len(),
            after_edit,
            "an edit made during a background rebuild is gone: a stale result overwrote the live document"
        );
    }

    /// A QUIET REBUILD FOLLOWS THE SAME LAW. It runs WITH no window and no barrier: nobody stops a
    /// person editing while it computes, and that is exactly why a stale result must be rejected and a
    /// rebuild asked for again. Otherwise silence in the interface would become silence while work is
    /// being lost.
    #[test]
    fn an_edit_during_a_quiet_rebuild_is_not_lost_and_the_rebuild_is_asked_again() {
        let mut app = App::default();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        super::super::joint_flow::tests::add_part_at(&mut app, 100.0);
        app.regenerate_now();

        let first = app.project.timeline.iter().find(|n| n.kind.body().is_some()).map(|n| n.id).expect("the node of the body");
        app.project.mark_node_dirty(first);
        app.spawn_regen();
        assert!(matches!(&app.regen.busy, Some(b) if b.quiet), "setup: editing one node must compute quietly");

        let stale = app.project.clone_without_source_data();
        let stamp = app.regen_doc_stamp();
        super::super::joint_flow::tests::add_part_at(&mut app, 200.0); // a person is working, nobody is holding them
        let after_edit = app.project.components.len();

        app.finish_regen_checked(stamp, stale, Vec::new(), Vec::new(), Vec::new(), false);
        assert_eq!(app.project.components.len(), after_edit, "an edit during a QUIET rebuild is gone");
        assert!(
            app.regen.wanted || app.regen.busy.is_some() || app.project.timeline.iter().any(|n| n.dirty),
            "the stale result was rejected and no new rebuild was asked for — the document would be left unbuilt"
        );
    }

    /// The document did not change, so the result is applied — otherwise a rebuild would be
    /// pointless.
    #[test]
    fn an_untouched_document_accepts_the_rebuild_result() {
        let mut app = App::default();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        app.rebuild_if_dirty();
        let mut rebuilt = app.project.clone_without_source_data();
        let stamp = app.regen_doc_stamp();
        rebuilt.add_part("came from the thread"); // a distinguishable sign that it was this one that was applied
        let want = rebuilt.components.len();
        app.finish_regen_checked(stamp, rebuilt, Vec::new(), Vec::new(), Vec::new(), false);
        assert_eq!(app.project.components.len(), want, "an untouched document must accept the result of a rebuild");
    }

    /// DRAGGING A PART DURING A REBUILD NEITHER CANCELS THE REBUILD NOR IS CANCELLED BY IT.
    ///
    /// Reported behaviour, and the reason this exists: press Edit -> Rebuild everything, start
    /// dragging — and the modal rebuild window flashes; drag on, and the second joint behaves like
    /// rubber, falling behind and catching up later.
    ///
    /// HERE IS WHERE BOTH COME FROM. "Rebuild everything" takes the work into a thread. On return the
    /// result was compared against the FULL fingerprint of the document, and that includes THE
    /// ARRANGEMENT — where the parts stand. A person drags a part, the fingerprint changes every
    /// frame, the result is declared stale and THROWN AWAY, and a new rebuild is asked for after it.
    /// The circle closes: the window flashes until the hand stops, and the placements only catch up
    /// once the circle is finally broken.
    ///
    /// The right question is a different one: a result is stale if WHAT IT WAS COMPUTED FROM has
    /// changed. Dragging a part is not the timeline. So the result is accepted, and the arrangement is
    /// taken LIVE — the one a person has just made by hand, not the one copied off a minute ago.
    #[test]
    fn dragging_while_a_rebuild_runs_keeps_both_the_drag_and_the_rebuild() {
        let mut app = App::default();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        super::super::joint_flow::tests::add_part_at(&mut app, 60.0);
        app.rebuild_if_dirty();

        // the rebuild went into the thread with this snapshot
        let mut rebuilt = app.project.clone_without_source_data();
        let stamp = app.regen_doc_stamp();
        rebuilt.add_part("came from the thread"); // a distinguishable sign that the result WAS APPLIED

        // while it was computing, a person was DRAGGING a part: the arrangement changed, the timeline did not
        let comp = app.project.components.iter().find(|c| c.parent == Some(app.project.root)).map(|c| c.id).expect("a part in the assembly");
        let mut moved = app.project.component_transform(comp);
        moved[3] += 37.0;
        app.project.set_component_transform(comp, moved);

        app.finish_regen_checked(stamp, rebuilt, Vec::new(), Vec::new(), Vec::new(), false);

        assert!(
            app.project.components.iter().any(|c| c.name == "came from the thread"),
            "the result of the rebuild was thrown away because a person was dragging a part: {}",
            app.status
        );
        let now = app.project.component_transform(comp);
        assert!(
            (now[3] - moved[3]).abs() < 1e-9,
            "the rebuild put the part back where it stood before the drag: x={} was expected, and x={} came out",
            moved[3],
            now[3]
        );
    }
}
