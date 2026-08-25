//! DRAG A PART ALONG A JOINT AND THE REBUILD WINDOW DOES NOT FLICKER.
//!
//! Reported behaviour: while a joint is being moved, the modal rebuild window flashes endlessly,
//! reading "0 of 0 parts".
//!
//! What actually happens. Dragging a part along a degree of freedom changes THE ARRANGEMENT (the value
//! of the joint and the transforms of the components) and not the feature timeline: not one body is
//! rebuilt because of it. But the frame scheduler compares "did anything change" by the key of the
//! WHOLE document, and the value of a joint is part of it. While the timeline holds even one dirty
//! node (a live document nearly always has one: a node that lacked an input stays dirty deliberately),
//! every frame of a drag reads as "the document moved, compute it again", and a person gets a modal
//! window 20 times a second.
//!
//! The check measures exactly what a person sees: how many times a rebuild was asked for during one
//! drag.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::App;
    use qymcad_core::feature::JointKind;
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

    /// A SLIDER BETWEEN TWO PARTS, assembled BY HAND; the first one is grounded.
    fn a_slider_by_hand(app: &mut App) -> (Id, Id) {
        let before: Vec<Id> = app.project.bodies.iter().map(|b| b.id).collect();
        super::super::joint_flow::tests::add_part_at(app, 0.0);
        super::super::joint_flow::tests::add_part_at(app, 60.0);
        let root = app.project.root;
        app.enter_component(root);
        app.rebuild_if_dirty();
        app.refresh_edges();
        let mine: Vec<Id> = app.project.bodies.iter().map(|b| b.id).filter(|b| !before.contains(b)).collect();
        assert_eq!(mine.len(), 2, "setup: there should be two bodies of our own, and there are {}", mine.len());
        for (k, b) in mine.iter().enumerate() {
            if let Some(o) = app.project.body_owner(*b) {
                if let Some(i) = app.project.component_index(o) {
                    app.project.components[i].transform = [1.0, 0.0, 0.0, k as f64 * 60.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];
                }
                if k == 0 {
                    app.project.set_grounded(o, true);
                }
            }
        }
        app.rebuild_if_dirty();
        app.refresh_edges();
        let (pa, pb) = (aim(app, mine[0]), aim(app, mine[1]));
        let mut hand = Hand::new(app);
        hand.look_at([30.0, 10.0, 5.0], 6.0).mate(JointKind::Slider).anchor(3).click(pa).click(pb);
        app.rebuild_if_dirty();
        let jid = app.project.joints.last().map(|j| j.id).expect("two clicks must create the joint");
        (jid, mine[1])
    }

    /// A DIRTY NODE THAT LIVES IN THE DOCUMENT PERMANENTLY.
    ///
    /// That is what a real document looks like: a node that lacked an input stays marked deliberately
    /// — the attempt must happen again once the input appears. Without it the trouble does not
    /// reproduce at all, and the check would say "all is well" exactly where things are bad for a
    /// person.
    fn leave_one_node_dirty(app: &mut App) {
        let node = app.project.timeline.iter().find(|n| n.kind.body().is_some()).map(|n| n.id).expect("the node of the body");
        app.project.mark_node_dirty(node);
        app.rebuild_if_dirty();
        app.project.mark_node_dirty(node); // the rebuild cleared it — mark it again, like a failing node
    }

    /// HOW MANY TIMES A REBUILD WAS ASKED FOR DURING A DRAG.
    ///
    /// What is counted is exactly what makes the modal window flash in a live window: `regen_wanted`.
    /// The frame is repeated as in `update` — the scheduler is called once per frame.
    fn rebuilds_asked_during_a_drag(app: &mut App, body: Id) -> usize {
        // THE WINDOW IS LIVE, as it is for a person: the rebuild goes into a thread and draws that
        // very window. That is also the condition under which a dirty node STAYS dirty for the length
        // of the drag: the thread is computing, the marks are cleared by its result, and it has not
        // arrived yet.
        app.regen.ui_running = true;
        app.rebuild_if_dirty(); // the frame BEFORE the drag: let what has accumulated be computed — that is legitimate
        app.regen.wanted = false;
        let basis = app.cam.basis();
        let at = app.project3(aim(app, body), viewport(), &basis).0;
        let by = egui::vec2(40.0, 0.0);
        assert!(app.joint_grab_part_at_for_test(viewport(), at, by, &basis), "setup: it must be possible to grab the part");
        let mut asks = 0;
        for k in 1..=8 {
            let step = by * (k as f32 / 8.0);
            app.joint_giz_drag_to_for_test(at + step, by / 8.0, viewport(), &basis);
            app.rebuild_if_dirty(); // the same thing a frame does
            if std::mem::take(&mut app.regen.wanted) {
                asks += 1;
            }
        }
        app.joint_giz_end_for_test();
        asks
    }

    /// DRAG THE PART — A REBUILD IS NOT ASKED FOR ONCE.
    #[test]
    fn dragging_a_part_along_its_freedom_never_asks_for_a_rebuild() {
        let mut app = App::default();
        let (_, moving) = a_slider_by_hand(&mut app);
        leave_one_node_dirty(&mut app);
        let asks = rebuilds_asked_during_a_drag(&mut app, moving);
        assert_eq!(
            asks, 0,
            "a rebuild was asked for {asks} times during one drag — in a live window that is as many flashes of the modal rebuild window"
        );
    }

    /// THE OTHER SIDE OF THE SAME CHANGE: WHERE THE ARRANGEMENT REALLY DRIVES THE GEOMETRY, A REBUILD
    /// MUST HAPPEN.
    ///
    /// It is easy to make the cure for the flicker worse than the illness: stop rebuilding altogether.
    /// But a sketch placed on a face of SOMEBODY ELSE'S part is a live external reference (top-down),
    /// and moving the source really does change its geometry. Such a rebuild must happen — once, on
    /// releasing the part, and not on every frame of the drag.
    #[test]
    fn a_part_that_feeds_someone_else_still_rebuilds_when_it_stops_moving() {
        use qymcad_core::feature::{FaceKey, SketchPlane};
        let mut app = App::default();
        let (_, moving) = a_slider_by_hand(&mut app);

        // THE NEIGHBOUR'S SKETCH ON A FACE OF THE DRIVEN PART — that very external reference.
        let faces = app.project.regen_faces.get(&moving).cloned().expect("the faces of the driven part");
        let (fi, f) = faces.iter().enumerate().max_by(|a, b| a.1.normal[2].total_cmp(&b.1.normal[2])).expect("the +Z face");
        let key = FaceKey { index: fi as u32, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id };
        let neighbour = app.project.add_part("neighbour");
        app.enter_component(neighbour);
        app.create_sketch_on(SketchPlane::Face(moving, key));
        app.finish_sketch_edit();
        app.exit_context(); // back into the assembly, by the same path a person takes
        assert!(app.project.external_ref_for(neighbour, moving).is_some(), "setup: a sketch on somebody else's face must create an external reference");
        app.rebuild_if_dirty();
        // THE HAND IS BACK IN THE ASSEMBLY: leaving the part reset both the workbench and the camera.
        let mut hand = Hand::new(&mut app);
        hand.look_at([30.0, 10.0, 5.0], 6.0);
        app.workbench = super::super::Workbench::Assembly;
        app.refresh_edges();

        let asks = rebuilds_asked_during_a_drag(&mut app, moving);
        assert_eq!(asks, 0, "dragging a part does not rebuild the timeline even with an external reference: it was asked for {asks} times");
        // `rebuilds_asked_during_a_drag` ends with the release — now the rebuild must happen
        assert!(
            app.regen.wanted,
            "the part was released and nobody rebuilt the consumer of its face — the top-down associativity is lost"
        );
    }
}
