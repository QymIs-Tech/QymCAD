//! Reported behaviour: an assembly was created with a part in it, "create a sketch" was pressed, and the
//! rebuild window flickered back and forth leaving nothing possible to do. What runs here is A REAL frame
//! loop: the plane pick calls `refresh_edges` every frame while the scheduler starts the deferred rebuild and
//! takes its result. How many rebuilds start over 12 frames is counted: the right answer is ONE.
use super::{App, BgKind, JobResult, Picking, Sel};

/// One frame of the scheduler: if a rebuild was requested and the queue is free, start it and wait.
/// This is what `tick_async` does in a live window, only without egui.
fn pump_frame(app: &mut App, regens: &mut usize) {
    crate::gui::commands::refresh_edges(&mut app.part_ctx()); // in a live window this call happens EVERY frame
    if app.regen.wanted && app.regen.busy.is_none() {
        app.regen.wanted = false;
        app.spawn_regen();
        *regens += 1;
    }
    if let Some(busy) = app.regen.busy.take().filter(|b| b.kind == BgKind::Regen) {
        match busy.rx.recv().expect("the rebuild thread reported back") {
            JobResult::Regenerated { stamp, project, shapes, built, errors, cancelled } => app.finish_regen_checked(stamp, *project, shapes, built, errors, cancelled),
            _ => panic!("a rebuild result was expected"),
        }
    }
}

#[test]
fn picking_a_sketch_plane_rebuilds_once_not_every_frame() {
    let mut app = App::default();
    // a part inside an assembly - the reported context
    let asm = app.project.add_assembly("Assembly");
    app.enter_component(asm);
    let part = app.project.add_part("Part");
    app.enter_component(part);
    let si = {
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, 0.0, 0.0, 30.0, 20.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        si
    };
    app.chosen.sel = Sel::Sketch(si);
    app.feat.op = 0;
    app.start_feat_cmd(1);
    if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "height") {
        p.val = 10.0;
        p.txt = "10".into();
    }
    app.apply_feat_cmd();
    assert!(!app.project.bodies.is_empty(), "the part's body was built: {}", app.status);

    // IN A REAL ASSEMBLY there are bodies that never get a live B-rep at all (an import waiting for its
    // embedded STEP to be parsed, a node that does not build). They are exactly why the preparation could not
    // declare itself finished and went round again every frame. The same kind of failing node is modelled here.
    let empty_si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    app.finish_sketch_edit();
    let empty_sid = app.project.sketches[empty_si].id;
    let stuck = app.project.add_extrude_multi(empty_sid, Vec::new(), 5.0, qymcad_core::feature::Reach::Forward, 0.0, Vec::new());
    let _ = app.project.finish_base_body(stuck, 1);

    // as after OPENING a file: the geometry from the bundle is there and there is no live B-rep
    app.live.shapes.clear();
    app.live.ready = false;
    app.live.tried_rev = None;
    app.regen.ui_running = true; // A LIVE WINDOW: the rebuild is asynchronous, and that is where the cycle lived
    app.viewing.mode_3d = true;
    app.tools.picking = Picking::SketchPlane(None); // "create a sketch" was pressed

    let mut regens = 0usize;
    for _ in 0..12 {
        pump_frame(&mut app, &mut regens);
    }
    assert_eq!(regens, 1, "the B-rep preparation starts ONCE rather than every frame (which is what made the overlay flicker)");
    assert!(!app.live.shapes.is_empty(), "the live B-rep is built - there is something to bind the sketch's origin to");
    // The "ready" flag stays FALSE, and that is the truth: one body never built. What matters is that a false
    // flag no longer drives a fresh attempt every frame - the next one comes when the geometry changes and
    // trying makes sense again.
    assert!(!app.live.ready, "the cache is honestly not declared ready while a body has no B-rep");
    for _ in 0..6 {
        pump_frame(&mut app, &mut regens);
    }
    assert_eq!(regens, 1, "and no new rebuilds follow - the cycle does not come back");
}
