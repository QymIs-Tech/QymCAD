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
        app.sel = Sel::Sketch(si);
        (app, si)
    }

    /// Select every entity of the sketch — that is what the copy will take.
    fn select_all(app: &mut App, si: usize) {
        let ids: Vec<qymcad_core::model::Id> = app.project.sketches[si].entities.iter().map(|e| e.id).collect();
        assert!(!ids.is_empty(), "setup: the sketch has no entities to copy");
        app.sel_sk.items = ids.into_iter().map(|id| (1u8, id)).collect();
    }

    /// COPYING DROPS THE TOOL AND WHAT WAS HALF-BUILT WITH IT.
    #[test]
    fn copying_drops_an_active_tool() {
        let (mut app, si) = sketch_with_a_rectangle();
        select_all(&mut app, si);

        // A tool in hand and one vertex already put down: the line is half-built.
        Hand::new(&mut app).sk_tool(1).click2d(30.0, 30.0);
        assert_ne!(app.tool.kind, 0, "setup: the tool is not in hand — there is nothing to check");
        assert!(!app.tool.pts.is_empty(), "setup: the tool holds nothing half-built — the check would prove nothing");

        app.clipboard_copy_for_test(false);

        assert_eq!(app.tool.kind, 0, "the copy left the tool in hand: the next click has two claimants");
        assert!(app.tool.pts.is_empty(), "a half-built shape was left on the canvas with nothing to finish it");
    }

    /// THE COPY ITSELF STILL HAPPENS — the tool is put down, not the command.
    #[test]
    fn the_copy_still_waits_for_its_base_point() {
        let (mut app, si) = sketch_with_a_rectangle();
        select_all(&mut app, si);
        Hand::new(&mut app).sk_tool(1).click2d(30.0, 30.0);

        app.clipboard_copy_for_test(false);
        assert!(app.clip_geom_pending_for_test(), "the copy was lost together with the tool");
    }

    /// WITHOUT A TOOL IN HAND NOTHING CHANGES — the old path is untouched.
    #[test]
    fn copying_without_a_tool_is_unchanged() {
        let (mut app, si) = sketch_with_a_rectangle();
        select_all(&mut app, si);
        Hand::new(&mut app).sk_select();

        app.clipboard_copy_for_test(false);
        assert!(app.clip_geom_pending_for_test(), "the copy did not arm with no tool in hand either");
        assert_eq!(app.tool.kind, 0, "there was no tool, and one appeared");
    }

    /// NOTHING SELECTED: the tool stays in hand, because there was no copy to make.
    ///
    /// Putting the tool down here would punish a person for a mis-press: they meant to copy, nothing was
    /// selected, and the shape they were drawing would vanish along with the refusal.
    #[test]
    fn a_refused_copy_keeps_the_tool() {
        let (mut app, _si) = sketch_with_a_rectangle();
        Hand::new(&mut app).sk_tool(1).click2d(30.0, 30.0);
        let before = app.tool.kind;

        app.clipboard_copy_for_test(false);
        assert_eq!(app.tool.kind, before, "a copy with nothing selected took the tool away — a mis-press must not cost the drawing");
        assert!(!app.clip_geom_pending_for_test(), "with nothing selected there is nothing to copy");
    }

    /// The sketch entity ids stay put — nothing is being deleted here.
    #[test]
    fn the_geometry_survives_the_copy() {
        let (mut app, si) = sketch_with_a_rectangle();
        select_all(&mut app, si);
        let before = app.project.sketches[si].entities.len();
        Hand::new(&mut app).sk_tool(1).click2d(30.0, 30.0);

        app.clipboard_copy_for_test(false);
        assert_eq!(app.project.sketches[si].entities.len(), before, "putting the tool down deleted geometry");
        let _ = Point2::new(0.0, 0.0);
    }
}
