//! JOINTS IN THE LIVE FLOW OF THE APPLICATION: from a click on geometry to parts standing in place.
//!
//! The kernel of the solver has tests of its own, but between it and a person lies the whole path of the
//! application: the anchor mode, two clicks, the creation of connectors, the drag of a gizmo, the
//! rebuild. It was that path the reports were about — choosing an anchor hangs, the joints do not work —
//! and it was not covered.
#[cfg(test)]
pub(in crate::gui) mod tests {
    use super::super::{App, Sel};

    /// A separate PART with a body of its own at a distance `x` from the origin.
    ///
    /// The part is created IN THE ROOT OF THE ASSEMBLY: otherwise they nest inside one another in a chain
    /// and "two parts side by side" turns out to be one branch — such parts cannot be joined, and the test
    /// would check the wrong thing.
    pub(in crate::gui) fn add_part_at(app: &mut App, x: f64) {
        let root = app.project.root;
        app.project.set_active_component(Some(root));
        let part = app.project.add_part(format!("Part {x}"));
        app.enter_component(part);
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, x, 0.0, x + 20.0, 20.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        app.sel = Sel::Sketch(si);
        app.start_feat_cmd(1);
        if let Some(p) = app.cmd.params.iter_mut().find(|p| p.key == "height") {
            p.val = 10.0;
            p.txt = "10".into();
        }
        app.apply_feat_cmd();
    }

    /// A JOINT ON EDGES GOES FROM THE CLICK TO THE SOLUTION.
    ///
    /// The reports were about the edge and vertex anchors specifically. The kernel is covered for
    /// `EdgeMid` connectors and the flow of the application is not: the click, collecting the second
    /// anchor, creating the joint and placing the parts. What is checked here is a cylindrical joint
    /// between two NEIGHBOURING parts: the grounded one stays where it is and the free one must come onto
    /// its axis.
    #[test]
    fn a_joint_on_edges_goes_from_click_to_solution() {
        let mut app = App::default();
        add_part_at(&mut app, 0.0);
        add_part_at(&mut app, 200.0);
        let root = app.project.root;
        app.enter_component(root);
        app.rebuild_if_dirty();
        let bodies: Vec<_> = (0..app.project.bodies.len()).filter_map(|mi| app.project.mesh_id(mi)).collect();
        assert_eq!(bodies.len(), 2, "two parts mean two anchors for a joint");
        let (ba, bb) = (bodies[0], bodies[1]);
        let edge_of = |app: &App, b: qymcad_core::model::Id| -> u32 {
            app.body_edges_cached(b).and_then(|e| e.1.iter().copied().find(|&id| id != 0)).expect("the body has edges with persistent ids")
        };
        let (ea, eb) = (edge_of(&app, ba), edge_of(&app, bb));
        let owner_a = app.project.body_owner(ba).expect("the owner of A");
        let owner_b = app.project.body_owner(bb).expect("the owner of B");
        app.project.set_grounded(owner_a, true);
        let ground_before = app.project.world_transform(owner_a);

        app.joint.pick_faces = true;
        app.joint.anchor_mode = 1; // THE EDGE ANCHOR — the very mode that was reported
        app.joint.new_kind = qymcad_core::feature::JointKind::Cylindrical;
        app.joint_pick_edge_click(ba, ea);
        assert!(app.joint.pick_first.is_some(), "the first click must fix anchor A");
        app.joint_pick_edge_click(bb, eb);
        assert_eq!(app.project.joints.len(), 1, "two clicks on edges must create a joint; the status line: {}", app.status);
        assert_eq!(app.project.connectors.len(), 2, "a joint must have two anchors");
        let (ja, jb) = (app.project.joints[0].a, app.project.joints[0].b);
        for cid in [ja, jb] {
            let c = app.project.connector(cid).expect("the anchor is in place");
            assert!(matches!(c.anchor, qymcad_core::feature::AnchorRef::EdgeMid(..)), "the anchor must be on an edge, and it is {:?}", c.anchor);
        }
        app.rebuild_if_dirty();

        // THE GROUNDED ONE DOES NOT MOVE, the free one comes onto the axis of the grounded one.
        let ground_after = app.project.world_transform(owner_a);
        assert!(
            ground_before.iter().zip(ground_after.iter()).all(|(x, y)| (x - y).abs() < 1e-9),
            "a joint has no right to move a grounded part"
        );
        let (fa, fb) = (app.project.connector_matrix(ja).expect("the frame of A"), app.project.connector_matrix(jb).expect("the frame of B"));
        let wa = qymcad_core::feature::mat_mul12(&app.project.world_transform(owner_a), &fa);
        let wb = qymcad_core::feature::mat_mul12(&app.project.world_transform(owner_b), &fb);
        let axis = [wa[2], wa[6], wa[10]];
        let d = [wb[3] - wa[3], wb[7] - wa[7], wb[11] - wa[11]];
        let along = d[0] * axis[0] + d[1] * axis[1] + d[2] * axis[2];
        let perp = [d[0] - along * axis[0], d[1] - along * axis[1], d[2] - along * axis[2]];
        let off = (perp[0] * perp[0] + perp[1] * perp[1] + perp[2] * perp[2]).sqrt();
        assert!(off < 1e-3, "a cylindrical joint must bring the axes of the anchors together, and the miss is {off:.4} mm");
    }

