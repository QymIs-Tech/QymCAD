//! THE SKETCH OF THE REPORT TAKES A LINE: the document of the report opened through File, the part and then its sketch
//! entered by a double click in the tree, the Line tool taken and two points clicked - each step in the time of a few
//! frames. The sketch: 973 lines and four patterns tied by 1 062 points on lines into one part, 26 335 loops.
//!
//! Reported behaviour: in that sketch every point of a line took some 20 s, and nothing could be drawn. Each change
//! counted the redundant constraints one by one over the whole part (9 s), and each frame with the sketch selected in
//! the 3D view looked for every loop along the list of the sketch's loops (4.3 s).
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use crate::gui::import_door::tests::running;
    use std::time::{Duration, Instant};

    /// The time of `work`.
    fn timed(work: impl FnOnce()) -> Duration {
        let started = Instant::now();
        work();
        started.elapsed()
    }

    /// A step of the hand and its time.
    struct Step {
        what: &'static str,
        took: Duration,
        kind: Kind,
    }

    /// What a step is: an action of the hand, several frames, or one frame of the window, or the way in and out of the
    /// sketch - timed and told, not held to a time: entering the part and leaving the sketch rebuild its bodies, 0.36 s
    /// and under 1 s alone, 7.3 s and 5.7 s beside a thousand other checks
    #[derive(Clone, Copy, PartialEq)]
    enum Kind {
        Action,
        Frame,
        Way,
    }

    /// The path of the sample of the report, where the tree has it.
    fn sample() -> Option<String> {
        let path = format!("{}/../../samples/pref_sketch.qcad", env!("CARGO_MANIFEST_DIR"));
        if std::path::Path::new(&path).exists() {
            Some(path)
        } else {
            eprintln!("PASSED OVER: the private sample pref_sketch.qcad is not in this tree - the check on it runs only where the samples are");
            None
        }
    }

    /// File, Open: the chooser answers with `path`, and the load runs in the background as in the window; the hand waits
    /// for it as a person waits for the spinner.
    fn opened(app: &mut crate::gui::App, path: &str) {
        let (tx, rx) = std::sync::mpsc::channel();
        app.arm_file_ask(rx, |app, p| crate::gui::io_jobs::spawn_project_load(&mut app.regen, p.to_string_lossy().into_owned()));
        tx.send(Some(std::path::PathBuf::from(path))).expect("the chooser's channel is open");
        let mut hand = Hand::new(app);
        let opening = Instant::now();
        hand.frame(Vec::new());
        while (hand.app.regen.busy.is_some() || hand.app.project.sketches.is_empty()) && opening.elapsed() < Duration::from_secs(300) {
            std::thread::sleep(Duration::from_millis(20));
            hand.frame(Vec::new());
        }
    }

    #[test]
    fn the_sketch_of_the_report_takes_a_line() {
        let Some(path) = sample() else { return };
        let (mut app, _ctx) = running();
        opened(&mut app, &path);
        let mut hand = Hand::new(&mut app);
        let name = hand.app.project.sketches.first().map(|s| s.name.clone()).expect("the document holds its sketch");
        let lines = hand.app.project.sketches[0].entities.len();
        let mut steps: Vec<Step> = Vec::new();
        let took = timed(|| assert!(hand.double_click_word("Part 1"), "no row of the part in the tree"));
        steps.push(Step { what: "the part entered", took, kind: Kind::Way });
        let took = timed(|| assert!(hand.double_click_word(&name), "no row {name:?} in the tree"));
        steps.push(Step { what: "the sketch entered", took, kind: Kind::Action });
        let took = timed(|| {
            hand.frame(Vec::new());
        });
        steps.push(Step { what: "a frame of the sketch", took, kind: Kind::Frame });
        let took = timed(|| {
            hand.sk_tool(1);
        });
        steps.push(Step { what: "the Line tool taken", took, kind: Kind::Action });
        let took = timed(|| {
            hand.click2d(-500.0, -500.0);
        });
        steps.push(Step { what: "the first point", took, kind: Kind::Action });
        let took = timed(|| {
            hand.click2d(-400.0, -450.0);
        });
        steps.push(Step { what: "the second point", took, kind: Kind::Action });
        let lines_drawn = hand.app.project.sketches[0].entities.len();
        // the tool put down with Esc and the sketch left with Ctrl+Enter: the view is back in 3D with the sketch selected
        let took = timed(|| {
            hand.key(egui::Key::Escape).key(egui::Key::Escape).ctrl(egui::Key::Enter);
        });
        steps.push(Step { what: "the sketch left", took, kind: Kind::Way });
        assert!(hand.app.sketch_ses.editing.is_none(), "Ctrl+Enter did not leave the sketch");
        assert!(hand.app.viewing.mode_3d && matches!(hand.app.chosen.sel, super::super::Sel::Sketch(0)), "the sketch left is not selected in the 3D view");
        let took = timed(|| {
            hand.frame(Vec::new());
        });
        steps.push(Step { what: "a frame of the 3D view", took, kind: Kind::Frame });
        let said: Vec<String> = steps.iter().map(|s| format!("{} {:?}", s.what, s.took)).collect();
        eprintln!("the sketch of the report: {}", said.join(", "));
        assert_eq!(lines_drawn, lines + 1, "the two clicks drew no line");
        // measured in a release build: an action 0.18 - 0.39 s, some 20 s a point before; a frame 30 ms, 4.3 s before in
        // the 3D view with the sketch selected. A test build is several times slower
        let budget = |k: Kind| match (k, cfg!(debug_assertions)) {
            (Kind::Way, _) => Duration::MAX,
            (Kind::Action, true) => Duration::from_secs(5),
            (Kind::Action, false) => Duration::from_secs(1),
            (Kind::Frame, true) => Duration::from_secs(1),
            (Kind::Frame, false) => Duration::from_millis(200),
        };
        assert!(steps.iter().all(|s| qymcad_core::time_budget::within(s.took, budget(s.kind))), "steps over their budget in the sketch of the report: {}", said.join(", "));
    }

    /// The loops of the first sketch as a sorted list of what each is, as the probes of the loops print them.
    fn loops(p: &qymcad_core::model::Project) -> Vec<String> {
        let mut out: Vec<String> = p.sketches[0]
            .contour_ids
            .iter()
            .filter_map(|cid| {
                let c = &p.contours[p.contour_index(*cid)?];
                let mut src = c.edge_src.clone();
                src.sort_unstable();
                src.dedup();
                let n = c.points.len().max(1) as f64;
                let (sx, sy) = c.points.iter().fold((0.0, 0.0), |(a, b), q| (a + q.x, b + q.y));
                Some(format!("{} {:?} ({:.6}, {:.6}) {:.6}", c.closed, src, sx / n, sy / n, c.signed_area().abs()))
            })
            .collect();
        out.sort();
        out
    }

    /// What the sketch holds, to hold a step to.
    #[derive(Clone, Copy, Debug)]
    struct Count {
        curves: usize,
        constraints: usize,
    }

    fn count(hand: &Hand) -> Count {
        let s = &hand.app.project.sketches[0];
        Count { curves: s.entities.len(), constraints: s.constraints.len() }
    }

    /// The new lines of the sketch since `had` curves, in their order.
    fn new_lines(hand: &Hand, had: usize) -> Vec<u64> {
        hand.app.project.sketches[0].entities[had..].iter().filter(|e| matches!(e.kind, qymcad_core::model::EntityKind::Line { .. })).map(|e| e.id).collect()
    }

    #[test]
    fn every_tool_of_the_window_works_on_the_sketch_of_the_report() {
        // EVERY TOOL AS A PERSON USES IT on the sketch of the report: each step through the window, timed, held to what it
        // leaves and to the loops of a rebuild from all the curves. What a tool works on is drawn by hand first, past the
        // right of the drawing, as nothing in it has a corner
        let Some(path) = sample() else { return };
        // a release build alone: in a test build the eleven steps on 26 335 loops take 8 minutes of a run of 3
        if cfg!(debug_assertions) {
            eprintln!("PASSED OVER in a test build: every tool of the window on the sketch of the report runs in a release build (cargo test --release)");
            return;
        }
        let (mut app, _ctx) = running();
        opened(&mut app, &path);
        let mut hand = Hand::new(&mut app);
        let name = hand.app.project.sketches[0].name.clone();
        assert!(hand.double_click_word("Part 1") && hand.double_click_word(&name), "no way into the sketch through the tree");
        // a rebuild under way refuses input: the hand waits for it to end, as a person waits for the spinner to go
        let waiting = Instant::now();
        hand.frame(Vec::new());
        while hand.app.regen.busy.is_some() && waiting.elapsed() < Duration::from_secs(120) {
            std::thread::sleep(Duration::from_millis(20));
            hand.frame(Vec::new());
        }
        let x0 = hand.app.project.sketches[0].points.iter().map(|q| q.x).fold(0.0_f64, f64::max) + 50.0;
        // the hand knows where the buttons it will press stand, as a person knows the panel: found while timed, the
        // looking over the hint of every button took 6 s of a step of 0.5 s
        for key in [
            "tb-line-hint",
            "tb-rect-hint",
            "tb-circle-hint",
            "tb-dim-hint",
            "tb-fillet-sketch-hint",
            "tb-trim-hint",
            "tb-move-hint",
            "tb-lin-array-hint",
            "con-horizontal-hint",
            "con-parallel-hint",
        ] {
            assert!(hand.know_hint(&crate::i18n::tr(key)), "no button {key} in the panel of the sketch");
        }
        let mut failures: Vec<String> = Vec::new();
        let mut times: Vec<String> = Vec::new();
        // the longest frame of a step: what a person waits - 55 - 88 ms measured
        let budget = Duration::from_millis(500);
        let mut held = |hand: &mut Hand, what: &str, took: Duration, ok: bool, failures: &mut Vec<String>| {
            let worst = hand.worst_frame();
            times.push(format!("{what}: the longest frame {worst:.0?} (the gesture {took:.1?})"));
            if worst > budget {
                failures.push(format!("{what}: a frame of {worst:?}, budget {budget:?}"));
            }
            if !ok {
                failures.push(format!("{what}: did not leave what it should"));
            }
            let mut whole = hand.app.project.clone();
            whole.regen_sketch_whole(0);
            if loops(&hand.app.project) != loops(&whole) {
                failures.push(format!("{what}: the loops are not those of a rebuild"));
            }
        };
        let _ = hand.worst_frame(); // the frames of the way in and of learning the panel are not a step
        let had = count(&hand);
        let took = timed(|| {
            hand.sk_tool(2).click2d(x0, 0.0).click2d(x0 + 40.0, 30.0).key(egui::Key::Escape).key(egui::Key::Escape);
        });
        let rect = new_lines(&hand, had.curves);
        let ok = rect.len() == 4;
        held(&mut hand, "a rectangle drawn", took, ok, &mut failures);
        let had = count(&hand);
        let took = timed(|| {
            hand.sk_tool(3).click2d(x0 + 90.0, 20.0).click2d(x0 + 100.0, 20.0).key(egui::Key::Escape).key(egui::Key::Escape);
        });
        let ok = count(&hand).curves == had.curves + 1;
        held(&mut hand, "a circle drawn", took, ok, &mut failures);
        let had = count(&hand);
        let took = timed(|| {
            hand.sk_tool(1).click2d(x0, 100.0).click2d(x0 + 30.0, 104.0).key(egui::Key::Escape).key(egui::Key::Escape);
            hand.sk_tool(1).click2d(x0 + 5.0, 112.0).click2d(x0 + 33.0, 121.0).key(egui::Key::Escape).key(egui::Key::Escape);
        });
        let two = new_lines(&hand, had.curves);
        let ok = two.len() == 2;
        held(&mut hand, "two lines drawn", took, ok, &mut failures);
        let (one, other) = (two.first().copied().unwrap_or(0), two.get(1).copied().unwrap_or(0));
        let had = count(&hand);
        let took = timed(|| {
            hand.sk_tool(0);
            let picked = hand.select2d(&[(1, one), (1, other)]);
            assert!(picked, "the two lines could not be picked");
            hand.constraint(3);
        });
        let ok = count(&hand).constraints > had.constraints;
        held(&mut hand, "Parallel laid by its button", took, ok, &mut failures);
        let had = count(&hand);
        let took = timed(|| {
            hand.sk_tool(0);
            assert!(hand.select2d(&[(1, one)]), "the line could not be picked");
            hand.constraint(1);
        });
        let ok = count(&hand).constraints > had.constraints;
        held(&mut hand, "Horizontal laid by its button", took, ok, &mut failures);
        let had = count(&hand);
        let took = timed(|| {
            hand.sk_tool(0);
            assert!(hand.press_hint(&crate::i18n::tr("tb-dim-hint")), "the dimension tool");
            hand.click2d(x0 + 20.0, 0.0);
            hand.click2d(x0 + 20.0, -8.0);
            hand.key(egui::Key::Enter).key(egui::Key::Escape);
        });
        let ok = count(&hand).constraints > had.constraints;
        held(&mut hand, "a dimension laid", took, ok, &mut failures);
        let had = count(&hand);
        let corners: Vec<(u8, u64)> = rect.iter().map(|&e| (1u8, e)).collect();
        let took = timed(|| {
            assert!(hand.sk_corner_set("tb-fillet-sketch-hint", &corners, 3.0), "the corners of the rectangle could not be picked");
        });
        let ok = count(&hand).curves > had.curves;
        held(&mut hand, "the corners of the rectangle filleted", took, ok, &mut failures);
        let had = count(&hand);
        let took = timed(|| {
            hand.sk_tool(1).click2d(x0 + 75.0, 20.0).click2d(x0 + 115.0, 20.0).key(egui::Key::Escape).key(egui::Key::Escape);
            hand.sk_tool(0);
            assert!(hand.press_hint(&crate::i18n::tr("tb-trim-hint")), "the trim tool");
            hand.click2d(x0 + 90.0, 20.0).key(egui::Key::Escape);
        });
        let ok = count(&hand).curves > had.curves;
        held(&mut hand, "a line drawn across the circle and trimmed inside it", took, ok, &mut failures);
        let before: Vec<(f64, f64)> = hand.app.project.sketches[0].points.iter().map(|q| (q.x, q.y)).collect();
        let took = timed(|| {
            hand.sk_move(1, (x0 + 19.0, 116.0), (x0 + 19.0, 116.0), (x0 + 19.0, 136.0));
            hand.key(egui::Key::Escape);
        });
        let moved = hand.app.project.sketches[0].points.iter().map(|q| (q.x, q.y)).zip(&before).any(|(a, b)| a != *b);
        let ok = moved;
        held(&mut hand, "a line moved", took, ok, &mut failures);
        let had = count(&hand);
        let took = timed(|| {
            hand.sk_tool(0);
            assert!(hand.select2d(&[(1, other)]), "the line could not be picked");
            hand.key(egui::Key::Delete).key(egui::Key::Escape);
        });
        let ok = count(&hand).curves < had.curves;
        held(&mut hand, "a line deleted with Delete", took, ok, &mut failures);
        let had = count(&hand);
        let took = timed(|| {
            hand.sk_tool(0);
            assert!(hand.select2d(&[(1, one)]), "the line could not be picked");
            assert!(hand.press_hint(&crate::i18n::tr("tb-lin-array-hint")), "the linear pattern");
            assert!(hand.type_after_word(&crate::i18n::tr("opt-count"), "5"), "the count of the pattern");
            assert!(hand.type_after_word(&crate::i18n::tr("opt-step-x"), "0"), "the step X");
            assert!(hand.type_after_word(&crate::i18n::tr("opt-step-y"), "6"), "the step Y");
            hand.key(egui::Key::Enter).key(egui::Key::Escape);
        });
        let ok = count(&hand).curves >= had.curves + 4;
        held(&mut hand, "a line patterned 5 times", took, ok, &mut failures);
        eprintln!("the tools on the sketch of the report: {}", times.join(", "));
        assert!(failures.is_empty(), "tools slow or wrong on the sketch of the report through the window:\n{}", failures.join("\n"));
    }
}
