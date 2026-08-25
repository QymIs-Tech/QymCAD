//! PREPARING THE LIVE B-rep DOES NOT GO ROUND IN CIRCLES. Reported behaviour: pressing "rebuild
//! everything" throws the CAD into a fever.
//!
//! FOUND BY MEASUREMENT IN A LIVE WINDOW, on a real document. The program printed on every turn:
//!
//! ```text
//! B-rep preparation started: live shapes 0, brep_ready=false
//! rebuild result accepted
//! rebuild: 0 nodes of 142, quiet=true, from: tick_async <- frame_prologue <- update
//! B-rep preparation started: live shapes 0 ...   and so on without end
//! ```
//!
//! The circle works like this: "rebuild everything" throws away the live B-rep of ALL the bodies,
//! imports included. The timeline cannot restore an import — its geometry lives in the embedded STEP
//! rather than in the recipe — so the rebuild arrives with a plan of "0 nodes" and changes nothing.
//! The preparation sees that there are still no shapes, demands a rebuild again, and so on until the
//! program is closed.
//!
//! What is checked here is a PROPERTY rather than a case: an attempt that changed nothing is not
//! repeated. That guards the whole class — whatever the reason that prevents the live geometry from
//! coming up, the program must stop and say so rather than churn frame after frame.
#[cfg(test)]
mod tests {
    use super::super::App;

    /// A SECOND ATTEMPT DOES NOT START IF THE FIRST CHANGED NOTHING.
    ///
    /// Between the attempts the document is touched DELIBERATELY — exactly the way the rebuild itself
    /// touches it (it rewrites node names, clears and sets marks). The former guard took the state of
    /// the whole document as its key and therefore let the circle through: the document is different
    /// all right, and there is still nothing to do.
    #[test]
    fn a_brep_attempt_that_changed_nothing_is_not_repeated() {
        let mut app = App::default();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        app.rebuild_if_dirty();

        // THERE IS NO LIVE GEOMETRY AND THERE WILL BE NONE — as with imports after "rebuild
        // everything".
        app.regen.ui_running = true; // a live window: the attempt goes into a thread
        app.live.shapes.clear();
        app.live.ready = false;
        app.live.wait = None;
        app.live.tried_rev = None;

        app.ensure_brep(); // the first attempt is legitimate
        assert!(app.regen.wanted || app.live.wait.is_some(), "setup: the first attempt must happen");

        // THE REBUILD ARRIVED AND GAVE NOTHING: no shapes were added, but it touched the document.
        app.live.wait = None;
        app.regen.wanted = false;
        if let Some(n) = app.project.timeline.first_mut() {
            n.name = format!("{} ", n.name); // that is how topological naming touches it
        }

        app.ensure_brep(); // the second must not start
        assert!(
            !app.regen.wanted,
            "the B-rep preparation went round a second time having changed nothing on the first — in a live window that is the endless flashing of the rebuild window"
        );
    }

