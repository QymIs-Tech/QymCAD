//! THE GUI TESTS, lifted out of `gui.rs`.
//!
//! This is a CHILD module of `gui` rather than a neighbouring file: `App`'s private fields are visible to the
//! owning module and its descendants, so the headless flows keep working with the internal state directly.
// DO NOT REMOVE: rustc calls this import unused, yet without it the file does not compile - it is what lets the
// nested test modules see the `gui` names by the `super::` path (a private import is available to descendants).
#[allow(unused_imports)]
use super::*;

#[cfg(test)]
mod command_flow_tests {
    //! GUI PARITY: real user flows through App's COMMAND LAYER (headless, with the real OCCT kernel). Every fault
    //! found by hand ("it does not extrude", stale states) lived exactly here - the model-level matrices never saw
    //! them. Every new GUI fault becomes a flow in this module.
    use super::{App, BgKind, Busy, ExportTarget, JobResult, Picking, Sel};
    use qymcad_core::geom::Point3;
    use qymcad_core::model::Id;

    /// Draw a rectangle in a NEW sketch of the active part and leave editing.
    fn sketch_rect(app: &mut App, x0: f64, y0: f64, x1: f64, y1: f64) -> usize {
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, x0, y0, x1, y1, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        si
    }

    fn total_volume(app: &App) -> f64 {
        app.live.shapes.values().map(|s| s.volume()).sum()
    }

