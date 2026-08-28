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
        app.rebuild_if_dirty();
        app.refresh_edges();
        app.mode_3d = true;
        app.cam.init = true;
        app.cam.scale = 6.0;
        app.cam.target = [30.0, 10.0, 5.0];
        app.project.bodies.iter().map(|b| b.id).filter(|b| !before.contains(b)).collect()
    }

    /// A screen point OVER A PART: the centre of its top face.
    fn over_the_part(app: &App, body: Id) -> egui::Pos2 {
        let wt = app.project.body_display_transform(body, app.current_ctx_id_for_test());
        let f = app
            .project
            .regen_faces
            .get(&body)
            .and_then(|fs| fs.iter().max_by(|a, b| a.centroid.z.total_cmp(&b.centroid.z)))
            .expect("the body has faces");
        let w = qymcad_core::feature::apply12(&wt, [f.centroid.x, f.centroid.y, f.centroid.z]);
        let basis = app.cam.basis();
        app.project3(w, viewport(), &basis).0
    }

    /// How many shapes the WHOLE highlight pass draws with the cursor at `at`.
    fn shapes_with_cursor(app: &mut App, at: Option<egui::Pos2>) -> usize {
        let ctx = egui::Context::default();
        super::super::install_fonts(&ctx);
        app.refresh_edges();
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
                    app.draw_pick_highlights_for_test(&painter, viewport());
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
        let tools: [(&str, fn(&mut App)); 5] = [
            ("mate", |a: &mut App| a.arm_joint_pick_for_test()),
            ("anchor", |a: &mut App| a.start_conn_pick()),
            ("group", |a: &mut App| a.start_group_pick()),
            ("width", |a: &mut App| a.start_width_pick()),
            ("tangency", |a: &mut App| a.start_tangent_pick()),
        ];
        let mut blind: Vec<String> = Vec::new();
        for (name, arm) in tools {
            let mut app = App::default();
            let mine = two_parts(&mut app);
            assert_eq!(mine.len(), 2, "setup: there should be two bodies of our own, and there are {}", mine.len());
            app.workbench = super::super::Workbench::Assembly;
            arm(&mut app);
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
