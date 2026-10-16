//! Features edited and deleted, the thread by its standard, and Esc stepping back out of a tool.

use super::*;

/// A fillet BY BUTTON: pick an edge, set a radius, press Enter (without entering the part).
#[test]
fn fillet_button_flow() {
    let mut app = App::default();
    let cube = build_cube(&mut app);
    let eid = app.project.regen_edges.get(&cube).and_then(|es| es.iter().find(|e| e.a[0].abs() < 1e-6 && e.a[1].abs() < 1e-6 && (e.a[2] - e.b[2]).abs() > 1.0)).map(|e| e.id).expect("a vertical edge");
    app.start_feat_cmd(4);
    assert_eq!(app.tools.armed.cmd_kind(), 4, "the fillet started: {}", app.status);
    app.tools.gsel.edges.insert(eid);
    app.edges.body = Some(cube);
    if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "radius") {
        p.val = 4.0;
        p.txt = "4".into();
    }
    app.apply_feat_cmd();
    assert_eq!(app.tools.armed.cmd_kind(), 0, "the fillet was applied: {}", app.status);
    let consumed = qymcad_ui_state::consumed_bodies(&app.project);
    let v: f64 = app.live.shapes.iter().filter(|(b, _)| !consumed.contains(b)).map(|(_, s)| s.volume()).sum();
    let exp = 8000.0 - (16.0 - std::f64::consts::PI * 4.0) * 20.0;
    assert!((v - exp).abs() < 10.0, "a fillet of r4: V={v:.1}, expected {exp:.1}");
}

/// EDITING A FILLET: the picked edges are LIT (the edge selection is restored, and the edges come from THE SOURCE BODY).
#[test]
fn edit_fillet_shows_selected_edges() {
    let mut app = App::default();
    let cube = build_cube(&mut app);
    let eid = app.project.regen_edges.get(&cube).and_then(|es| es.iter().find(|e| (e.a[2] - e.b[2]).abs() > 10.0)).map(|e| e.id).expect("an edge");
    app.start_feat_cmd(4);
    app.tools.gsel.edges.insert(eid);
    app.edges.body = Some(cube);
    if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "radius") {
        p.val = 3.0;
        p.txt = "3".into();
    }
    app.apply_feat_cmd();
    let fid = app.project.timeline.iter().find(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::Fillet { .. })).map(|n| n.id).expect("the fillet's node");
    crate::gui::commands::start_feat_cmd_edit(&mut app.part_ctx(), fid);
    crate::gui::commands::refresh_edges(&mut app.part_ctx()); // a UI frame: the edges must stay those of THE SOURCE and the selection must stay alive
    assert_eq!(app.edges.body, Some(cube), "the edges aim at the SOURCE body, not at the fillet's output");
    assert!(app.tools.gsel.edges.contains(&eid), "the edges picked earlier are LIT while editing");
}

/// EDITING A FEATURE (a double click, change the height, Enter): it updates IN PLACE and no nodes are bred.
#[test]
fn edit_feature_height_flow() {
    let mut app = App::default();
    let _cube = build_cube(&mut app);
    let nodes_before = app.project.timeline.len();
    let fid = app.project.timeline.iter().find(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::Extrude { .. })).map(|n| n.id).expect("the extrude's node");
    crate::gui::commands::start_feat_cmd_edit(&mut app.part_ctx(), fid);
    assert!(app.tools.cmd.edit.is_some(), "the edit mode is open: {}", app.status);
    if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "height") {
        p.val = 30.0;
        p.txt = "30".into();
    }
    app.apply_feat_cmd();
    assert_eq!(app.project.timeline.len(), nodes_before, "editing breeds NO nodes");
    let consumed = qymcad_ui_state::consumed_bodies(&app.project);
    let v: f64 = app.live.shapes.iter().filter(|(b, _)| !consumed.contains(b)).map(|(_, s)| s.volume()).sum();
    assert!((v - 12000.0).abs() < 120.0, "the height went 20 -> 30: V={v:.0}; status: {}", app.status);
}