    /// A regression: an empty part, a sketch, the Extrude button (op=0), Enter. A body must appear.
    #[test]
    fn empty_part_extrude_flow() {
        let mut app = App::default();
        let si = sketch_rect(&mut app, 0.0, 0.0, 30.0, 30.0);
        app.sel = Sel::Sketch(si);
        app.feat.op = 0; // the Extrude button
        app.start_feat_cmd(1);
        assert_eq!(app.cmd.kind, 1, "the command started: {}", app.status);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 10.0;
            p.txt = "10".into();
        }
        app.apply_feat_cmd();
        assert_eq!(app.cmd.kind, 0, "the command finished (Enter worked): {}", app.status);
        let v = total_volume(&app);
        assert!((v - 9000.0).abs() < 90.0, "a 30x30x10 body was built, V={v:.0}; status: {}", app.status);
    }

    /// The root of that fault: a stale "through all" from the previous command does NOT leak into a new Extrude.
    #[test]
    fn stale_through_extent_does_not_leak() {
        let mut app = App::default();
        let si = sketch_rect(&mut app, 0.0, 0.0, 20.0, 20.0);
        app.cmd.extent = super::ExtentMode::Through; // the "previous command" left "through all" behind
        app.sel = Sel::Sketch(si);
        app.feat.op = 0;
        app.start_feat_cmd(1);
        assert!(matches!(app.cmd.extent, super::ExtentMode::Length), "the extent is reset when the command starts");
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
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
        for (nm, extent, depth) in [("to a depth of 5", super::ExtentMode::Length, 5.0), ("through all", super::ExtentMode::Through, 5.0)] {
            let mut app = App::default();
            let si = sketch_rect(&mut app, 0.0, 0.0, 20.0, 20.0);
            app.sel = Sel::Sketch(si);
            app.feat.op = 0;
            app.start_feat_cmd(1);
            if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
                p.val = 5.0;
                p.txt = "5".into();
            }
            app.apply_feat_cmd();
            let base = live_volume(&app);
            assert!((base - 2000.0).abs() < 20.0, "{nm}: a 20x20x5 plate, V={base:.1}");

            // a 5x5 square inside the plate, sketched on the same base plane
            let si2 = sketch_rect(&mut app, 5.0, 5.0, 10.0, 10.0);
            app.sel = Sel::Sketch(si2);
            app.feat.op = 2; // Cut
            app.start_feat_cmd(1);
            app.cmd.extent = extent;
            if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
                p.val = depth;
                p.txt = format!("{depth}");
            }
            app.apply_feat_cmd();
            assert_eq!(app.cmd.kind, 0, "{nm}: the cut finished: {}", app.status);
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
        app.sel = Sel::Sketch(si);
        app.feat.op = 0;
        app.start_feat_cmd(1);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 10.0;
            p.txt = "10".into();
        }
        app.apply_feat_cmd();
        assert!(!app.project.bodies.is_empty(), "there is a body: {}", app.status);

        // as after OPENING a file: the meshes are there, there is no live B-rep and nothing has tried to build one
        app.live.shapes.clear();
        app.live.ready = false;
        app.live.tried_rev = None;
        app.mode_3d = true;
        app.refresh_edges();
        assert!(app.live.shapes.is_empty(), "a B-rep is not built for no reason - that is the point of the lazy build");

        // the plane pick for a new sketch is on, so the vertices and edges are needed to bind the origin
        app.picking = Picking::SketchPlane(None);
        app.refresh_edges();
        assert!(!app.live.shapes.is_empty(), "the B-rep is ready for the sketch plane pick - there is something to bind the origin to");

        // and the axis candidates (a body's edge) are not found without a B-rep either
        app.live.shapes.clear();
        app.live.ready = false;
        app.live.tried_rev = None;
        app.picking.clear();
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
        app.sel = Sel::Sketch(si);
        app.feat.op = 0;
        app.start_feat_cmd(1);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 20.0;
            p.txt = "20".into();
        }
        app.apply_feat_cmd();

        // the intermediate operation (it gets deleted later)
        let si2 = sketch_rect(&mut app, 2.0, 2.0, 10.0, 10.0);
        app.sel = Sel::Sketch(si2);
        app.feat.op = 2;
        app.start_feat_cmd(1);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 3.0;
            p.txt = "3".into();
        }
        app.apply_feat_cmd();
        let mid_node = app.project.timeline.last().map(|n| n.id).unwrap();

        // an INDEPENDENT cut further down the timeline: a different sketch in a different place
        let si3 = sketch_rect(&mut app, 25.0, 25.0, 30.0, 30.0);
        app.sel = Sel::Sketch(si3);
        app.feat.op = 2;
        app.start_feat_cmd(1);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 20.0;
            p.txt = "20".into();
        }
        app.apply_feat_cmd();
        let cuts_before = app.project.timeline.iter().filter(|n| n.name.starts_with("feat-name-combine")).count();
        let v_before = live_volume(&app);

        let ti = app.project.timeline.iter().position(|n| n.id == mid_node).unwrap();
        app.delete_feature(ti);
        app.regenerate_now();

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
        app.sel = Sel::Sketch(si);
        app.feat.op = 2; // the Cut button
        app.start_feat_cmd(1);
        app.apply_feat_cmd();
        assert!(app.status.contains(&crate::i18n::tr("cmd-cut-needs-body")), "a clear status rather than silence: \"{}\"", app.status);
        assert!(total_volume(&app) < 1e-6, "no body appeared");
    }

    /// A full flow: extrude a base, then cut a pocket with a second sketch. The volumes are exact.
    #[test]
    fn extrude_then_cut_flow() {
        let mut app = App::default();
        let si = sketch_rect(&mut app, 0.0, 0.0, 20.0, 20.0);
        app.sel = Sel::Sketch(si);
        app.feat.op = 0;
        app.start_feat_cmd(1);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 20.0;
            p.txt = "20".into();
        }
        app.apply_feat_cmd();
        assert!((total_volume(&app) - 8000.0).abs() < 80.0, "a 20-cubed base: {}", app.status);
        // the second sketch: a 10x10 pocket, cut to a depth of 5
        let s2 = sketch_rect(&mut app, 5.0, 5.0, 15.0, 15.0);
        app.sel = Sel::Sketch(s2);
        app.feat.op = 2;
        app.start_feat_cmd(1);
        assert_eq!(app.cmd.kind, 1, "the cut started: {}", app.status);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 5.0;
            p.txt = "5".into();
        }
        app.apply_feat_cmd();
        assert_eq!(app.cmd.kind, 0, "the cut was applied: {}", app.status);
        // the consumed bodies are hidden; what is measured is the volume of THE RESULT (the last live body)
        let consumed = app.consumed_bodies();
        let v: f64 = app.live.shapes.iter().filter(|(b, _)| !consumed.contains(b)).map(|(_, s)| s.volume()).sum();
        assert!((v - 7500.0).abs() < 75.0, "the pocket is cut: V={v:.0}; status: {}", app.status);
    }

    /// THE RULE THAT A PART IS ONE BODY: two extrudes in a row (without entering a part) must give ONE unconsumed
    /// body (the second is added by a boolean) rather than two independent ones.
    #[test]
    fn two_extrudes_make_one_body() {
        let mut app = App::default();
        let si = sketch_rect(&mut app, 0.0, 0.0, 20.0, 20.0);
        app.sel = Sel::Sketch(si);
        app.feat.op = 0;
        app.start_feat_cmd(1);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 10.0;
            p.txt = "10".into();
        }
        app.apply_feat_cmd();
        // the second sketch OVERLAPS the first by half, so it becomes a boss
        let s2 = sketch_rect(&mut app, 10.0, 0.0, 30.0, 20.0);
        app.sel = Sel::Sketch(s2);
        app.feat.op = 0;
        app.start_feat_cmd(1);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 10.0;
            p.txt = "10".into();
        }
        app.apply_feat_cmd();
        let consumed = app.consumed_bodies();
        let live: Vec<f64> = app.live.shapes.iter().filter(|(b, _)| !consumed.contains(b)).map(|(_, s)| s.volume()).collect();
        assert_eq!(live.len(), 1, "a part is ONE live body, not {} (bodies are breeding); status: {}", live.len(), app.status);
        assert!((live[0] - 6000.0).abs() < 60.0, "the union of two 20x20x10 blocks overlapping by 10x20x10 is 6000, V={:.0}", live[0]);
    }

    /// A cylinder of diameter `d` and height `h` through the primitive command (the full flow). Returns (the body, the id of the rim's circular edge).
    fn build_shaft(app: &mut App, d: f64, h: f64) -> (u64, u32) {
        app.start_prim_cmd(11);
        for (k, v) in [("r", d * 0.5), ("h", h)] {
            if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == k) {
                p.val = v;
                p.txt = format!("{v}");
            }
        }
        app.apply_feat_cmd();
        let consumed = app.consumed_bodies();
        let body = *app.live.shapes.keys().find(|b| !consumed.contains(b)).expect("the shaft's body");
        let eid = app
            .project
            .regen_edges
            .get(&body)
            .and_then(|es| es.iter().find(|e| (e.radius - d * 0.5).abs() < 0.05).map(|e| e.id))
            .expect("the shaft's circular rim");
        (body, eid)
    }

    /// The part's live volume (excluding the consumed bodies), MEASURED FROM THE MESH. The kernel's integrator is
    /// off by whole multiples on helical surfaces (see `thread_profile_fidelity.rs`), so a thread cannot be
    /// measured with it.
    fn live_volume(app: &App) -> f64 {
        let consumed = app.consumed_bodies();
        app.live.shapes
            .iter()
            .filter(|(b, _)| !consumed.contains(b))
            .map(|(_, s)| s.tessellate(0.02).iter().map(|b| b.0.volume()).sum::<f64>())
            .sum()
    }

    /// Run the thread command on a shaft through the FULL GUI flow and return the volume removed.
    fn thread_on_shaft(d: f64, h: f64, form: u8, params: &[(&str, f64)], auger: bool) -> (f64, App) {
        let mut app = App::default();
        let (body, eid) = build_shaft(&mut app, d, h);
        let before = live_volume(&app);
        app.select_body(body);
        app.start_thread_cmd();
        app.thread.auger = auger;
        app.set_thread_params();
        app.thread.form = form;
        app.thread.src = Some(body);
        app.thread.edge = eid;
        app.thread.radius = d * 0.5;
        app.thread.axis = ([0.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
        for (k, v) in params {
            if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == *k) {
                p.val = *v;
                p.txt = format!("{v}");
            }
        }
        app.apply_feat_cmd();
        assert_eq!(app.cmd.kind, 0, "the command finished: {}", app.status);
        (before - live_volume(&app), app)
    }

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
        app.sel = Sel::Sketch(si);
        app.feat.op = 0;
        app.start_feat_cmd(3);
        assert_eq!(app.cmd.kind, 3, "the revolve command started: {}", app.status);
        let cands = app.profile_axis_lines(si);
        assert!(cands.len() >= 2, "the sketch holds several candidate lines");

        use egui::{Pos2, Rect, Vec2};
        use qymcad_core::geom::Point2;
        let rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0));

        // THE LINE IS CHOSEN BY A REAL CLICK on the line in the flat half-sketcher. There used to be a check of
        // flags alone here, and it missed exactly the fault that was reported: the search returned the line's END
        // POINTS, they were compared against the list of LINES, and there was never a match - the sketch opened
        // flat and the clicks selected nothing.
        app.rev.pick_line = true;
        app.mode_3d = false;
        app.view.center = Vec2::ZERO;
        app.view.scale = 8.0;
        for (want, x) in [(l1, -10.0_f64), (l2, 40.0)] {
            let pos = app.to_screen(rect, Point2::new(x, 5.0));
            let got = app.nearest_line_id(rect, pos, si, &cands);
            assert_eq!(got, Some(want), "a click on the line at x={x} picks THAT line (got {got:?})");
            assert!(cands.contains(&got.unwrap()), "what was picked is a LINE from the candidates, not one of its ends");
        }
        // a click into empty space picks nothing (otherwise the axis would jump on a miss)
        assert_eq!(app.nearest_line_id(rect, app.to_screen(rect, Point2::new(150.0, 150.0)), si, &cands), None, "a miss picks no line");

        // PICKING AN AXIS IN 3D: a datum axis built inside a part must both be drawn and be catchable by a click.
        app.rev.pick_line = false;
        app.rev.pick_axis = true;
        app.mode_3d = true; // exactly what the button does: the candidates are hit-tested in 3D only
        let ax = app.project.add_datum_axis(qymcad_core::model::DatumAxis::manual("Axis 1", [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]));
        assert!(app.datum_render_transform(ax).is_some(), "the datum axis is visible in its own context - otherwise there is nothing to click");
        let basis = app.cam.basis();
        let (s3, e3) = super::axis_segment([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 45.0);
        let (sa, sb) = (app.project3(s3, rect, &basis).0, app.project3(e3, rect, &basis).0);
        assert!(sa.distance(sb) > 20.0, "the axis projects into a segment rather than a point - there is something to hit");
        let mid = Pos2::new(0.5 * (sa.x + sb.x), 0.5 * (sa.y + sb.y));
        assert!(matches!(app.pick_axis_at(rect, mid), Some(super::AxisHit::Datum(id)) if id == ax), "a click on the datum axis picks it");
        assert!(app.axis_ref_world(super::AxisHit::Datum(ax)).is_some(), "the datum axis resolves into a revolve axis");
    }

    /// Reported behaviour: the holes in a section were still there after the profile had been fixed and the
    /// program restarted. The file stores FINISHED meshes and recomputes nothing on opening, so a part built by an
    /// older version of the kernel stays as it was. Edit -> Rebuild everything must mark THE WHOLE timeline dirty,
    /// throw away the live B-rep and go into a BACKGROUND rebuild (rather than a modal that dims the screen).
    #[test]
    fn rebuild_everything_marks_the_whole_timeline_and_goes_background() {
        let mut app = App::default();
        let si = sketch_rect(&mut app, 0.0, 0.0, 20.0, 20.0);
        app.sel = Sel::Sketch(si);
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
        assert_eq!(app.current_ctx_id(), part, "we are inside the part");
        assert!(app.datum_render_transform(ax).is_some(), "the part's own axis is visible from inside the part");

        app.mode_3d = true;
        app.cmd.kind = 3;
        app.rev.pick_axis = true;
        let rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0));
        let basis = app.cam.basis();
        let (s3, e3) = super::axis_segment([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 45.0);
        let (sa, sb) = (app.project3(s3, rect, &basis).0, app.project3(e3, rect, &basis).0);
        let mid = Pos2::new(0.5 * (sa.x + sb.x), 0.5 * (sa.y + sb.y));
        assert!(matches!(app.pick_axis_at(rect, mid), Some(super::AxisHit::Datum(id)) if id == ax), "the part's axis is caught by a click inside it");
        // and THE CLICK really carries it into the command's parameter: the click-handling branch used to sit in the
        // 2D half of the viewport while the candidates are hit-tested in 3D only - the press did nothing at all
        assert!(app.rev_axis_pick_click(rect, mid), "the click on the axis was accepted");
        assert_eq!(app.rev.axis_datum, ax, "the revolve axis is the datum axis that was clicked");
        assert!(!app.rev.pick_axis, "the pick sub-mode closed after the choice");
        // a miss neither clears the axis already chosen nor leaves the mode silently
        app.rev.pick_axis = true;
        assert!(!app.rev_axis_pick_click(rect, Pos2::new(rect.max.x - 2.0, rect.max.y - 2.0)), "a miss picks no axis");
        assert!(app.rev.pick_axis, "after a miss the axis pick stays on");

        // and from THE ROOT (outside the part) it is neither shown nor caught - otherwise another part's geometry would litter the assembly
        let root = app.project.root;
        app.enter_component(root);
        assert!(app.datum_render_transform(ax).is_none(), "the part's axis does not stick out into the assembly");
    }

    /// Reported behaviour: a section through a threaded part was filled only in the smooth part and empty in the
    /// thread. What is counted here is WHAT ACTUALLY GOES TO THE DRAWING: the cap's amber vertices by bands of
    /// height. The cap must be present in the threaded zone too, not only where the part is smooth.
    #[test]
    fn section_cap_covers_the_threaded_zone_too() {
        let mut app = App::default();
        let (body, eid) = build_shaft(&mut app, 20.0, 60.0);
        app.select_body(body);
        app.start_thread_cmd();
        app.thread.src = Some(body);
        app.thread.edge = eid;
        app.thread.radius = 10.0;
        app.thread.axis = ([0.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
        for (k, v) in [("nominal", 20.0), ("pitch", 2.5), ("length", 30.0), ("fit", 0.2), ("lead_in", 0.0), ("lead_out", 0.0)] {
            if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == k) {
                p.val = v;
                p.txt = format!("{v}");
            }
        }
        app.apply_feat_cmd();
        assert_eq!(app.cmd.kind, 0, "the thread was built: {}", app.status);

        // a section ALONG the axis: a plane through the axis with a +Y normal
        app.section.plane = Some(([0.0, 0.0, 0.0], [0.0, 1.0, 0.0]));
        let (verts, _) = app.gpu_scene();
        let amber = u32::from_le_bytes([224, 168, 92, 255]);
        let amber_back = u32::from_le_bytes([176, 128, 66, 255]);
        let cap: Vec<&crate::viewport_gpu::GpuVert> = verts.iter().filter(|v| v.color == amber || v.color == amber_back).collect();
        let _in_band = |a: f64, b: f64| cap.iter().filter(|v| (v.pos[2] as f64) >= a && (v.pos[2] as f64) <= b).count();
        eprintln!("cap vertices in the scene: {}", cap.len());
        // THE CAP MUST cover the whole outline of the cut. It is measured by area rather than by vertices in a
        // band: triangles can be long and their vertices may miss a narrow band entirely.
        let consumed = app.consumed_bodies();
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
        app.regenerate_all();
        assert!(app.regen.wanted, "the rebuild was queued rather than done on the spot");
        assert!(app.regen.busy.is_none(), "the work itself starts a frame later - the indicator is shown first");

        // as soon as the window is back in headless mode, the work happens immediately again
        app.regen.ui_running = false;
        app.regen.wanted = false;
        app.regenerate_all();
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

    /// A 20-cubed block through the full flows (sketch, then extrude). Returns the body's id.
    pub(super) fn build_cube(app: &mut App) -> u64 {
        let si = sketch_rect(app, 0.0, 0.0, 20.0, 20.0);
        app.sel = Sel::Sketch(si);
        app.feat.op = 0;
        app.start_feat_cmd(1);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 20.0;
            p.txt = "20".into();
        }
        app.apply_feat_cmd();
        let consumed = app.consumed_bodies();
        *app.live.shapes.keys().find(|b| !consumed.contains(b)).expect("the block's body")
    }

    /// A fillet BY BUTTON: pick an edge, set a radius, press Enter (without entering the part).
    #[test]
    fn fillet_button_flow() {
        let mut app = App::default();
        let cube = build_cube(&mut app);
        let eid = app
            .project
            .regen_edges
            .get(&cube)
            .and_then(|es| es.iter().find(|e| e.a[0].abs() < 1e-6 && e.a[1].abs() < 1e-6 && (e.a[2] - e.b[2]).abs() > 1.0))
            .map(|e| e.id)
            .expect("a vertical edge");
        app.start_feat_cmd(4);
        assert_eq!(app.cmd.kind, 4, "the fillet started: {}", app.status);
        app.gsel.edges.insert(eid);
        app.edges.body = Some(cube);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "radius") {
            p.val = 4.0;
            p.txt = "4".into();
        }
        app.apply_feat_cmd();
        assert_eq!(app.cmd.kind, 0, "the fillet was applied: {}", app.status);
        let consumed = app.consumed_bodies();
        let v: f64 = app.live.shapes.iter().filter(|(b, _)| !consumed.contains(b)).map(|(_, s)| s.volume()).sum();
        let exp = 8000.0 - (16.0 - std::f64::consts::PI * 4.0) * 20.0;
        assert!((v - exp).abs() < 10.0, "a fillet of r4: V={v:.1}, expected {exp:.1}");
    }

    /// EDITING A FILLET: the picked edges are LIT (the edge selection is restored, and the edges come from THE SOURCE BODY).
    #[test]
    fn edit_fillet_shows_selected_edges() {
        let mut app = App::default();
        let cube = build_cube(&mut app);
        let eid = app
            .project
            .regen_edges
            .get(&cube)
            .and_then(|es| es.iter().find(|e| (e.a[2] - e.b[2]).abs() > 10.0))
            .map(|e| e.id)
            .expect("an edge");
        app.start_feat_cmd(4);
        app.gsel.edges.insert(eid);
        app.edges.body = Some(cube);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "radius") {
            p.val = 3.0;
            p.txt = "3".into();
        }
        app.apply_feat_cmd();
        let fid = app
            .project
            .timeline
            .iter()
            .find(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::Fillet { .. }))
            .map(|n| n.id)
            .expect("the fillet's node");
        app.start_feat_cmd_edit(fid);
        app.refresh_edges(); // a UI frame: the edges must stay those of THE SOURCE and the selection must stay alive
        assert_eq!(app.edges.body, Some(cube), "the edges aim at the SOURCE body, not at the fillet's output");
        assert!(app.gsel.edges.contains(&eid), "the edges picked earlier are LIT while editing");
    }

    /// EDITING A FEATURE (a double click, change the height, Enter): it updates IN PLACE and no nodes are bred.
    #[test]
    fn edit_feature_height_flow() {
        let mut app = App::default();
        let _cube = build_cube(&mut app);
        let nodes_before = app.project.timeline.len();
        let fid = app
            .project
            .timeline
            .iter()
            .find(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::Extrude { .. }))
            .map(|n| n.id)
            .expect("the extrude's node");
        app.start_feat_cmd_edit(fid);
        assert!(app.cmd.edit.is_some(), "the edit mode is open: {}", app.status);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 30.0;
            p.txt = "30".into();
        }
        app.apply_feat_cmd();
        assert_eq!(app.project.timeline.len(), nodes_before, "editing breeds NO nodes");
        let consumed = app.consumed_bodies();
        let v: f64 = app.live.shapes.iter().filter(|(b, _)| !consumed.contains(b)).map(|(_, s)| s.volume()).sum();
        assert!((v - 12000.0).abs() < 120.0, "the height went 20 -> 30: V={v:.0}; status: {}", app.status);
    }

    /// Deleting an operation from the tree: the cut is gone and the block is back; deleting the base leaves nothing and no orphans.
    #[test]
    fn delete_feature_flow() {
        let mut app = App::default();
        let _cube = build_cube(&mut app);
        let s2 = sketch_rect(&mut app, 5.0, 5.0, 15.0, 15.0);
        app.sel = Sel::Sketch(s2);
        app.feat.op = 2;
        app.start_feat_cmd(1);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 5.0;
            p.txt = "5".into();
        }
        app.apply_feat_cmd();
        let cut_ti = app
            .project
            .timeline
            .iter()
            .position(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::Combine { .. }))
            .expect("the cut's node");
        app.delete_feature(cut_ti);
        app.regenerate_all();
        let consumed = app.consumed_bodies();
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
        let consumed = app.consumed_bodies();
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
        app.select_body(cube);
        app.start_thread_cmd();
        assert_eq!(app.cmd.kind, 24, "the thread command started: {}", app.status);

        // the fields are the size and the fit, NOT the angle and the depth
        let keys: Vec<String> = app.cmd.params.iter().map(|p| p.key.to_string()).collect();
        assert!(keys.iter().any(|k| k == "nominal") && keys.iter().any(|k| k == "pitch") && keys.iter().any(|k| k == "fit"), "the thread's fields: {keys:?}");
        assert!(!keys.iter().any(|k| k == "angle") && !keys.iter().any(|k| k == "depth"), "the angle and the thread depth are no longer typed by hand: {keys:?}");
        assert!((app.cmd_val("pitch") - 0.0).abs() < 1e-9, "a default pitch of 0 means the standard coarse one");

        // switching to the AUGER changes the set of fields to the ribbon's
        app.thread.auger = true;
        app.set_thread_params();
        let keys: Vec<String> = app.cmd.params.iter().map(|p| p.key.to_string()).collect();
        assert!(keys.iter().any(|k| k == "outer") && keys.iter().any(|k| k == "thickness") && keys.iter().any(|k| k == "edge_r"), "the auger's fields: {keys:?}");

        // applying the auger puts an Auger feature with its own specification into the timeline
        app.thread.auger = true;
        app.thread.src = Some(cube);
        app.thread.edge = 1;
        app.thread.radius = 5.0;
        let body = app.apply_thread_cmd().expect("the auger's node was created");
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
        app.select_body(cube);
        app.start_thread_cmd();
        app.thread.src = Some(cube);
        app.thread.edge = 1;
        app.thread.radius = 5.0;
        app.thread.form = 0; // metric ISO
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "nominal") {
            p.val = 10.0;
        }
        let body = app.apply_thread_cmd().expect("the thread's node was created");
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
        let cands = app.profile_axis_lines(si);
        assert!(cands.contains(&plain), "an ordinary line must be an axis candidate: {cands:?}");
        assert_eq!(app.axis_line_label(si, plain, 1), crate::i18n::tr1("sk-line-n", "n", "1"), "the label says honestly that this is an ordinary line");

        // a construction line is a candidate too, and comes FIRST (it is drawn precisely to be an axis)
        let constr = app.project.add_line_entity(si, -50.0, 0.0, 50.0, 0.0, qymcad_core::feature::Purpose::Construction);
        app.project.regen_sketch(si);
        let cands = app.profile_axis_lines(si);
        assert_eq!(cands.first(), Some(&constr), "the construction line is offered first: {cands:?}");
        assert!(cands.contains(&plain), "the ordinary one has not gone anywhere");
        assert_eq!(app.axis_line_label(si, constr, 1), crate::i18n::tr1("sk-axis-line-n", "n", "1"));
    }

    /// Reported behaviour: in a sketch, Esc with the measure tool active finished the sketch straight away, and
    /// the same happened with elements selected. ESC IS A LADDER: it clears the innermost state and only at the
    /// bottom leaves the sketch. The measure tool and the selection were not on the ladder at all, so the very
    /// first Esc fell through to `finish_sketch_edit`.
    #[test]
    fn escape_cancels_tool_then_selection_then_sketch() {
        let mut app = App::default();
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, 0.0, 0.0, 20.0, 20.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        assert!(app.sketch_ses.editing.is_some(), "the sketch is open");

        // 1) the measure tool is active: Esc returns to Select and the sketch STAYS open
        app.measure.on = true;
        app.measure.pts.push(qymcad_core::geom::Point2::new(1.0, 1.0));
        app.on_escape();
        assert!(!app.measure.on && app.measure.pts.is_empty(), "the measure tool is dropped: {}", app.status);
        assert!(app.sketch_ses.editing.is_some(), "the sketch was NOT closed by the very first Esc");

        // 2) there are selected elements: Esc clears the selection and the sketch is still open
        let ent = app.project.sketches[si].entities.first().map(|e| e.id).expect("an entity");
        app.sel_sk.items.push((1, ent));
        app.on_escape();
        assert!(app.sel_sk.items.is_empty(), "the selection is cleared: {}", app.status);
        assert!(app.sketch_ses.editing.is_some(), "and the sketch is still open");

        // 3) nothing is active: only now does Esc finish the sketch
        app.on_escape();
        assert!(app.sketch_ses.editing.is_none(), "at the bottom of the ladder Esc closes the sketch");
    }

    /// The same ladder for the drawing and dimension tools - they already worked, but a regression is caught by
    /// the same test (the order of the rungs matters).
    #[test]
    fn escape_ladder_covers_draw_and_dim_tools() {
        let mut app = App::default();
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, 0.0, 0.0, 20.0, 20.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);

        app.set_dim_tool(1); // the distance dimension
        app.on_escape();
        assert_eq!(app.dim.kind, 0, "the dimension tool is dropped");
        assert!(app.sketch_ses.editing.is_some(), "the sketch is open");

        app.tool.kind = 2; // a drawing tool
        app.tool.pts.push(qymcad_core::geom::Point2::new(0.0, 0.0));
        app.on_escape();
        assert!(app.tool.pts.is_empty() && app.tool.kind == 2, "the first Esc aborts the UNFINISHED construction");
        app.on_escape();
        assert_eq!(app.tool.kind, 0, "the second Esc leaves the tool for Select");
        assert!(app.sketch_ses.editing.is_some(), "and only after that may the sketch be closed");
    }

    /// Reported behaviour: every opening ran "Preparing B-rep", and afterwards the program asked to save although
    /// nothing had been changed. Two faults at once: (1) `refresh_edges` is called EVERY FRAME in 3D and pulled
    /// `ensure_brep` unconditionally, turning the lazy B-rep build into an eager one right after opening; (2)
    /// building that cache raised `geom_rev`, which is part of the state key, so a clean project became dirty
    /// without a single edit.
    #[test]
    fn opening_and_preparing_brep_keeps_project_clean() {
        let dir = std::env::temp_dir().join("qym_open_clean_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("proj.qcad").to_string_lossy().into_owned();
        let _ = std::fs::remove_file(&path);
        let mut app = App::default();
        let cube = build_cube(&mut app);
        app.project_path = Some(path.clone());
        app.save_project();
        app.wait_bg();

        // reopen it, the way the application does
        let loaded = qymcad_io::load_project(&path).expect("the bundle reads");
        let mut app2 = App::default();
        app2.finish_project_load(path.clone(), loaded, Vec::new());
        assert!(!app2.is_dirty(), "a project just opened is clean");

        // a frame of the 3D viewport: the edges refresh every frame, but the B-rep must NOT be built eagerly
        app2.refresh_edges();
        assert!(!app2.live.ready && app2.live.shapes.is_empty(), "the B-rep stays lazy: brep_ready={}, shapes={}", app2.live.ready, app2.live.shapes.len());
        assert!(!app2.is_dirty(), "looking at the model does not make the project dirty");

        // and now an operation really asks for the B-rep: it gets built while the project stays CLEAN
        app2.ensure_brep();
        assert!(app2.live.shapes.contains_key(&cube), "the B-rep was built on demand");
        assert!(!app2.is_dirty(), "rebuilding a cache is not a person's edit; there is nothing to ask to save");

        // and only a real edit makes the project dirty
        let si = sketch_rect(&mut app2, 40.0, 0.0, 60.0, 20.0);
        app2.sel = Sel::Sketch(si);
        app2.feat.op = 0;
        app2.start_feat_cmd(1);
        app2.apply_feat_cmd();
        assert!(app2.is_dirty(), "after a real edit the project is dirty");
        let _ = std::fs::remove_file(&path);
    }

    /// `wait_bg` used to wait on `recv()` WITHOUT a timeout. Closing the window during long background work
    /// (restoring a B-rep from an 80 MB STEP takes 36 seconds) gave a dead-frozen window again, this time without
    /// even a spinner. The wait now has a ceiling: a B-rep top-up is waited for only briefly (it can be repeated).
    #[test]
    fn wait_bg_gives_up_on_slow_background_job() {
        let mut app = App::default();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(30)); // a "slow" worker
            let _ = tx.send(JobResult::ImportShapes { shapes: Vec::new(), regen: false });
        });
        app.regen.bg.push(Busy { label: "a long top-up".into(), rx, kind: BgKind::ImportShapes, pulse: None, quiet: false });
        let t = std::time::Instant::now();
        app.wait_bg();
        let waited = t.elapsed();
        assert!(waited < std::time::Duration::from_secs(3), "quitting must not hang on a background task: waited {waited:?}");
        assert!(app.status.contains(&crate::i18n::tr_prefix("io-not-waited", "what")), "it is said honestly that the task stayed in the background: {}", app.status);
        assert!(app.regen.bg.is_empty(), "the queue is cleared and the window will close");
    }

    /// An undo snapshot must not carry the BYTES of the embedded sources. They never change and weigh tens of
    /// megabytes: 40 steps at 89 MB on a real assembly means gigabytes of memory and a ~90 MB memcpy on every
    /// committed edit. The undo must nevertheless stay COMPLETE: after a rollback the bytes are there and the file
    /// saves whole.
    #[test]
    fn undo_snapshot_does_not_carry_embedded_source_bytes() {
        let mut app = App::default();
        let _cube = build_cube(&mut app);
        let src_id = app.project.alloc_id();
        let payload = vec![7u8; 3_000_000]; // an "embedded STEP" of 3 MB
        app.project.sources.push(qymcad_core::model::SourceFile { id: src_id, name: "big.step".into(), ext: "step".into(), data: payload.clone() });

        let snap = app.snapshot();
        let in_snap: usize = snap.project.sources.iter().map(|s| s.data.len()).sum();
        assert_eq!(in_snap, 0, "the snapshot holds NO source bytes (that would be a memcpy on every edit)");
        assert_eq!(snap.project.sources.len(), 1, "the source's record itself is in the snapshot (name, extension, id)");

        // and the undo still restores the state COMPLETELY: the bytes come back from the live project
        app.edits.baseline = snap;
        let si = sketch_rect(&mut app, 40.0, 0.0, 60.0, 20.0);
        app.sel = Sel::Sketch(si);
        app.feat.op = 0;
        app.start_feat_cmd(1);
        app.apply_feat_cmd();
        // the undo step is created by THE COMMAND itself (the edit boundary) - nothing has to be pushed by hand
        app.undo();
        let live: Vec<u8> = app.project.sources.iter().find(|s| s.id == src_id).map(|s| s.data.clone()).unwrap_or_default();
        assert_eq!(live, payload, "after an undo the source's bytes are there - saving will not lose the original");
    }

    /// There can be SEVERAL background tasks. There used to be a single slot: Ctrl+S during the restoration of the
    /// imports' B-rep overwrote its receiver - the thread finished computing and sent the shapes into a closed
    /// channel, and the import's B-rep did not appear until a restart (while the ready flag was already set).
    #[test]
    fn save_during_import_restore_does_not_lose_shapes() {
        use qymcad_core::feature::FeatureKind;
        let dir = std::env::temp_dir().join("qym_bg_jobs_test");
        std::fs::create_dir_all(&dir).unwrap();
        let step = dir.join("src.step");
        let proj_path = dir.join("proj.qcad").to_string_lossy().into_owned();
        let _ = std::fs::remove_file(&proj_path);

        // a real embedded source: a block, then STEP, then the bytes into the project plus an import node on them
        let cube = qymcad_kernel::Shape::extrude(&[0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0], 10.0).expect("the block");
        qymcad_kernel::write_step(&[(&cube, qymcad_core::feature::PLACE_IDENTITY)], step.to_string_lossy().as_ref()).expect("the step was written");
        let bytes = std::fs::read(&step).expect("the step's bytes");
        let mut app = App::default();
        let src_id = app.project.alloc_id();
        app.project.sources.push(qymcad_core::model::SourceFile { id: src_id, name: "src.step".into(), ext: "step".into(), data: bytes });
        let body = app.project.add_mesh(qymcad_core::geom::Mesh::default());
        app.project.timeline.push(qymcad_core::feature::FeatureNode {
            id: body,
            name: "Import".into(),
            kind: FeatureKind::Import { body, source: src_id, solid: 0 },
            parent: Some(app.project.root),
            dirty: false,
            suppressed: false,
        });
        app.project_path = Some(proj_path.clone());

        // BOTH tasks at once: topping up the import's B-rep and writing the project
        app.spawn_import_shapes(false);
        app.save_project();
        assert_eq!(app.regen.bg.len(), 2, "two background tasks live at once (the second used to overwrite the first)");
        app.wait_bg();

        assert!(app.live.shapes.contains_key(&body), "the import's B-rep was restored rather than lost in a closed channel");
        assert!(std::path::Path::new(&proj_path).exists(), "the project was written: {}", app.status);
        assert!(app.regen.bg.is_empty(), "the background queue is empty");
        let _ = std::fs::remove_file(&proj_path);
        let _ = std::fs::remove_file(&step);
    }

    /// Two writes of one file at once are a race between the temporary file and the rename. The second request
    /// must WAIT rather than start in parallel; and it is not lost - it runs immediately after.
    #[test]
    fn second_save_is_queued_not_parallel() {
        let dir = std::env::temp_dir().join("qym_bg_queue_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("proj.qcad").to_string_lossy().into_owned();
        let _ = std::fs::remove_file(&path);
        let mut app = App::default();
        let _cube = build_cube(&mut app);
        app.project_path = Some(path.clone());

        app.save_project();
        app.save_project(); // the second request, while the first is still in flight
        assert_eq!(app.regen.bg.iter().filter(|b| b.kind == BgKind::Save).count(), 1, "there are no parallel writes");
        assert!(app.io.save_request.is_some(), "the second request is deferred rather than thrown away");
        app.wait_bg();
        assert!(app.io.save_request.is_none(), "the deferred write was carried out");
        assert!(std::path::Path::new(&path).exists() && !app.is_dirty(), "the file is written and the project is clean: {}", app.status);
        let _ = std::fs::remove_file(&path);
    }

    /// Save used to mark the project CLEAN before the background write had finished. If the write failed (no
    /// permission, a full disk, a broken path), `is_dirty()` was already false, the "save?" dialogue would not
    /// appear on closing and the edits would go silently. The snapshot's key must be applied ONLY on success.
    #[test]
    fn failed_save_keeps_project_dirty() {
        let mut app = App::default();
        let _cube = build_cube(&mut app);
        assert!(app.is_dirty(), "after building there are unsaved edits");
        // a deliberately impossible path: a directory instead of a file, with non-existent parents
        app.project_path = Some("/proc/qymcad-no-such/path/proj.qcad".into());
        app.save_project();
        app.wait_bg();
        assert!(app.status.contains(&crate::i18n::tr_prefix("io-save-error", "error")), "the error is honestly in the status line: {}", app.status);
        assert!(app.is_dirty(), "the project STAYS dirty - otherwise quitting without a warning would lose the work");
    }

    /// A successful write, conversely, must clear the dirt - and by THE SNAPSHOT: an edit made while the write was
    /// running makes the project dirty again.
    #[test]
    fn successful_save_clears_dirty_but_edits_during_write_do_not() {
        let dir = std::env::temp_dir().join("qym_save_dirty_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("proj.qcad").to_string_lossy().into_owned();
        let _ = std::fs::remove_file(&path);
        let mut app = App::default();
        let _cube = build_cube(&mut app);
        app.project_path = Some(path.clone());
        app.save_project();
        app.wait_bg();
        assert!(!app.is_dirty(), "after a successful write the project is clean: {}", app.status);
        assert!(std::path::Path::new(&path).exists(), "the file is on disk");
        // an edit after the snapshot makes it dirty again (a full edit through the command layer)
        let si = sketch_rect(&mut app, 40.0, 40.0, 50.0, 50.0);
        app.sel = Sel::Sketch(si);
        app.feat.op = 0;
        app.start_feat_cmd(1);
        app.apply_feat_cmd();
        assert!(app.is_dirty(), "a new edit makes the project dirty again");
        let _ = std::fs::remove_file(&path);
    }

    /// `ensure_brep` used to raise the "cache is ready" flag UNCONDITIONALLY. When there was nothing to rebuild
    /// from (an import waiting for its B-rep to be restored from an embedded STEP), operations silently fell into
    /// "the body was not built" and there was never a second attempt. The flag must reflect THE FACT.
    #[test]
    fn ensure_brep_reports_readiness_by_fact() {
        use qymcad_core::feature::FeatureKind;
        let mut app = App::default();
        let cube = build_cube(&mut app);
        // emulate opening from a bundle: the geometry is there and there is no live B-rep
        app.live.shapes.clear();
        app.live.ready = false;
        app.ensure_brep();
        assert!(app.live.ready && app.live.shapes.contains_key(&cube), "an ordinary feature rebuilds: {}", app.status);

        // and now a body there is NOTHING to rebuild from: an import node with no source in the kernel's cache
        let body = app.project.add_mesh(qymcad_core::geom::Mesh::default());
        app.project.timeline.push(qymcad_core::feature::FeatureNode {
            id: body,
            name: "Import".into(),
            kind: FeatureKind::Import { body, source: 0, solid: 0 },
            parent: Some(app.project.root),
            dirty: false,
            suppressed: false,
        });
        app.live.ready = false;
        app.ensure_brep();
        assert!(!app.live.ready, "the import's B-rep is not restored - the cache is NOT ready and the attempt must be repeated");
    }

    /// Reported behaviour: pressing "create a sketch" made the rebuild window flicker back and forth. Preparing the
    /// B-rep is called EVERY FRAME (the sketch plane pick), and when there is nothing to rebuild a body from, the
    /// attempt must happen ONCE: the "already tried" mark was set BEFORE the rebuild, which moves geom_rev, so the
    /// mark never matched and the rebuild went into an endless loop, flickering the overlay.
    #[test]
    fn failed_brep_attempt_is_not_repeated_every_frame() {
        use qymcad_core::feature::FeatureKind;
        let mut app = App::default();
        build_cube(&mut app);
        // a body there is NOTHING to rebuild from (an import with no source) - the cache will never become ready
        let body = app.project.add_mesh(qymcad_core::geom::Mesh::default());
        app.project.timeline.push(qymcad_core::feature::FeatureNode {
            id: body,
            name: "Import".into(),
            kind: FeatureKind::Import { body, source: 0, solid: 0 },
            parent: Some(app.project.root),
            dirty: false,
            suppressed: false,
        });
        app.live.shapes.clear();
        app.live.ready = false;
        app.live.tried_rev = None;

        app.ensure_brep();
        assert!(!app.live.ready, "the cache is honestly NOT ready - there is nothing to rebuild the import from");
        let rev = app.regen.geom_rev;

        // the next frames of the plane pick: not one new rebuild while the geometry has not changed
        for _ in 0..5 {
            app.ensure_brep();
        }
        assert_eq!(app.regen.geom_rev, rev, "there are no repeated rebuilds - the overlay does not flicker");
    }

    /// AN UNDO must roll back the live B-rep as well, not only the meshes. `restore()` returned `project`, `faces`
    /// and `mesh_visible` but NOT `App.shapes` - after Ctrl+Z the mesh showed the old geometry while the kernel's
    /// cache held the NEW one, and the next operation was built on the undone shape. Silently: after a rollback the
    /// nodes are not dirty, so this does not heal by itself.
    #[test]
    fn undo_restores_brep_cache_not_only_meshes() {
        use qymcad_core::feature::FeatureKind;
        let mut app = App::default();
        let cube = build_cube(&mut app);
        let v0 = app.live.shapes.get(&cube).map(|s| s.volume()).unwrap_or(0.0);
        assert!((v0 - 8000.0).abs() < 1.0, "a 20-cubed block was built: {v0}");
        app.edits.baseline = app.snapshot(); // as in the real cycle: the state before the edit is fixed

        // editing the extrude's HEIGHT from 20 to 30 (the same body, the Id does not change - the trickiest case)
        let fid = app.project.timeline.iter().find(|n| matches!(n.kind, FeatureKind::Extrude { .. })).map(|n| n.id).expect("the extrude's node");
        app.start_feat_cmd_edit(fid);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 30.0;
            p.txt = "30".into();
        }
        app.apply_feat_cmd();
        let body = app.project.timeline.iter().find_map(|n| if n.id == fid { n.kind.body() } else { None }).expect("the feature's body");
        let v1 = app.live.shapes.get(&body).map(|s| s.volume()).unwrap_or(0.0);
        assert!((v1 - 12000.0).abs() < 120.0, "after editing 20 -> 30: V={v1:.0}");
        // the undo step is created by THE COMMAND itself (the edit boundary) - nothing has to be pushed by hand

        app.undo();

        // the mesh came back - that already worked
        let tris = app.project.mesh_index(body).map(|i| app.project.bodies[i].mesh.tris.len()).unwrap_or(0);
        assert!(tris > 0, "the body is there after the undo");
        let mz = app.project.mesh_index(body).and_then(|i| app.project.bodies[i].mesh.bounds()).map(|b| b.max.z).unwrap_or(0.0);
        assert!((mz - 20.0).abs() < 0.1, "the mesh rolled back to a height of 20: max z = {mz}");
        // and the B-rep is where the defect lived: the cache must match what is on the screen
        let vu = app.live.shapes.get(&body).map(|s| s.volume()).unwrap_or(0.0);
        assert!(
            (vu - 8000.0).abs() < 80.0,
            "after an undo the live B-rep must be the pre-edit one: V={vu:.0} (expected 8000; 12000 means the cache stayed from the undone edit)"
        );
    }

    /// The basic undo and redo cycle through the command layer: edit a sketch, undo, redo.
    /// There were NO tests of undo at all, although it is one of the most used functions.
    #[test]
    fn undo_redo_cycle_on_sketch_edit() {
        let mut app = App::default();
        let _cube = build_cube(&mut app);
        let volume = |a: &App| -> f64 {
            let consumed = a.consumed_bodies();
            a.live.shapes.iter().filter(|(b, _)| !consumed.contains(b)).map(|(_, s)| s.volume()).sum()
        };
        app.edits.baseline = app.snapshot();
        let v0 = volume(&app);

        // stretch the base rectangle from 20 to 30 through the GUI cycle. In a live window this is a point DRAG,
        // which opens an edit of its own (see `drag_pt`): the same thing is done here by hand - an edit boundary.
        app.enter_sketch_edit(0);
        app.begin_edit("Moving a point");
        for pt in &mut app.project.sketches[0].points {
            if (pt.x - 20.0).abs() < 1e-9 {
                pt.x = 30.0;
            }
        }
        app.project.solve_sketch(0);
        app.project.regen_sketch(0);
        app.finish_sketch_edit();
        app.commit_edit();
        let v1 = volume(&app);
        assert!((v1 - 12000.0).abs() < 120.0, "the sketch edit was applied: {v1:.0}");
        // the undo step is created by THE COMMAND itself (the edit boundary) - nothing has to be pushed by hand

        app.undo();
        assert!((volume(&app) - v0).abs() < 80.0, "the undo restored a volume of {v0:.0}, got {:.0}", volume(&app));
        app.redo();
        assert!((volume(&app) - v1).abs() < 80.0, "the redo restored a volume of {v1:.0}, got {:.0}", volume(&app));
    }

    /// Reported behaviour: deleting a boss in a large project froze it for minutes. DELETING a node rebuilds only
    /// what it touches, not the whole project. `resync_after_topology_change` used to clear the faces and call a
    /// FORCED rebuild (the only thing that filled them back in) - on an assembly of a thousand bodies that is a
    /// re-tessellation of everything. The trap in this test: the mesh of an unrelated body is SPOILED; a full
    /// rebuild would restore it, an honest "only what is dirty" leaves it alone. And the faces of unrelated bodies
    /// must survive the topology change.
    #[test]
    fn deleting_feature_does_not_rebuild_whole_project() {
        let mut app = App::default();
        let keep = build_cube(&mut app); // part one, which is left alone
        // part two with a body of its own (this is the one that gets deleted)
        let part2 = app.project.add_part("Part two");
        app.enter_component(part2);
        let before: std::collections::HashSet<Id> = app.live.shapes.keys().copied().collect();
        let si = sketch_rect(&mut app, 40.0, 0.0, 60.0, 20.0);
        app.sel = Sel::Sketch(si);
        app.feat.op = 0;
        app.start_feat_cmd(1);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 10.0;
            p.txt = "10".into();
        }
        app.apply_feat_cmd();
        let consumed = app.consumed_bodies();
        let victim = *app.live.shapes.keys().find(|b| !before.contains(b) && !consumed.contains(b)).expect("part two's body");

        // the trap: the mesh of an UNRELATED body is spoiled and its faces are remembered
        let keep_faces = app.project.mesh_index(keep).and_then(|i| app.project.bodies.get(i).map(|b| b.faces.clone())).unwrap_or_default();
        assert!(!keep_faces.is_empty(), "the unrelated body has B-rep faces");
        if let Some(i) = app.project.mesh_index(keep) {
            app.project.bodies[i].mesh.tris.clear(); // the spoiling: a full rebuild would restore this, a targeted one would not
        }

        // delete part two's node
        let ti = app
            .project
            .timeline
            .iter()
            .position(|n| n.kind.body() == Some(victim))
            .expect("the node of part two's body");
        app.delete_feature(ti);

        assert!(app.project.mesh_index(victim).is_none() && !app.live.shapes.contains_key(&victim), "the deleted body went away entirely");
        let ki = app.project.mesh_index(keep).expect("the unrelated body is still there");
        assert!(app.project.bodies[ki].mesh.tris.is_empty(), "the unrelated bodies were NOT rebuilt (that is what froze a large project)");
        assert_eq!(app.project.bodies.get(ki).map(|b| b.faces.len()), Some(keep_faces.len()), "the unrelated body's faces survived the topology change (cached by body id)");
    }

    /// Reported behaviour: there was no amber cap and the bodies looked hollow inside - no CAD cuts that way. The
    /// section cap is built FROM THE MESH and therefore exists ALWAYS - including without a live B-rep (right after
    /// opening from a bundle, or for an imported STL) and during a gizmo drag. The bodies the plane does not touch
    /// are rejected by their extent - otherwise a frame would cost minutes on a thousand bodies.
    #[test]
    fn section_cap_is_built_from_mesh_always() {
        let mut app = App::default();
        let cube = build_cube(&mut app); // a block from 0 to 20
        let mi = app.project.mesh_index(cube).expect("the block's mesh");
        assert!(app.mesh_crosses_plane(mi, [0.0, 0.0, 10.0], [0.0, 0.0, 1.0]), "the plane goes through the body, so a cap is needed");
        assert!(!app.mesh_crosses_plane(mi, [0.0, 0.0, 100.0], [0.0, 0.0, 1.0]), "the plane misses the body, so no cap is computed");
        assert!(!app.mesh_crosses_plane(mi, [-50.0, 0.0, 0.0], [1.0, 0.0, 0.0]), "and a miss from the side counts the same");

        // THE KEY POINT: there is no live B-rep at all and the cap must still be there (it used to be computed by a kernel boolean)
        app.live.shapes.clear();
        app.section.plane = Some(([0.0, 0.0, 10.0], [0.0, 0.0, 1.0]));
        let caps = app.section_caps_for_frame();
        assert!(!caps.is_empty(), "the cap is there without a B-rep too");
        let area: f64 = caps
            .iter()
            .flat_map(|m| (0..m.tris.len()).map(|i| m.tri_normal_area(i).1))
            .sum();
        assert!((area - 400.0).abs() < 1e-6, "the cap's area equals the block's 20x20 section: {area}");
        for m in caps.iter() {
            for v in &m.verts {
                assert!((v.z - 10.0).abs() < 1e-9, "the cap lies exactly in the cutting plane: z={}", v.z);
            }
        }

        // and during a gizmo drag the cap does NOT vanish (from the mesh this is cheap)
        app.section.drag = true;
        app.section.offset = 2.0;
        assert!(!app.section_caps_for_frame().is_empty(), "during a drag the section stays a closed body");
        // a plane that misses the whole scene gives no caps (and the kernel is not spent on it)
        app.section.drag = false;
        app.section.offset = 500.0;
        assert!(app.section_caps_for_frame().is_empty(), "a plane outside the model gives no caps");
    }

    /// Reported behaviour: loading a large project froze it and brought up a "not responding" window. Opening now
    /// takes the geometry FROM THE BUNDLE and does NOT rebuild the timeline. `write_project` used to throw away the
    /// features' bodies ("the timeline will rebuild them") while loading called a forced rebuild: on a real
    /// assembly that was 36 seconds of parsing the embedded STEP plus 31 seconds of tessellating 1170 solids, the
    /// tessellation running straight on the UI thread.
    #[test]
    fn open_uses_bundle_geometry_without_rebuild() {
        let dir = std::env::temp_dir().join("qym_open_fast_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("proj.qcad").to_string_lossy().into_owned();
        let _ = std::fs::remove_file(&path);

        let mut app = App::default();
        let cube = build_cube(&mut app);
        let tris_before = app.project.mesh_index(cube).map(|i| app.project.bodies[i].mesh.tris.len()).unwrap_or(0);
        assert!(tris_before > 0, "the block was built");
        app.project_path = Some(path.clone());
        app.save_project();
        app.wait_bg();

        // 1. the geometry is REALLY in the file (the features' bodies used to be thrown away, leaving a bundle with no meshes)
        let loaded = qymcad_io::load_project(&path).expect("the bundle reads");
        let li = loaded.mesh_index(cube).expect("the block's body was saved into the bundle");
        assert_eq!(loaded.bodies[li].mesh.tris.len(), tris_before, "the body's mesh lies in the file whole");
        assert!(!loaded.bodies[li].faces.is_empty(), "the B-rep faces arrived INSIDE the body");

        // 2. opening: not one dirty node and NOT ONE rebuilt body - only what the file shows
        let mut app2 = App::default();
        app2.finish_project_load(path.clone(), loaded, Vec::new());
        assert!(app2.project.timeline.iter().all(|n| !n.dirty), "after opening there is nothing to rebuild");
        assert!(app2.live.shapes.is_empty(), "the live B-rep was NOT built on opening (that is the whole point)");
        assert!(!app2.live.ready, "the B-rep cache is marked as one to be built on demand");
        let i2 = app2.project.mesh_index(cube).expect("the body is there");
        assert_eq!(app2.project.bodies[i2].mesh.tris.len(), tris_before, "what is on the screen is the geometry from the file");
        assert!(app2.project.regen_faces.contains_key(&cube), "the B-rep faces came back into the project (associativity)");

        // 3. the first operation that needs a B-rep brings the cache up itself (resolved on demand)
        app2.ensure_brep();
        assert!(app2.live.ready && app2.live.shapes.contains_key(&cube), "the B-rep was built on demand: {}", app2.status);
        let v: f64 = app2.live.shapes.get(&cube).map(|s| s.volume()).unwrap_or(0.0);
        assert!((v - 8000.0).abs() < 80.0, "and it is the same geometry: V={v:.0}");
        let _ = std::fs::remove_file(&path);
    }

    /// A SWEEP WITH AN OPERATION (a cut or an intersection), as a revolve has. It lives in the command layer only:
    /// the `Sweep` node does not store the operation, `feat_op` sets it through `finish_base_body`. So this is where
    /// it has to be checked: with the same profile and path, Add GIVES material and Cut TAKES it away.
    #[test]
    fn sweep_with_operation_flow() {
        use qymcad_core::feature::{BasePlane, SketchPlane};
        // a 20-cubed block + sweeping a circle of diameter 6 along a vertical 30 through it; the caller sets op
        let build = |op: u8| -> f64 {
            let mut app = App::default();
            let _cube = build_cube(&mut app);
            let sprof = app.project.new_sketch("the profile");
            let prof_sid = app.project.sketches[sprof].id;
            app.project.add_circle_entity(sprof, 10.0, 10.0, 3.0, qymcad_core::feature::Purpose::Real);
            app.project.regen_sketch(sprof);
            let spath = app.project.new_sketch("the path");
            let path_sid = app.project.sketches[spath].id;
            app.project.sketches[spath].plane = SketchPlane::World(BasePlane::XZ);
            app.project.add_line_entity(spath, 10.0, 0.0, 10.0, 30.0, qymcad_core::feature::Purpose::Real);
            app.project.regen_sketch(spath);
            app.sweep.prof_sid = prof_sid;
            app.sweep.path_sid = path_sid;
            app.feat.op = op;
            app.apply_sweep_cmd();
            app.regenerate_all();
            let consumed = app.consumed_bodies();
            app.live.shapes.iter().filter(|(b, _)| !consumed.contains(b)).map(|(_, s)| s.volume()).sum()
        };
        let (add, cut) = (build(0), build(2));
        assert!(add > 8000.0 + 1.0, "an Add sweep must GIVE material: V={add:.0} (the block is 8000)");
        assert!(cut < 8000.0 - 1.0, "a Cut sweep must TAKE material away: V={cut:.0} (the block is 8000)");
        // the tool is one and the same: outside the block (the boss) plus inside it (the cut) equals the whole swept
        // cylinder of diameter 6 and length 30 - that is how THE OPERATION itself gets checked, without depending on
        // where exactly the sweep convention places the profile along the path
        let full = std::f64::consts::PI * 9.0 * 30.0;
        let sum = (add - 8000.0) + (8000.0 - cut);
        assert!((sum - full).abs() < 0.03 * full, "the boss {:.0} plus the cut {:.0} is {sum:.0}, a cylinder of {full:.0} was expected", add - 8000.0, 8000.0 - cut);
    }

    /// A MATRIX of edits THROUGH THE COMMAND LAYER. The model layer is covered by the kernel's matrices
    /// (matrix_assoc, chain_edit, onface), but the complaint lived in the GUI layer, which is what this aims at:
    /// an edit under a chain of modifiers, an edit through a feature's dimension popup, an honest error when a
    /// support is removed (keep-last-good), an edit of a sketch AFTER the timeline was rolled back. The failures
    /// accumulate.
    #[test]
    fn sketch_edit_matrix_gui() {
        use qymcad_core::feature::FeatureKind;
        let mut fails: Vec<String> = Vec::new();
        let live_volume = |app: &App| -> f64 {
            let consumed = app.consumed_bodies();
            app.live.shapes.iter().filter(|(b, _)| !consumed.contains(b)).map(|(_, s)| s.volume()).sum()
        };
        // stretch sketch `si`'s base rectangle along X from 20 to `to`
        let stretch = |app: &mut App, si: usize, to: f64| {
            app.enter_sketch_edit(si);
            for pt in &mut app.project.sketches[si].points {
                if (pt.x - 20.0).abs() < 1e-9 {
                    pt.x = to;
                }
            }
            app.project.solve_sketch(si);
            app.project.regen_sketch(si);
            app.finish_sketch_edit();
        };

        // 1. UNDER A CHAIN: a block plus an edge fillet, then the base sketch is stretched. The fillet must survive
        //    the edit (the contour was not substituted, the edge resolves) and the node must not be red.
        {
            let mut app = App::default();
            let cube = build_cube(&mut app);
            let eid = app
                .project
                .regen_edges
                .get(&cube)
                .and_then(|es| es.iter().find(|e| e.a[0].abs() < 1e-6 && e.a[1].abs() < 1e-6 && (e.a[2] - e.b[2]).abs() > 1.0))
                .map(|e| e.id);
            if let Some(eid) = eid {
                app.start_feat_cmd(4); // an edge fillet
                app.gsel.edges.insert(eid);
                app.edges.body = Some(cube);
                if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "radius") {
                    p.val = 2.0;
                    p.txt = "2".into();
                }
                app.apply_feat_cmd();
                let v0 = live_volume(&app);
                if !(v0 > 7000.0 && v0 < 8000.0) {
                    fails.push(format!("the block's fillet did not build: V={v0:.0} (a little under 8000 was expected); status: {}", app.status));
                }
                stretch(&mut app, 0, 30.0);
                let v1 = live_volume(&app);
                if (v1 - (v0 + 4000.0)).abs() > 60.0 {
                    fails.push(format!("editing the sketch under a fillet: V={v1:.0}, {:.0} expected; status: {}", v0 + 4000.0, app.status));
                }
                if !app.project.regen_errors.is_empty() {
                    fails.push(format!("after the edit the nodes are red: {:?}", app.project.regen_errors.values().collect::<Vec<_>>()));
                }
            } else {
                fails.push("the block has no edges to fillet".into());
            }
        }

        // 2. EDITING A FEATURE'S DIMENSION through the popup (start_feat_cmd_edit) RIGHT AFTER a sketch edit -
        //    the previous command's stale parameters must not mix in.
        {
            let mut app = App::default();
            let _cube = build_cube(&mut app);
            stretch(&mut app, 0, 40.0); // 40×20×20 = 16000
            let fid = app.project.timeline.iter().find(|n| matches!(n.kind, FeatureKind::Extrude { .. })).map(|n| n.id);
            if let Some(fid) = fid {
                app.start_feat_cmd_edit(fid);
                if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
                    p.val = 10.0;
                    p.txt = "10".into();
                }
                app.apply_feat_cmd();
                let v = live_volume(&app);
                if (v - 8000.0).abs() > 80.0 {
                    fails.push(format!("editing the sketch and the height: V={v:.0}, 8000 expected (40x20x10); status: {}", app.status));
                }
            } else {
                fails.push("no extrude node was found".into());
            }
        }

        // 3. REMOVING A SUPPORT: the contour an extrude stands on is taken out of the sketch, so the node must go
        //    red HONESTLY while the body stays the last good one (keep-last-good) rather than vanishing silently.
        {
            let mut app = App::default();
            let _cube = build_cube(&mut app);
            let v0 = live_volume(&app);
            app.enter_sketch_edit(0);
            app.project.sketches[0].entities.clear();
            app.project.regen_sketch(0);
            app.finish_sketch_edit();
            if app.project.regen_errors.is_empty() {
                fails.push("removing the supporting contour passed SILENTLY (a red node was expected)".into());
            }
            let v1 = live_volume(&app);
            if (v1 - v0).abs() > 1.0 {
                fails.push(format!("the body did not hold on to the last good geometry: {v0:.0} -> {v1:.0}"));
            }
        }

        // 4. AN EDIT UNDER A ROLLBACK: the rollback line is raised above the cut, so editing the sketch rebuilds
        //    what is above the line and does NOT resurrect the cut the rollback suppressed.
        {
            let mut app = App::default();
            let _cube = build_cube(&mut app);
            let s2 = sketch_rect(&mut app, 5.0, 5.0, 15.0, 15.0);
            app.sel = Sel::Sketch(s2);
            app.feat.op = 2;
            app.start_feat_cmd(1);
            if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
                p.val = 5.0;
                p.txt = "5".into();
            }
            app.apply_feat_cmd();
            let cut_ti = app.project.timeline.iter().position(|n| matches!(n.kind, FeatureKind::Combine { .. }));
            if let Some(cut_ti) = cut_ti {
                app.project.rollback = Some(cut_ti); // the rollback sits BEFORE the cut
                app.regenerate_all();
                stretch(&mut app, 0, 30.0);
                let v = live_volume(&app);
                if (v - 12000.0).abs() > 120.0 {
                    fails.push(format!("an edit under a rollback: V={v:.0}, 12000 expected (the cut must not build); status: {}", app.status));
                }
            } else {
                fails.push("no cut node was found for the rollback".into());
            }
        }

        assert!(fails.is_empty(), "the GUI matrix of sketch edits:\n{}", fails.join("\n"));
    }

    /// Cancelling with Esc mid-command leaves NO stale state: the next extrude is clean.
    #[test]
    fn cancel_then_extrude_flow() {
        let mut app = App::default();
        let si = sketch_rect(&mut app, 0.0, 0.0, 20.0, 20.0);
        app.sel = Sel::Sketch(si);
        app.feat.op = 2; // a CUT was started (not allowed in an empty part, but the command can be opened)
        app.start_feat_cmd(1);
        app.cancel_feat_cmd(); // Esc
        assert_eq!(app.cmd.kind, 0, "the command was dropped");
        // now an honest extrude - nothing leaked from the cut
        app.sel = Sel::Sketch(si);
        app.feat.op = 0;
        app.start_feat_cmd(1);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 10.0;
            p.txt = "10".into();
        }
        app.apply_feat_cmd();
        let v = total_volume(&app);
        assert!((v - 4000.0).abs() < 40.0, "after Esc the extrude is clean: V={v:.0}; status: {}", app.status);
    }

    /// Editing a cut's DEPTH by a double click: 5 to 10, the volume is recomputed and no nodes are bred.
    #[test]
    fn edit_cut_depth_flow() {
        let mut app = App::default();
        let _cube = build_cube(&mut app);
        let s2 = sketch_rect(&mut app, 5.0, 5.0, 15.0, 15.0);
        app.sel = Sel::Sketch(s2);
        app.feat.op = 2;
        app.start_feat_cmd(1);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 5.0;
            p.txt = "5".into();
        }
        app.apply_feat_cmd();
        let nodes_before = app.project.timeline.len();
        let fid = app
            .project
            .timeline
            .iter()
            .find(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::Combine { .. }))
            .map(|n| n.id)
            .expect("the cut's node");
        app.start_feat_cmd_edit(fid);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 10.0;
            p.txt = "10".into();
        }
        app.apply_feat_cmd();
        assert_eq!(app.project.timeline.len(), nodes_before, "editing a cut breeds no nodes");
        let consumed = app.consumed_bodies();
        let v: f64 = app.live.shapes.iter().filter(|(b, _)| !consumed.contains(b)).map(|(_, s)| s.volume()).sum();
        assert!((v - 7000.0).abs() < 70.0, "the cut went 5 -> 10: V={v:.0} (8000 minus 1000); status: {}", app.status);
    }

    /// The Box primitive by button: the sizes, then Enter. Plus a cylinder that is ADDED into the same body (one part).
    #[test]
    fn primitive_flows() {
        let mut app = App::default();
        app.start_prim_cmd(10); // a box
        for (k, v) in [("dx", 20.0), ("dy", 20.0), ("dz", 20.0)] {
            if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == k) {
                p.val = v;
                p.txt = format!("{v}");
            }
        }
        app.apply_feat_cmd();
        assert_eq!(app.cmd.kind, 0, "the box was applied: {}", app.status);
        let consumed = app.consumed_bodies();
        let live: Vec<f64> = app.live.shapes.iter().filter(|(b, _)| !consumed.contains(b)).map(|(_, s)| s.volume()).collect();
        assert_eq!(live.len(), 1, "one body after the box");
        assert!((live[0] - 8000.0).abs() < 80.0, "a 20-cubed box: V={:.0}", live[0]);
    }

    /// A shell BY BUTTON: a body, a face, a thickness, then Enter (without entering the part).
    #[test]
    fn shell_button_flow() {
        let mut app = App::default();
        let cube = build_cube(&mut app);
        let top = app
            .project
            .regen_faces
            .get(&cube)
            .and_then(|fs| fs.iter().find(|f| f.normal[2] > 0.9))
            .map(|f| f.id)
            .expect("the top face");
        app.start_feat_cmd(6);
        assert_eq!(app.cmd.kind, 6, "the shell started: {}", app.status);
        app.gsel.faces.insert(top);
        app.gsel.faces_body = Some(cube);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "thickness") {
            p.val = 2.0;
            p.txt = "2".into();
        }
        app.apply_feat_cmd();
        assert_eq!(app.cmd.kind, 0, "the shell was applied: {}", app.status);
        let consumed = app.consumed_bodies();
        let v: f64 = app.live.shapes.iter().filter(|(b, _)| !consumed.contains(b)).map(|(_, s)| s.volume()).sum();
        let exp = 8000.0 - 16.0 * 16.0 * 18.0;
        assert!((v - exp).abs() < 30.0, "a shell of t2: V={v:.0}, {exp:.0} expected");
    }

    /// IMPOSSIBLE CASES: the commands must give a clear refusal or an honest error, neither rubbish nor silence.
    #[test]
    fn impossible_cases_are_honest() {
        // 1) extruding a sketch WITHOUT a closed contour (one line): a refusal at the start
        {
            let mut app = App::default();
            let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
            app.project.add_line_entity(si, 0.0, 0.0, 30.0, 0.0, qymcad_core::feature::Purpose::Real);
            app.project.regen_sketch(si);
            app.finish_sketch_edit();
            app.sel = Sel::Sketch(si);
            app.feat.op = 0;
            app.start_feat_cmd(1);
            assert_eq!(app.cmd.kind, 0, "the command does NOT start without a closed contour");
            assert_eq!(app.status, crate::i18n::tr("msg-no-closed-contour"), "a clear status");
        }
        // 2) a cut ENTIRELY CLEAR of the body: the node is created, but the rebuild must give an honest error or
        //    cut nothing at all (the volume does not change) - never a rubbish body
        {
            let mut app = App::default();
            let _cube = build_cube(&mut app);
            let s2 = sketch_rect(&mut app, 100.0, 100.0, 120.0, 120.0); // far from the block
            app.sel = Sel::Sketch(s2);
            app.feat.op = 2;
            app.start_feat_cmd(1);
            if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
                p.val = 5.0;
                p.txt = "5".into();
            }
            app.apply_feat_cmd();
            let consumed = app.consumed_bodies();
            let v: f64 = app.live.shapes.iter().filter(|(b, _)| !consumed.contains(b)).map(|(_, s)| s.volume()).sum();
            assert!((v - 8000.0).abs() < 80.0, "a cut that misses: the block neither changed nor multiplied, V={v:.0}; status: {}", app.status);
        }
        // 3) a shell thicker than half the body: an honest error on the node (red) and no rubbish body
        {
            let mut app = App::default();
            let cube = build_cube(&mut app);
            let top = app.project.regen_faces.get(&cube).and_then(|fs| fs.iter().find(|f| f.normal[2] > 0.9)).map(|f| f.id).unwrap();
            app.start_feat_cmd(6);
            app.gsel.faces.insert(top);
            app.gsel.faces_body = Some(cube);
            if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "thickness") {
                p.val = 15.0; // more than the half-body's 10
                p.txt = "15".into();
            }
            app.apply_feat_cmd();
            let consumed = app.consumed_bodies();
            let live: Vec<f64> = app.live.shapes.iter().filter(|(b, _)| !consumed.contains(b)).map(|(_, s)| s.volume()).collect();
            let errored = !app.project.regen_errors.is_empty();
            let sane = live.iter().all(|v| *v > 0.0 && *v <= 8000.0 + 1.0);
            assert!(sane || errored, "a shell of t15: either a valid body or an honest error; V={live:?}, err={errored}");
        }
        // 4) a fillet with a zero radius: a no-op or an honest refusal, but NOT a crash and not an empty body
        {
            let mut app = App::default();
            let cube = build_cube(&mut app);
            let eid = app.project.regen_edges.get(&cube).and_then(|es| es.first()).map(|e| e.id).unwrap();
            app.start_feat_cmd(4);
            app.gsel.edges.insert(eid);
            app.edges.body = Some(cube);
            if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "radius") {
                p.val = 0.0;
                p.txt = "0".into();
            }
            app.apply_feat_cmd();
            let consumed = app.consumed_bodies();
            let v: f64 = app.live.shapes.iter().filter(|(b, _)| !consumed.contains(b)).map(|(_, s)| s.volume()).sum();
            assert!(v > 7900.0 && v < 8100.0, "a radius of 0: the body is alive (V={v:.0}) and not rubbish; status: {}", app.status);
        }
    }

    /// A mirror by button: a body, the YZ plane, Enter with the original kept, and the volume doubles.
    #[test]
    fn mirror_button_flow() {
        let mut app = App::default();
        let _cube = build_cube(&mut app); // a block from 0 to 20, touching the YZ plane along the face at x=0
        app.start_feat_cmd(16);
        assert_eq!(app.cmd.kind, 16, "the mirror started: {}", app.status);
        app.mirror.plane = Some(qymcad_core::feature::SketchPlane::World(qymcad_core::feature::BasePlane::YZ));
        app.opts.mirror_keep = true;
        app.apply_feat_cmd();
        assert_eq!(app.cmd.kind, 0, "the mirror was applied: {}", app.status);
        let consumed = app.consumed_bodies();
        let v: f64 = app.live.shapes.iter().filter(|(b, _)| !consumed.contains(b)).map(|(_, s)| s.volume()).sum();
        assert!((v - 16000.0).abs() < 160.0, "a mirror keeping the original: V={v:.0}, 16000 expected");
    }

    /// A linear pattern by button: 3 copies at a step of 40 (disjoint), so three times the volume.
    #[test]
    fn linear_array_button_flow() {
        let mut app = App::default();
        let _cube = build_cube(&mut app);
        app.start_array_cmd(17);
        assert_eq!(app.cmd.kind, 17, "the pattern started: {}", app.status);
        app.arr.count = 3;
        app.arr.two = false;
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "step") {
            p.val = 40.0;
            p.txt = "40".into();
        }
        app.apply_feat_cmd();
        assert_eq!(app.cmd.kind, 0, "the pattern was applied: {}", app.status);
        let consumed = app.consumed_bodies();
        let v: f64 = app.live.shapes.iter().filter(|(b, _)| !consumed.contains(b)).map(|(_, s)| s.volume()).sum();
        assert!((v - 24000.0).abs() < 240.0, "three blocks: V={v:.0}");
    }

    /// THE SECTION view: half the scene is hidden, Flip shows the other half, Off brings everything back.
    #[test]
    fn section_view_hides_half() {
        let mut app = App::default();
        let _cube = build_cube(&mut app); // a block from 0 to 20
        let (full, _) = app.gpu_scene();
        assert!(!full.is_empty(), "the scene is not empty");
        // a section through the centre with a +X normal, so the x>10 half is hidden
        app.section.plane = Some(([10.0, 0.0, 0.0], [1.0, 0.0, 0.0]));
        let (half, _) = app.gpu_scene();
        assert!(!half.is_empty(), "the visible half is still there");
        // What is checked is THE BODY's clip: the section cap is not part of it - the cap is deliberately nudged a
        // hair beyond the plane, into the cut-away side, so that the clipped triangles of the thread turns do not
        // occlude it.
        let amber_cap = [u32::from_le_bytes([224, 168, 92, 255]), u32::from_le_bytes([176, 128, 66, 255])];
        for v in half.iter().filter(|v| !amber_cap.contains(&v.color)) {
            assert!(v.pos[0] <= 10.0 + 1e-3, "an honest clip: EVERY vertex of the visible part has x<=10 (the cut runs exactly along the plane): {}", v.pos[0]);
        }
        assert!(half.iter().any(|v| (v.pos[0] - 10.0).abs() < 1e-3), "there are vertices EXACTLY on the cutting plane");
        // THE CAP: amber vertices at the plane itself (the cut is filled and one cannot see straight through it).
        // A tolerance rather than exact equality: the cap is deliberately nudged a hair into the CUT-AWAY side -
        // there is no material left there, so the body's clipped triangles do not occlude it. Lying exactly in the
        // plane, it argued with them over depth, and on a thread the fill disappeared in patches.
        let amber = u32::from_le_bytes([224, 168, 92, 255]);
        let cap_verts = half.iter().filter(|v| v.color == amber && (v.pos[0] - 10.0).abs() < 0.05).count();
        assert!(cap_verts >= 3, "the section cap is present ({cap_verts} amber vertices at the plane)");
        let wrong_side = half.iter().filter(|v| v.color == amber).any(|v| v.pos[0] < 10.0 - 1e-6);
        assert!(!wrong_side, "the cap is nudged into the CUT-AWAY side (x >= 10): inside the material the thread turns occlude it");
        // flip: the other half becomes visible
        app.section.plane = Some(([10.0, 0.0, 0.0], [-1.0, 0.0, 0.0]));
        let (other, _) = app.gpu_scene();
        for v in other.iter().filter(|v| !amber_cap.contains(&v.color)) {
            assert!(v.pos[0] >= 10.0 - 1e-3, "after the flip every vertex has x>=10: {}", v.pos[0]);
        }
        // the offset moves the plane: a -X normal with an offset of 5 puts the plane at x=5 and shows x>=5
        app.section.offset = 5.0;
        let (shifted, _) = app.gpu_scene();
        let minx = shifted.iter().filter(|v| !amber_cap.contains(&v.color)).map(|v| v.pos[0]).fold(f32::MAX, f32::min);
        assert!((minx - 5.0).abs() < 1e-3, "the cut moved to x=5 (min x = {minx})");
        // off: everything is back
        app.section.plane = None;
        app.section.offset = 0.0;
        let (back, _) = app.gpu_scene();
        assert_eq!(back.len(), full.len(), "Off brings the whole scene back");
    }

    /// AUTOSAVE: an edit writes an autosave beside the project; that autosave reads back; Save removes it; and a
    /// clean state is not written again.
    #[test]
    fn autosave_cycle() {
        let dir = std::env::temp_dir().join("qym_autosave_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("proj.qcad").to_string_lossy().into_owned();
        let auto = std::path::Path::new(&path).with_extension("").to_string_lossy().into_owned() + ".autosave.qcad";
        let _ = std::fs::remove_file(&auto);
        let mut app = App::default();
        app.project_path = Some(path.clone());
        let _cube = build_cube(&mut app); // unsaved edits
        app.maybe_autosave(true);
        app.wait_bg(); // the write runs in the background, so the test synchronises explicitly
        assert!(std::path::Path::new(&auto).exists(), "the autosave was created: {}", app.status);
        let loaded = qymcad_io::load_project(&auto).expect("the autosave reads");
        assert_eq!(loaded.timeline.len(), app.project.timeline.len(), "the autosave holds the whole timeline");
        // forcing it again with no new edits does not rewrite it (the key matched)
        let m1 = std::fs::metadata(&auto).unwrap().modified().unwrap();
        app.maybe_autosave(true);
        app.wait_bg();
        let m2 = std::fs::metadata(&auto).unwrap().modified().unwrap();
        assert_eq!(m1, m2, "the same state is not written twice");
        // Save removes the autosave
        app.save_project();
        app.wait_bg();
        assert!(!std::path::Path::new(&auto).exists(), "after Save the autosave is gone");
        assert!(std::path::Path::new(&path).exists(), "the project was written: {}", app.status);
    }

    /// A revolve from the button (feat_op is reset at the start) - the full flow.
    #[test]
    fn revolve_flow() {
        let mut app = App::default();
        let si = {
            let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
            app.project.add_rect_entity(si, 10.0, 0.0, 20.0, 30.0, qymcad_core::feature::Purpose::Real);
            app.project.regen_sketch(si);
            app.finish_sketch_edit();
            si
        };
        app.sel = Sel::Sketch(si);
        app.feat.op = 2; // stale from a previous cut - the revolve must reset it to 0
        app.start_feat_cmd(3);
        assert_eq!(app.feat.op, 0, "the revolve starts on Add");
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "angle") {
            p.val = 360.0;
            p.txt = "360".into();
        }
        app.apply_feat_cmd();
        let v = total_volume(&app);
        let exp = std::f64::consts::PI * 300.0 * 30.0;
        assert!((v - exp).abs() / exp < 0.02, "the revolved tube: V={v:.0}, {exp:.0} expected; status: {}", app.status);
    }

    /// Reported behaviour: only the outer, visible faces one can reach with the mouse should be selectable. A
    /// neighbouring part's ghost body (shown under "in context") is visible translucently in the background but
    /// must NOT be pickable as a plane or a face for a mirror or a section - it is another part's reference
    /// geometry, not one's own.
    #[test]
    fn ghost_body_face_not_pickable_for_plane_pick() {
        let mut app = App::default();
        let cube_a = build_cube(&mut app); // a block from 0 to 20 in part A (the root)
        let comp_a = app.project.body_owner(cube_a).expect("A's owner");
        let comp_b = app.project.add_part("B");
        app.enter_component(comp_b); // the context is now B (a neighbour of A, neither ancestor nor descendant)
        app.win.context = true;
        // the camera aims EXACTLY at the centre of block A's top face, so project3(target) equals rect.center()
        app.cam.target = [10.0, 10.0, 20.0];
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        let mi = app.project.mesh_index(cube_a).expect("block A's mesh");
        assert!(app.body_shown(mi), "block A is shown in the background (show_context=true)");
        assert!(app.body_is_ghost(mi), "block A is a ghost seen from context B (a neighbour, not one's own)");
        app.mirror.part = Some(comp_a);
        // there is nothing behind the ghost at this point - the pick either misses or (plausibly) falls through it
        // to a base plane; what matters is that it is NOT a face of ghost A
        let hit = app.pick_sketch_plane_at(rect, rect.center());
        if let Some(qymcad_core::feature::SketchPlane::Face(body, _)) = hit {
            assert_ne!(body, cube_a, "a GHOST's face (A) must not be pickable in mirror mode: {hit:?}");
        }
    }

    /// A top-down reference: a sketch placed on a face of a NEIGHBOURING part's body is a LIVE external reference
    /// rather than a one-off copy. `resolve_placement_plane` used to substitute a fixed datum plane for the face
    /// silently: the coordinates were right, but editing the neighbour no longer drove the part - which is exactly
    /// what the "in context" mode exists for.
    #[test]
    fn sketch_on_neighbour_face_is_live_external_ref() {
        use qymcad_core::feature::{FaceKey, SketchPlane};
        let mut app = App::default();
        let cube = build_cube(&mut app); // part A (a block from 0 to 20)
        let comp_a = app.project.body_owner(cube).expect("A's owner");
        // block A's top face (a +Z normal), taken from the rebuild's data, as the viewport's pick does
        let faces = app.project.regen_faces.get(&cube).cloned().expect("the block's faces");
        let (fi, face) = faces.iter().enumerate().max_by(|a, b| a.1.normal[2].partial_cmp(&b.1.normal[2]).unwrap()).expect("the +Z face");
        let key = FaceKey { index: fi as u32, centroid: [face.centroid.x, face.centroid.y, face.centroid.z], normal: face.normal, id: face.id };

        // B is a NEIGHBOUR of A (both sit in the root assembly) rather than nested inside it: moving A must change
        // their relative position, otherwise the test checks nothing
        app.project.set_active_component(Some(app.project.root));
        let part_b = app.project.add_part("B");
        app.enter_component(part_b);
        let si = app.create_sketch_on(SketchPlane::Face(cube, key));
        app.finish_sketch_edit();

        // 1. the plane stayed a LIVE face of the neighbour, and the cross-reference is authorised
        assert!(matches!(app.project.sketches[si].plane, SketchPlane::Face(b, _) if b == cube), "the sketch's plane is the neighbour's face, not a copy: {:?}", app.project.sketches[si].plane);
        let rid = app.project.external_ref_for(part_b, cube).expect("the external reference is registered").id;
        let f0 = app.project.sketch_frame(si).expect("the frame computes (the authorisation is there)");

        // 2. the neighbour moved and the sketch went with it (that is what top-down means)
        let mut mat = qymcad_core::feature::PLACE_IDENTITY;
        mat[3] = 25.0;
        app.project.set_component_transform(comp_a, mat);
        let f1 = app.project.sketch_frame(si).expect("the frame after the neighbour moved");
        assert!((f1.origin[0] - f0.origin[0] - 25.0).abs() < 1e-9, "the sketch did not follow the neighbour: {:?} -> {:?}", f0.origin, f1.origin);

        // 3. breaking the link from the part's properties freezes the geometry in place and leaves the part valid
        assert_eq!(app.project.break_external_ref(rid), 1, "one sketch was frozen");
        assert!(matches!(app.project.sketches[si].plane, SketchPlane::Datum(_)), "after the break it is a datum copy");
        let f2 = app.project.sketch_frame(si).expect("the copy's frame");
        for k in 0..3 {
            assert!((f2.origin[k] - f1.origin[k]).abs() < 1e-9, "the break moved the sketch: {:?} -> {:?}", f1.origin, f2.origin);
        }
    }

    /// STEP and STL come from ONE parse of the bodies and say the same thing about what was skipped. The scene
    /// holds all three cases at once: a live B-rep, an imported mesh (there never was a B-rep) and a body whose
    /// rebuild failed (the recipe is there, the B-rep is lost). STEP used to throw the last two away silently while
    /// STL just as silently wrote them out as meshes - two files of one project disagreed on their contents.
    #[test]
    fn export_plan_is_shared_by_step_and_stl() {
        let mut app = App::default();
        let brep = build_cube(&mut app); // part one, with a live B-rep

        // part two with a LOST B-rep (emulating a failed rebuild: the mesh of the last good build remained)
        let part2 = app.project.add_part("Part two");
        app.enter_component(part2);
        let before: std::collections::HashSet<Id> = app.live.shapes.keys().copied().collect();
        let si = sketch_rect(&mut app, 40.0, 0.0, 60.0, 20.0);
        app.sel = Sel::Sketch(si);
        app.feat.op = 0;
        app.start_feat_cmd(1);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 20.0;
            p.txt = "20".into();
        }
        app.apply_feat_cmd();
        let consumed = app.consumed_bodies();
        let stale = *app.live.shapes.keys().find(|b| !before.contains(b) && !consumed.contains(b)).expect("part two's body");
        app.live.shapes.remove(&stale); // the B-rep is gone, the node is red and the mesh stayed on screen

        // An STL import: a mesh body with no timeline node
        let mut mesh = qymcad_core::geom::Mesh::default();
        mesh.verts.extend([Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 0.0, 0.0), Point3::new(0.0, 1.0, 0.0)]);
        mesh.tris.push([0, 1, 2]);
        app.add_bodies(vec![(mesh, Vec::new())]);
        let mesh_only = app.project.bodies.last().map(|b| b.id).expect("the imported body");

        let plan = app.export_plan(ExportTarget::Project);
        assert_eq!(plan.brep, vec![brep], "only a live B-rep goes into STEP");
        assert_eq!(plan.mesh_only, vec![mesh_only], "an STL import is a category of its own, not an error");
        assert_eq!(plan.stale, vec![stale], "a body with a recipe but no B-rep is a failed rebuild");
        let stl = plan.stl_bodies();
        assert_eq!(stl.len(), 3, "STL exports EVERYTHING visible: {stl:?}");
        for b in [brep, mesh_only, stale] {
            assert!(stl.contains(&b), "body {b} must land in the STL");
        }

        // both statuses name THE DIFFERENCE rather than keeping quiet about it
        let (step_note, stl_note) = (plan.note(true), plan.note(false));
        for (who, note) in [("STEP", &step_note), ("STL", &stl_note)] {
            assert!(note.contains(&crate::i18n::tr1("note-mesh-bodies", "n", "1")), "{who} must name the mesh body: {note}");
            assert!(note.contains(&crate::i18n::tr1("note-failed-bodies", "n", "1")), "{who} must name the failed rebuild: {note}");
        }
        assert!(step_note.contains(&crate::i18n::tr_prefix("note-not-exported", "what")), "STEP says those bodies are NOT in the file: {step_note}");
        assert!(stl_note.contains(&crate::i18n::tr_prefix("note-as-mesh", "what")), "STL says those bodies came out AS A MESH: {stl_note}");

        // a project with no defective bodies gets no note at all (nothing to alarm anyone about)
        let clean = App::default();
        assert!(clean.export_plan(ExportTarget::Project).note(true).is_empty(), "a clean project carries no note");
    }

    /// A regression of the previous test. Reported behaviour: face selection inside a Part broke while "in context"
    /// was on, which is exactly how a sketch is built off another part. THE SAME scene as in that test, but WITHOUT
    /// mirror or section mode - this is an ordinary plane pick (New sketch, or Plane from a face). A ghost's face
    /// MUST be pickable: the ghosts are shown precisely for the top-down case where a sketch on a neighbour's face
    /// becomes an external reference.
    #[test]
    fn ghost_body_face_is_pickable_for_plain_sketch_plane_pick() {
        let mut app = App::default();
        let cube_a = build_cube(&mut app); // a block from 0 to 20 in part A (the root)
        let comp_b = app.project.add_part("B");
        app.enter_component(comp_b); // we are inside the neighbouring part B
        app.win.context = true;
        app.cam.target = [10.0, 10.0, 20.0]; // the centre of block A's top face is under the cursor
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        let mi = app.project.mesh_index(cube_a).expect("block A's mesh");
        assert!(app.body_is_ghost(mi), "block A is a ghost seen from context B");
        assert!(app.mirror.part.is_none() && !app.section.pick, "an ordinary pick, not a mirror or a section");
        let hit = app.pick_sketch_plane_at(rect, rect.center());
        match hit {
            Some(qymcad_core::feature::SketchPlane::Face(body, _)) => {
                assert_eq!(body, cube_a, "the pick must land on ghost A's face rather than on some other body");
            }
            other => panic!("ghost A's face must be pickable for an ordinary sketch, got {other:?}"),
        }
    }

    /// What was asked for: only the datums of the assembly one is currently in should be drawn, not those from
    /// nested assemblies and parts. A datum plane created inside ANOTHER component (not the current context, be it
    /// a direct child or a deeper descendant) is NOT pickable and is NOT drawn in mirror or section mode. Only the
    /// context's OWN datums are visible - entering the owning Part makes it and its own datum visible.
    #[test]
    fn nested_datum_not_pickable_outside_owning_context() {
        use qymcad_core::model::{PlaneDef, WorkPlane};
        let mut app = App::default();
        let asm = app.project.add_assembly("Assembly");
        app.project.set_active_component(Some(asm));
        let part = app.project.add_part("Part");
        app.project.set_active_component(Some(part));
        let wp = WorkPlane { name: "Datum".into(), origin: [0.0, 0.0, 5.0], normal: [0.0, 0.0, 1.0], def: PlaneDef::Manual, ..Default::default() };
        let datum_id = app.project.add_plane(wp);
        app.project.active_component = Some(app.project.root);
        app.mirror.part = Some(asm); // the pick mode is active
        // from THE ROOT: the Part (the datum's owner) is not the current context, so it is hidden
        assert!(app.datum_render_transform(datum_id).is_none(), "another component's datum is not visible from the root");
        // from the Assembly (the Part's direct parent, but NOT the owner itself) it is hidden too
        app.active_path = vec![app.project.root, asm];
        assert!(app.datum_render_transform(datum_id).is_none(), "a DIRECT child's datum is NOT visible - it is not this context's own");
        // entering the Part (the owner itself) makes it visible
        app.active_path = vec![app.project.root, asm, part];
        assert!(app.datum_render_transform(datum_id).is_some(), "the owning context's own datum is visible");
    }
}