    /// DRAGGING THE GIZMO MOVES THE PART AND PINS THE VALUE.
    ///
    /// A gizmo is an explicit action, so it SETS a value rather than showing one. The drag used to write
    /// into the reading field, and the difference was invisible while the reading served as the setting.
    /// After they were separated this path must be checked on its own: otherwise the gizmo would be
    /// dragged for nothing, the solver would return the part at once, and it would look like the joint
    /// not obeying the mouse.
    #[test]
    fn dragging_the_gizmo_moves_the_part_and_pins_the_value() {
        use qymcad_core::feature::JointKind;
        let mut app = App::default();
        add_part_at(&mut app, 0.0);
        add_part_at(&mut app, 200.0);
        let root = app.project.root;
        app.enter_component(root);
        app.rebuild_if_dirty();
        let bodies: Vec<_> = (0..app.project.bodies.len()).filter_map(|mi| app.project.mesh_id(mi)).collect();
        let (ba, bb) = (bodies[0], bodies[1]);
        let owner_a = app.project.body_owner(ba).expect("the owner of A");
        let owner_b = app.project.body_owner(bb).expect("the owner of B");
        app.project.set_grounded(owner_a, true);
        let (ca, cb) = (
            app.project.add_connector(owner_a, qymcad_core::feature::AnchorRef::Origin),
            app.project.add_connector(owner_b, qymcad_core::feature::AnchorRef::Origin),
        );
        let jid = app.project.add_joint(ca, cb, JointKind::Slider);
        app.project.solve_joints();
        let before = app.project.world_transform(owner_b);

        // the slide handle (slot 1) is dragged by 40 mm
        app.joint.giz_drag = Some(super::super::JointGizDrag {
            jid,
            slot: 1,
            ring: false,
            start: 0.0,
            amt: 40.0,
            o: [0.0; 3],
            dir: [0.0, 0.0, 1.0],
        });
        app.apply_joint_giz();

        let j = app.project.joints.iter().find(|x| x.id == jid).expect("the joint is in place");
        assert_eq!(j.drive[1], Some(40.0), "the drag must SET a value rather than record a reading");
        let after = app.project.world_transform(owner_b);
        let moved = ((after[3] - before[3]).powi(2) + (after[7] - before[7]).powi(2) + (after[11] - before[11]).powi(2)).sqrt();
        assert!((moved - 40.0).abs() < 1e-3, "the part must travel exactly the 40 mm that were set, and it travelled {moved:.3}");

        // and it does NOT travel back on the next solve: what was set holds
        app.project.solve_joints();
        let again = app.project.world_transform(owner_b);
        let drift = ((again[3] - after[3]).powi(2) + (again[7] - after[7]).powi(2) + (again[11] - after[11]).powi(2)).sqrt();
        assert!(drift < 1e-6, "the value that was set must hold, and the part travelled a further {drift:.3} mm");
    }

