//! COPYING PUTS THE DRAWING TOOL DOWN FIRST.
//!
//! Reported behaviour: copying while a tool is active does not cancel the tool, and the result is a defect.
//!
//! Copying is a command over what is SELECTED, while a tool is busy with what is being BUILT. Leaving both
//! armed puts two consumers on the next click: the copy waits for a base point, the tool waits for the next
//! vertex. Whichever gets there first, the person did not ask for it — and a half-built shape stays behind on
//! the canvas with nothing to finish it.
#[cfg(test)]
mod tests {
    use super::super::hand::Hand;
    use super::super::{App, Sel};
    use qymcad_core::feature::SketchPlane;
    use qymcad_core::geom::Point2;

    /// A sketch being edited, with a rectangle in it. Returns the application and the sketch index.
    fn sketch_with_a_rectangle() -> (App, usize) {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        app.project.add_rect_entity(si, -20.0, -20.0, 20.0, 20.0, qymcad_core::feature::Purpose::Real);
        app.project.regen_sketch(si);
        app.chosen.sel = Sel::Sketch(si);
        (app, si)
    }

    /// Select every entity of the sketch with Ctrl+A — that is what the copy will take.
    ///
    /// By the keys and not by writing the selection in: the window drops the selection on the first frame it
    /// sees a sketch it has not seen open, so a selection written in before any frame is gone by the click.
    fn select_all(app: &mut App, si: usize) {
        Hand::canvas(app).ctrl(egui::Key::A);
        let want = app.project.sketches[si].entities.len();
        let got = app.tools.sel_sk.items.iter().filter(|it| it.kind == qymcad_ui_state::ItemKind::Curve).count();
        assert!(want > 0 && got == want, "setup: Ctrl+A selected {got} of the {want} entities to copy");
    }

    /// COPYING DROPS THE TOOL AND WHAT WAS HALF-BUILT WITH IT.
    #[test]
    fn copying_drops_an_active_tool() {
        let (mut app, si) = sketch_with_a_rectangle();
        // A tool in hand and one vertex already put down: the line is half-built. Then everything selected with
        // Ctrl+A - after the tool, since taking a drawing tool drops the selection made before it.
        Hand::new(&mut app).sk_tool(qymcad_ui_state::SketchShape::Line).click2d(30.0, 30.0);
        select_all(&mut app, si);
        assert_ne!(app.tools.armed.drawing(), None, "setup: the tool is not in hand — there is nothing to check");
        assert!(!app.tools.tool.pts.is_empty(), "setup: the tool holds nothing half-built — the check would prove nothing");

        Hand::canvas(&mut app).copy(); // Ctrl+C

        assert_eq!(app.tools.armed.drawing(), None, "the copy left the tool in hand: the next click has two claimants");
        assert!(app.tools.tool.pts.is_empty(), "a half-built shape was left on the canvas with nothing to finish it");
    }

    /// THE COPY ITSELF STILL HAPPENS — the tool is put down, not the command.
    #[test]
    fn the_copy_still_waits_for_its_base_point() {
        let (mut app, si) = sketch_with_a_rectangle();
        Hand::new(&mut app).sk_tool(qymcad_ui_state::SketchShape::Line).click2d(30.0, 30.0);
        select_all(&mut app, si); // after the tool: taking a drawing tool drops the selection made before it

        Hand::canvas(&mut app).copy(); // Ctrl+C
        assert!(app.side.clip.geom_pending.is_some(), "the copy was lost together with the tool");
    }

    /// WITHOUT A TOOL IN HAND NOTHING CHANGES — the old path is untouched.
    #[test]
    fn copying_without_a_tool_is_unchanged() {
        let (mut app, si) = sketch_with_a_rectangle();
        select_all(&mut app, si);
        Hand::new(&mut app).sk_select();

        Hand::canvas(&mut app).copy(); // Ctrl+C
        assert!(app.side.clip.geom_pending.is_some(), "the copy did not arm with no tool in hand either");
        assert_eq!(app.tools.armed.drawing(), None, "there was no tool, and one appeared");
    }

    /// NOTHING SELECTED: the tool stays in hand, because there was no copy to make.
    ///
    /// Putting the tool down here would punish a person for a mis-press: they meant to copy, nothing was
    /// selected, and the shape they were drawing would vanish along with the refusal.
    #[test]
    fn a_refused_copy_keeps_the_tool() {
        let (mut app, _si) = sketch_with_a_rectangle();
        Hand::new(&mut app).sk_tool(qymcad_ui_state::SketchShape::Line).click2d(30.0, 30.0);
        let before = app.tools.armed.drawing();

        Hand::canvas(&mut app).copy(); // Ctrl+C
        assert_eq!(app.tools.armed.drawing(), before, "a copy with nothing selected took the tool away — a mis-press must not cost the drawing");
        assert!(!app.side.clip.geom_pending.is_some(), "with nothing selected there is nothing to copy");
    }

    /// The sketch entity ids stay put — nothing is being deleted here.
    #[test]
    fn the_geometry_survives_the_copy() {
        let (mut app, si) = sketch_with_a_rectangle();
        select_all(&mut app, si);
        let before = app.project.sketches[si].entities.len();
        Hand::new(&mut app).sk_tool(qymcad_ui_state::SketchShape::Line).click2d(30.0, 30.0);

        Hand::canvas(&mut app).copy(); // Ctrl+C
        assert_eq!(app.project.sketches[si].entities.len(), before, "putting the tool down deleted geometry");
        let _ = Point2::new(0.0, 0.0);
    }
}