#[cfg(test)]
mod section_clip_tests {
    use super::smallvec_tris::clip_by_dists;

    /// A triangle crossing the plane: the cut runs EXACTLY along it (the new vertices sit at d=0) and the area is preserved.
    #[test]
    fn crossing_triangle_cut_exactly_on_plane() {
        // the vertices have d = [-1, -1, +2]: two visible and one cut away, giving a quadrilateral (4 fan vertices)
        let v = [[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [0.0, 9.0, 0.0]];
        let out = clip_by_dists(v, [-1.0, -1.0, 2.0]);
        assert!(!out.whole);
        assert_eq!(out.verts.len(), 4, "a quad of 4 vertices (2 original + 2 on the plane)");
        // the cut points carry weights that give d=0: w.d = 0
        for cv in &out.verts {
            let d = cv.w[0] * -1.0 + cv.w[1] * -1.0 + cv.w[2] * 2.0;
            assert!(d <= 1e-12, "the vertex is on the visible side or on the plane: d={d}");
        }
        // one visible vertex gives a triangle of 3
        let out2 = clip_by_dists(v, [-1.0, 2.0, 2.0]);
        assert_eq!(out2.verts.len(), 3);
        // wholly visible, or wholly hidden
        assert!(clip_by_dists(v, [-1.0, -1.0, -1.0]).whole);
        assert!(clip_by_dists(v, [1.0, 1.0, 1.0]).verts.is_empty());
    }
}

#[cfg(test)]
mod gizmo_ring_sign_tests {
    use super::ring_drag_sign;

