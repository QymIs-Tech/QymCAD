//! SOMETHING LIGHTS UP UNDER THE CURSOR FOR EVERY TOOL THAT ASKS FOR GEOMETRY.
//!
//! Reported behaviour: edge highlighting worked neither in the reference axis nor in "pick axis", no
//! matter how the cursor was moved. The cause was one line: the mode was simply not named in the
//! drawing condition. The trouble itself takes a minute to fix, but its CLASS remains: any new tool
//! will be forgotten in the same place, and a person will aim blind again.
//!
//! So the guard checks them all at once: a tool is taken, the cursor is placed ON A PART, and more
//! must be painted in the frame than with the cursor over emptiness. What exactly lights up is the
//! tool's business; what matters here is that a person SEES where they are aiming.
#[cfg(test)]
mod tests {
    use super::super::App;
    use qymcad_core::model::Id;

    fn viewport() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0))
    }

    /// Two parts side by side, with the camera looking at them.
    fn two_parts(app: &mut App) -> Vec<Id> {
        let before: Vec<Id> = app.project.bodies.iter().map(|b| b.id).collect();
        super::super::joint_flow::tests::add_part_at(app, 0.0);
        super::super::joint_flow::tests::add_part_at(app, 60.0);
        let root = app.project.root;
        app.enter_component(root);
        qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
        crate::gui::commands::refresh_edges(&mut app.part_ctx());
        app.viewing.mode_3d = true;
        app.viewing.cam.init = true;
        app.viewing.cam.scale = 6.0;
        app.viewing.cam.target = [30.0, 10.0, 5.0];
        app.project.bodies.iter().map(|b| b.id).filter(|b| !before.contains(b)).collect()
    }

    /// A screen point OVER A PART: the centre of its top face.
    fn over_the_part(app: &App, body: Id) -> egui::Pos2 {
        let wt = app.project.body_display_transform(body, qymcad_ui_state::current_ctx_id(&app.active_path, &app.project));
        let f = app
            .project
            .regen_faces
            .get(&body)
            .and_then(|fs| fs.iter().max_by(|a, b| a.centroid.z.total_cmp(&b.centroid.z)))
            .expect("the body has faces");
        let w = qymcad_core::feature::apply12(&wt, [f.centroid.x, f.centroid.y, f.centroid.z]);
        let basis = app.viewing.cam.basis();
        qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect: viewport(), basis: &basis }.at(w).0
    }

    /// How many shapes the WHOLE highlight pass draws with the cursor at `at`.
    fn shapes_with_cursor(app: &mut App, at: Option<egui::Pos2>) -> usize {
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        crate::gui::commands::refresh_edges(&mut app.part_ctx());
        let mut count = 0;
        // TWO FRAMES: egui learns the cursor position from an event, and there is no hover until the next frame.
        for _ in 0..2 {
            let mut input = egui::RawInput { screen_rect: Some(viewport()), ..Default::default() };
            if let Some(p) = at {
                input.events.push(egui::Event::PointerMoved(p));
            }
            let out = ctx.run_ui(input, |c| {
                egui::CentralPanel::default().show(c, |ui| {
                    let painter = ui.painter().clone();
                    crate::gui::render::draw_joint_pick_highlight(&app.painting(), &painter, viewport());
                });
            });
            count = out.shapes.len();
        }
        count
    }

    #[test]
    fn every_tool_that_asks_for_geometry_shows_what_is_under_the_cursor() {
        // THE TOOLS THAT ASK FOR GEOMETRY. A relation is not among them: it is picked by clicking
        // JOINTS in the list rather than a part in the viewport.
        // EVERY TOOL THERE IS, from the shared door table (`assembly_tools::doors`). It used to be a
        // hand-written subset here - and each of the four checks of this kind had its OWN subset, of seven,
        // seven, five and six, with no reason written for what was left out.
        let mut blind: Vec<String> = Vec::new();
        // ONLY THE TOOLS THAT ASK FOR GEOMETRY, by the criterion the code already carries
        // (`AssemblyTool::wants_geometry`) rather than by a subset chosen here. The relation tool asks for
        // clicks on the MATES IN THE LIST and never points at a body, so "nothing lights up under the
        // cursor" is right for it - and a hand-written subset said the same thing without saying why.
        for t in super::super::assembly_tools::AssemblyTool::ALL.into_iter().filter(|t| t.wants_geometry()) {
            let name = t.help_mode();
            let mut app = App::default();
            let mine = two_parts(&mut app);
            assert_eq!(mine.len(), 2, "setup: there should be two bodies of our own, and there are {}", mine.len());
            app.workbench = super::super::Workbench::Assembly;
            crate::gui::assembly_tools::doors::arm(&mut app, t);
            let at = over_the_part(&app, mine[0]);

            let empty = shapes_with_cursor(&mut app, None);
            let hovered = shapes_with_cursor(&mut app, Some(at));
            if hovered <= empty {
                blind.push(format!("\"{name}\": nothing lights up under the cursor ({empty} shapes without it, {hovered} with it)"));
            }
        }
        assert!(
            blind.is_empty(),
            "the tool asks for geometry while the person aims blind:\n{}",
            blind.join("\n")
        );
    }
}
