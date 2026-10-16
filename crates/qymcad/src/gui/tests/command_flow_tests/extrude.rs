//! Extrudes and cuts through the command layer, and the plane of a sketch picked for them.

use super::*;

/// A regression: an empty part, a sketch, the Extrude button (op=0), Enter. A body must appear.
#[test]
fn empty_part_extrude_flow() {
    let mut app = App::default();
    let si = sketch_rect(&mut app, 0.0, 0.0, 30.0, 30.0);
    app.chosen.sel = Sel::Sketch(si);
    app.feat.op = 0; // the Extrude button
    app.start_feat_cmd(1);
    assert_eq!(app.tools.armed.cmd_kind(), 1, "the command started: {}", app.status);
    if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "height") {
        p.val = 10.0;
        p.txt = "10".into();
    }
    app.apply_feat_cmd();
    assert_eq!(app.tools.armed.cmd_kind(), 0, "the command finished (Enter worked): {}", app.status);
    let v = total_volume(&app);
    assert!((v - 9000.0).abs() < 90.0, "a 30x30x10 body was built, V={v:.0}; status: {}", app.status);
}

/// The root of that fault: a stale "through all" from the previous command does NOT leak into a new Extrude.
#[test]
fn stale_through_extent_does_not_leak() {
    let mut app = App::default();
    let si = sketch_rect(&mut app, 0.0, 0.0, 20.0, 20.0);
    app.tools.cmd.extent = super::super::ExtentMode::Through; // the "previous command" left "through all" behind
    app.chosen.sel = Sel::Sketch(si);
    app.feat.op = 0;
    app.start_feat_cmd(1);
    assert!(matches!(app.tools.cmd.extent, super::super::ExtentMode::Length), "the extent is reset when the command starts");
    if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "height") {
        p.val = 10.0;
        p.txt = "10".into();
    }
    app.apply_feat_cmd();
    let v = total_volume(&app);
    assert!((v - 4000.0).abs() < 40.0, "an ordinary 20x20x10 body rather than a through-all monster: V={v:.0}");
}

/// Reported behaviour: cutting a 5x5 square through a 5 mm thick body left a wall where a hole should be.
/// The same chain runs here on the live kernel: a 20x20x5 plate, a second sketch with a 5x5 square, a cut to a
/// depth of 5 and a cut "through all". In both cases the volume must drop by exactly 5*5*5 = 125 mm^3.
#[test]
fn cut_5x5_through_a_5mm_plate_leaves_no_wall() {
    for (nm, extent, depth) in [("to a depth of 5", super::super::ExtentMode::Length, 5.0), ("through all", super::super::ExtentMode::Through, 5.0)] {
        let mut app = App::default();
        let si = sketch_rect(&mut app, 0.0, 0.0, 20.0, 20.0);
        app.chosen.sel = Sel::Sketch(si);
        app.feat.op = 0;
        app.start_feat_cmd(1);
        if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 5.0;
            p.txt = "5".into();
        }
        app.apply_feat_cmd();
        let base = live_volume(&app);
        assert!((base - 2000.0).abs() < 20.0, "{nm}: a 20x20x5 plate, V={base:.1}");

        // a 5x5 square inside the plate, sketched on the same base plane
        let si2 = sketch_rect(&mut app, 5.0, 5.0, 10.0, 10.0);
        app.chosen.sel = Sel::Sketch(si2);
        app.feat.op = 2; // Cut
        app.start_feat_cmd(1);
        app.tools.cmd.extent = extent;
        if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = depth;
            p.txt = format!("{depth}");
        }
        app.apply_feat_cmd();
        assert_eq!(app.tools.armed.cmd_kind(), 0, "{nm}: the cut finished: {}", app.status);
        let v = live_volume(&app);
        eprintln!("{nm}: V before {base:.1} -> after {v:.1} (removed {:.1}, expected 125)", base - v);
        assert!((base - v - 125.0).abs() < 1.5, "{nm}: {:.1} mm^3 removed instead of 125 - the wall stayed. Status: {}", base - v, app.status);
    }
}