    /// The contract: with the axis TOWARDS THE VIEWER (a negative depth component), a visually counter-clockwise
    /// drag gives a positive angle (the right-hand rule); with the axis INTO THE SCREEN, the same visual
    /// counter-clockwise drag gives a negative one. It used to be the other way round (the part turned against the
    /// drag, and dragging to the right gave negative degrees).
    #[test]
    fn ccw_toward_viewer_is_positive() {
        assert_eq!(ring_drag_sign(-1.0), 1.0, "axis towards the viewer: counter-clockwise is positive");
        assert_eq!(ring_drag_sign(1.0), -1.0, "axis into the screen: counter-clockwise is negative");
    }
}

#[cfg(test)]
mod section_drag_tests {
    use super::section_drag_delta_offset;
    use egui::Pos2;

    /// Reported behaviour: the plane reset every time the gizmo was grabbed. At the moment of the grab the cursor
    /// stands NOT at o0 (offset=0) but at THE ARROW'S TIP - already far from o0, by both the current offset and the
    /// arrow's length. The old formula (an absolute reprojection from o0) jumped by that distance on the very first
    /// frame. A delta from the anchor must give EXACTLY off0 (no change) while the cursor has not moved.
    #[test]
    fn no_jump_at_grab_regardless_of_arrow_length() {
        let s0 = Pos2::new(400.0, 300.0); // the screen projection of o0 (offset=0)
        let s1 = Pos2::new(450.0, 300.0); // the projection of o0 plus the normal (50px = 1 mm along the normal on screen)
        // the cursor is GRABBED far from s0 (at the arrow, with 30 mm of offset already accumulated plus the arrow's 80px)
        let p0 = Pos2::new(1200.0, 300.0);
        let off0 = 30.0;
        let new_off = section_drag_delta_offset(off0, p0, s0, s1, p0).expect("not degenerate");
        assert!((new_off - off0).abs() < 1e-9, "the cursor has not moved since the grab, so the offset did not change: {new_off} (was {off0})");
    }

