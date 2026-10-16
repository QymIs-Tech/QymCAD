//! A shaft: threads and augers, the axis of a revolve, datum axes, the section through a thread, deferred rebuilds.

use super::*;

/// Reported behaviour: with several lines in a sketch there was no way to choose the right one - the
/// half-sketcher did not open and the only offer was a drop-down of numbered lines; and the "pick an axis in
/// 3D" button would not take a datum axis. A revolve axis is chosen BY A CLICK: on a line in the flat sketch
/// view, or on a datum axis in 3D.
#[test]
fn revolve_axis_is_picked_by_clicking_not_from_a_dropdown() {
    let mut app = App::default();
    let si = sketch_rect(&mut app, 0.0, 0.0, 20.0, 20.0);
    // two centrelines - they cannot be told apart by number, so the choice has to be a click
    let l1 = app.project.add_line_entity(si, -10.0, -30.0, -10.0, 30.0, qymcad_core::feature::Purpose::Construction);
    let l2 = app.project.add_line_entity(si, 40.0, -30.0, 40.0, 30.0, qymcad_core::feature::Purpose::Construction);
    app.project.regen_sketch(si);
    app.chosen.sel = Sel::Sketch(si);
    app.feat.op = 0;
    app.start_feat_cmd(3);
    assert_eq!(app.tools.armed.cmd_kind(), 3, "the revolve command started: {}", app.status);
    let cands = qymcad_ui_state::profile_axis_lines(&app.project, si);
    assert!(cands.len() >= 2, "the sketch holds several candidate lines");

    use egui::{Pos2, Rect, Vec2};
    use qymcad_core::geom::Point2;
    let rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0));

    // THE LINE IS CHOSEN BY A REAL CLICK on the line in the flat half-sketcher. There used to be a check of
    // flags alone here, and it missed exactly the fault that was reported: the search returned the line's END
    // POINTS, they were compared against the list of LINES, and there was never a match - the sketch opened
    // flat and the clicks selected nothing.
    app.params.rev.pick_line = true;
    app.viewing.mode_3d = false;
    app.viewing.view.center = Vec2::ZERO;
    app.viewing.view.scale = 8.0;
    for (want, x) in [(l1, -10.0_f64), (l2, 40.0)] {
        let pos = (qymcad_ui_state::Sheet { view: app.viewing.view, rect }).at(Point2::new(x, 5.0));
        let got = crate::gui::pick::nearest_line_id(&app.pick_ctx(), rect, pos, si, &cands);
        assert_eq!(got, Some(want), "a click on the line at x={x} picks THAT line (got {got:?})");
        assert!(cands.contains(&got.unwrap()), "what was picked is a LINE from the candidates, not one of its ends");
    }
    // a click into empty space picks nothing (otherwise the axis would jump on a miss)
    assert_eq!(
        crate::gui::pick::nearest_line_id(&app.pick_ctx(), rect, (qymcad_ui_state::Sheet { view: app.viewing.view, rect }).at(Point2::new(150.0, 150.0)), si, &cands),
        None,
        "a miss picks no line"
    );

    // PICKING AN AXIS IN 3D: a datum axis built inside a part must both be drawn and be catchable by a click.
    app.params.rev.pick_line = false;
    app.params.rev.pick_axis = true;
    app.viewing.mode_3d = true; // exactly what the button does: the candidates are hit-tested in 3D only
    let ax = app.project.add_datum_axis(qymcad_core::model::DatumAxis::manual("Axis 1", [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]));
    assert!(qymcad_ui_state::datum_render_transform(&app.painting(), ax).is_some(), "the datum axis is visible in its own context - otherwise there is nothing to click");
    let basis = app.viewing.cam.basis();
    let (s3, e3) = super::super::axis_segment([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 45.0);
    let scr = qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect, basis: &basis };
    let (sa, sb) = (scr.at(s3).0, scr.at(e3).0);
    assert!(sa.distance(sb) > 20.0, "the axis projects into a segment rather than a point - there is something to hit");
    let mid = Pos2::new(0.5 * (sa.x + sb.x), 0.5 * (sa.y + sb.y));
    assert!(matches!(crate::gui::pick::pick_axis_at(&app.painting(), rect, mid), Some(super::super::AxisHit::Datum(id)) if id == ax), "a click on the datum axis picks it");
    assert!(crate::gui::axis_ref_world(&app.active_path, &app.edges, &app.live, &app.project, super::super::AxisHit::Datum(ax)).is_some(), "the datum axis resolves into a revolve axis");
}

