//! MEASURING IN 3D — the whole path a person takes.
//!
//! The arithmetic is checked in the kernel on known geometry; this is the wiring: the button,
//! switching it on, a click on real geometry, resolving the hit into the right element, the number in
//! one place for both the status line and the plate, and leaving by Esc.
#[cfg(test)]
mod tests {
    use super::super::App;
    use qymcad_core::measure::MeasureItem;

    /// A cube in a part plus a camera from which it can be hit by a click.
    fn cube_in_view(app: &mut App) -> (usize, u64) {
        super::super::joint_flow::tests::add_part_at(app, 0.0);
        app.rebuild_if_dirty();
        let body = app.project.mesh_id(0).expect("the body");
        if let Some(owner) = app.project.body_owner(body) {
            app.enter_component(owner);
        }
        let mi = app.project.mesh_index(body).expect("the mesh");
        app.mode_3d = true;
        app.cam.init = true;
        app.cam.scale = 8.0;
        app.cam.target = [10.0, 10.0, 5.0];
        (mi, body)
    }

    fn rect() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0))
    }

    /// The button exists, and it switches the measuring tool on.
    #[test]
    fn the_tool_has_a_button_and_turns_on() {
        assert!(crate::gui::panels_source::PANELS.contains("self.toggle_measure_3d();"), "without a button the tool does not exist for a person");
        let mut app = App::default();
        cube_in_view(&mut app);
        app.toggle_measure_3d();
        assert!(app.m3.on, "the measuring tool must switch on");
        assert!(app.mode_3d, "measuring happens in 3D");
        app.toggle_measure_3d();
        assert!(!app.m3.on, "a second press switches it off");
    }

    /// THE WHOLE PATH: a click on a face, a click on the opposite face, and the height of the cube.
    #[test]
    fn clicking_two_opposite_faces_measures_the_thickness() {
        let mut app = App::default();
        let (mi, _body) = cube_in_view(&mut app);
        let bb = app.project.bodies[mi].mesh.bounds().expect("the bounding box");
        let h = bb.max.z - bb.min.z;
        app.toggle_measure_3d();

        // the top face: the aim is the centre of its area
        let basis = app.cam.basis();
        let top = [(bb.min.x + bb.max.x) * 0.5, (bb.min.y + bb.max.y) * 0.5, bb.max.z];
        let at = app.project3(top, rect(), &basis).0;
        app.measure_3d_click(rect(), at);
        assert_eq!(app.m3.picks.len(), 1, "the first click must land; status: {}", app.status);

        // the bottom face — aimed at from below, with the camera turned round
        app.cam.yaw += std::f64::consts::PI;
        app.cam.pitch = -app.cam.pitch;
        let basis = app.cam.basis();
        let bot = [(bb.min.x + bb.max.x) * 0.5, (bb.min.y + bb.max.y) * 0.5, bb.min.z];
        let at = app.project3(bot, rect(), &basis).0;
        app.measure_3d_click(rect(), at);
        assert_eq!(app.m3.picks.len(), 2, "the second click must land; status: {}", app.status);

        let r = app.measure_result().expect("there is a result");
        let d = r.distance.expect("there must be a distance between parallel faces");
        assert!((d - h).abs() < 1e-6, "the thickness of the cube is {h}, and {d} was measured");
        assert!(app.status.contains(&crate::i18n::tr1("m3-distance", "v", "10.000")), "the number must be IN THE STATUS LINE rather than only inside: {}", app.status);
    }

    /// A THIRD CLICK starts a new measurement rather than accumulating a third element.
    #[test]
    fn a_third_click_starts_a_new_measurement() {
        let mut app = App::default();
        app.toggle_measure_3d();
        app.m3.picks = vec![
            super::super::measure3d::MeasurePick { item: MeasureItem::Point([0.0; 3]), what: "vertex".into(), at: [0.0; 3] },
            super::super::measure3d::MeasurePick { item: MeasureItem::Point([1.0, 0.0, 0.0]), what: "vertex".into(), at: [1.0, 0.0, 0.0] },
        ];
        let (mi, _b) = cube_in_view(&mut app);
        let bb = app.project.bodies[mi].mesh.bounds().expect("the bounding box");
        let basis = app.cam.basis();
        let top = [(bb.min.x + bb.max.x) * 0.5, (bb.min.y + bb.max.y) * 0.5, bb.max.z];
        app.measure_3d_click(rect(), app.project3(top, rect(), &basis).0);
        assert_eq!(app.m3.picks.len(), 1, "a third click must start a new measurement rather than become a third element");
    }

    /// A miss says so rather than staying silent.
    #[test]
    fn a_miss_says_so() {
        let mut app = App::default();
        cube_in_view(&mut app);
        app.toggle_measure_3d();
        app.measure_3d_click(rect(), egui::pos2(895.0, 695.0)); // a corner of the screen — deliberately a miss
        assert!(app.m3.picks.is_empty(), "a miss collects nothing");
        assert_eq!(app.status, crate::i18n::tr("m3-miss"), "the reason must be said");
    }

    /// ONE ELEMENT already gives a number: the length of an edge or the diameter of a hole.
    #[test]
    fn one_element_already_reports_its_own_size() {
        let mut app = App::default();
        app.toggle_measure_3d();
        app.m3.picks = vec![super::super::measure3d::MeasurePick {
            item: MeasureItem::Line { origin: [0.0; 3], dir: [1.0, 0.0, 0.0], len: 42.0 },
            what: "edge".into(),
            at: [0.0; 3],
        }];
        let t = app.measure_text();
        assert!(t.contains("42.000"), "the length of a single edge must be shown at once: {t}");
    }

    /// NON-PARALLEL FACES: an angle and an honest explanation instead of an invented distance.
    #[test]
    fn converging_faces_report_the_angle_not_a_made_up_distance() {
        let mut app = App::default();
        app.toggle_measure_3d();
        app.m3.picks = vec![
            super::super::measure3d::MeasurePick { item: MeasureItem::Plane { origin: [0.0; 3], normal: [0.0, 0.0, 1.0] }, what: "face".into(), at: [0.0; 3] },
            super::super::measure3d::MeasurePick { item: MeasureItem::Plane { origin: [0.0; 3], normal: [0.0, 1.0, 1.0] }, what: "face".into(), at: [0.0; 3] },
        ];
        let t = app.measure_text();
        assert!(t.contains(&crate::i18n::tr1("m3-angle", "v", "45.000")), "the angle must be shown: {t}");
        // THE LABEL IS TAKEN FROM THE CATALOGUE rather than typed as a word: a literal would tie the
        // check to one language and go blind the moment the catalogue is edited. The label is separated
        // from the place of the value.
        let marker = crate::i18n::tr1("m3-distance", "v", "\u{1}");
        let label = marker.split('\u{1}').next().unwrap_or("");
        assert!(!label.is_empty(), "the distance label is empty — the check below would mean nothing");
        assert!(!t.contains(label), "converging faces must have NO distance: {t}");
    }

    /// Esc first clears what was clicked and only then leaves — measuring is usually done several
    /// times over.
    #[test]
    fn escape_clears_the_picks_before_leaving_the_tool() {
        let mut app = App::default();
        app.toggle_measure_3d();
        app.m3.picks = vec![super::super::measure3d::MeasurePick { item: MeasureItem::Point([0.0; 3]), what: "vertex".into(), at: [0.0; 3] }];
        app.on_escape();
        assert!(app.m3.on, "the first Esc must clear what was clicked rather than throw one out of the tool");
        assert!(app.m3.picks.is_empty(), "what was clicked is cleared");
        app.on_escape();
        assert!(!app.m3.on, "the second Esc leaves the tool");
    }

    /// The result is VISIBLE at the geometry rather than only in the status line.
    #[test]
    fn the_result_is_drawn_at_the_geometry() {
        let src = crate::gui::render_source::RENDER;
        assert!(src.contains("pub(super) fn draw_measure_3d"), "the measuring tool must have a drawing layer of its own");
        assert!(src.contains("self.draw_measure_3d(painter, rect);"), "the layer must be called from the frame");
        let a = src.find("pub(super) fn draw_measure_3d").expect("the block");
        let b = src[a..].find("\n    /// THE DRIVEN GEOMETRY").map(|i| a + i).unwrap_or(src.len());
        assert!(src[a..b].contains("self.measure_text()"), "the plate must take the number FROM THE SAME source as the status line");
    }

    /// Measuring DOES NOT DISTURB THE SELECTION: measure a gap and the part stays selected.
    #[test]
    fn measuring_does_not_disturb_the_selection() {
        let mut app = App::default();
        let (mi, _body) = cube_in_view(&mut app);
        app.sel = super::super::Sel::Mesh(mi);
        app.toggle_measure_3d();
        app.sel = super::super::Sel::Mesh(mi); // the tool is on and the selection was restored by the person
        let bb = app.project.bodies[mi].mesh.bounds().expect("the bounding box");
        let basis = app.cam.basis();
        let top = [(bb.min.x + bb.max.x) * 0.5, (bb.min.y + bb.max.y) * 0.5, bb.max.z];
        app.measure_3d_click(rect(), app.project3(top, rect(), &basis).0);
        assert!(matches!(app.sel, super::super::Sel::Mesh(_)), "a click of the measuring tool must not overwrite the selection");
    }
    /// WHAT IS OCCLUDED IS NOT PICKED — a regression found by this very test.
    ///
    /// In an isometric view the far bottom corner of a part lands exactly on the middle of its top
    /// face. Picking an edge did not look at depth, and a click on a VISIBLE face returned an edge from
    /// THE FAR SIDE: instead of the thickness of the part a diagonal came out. The numbers were
    /// "correct" all the while — for the wrong geometry.
    #[test]
    fn a_click_on_a_visible_face_never_returns_an_edge_behind_it() {
        let mut app = App::default();
        let (mi, _body) = cube_in_view(&mut app);
        let bb = app.project.bodies[mi].mesh.bounds().expect("the bounding box");
        app.toggle_measure_3d();
        let basis = app.cam.basis();
        let top = [(bb.min.x + bb.max.x) * 0.5, (bb.min.y + bb.max.y) * 0.5, bb.max.z];
        let at = app.project3(top, rect(), &basis).0;

        // at this point there IS a nearby edge — but it is on the far side of the part
        let got = app.measure_resolve(rect(), at).expect("there is a hit");
        assert!(
            matches!(got.item, MeasureItem::Plane { .. }),
            "a click on a visible face must give a FACE, and it gave {:?} ({})",
            got.item,
            got.what
        );
    }

}