    /// From then on the drag is proportional to the mouse's movement from the anchor (in px per mm from s0/s1), not from o0.
    #[test]
    fn tracks_cursor_movement_from_anchor() {
        let s0 = Pos2::new(400.0, 300.0);
        let s1 = Pos2::new(450.0, 300.0); // 50px = 1 mm
        let p0 = Pos2::new(1200.0, 300.0);
        let off0 = 30.0;
        let cur = Pos2::new(1250.0, 300.0); // +50px from the anchor is +1 mm
        let new_off = section_drag_delta_offset(off0, p0, s0, s1, cur).unwrap();
        assert!((new_off - 31.0).abs() < 1e-9, "a movement of +50px (1 mm) from the anchor gives an offset of 30+1=31: {new_off}");
    }

    /// The degenerate case (the section's normal points straight at the camera, so s1 equals s0 on screen) does not panic.
    #[test]
    fn degenerate_zero_length_screen_normal_returns_none() {
        let s0 = Pos2::new(400.0, 300.0);
        assert!(section_drag_delta_offset(0.0, s0, s0, s0, s0).is_none());
    }
}

#[cfg(test)]
mod pick_depth_tests {
    use super::tri_depth_at;
    use egui::Pos2;

    /// Thin 2 mm walls: a click near the FAR edge of a large triangle on the outer face. The mean depth of the outer
    /// triangle's vertices, (0+0+30)/3=10, is DEEPER than the centroid of the small inner triangle (2), so the old
    /// pick returned the inner wall. The barycentric depth AT THE CLICK POINT must return the outer one (0.6 < 2.0).
    #[test]
    fn thin_wall_outer_face_wins_at_click_point() {
        // the outer face: a huge triangle tilted away into the depth (vertex C is far off)
        let (a, da) = (Pos2::new(0.0, 0.0), 0.0);
        let (b, db) = (Pos2::new(100.0, 0.0), 0.0);
        let (c, dc) = (Pos2::new(0.0, 100.0), 30.0);
        // the inner face (2 mm deeper at the click point): a small triangle around the click
        let (a2, da2) = (Pos2::new(5.0, -5.0), 2.0);
        let (b2, db2) = (Pos2::new(25.0, 15.0), 2.0);
        let (c2, dc2) = (Pos2::new(5.0, 15.0), 2.0);
        let click = Pos2::new(10.0, 2.0); // inside both
        let outer_depth = tri_depth_at(click, a, da, b, db, c, dc); // about 0.6 (the click is near the close edge)
        let inner_depth = tri_depth_at(click, a2, da2, b2, db2, c2, dc2); // 2.0
        assert!(outer_depth < inner_depth, "the outer wall is nearer AT THE CLICK POINT: {outer_depth:.2} < {inner_depth:.2}");
        // the old (centroid) criterion chose WRONGLY - this records that the fault was real
        let outer_centroid = (da + db + dc) / 3.0;
        assert!(outer_centroid > inner_depth, "the centroid criterion gave the outer face a depth of {outer_centroid:.2} > {inner_depth:.2}, hence the wrong pick (the proof of the fault)");
    }