    /// AN EDGE ANCHOR NEEDS THE LIVE B-rep — AND MUST ASK FOR IT.
    ///
    /// A project opened from a bundle carries no live B-rep: it is assembled ON DEMAND. The demand is
    /// stated by the chamfer, the fillet and the pick of a sketch plane — and the edge and vertex anchors
    /// of a joint did not state it. Their edges come from exactly the same place, so on a freshly opened
    /// project hovering and clicking an edge found NOTHING: silently, with no error and no hint.
    #[test]
    fn picking_an_edge_anchor_asks_for_the_live_brep() {
        let mut app = App::default();
        add_part_at(&mut app, 0.0);
        app.rebuild_if_dirty();
        // the state of having just opened a bundle: there are meshes and faces enough, and no live B-rep
        app.live.shapes.clear();
        app.live.ready = false;
        app.live.tried_rev = None;

        app.joint.pick_faces = true;
        app.joint.anchor_mode = 1; // the EDGE anchor
        app.refresh_edges();
        assert!(
            app.live.ready || app.regen.wanted || !app.live.shapes.is_empty(),
            "choosing an edge anchor must ask for the live B-rep, otherwise there is nothing to click and nobody will understand why"
        );
    }

    /// A SKETCH ON A FACE SHOWS THE OUTLINE OF THAT FACE ON A FRESHLY OPENED PROJECT TOO.
    ///
    /// The outline comes from the live B-rep, which is assembled on demand. The demand was stated only
    /// while CHOOSING the plane of a sketch; once the sketch was open on a face it disappeared — and
    /// after opening a bundle the face under the sketch came out empty: draw blind, with nothing to snap
    /// to.
    #[test]
    fn a_sketch_on_a_face_still_shows_its_outline_after_opening_a_bundle() {
        let mut app = App::default();
        add_part_at(&mut app, 0.0);
        app.rebuild_if_dirty();
        let body = app.project.mesh_id(0).expect("the body");
        let mi = app.project.mesh_index(body).expect("the mesh");
        let face = app.project.bodies[mi].faces[0].clone();
        let key = qymcad_core::feature::FaceKey {
            index: 0,
            centroid: [face.centroid.x, face.centroid.y, face.centroid.z],
            normal: face.normal,
            id: face.id,
        };
        // first as it stands (the B-rep is live), then as after opening a bundle
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::Face(body, key));
        let with_brep = app.sketch_ref_edges_2d(si).len();
        app.live.shapes.clear();
        app.live.ready = false;
        app.live.tried_rev = None;
        app.refresh_edges();
        let after_open = app.sketch_ref_edges_2d(si).len();
        assert!(with_brep > 0, "setup: with a live B-rep the outline of the face must be visible");
        assert_eq!(after_open, with_brep, "after opening a bundle the outline of the face under the sketch must be the same rather than empty");
    }

    /// THE THREAD TOOL ALSO WORKS OFF THE LIVE B-rep — and without it, it lied.
    ///
    /// The axis of a cylindrical face comes from the live B-rep. The thread tool did not ask for it, and
    /// on a freshly opened project a click on a cylinder answered "a miss — click a CYLINDRICAL face",
    /// that is, the tool blamed a person for doing the right thing.
    #[test]
    fn the_thread_tool_asks_for_the_live_brep() {
        let mut app = App::default();
        add_part_at(&mut app, 0.0);
        app.rebuild_if_dirty();
        app.live.shapes.clear();
        app.live.ready = false;
        app.live.tried_rev = None;
        app.cmd.kind = 24; // THE THREAD
        app.refresh_edges();
        assert!(
            app.live.ready || app.regen.wanted || !app.live.shapes.is_empty(),
            "the thread tool must ask for the live B-rep: without it, it answers a correct click with a miss"
        );
    }

    /// THE GIZMO OF A JOINT TURNS THE SAME WAY THE GIZMO OF A BODY DOES.
    ///
    /// Reported behaviour: drag the ring clockwise and the part goes counter-clockwise, while the ordinary
    /// gizmo of a body turns correctly. The convention is documented and covered by the test
    /// `ccw_toward_viewer_is_positive`: an axis TOWARDS THE VIEWER plus a visual drag COUNTER-clockwise
    /// gives a POSITIVE angle (the right hand). That test checks the helper `ring_drag_sign` in isolation
    /// and could not catch this defect: the gizmo of a joint computed the sign by A FORMULA OF ITS OWN,
    /// exactly the opposite one.
    #[test]
    fn the_joint_ring_turns_the_same_way_as_the_body_ring() {
        use qymcad_core::feature::JointKind;
        let mut app = App::default();
        add_part_at(&mut app, 0.0);
        add_part_at(&mut app, 200.0);
        let root = app.project.root;
        app.enter_component(root);
        app.rebuild_if_dirty();
        let bodies: Vec<_> = (0..app.project.bodies.len()).filter_map(|mi| app.project.mesh_id(mi)).collect();
        let (owner_a, owner_b) = (
            app.project.body_owner(bodies[0]).expect("the owner of A"),
            app.project.body_owner(bodies[1]).expect("the owner of B"),
        );
        app.project.set_grounded(owner_a, true);
        let ca = app.project.add_connector(owner_a, qymcad_core::feature::AnchorRef::Origin);
        let cb = app.project.add_connector(owner_b, qymcad_core::feature::AnchorRef::Origin);
        // EVERY kind with rotation: if one gets confused they all do, because the drag is shared
        for kind in [JointKind::Revolute, JointKind::Cylindrical, JointKind::Ball, JointKind::PinSlot] {
            check_ring_sign(&mut app, ca, cb, kind);
        }
    }

    /// One kind of joint: the ring of the gizmo must give a positive angle on a counter-clockwise drag
    /// around an axis pointing towards the viewer.
    fn check_ring_sign(app: &mut App, ca: qymcad_core::model::Id, cb: qymcad_core::model::Id, kind: qymcad_core::feature::JointKind) {
        let jid = app.project.add_joint(ca, cb, kind);
        app.project.solve_joints();

        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
        let basis = app.cam.basis();
        // AN AXIS TOWARDS THE VIEWER: its projection onto the into-the-screen direction is negative
        let axis = {
            let d = basis.2;
            [-d[0], -d[1], -d[2]]
        };
        assert!(
            axis[0] * basis.2[0] + axis[1] * basis.2[1] + axis[2] * basis.2[2] < 0.0,
            "setup: the axis must look AT the viewer"
        );
        app.joint.giz_drag = Some(super::super::JointGizDrag { jid, slot: 0, ring: true, start: 0.0, amt: 0.0, o: [0.0; 3], dir: axis });

        // A COUNTER-CLOCKWISE DRAG on screen: the cursor at twelve o'clock relative to the centre, moving
        // left. On screen y grows DOWNWARDS, so twelve o'clock is minus y.
        let center = app.project3([0.0; 3], rect, &basis).0;
        let cursor = center + egui::vec2(0.0, -80.0);
        app.joint_giz_drag_to(cursor, egui::vec2(-6.0, 0.0), rect, &basis);

        let amt = app.joint.giz_drag.expect("the drag is alive").amt;
        assert!(
            amt > 0.0,
            "{kind:?}: an axis towards the viewer and a COUNTER-clockwise drag must give a POSITIVE angle \
             (the right hand, the same convention as the gizmo of a body), and out came {amt:.3} — the joint \
             turns the part against the mouse"
        );
        app.joint.giz_drag = None;
        let last = app.project.joints.len() - 1;
        app.project.joints.remove(last); // the next kind is checked on a clean joint
    }

    /// ALL SEVEN KINDS ARE CREATED through one button and a choice of kind in the bar.
    ///
    /// There used to be seven buttons, one per kind, and the kind was changed by a combo box in the same
    /// bar anyway. Before reducing them to one button the main thing was checked: the CREATION combo did
    /// not have the rigid kind — it was created only by its own button. Remove the buttons without filling
    /// in the combo and that kind would become unreachable.
    #[test]
    fn every_joint_kind_is_creatable_through_the_bar() {
        use qymcad_core::feature::JointKind;
        let kinds = [
            JointKind::Rigid,
            JointKind::Revolute,
            JointKind::Slider,
            JointKind::Cylindrical,
            JointKind::Planar,
            JointKind::Ball,
            JointKind::PinSlot,
        ];
        // ALL seven are offered in the creation bar
        let src = include_str!("joints.rs");
        let a = src.find("joint_bar_kind").expect("the kind combo of the creation bar is in place");
        let b = src[a..].find("});").map(|i| a + i).unwrap_or(src.len());
        for k in kinds {
            assert!(
                src[a..b].contains(&format!("JointKind::{k:?}")),
                "{k:?} is not offered in the creation bar — and there are no per-kind buttons any more, so the kind is unreachable"
            );
        }

        // and each one IS REALLY created with the kind chosen in the bar
        for kind in kinds {
            let mut app = App::default();
            add_part_at(&mut app, 0.0);
            add_part_at(&mut app, 200.0);
            let root = app.project.root;
            app.enter_component(root);
            app.rebuild_if_dirty();
            let bodies: Vec<_> = (0..app.project.bodies.len()).filter_map(|mi| app.project.mesh_id(mi)).collect();
            let (ba, bb) = (bodies[0], bodies[1]);
            let edge_of = |app: &App, b: qymcad_core::model::Id| -> u32 {
                app.body_edges_cached(b).and_then(|e| e.1.iter().copied().find(|&id| id != 0)).expect("there are edges")
            };
            let (ea, eb) = (edge_of(&app, ba), edge_of(&app, bb));
            app.project.set_grounded(app.project.body_owner(ba).expect("the owner"), true);

            app.joint.pick_faces = true; // the Mate button
            app.joint.new_kind = kind; // the choice of kind in the bar
            app.joint.anchor_mode = 1;
            app.joint_pick_edge_click(ba, ea);
            app.joint_pick_edge_click(bb, eb);
            assert_eq!(app.project.joints.len(), 1, "{kind:?}: the joint was not created; the status line: {}", app.status);
            assert_eq!(app.project.joints[0].kind, kind, "{kind:?}: a joint of another kind was created");
        }
    }

    /// THE SLIDE ARROW OF A PIN-SLOT POINTS WHERE THE PART WILL ACTUALLY GO.
    ///
    /// The direction of the slot belongs to THE SECOND anchor (by the usual convention: the first is the
    /// pin and the point of rotation, the second is the translation). The frame of the gizmo, however,
    /// was ALWAYS built from the first anchor: the arrow pointed one way and the part went another. The
    /// handle is dragged sideways and the gantry travels forwards, with nothing to explain it.
    #[test]
    fn the_slide_arrow_of_a_pin_slot_points_where_the_part_will_go() {
        use qymcad_core::feature::{AnchorRef, JointKind};

        let mut app = App::default();
        add_part_at(&mut app, 0.0);
        add_part_at(&mut app, 60.0);
        for _ in 0..4 {
            if app.current_ctx_id_for_test() == app.project.root {
                break;
            }
            app.exit_context_for_test();
        }
        app.rebuild_if_dirty_for_test();
        let a = app.project.mesh_id(0).and_then(|b| app.project.body_owner(b)).expect("part A");
        let b = app.project.mesh_id(1).and_then(|b| app.project.body_owner(b)).expect("part B");
        app.project.set_grounded(a, true);
        let ca = app.project.add_connector(a, AnchorRef::Origin);
        let cb = app.project.add_connector(b, AnchorRef::Origin);
        // THE SLOT LOOKS ITS OWN WAY: its secondary axis is turned by a quarter, so its X is world Y.
        app.project.connectors.iter_mut().find(|x| x.id == cb).expect("the anchor of the slot").rot_deg = 90.0;
        let j = app.project.add_joint(ca, cb, JointKind::PinSlot);

        // Where does the slide arrow POINT?
        let (_, handles) = app.joint_giz_handles_for_test(j).expect("the handles of the gizmo are there");
        let (_, _, dir) = *handles.iter().find(|(slot, ring, _)| *slot == 1 && !*ring).expect("a pin-slot has a slide handle");

        // Where does the part REALLY GO?
        let before = app.project.world_transform(b);
        app.project.joints.iter_mut().find(|x| x.id == j).unwrap().drive[1] = Some(10.0);
        app.project.solve_joints();
        let after = app.project.world_transform(b);
        let went = [after[3] - before[3], after[7] - before[7], after[11] - before[11]];
        let len = (went[0] * went[0] + went[1] * went[1] + went[2] * went[2]).sqrt();
        assert!(len > 1e-6, "the part did not move at all — there is no direction to check");
        let unit = [went[0] / len, went[1] / len, went[2] / len];

        let dot = dir[0] * unit[0] + dir[1] * unit[1] + dir[2] * unit[2];
        assert!(
            dot > 0.999,
            "the slide arrow points at {dir:?} and the part went to {unit:?} — the handle is dragged one way and the part goes another"
        );
    }

    /// THE GEOMETRY IS UP BEFORE A FACE IS PICKED.
    ///
    /// A document from a bundle carries no live B-rep: it is assembled ON DEMAND. The demand is stated by
    /// `needs_live_brep`, and joints on edges and vertices are accounted for there — while COLLECTING a
    /// joint BY FACE was not: the condition fired only for the edge and vertex modes. And the default
    /// anchor mode of a slider is precisely THE FACE.
    ///
    /// What that ends in: the first joint of a document is placed while `regen_faces` is still empty, the
    /// principal direction of the face is absent, the axis of travel is taken from the world axes — and
    /// the part goes the wrong way. The geometry comes up afterwards (the joint already placed demands
    /// it), but the part stays where it stood: a minimal displacement does not move it for nothing. One
    /// wrong second and the assembly is crooked for ever.
    #[test]
    fn the_geometry_is_up_before_the_first_face_anchor_is_picked() {
        use qymcad_core::feature::JointKind;

        let mut app = App::default();
        add_part_at(&mut app, 0.0);
        add_part_at(&mut app, 60.0);
        for _ in 0..4 {
            if app.current_ctx_id_for_test() == app.project.root {
                break;
            }
            app.exit_context_for_test();
        }
        app.rebuild_if_dirty_for_test();
        let body = app.project.mesh_id(0).expect("body A");

        // A DOCUMENT JUST OPENED FROM A BUNDLE: there are meshes, no live B-rep and no faces.
        app.live.shapes.clear();
        app.project.regen_faces.clear();
        app.live.ready = false;
        app.live.tried_rev = None;
        assert!(app.project.regen_faces.is_empty(), "setup: there must be no faces");

        // A SLIDER is started — the default anchor mode is the face.
        app.joint.new_kind = JointKind::Slider;
        app.start_joint_pick_for_test();
        assert_eq!(app.joint.anchor_mode, 0, "setup: the default anchor of a slider is the face");

        // A frame is drawn — and by the time the mouse is brought over, the geometry must be there.
        app.refresh_edges();
        app.drain_bg_for_test();
        app.rebuild_if_dirty_for_test();

        assert!(
            app.project.regen_faces.get(&body).is_some_and(|f| !f.is_empty()),
            "a face is being picked and the body has no faces: the joint will stand by the world axes and the part will go the wrong way"
        );
    }

    /// WHILE THE GEOMETRY IS ON ITS WAY, NO JOINT IS PLACED.
    ///
    /// In a live window the preparation of the B-rep goes into a BACKGROUND thread, while picking faces
    /// works off the mesh and is available at once. On a real assembly the preparation takes seconds:
    /// clicking both faces before the geometry arrives is a matter of one second. And a joint placed
    /// without geometry takes its axis of travel from the world axes, the part goes the wrong way and
    /// STAYS there: a minimal displacement does not move it for nothing, however much is computed
    /// afterwards.
    ///
    /// The rule is the same as for any other preparation: the tool says it is preparing the geometry and
    /// takes no picks until it is ready.
    #[test]
    fn while_the_geometry_is_on_its_way_the_tool_does_not_take_picks() {
        use qymcad_core::feature::{FaceKey, JointKind};

        let mut app = App::default();
        add_part_at(&mut app, 0.0);
        add_part_at(&mut app, 60.0);
        for _ in 0..4 {
            if app.current_ctx_id_for_test() == app.project.root {
                break;
            }
            app.exit_context_for_test();
        }
        app.rebuild_if_dirty_for_test();
        let body = app.project.mesh_id(0).expect("body A");
        let key = app
            .project
            .regen_faces
            .get(&body)
            .and_then(|fs| fs.iter().find(|f| f.normal[2] > 0.99))
            .map(|f| FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id })
            .expect("the top face of part A");

        // A LIVE WINDOW plus a document just opened from a bundle: there are meshes and no live B-rep.
        app.regen.ui_running = true;
        app.live.shapes.clear();
        app.project.regen_faces.clear();
        app.live.ready = false;
        app.live.tried_rev = None;

        app.joint.new_kind = JointKind::Slider;
        app.start_joint_pick_for_test();
        app.refresh_edges(); // a frame: the tool asked for the geometry and it went into the background
        assert!(app.project.regen_faces.is_empty(), "setup: the geometry must still be on its way");

        // A face is clicked without waiting for anything.
        app.joint_pick_face_click_for_test(body, key);
        assert!(
            app.joint_pick_first_anchor_for_test().is_none(),
            "the tool took an anchor while there is no geometry: the joint will stand by the world axes and the part will go the wrong way"
        );
        assert!(app.joint_pick_active_for_test(), "the tool was dropped instead of waiting for the geometry");
        assert_eq!(app.status, crate::i18n::tr("j-geometry-on-its-way"), "nobody was told WHAT to wait for: {}", app.status);

        // "AND THAT THE ANCHOR IS TAKEN AFTERWARDS" IS NOT CHECKED HERE. A deferred preparation is
        // concluded by the result arriving from the background thread (`brep_wait` is cleared by
        // `settle_brep_wait` in `tick_async`), and this check has no such frame loop. That the anchor is
        // taken once the geometry is in place is shown by the neighbouring checks of collecting a joint —
        // there are dozens of them.
    }

    /// WHEN THERE IS NOTHING TO WAIT FOR, SOMETHING ELSE IS SAID. "One moment" after the preparation has
    /// finished and the body is still absent is a lie: the face will be clicked until it is given up.
    #[test]
    fn when_the_body_never_built_the_tool_says_so_instead_of_promising_a_moment() {
        use qymcad_core::feature::{FaceKey, JointKind};

        let mut app = App::default();
        add_part_at(&mut app, 0.0);
        add_part_at(&mut app, 60.0);
        for _ in 0..4 {
            if app.current_ctx_id_for_test() == app.project.root {
                break;
            }
            app.exit_context_for_test();
        }
        app.rebuild_if_dirty_for_test();
        let body = app.project.mesh_id(0).expect("body A");
        let key = app
            .project
            .regen_faces
            .get(&body)
            .and_then(|fs| fs.iter().find(|f| f.normal[2] > 0.99))
            .map(|f| FaceKey { index: 0, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id })
            .expect("the top face of part A");

        // The preparation IS OVER and there is no live body — that is what an import that did not recover
        // looks like.
        app.live.shapes.remove(&body);
        app.live.ready = true;
        app.joint.new_kind = JointKind::Slider;

        // A FACE IS TAKEN EVEN WITHOUT A LIVE BODY — it has everything of its own: the centre, the normal,
        // the principal direction.
        //
        // The opposite used to be demanded here, and it locked people out completely: right after a
        // document is opened not one body has a live B-rep (a measurement on a real machine: 0 out of 138)
        // while every one of them has faces. The report: faces can only be chosen through the origin
        // button and in no other way.
        app.start_joint_pick_for_test();
        app.joint_pick_face_click_for_test(body, key);
        assert!(
            app.joint_pick_first_anchor_for_test().is_some(),
            "an anchor on a face was not taken although the faces are computed: {}",
            app.status
        );

        // AN EDGE, THOUGH, REALLY HAS NOTHING TO WAIT FOR: the reference direction is read from the edges
        // of the model, there are none and there will be none — no live body is left. Here "one moment"
        // would be a lie.
        app.on_escape();
        app.project.regen_edges.remove(&body);
        app.start_joint_pick_for_test();
        app.joint_pick_edge_click_for_test(body, 1);
        assert!(app.joint_pick_first_anchor_for_test().is_none(), "an edge anchor was taken where there are no edges at all");
        assert_eq!(app.status, crate::i18n::tr("j-geometry-missing"), "one moment is promised where there is nothing to wait for: {}", app.status);
    }

    /// THE SECOND PICK AS A PERSON MAKES IT: press "point at the axis", click an edge, and the slider
    /// travels along it.
    ///
    /// The model and the click handler are covered by the kernel, and the path a person walks — a button
    /// in the frame plus a click on geometry — was not. It is at exactly this joint that things have
    /// broken before: "wired in one place means wired everywhere" has proved untrue twice.
    #[test]
    fn pointing_at_an_edge_sets_the_anchor_axis_from_the_frame() {
        use qymcad_core::feature::{AnchorRef, JointKind};

        let mut app = App::default();
        add_part_at(&mut app, 0.0);
        add_part_at(&mut app, 60.0);
        for _ in 0..4 {
            if app.current_ctx_id_for_test() == app.project.root {
                break;
            }
            app.exit_context_for_test();
        }
        app.rebuild_if_dirty_for_test();
        app.mode_3d = true;
        app.cam.init = true;
        app.cam.scale = 6.0;
        app.cam.target = [30.0, 10.0, 5.0];
        let (ba, bb) = (app.project.mesh_id(0).expect("body A"), app.project.mesh_id(1).expect("body B"));
        let (a, b) = (app.project.body_owner(ba).expect("part A"), app.project.body_owner(bb).expect("part B"));
        app.project.set_grounded(a, true);
        let ca = app.project.add_connector(a, AnchorRef::Origin);
        let cb = app.project.add_connector(b, AnchorRef::Origin);
        let jid = app.project.add_joint(ca, cb, JointKind::Slider);
        app.joint.edit = Some(jid);
        app.workbench = super::super::Workbench::Assembly;

        // "POINT AT THE AXIS" IS PRESSED — the coordinate of the button is taken FROM THE FRAME.
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0));
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        let mut at = None;
        for _ in 0..2 {
            let out = ctx.run_ui(egui::RawInput { screen_rect: Some(rect), ..Default::default() }, |c| {
                app.joint_popup_for_test(c, rect);
            });
            at = None;
            for cs in &out.shapes {
                super::super::screen_keys::tests::text_pos(&cs.shape, &crate::i18n::tr("j-axis-pick"), &mut at);
            }
        }
        let at = at.expect("the point-at-the-axis button is in the frame");
        let btn = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        for events in [vec![egui::Event::PointerMoved(at)], vec![egui::Event::PointerMoved(at), btn(true)], vec![btn(false)]] {
            let _ = ctx.run_ui(egui::RawInput { screen_rect: Some(rect), events, ..Default::default() }, |c| {
                app.joint_popup_for_test(c, rect);
            });
        }
        assert_eq!(app.joint.axis_pick, Some(ca), "the point-at-the-axis button did not take the tool: {:?}", app.joint.axis_pick);

        // AN EDGE IS CLICKED — and the axis of the anchor runs along it.
        app.refresh_edges();
        app.ensure_brep_for_test();
        let e = app.project.regen_edges.get(&ba).and_then(|es| es.first().cloned()).expect("an edge of part A");
        app.joint_axis_pick_apply_for_test(AnchorRef::EdgeMid(ba, e.id));

        let got = app.project.connector(ca).and_then(|c| c.axis_ref.clone());
        assert!(matches!(got, Some(AnchorRef::EdgeMid(_, id)) if id == e.id), "the axis of the anchor was not taken from the edge that was pointed at: {got:?}");
        assert!(app.joint.axis_pick.is_none(), "the point-at-the-axis tool did not release after the pick");

        // AND IT SHOWS IN THE MOVEMENT: the slider runs along the edge rather than along a guess.
        let axis = app.project.joint_slot_axis(jid, 1, app.current_ctx_id_for_test()).expect("the axis of travel");
        let want = e.dir;
        let dot = (axis[0] * want[0] + axis[1] * want[1] + axis[2] * want[2]).abs();
        assert!(dot > 0.999, "the axis of travel {axis:?} did not match the edge that was pointed at {want:?}");
    }
}