    /// A REBUILD RESULT DOES NOT WIPE THE LIVE GEOMETRY THAT APPEARED WHILE IT RAN.
    ///
    /// THAT WAS THE LOCK OF THE ENDLESS CIRCLE, and a measurement in a live window caught it:
    ///
    /// ```text
    /// B-rep preparation started: live shapes 136   <- the imports came back
    /// rebuild result accepted
    /// B-rep preparation started: live shapes 0     <- and they were wiped
    /// ```
    ///
    /// A rebuild TAKES the cache of live shapes with it into the thread and on return lays its own copy
    /// ON TOP. At the moment of sending the cache was empty, so the copy is empty — and everything that
    /// was restored while the thread computed vanished. Then the preparation saw zero shapes again and
    /// asked for another rebuild.
    ///
    /// The same class as with the arrangement: the copy from the thread is stale for everything the
    /// thread DID NOT COMPUTE. The result must be ADDED to the live cache rather than replace it
    /// wholesale.
    #[test]
    fn a_rebuild_result_does_not_wipe_shapes_restored_while_it_ran() {
        let mut app = App::default();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        app.rebuild_if_dirty();
        let body = *app.live.shapes.keys().next().expect("setup: the body has a live shape");

        // THE REBUILD WENT INTO THE THREAD AND TOOK THE CACHE WITH IT (it was empty).
        let saved = app.live.shapes.remove(&body).expect("the shape was taken into the thread");
        let rebuilt = app.project.clone_without_source_data();
        let stamp = app.regen_doc_stamp();

        // WHILE IT COMPUTED, the restoration of the imports brought the live shape back.
        app.live.shapes.insert(body, saved);

        // AND HERE IT COMES BACK — with a cache of its own (an empty one).
        app.finish_regen_checked(stamp, rebuilt, Vec::new(), Vec::new(), Vec::new(), false);

        assert!(
            app.live.shapes.contains_key(&body),
            "the rebuild wiped the live geometry raised while it ran — hence the endless circle of zero shapes, rebuild, zero shapes"
        );
    }

    /// WHILE THE CACHE IS AWAY IN THE WORKER, THE PREPARATION PASSES NO JUDGEMENT ON IT.
    ///
    /// THE LAST LINK OF THE CIRCLE, and a measurement in a live window named that too: "live shapes
    /// 136" alternated with "live shapes 0" and back. A rebuild TAKES the cache of live shapes for the
    /// duration of the computation (otherwise the thread has nothing to work with), and for all that
    /// time the application simply DOES NOT HAVE it. The preparation, asked at that moment, honestly
    /// answers "not one body is raised" — and orders a rebuild that has only just started. The answer
    /// depends on whether a computation is running rather than on what has been done — and the "we have
    /// already tried this" guard misses every other time.
    ///
    /// One must not judge what one does not hold. While the computation runs, the preparation stays
    /// silent.
    #[test]
    fn while_the_cache_is_away_in_the_worker_the_preparation_says_nothing() {
        let mut app = App::default();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        app.rebuild_if_dirty();
        app.regen.ui_running = true;

        app.spawn_regen(); // the cache went into the thread, the application has zero live shapes
        assert!(app.regen.busy.is_some(), "setup: the rebuild must be in flight");
        assert!(app.live.shapes.is_empty(), "setup: the cache must go into the thread");
        app.regen.wanted = false;
        app.live.ready = false;
        app.live.wait = None;
        app.live.tried_rev = None;

        app.ensure_brep();

        assert!(
            !app.regen.wanted,
            "the B-rep preparation judged the live geometry while the cache was away in the thread and ordered a rebuild on top of the running one — that is the endless circle"
        );
    }

    /// "REBUILD EVERYTHING" BRINGS BACK WHAT IT THREW AWAY.
    ///
    /// The command clears the live B-rep of every body. For those built by the timeline it appears
    /// again on the rebuild, and for IMPORTS it does not: their geometry lies in the embedded STEP, and
    /// a separate path raises it. Not calling that path means taking the live geometry of a whole
    /// imported assembly away from a person until the program is restarted, and leaving the preparation
    /// with no hope of ever finishing.
    #[test]
    fn rebuild_everything_asks_the_imports_back() {
        let mut app = App::default();
        super::super::joint_flow::tests::add_part_at(&mut app, 0.0);
        app.rebuild_if_dirty();
        app.regen.ui_running = true;
        let bg_before = app.regen.bg.len();

        app.rebuild_everything_for_test();

        assert!(
            app.regen.bg.len() > bg_before || app.regen.busy.is_some(),
            "rebuild-everything threw away the live B-rep and did not ask for the imports back: their geometry will not come alive until a restart"
        );
        assert!(
            app.import_shapes_asked_for_test(),
            "rebuild-everything did not call the restoration of the imports — the timeline cannot raise them, their geometry is in the embedded STEP"
        );
    }
}