/// Deleting an operation from the tree: the cut is gone and the block is back; deleting the base leaves nothing and no orphans.
#[test]
fn delete_feature_flow() {
    let mut app = App::default();
    let _cube = build_cube(&mut app);
    let s2 = sketch_rect(&mut app, 5.0, 5.0, 15.0, 15.0);
    app.chosen.sel = Sel::Sketch(s2);
    app.feat.op = 2;
    app.start_feat_cmd(1);
    if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "height") {
        p.val = 5.0;
        p.txt = "5".into();
    }
    app.apply_feat_cmd();
    let cut_ti = app.project.timeline.iter().position(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::Combine { .. })).expect("the cut's node");
    crate::gui::commands::delete_feature(&mut app.part_ctx(), cut_ti, false);
    qymcad_ui_state::regenerate_all(&mut app.rebuild_ctx());
    let consumed = qymcad_ui_state::consumed_bodies(&app.project);
    let live: Vec<f64> = app.live.shapes.iter().filter(|(b, _)| !consumed.contains(b)).map(|(_, s)| s.volume()).collect();
    assert_eq!(live.len(), 1, "after the cut is deleted there is one body");
    assert!((live[0] - 8000.0).abs() < 80.0, "the block is restored: V={:.0}", live[0]);
}

/// EDITING THE SKETCH of a built body THROUGH THE GUI CYCLE (enter, move the points, finish):
/// the body must rebuild.
#[test]
fn edit_sketch_of_built_body_flow() {
    let mut app = App::default();
    let _cube = build_cube(&mut app);
    let si = 0; // the block's base sketch
    app.enter_sketch_edit(si);
    for pt in &mut app.project.sketches[si].points {
        if (pt.x - 20.0).abs() < 1e-9 {
            pt.x = 30.0;
        }
    }
    app.project.solve_sketch(si);
    app.project.regen_sketch(si);
    app.finish_sketch_edit();
    let consumed = qymcad_ui_state::consumed_bodies(&app.project);
    let v: f64 = app.live.shapes.iter().filter(|(b, _)| !consumed.contains(b)).map(|(_, s)| s.volume()).sum();
    assert!((v - 12000.0).abs() < 120.0, "stretched 20 -> 30 through the GUI cycle: V={v:.0}; status: {}", app.status);
}

/// Reported behaviour: the thread tool did not work, and what was wanted was easy generation of threads and
/// augers. THE COMMAND is now specified the professional way - a standard and a size - while the depth, the
/// diameters and the profile are computed by the model core. The bar used to hold a bare angle and a bare
/// thread depth as plain numbers, left to be guessed.
#[test]
fn thread_command_is_standard_driven_and_auger_switches_fields() {
    use qymcad_core::feature::FeatureKind;
    let mut app = App::default();
    let cube = build_cube(&mut app);
    qymcad_ui_state::select_body(&mut app.project, &mut app.chosen.sel, &mut app.viewing.view, cube);
    crate::gui::commands::start_thread_cmd(&mut app.part_ctx());
    assert_eq!(app.tools.armed.cmd_kind(), 24, "the thread command started: {}", app.status);

    // the fields are the size and the fit, NOT the angle and the depth
    let keys: Vec<String> = app.tools.cmd.params.iter().map(|p| p.key.to_string()).collect();
    assert!(keys.iter().any(|k| k == "nominal") && keys.iter().any(|k| k == "pitch") && keys.iter().any(|k| k == "fit"), "the thread's fields: {keys:?}");
    assert!(!keys.iter().any(|k| k == "angle") && !keys.iter().any(|k| k == "depth"), "the angle and the thread depth are no longer typed by hand: {keys:?}");
    assert!((qymcad_ui_state::cmd_val(&app.tools.cmd, "pitch") - 0.0).abs() < 1e-9, "a default pitch of 0 means the standard coarse one");

    // switching to the AUGER changes the set of fields to the ribbon's
    app.params.thread.auger = true;
    qymcad_ui_state::set_thread_params(&mut app.tools.cmd, app.params.thread);
    let keys: Vec<String> = app.tools.cmd.params.iter().map(|p| p.key.to_string()).collect();
    assert!(keys.iter().any(|k| k == "outer") && keys.iter().any(|k| k == "thickness") && keys.iter().any(|k| k == "edge_r"), "the auger's fields: {keys:?}");

    // applying the auger puts an Auger feature with its own specification into the timeline
    app.params.thread.auger = true;
    app.params.thread.src = Some(cube);
    app.params.thread.edge = 1;
    app.params.thread.radius = 5.0;
    let body = crate::gui::commands::apply_thread_cmd(&app.tools.cmd, &mut app.project, &mut app.status, app.params.thread).expect("the auger's node was created");
    match app.project.timeline.iter().find(|n| n.id == body).map(|n| &n.kind) {
        Some(FeatureKind::Auger { spec, length, .. }) => {
            assert!(spec.outer_d > spec.shaft_d, "the ribbon is wider than the shaft: {:.1} against a shaft of {:.1}", spec.outer_d, spec.shaft_d);
            assert!(*length > 0.0 && spec.pitch > 0.0);
        }
        other => panic!("an auger feature was expected, got {other:?}"),
    }
}