/// Reported behaviour: the choice of a sketch's origin on a face had disappeared - it used to be possible to
/// click where the sketch's zero would sit. The binding is alive, but it snaps to the VERTICES and EDGES of the
/// live B-rep, which is no longer built when a project is opened: there was nothing to snap to, and the origin
/// silently fell back to the default. Picking a sketch plane must bring the B-rep cache up, exactly as the
/// chamfer and the fillet do.
#[test]
fn picking_a_sketch_plane_prepares_the_brep_for_origin_snap() {
    let mut app = App::default();
    let si = sketch_rect(&mut app, 0.0, 0.0, 30.0, 30.0);
    app.chosen.sel = Sel::Sketch(si);
    app.feat.op = 0;
    app.start_feat_cmd(1);
    if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "height") {
        p.val = 10.0;
        p.txt = "10".into();
    }
    app.apply_feat_cmd();
    assert!(!app.project.bodies.is_empty(), "there is a body: {}", app.status);

    // as after OPENING a file: the meshes are there, there is no live B-rep and nothing has tried to build one
    app.live.shapes.clear();
    app.live.ready = false;
    app.live.tried_rev = None;
    app.viewing.mode_3d = true;
    crate::gui::commands::refresh_edges(&mut app.part_ctx());
    assert!(app.live.shapes.is_empty(), "a B-rep is not built for no reason - that is the point of the lazy build");

    // the plane pick for a new sketch is on, so the vertices and edges are needed to bind the origin
    app.tools.picking = Picking::SketchPlane(None);
    crate::gui::commands::refresh_edges(&mut app.part_ctx());
    assert!(!app.live.shapes.is_empty(), "the B-rep is ready for the sketch plane pick - there is something to bind the origin to");

    // and the axis candidates (a body's edge) are not found without a B-rep either
    app.live.shapes.clear();
    app.live.ready = false;
    app.live.tried_rev = None;
    app.tools.picking.clear();
    app.refresh_axis_edges();
    assert!(!app.edges.axes.is_empty(), "the body's straight edges are available as an axis");
}

/// Reported behaviour: a node higher up the timeline was deleted, and a cut driven by another sketch vanished
/// along with two chamfers, leaving a wall where an opening should have been. What is checked here is THE
/// BEHAVIOUR OF DELETION: removing one operation must not silently carry away INDEPENDENT operations further
/// down the timeline, even when their sketch sat on a face of the intermediate body.
#[test]
fn deleting_a_feature_does_not_silently_eat_the_cut_below() {
    let mut app = App::default();
    let si = sketch_rect(&mut app, 0.0, 0.0, 40.0, 40.0);
    app.chosen.sel = Sel::Sketch(si);
    app.feat.op = 0;
    app.start_feat_cmd(1);
    if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "height") {
        p.val = 20.0;
        p.txt = "20".into();
    }
    app.apply_feat_cmd();

    // the intermediate operation (it gets deleted later)
    let si2 = sketch_rect(&mut app, 2.0, 2.0, 10.0, 10.0);
    app.chosen.sel = Sel::Sketch(si2);
    app.feat.op = 2;
    app.start_feat_cmd(1);
    if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "height") {
        p.val = 3.0;
        p.txt = "3".into();
    }
    app.apply_feat_cmd();
    let mid_node = app.project.timeline.last().map(|n| n.id).unwrap();

    // an INDEPENDENT cut further down the timeline: a different sketch in a different place
    let si3 = sketch_rect(&mut app, 25.0, 25.0, 30.0, 30.0);
    app.chosen.sel = Sel::Sketch(si3);
    app.feat.op = 2;
    app.start_feat_cmd(1);
    if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "height") {
        p.val = 20.0;
        p.txt = "20".into();
    }
    app.apply_feat_cmd();
    let cuts_before = app.project.timeline.iter().filter(|n| n.name.starts_with("feat-name-combine")).count();
    let v_before = live_volume(&app);

    let ti = app.project.timeline.iter().position(|n| n.id == mid_node).unwrap();
    crate::gui::commands::delete_feature(&mut app.part_ctx(), ti, false);
    crate::gui::io_jobs::regenerate_now(&mut app.rebuild_ctx());

    let cuts_after = app.project.timeline.iter().filter(|n| n.name.starts_with("feat-name-combine")).count();
    assert_eq!(cuts_after, cuts_before - 1, "exactly one operation was removed, not everything below it");
    // the volume: the 8x8x3=192 pocket came back, the independent 5x5x20=500 cut is still removed
    let v = live_volume(&app);
    assert!((v - (v_before + 192.0)).abs() < 5.0, "only the deleted pocket returned: V={v:.1}, was {v_before:.1}");
}