/// Reported behaviour: the holes in a section were still there after the profile had been fixed and the
/// program restarted. The file stores FINISHED meshes and recomputes nothing on opening, so a part built by an
/// older version of the kernel stays as it was. Edit -> Rebuild everything must mark THE WHOLE timeline dirty,
/// throw away the live B-rep and go into a BACKGROUND rebuild (rather than a modal that dims the screen).
#[test]
fn rebuild_everything_marks_the_whole_timeline_and_goes_background() {
    let mut app = App::default();
    let si = sketch_rect(&mut app, 0.0, 0.0, 20.0, 20.0);
    app.chosen.sel = Sel::Sketch(si);
    app.feat.op = 0;
    app.start_feat_cmd(1);
    app.apply_feat_cmd();
    assert!(!app.project.timeline.is_empty(), "the timeline has something to rebuild: {}", app.status);
    for n in app.project.timeline.iter_mut() {
        n.dirty = false; // as after opening a file: everything is clean and the meshes come from the bundle
    }
    app.rebuild_everything();
    assert!(app.project.timeline.iter().all(|n| n.dirty), "the rebuild marks THE WHOLE timeline dirty");
    assert!(app.live.shapes.is_empty(), "the live B-rep is cleared - the features will not land on the old faces");
    assert!(matches!(&app.regen.busy, Some(b) if matches!(b.kind, BgKind::Regen)), "the rebuild went into the background");
}

/// Reported behaviour: a datum axis built INSIDE a part could not be picked from a sketch. The same click-pick,
/// but the axis belongs to the PART rather than the root: a datum's visibility depends on the context
/// (`datum_render_transform`), and had that gate cut off the part's own axis, the "pick an axis in 3D" button
/// would silently find nothing.
#[test]
fn datum_axis_built_inside_a_part_is_pickable_there() {
    use egui::{Pos2, Rect};
    let mut app = App::default();
    let part = app.project.add_part("Part with an axis");
    app.enter_component(part); // as a double click on a part in the tree does
    let ax = app.project.add_datum_axis(qymcad_core::model::DatumAxis::manual("Axis 1", [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]));
    assert_eq!(qymcad_ui_state::current_ctx_id(&app.active_path, &app.project), part, "we are inside the part");
    assert!(qymcad_ui_state::datum_render_transform(&app.painting(), ax).is_some(), "the part's own axis is visible from inside the part");

    app.viewing.mode_3d = true;
    app.tools.armed = qymcad_ui_state::Armed::Command(3);
    app.params.rev.pick_axis = true;
    let rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0));
    let basis = app.viewing.cam.basis();
    let (s3, e3) = super::super::axis_segment([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 45.0);
    let scr = qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect, basis: &basis };
    let (sa, sb) = (scr.at(s3).0, scr.at(e3).0);
    let mid = Pos2::new(0.5 * (sa.x + sb.x), 0.5 * (sa.y + sb.y));
    assert!(matches!(crate::gui::pick::pick_axis_at(&app.painting(), rect, mid), Some(super::super::AxisHit::Datum(id)) if id == ax), "the part's axis is caught by a click inside it");
    // and THE CLICK really carries it into the command's parameter: the click-handling branch used to sit in the
    // 2D half of the viewport while the candidates are hit-tested in 3D only - the press did nothing at all
    assert!(app.rev_axis_pick_click(rect, mid), "the click on the axis was accepted");
    assert_eq!(app.params.rev.axis_datum, ax, "the revolve axis is the datum axis that was clicked");
    assert!(!app.params.rev.pick_axis, "the pick sub-mode closed after the choice");
    // a miss neither clears the axis already chosen nor leaves the mode silently
    app.params.rev.pick_axis = true;
    assert!(!app.rev_axis_pick_click(rect, Pos2::new(rect.max.x - 2.0, rect.max.y - 2.0)), "a miss picks no axis");
    assert!(app.params.rev.pick_axis, "after a miss the axis pick stays on");

    // and from THE ROOT (outside the part) it is neither shown nor caught - otherwise another part's geometry would litter the assembly
    let root = app.project.root;
    app.enter_component(root);
    assert!(qymcad_ui_state::datum_render_transform(&app.painting(), ax).is_none(), "the part's axis does not stick out into the assembly");
}