/// A thread in the timeline carries THE STANDARD and the size, and the geometry is computed from them (M10 gives a pitch of 1.5).
#[test]
fn thread_feature_carries_standard_and_derives_geometry() {
    use qymcad_core::feature::FeatureKind;
    let mut app = App::default();
    let cube = build_cube(&mut app);
    qymcad_ui_state::select_body(&mut app.project, &mut app.chosen.sel, &mut app.viewing.view, cube);
    crate::gui::commands::start_thread_cmd(&mut app.part_ctx());
    app.params.thread.src = Some(cube);
    app.params.thread.edge = 1;
    app.params.thread.radius = 5.0;
    app.params.thread.form = 0; // metric ISO
    if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "nominal") {
        p.val = 10.0;
    }
    let body = crate::gui::commands::apply_thread_cmd(&app.tools.cmd, &mut app.project, &mut app.status, app.params.thread).expect("the thread's node was created");
    match app.project.timeline.iter().find(|n| n.id == body).map(|n| &n.kind) {
        Some(FeatureKind::Thread { spec, .. }) => {
            assert_eq!(spec.standard, qymcad_core::thread::ThreadStandard::MetricIso);
            let g = spec.geometry();
            assert!((g.pitch - 1.5).abs() < 1e-9, "M10 gives a coarse pitch of 1.5, got {}", g.pitch);
            assert!((g.pitch_d - 9.026).abs() < 0.01 && (g.minor_d - 8.160).abs() < 0.01, "the diameters follow ISO 68-1: d2={:.3} d3={:.3}", g.pitch_d, g.minor_d);
            assert!(g.groove.len() >= 3, "the groove's profile was computed");
        }
        other => panic!("a thread feature was expected, got {other:?}"),
    }
}

/// Reported behaviour: Revolve offered only X or Y and no axis of one's own, and a sphere could not be made
/// from a drawn circle. The core could do all of it from the start: an axis from ANY sketch line, a slanted
/// axis, a sphere from a half-disc (see the `matrix_revolve_axis` matrix). The limitation sat in the UI: the
/// list of axis candidates filtered the lines by `construction`, so an ordinary drawn line was never offered
/// and only X and Y were left in the bar.
#[test]
fn any_sketch_line_is_offered_as_revolve_axis() {
    let mut app = App::default();
    let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    // an ordinary (NOT construction) line - that is exactly what gets drawn for an axis
    let plain = app.project.add_line_entity(si, 0.0, -50.0, 0.0, 50.0, qymcad_core::feature::Purpose::Real);
    app.project.regen_sketch(si);
    let cands = qymcad_ui_state::profile_axis_lines(&app.project, si);
    assert!(cands.contains(&plain), "an ordinary line must be an axis candidate: {cands:?}");
    assert_eq!(qymcad_ui_state::axis_line_label(&app.project, si, plain, 1), crate::i18n::tr1("sk-line-n", "n", "1"), "the label says honestly that this is an ordinary line");

    // a construction line is a candidate too, and comes FIRST (it is drawn precisely to be an axis)
    let constr = app.project.add_line_entity(si, -50.0, 0.0, 50.0, 0.0, qymcad_core::feature::Purpose::Construction);
    app.project.regen_sketch(si);
    let cands = qymcad_ui_state::profile_axis_lines(&app.project, si);
    assert_eq!(cands.first(), Some(&constr), "the construction line is offered first: {cands:?}");
    assert!(cands.contains(&plain), "the ordinary one has not gone anywhere");
    assert_eq!(qymcad_ui_state::axis_line_label(&app.project, si, constr, 1), crate::i18n::tr1("sk-axis-line-n", "n", "1"));
}

