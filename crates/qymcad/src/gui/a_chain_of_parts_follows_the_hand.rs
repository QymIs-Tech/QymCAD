//! DRAG THE FIRST JOINT AND THE THIRD PART GOES WITH IT RATHER THAN CATCHING UP LATER.
//!
//! Reported behaviour: with two joints, dragging the first one leaves the second joint and the third
//! part trailing behind, and what comes out is jelly; the first joint is horizontal, the second
//! vertical, the first is dragged along the horizon and the third part lags and catches up later.
//!
//! The third part is tied to the second by a VERTICAL slider: horizontally it has no freedom at all, so
//! it must follow the second one exactly and ON THE SAME FRAME. "Catches up later" is not "slowly" but
//! wrong: what is seen is a mechanism crawling apart under the hand.
//!
//! The check goes BY THAT PATH: the part is taken by itself (the same resolution the mouse uses) and led
//! step by step, the way a hand leads. The kernel computes this chain correctly
//! (`qymcad-core/tests/a_chain_of_mates_moves_as_one.rs`), so the question here is put to the
//! application — where the computation can be one frame late.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::App;
    use qymcad_core::feature::{AnchorRef, BasePlane, JointKind};
    use qymcad_core::model::Id;

    fn viewport() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0))
    }

    /// A point ON THE BODY that can be clicked: the centre of the topmost face.
    fn aim(app: &App, body: Id) -> [f64; 3] {
        let wt = app.project.body_display_transform(body, app.current_ctx_id_for_test());
        let f = app
            .project
            .regen_faces
            .get(&body)
            .and_then(|fs| fs.iter().max_by(|a, b| a.centroid.z.total_cmp(&b.centroid.z)))
            .expect("the body has faces");
        qymcad_core::feature::apply12(&wt, [f.centroid.x, f.centroid.y, f.centroid.z])
    }

    fn origin_of(app: &App, comp: Id) -> [f64; 3] {
        qymcad_core::feature::apply12(&app.project.world_transform(comp), [0.0, 0.0, 0.0])
    }

    /// A CHAIN OF THREE PARTS: A is grounded, A-B is a HORIZONTAL slider, B-C is a VERTICAL slider.
    ///
    /// The axes are given by the base planes of the component (the normal of YZ is X, the normal of XY is
    /// Z): that way each joint has an axis OF ITS OWN, and it does not depend on which way up the body
    /// happened to land. Returns (body B, component B, component C).
    fn a_chain_by_hand(app: &mut App) -> (Id, Id, Id) {
        let before: Vec<Id> = app.project.bodies.iter().map(|b| b.id).collect();
        for k in 0..3 {
            super::super::joint_flow::tests::add_part_at(app, k as f64 * 60.0);
        }
        let root = app.project.root;
        app.enter_component(root);
        app.rebuild_if_dirty();
        app.refresh_edges();
        let mine: Vec<Id> = app.project.bodies.iter().map(|b| b.id).filter(|b| !before.contains(b)).collect();
        assert_eq!(mine.len(), 3, "setup: there should be three bodies of our own, and there are {}", mine.len());
        let comps: Vec<Id> = mine.iter().map(|b| app.project.body_owner(*b).expect("the owner of the body")).collect();
        app.project.set_grounded(comps[0], true);

        let ja = app.project.add_connector(comps[0], AnchorRef::BasePlane(BasePlane::YZ)); // the normal is X
        let jb = app.project.add_connector(comps[1], AnchorRef::BasePlane(BasePlane::YZ));
        app.project.add_joint(ja, jb, JointKind::Slider);
        let kb = app.project.add_connector(comps[1], AnchorRef::BasePlane(BasePlane::XY)); // the normal is Z
        let kc = app.project.add_connector(comps[2], AnchorRef::BasePlane(BasePlane::XY));
        app.project.add_joint(kb, kc, JointKind::Slider);

        app.rebuild_if_dirty();
        app.refresh_edges();
        let mut hand = Hand::new(app);
        hand.look_at([60.0, 10.0, 5.0], 4.0);
        app.workbench = super::super::Workbench::Assembly;
        (mine[1], comps[1], comps[2])
    }

    /// LEAD THE SECOND PART ALONG THE HORIZON AND THE THIRD GOES WITH IT ON EVERY FRAME.
    #[test]
    fn the_third_part_never_lags_behind_while_the_hand_drags_the_second() {
        let mut app = App::default();
        let (body_b, comp_b, comp_c) = a_chain_by_hand(&mut app);
        app.project.solve_joints();
        // THE WINDOW IS LIVE: in it the rebuild goes into a thread, and a computation one frame late
        // would become visible.
        app.regen.ui_running = true;

        let basis = app.cam.basis();
        let at = app.project3(aim(&app, body_b), viewport(), &basis).0;
        let by = egui::vec2(60.0, 0.0);
        assert!(app.joint_grab_part_at_for_test(viewport(), at, by, &basis), "setup: the second part must be grabbable");

        let mut worst = 0.0f64;
        for k in 1..=8 {
            let step = by * (k as f32 / 8.0);
            app.joint_giz_drag_to_for_test(at + step, by / 8.0, viewport(), &basis);
            app.rebuild_if_dirty(); // the same thing a frame does
            let (pb, pc) = (origin_of(&app, comp_b), origin_of(&app, comp_c));
            // C is held by a VERTICAL slider: horizontally there is no freedom, so x and y must match B
            let lag = ((pc[0] - pb[0]).powi(2) + (pc[1] - pb[1]).powi(2)).sqrt();
            worst = worst.max(lag);
            assert!(
                lag < 1e-3,
                "frame {k}: the third part lagged behind the second by {lag:.3} mm (B at {pb:?}, C at {pc:?}) — that is the jelly"
            );
        }
        app.joint_giz_end_for_test();
        let (pb, pc) = (origin_of(&app, comp_b), origin_of(&app, comp_c));
        let lag = ((pc[0] - pb[0]).powi(2) + (pc[1] - pb[1]).powi(2)).sqrt();
        assert!(lag < 1e-3, "the part was released and the third one stayed {lag:.3} mm aside (B {pb:?}, C {pc:?}); the worst during the drag was {worst:.3}");
    }

    /// THE WHOLE PATH: "REBUILD EVERYTHING" AND THEN STRAIGHT INTO DRAGGING.
    ///
    /// Reported behaviour: Edit -> Rebuild everything is pressed, dragging begins, and the modal rebuild
    /// window flashes over and over; while dragging, the second joint is rubbery, does not keep up and
    /// arrives afterwards.
    ///
    /// The check goes exactly that way and asks TWO things at once, because the trouble was one:
    /// * there is ONE rebuild over the whole drag — the one that was actually asked for. Every extra one
    ///   in a live window is another flash of the modal window;
    /// * its result IS ACCEPTED, and the part stays where the hand led it. The result used to be thrown
    ///   away as stale (the placement was part of the fingerprint), a new rebuild was asked for at once
    ///   — and the circle did not break until the hand stopped.
    #[test]
    fn rebuild_everything_then_dragging_does_not_flash_the_modal_over_and_over() {
        let mut app = App::default();
        let (body_b, comp_b, comp_c) = a_chain_by_hand(&mut app);
        app.project.solve_joints();
        app.regen.ui_running = true; // a live window: the rebuild goes into a thread and draws a window

        app.rebuild_everything_for_test(); // "Edit -> Rebuild everything"
        assert!(app.regen.busy.is_some(), "setup: \"Rebuild everything\" must go into the background");

        let basis = app.cam.basis();
        let at = app.project3(aim(&app, body_b), viewport(), &basis).0;
        let by = egui::vec2(60.0, 0.0);
        assert!(app.joint_grab_part_at_for_test(viewport(), at, by, &basis), "setup: the part must be grabbable");

        // THE FIRST HALF OF THE DRAG — while the rebuild is still being computed in the thread
        for k in 1..=4 {
            app.joint_giz_drag_to_for_test(at + by * (k as f32 / 8.0), by / 8.0, viewport(), &basis);
        }
        let led_to = origin_of(&app, comp_b);

        app.drain_busy_for_test(); // the result has arrived — this is where it used to be thrown away
        assert!(
            !app.status.contains(&crate::i18n::tr("io-doc-changed")),
            "the result of the rebuild was thrown away because a part was being dragged: \"{}\"",
            app.status
        );
        let after = origin_of(&app, comp_b);
        let jump = ((after[0] - led_to[0]).powi(2) + (after[1] - led_to[1]).powi(2) + (after[2] - led_to[2]).powi(2)).sqrt();
        assert!(jump < 1e-3, "the rebuild arrived and threw the part {jump:.3} mm back: {led_to:?} -> {after:?}");

        // THE SECOND HALF — there must not be a single new rebuild
        app.regen.wanted = false;
        let mut asks = 0;
        for k in 5..=8 {
            app.joint_giz_drag_to_for_test(at + by * (k as f32 / 8.0), by / 8.0, viewport(), &basis);
            app.rebuild_if_dirty(); // the same thing a frame does
            if std::mem::take(&mut app.regen.wanted) {
                asks += 1;
            }
            let (pb, pc) = (origin_of(&app, comp_b), origin_of(&app, comp_c));
            let lag = ((pc[0] - pb[0]).powi(2) + (pc[1] - pb[1]).powi(2)).sqrt();
            assert!(lag < 1e-3, "frame {k}: the third part lagged by {lag:.3} mm — a rubbery joint (B {pb:?}, C {pc:?})");
        }
        assert_eq!(asks, 0, "after \"Rebuild everything\" the drag asked for {asks} more rebuilds — as many flashes of the window");
        app.joint_giz_end_for_test();
    }

    /// A DRAG IS ONE UNDO OPERATION, NOT ONE PER FRAME.
    ///
    /// Reported behaviour: the joint follows reluctantly, sluggishly. The cause is not in the solver but
    /// in the price of a frame: the boundary of an operation (`begin_edit`) takes A FULL COPY of the
    /// document, and it was being taken on EVERY frame of the drag — opened and closed inside one
    /// movement of the mouse.
    ///
    /// A MEASUREMENT ON A REAL DOCUMENT (138 bodies with meshes): one frame of a drag cost 13-18 ms for
    /// THAT ALONE, without any drawing; after the fix, 3.5-4.5 ms. Fivefold. That is what is seen as
    /// "does not keep up".
    ///
    /// The check measures not milliseconds (they float from machine to machine) but THE MECHANISM: the
    /// copy is taken once per drag, so there is exactly one step of undo. That is also what is expected:
    /// one movement of the mouse is rolled back by one Ctrl+Z rather than by twenty.
    #[test]
    fn one_drag_is_one_undo_step_not_one_per_frame() {
        let mut app = App::default();
        let (body_b, _, _) = a_chain_by_hand(&mut app);
        app.project.solve_joints();
        app.rebuild_if_dirty();

        let basis = app.cam.basis();
        let at = app.project3(aim(&app, body_b), viewport(), &basis).0;
        let by = egui::vec2(60.0, 0.0);
        let before = app.undo_len_for_test();
        assert!(app.joint_grab_part_at_for_test(viewport(), at, by, &basis), "setup: the part must be grabbable");
        for k in 1..=8 {
            app.joint_giz_drag_to_for_test(at + by * (k as f32 / 8.0), by / 8.0, viewport(), &basis);
        }
        app.joint_giz_end_for_test();

        let steps = app.undo_len_for_test() - before;
        assert_eq!(
            steps, 1,
            "one drag put {steps} steps into the undo stack — so a copy of the document was taken on every frame, and on a large assembly that is exactly the reluctant following"
        );
    }

    /// A DOCUMENT WHERE THE LIVE B-rep IS NOT ALL RAISED — AND THE DRAG STILL STAYS SILENT.
    ///
    /// This is what a REAL document looks like: a hundred-odd imported bodies, live geometry raised
    /// lazily and not for all of them at once, and a node that did not get enough of it honestly stays
    /// with the error "the body is not built" — which is temporary (`retryable`), and the program must
    /// try again once the input appears.
    ///
    /// AND HERE IS WHERE THE THIRD DOOR HID. The retry was guarded by a flag saying "this revision has
    /// been tried already", and the revision was `geom_rev` — the counter of the DRAWING cache. Every
    /// drag of a part moves it (`invalidate` on every frame of the drag), though the live B-rep does not
    /// depend on where a part stands at all. The result: a frame of a drag -> a different revision -> the
    /// preparation runs again -> it marks nodes dirty and DEMANDS a rebuild (an explicit request bypasses
    /// every check of the scheduler) -> the window. Twenty times a second until the hand stops.
    #[test]
    fn dragging_is_silent_even_when_the_live_brep_is_not_ready() {
        let mut app = App::default();
        let (body_b, comp_b, _) = a_chain_by_hand(&mut app);
        app.project.solve_joints();
        app.regen.ui_running = true;

        // A JOINT ON A FACE — as in a real document. It is what makes a frame bring the live B-rep up
        // EVERY TIME (`needs_live_brep`), and without it the trouble does not reproduce at all: a scene
        // on base planes alone demands no live geometry, and the check would lie that all is quiet.
        let extra = {
            let faces = app.project.regen_faces.get(&body_b).cloned().expect("the faces of the part");
            let (fi, f) = faces.iter().enumerate().max_by(|a, b| a.1.normal[2].total_cmp(&b.1.normal[2])).expect("a +Z face");
            let key = qymcad_core::feature::FaceKey { index: fi as u32, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id };
            app.project.add_connector(comp_b, qymcad_core::feature::AnchorRef::FaceCenter(body_b, key))
        };
        assert!(
            app.project.connectors.iter().any(|c| c.id == extra),
            "setup: the anchor on a face must be created"
        );
        let free = app.project.add_part("hanger");
        let cf = app.project.add_connector(free, AnchorRef::BasePlane(BasePlane::XY));
        app.project.add_joint(extra, cf, JointKind::Rigid); // the face anchor is in a joint now, so the frame demands a live B-rep

        // THE STATE OF A REAL DOCUMENT: the live geometry is not all raised, a node waits for its input.
        let node = app.project.timeline.iter().find(|n| n.kind.body().is_some()).map(|n| n.id).expect("a body node");
        app.live.shapes.clear();
        app.live.ready = false;
        app.project.regen_errors.insert(node, qymcad_core::errors::CoreError::SourceBodyNotBuilt);
        assert!(app.project.regen_errors.values().any(|e| e.retryable()), "setup: the error must be a temporary one");

        let basis = app.cam.basis();
        let at = app.project3(aim(&app, body_b), viewport(), &basis).0;
        let by = egui::vec2(60.0, 0.0);
        assert!(app.joint_grab_part_at_for_test(viewport(), at, by, &basis), "setup: the part must be grabbable");
        // THE FRAME BEFORE THE DRAG: the first attempt to raise the live B-rep is lawful and worth a
        // rebuild of its own. Only what THE DRAG ITSELF asks for is counted.
        app.refresh_edges();
        app.rebuild_if_dirty();
        app.regen.wanted = false;

        let mut asks = 0;
        for k in 1..=8 {
            app.joint_giz_drag_to_for_test(at + by * (k as f32 / 8.0), by / 8.0, viewport(), &basis);
            // A FRAME MAKES BOTH CALLS, AND THE SECOND IS THE IMPORTANT ONE HERE. `refresh_edges` runs
            // on EVERY frame in 3D, and in a document with joints on faces it brings the live B-rep up
            // each time. A check that calls one scheduler does not see the trouble — and did not.
            app.refresh_edges();
            app.rebuild_if_dirty();
            if std::mem::take(&mut app.regen.wanted) {
                asks += 1;
            }
        }
        app.joint_giz_end_for_test();
        assert_eq!(
            asks, 0,
            "on a document with the B-rep not raised, the drag asked for {asks} rebuilds — as many flashes of the rebuild window"
        );
        // and the part did arrive after all: the silence must not cost the movement
        let now = origin_of(&app, comp_b);
        assert!(now[0].abs() > 1.0, "the part did not move at all: {now:?}");
    }
}
