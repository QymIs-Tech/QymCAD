//! A FILLET WITH A RADIUS OF ITS OWN AT A CORNER REOPENS WITH THAT CORNER, on a document just opened.
//!
//! The radius at a corner is a field keyed by the name of the vertex, and the name is found on the edges of the body.
//! A document just opened holds no edges of a body until it is rebuilt, so a fillet of edges picked one by one
//! reopened without its corner field, and Enter wrote the empty table back: the corner took the common radius again.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::App;
    use qymcad_core::feature::FeatureKind;
    use qymcad_core::model::Id;

    /// The fillet of the document.
    struct Made {
        node: Id,
        src: Id,
        corners: Vec<f64>,
    }

    fn the_fillet(app: &App) -> Made {
        app.project
            .timeline
            .iter()
            .find_map(|n| match &n.kind {
                FeatureKind::Fillet { src, at_vertices, .. } => Some(Made { node: n.id, src: *src, corners: at_vertices.iter().map(|v| v.1).collect() }),
                _ => None,
            })
            .expect("a fillet in the timeline")
    }

    /// The radius in the corner field of the open command, if it has one.
    fn corner_field(app: &App) -> Option<f64> {
        app.tools.cmd.params.iter().find(|p| p.key.starts_with("at")).map(|p| p.val)
    }

    /// A plate 60 x 40 x 12 drawn and extruded by hand; its four top edges picked one by one with Fillet, R1, and R2 at
    /// the corner (0, 0, 12); Enter.
    fn a_plate_with_a_variable_fillet(hand: &mut Hand) {
        assert!(hand.press_word(&qymcad_i18n::tr("plane-xy-table"), egui::pos2(0.0, 0.0)), "setup: a sketch is started on XY");
        hand.sk_tool(qymcad_ui_state::SketchShape::Rect).click2d(0.0, 0.0).click2d(60.0, 40.0);
        assert!(hand.press_word(&qymcad_i18n::tr("wb-finish"), egui::pos2(0.0, 0.0)), "setup: the sketch is finished");
        hand.close_window();
        hand.look_at([30.0, 20.0, 6.0], 8.0).tool(qymcad_ui_state::PartTool::Extrude).set("height", 12.0).enter();
        hand.tool(qymcad_ui_state::PartTool::Fillet).set("radius", 1.0);
        for mid in [[30.0, 0.0, 12.0], [60.0, 20.0, 12.0], [30.0, 40.0, 12.0], [0.0, 20.0, 12.0]] {
            hand.click(mid);
        }
        assert_eq!(hand.app.tools.gsel.edges.len(), 4, "setup: the four top edges are picked; the program says {:?}", hand.app.status);
        hand.click([0.0, 0.0, 12.0]);
        let key = hand.app.tools.cmd.params.iter().find(|p| p.key.starts_with("at")).map(|p| p.key.to_string()).expect("setup: a click on the corner gives it a field");
        hand.set(&key, 2.0).enter();
        let made = the_fillet(hand.app);
        assert!(!hand.app.project.regen_errors.contains_key(&made.node), "setup: the fillet builds: {:?}", hand.app.project.regen_errors.get(&made.node));
        assert_eq!(made.corners, vec![2.0], "setup: the fillet holds R2 at its corner");
    }

    #[test]
    fn a_variable_fillet_reopens_with_its_corner_after_opening_the_file() {
        let path = std::env::temp_dir().join(format!("qym-variable-fillet-reopen-{}.qcad", std::process::id())).to_string_lossy().into_owned();
        {
            let mut maker = App::default();
            a_plate_with_a_variable_fillet(&mut Hand::new(&mut maker));
            crate::gui::set_project_path(&mut maker.disk.project_path, &mut maker.set, path.clone());
            maker.save_project();
            maker.wait_bg();
        }
        let mut app = App::default();
        crate::gui::io_jobs::spawn_project_load(&mut app.regen, path.clone());
        app.drain_busy_for_test();
        let _ = std::fs::remove_file(&path);
        let made = the_fillet(&app);
        assert!(!app.project.regen_edges.contains_key(&made.src), "GUARD: a file just opened holds no edges of the plate, otherwise there is no trap");
        let part = app.project.body_owner(made.src).expect("the part of the plate");
        let mut hand = Hand::new(&mut app);
        let name = hand.app.project.components.iter().find(|c| c.id == part).map(|c| crate::i18n::name(&c.name)).expect("the name of the part");
        assert!(hand.double_click_word(&name), "the tree shows the part as {name:?}");
        hand.look_at([30.0, 20.0, 6.0], 8.0);
        let ti = hand.app.project.timeline.iter().position(|n| n.id == made.node).expect("the node in the timeline");
        let label = crate::gui::panels_tree::feature_row_label(&hand.app.project, ti);
        assert!(hand.double_click_word(&label), "the tree shows the fillet as {label:?}");
        assert_eq!(hand.app.tools.cmd.edit, Some(made.node), "a double click on the row reopens the fillet");
        assert_eq!(corner_field(hand.app), Some(2.0), "the reopened fillet shows the field of its corner, R2");
        let said = hand.trial_says();
        assert!(said.is_none(), "the reopened fillet must build as it stands, and the trial says {said:?}");
        hand.set("radius", 1.5).enter();
        assert_eq!(the_fillet(hand.app).corners, vec![2.0], "Enter on the reopened fillet keeps R2 at its corner");
    }
}