    /// A flat triangle (a constant depth): the interpolation matches the mean.
    #[test]
    fn flat_triangle_depth_constant() {
        let d = tri_depth_at(Pos2::new(3.0, 3.0), Pos2::new(0.0, 0.0), 7.0, Pos2::new(10.0, 0.0), 7.0, Pos2::new(0.0, 10.0), 7.0);
        assert!((d - 7.0).abs() < 1e-9);
    }
}

#[cfg(test)]
mod gizmo_math_tests {
    use super::{compose12, rot_about_point};
    use qymcad_core::feature::{apply12, is_identity12, PLACE_IDENTITY};

    fn close(a: [f64; 3], b: [f64; 3]) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < 1e-9)
    }

    #[test]
    fn compose_identity_is_noop() {
        let t = [1.0, 0.0, 0.0, 4.0, 0.0, 1.0, 0.0, 5.0, 0.0, 0.0, 1.0, 6.0]; // a translation of (4,5,6)
        assert_eq!(compose12(&PLACE_IDENTITY, &t), t, "I∘T = T");
        assert_eq!(compose12(&t, &PLACE_IDENTITY), t, "T∘I = T");
    }

    #[test]
    fn compose_translations_add() {
        let a = [1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 2.0, 0.0, 0.0, 1.0, 3.0];
        let b = [1.0, 0.0, 0.0, 4.0, 0.0, 1.0, 0.0, 5.0, 0.0, 0.0, 1.0, 6.0];
        let c = compose12(&a, &b); // the two translations add up
        assert!(close(apply12(&c, [0.0, 0.0, 0.0]), [5.0, 7.0, 9.0]), "the translations add up");
    }

    #[test]
    fn sketch_origin_uv_invariant_under_placement() {
        // the snapped origin is stored as (u,v) in the frame's axes. project(snap) on the WORLD frame gives (u,v),
        // and lift(u,v) on the LOCAL frame plus the body's placement reproduces the same world point.
        use qymcad_core::feature::PlaneFrame;
        use qymcad_core::geom::{Point2, Point3};
        let f_local = PlaneFrame::world_aligned([3.0, 4.0, 2.0], [0.0, 0.0, 1.0], 0.0);
        let wt = compose12(&rot_about_point(2, 90.0, [0.0, 0.0, 0.0]), &[1.0, 0.0, 0.0, 10.0, 0.0, 1.0, 0.0, 20.0, 0.0, 0.0, 1.0, 30.0]);
        let f_world = f_local.transformed(&wt);
        let (u, v) = (5.0, -7.0);
        let w = f_world.lift(Point2::new(u, v)); // the snapped world point on the plane
        let uv = f_world.project(Point3::new(w.x, w.y, w.z)); // pick→project
        assert!((uv.x - u).abs() < 1e-9 && (uv.y - v).abs() < 1e-9, "project after lift is the identity in the world frame");
        let o_local = f_local.lift(uv); // as in sketch_frame: the local origin is shifted
        let o_world = apply12(&wt, [o_local.x, o_local.y, o_local.z]);
        assert!(close(o_world, [w.x, w.y, w.z]), "u*X + v*Y is invariant to the body's placement");
    }

    #[test]
    fn rot_z90_about_origin() {
        let m = rot_about_point(2, 90.0, [0.0, 0.0, 0.0]); // 90 degrees about Z
        assert!(close(apply12(&m, [1.0, 0.0, 0.0]), [0.0, 1.0, 0.0]), "X goes to Y at +90 degrees about Z");
        assert!(close(apply12(&m, [0.0, 0.0, 5.0]), [0.0, 0.0, 5.0]), "Z stays put");
    }

    #[test]
    fn rot_about_point_keeps_center_fixed() {
        let c = [10.0, 20.0, 0.0];
        let m = rot_about_point(2, 37.0, c);
        assert!(close(apply12(&m, c), c), "the centre of rotation stays put");
        // accumulating by composition (as during a repeated drag): two 45-degree turns make 90
        let m45 = rot_about_point(2, 45.0, [0.0, 0.0, 0.0]);
        let m90 = compose12(&m45, &m45);
        assert!(close(apply12(&m90, [1.0, 0.0, 0.0]), [0.0, 1.0, 0.0]), "45°∘45° = 90°");
        assert!(!is_identity12(&m90), "not the identity");
    }

    #[test]
    fn rot_x_and_y_axes() {
        // 90 degrees about X takes Y to Z
        let mx = rot_about_point(0, 90.0, [0.0, 0.0, 0.0]);
        assert!(close(apply12(&mx, [0.0, 1.0, 0.0]), [0.0, 0.0, 1.0]), "about X: Y goes to Z");
        // 90 degrees about Y takes Z to X
        let my = rot_about_point(1, 90.0, [0.0, 0.0, 0.0]);
        assert!(close(apply12(&my, [0.0, 0.0, 1.0]), [1.0, 0.0, 0.0]), "about Y: Z goes to X");
    }

    #[test]
    fn straight_poly_accepts_line_rejects_arc() {
        use super::is_straight_poly;
        // a straight edge (several points on one line) works as an axis
        let line = [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0], [4.0, 0.0, 0.0]];
        assert!(is_straight_poly(&line), "collinear points make a straight line");
        // a quarter arc of R=10 is NOT an axis (its chord would become a false one)
        let arc: Vec<[f32; 3]> = (0..=8).map(|i| { let a = std::f64::consts::FRAC_PI_2 * i as f64 / 8.0; [10.0 * a.cos() as f32, 10.0 * a.sin() as f32, 0.0] }).collect();
        assert!(!is_straight_poly(&arc), "an arc is not straight");
        // the degenerate cases (one point, or zero length) are not axes
        assert!(!is_straight_poly(&[[1.0f32, 1.0, 1.0]]));
        assert!(!is_straight_poly(&[[1.0f32, 1.0, 1.0], [1.0, 1.0, 1.0]]));
    }

    #[test]
    fn axis_segment_centers_on_origin_along_dir() {
        use super::axis_segment;
        // an axis along Y through (0,5,0) of length +/-10 gives the ends (0,-5,0) and (0,15,0); dir is normalised
        let (a, b) = axis_segment([0.0, 5.0, 0.0], [0.0, 2.0, 0.0], 10.0);
        assert!(close(a, [0.0, -5.0, 0.0]) && close(b, [0.0, 15.0, 0.0]), "plus and minus len*dir from the origin: {a:?} {b:?}");
    }

    #[test]
    fn camera_basis_top_view_stays_orthonormal() {
        use super::Cam3;
        // THE TOP VIEW (a pitch of a quarter turn): the basis used to degenerate here (the cross product with the world Z is zero, giving NaN and a collapsed picture).
        let mut cam = Cam3::default();
        cam.pitch = std::f64::consts::FRAC_PI_2;
        let (r, u, f) = cam.basis();
        let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let len = |a: [f64; 3]| dot(a, a).sqrt();
        for v in [r, u, f] {
            assert!((len(v) - 1.0).abs() < 1e-6 && v.iter().all(|x| x.is_finite()), "a unit, finite vector: {v:?}");
        }
        assert!(dot(r, u).abs() < 1e-6 && dot(r, f).abs() < 1e-6 && dot(u, f).abs() < 1e-6, "the top view's basis is orthogonal");
        assert!(f[2] < -0.99, "a camera above looks downwards (-Z): {f:?}");
    }

    #[test]
    fn ray_plane_hits_and_skips_parallel() {
        use super::ray_plane;
        // a ray from (2,3,10) downwards along -Z meets the plane Z=0 at (2,3,0)
        let hit = ray_plane([2.0, 3.0, 10.0], [0.0, 0.0, -1.0], [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
        assert!(hit.is_some_and(|w| close(w, [2.0, 3.0, 0.0])), "the intersection with Z=0: {hit:?}");
        // an offset plane at Z=5 gives (2,3,5)
        let hit2 = ray_plane([2.0, 3.0, 10.0], [0.0, 0.0, -1.0], [0.0, 0.0, 5.0], [0.0, 0.0, 1.0]);
        assert!(hit2.is_some_and(|w| close(w, [2.0, 3.0, 5.0])), "the intersection with Z=5: {hit2:?}");
        // a ray parallel to the plane (along X, with a Z normal) gives None
        assert!(ray_plane([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]).is_none(), "parallel means no intersection");
    }
}

#[cfg(test)]
mod sketch_conflict_ui_tests {
    //! ARGUING CONSTRAINTS IN THE SKETCHER: what a person sees. The core has long been able to name the arguing
    //! set, but it reached the viewport and the panel only through the dimensions - the geometric constraints
    //! argued silently while their glyph stayed green. What runs here is exactly the functions the highlight is
    //! drawn from.
    use super::{App, Rect};
    use qymcad_core::model::Constraint;

    /// A segment with two incompatible lengths: 30 and 50.
    fn app_with_conflict() -> (App, usize) {
        let mut app = App::default();
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_line_entity(si, 0.0, 0.0, 30.0, 0.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        let (a, b) = (app.project.sketches[si].points[0].id, app.project.sketches[si].points[1].id);
        let s = &mut app.project.sketches[si];
        s.constraints.push(Constraint::Fixed { p: a });
        s.constraints.push(Constraint::Horizontal { a, b });
        s.constraints.push(Constraint::Distance { a, b, d: 30.0, off: 0.0, expr: String::new(), driven: false, axis: 0 });
        s.constraints.push(Constraint::Distance { a, b, d: 50.0, off: 0.0, expr: String::new(), driven: false, axis: 0 });
        app.project.solve_sketch(si);
        (app, si)
    }

    /// A segment whose GEOMETRIC constraints argue: horizontal plus vertical against a given length.
    /// Those are the ones with glyphs - the dimensions are always drawn in the viewport and the toggle does not hide them.
    fn app_with_geometric_conflict() -> (App, usize) {
        let mut app = App::default();
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_line_entity(si, 0.0, 0.0, 30.0, 0.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        let (a, b) = (app.project.sketches[si].points[0].id, app.project.sketches[si].points[1].id);
        let s = &mut app.project.sketches[si];
        s.constraints.push(Constraint::Fixed { p: a });
        s.constraints.push(Constraint::Distance { a, b, d: 30.0, off: 0.0, expr: String::new(), driven: false, axis: 0 });
        s.constraints.push(Constraint::Horizontal { a, b });
        s.constraints.push(Constraint::Vertical { a, b });
        app.project.solve_sketch(si);
        (app, si)
    }

    /// The arguing set reaches the UI, and the geometry it holds is known by name - the red highlight in the
    /// viewport is drawn from it.
    #[test]
    fn conflict_set_reaches_the_ui_with_its_geometry() {
        let (app, si) = app_with_conflict();
        let diag = app.sketch_diag(si);
        assert!(diag.conflicts.len() >= 2, "the arguing set is visible to the UI: {:?}", diag.conflicts);
        let mut hot: std::collections::HashSet<qymcad_core::model::Id> = std::collections::HashSet::new();
        for &ci in &diag.conflicts {
            hot.extend(app.project.sketch_constraint_points(si, ci));
        }
        assert_eq!(hot.len(), 2, "both ends of the arguing segment are lit: {hot:?}");
    }

    /// AN ERROR CANNOT BE SWITCHED OFF BY A CHECKBOX: the "show constraints" toggle hides the ordinary glyphs, but not the arguing ones.
    #[test]
    fn conflicting_glyphs_stay_visible_with_the_toggle_off() {
        let (mut app, si) = app_with_geometric_conflict();
        let rect = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
        app.win.constraints = true;
        let all = app.visible_constraint_glyphs(rect, si).len();
        app.win.constraints = false;
        let shown = app.visible_constraint_glyphs(rect, si);
        assert!(all > shown.len(), "the toggle hides the ordinary glyphs: there were {all}, now {}", shown.len());
        assert!(!shown.is_empty(), "but the arguing ones stay visible with the toggle off");
        let conflicts = app.sketch_diag(si).conflicts;
        assert!(shown.iter().all(|(ci, _, _)| conflicts.contains(ci)), "EXACTLY the arguing ones are shown");
    }

    /// WITH NO ARGUMENT, a switched-off toggle shows nothing (the highlight does not stick).
    #[test]
    fn no_conflict_means_nothing_is_forced_on_screen() {
        let (mut app, si) = app_with_geometric_conflict();
        // remove ANY constraint from the arguing set - by contract that is enough
        let ci = *app.sketch_diag(si).conflicts.iter().next().expect("there is an argument");
        app.project.delete_sketch_constraint(si, ci);
        app.project.solve_sketch(si);
        assert!(app.sketch_diag(si).conflicts.is_empty(), "the argument is resolved");
        app.win.constraints = false;
        let rect = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
        assert!(app.visible_constraint_glyphs(rect, si).is_empty(), "with no argument the toggle is in charge again");
    }

    /// A ONE-CLICK RESOLUTION: the "make it a reference" button ends the argument and leaves the dimension on the drawing.
    #[test]
    fn making_the_conflicting_dimension_driven_from_the_panel_resolves_it() {
        let (mut app, si) = app_with_conflict();
        let ci = *app.sketch_diag(si).conflicts.iter().max().expect("there is an argument");
        let before = app.project.sketches[si].constraints.len();
        assert!(app.make_dim_driven(si, ci), "the button worked: {}", app.status);
        assert!(app.sketch_diag(si).conflicts.is_empty(), "the argument is over: {:?}", app.sketch_diag(si).conflicts);
        assert_eq!(app.project.sketches[si].constraints.len(), before, "the dimension stayed on the drawing rather than being deleted");
        assert!(app.project.sketches[si].constraints[ci].is_driven(), "it is a reference dimension now");
    }

    /// THE DIAGNOSTICS CACHE DOES NOT LIE: editing the constraints changes the fingerprint, so the set is recomputed rather than sticking.
    #[test]
    fn the_diagnostic_cache_follows_edits() {
        let (mut app, si) = app_with_conflict();
        assert!(!app.sketch_diag(si).conflicts.is_empty(), "there is an argument");
        assert!(!app.sketch_diag(si).conflicts.is_empty(), "a second call (from the cache) answers the same");
        app.project.delete_sketch_constraint(si, 3);
        app.project.solve_sketch(si);
        assert!(app.sketch_diag(si).conflicts.is_empty(), "after the edit the cache was recomputed rather than handing back the old answer");
    }
}

#[cfg(test)]
mod brep_warmup_tests {
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
        app.ensure_brep();
        assert!(app.regen.wanted, "the first frame asks for a rebuild");
        // the request has been taken up and the overlay is spinning. While it spins, THE GEOMETRY REVISION may
        // move - any background task moves it (and so does the rebuild itself, once it arrives).
        app.regen.wanted = false;
        app.invalidate();
        app.ensure_brep();
        assert!(!app.regen.wanted, "no second request goes out: the preparation is ALREADY under way - otherwise the overlay flickers");
        app.invalidate();
        app.ensure_brep();
        assert!(!app.regen.wanted, "nor a third");
    }

    /// AFTER the rebuild finishes the cache is declared ready, and there are no new requests.
    #[test]
    fn warmup_settles_when_the_background_rebuild_lands() {
        let mut app = app_needing_brep();
        app.ensure_brep();
        assert!(app.regen.wanted, "the preparation was requested");
        // as the scheduler does it: start the task and wait for its result
        app.regen.wanted = false;
        app.spawn_regen();
        let busy = app.regen.busy.take().expect("the rebuild task was created");
        match busy.rx.recv().expect("the thread reported back") {
            JobResult::Regenerated { stamp, project, shapes, built, errors, cancelled } => app.finish_regen_checked(stamp, *project, shapes, built, errors, cancelled),
            _ => panic!("a rebuild result was expected"),
        }
        assert!(app.live.ready, "the live B-rep is built and the cache is ready: {} bodies without a shape", app.project.timeline.iter().filter_map(|n| n.kind.body()).filter(|b| !app.live.shapes.contains_key(b)).count());
        app.ensure_brep();
        assert!(!app.regen.wanted, "a ready cache asks for nothing more");
    }
}

#[cfg(test)]
mod sketch_plane_pick_frames_tests {
    //! Reported behaviour: an assembly was created with a part in it, "create a sketch" was pressed, and the
    //! rebuild window flickered back and forth leaving nothing possible to do. What runs here is A REAL frame
    //! loop: the plane pick calls `refresh_edges` every frame while the scheduler starts the deferred rebuild and
    //! takes its result. How many rebuilds start over 12 frames is counted: the right answer is ONE.
    use super::{App, BgKind, Busy, JobResult, Picking, Sel};

    /// One frame of the scheduler: if a rebuild was requested and the queue is free, start it and wait.
    /// This is what `tick_async` does in a live window, only without egui.
    fn pump_frame(app: &mut App, regens: &mut usize) {
        app.refresh_edges(); // in a live window this call happens EVERY frame
        if app.regen.wanted && app.regen.busy.is_none() {
            app.regen.wanted = false;
            app.spawn_regen();
            *regens += 1;
        }
        if let Some(Busy { rx, kind: BgKind::Regen, .. }) = app.regen.busy.take() {
            match rx.recv().expect("the rebuild thread reported back") {
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
        app.sel = Sel::Sketch(si);
        app.feat.op = 0;
        app.start_feat_cmd(1);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
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
        app.mode_3d = true;
        app.picking = Picking::SketchPlane(None); // "create a sketch" was pressed

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
}
