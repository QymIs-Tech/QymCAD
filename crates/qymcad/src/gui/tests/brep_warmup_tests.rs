//! Reported behaviour: pressing "create a sketch" made the rebuild window flicker back and forth, leaving
//! nothing possible to do.
//!
//! The sketch plane pick calls `ensure_brep` EVERY FRAME. In a live window the rebuild goes into a thread, and
//! the "already tried at this revision" guard was set BEFORE it, against the old revision. The rebuild moved
//! the revision later, the mark stopped matching, and the next frame started it all over again: an endless
//! cycle of the overlay appearing and vanishing. In the headless tests there is no window and the rebuild runs
//! synchronously - which is why the cycle lived only in a real window and no test ever saw it.
use super::{App, JobResult};

/// A body with no live B-rep plus a running window is exactly the reported situation.
fn app_needing_brep() -> App {
    let mut app = App::default();
    let _cube = super::command_flow_tests::build_cube(&mut app);
    app.live.shapes.clear(); // as after opening from a bundle: the geometry is there and there is no live B-rep
    app.live.ready = false;
    app.live.tried_rev = None;
    app.regen.ui_running = true; // the window is live, so the rebuild is asynchronous
    app
}

/// THE MAIN POINT: while a started preparation has not finished, the next frame does NOT start a second one.
#[test]
fn warmup_is_requested_once_not_every_frame() {
    let mut app = app_needing_brep();
    crate::gui::io_jobs::ensure_brep(&mut app.rebuild_ctx());
    assert!(app.regen.wanted, "the first frame asks for a rebuild");
    // the request has been taken up and the overlay is spinning. While it spins, THE GEOMETRY REVISION may
    // move - any background task moves it (and so does the rebuild itself, once it arrives).
    app.regen.wanted = false;
    qymcad_ui_state::invalidate(&mut app.regen);
    crate::gui::io_jobs::ensure_brep(&mut app.rebuild_ctx());
    assert!(!app.regen.wanted, "no second request goes out: the preparation is ALREADY under way - otherwise the overlay flickers");
    qymcad_ui_state::invalidate(&mut app.regen);
    crate::gui::io_jobs::ensure_brep(&mut app.rebuild_ctx());
    assert!(!app.regen.wanted, "nor a third");
}

/// AFTER the rebuild finishes the cache is declared ready, and there are no new requests.
#[test]
fn warmup_settles_when_the_background_rebuild_lands() {
    let mut app = app_needing_brep();
    crate::gui::io_jobs::ensure_brep(&mut app.rebuild_ctx());
    assert!(app.regen.wanted, "the preparation was requested");
    // as the scheduler does it: start the task and wait for its result
    app.regen.wanted = false;
    app.spawn_regen();
    let busy = app.regen.busy.take().expect("the rebuild task was created");
    match busy.rx.recv().expect("the thread reported back") {
        JobResult::Regenerated { stamp, project, shapes, built, errors, cancelled } => app.finish_regen_checked(stamp, *project, shapes, built, errors, cancelled),
        _ => panic!("a rebuild result was expected"),
    }
    assert!(
        app.live.ready,
        "the live B-rep is built and the cache is ready: {} bodies without a shape",
        app.project.timeline.iter().filter_map(|n| n.kind.body()).filter(|b| !app.live.shapes.contains_key(b)).count()
    );
    crate::gui::io_jobs::ensure_brep(&mut app.rebuild_ctx());
    assert!(!app.regen.wanted, "a ready cache asks for nothing more");
}
