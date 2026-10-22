//! A FILLET, A CHAMFER OR A PATCH PICKED BY A FACE REOPENS WITH THAT FACE.
//!
//! A click on a face in the fillet, the chamfer or the patch is recorded as "every edge of this face"
//! (`Adjacent(Id(face))`), not as a list of edges. Reported behaviour: a double click on such a fillet in the tree
//! highlighted no edge and previewed both rims of a cylinder, and Enter wrote the edges back as a list; right after the
//! file was opened the same double click showed "Not one of the 1 named edges is left in the body" instead. The patch
//! reopened through the same reading of the face number as an edge.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::App;
    use qymcad_core::feature::FeatureKind;
    use qymcad_core::model::Id;
    use qymcad_core::refs::Query;

    /// The tool a face of the cylinder is clicked with.
    #[derive(Clone, Copy, Debug)]
    enum Tool {
        Fillet,
        Chamfer,
        Patch,
    }

    impl Tool {
        fn button(self) -> qymcad_ui_state::PartTool {
            match self {
                Tool::Fillet => qymcad_ui_state::PartTool::Fillet,
                Tool::Chamfer => qymcad_ui_state::PartTool::Chamfer,
                Tool::Patch => qymcad_ui_state::PartTool::Patch,
            }
        }

        /// The field of the size of a fillet or a chamfer: its key, and none for a patch.
        fn size_key(self) -> Option<&'static str> {
            match self {
                Tool::Fillet => Some("radius"),
                Tool::Chamfer => Some("dist"),
                Tool::Patch => None,
            }
        }
    }

    /// The node the tool laid.
    struct Made {
        node: Id,
        src: Id,
        body: Id,
        edges: qymcad_core::refs::Ref,
    }

    /// A cylinder R10 x 20 standing on XY, drawn and extruded by hand: a sketch started on XY, a circle clicked from
    /// its centre to its rim, the sketch finished, Extrude of 20.
    fn a_cylinder(hand: &mut Hand) {
        assert!(hand.press_word(&qymcad_i18n::tr("plane-xy-table"), egui::pos2(0.0, 0.0)), "setup: a sketch is started on XY");
        hand.sk_tool(qymcad_ui_state::SketchShape::Circle).click2d(0.0, 0.0).click2d(10.0, 0.0).key(egui::Key::Enter);
        assert!(hand.press_word(&qymcad_i18n::tr("wb-finish"), egui::pos2(0.0, 0.0)), "setup: the sketch is finished");
        hand.close_window();
        hand.look_at([0.0, 0.0, 10.0], 9.0).tool(qymcad_ui_state::PartTool::Extrude).set("height", 20.0).enter();
    }

    /// The node `tool` laid in the document.
    fn the_node(app: &App, tool: Tool) -> Made {
        app.project
            .timeline
            .iter()
            .find_map(|n| match (tool, &n.kind) {
                (Tool::Fillet, FeatureKind::Fillet { src, edges, .. }) | (Tool::Chamfer, FeatureKind::Chamfer { src, edges, .. }) => {
                    Some(Made { node: n.id, src: *src, body: n.id, edges: edges.clone() })
                }
                (Tool::Patch, FeatureKind::Patch { src, edges, body, .. }) => Some(Made { node: n.id, src: *src, body: *body, edges: edges.clone() }),
                _ => None,
            })
            .unwrap_or_else(|| panic!("the {tool:?} node in the timeline"))
    }

    /// The face the reference is phrased through.
    fn its_face(r: &qymcad_core::refs::Ref) -> u32 {
        match &r.query {
            Query::Adjacent(inner) => match **inner {
                Query::Id(f) => f,
                ref other => panic!("the description names one face, and it holds {other:?}"),
            },
            other => panic!("a click on a face is recorded as \"every edge of this face\", and it is {other:?}"),
        }
    }

    /// The edges of `face` on the live body `src`: what a click on that face picks.
    fn edges_of_face(app: &App, src: Id, face: u32) -> std::collections::BTreeSet<u32> {
        app.live.shapes.get(&src).map(|s| s.face_edge_ids(face)).unwrap_or_default().into_iter().collect()
    }

    /// That the node built on the top rim alone. A cylinder has 3 faces, and a fillet or a chamfer of one rim adds 1, of
    /// both rims 2.
    /// A patch over the top rim is a disc of pi 10^2 = 314.2 mm^2, 313.0 as the faceted face measures; over both rims
    /// it would be twice that.
    fn on_the_top_rim_alone(app: &App, tool: Tool, when: &str) {
        let made = the_node(app, tool);
        assert!(!app.project.regen_errors.contains_key(&made.node), "[{when}] the {tool:?} builds: {:?}", app.project.regen_errors.get(&made.node));
        match tool {
            Tool::Fillet | Tool::Chamfer => {
                let faces = app.project.regen_faces.get(&made.body).map(|f| f.len()).unwrap_or(0);
                assert_eq!(faces, 4, "[{when}] the {tool:?} takes the top rim alone: 3 faces of the cylinder and 1 of its own, and there are {faces}");
            }
            Tool::Patch => {
                let area: f64 = app.project.regen_faces.get(&made.body).map_or(0.0, |fs| fs.iter().map(|f| f.area).sum());
                let disc = std::f64::consts::PI * 100.0;
                assert!((area - disc).abs() < 5.0, "[{when}] the patch spans the top rim alone, a disc of {disc:.1} mm^2, and it is {area:.1}");
            }
        }
    }

    /// A cylinder, the tool, a click on its top face, Enter - by hand.
    fn a_top_face_pick(hand: &mut Hand, tool: Tool) {
        hand.look_at([0.0, 0.0, 10.0], 9.0).tool(tool.button()).click([0.0, 0.0, 20.0]);
        if let Some(key) = tool.size_key() {
            hand.set(key, 2.0);
        }
        hand.enter();
        let _ = its_face(&the_node(hand.app, tool).edges);
        on_the_top_rim_alone(hand.app, tool, "setup");
    }

    /// A double click on the node's row in the tree, as a person reopens it.
    fn reopen_from_the_tree(hand: &mut Hand, tool: Tool) {
        let made = the_node(hand.app, tool);
        let ti = hand.app.project.timeline.iter().position(|n| n.id == made.node).expect("the node in the timeline");
        let label = crate::gui::panels_tree::feature_row_label(&hand.app.project, ti);
        assert!(hand.double_click_word(&label), "the tree shows the {tool:?} as {label:?}");
        assert_eq!(hand.app.tools.cmd.edit, Some(made.node), "a double click on the row reopens the {tool:?}");
    }

    /// What is checked once the node is open again: the top rim alone is picked and highlighted, the description is
    /// back, and the trial does not refuse.
    fn reopened_with_its_face(hand: &mut Hand, tool: Tool, when: &str) {
        let made = the_node(hand.app, tool);
        let rim = edges_of_face(hand.app, made.src, its_face(&made.edges));
        assert!(!rim.is_empty(), "GUARD [{when}]: the top face of the live cylinder has edges");
        let picked: std::collections::BTreeSet<u32> = hand.app.tools.gsel.edges.iter().copied().collect();
        assert_eq!(picked, rim, "[{when}] the reopened {tool:?} highlights the top rim, the edges of its face");
        assert_eq!(hand.app.tools.gsel.described.as_ref(), Some(&made.edges.query), "[{when}] the reopened {tool:?} holds its description, not a list");
        let said = hand.trial_says();
        assert!(said.is_none(), "[{when}] the reopened {tool:?} must build as it stands, and the trial says {said:?}");
    }

    /// Enter on the reopened node, a fillet or a chamfer with a new size: "every edge of the top face" is kept and the
    /// top rim alone is taken.
    fn enter_keeps_the_face(hand: &mut Hand, tool: Tool, when: &str) {
        let before = the_node(hand.app, tool).edges;
        if let Some(key) = tool.size_key() {
            hand.set(key, 3.0);
        }
        hand.enter();
        let after = the_node(hand.app, tool).edges;
        assert_eq!(after.query, before.query, "[{when}] Enter on the reopened {tool:?} keeps \"every edge of this face\"");
        on_the_top_rim_alone(hand.app, tool, when);
    }

    fn reopens_in_the_session(tool: Tool) {
        let mut app = App::default();
        let mut hand = Hand::new(&mut app);
        a_cylinder(&mut hand);
        a_top_face_pick(&mut hand, tool);
        reopen_from_the_tree(&mut hand, tool);
        reopened_with_its_face(&mut hand, tool, "in the session");
        enter_keeps_the_face(&mut hand, tool, "in the session");
    }

    fn reopens_after_opening_the_file(tool: Tool) {
        let path = std::env::temp_dir().join(format!("qym-face-pick-reopen-{tool:?}-{}.qcad", std::process::id())).to_string_lossy().into_owned();
        {
            let mut maker = App::default();
            let mut hand = Hand::new(&mut maker);
            a_cylinder(&mut hand);
            a_top_face_pick(&mut hand, tool);
            crate::gui::set_project_path(&mut maker.disk.project_path, &mut maker.set, path.clone());
            maker.save_project();
            maker.wait_bg();
        }
        let mut app = App::default();
        crate::gui::io_jobs::spawn_project_load(&mut app.regen, path.clone());
        app.drain_busy_for_test();
        let _ = std::fs::remove_file(&path);
        let src = the_node(&app, tool).src;
        assert!(!app.project.regen_edges.contains_key(&src), "GUARD: a file just opened holds no edges of the cylinder, otherwise there is no trap");
        let part = app.project.body_owner(src).expect("the part of the cylinder");
        let mut hand = Hand::new(&mut app);
        let name = hand.app.project.components.iter().find(|c| c.id == part).map(|c| crate::i18n::name(&c.name)).expect("the name of the part");
        assert!(hand.double_click_word(&name), "the tree shows the part as {name:?}");
        hand.look_at([0.0, 0.0, 10.0], 9.0);
        reopen_from_the_tree(&mut hand, tool);
        reopened_with_its_face(&mut hand, tool, "after opening");
        enter_keeps_the_face(&mut hand, tool, "after opening");
    }

    #[test]
    fn a_face_fillet_reopens_with_its_face() {
        reopens_in_the_session(Tool::Fillet);
    }

    #[test]
    fn a_face_fillet_reopens_with_its_face_after_opening_the_file() {
        reopens_after_opening_the_file(Tool::Fillet);
    }

    #[test]
    fn a_face_chamfer_reopens_with_its_face() {
        reopens_in_the_session(Tool::Chamfer);
    }

    #[test]
    fn a_face_chamfer_reopens_with_its_face_after_opening_the_file() {
        reopens_after_opening_the_file(Tool::Chamfer);
    }

    #[test]
    fn a_face_patch_reopens_with_its_face() {
        reopens_in_the_session(Tool::Patch);
    }

    #[test]
    fn a_face_patch_reopens_with_its_face_after_opening_the_file() {
        reopens_after_opening_the_file(Tool::Patch);
    }
}
