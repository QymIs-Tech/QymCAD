//! A MESH BEING RECOGNISED, AND THE PART OR THE MESH DELETED BEFORE IT IS DONE.
//!
//! Reported behaviour: after deleting, or after recognising a body from an STL, the spinner turns for ever while
//! nothing happens, and the part with the import is already deleted. It happens now and then - for instance when a
//! body is being recognised and, without waiting for it, the part or the STL is deleted.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::import_door::tests::{answer, settle};
    use qymcad_ui_state::{Sel, Want};

    /// A ball of radius 20 standing on the ground, as a mesh of 400 x 200 bands (160k triangles): recognising it takes
    /// 14 s in a debug build, 11 s of them growing the regions - long enough for the deletion to land in the middle.
    fn ball() -> qymcad_core::geom::Mesh {
        let (n, m, r) = (400usize, 200usize, 20.0f64);
        let mut verts = vec![qymcad_core::geom::Point3::new(0.0, 0.0, 0.0)];
        for j in 1..m {
            let t = std::f64::consts::PI * j as f64 / m as f64;
            for i in 0..n {
                let a = 2.0 * std::f64::consts::PI * i as f64 / n as f64;
                verts.push(qymcad_core::geom::Point3::new(r * t.sin() * a.cos(), r * t.sin() * a.sin(), r - r * t.cos()));
            }
        }
        verts.push(qymcad_core::geom::Point3::new(0.0, 0.0, 2.0 * r));
        let top = verts.len() - 1;
        let at = |j: usize, i: usize| 1 + (j - 1) * n + i % n;
        let mut tris = Vec::new();
        for i in 0..n {
            tris.push([0, at(1, i + 1), at(1, i)]);
            tris.push([top, at(m - 1, i), at(m - 1, i + 1)]);
        }
        for j in 1..m - 1 {
            for i in 0..n {
                tris.push([at(j, i), at(j, i + 1), at(j + 1, i)]);
                tris.push([at(j, i + 1), at(j + 1, i + 1), at(j + 1, i)]);
            }
        }
        qymcad_core::geom::Mesh { verts, tris: tris.into_iter().map(|t: [usize; 3]| t.map(|k| k as u32)).collect(), ..Default::default() }
    }

    /// A project holding the ball brought in from a file; returns the program and the part the ball landed in.
    fn a_ball_brought_in(name: &str) -> (super::super::App, qymcad_core::model::Id) {
        let dir = std::path::PathBuf::from(format!("{}/../../target/recognise-then-delete", env!("CARGO_MANIFEST_DIR")));
        std::fs::create_dir_all(&dir).expect("a folder for the check");
        let p = dir.join(format!("{name}.3mf"));
        qymcad_io::export_3mf(&[ball()], &p.to_string_lossy()).expect("written");
        let (mut app, ctx) = super::super::import_door::tests::running();
        answer(&mut app, &ctx, Want::Anything, &p.to_string_lossy());
        settle(&mut app, &ctx);
        let ball = app.project.bodies.last().map(|b| b.id).expect("the ball came in");
        let part = app.project.body_owner(ball).expect("the ball landed in a part");
        app.enter_component(part);
        (app, part)
    }

    /// Frames until the program is at rest (`session::waiting::until`): no job in the background, no rebuild running,
    /// nothing asked to be rebuilt, no sign of a rebuild on screen for ten frames in a row - a sign stays its time after
    /// its rebuild lands.
    fn comes_to_rest(h: &mut Hand, what: &str) {
        let mut quiet = 0;
        crate::gui::session::waiting::until(what, || {
            h.frame(Vec::new());
            let busy = h.app.regen.busy.is_some() || !h.app.regen.bg.is_empty() || h.app.regen.wanted || !crate::gui::tests::a_quick_rebuild_does_not_blink::tests::signs_painted(h).is_empty();
            quiet = if busy { 0 } else { quiet + 1 };
            quiet >= 10
        });
    }

    /// THE STOP OF THE REBUILD RUNNING NOW, taken before what is done to it: it is read after the rebuild has gone.
    fn stop_of_the_rebuild(h: &Hand) -> std::sync::Arc<std::sync::atomic::AtomicBool> {
        h.app.regen.busy.as_ref().and_then(|b| b.pulse.as_ref()).map(|p| p.stop.clone()).unwrap_or_else(|| panic!("setup: no rebuild is running: {}", h.app.status))
    }

    /// The rebuild whose stop is `stop` has been asked to stop.
    fn asked(stop: &std::sync::atomic::AtomicBool) -> bool {
        stop.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// What the canvas says while the program is at rest: the spinner of a quiet rebuild, or the bar still counting.
    fn what_still_turns(h: &mut Hand) -> Vec<String> {
        h.frame(Vec::new());
        let mut out = Vec::new();
        if !crate::gui::tests::a_quick_rebuild_does_not_blink::tests::signs_painted(h).is_empty() || h.app.regen.busy.is_some() {
            out.push(format!("the spinner of a rebuild: {}", h.app.status));
        }
        if h.app.tools.armed.cmd_kind() == 35 && h.app.params.recognise.src.is_some() && h.app.params.recognise.ready(|_| ()).is_none() {
            out.push("the bar of the recognition is still looking for surfaces".into());
        }
        out
    }

    /// THE PART DELETED WHILE THE TOOL IN HAND COUNTS ITS MESH: the tool lets the mesh go.
    #[test]
    fn deleting_the_part_while_the_mesh_is_counted_leaves_nothing_turning() {
        let (mut app, part) = a_ball_brought_in("counted");
        let mut h = Hand::new(&mut app);
        h.tool(35).frame(Vec::new());
        h.click([0.0, 0.0, 40.0]).frame(Vec::new());
        assert!(h.app.params.recognise.src.is_some(), "setup: the click on the ball was not taken: {}", h.app.status);
        let ci = h.app.project.components.iter().position(|c| c.id == part).expect("the part");
        qymcad_ui_state::ask_delete(&mut h.app.deferred, Sel::Component(ci));
        h.app.execute_deferred_delete_for_test();
        assert!(h.app.project.components.iter().all(|c| c.id != part), "setup: the part was not deleted");
        comes_to_rest(&mut h, "the program to come to rest after the part was deleted");
        let turning = what_still_turns(&mut h);
        assert!(turning.is_empty(), "the part is deleted and something still turns: {turning:?}");
    }

    /// THE PART DELETED WHILE ITS MESH IS BEING RECOGNISED: the rebuild is asked to stop at once, it ends, and nothing
    /// turns afterwards. That the recognition hears the stop in the middle of growing its regions is the split's own
    /// check (`a_stop_asked_on_the_way_ends_the_split_at_once`); held to 8 s of the wall here, the rest went red on a
    /// runner of the CI that needed longer for the preparation, which is not broken off.
    #[test]
    fn deleting_the_part_while_the_mesh_is_recognised_leaves_nothing_turning() {
        let (mut app, part) = a_ball_brought_in("recognised");
        let mut h = Hand::new(&mut app);
        h.tool(35).frame(Vec::new());
        h.click([0.0, 0.0, 40.0]).frame(Vec::new());
        assert!(h.app.params.recognise.src.is_some(), "setup: the click on the ball was not taken: {}", h.app.status);
        h.enter().frame(Vec::new());
        let stop = stop_of_the_rebuild(&h);
        let ci = h.app.project.components.iter().position(|c| c.id == part).expect("the part");
        qymcad_ui_state::ask_delete(&mut h.app.deferred, Sel::Component(ci));
        h.app.execute_deferred_delete_for_test();
        h.frame(Vec::new());
        assert!(asked(&stop), "the part is deleted and the rebuild recognising its mesh was not asked to stop - it goes on over nothing: {}", h.app.status);
        comes_to_rest(&mut h, "the program to come to rest after the part was deleted");
        let turning = what_still_turns(&mut h);
        assert!(turning.is_empty(), "the part is deleted and something still turns: {turning:?}");
    }

    /// THE MESH ITSELF DELETED WHILE IT IS BEING RECOGNISED: the same.
    #[test]
    fn deleting_the_mesh_while_it_is_recognised_leaves_nothing_turning() {
        let (mut app, _) = a_ball_brought_in("mesh-gone");
        let mut h = Hand::new(&mut app);
        h.tool(35).frame(Vec::new());
        h.click([0.0, 0.0, 40.0]).frame(Vec::new());
        let src = h.app.params.recognise.src.expect("setup: the click on the ball was not taken");
        h.enter().frame(Vec::new());
        let stop = stop_of_the_rebuild(&h);
        let mi = h.app.project.mesh_index(src).expect("the mesh");
        qymcad_ui_state::ask_delete(&mut h.app.deferred, Sel::Mesh(mi));
        h.app.execute_deferred_delete_for_test();
        h.frame(Vec::new());
        assert!(asked(&stop), "the mesh is deleted and the rebuild recognising it was not asked to stop - it goes on over nothing: {}", h.app.status);
        comes_to_rest(&mut h, "the program to come to rest after the mesh was deleted");
        let turning = what_still_turns(&mut h);
        assert!(turning.is_empty(), "the mesh is deleted and something still turns: {turning:?}");
    }

    /// CANCEL STOPS A LONG RECOGNITION WHERE IT IS, not after it. Reported behaviour: the rebuild window said
    /// "feature 0 of 1" for minutes, and its Cancel cancelled nothing - the stop was asked only between nodes, and the
    /// recognition is one node.
    #[test]
    fn cancel_stops_a_long_recognition() {
        let (mut app, _) = a_ball_brought_in("cancelled");
        let mut h = Hand::new(&mut app);
        h.tool(35).frame(Vec::new());
        h.click([0.0, 0.0, 40.0]).frame(Vec::new());
        assert!(h.app.params.recognise.src.is_some(), "setup: the click on the ball was not taken: {}", h.app.status);
        let nodes = h.app.project.timeline.len();
        h.enter().frame(Vec::new());
        let stop = stop_of_the_rebuild(&h);
        let cancel = crate::i18n::tr("io-rebuild-cancel");
        // the card goes up once the rebuild has run `REBUILD_SIGN_AFTER`; Cancel is pressed when it is there
        crate::gui::session::waiting::until("the card of the recognition with its Cancel", || h.shows(&cancel));
        assert!(h.press_word(&cancel, egui::pos2(700.0, 450.0)), "setup: the rebuild window shows no Cancel");
        assert!(asked(&stop), "Cancel was pressed and the recognition was not asked to stop: {}", h.app.status);
        comes_to_rest(&mut h, "the recognition to stop after Cancel");
        let body = h.app.project.timeline.iter().skip(nodes.saturating_sub(1)).any(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::MeshRecognised { .. }) && !n.dirty);
        assert!(!body, "the recognition was cancelled and its node stands built all the same");
        assert!(h.app.project.regen_errors.is_empty(), "Cancel left the node red, as if it had failed: {:?}", h.app.project.regen_errors);
        assert!(h.app.status.words.contains(&crate::i18n::tr("io-rebuild-cancelled")), "Cancel was pressed and the program does not say the rebuild was cancelled: {}", h.app.status);
    }
}
