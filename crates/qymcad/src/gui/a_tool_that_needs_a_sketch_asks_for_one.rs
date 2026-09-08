//! A TOOL THAT NEEDS A SKETCH ASKS FOR ONE INSTEAD OF NOT STARTING.
//!
//! Reported behaviour: "choosing the sketch for the operations in a part must be made more obvious -
//! perhaps, when a tool that needs geometry from a sketch is pressed, instead of quietly not turning it on
//! as it does now, we ask the person to pick a sketch."
//!
//! The tool was not quite silent: it wrote "Pick a sketch in the tree first, then the command" into the
//! status line. But nobody reads the status line at the moment of pressing a button - what a person sees
//! is that they pressed and nothing happened. A line of text is not an answer to a click.
//!
//! So the answer is a STATE, not a sentence: the tool is taken in hand and waits, and the next sketch that
//! gets selected continues the command.
//!
//! WAITING IS ANSWERED IN ONE PLACE, in the frame, rather than in the tree and in the viewport separately.
//! A sketch can be picked in either, and two copies of "if a command is waiting, continue it" would drift
//! apart at the first edit of one of them.
#[cfg(test)]
mod tests {
    use super::super::App;

    /// A part with a sketch that is NOT selected - the case the report is about.
    fn a_part_with_an_unselected_sketch() -> (App, usize) {
        let mut app = App::default();
        app.set.gpu_viewport = false;
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        app.project.add_rect_entity(si, 0.0, 0.0, 60.0, 40.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.finish_sketch_edit();
        app.chosen.sel = super::super::Sel::None;
        (app, si)
    }

    /// PRESSING THE TOOL WITH NOTHING SELECTED PUTS IT IN HAND AND WAITS.
    #[test]
    fn pressing_extrude_without_a_sketch_asks_for_one() {
        let (mut app, _) = a_part_with_an_unselected_sketch();
        app.start_feat_cmd(1);

        assert_eq!(
            app.tools.picking.sketch_for(),
            Some(1),
            "the tool answered the click with a line in the status bar and nothing else: a person sees that they pressed and nothing happened"
        );
        assert!(!app.status.is_empty(), "and the waiting must say what it is waiting for");
    }

    /// THE SKETCH IS PICKED BY CLICKING IT IN THE 3D VIEW, and the command opens.
    ///
    /// THE FIRST EDITION OF THIS CHECK ASSIGNED THE SELECTION instead of clicking, and that is why it was
    /// green over a half-made tool: a person had no way to select a sketch in the 3D view at all - only the
    /// tree set `Sel::Sketch`. Assigning what a click would have produced checks a path nobody has.
    #[test]
    fn clicking_the_sketch_in_the_viewport_opens_the_command() {
        let (mut app, si) = a_part_with_an_unselected_sketch();
        app.viewing.mode_3d = true;
        app.sync_workbench();
        app.start_feat_cmd(1);
        assert_eq!(app.tools.picking.sketch_for(), Some(1), "GUARD: the tool must be waiting");

        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0));
        crate::gui::fit3d(&mut app.viewing.cam, &app.project, rect);
        let basis = app.viewing.cam.basis();
        let on_outline = point_on_the_outline(&app, si, rect, &basis).expect("a point of the sketch outline on screen");
        app.viewport_3d_click_at(on_outline, rect, &basis);
        qymcad_part::take_sketch_if_waiting(&mut app.part_ctx());

        assert_eq!(app.tools.armed.cmd_kind(), 1, "a click on the sketch outline in the 3D view must open the command that was waiting for it");
    }

    /// Where a point of the sketch's outline lands on screen.
    fn point_on_the_outline(app: &App, si: usize, rect: egui::Rect, basis: &([f64; 3], [f64; 3], [f64; 3])) -> Option<egui::Pos2> {
        let cid = *app.project.sketches[si].contour_ids.first()?;
        let ci = app.project.contour_index(cid)?;
        let a = app.project.contours[ci].points[0];
        let b = app.project.contours[ci].points[1];
        let mid = qymcad_core::geom::Point2::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
        Some(qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect, basis }.at([mid.x, mid.y, 0.0]).0)
    }

    /// AND THE SKETCH UNDER THE CURSOR LIGHTS UP while the tool waits - as a face does under a fillet.
    #[test]
    fn the_sketch_under_the_cursor_lights_up_while_the_tool_waits() {
        let (mut app, si) = a_part_with_an_unselected_sketch();
        app.viewing.mode_3d = true;
        app.sync_workbench();
        app.start_feat_cmd(1);
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0));
        crate::gui::fit3d(&mut app.viewing.cam, &app.project, rect);
        let basis = app.viewing.cam.basis();
        let on_outline = point_on_the_outline(&app, si, rect, &basis).expect("a point of the outline");

        assert_eq!(
            crate::gui::pick::sketch_at_3d(&app.painting(), rect, on_outline),
            Some(si),
            "the cursor is on the outline and the program does not know which sketch it is over, so nothing can light up"
        );
    }

    /// EVERY TOOL THAT NEEDS A SKETCH WAITS - not two of the four.
    ///
    /// Extrude and revolve open through one door, sweep and loft through two others. The first edition
    /// taught only the first door, so pressing Sweep or Loft with nothing selected went on doing what it
    /// always did: a line in the status bar and no tool in hand.
    #[test]
    fn all_four_tools_that_need_a_sketch_wait_for_one() {
        let mut missing = Vec::new();
        for kind in [1u8, 3, 8, 9] {
            let (mut app, _) = a_part_with_an_unselected_sketch();
            app.start_feat_cmd(kind);
            if app.tools.picking.sketch_for() != Some(kind) {
                missing.push(format!("{kind}: waiting for {:?}", app.tools.picking.sketch_for()));
            }
        }
        assert!(missing.is_empty(), "a tool that needs a sketch does not ask for one, so pressing it does nothing a person can see:\n{}", missing.join("\n"));
    }

    /// WITH A SKETCH ALREADY SELECTED NOTHING WAITS: the command opens at once, as before.
    ///
    /// Without this the change would be an improvement for the empty case and an extra step for the
    /// ordinary one.
    #[test]
    fn with_the_sketch_already_selected_the_command_opens_at_once() {
        let (mut app, _) = a_part_with_an_unselected_sketch();
        app.chosen.sel = super::super::Sel::Sketch(0);
        app.start_feat_cmd(1);

        assert_eq!(app.tools.picking.sketch_for(), None, "there is nothing to wait for - the sketch is chosen");
        assert_eq!(app.tools.armed.cmd_kind(), 1, "and the command is open");
    }

    /// ESCAPE PUTS THE WAITING TOOL DOWN.
    ///
    /// A tool that waits and cannot be put down is worse than one that does not start: the next click goes
    /// somewhere the person did not intend.
    #[test]
    fn escape_puts_the_waiting_tool_down() {
        let (mut app, _) = a_part_with_an_unselected_sketch();
        app.start_feat_cmd(1);
        assert_eq!(app.tools.picking.sketch_for(), Some(1), "GUARD: the tool must be waiting before Esc has anything to do");

        app.on_escape();
        assert_eq!(app.tools.picking.sketch_for(), None, "Esc must put down a tool that is waiting for a sketch");
    }
}
