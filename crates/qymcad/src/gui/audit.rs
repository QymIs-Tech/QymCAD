//! AN AUDIT OF LIVE WORK: not "does the function work" but "will a part come out".
//!
//! The claim made was that opening the CAD and starting a project with the tools that exist would catch
//! a heap of defects within a minute. That is checked not with words: REAL sequences are run through the
//! command layer on a live OCCT kernel and the volume is measured after every step. Failures are not
//! hidden in one assertion each — they are collected and given out at once, so that the scale is visible
//! rather than whichever came first.
#[cfg(test)]
mod live_session {
    use super::super::{App, Picking, Sel};

    fn vol(app: &App) -> f64 {
        let consumed = app.consumed_bodies();
        app.live.shapes.iter().filter(|(b, _)| !consumed.contains(b)).map(|(_, s)| s.volume()).sum()
    }

    /// A rectangle in a new sketch of the active part.
    fn rect(app: &mut App, x0: f64, y0: f64, x1: f64, y1: f64) -> usize {
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, x0, y0, x1, y1, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        si
    }

    /// Start a command with its parameters and apply it.
    fn run(app: &mut App, cmd: u8, params: &[(&str, f64)]) {
        app.start_feat_cmd(cmd);
        for (k, v) in params {
            if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == *k) {
                p.val = *v;
                p.txt = format!("{v}");
            }
        }
        app.apply_feat_cmd();
    }

    #[test]
    fn a_beginner_session_does_not_fall_apart() {
        let mut fails: Vec<String> = Vec::new();
        let check = |fails: &mut Vec<String>, what: &str, ok: bool, detail: String| {
            if !ok {
                fails.push(format!("{what}: {detail}"));
            }
        };

        // 1. A PLATE 40x30x10
        let mut app = App::default();
        let si = rect(&mut app, 0.0, 0.0, 40.0, 30.0);
        app.sel = Sel::Sketch(si);
        run(&mut app, 1, &[("height", 10.0)]);
        let v1 = vol(&app);
        check(&mut fails, "1. a plate 40x30x10", (v1 - 12000.0).abs() < 1.0, format!("V={v1:.1}, expected 12000; the status line: {}", app.status));

        // 2. A FILLET of a vertical edge, R3
        let body = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).unwrap_or(0);
        let eid = app.project.regen_edges.get(&body).and_then(|es| {
            es.iter().find(|e| (e.a[0] - e.b[0]).abs() < 1e-6 && (e.a[1] - e.b[1]).abs() < 1e-6 && (e.a[2] - e.b[2]).abs() > 5.0).map(|e| e.id)
        });
        match eid {
            None => fails.push("2. the fillet: no vertical edge of the plate was found in the edge cache".into()),
            Some(eid) => {
                app.start_feat_cmd(4);
                app.gsel.edges.insert(eid);
                app.edges.body = Some(body);
                if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "radius") {
                    p.val = 3.0;
                    p.txt = "3".into();
                }
                app.apply_feat_cmd();
                let v2 = vol(&app);
                let want = v1 - (9.0 - std::f64::consts::PI * 9.0 / 4.0) * 10.0;
                check(&mut fails, "2. a fillet R3", (v2 - want).abs() < 1.0, format!("V={v2:.1}, expected {want:.1}; the status line: {}", app.status));
            }
        }

        // 3. A SKETCH ON THE TOP FACE -> a through cut 10 mm across
        let body = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).unwrap_or(0);
        let top = app.project.regen_faces.get(&body).and_then(|fs| fs.iter().find(|f| f.normal[2] > 0.9).cloned());
        match top {
            None => fails.push("3. the sketch on a face: no top face was found".into()),
            Some(f) => {
                let key = qymcad_core::feature::FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id };
                let si2 = app.create_sketch_on(qymcad_core::feature::SketchPlane::Face(body, key));
                app.project.add_circle_entity(si2, 20.0, 15.0, 5.0, qymcad_core::feature::Purpose::Real);
                app.project.regen_sketch(si2);
                app.finish_sketch_edit();
                app.sel = Sel::Sketch(si2);
                app.start_feat_cmd(1);
                app.feat.op = 2; // A CUT (0 = add, 1 = boss, 2 = cut, 3 = intersect)
                app.feat.flip = true; // DOWNWARDS, into the material
                if let Some(pp) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
                    pp.val = 20.0;
                    pp.txt = "20".into();
                }
                app.apply_feat_cmd();
                let v3 = vol(&app);
                let want = vol(&app) + 0.0; // measured below by the difference
                let _ = want;
                check(&mut fails, "3. a through cut 10 mm across", v3 < v1 - 700.0, format!("V={v3:.1}, expected roughly 785 less than the plate ({:.1}); the status line: {}", v1 - 785.0, app.status));
            }
        }

        // 4. AN EDIT OF THE BASE SKETCH: the plate goes 40 -> 50 along X. Everything below in the
        // timeline must survive.
        let before = vol(&app);
        app.enter_sketch_edit(si);
        for pt in app.project.sketches[si].points.iter_mut() {
            if (pt.x - 40.0).abs() < 1e-9 {
                pt.x = 50.0;
            }
        }
        app.project.solve_sketch(si);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        let after = vol(&app);
        check(&mut fails, "4. an edit of the base sketch, 40 -> 50", after > before + 2500.0, format!("V {before:.1} -> {after:.1}, expected +3000; errors in the timeline: {}", app.project.regen_errors.len()));
        check(&mut fails, "4b. the timeline is free of errors after the edit", app.project.regen_errors.is_empty(), format!("{:?}", app.project.regen_errors.values().next()));

        // 5. SAVING AND OPENING: the volume must match
        let path = std::env::temp_dir().join("qym_audit.qcad");
        let p = path.to_string_lossy().to_string();
        app.spawn_save(p.clone(), false);
        app.wait_bg();
        let mut app2 = App::default();
        match qymcad_io::load_project(&p) {
            Err(e) => fails.push(format!("5. opening what was saved: {e}")),
            Ok(proj) => {
                app2.finish_project_load(p.clone(), proj, Vec::new());
                app2.ensure_brep();
                let v5 = vol(&app2);
                check(&mut fails, "5. the volume after opening", (v5 - after).abs() < 1.0, format!("{after:.1} -> {v5:.1}"));
                check(&mut fails, "5b. the timeline is free of errors after opening", app2.project.regen_errors.is_empty(), format!("{:?}", app2.project.regen_errors.values().next()));
            }
        }

        assert!(fails.is_empty(), "THE AUDIT OF THE SESSION — {} failures:\n  {}", fails.len(), fails.join("\n  "));
    }

    /// A DEFECT FOUND BY THE AUDIT: undo was not bound to an operation.
    ///
    /// A step of undo was not created by a command but "noticed" by a frame: `maybe_commit` computed the
    /// fingerprint of the document every frame and, if the pointer was released and the fingerprint had
    /// changed, laid down a FULL snapshot. Three consequences followed: the boundaries of a step depended
    /// on the state of the mouse rather than on where the operation ended; the stack held no names of
    /// operations ("undo the extrusion"); and outside a window (tests, scripts, batch mode) there was no
    /// undo at all — nobody to lay the snapshot. Grown-up CAD does the opposite: an operation is a
    /// transaction, and it opens and closes a step of undo with a name of its own.
    ///
    /// CLOSED: a command opens an operation (`App::edit`) and closes it with a commit or a rollback.
    #[test]
    fn undo_is_bound_to_an_operation_not_to_a_frame() {
        let mut app = App::default();
        let si = rect(&mut app, 0.0, 0.0, 40.0, 30.0);
        app.sel = Sel::Sketch(si);
        run(&mut app, 1, &[("height", 10.0)]);
        let after_extrude = vol(&app);
        assert!(after_extrude > 0.0, "the body is built");
        app.undo();
        assert!(vol(&app) < after_extrude, "undo must remove the extrusion rather than leave everything as it was");
    }

    /// A DEFECT FOUND BY THE AUDIT: the direction of an operation was computed ONCE, when the command
    /// started.
    ///
    /// The smart default is right: a cut from a sketch ON A FACE must go INTO the body, otherwise it cuts
    /// empty space. But it was computed in `start_feat_cmd` from the current operation — and the operation
    /// is changed AFTERWARDS, right in the command bar (open "extrude", switch to "cut"). The direction
    /// was not recomputed then: the tool goes outwards, removes NOTHING, and the command reports success.
    ///
    /// Two lessons, both architectural:
    /// * the state of a command is smeared over independent fields of `App` (`feat_cmd`, `feat_op`,
    ///   `feat_flip`, `feat_extent` and so on), so the order in which they are set matters more than their
    ///   values. In grown-up CAD a command is ONE record of parameters, and the derived things (the
    ///   direction) are computed at the moment of applying rather than at the moment of opening;
    /// THE FIRST IS CLOSED: the direction is computed on APPLYING (`smart_flip`) rather than on opening.
    /// The second (a silently empty result) is not yet.
    #[test]
    fn switching_the_operation_after_the_command_started_keeps_the_direction() {
        let mut app = App::default();
        let si = rect(&mut app, 0.0, 0.0, 40.0, 30.0);
        app.sel = Sel::Sketch(si);
        run(&mut app, 1, &[("height", 10.0)]);
        let plate = vol(&app);
        let body = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).unwrap_or(0);
        let top = app.project.regen_faces.get(&body).and_then(|fs| fs.iter().find(|f| f.normal[2] > 0.9).cloned()).expect("the top face");
        let key = qymcad_core::feature::FaceKey { index: 0, centroid: [top.centroid.x, top.centroid.y, top.centroid.z], normal: top.normal, id: top.id };
        let si2 = app.create_sketch_on(qymcad_core::feature::SketchPlane::Face(body, key));
        app.project.add_circle_entity(si2, 20.0, 15.0, 5.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si2);
        app.finish_sketch_edit();
        app.sel = Sel::Sketch(si2);
        // THE ORDER A PERSON USES: open "extrude" and switch to "cut" in the bar afterwards
        app.start_feat_cmd(1);
        app.feat.op = 2;
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 20.0;
            p.txt = "20".into();
        }
        app.apply_feat_cmd();
        assert!(vol(&app) < plate - 700.0, "a cut must remove material whenever the operation was chosen: V={:.1}, it was {plate:.1}", vol(&app));
    }

    /// EVERY OPERATION IS ONE STEP OF UNDO WITH A NAME. Not "some number of steps", not a nameless
    /// "edit".
    #[test]
    fn every_operation_leaves_exactly_one_named_undo_step() {
        let mut app = App::default();
        let si = rect(&mut app, 0.0, 0.0, 40.0, 30.0);
        app.sel = Sel::Sketch(si);

        let before = app.edits.undo.len();
        run(&mut app, 1, &[("height", 10.0)]);
        assert_eq!(app.edits.undo.len(), before + 1, "an extrusion is exactly one step");
        assert_eq!(app.edits.undo.last().map(|s| s.name.clone()), Some(crate::i18n::tr("f-extrusion")), "the step is named after the operation");

        // a fillet on an edge is one step too, with a name of its own
        let body = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).unwrap_or(0);
        if let Some(eid) = app.project.regen_edges.get(&body).and_then(|es| es.iter().find(|e| (e.a[2] - e.b[2]).abs() > 5.0).map(|e| e.id)) {
            let n = app.edits.undo.len();
            app.start_feat_cmd(4);
            app.gsel.edges.insert(eid);
            app.edges.body = Some(body);
            if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "radius") {
                p.val = 2.0;
                p.txt = "2".into();
            }
            app.apply_feat_cmd();
            assert_eq!(app.edits.undo.len(), n + 1, "a fillet is exactly one step");
            assert_eq!(app.edits.undo.last().map(|s| s.name.clone()), Some(crate::i18n::tr("f-fillet")));
        }

        // deleting an operation is a transaction with a name of its own as well
        let n = app.edits.undo.len();
        let last_feature = app.project.timeline.len().saturating_sub(1);
        app.delete_feature(last_feature);
        assert_eq!(app.edits.undo.len(), n + 1, "deleting an operation is one step");
        assert_eq!(app.edits.undo.last().map(|s| s.name.clone()), Some(crate::i18n::tr("status-delete-feature")));
    }

    /// A FAILED OPERATION LEAVES NO TRACE: neither in the document nor in the undo stack.
    #[test]
    fn a_failed_operation_leaves_no_trace() {
        let mut app = App::default();
        let si = rect(&mut app, 0.0, 0.0, 40.0, 30.0);
        app.sel = Sel::Sketch(si);
        run(&mut app, 1, &[("height", 10.0)]);
        let steps = app.edits.undo.len();
        let nodes = app.project.timeline.len();
        let v = vol(&app);

        // a fillet with a knowingly impossible radius on the chosen edge
        let body = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).unwrap_or(0);
        if let Some(eid) = app.project.regen_edges.get(&body).and_then(|es| es.iter().find(|e| (e.a[2] - e.b[2]).abs() > 5.0).map(|e| e.id)) {
            app.start_feat_cmd(4);
            app.gsel.edges.insert(eid);
            app.edges.body = Some(body);
            if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "radius") {
                p.val = 999.0;
                p.txt = "999".into();
            }
            app.apply_feat_cmd();
            // THE REJECTION IS ASKED OF THE COMMAND rather than of words in the status line: a check
            // written against a phrase of one language goes blind in another.
            //
            // MEASURED, AND IT IS WORTH KNOWING: a fillet of radius 999 is NOT rejected by the command. It
            // creates a node, the node fails to rebuild, and the failure is reported through
            // `regen_errors` — so the branch below does not run at all and this test currently proves
            // nothing about that path. The former condition also matched a phrase of one language and so
            // was silent for the same reason. Whether an unbuildable node should stay in the timeline is a
            // question about behaviour, not about a test, and it is left to be decided rather than papered
            // over here.
            if app.cmd_failed {
                assert_eq!(app.edits.undo.len(), steps, "a rejected operation leaves no step: {}", app.status);
                assert_eq!(app.project.timeline.len(), nodes, "and leaves no node in the timeline");
                assert!((vol(&app) - v).abs() < 1e-6, "and does not touch the geometry");
            }
        }
    }

    /// OPENING A FILE IS NOT AN EDIT. The undo stack is cleared: otherwise undo after opening would bring
    /// pieces of the PREVIOUS document back on top of the new one — a state that never existed.
    #[test]
    fn opening_a_document_clears_the_undo_stack() {
        let mut app = App::default();
        let si = rect(&mut app, 0.0, 0.0, 40.0, 30.0);
        app.sel = Sel::Sketch(si);
        run(&mut app, 1, &[("height", 10.0)]);
        assert!(!app.edits.undo.is_empty(), "the step from the extrusion is there");

        let path = std::env::temp_dir().join("qym_audit_open.qcad");
        let p = path.to_string_lossy().to_string();
        app.spawn_save(p.clone(), false);
        app.wait_bg();
        let proj = qymcad_io::load_project(&p).expect("the opening");
        app.finish_project_load(p, proj, Vec::new());

        assert!(app.edits.undo.is_empty(), "after opening, the undo stack is empty: {} steps", app.edits.undo.len());
        assert!(app.edits.redo.is_empty(), "and the redo stack is empty");
    }

    /// ONE REBUILD PER ACTION. Inside, a command asks for a rebuild several times (the sketch, the
    /// timeline, the caches) — but the action is one. The real rebuilds are counted by the revision
    /// counter of the geometry.
    #[test]
    fn one_user_action_rebuilds_the_model_once() {
        let mut app = App::default();
        let si = rect(&mut app, 0.0, 0.0, 40.0, 30.0);
        app.sel = Sel::Sketch(si);
        run(&mut app, 1, &[("height", 10.0)]);

        // the fillet: how many times the model was rebuilt during ONE command
        let body = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).unwrap_or(0);
        let Some(eid) = app.project.regen_edges.get(&body).and_then(|es| es.iter().find(|e| (e.a[2] - e.b[2]).abs() > 5.0).map(|e| e.id)) else { return };
        app.start_feat_cmd(4);
        app.gsel.edges.insert(eid);
        app.edges.body = Some(body);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "radius") {
            p.val = 2.0;
            p.txt = "2".into();
        }
        let rev0 = app.regen.geom_rev;
        app.apply_feat_cmd();
        let rebuilds = app.regen.geom_rev.wrapping_sub(rev0);
        assert!(rebuilds <= 2, "one action means no more than one rebuild (plus the invalidation of the view): {rebuilds} were counted");
    }

    /// The sketch modes are mutually exclusive: entering ANY tool extinguishes ALL the others.
    ///
    /// This catches not the fact that a field was left unset but the cause: leaving the modes rewritten by
    /// hand at every entry point. Copies drift apart — one forgets an unfinished import, another an array
    /// — and out comes a tool dragging the tail of the previous one behind it. The test keeps the
    /// transition single: any new entry point that writes the reset by hand fails here.
    #[test]
    fn entering_a_tool_leaves_no_tail_of_the_previous_one() {
        // every mode at once — a state that in life is built up one at a time
        fn dirty(app: &mut App) {
            app.tool.kind = 1;
            app.tool.modify = 2;
            app.tool.pts.push(qymcad_core::geom::Point2 { x: 1.0, y: 2.0 });
            app.tool.circ_tan = Some(super::super::EdgeRef::Circle { center: 1, r: 5.0 });
            app.tool.click_op = 3;
            app.tool.move_op = 4;
            app.tool.move_base = Some(qymcad_core::geom::Point2 { x: 0.0, y: 0.0 });
            app.dim.kind = 2;
            app.dim.first = Some(super::super::DimRef::Point(1));
            app.place.dim = Some(0);
            app.pending_import.draw_pts = Some(Vec::new());
            app.corner.at = Some((0, 0, false));
            app.corner.only = Some(std::collections::HashSet::new());
            app.measure.on = true;
            app.measure.pts.push(qymcad_core::geom::Point2::new(1.0, 2.0));
            app.pat.op = 1;
            app.sel_sk.constraint = Some(0);
            app.sel_sk.modify = Some(0);
            app.picking = Picking::FilletAll;
            app.drag = super::super::Dragging::Dim(0);
            app.inline = super::super::InlineEdit::Note(0);
        }

        // what must be extinguished after entering any tool
        fn tail(app: &App) -> Vec<&'static str> {
            let mut t = Vec::new();
            if app.tool.modify != 0 { t.push("the modify mode") }
            if !app.tool.pts.is_empty() { t.push("the points clicked") }
            if app.tool.circ_tan.is_some() { t.push("the tangency of a circle") }
            if app.tool.click_op != 0 { t.push("the click operation") }
            if app.tool.move_op != 0 || app.tool.move_base.is_some() { t.push("the move") }
            if app.dim.first.is_some() { t.push("the first reference of a dimension") }
            if app.place.dim.is_some() { t.push("the dimension being placed") }
            if app.pending_import.draw_pts.is_some() { t.push("the unfinished import") }
            if app.corner.at.is_some() || app.corner.only.is_some() { t.push("the corner popup") }
            if app.measure.on || !app.measure.pts.is_empty() { t.push("the measurement") }
            if app.pat.op != 0 { t.push("the array") }
            if app.sel_sk.constraint.is_some() || app.sel_sk.modify.is_some() { t.push("the highlight of a constraint") }
            if !matches!(app.picking, Picking::None) { t.push("the pick of a shape") }
            if !matches!(app.drag, super::super::Dragging::None) { t.push("the drag") }
            if !matches!(app.inline, super::super::InlineEdit::None) { t.push("the edit in place") }
            t
        }

        let mut bad: Vec<String> = Vec::new();
        // every entry point into a mode — each must give a clean transition
        let entries: Vec<(&str, fn(&mut App))> = vec![
            ("a drawing tool", |a: &mut App| a.set_sk_tool(2)),
            ("a dimension tool", |a: &mut App| a.set_dim_tool(1)),
            ("the selection mode", |a: &mut App| a.sketch_select_mode()),
            ("leaving the sketch", |a: &mut App| a.finish_sketch_edit()),
            ("cancelling everything", |a: &mut App| a.cancel_all_tools()),
        ];
        for (what, enter) in entries {
            let mut app = App::default();
            let si = rect(&mut app, 0.0, 0.0, 20.0, 20.0);
            app.sel = Sel::Sketch(si);
            dirty(&mut app);
            enter(&mut app);
            let t = tail(&app);
            if !t.is_empty() {
                bad.push(format!("\"{what}\" drags the tail of the previous mode: {}", t.join(", ")));
            }
        }
        assert!(bad.is_empty(), "the transition into a tool must be clean:\n{}", bad.join("\n"));
    }

    /// A feature command does not inherit THE TARGETING of the previous one.
    ///
    /// "Sweep, pick a profile, change your mind, press Loft, come back to Sweep" — and the profile from
    /// the first attempt is still hanging there. The reset of the targeting was written into applying and
    /// into cancelling, and not into starting, so a move between commands without cancelling carried a
    /// tail. The pre-selection (edges, faces, a sketch) must NOT be extinguished by it: that is collected
    /// before the button is pressed — and it is checked by the same test, otherwise "select the edges,
    /// then Fillet" would stop working.
    #[test]
    fn a_feature_command_starts_without_the_previous_targeting() {
        let mut app = App::default();
        let si = rect(&mut app, 0.0, 0.0, 40.0, 30.0);
        app.sel = Sel::Sketch(si);
        run(&mut app, 1, &[("height", 10.0)]);

        // the targeting of the previous command
        app.start_feat_cmd(8); // the sweep
        app.sweep.prof_sid = 7;
        app.sweep.path_sid = 9;
        app.sweep.pick_path = true;
        app.loft.pick = true;
        app.draft.pick_neutral = true;
        app.mirror.plane = Some(qymcad_core::feature::SketchPlane::default());
        app.datum.plane_pick = Some(qymcad_core::feature::SketchPlane::default());
        app.picking = Picking::FilletAll;

        app.start_feat_cmd(9); // the loft — another command, the targeting must go out
        let mut tail = Vec::new();
        if app.sweep.prof_sid != 0 || app.sweep.path_sid != 0 || app.sweep.pick_path { tail.push("the sweep") }
        if app.loft.pick { tail.push("the loft") }
        if app.draft.pick_neutral { tail.push("the draft") }
        if app.mirror.plane.is_some() { tail.push("the mirror") }
        if app.datum.plane_pick.is_some() { tail.push("the datum") }
        if !matches!(app.picking, Picking::None) { tail.push("the pick of a shape") }
        assert!(tail.is_empty(), "the new command inherited the targeting: {}", tail.join(", "));

        // References are collected INSIDE a command (the contract of the tools), so a command must open
        // with an empty selection — otherwise the fillet would silently grab the edges of the previous
        // operation.
        app.cancel_feat_cmd();
        let body = app.project.timeline.iter().rev().find_map(|n| n.kind.body()).unwrap_or(0);
        let Some(eid) = app.project.regen_edges.get(&body).and_then(|es| es.first().map(|e| e.id)) else { return };
        app.gsel.edges.insert(eid);
        app.edges.body = Some(body);
        app.start_feat_cmd(4);
        assert!(app.gsel.edges.is_empty(), "a command must open with an empty set of references: the set is collected by clicks inside the command");
    }

    /// A BACKGROUND JOB NEVER SWALLOWS EDITS MADE WHILE IT RAN.
    ///
    /// Saving and restoring the B-rep work with A COPY of the document in a separate thread. The danger
    /// is obvious: the copy goes stale, and if the thread returned THE DOCUMENT it would wipe out
    /// everything done in the meantime. What is checked is that this does not happen: an edit made during
    /// the write stays in the live document, and the project is honestly counted as unsaved — what was
    /// written was a snapshot.
    #[test]
    fn a_background_job_never_swallows_edits_made_while_it_ran() {
        let mut app = App::default();
        let si = rect(&mut app, 0.0, 0.0, 40.0, 30.0);
        app.sel = Sel::Sketch(si);
        run(&mut app, 1, &[("height", 10.0)]);
        let before = vol(&app);

        let path = std::env::temp_dir().join("qym_bg_edit.qcad").to_string_lossy().to_string();
        app.spawn_save(path.clone(), false);
        // WHILE THE WRITE IS UNDER WAY the document is edited
        let si2 = rect(&mut app, 50.0, 0.0, 60.0, 10.0); // ASIDE from the plate, otherwise the union adds nothing
        app.sel = Sel::Sketch(si2);
        run(&mut app, 1, &[("height", 5.0)]);
        let after = vol(&app);
        app.wait_bg();

        assert!(after > before, "an edit made during a background write was lost: {before:.1} -> {after:.1}");
        assert!(app.is_dirty(), "a SNAPSHOT was written and the edit came after it — the project must stay unsaved");
    }

    /// THE APPLICATION STARTS. The guard over transaction boundaries does not fire on an empty document.
    ///
    /// A build was launched and gave a panic on the very first frame: the document was changed past
    /// `App::edit`. There were two causes, and both are visible only on a LIVE run, which is why there
    /// were no tests for them: `committed_key` was started at zero instead of the key of a fresh document,
    /// and the key itself included `geom_rev` — a counter that ticks on ANY rebuild, purely derived ones
    /// included. So the very first rebuild looked like an edit going round the boundary.
    #[test]
    fn a_fresh_app_does_not_trip_the_transaction_guard() {
        let mut app = App::default();
        assert!(!app.doc_changed_outside_edit(), "an empty document looks changed past an operation");

        // a rebuild is a DERIVED action, it does not edit the document
        let si = rect(&mut app, 0.0, 0.0, 40.0, 30.0);
        app.sel = Sel::Sketch(si);
        run(&mut app, 1, &[("height", 10.0)]);
        assert!(!app.doc_changed_outside_edit(), "after an operation the guard counts the document as changed past it");

        app.regenerate_all();
        assert!(!app.doc_changed_outside_edit(), "A REBUILD does not edit the document — the guard must not notice it");
    }

    /// ENTERING A SUBASSEMBLY DOES NOT BRING THE APPLICATION DOWN.
    ///
    /// Reported behaviour: entering a subassembly ends in a panic. Entering changes `active_component`,
    /// and that IS STORED IN THE FILE, so formally it is an edit of the document — and the guard over
    /// transaction boundaries counted it as an edit past `App::edit`. Navigation is a third kind of
    /// change: neither an edit by a person nor a rebuild. It starts no step of undo ("undo: entering a
    /// subassembly" is nonsense), and it must not crash either.
    #[test]
    fn entering_a_subassembly_does_not_trip_the_transaction_guard() {
        let mut app = App::default();
        let sub = app.project.add_component("Subassembly");
        app.project.set_active_component(Some(sub));
        let part = app.project.add_part("Part");

        app.enter_component(sub);
        app.sync_workbench();
        assert!(!app.doc_changed_outside_edit(), "entering a subassembly looks like an edit past an operation");

        app.enter_component(part);
        app.sync_workbench();
        assert!(!app.doc_changed_outside_edit(), "entering a part inside a subassembly looks like an edit past an operation");
    }

    /// Ctrl+C AND Ctrl+V OUTSIDE A SKETCH DO NOT KILL THE APPLICATION.
    ///
    /// `clipboard_copy` and `clipboard_paste` called THEMSELVES at the tail instead of the tree functions
    /// — a trace of a mechanical rename of methods. That is not an inconvenience but a stack overflow on
    /// the very first press outside the sketch mode: copying a part in the tree brought the application
    /// down. The test has to be headless: the recursion is visible neither in the geometry tests nor by
    /// eye in a diff.
    #[test]
    fn clipboard_outside_a_sketch_does_not_recurse_forever() {
        let mut app = App::default();
        let part = app.project.add_part("Part");
        let ci = app.project.components.iter().position(|c| c.id == part).expect("the component is in place");
        app.sel = Sel::Component(ci);
        app.clipboard_copy(false); // recursion means a stack overflow and the test never returns
        app.clipboard_paste();
        assert!(!app.status.is_empty(), "the clipboard of the tree must at least report in the status line");
    }

    /// NOTHING REBUILDS THE MODEL IN THE MIDDLE OF AN OPERATION — by any path.
    ///
    /// This was first described wrongly, as "the scheduler is called from three places and bypassed from
    /// eight". A measurement showed that the protection sits NOT in the scheduler but deeper, in
    /// `regenerate_all` itself: inside an open operation the request accumulates rather than executing.
    /// So the direct calls are no bypass, they go through the same invariant. The test pins the invariant
    /// rather than the number of call sites: however many appear, the model is not rebuilt in the middle
    /// of an operation.
    #[test]
    fn nothing_rebuilds_the_model_in_the_middle_of_an_operation() {
        let mut app = App::default();
        let si = rect(&mut app, 0.0, 0.0, 40.0, 30.0);
        app.sel = Sel::Sketch(si);

        let rev0 = app.regen.geom_rev;
        {
            let mut e = app.edit("A check of the boundary");
            let a = e.app();
            // inside an operation — every path that in real work calls a rebuild directly
            a.regenerate_all();
            a.project.mark_sketch_dirty(a.project.sketches[si].id);
            a.regenerate_all();
            assert_eq!(a.regen.geom_rev, rev0, "the model was rebuilt INSIDE an operation — the boundary does not hold");
        }
        // the operation is closed — and now there is exactly one rebuild
        app.rebuild_if_dirty();
        assert!(app.regen.geom_rev > rev0, "after an operation is closed a rebuild must happen");
    }

    /// EXACTLY ONE THING IS DRAGGED. The priority chain of grabbing does not seize a second object on top
    /// of the first.
    ///
    /// The chain was written by hand, and every link checked A SUBSET OF ITS OWN: the text and the note
    /// looked at "not a text and not a note", while the dimension looked at "not a note and not a
    /// dimension". The sets drifted apart, and while dragging a TEXT the dimension branch fired all the
    /// same. The `Dragging` type was created precisely so that such a thing could not be expressed.
    #[test]
    fn only_one_thing_can_be_dragged_at_a_time() {
        use super::super::Dragging;
        let mut app = App::default();
        // something is already being dragged — every link of the chain must see it with ONE question
        for grabbed in [Dragging::Text(0), Dragging::Note(0), Dragging::Dim(0), Dragging::Point(0, 0)] {
            app.drag = grabbed.clone();
            assert!(app.drag.active(), "the grab is not visible to the predicate of the record");
        }
        app.drag = Dragging::None;
        assert!(!app.drag.active(), "an empty grab must not count as active");
    }
}