/// Reported behaviour: in a sketch, Esc with the measure tool active finished the sketch straight away, and
/// the same happened with elements selected. ESC IS A LADDER: it clears the innermost state and only at the
/// bottom leaves the sketch. The measure tool and the selection were not on the ladder at all, so the very
/// first Esc fell through to `finish_sketch_edit`.
#[test]
fn escape_cancels_tool_then_selection_and_stops() {
    let mut app = App::default();
    let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    app.project.add_rect_entity(si, 0.0, 0.0, 20.0, 20.0, qymcad_core::feature::Purpose::Real);
    app.project.regen_sketch(si);
    assert!(app.sketch_ses.editing.is_some(), "the sketch is open");

    // 1) the measure tool is active: Esc returns to Select and the sketch STAYS open
    app.tools.armed = qymcad_ui_state::Armed::Measure;
    app.tools.measure.pts.push(qymcad_core::geom::Point2::new(1.0, 1.0));
    app.on_escape();
    assert!(!app.tools.armed.measuring() && app.tools.measure.pts.is_empty(), "the measure tool is dropped: {}", app.status);
    assert!(app.sketch_ses.editing.is_some(), "the sketch was NOT closed by the very first Esc");

    // 2) there are selected elements: Esc clears the selection and the sketch is still open
    let ent = app.project.sketches[si].entities.first().map(|e| e.id).expect("an entity");
    app.tools.sel_sk.items.push((1, ent));
    app.on_escape();
    assert!(app.tools.sel_sk.items.is_empty(), "the selection is cleared: {}", app.status);
    assert!(app.sketch_ses.editing.is_some(), "and the sketch is still open");

    // 3) nothing is active: the ladder has run out, and Esc STOPS THERE.
    //
    // It used to close the sketch on this rung, and that was the whole complaint: Esc gives things
    // back - the drawing under way, the tool, the selection - while finishing a context gives nothing
    // back and is the same act as leaving a part. A person tapping Esc to get rid of a tool lost the
    // sketch on the tap after the last one. Leaving is Ctrl+Enter, checked in
    // `the_sketch_is_left_by_ctrl_enter.rs`.
    app.on_escape();
    assert!(app.sketch_ses.editing.is_some(), "at the bottom of the ladder Esc must do nothing, not close the sketch");
}

/// The same ladder for the drawing and dimension tools - they already worked, but a regression is caught by
/// the same test (the order of the rungs matters).
#[test]
fn escape_ladder_covers_draw_and_dim_tools() {
    let mut app = App::default();
    let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    app.project.add_rect_entity(si, 0.0, 0.0, 20.0, 20.0, qymcad_core::feature::Purpose::Real);
    app.project.regen_sketch(si);

    qymcad_ui_state::set_dim_tool(&mut qymcad_ui_state::tools_of!(app), &mut app.viewing.mode_3d, &app.project, app.chosen.sel, app.sketch_ses, &mut app.status, 1); // the distance dimension
    app.on_escape();
    assert_eq!(app.tools.armed.dim_kind(), 0, "the dimension tool is dropped");
    assert!(app.sketch_ses.editing.is_some(), "the sketch is open");

    app.tools.armed = qymcad_ui_state::Armed::Draw(2); // a drawing tool
    app.tools.tool.pts.push(qymcad_core::geom::Point2::new(0.0, 0.0));
    app.on_escape();
    assert!(app.tools.tool.pts.is_empty() && app.tools.armed.draw_kind() == 2, "the first Esc aborts the UNFINISHED construction");
    app.on_escape();
    assert_eq!(app.tools.armed.draw_kind(), 0, "the second Esc leaves the tool for Select");
    assert!(app.sketch_ses.editing.is_some(), "and only after that may the sketch be closed");
}
