//! "REBUILD EVERYTHING" ASKS FOR THE LIVE GEOMETRY ANEW AS WELL.
//!
//! The command drops the live B-rep (otherwise features would land on stale faces) — and stopped
//! there. The two flags that describe exactly what it had just thrown away stayed as they were:
//! "preparation finished" and "already attempted at this revision". After it, the program believed
//! the live geometry was ready although no body had any, and a repeat preparation returned
//! immediately on the "already attempted" guard.
//!
//! Reported behaviour: the Rebuild Everything menu item made no difference and the B-rep still did
//! not build. That is what happened: the single command a person could use to put things right
//! declared the work done without doing it.
#[cfg(test)]
mod tests {
    

    #[test]
    fn after_rebuilding_everything_the_geometry_is_asked_for_anew() {
        let mut app = super::super::screen_keys::tests::plate();
        app.rebuild_if_dirty();
        app.ensure_brep_for_test();
        app.drain_bg_for_test();
        app.rebuild_if_dirty();

        // GUARD AGAINST A VACUOUS CHECK: a live body really did appear, otherwise there is nothing to drop and the test proves nothing.
        let body = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).expect("the part has a body");
        assert!(app.has_shape_for_test(body), "setup: there is no live B-rep, so there is nothing to drop");

        app.rebuild_everything_for_test();

        // The live body is gone, and the program must acknowledge that rather than count it ready.
        assert!(!app.has_shape_for_test(body), "\"rebuild everything\" must drop the live B-rep");
        assert!(
            !app.brep_ready_for_test(),
            "the live B-rep was thrown away while the preparation counts as finished — a person is told there is nothing to wait for exactly where waiting is required"
        );

        // And the repeat preparation must GET TO WORK rather than return on the already-attempted guard.
        app.ensure_brep_for_test();
        app.drain_bg_for_test();
        app.rebuild_if_dirty();
        assert!(
            app.has_shape_for_test(body),
            "after \"rebuild everything\" the live geometry did not come back — the command declared the work done without doing it"
        );
    }
}