/// Reported behaviour: a section through a threaded part was filled only in the smooth part and empty in the
/// thread. What is counted here is WHAT ACTUALLY GOES TO THE DRAWING: the cap's amber vertices by bands of
/// height. The cap must be present in the threaded zone too, not only where the part is smooth.
#[test]
fn section_cap_covers_the_threaded_zone_too() {
    let mut app = App::default();
    let (body, eid) = build_shaft(&mut app, 20.0, 60.0);
    qymcad_ui_state::select_body(&mut app.project, &mut app.chosen.sel, &mut app.viewing.view, body);
    crate::gui::commands::start_thread_cmd(&mut app.part_ctx());
    app.params.thread.src = Some(body);
    app.params.thread.edge = eid;
    app.params.thread.radius = 10.0;
    app.params.thread.axis = ([0.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
    for (k, v) in [("nominal", 20.0), ("pitch", 2.5), ("length", 30.0), ("fit", 0.2), ("lead_in", 0.0), ("lead_out", 0.0)] {
        if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == k) {
            p.val = v;
            p.txt = format!("{v}");
        }
    }
    app.apply_feat_cmd();
    assert_eq!(app.tools.armed.cmd_kind(), 0, "the thread was built: {}", app.status);

    // a section ALONG the axis: a plane through the axis with a +Y normal
    app.side.section.plane = Some(([0.0, 0.0, 0.0], [0.0, 1.0, 0.0]));
    let (verts, looks) = crate::gui::render_scene::gpu_scene_flat(&app.painting());
    // THE CAPS ARE KNOWN BY THEIR LOOK, not by a colour baked into the vertices: the colour lives in the
    // look table now, and state 3 is the cap of a section.
    let cap: Vec<&qymcad_ui_state::GpuVert> = verts.iter().filter(|v| looks.get(v.body as usize).is_some_and(|l| l.state == qymcad_ui_state::LOOK_CAP)).collect();
    let _in_band = |a: f64, b: f64| cap.iter().filter(|v| (v.pos[2] as f64) >= a && (v.pos[2] as f64) <= b).count();
    eprintln!("cap vertices in the scene: {}", cap.len());
    // THE CAP MUST cover the whole outline of the cut. It is measured by area rather than by vertices in a
    // band: triangles can be long and their vertices may miss a narrow band entirely.
    let consumed = qymcad_ui_state::consumed_bodies(&app.project);
    for (mi, m) in app.project.bodies.iter().map(|b| &b.mesh).enumerate() {
        let live = app.project.timeline.iter().any(|n| n.kind.body().map(|b| app.project.mesh_index(b) == Some(mi) && !consumed.contains(&b)).unwrap_or(false));
        if !live {
            continue;
        }
        let tris = qymcad_core::geom::mesh_section_cap(m, [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        let cap_area: f64 = tris
            .iter()
            .map(|t| {
                let (u, v) = ([t[1].x - t[0].x, t[1].y - t[0].y, t[1].z - t[0].z], [t[2].x - t[0].x, t[2].y - t[0].y, t[2].z - t[0].z]);
                let c = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
                (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt() * 0.5
            })
            .sum();
        // the area of the cut's own outline (by loops)
        let mut segs = Vec::new();
        for i in 0..m.tris.len() {
            let t = m.triangle(i);
            let mut hits = Vec::new();
            for k in 0..3 {
                let (a, b) = (t[k], t[(k + 1) % 3]);
                if (a.y > 0.0) != (b.y > 0.0) {
                    let u = a.y / (a.y - b.y);
                    hits.push(qymcad_core::geom::Point2::new(a.x + (b.x - a.x) * u, a.z + (b.z - a.z) * u));
                }
            }
            if hits.len() == 2 && hits[0].dist(hits[1]) > 1e-12 {
                segs.push((hits[0], hits[1]));
            }
        }
        let loops = qymcad_core::geom::stitch_segments(segs, 1e-4);
        let loop_area: f64 = loops
            .iter()
            .map(|l| {
                let p = &l.points;
                let n = p.len();
                (0..n).map(|i| p[i].x * p[(i + 1) % n].y - p[(i + 1) % n].x * p[i].y).sum::<f64>().abs() * 0.5
            })
            .sum();
        eprintln!(
            "mesh {mi}: {} loops, outline area {loop_area:.1}, cap of {} triangles with area {cap_area:.1} ({:.1}%)",
            loops.len(),
            tris.len(),
            if loop_area > 0.0 { cap_area / loop_area * 100.0 } else { 0.0 }
        );
        assert!(cap_area > 0.9 * loop_area, "the cap covers only {:.1}% of the cut's outline", cap_area / loop_area * 100.0);
    }
    assert!(!cap.is_empty(), "the cut's cap reached the scene");
}

/// Reported behaviour: threads took long to build and the system put up a "not responding" window. A HEAVY
/// REBUILD GOES INTO A THREAD. In a running window `regenerate_all` only raises a flag while a worker does the
/// work under an indicator; in the headless tests there is no window and the rebuild happens at once -
/// otherwise the result would not be available on the test's next line.
#[test]
fn regeneration_is_deferred_only_when_the_window_is_running() {
    let mut app = App::default();
    let (body, _eid) = build_shaft(&mut app, 20.0, 40.0);
    let v0 = live_volume(&app);
    assert!(v0 > 0.0, "the shaft is built at once: with no window the rebuild is synchronous");

    // the window has "started", so the rebuild is now deferred by a frame and goes into a thread
    app.regen.ui_running = true;
    app.project.mark_node_dirty(body);
    qymcad_ui_state::regenerate_all(&mut app.rebuild_ctx());
    assert!(app.regen.wanted, "the rebuild was queued rather than done on the spot");
    assert!(app.regen.busy.is_none(), "the work itself starts a frame later - the indicator is shown first");

    // as soon as the window is back in headless mode, the work happens immediately again
    app.regen.ui_running = false;
    app.regen.wanted = false;
    qymcad_ui_state::regenerate_all(&mut app.rebuild_ctx());
    assert!(!app.regen.wanted, "with no window the rebuild runs at once");
    assert!(live_volume(&app) > 0.0, "the model is there");
}

/// Reported behaviour: the thread did not work at all, its parameters changed nothing, and the augers did not
/// work either. AN END-TO-END flow of the tool on a REAL shaft rather than on substituted numbers: the earlier
/// GUI tests only checked that a node landed in the timeline and never built any real geometry.
#[test]
fn thread_command_actually_cuts_the_shaft() {
    let (removed, app) = thread_on_shaft(30.0, 60.0, 0, &[("nominal", 30.0), ("pitch", 3.5), ("length", 40.0), ("fit", 0.0), ("lead_in", 0.0), ("lead_out", 0.0)], false);
    let g = qymcad_core::thread::ThreadSpec { standard: qymcad_core::thread::ThreadStandard::MetricIso, nominal_d: 30.0, pitch: 3.5, ..Default::default() }.geometry();
    let ring = std::f64::consts::PI * (15.0_f64.powi(2) - (15.0 - g.depth).powi(2)) * 40.0;
    eprintln!("M30x3.5 over 40 mm through the GUI: {removed:.0} mm^3 removed, the ring is {ring:.0}");
    assert!(removed > 0.25 * ring && removed < 0.9 * ring, "{removed:.0} mm^3 removed against a ring of {ring:.0}; status: {}", app.status);
}

/// THE PARAMETERS MATTER: the pitch, the fit, the length and the crest radius each change the result. There
/// was no rounding in the command bar at all - the profile was always taken from the standard, whatever was done.
#[test]
fn thread_parameters_change_the_result() {
    let base = [("nominal", 30.0), ("pitch", 3.5), ("length", 40.0), ("fit", 0.0), ("lead_in", 0.0), ("lead_out", 0.0)];
    let run = |over: &[(&str, f64)]| {
        let mut ps: Vec<(&str, f64)> = base.to_vec();
        for (k, v) in over {
            match ps.iter_mut().find(|p| p.0 == *k) {
                Some(p) => p.1 = *v,
                None => ps.push((k, *v)),
            }
        }
        thread_on_shaft(30.0, 60.0, 0, &ps, false).0
    };
    let v0 = run(&[]);
    for (k, v) in [("pitch", 2.0), ("fit", 0.4), ("length", 20.0)] {
        let vi = run(&[(k, v)]);
        eprintln!("{k}={v}: {vi:.0} removed against {v0:.0}");
        assert!((vi - v0).abs() > 0.02 * v0, "parameter \"{k}\" changed nothing: {vi:.1} against {v0:.1}");
    }
    // The crest and root radii are limited by the profile's geometry (on a metric thread the crest flat is only
    // P/8), so their contribution to the volume is small - both the volume and the profile itself are checked.
    for (k, v) in [("crest_r", 0.3), ("root_r", 0.4)] {
        let vi = run(&[(k, v)]);
        eprintln!("{k}={v}: {vi:.0} removed against {v0:.0}");
        assert!((vi - v0).abs() > 0.002 * v0, "parameter \"{k}\" changed nothing: {vi:.1} against {v0:.1}");
    }
    // And THE PROFILE itself must respond: on a metric thread the crest flat is only P/8, so the radii move the
    // volume by a few per cent, but arcs must appear in the profile and their radius must change.
    use qymcad_core::geom::ProfEdge;
    use qymcad_core::thread::{ThreadSpec, ThreadStandard};
    let base_spec = ThreadSpec { standard: ThreadStandard::MetricIso, nominal_d: 30.0, pitch: 3.5, ..Default::default() };
    let radii = |sp: ThreadSpec| -> Vec<f64> {
        sp.geometry()
            .groove
            .iter()
            .filter_map(|e| match *e {
                ProfEdge::Arc { a, center, .. } => Some(((a.x - center.x).hypot(a.y - center.y) * 1e4).round() / 1e4),
                _ => None,
            })
            .collect()
    };
    assert!(radii(ThreadSpec { crest_r: Some(0.3), ..base_spec }).len() > radii(base_spec).len(), "a crest radius must add arcs to the profile");
    assert_ne!(radii(ThreadSpec { root_r: Some(0.2), ..base_spec }), radii(base_spec), "a root radius must change the radius of the profile's arcs");
}

/// A LEAD-IN and a RUN-OUT are different things, and both must work through the command bar. A lead-in at an
/// OPEN end countersinks the mouth with a cone, which is what a nut is started on. A run-out at a BLIND end
/// cuts an undercut groove for the thread to exit into: a circular ring to the profile's depth, so that when
/// tightened home the parts meet on their faces rather than on incomplete turns. Both remove more material
/// than a bare thread; the shape of each is checked by the kernel's tests (`thread_profile_fidelity`).
#[test]
fn thread_lead_in_and_run_out_work_through_the_command() {
    let base = [("nominal", 30.0), ("pitch", 3.5), ("length", 40.0), ("fit", 0.0)];
    let run = |li: f64, lo: f64| {
        let mut ps: Vec<(&str, f64)> = base.to_vec();
        ps.extend([("lead_in", li), ("lead_out", lo)]);
        thread_on_shaft(30.0, 60.0, 0, &ps, false)
    };
    let (plain, _) = run(0.0, 0.0);
    let (entry, app) = run(6.0, 0.0);
    let (fade, _) = run(0.0, 6.0);
    eprintln!("bare {plain:.0}, with a lead-in {entry:.0}, with a run-out groove {fade:.0}");
    // The threshold is deliberately small: a lead-in both CUTS the crests away with a cone (plus) and mutes the
    // turn over the lead-in's length (minus), so what the volume shows is the difference between the two.
    assert!(entry > plain * 1.02, "the lead-in does not cut the crests away: {entry:.0} against {plain:.0}; status: {}", app.status);
    assert!(fade > plain * 1.05, "the run-out cut no groove at the blind end: {fade:.0} against {plain:.0}");
}

/// AN AUGER through the full flow: the ribbon is WELDED onto the shaft and the volume grows. Reported behaviour:
/// the augers did not work at all - the flow used to be checked only as far as a node in the timeline.
#[test]
fn auger_command_actually_adds_a_flight() {
    let (removed, app) = thread_on_shaft(10.0, 80.0, 0, &[("outer", 30.0), ("pitch", 20.0), ("length", 60.0), ("thickness", 3.0), ("edge_r", 0.8)], true);
    let added = -removed;
    let a = qymcad_core::thread::AugerSpec { shaft_d: 10.0, outer_d: 30.0, pitch: 20.0, thickness: 3.0, edge_r: 0.8, ..Default::default() };
    let turns = 60.0 / a.lead();
    let mid_r = (10.0 + 30.0) * 0.25;
    let ribbon = turns * (2.0 * std::f64::consts::PI * mid_r).hypot(a.lead()) * a.thickness * a.flight_height();
    eprintln!("an auger through the GUI: {added:.0} mm^3 added against an estimated ribbon of {ribbon:.0}");
    assert!(added > 0.4 * ribbon && added < 1.8 * ribbon, "the ribbon was not welded on: {added:.0} added against an estimate of {ribbon:.0}; status: {}", app.status);
}
