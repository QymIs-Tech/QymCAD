//! A RANDOM SESSION: checking what nobody thought of.
//!
//! Every other test checks scenarios that were DELIBERATELY invented — and catches exactly what was
//! suspected. Defects were reported three times in places nobody was looking (a seventh copy of the
//! reset, the circular pattern, recursion in Ctrl+C). That is not chance but a property of the
//! method: a guess does not cover what was never guessed at.
//!
//! So the actions here are chosen PSEUDORANDOMLY, and after each one the invariants are checked —
//! invariants stated independently of any scenario. The generator is deterministic: a seed that
//! failed reproduces exactly, and the defect gets fixed rather than "caught sometimes".
#[cfg(test)]
mod random_session {
    use super::super::{App, Sel};

    /// A deterministic generator (xorshift): one seed, one and the same session.
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn pick(&mut self, n: usize) -> usize {
            (self.next() % n as u64) as usize
        }
        fn f(&mut self, lo: f64, hi: f64) -> f64 {
            lo + (self.next() % 1000) as f64 / 1000.0 * (hi - lo)
        }
    }

    /// INVARIANTS independent of any scenario: what must hold ALWAYS.
    fn invariants(app: &App, step: usize, what: &str) {
        assert!(
            app.project.contours.len() == app.project.contours.ids().len(),
            "step {step} ({what}): the contour list and its Ids have drifted apart"
        );
        for (b, fs) in app.project.regen_faces.iter() {
            let mut ids: Vec<u32> = fs.iter().map(|f| f.id).collect();
            let n = ids.len();
            ids.sort_unstable();
            ids.dedup();
            assert_eq!(ids.len(), n, "step {step} ({what}): body {b} — the faces share names ({n} faces, {} names)", ids.len());
        }
        assert!(
            app.project.timeline.iter().all(|n| n.id != 0),
            "step {step} ({what}): the timeline holds a node without an Id"
        );
        // A LEFTOVER MODE: after cancelling everything, NOT ONE mode may stay active.
        // This is the very class that was reported three times, and never by an invented scenario.
        if what == "cancel everything" {
            let tail = app.tools.armed.draw_kind() != 0
                || app.tools.armed.modify() != 0
                || app.tools.armed.dim_kind() != 0
                || app.tools.armed.measuring()
                || !app.tools.measure.pts.is_empty()
                || app.tools.corner.at.is_some()
                || app.tools.corner.only.is_some()
                || app.tools.armed.pat_op() != 0
                || app.tools.armed.move_op() != 0
                || app.tools.pending_import.draw_pts.is_some()
                || app.tools.pending_import.curves.is_some()
                || app.tools.armed.commanding();
            assert!(!tail, "step {step}: a mode is still active after cancelling everything");
        }
        assert!(!crate::gui::doc_changed_outside_edit(&app.disk.edits, &app.project), "step {step} ({what}): the document was changed outside an operation");
    }

    #[test]
    fn a_random_session_keeps_the_document_consistent() {
        for seed in 1..=40u64 {
            let mut r = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
            let mut app = App::default();
            for step in 0..20 {
                let what: &str = match r.pick(11) {
                    0 => {
                        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
                        let (x, y) = (r.f(-20.0, 20.0), r.f(-20.0, 20.0));
                        app.project.add_rect_entity(si, x, y, x + r.f(5.0, 30.0), y + r.f(5.0, 30.0), qymcad_core::feature::Purpose::Real);
                        app.project.regen_sketch(si);
                        app.finish_sketch_edit();
                        app.chosen.sel = Sel::Sketch(si);
                        "rectangle sketch"
                    }
                    1 => {
                        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
                        app.project.add_circle_entity(si, r.f(-10.0, 10.0), r.f(-10.0, 10.0), r.f(2.0, 8.0), qymcad_core::feature::Purpose::Real);
                        app.project.regen_sketch(si);
                        app.finish_sketch_edit();
                        app.chosen.sel = Sel::Sketch(si);
                        "circle sketch"
                    }
                    2 => {
                        app.start_feat_cmd(1);
                        if let Some(p) = app.tools.cmd.params.iter_mut().find(|p| p.key == "height") {
                            p.val = r.f(2.0, 20.0);
                            p.txt = format!("{}", p.val);
                        }
                        app.apply_feat_cmd();
                        "extrude"
                    }
                    3 => {
                        app.cancel_all_tools();
                        "cancel everything"
                    }
                    4 => {
                        app.set_sk_tool((r.pick(4) + 1) as u8);
                        "sketch tool"
                    }
                    5 => {
                        qymcad_ui_state::set_dim_tool(&mut qymcad_ui_state::tools_of!(app), &mut app.viewing.mode_3d, &app.project, app.chosen.sel, app.sketch_ses, &mut app.status, (r.pick(3) + 1) as u8);
                        "dimension tool"
                    }
                    6 => {
                        // measuring is a tool too: switch it on and click a point
                        app.tools.armed = qymcad_ui_state::Armed::Measure;
                        app.tools.measure.pts.push(qymcad_core::geom::Point2::new(r.f(-10.0, 10.0), r.f(-10.0, 10.0)));
                        "measure"
                    }
                    7 => {
                        // an unfinished corner fillet popup
                        app.tools.corner.at = Some((r.pick(4), 0, false));
                        app.tools.corner.only = Some(std::collections::HashSet::new());
                        "corner popup"
                    }
                    8 => {
                        // A MODIFIER on the last body: a pattern (which also checks instance names)
                        if let Some(b) = app.project.timeline.iter().rev().find_map(|n| n.kind.body()) {
                            let arr = app.project.add_linear_array(b, r.f(20.0, 60.0), 0.0, 0.0, (r.pick(3) + 2) as u32);
                            let _ = app.project.finish_base_body(arr, 1);
                            qymcad_ui_state::mark_dirty_for_rebuild(&mut app.rebuild_ctx());
                        }
                        "pattern"
                    }
                    9 => {
                        // SAVE AND OPEN: the document must survive a round trip through a file
                        let path = std::env::temp_dir().join(format!("qym_fuzz_{seed}.qcad")).to_string_lossy().to_string();
                        crate::gui::io_jobs::spawn_save(&mut app.disk.io, &mut app.live, &mut app.project, &mut app.regen, &mut app.status, path.clone(), false);
                        app.wait_bg();
                        if let Ok(proj) = qymcad_io::load_project(&path) {
                            let before = app.project.bodies.len();
                            let mut back = App::default();
                            back.finish_project_load(path, proj, Vec::new());
                            crate::gui::io_jobs::ensure_brep(&mut back.rebuild_ctx());
                            assert_eq!(back.project.bodies.len(), before, "step {step}: the round trip through a file lost bodies");
                        }
                        "save+open"
                    }
                    _ => {
                        app.undo();
                        "undo"
                    }
                };
                qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
                invariants(&app, step, what);
            }
        }
    }
}