/// A cut in an EMPTY part gives a clear refusal in the status line, the command stays active and no body appears.
#[test]
fn cut_in_empty_part_gives_message() {
    let mut app = App::default();
    let si = sketch_rect(&mut app, 0.0, 0.0, 20.0, 20.0);
    app.chosen.sel = Sel::Sketch(si);
    app.feat.op = 2; // the Cut button
    app.start_feat_cmd(1);
    app.apply_feat_cmd();
    assert!(app.status.words.contains(&crate::i18n::tr("cmd-cut-needs-body")), "a clear status rather than silence: \"{}\"", app.status);
    assert!(total_volume(&app) < 1e-6, "no body appeared");
}

/// A full flow: extrude a base, then cut a pocket with a second sketch. The volumes are exact.
#[test]
fn extrude_then_cut_flow() {
    let mut app = App::default();
    let si = sketch_rect(&mut app, 0.0, 0.0, 20.0, 20.0);
    app.chosen.sel = Sel::Sketch(si);
    app.feat.op = 0;
    app.start_feat_cmd(1);
    if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "height") {
        p.val = 20.0;
        p.txt = "20".into();
    }
    app.apply_feat_cmd();
    assert!((total_volume(&app) - 8000.0).abs() < 80.0, "a 20-cubed base: {}", app.status);
    // the second sketch: a 10x10 pocket, cut to a depth of 5
    let s2 = sketch_rect(&mut app, 5.0, 5.0, 15.0, 15.0);
    app.chosen.sel = Sel::Sketch(s2);
    app.feat.op = 2;
    app.start_feat_cmd(1);
    assert_eq!(app.tools.armed.cmd_kind(), 1, "the cut started: {}", app.status);
    if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "height") {
        p.val = 5.0;
        p.txt = "5".into();
    }
    app.apply_feat_cmd();
    assert_eq!(app.tools.armed.cmd_kind(), 0, "the cut was applied: {}", app.status);
    // the consumed bodies are hidden; what is measured is the volume of THE RESULT (the last live body)
    let consumed = qymcad_ui_state::consumed_bodies(&app.project);
    let v: f64 = app.live.shapes.iter().filter(|(b, _)| !consumed.contains(b)).map(|(_, s)| s.volume()).sum();
    assert!((v - 7500.0).abs() < 75.0, "the pocket is cut: V={v:.0}; status: {}", app.status);
}

/// THE RULE THAT A PART IS ONE BODY: two extrudes in a row (without entering a part) must give ONE unconsumed
/// body (the second is added by a boolean) rather than two independent ones.
#[test]
fn two_extrudes_make_one_body() {
    let mut app = App::default();
    let si = sketch_rect(&mut app, 0.0, 0.0, 20.0, 20.0);
    app.chosen.sel = Sel::Sketch(si);
    app.feat.op = 0;
    app.start_feat_cmd(1);
    if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "height") {
        p.val = 10.0;
        p.txt = "10".into();
    }
    app.apply_feat_cmd();
    // the second sketch OVERLAPS the first by half, so it becomes a boss
    let s2 = sketch_rect(&mut app, 10.0, 0.0, 30.0, 20.0);
    app.chosen.sel = Sel::Sketch(s2);
    app.feat.op = 0;
    app.start_feat_cmd(1);
    if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "height") {
        p.val = 10.0;
        p.txt = "10".into();
    }
    app.apply_feat_cmd();
    let consumed = qymcad_ui_state::consumed_bodies(&app.project);
    let live: Vec<f64> = app.live.shapes.iter().filter(|(b, _)| !consumed.contains(b)).map(|(_, s)| s.volume()).collect();
    assert_eq!(live.len(), 1, "a part is ONE live body, not {} (bodies are breeding); status: {}", live.len(), app.status);
    assert!((live[0] - 6000.0).abs() < 60.0, "the union of two 20x20x10 blocks overlapping by 10x20x10 is 6000, V={:.0}", live[0]);
}
