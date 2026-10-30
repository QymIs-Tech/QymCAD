//! WHAT A CLICK HANDS BACK AT ZERO IS THE PERSON'S GEOMETRY, and never the frame of reference.
//!
//! Four points stand at or beside the origin of every sketch a person has touched: the origin itself, the
//! anchor of the frame at the same (0,0), and the two axis guides one millimetre away at (1,0) and (0,1). At
//! the ordinary zoom of 6 px per mm a guide is 6 px from zero while the pick radius of a point is 10 - so a
//! click at zero has four candidates, three of which belong to the frame and not to the drawing.
//!
//! `sketch_hit`, which serves selection, knows this: it skips the anchor and the guides and lets ordinary
//! geometry win over the origin. `nearest_sketch_point`, which serves dimensions, references and the corner
//! tools, did not - and it is a different door to the same sketch.
#[cfg(test)]
mod tests {
    use super::super::{App, Sel};
    use qymcad_core::feature::{Purpose, SketchPlane};
    use qymcad_core::model::SketchAxis;

    #[test]
    fn a_point_picked_at_zero_is_geometry_and_not_the_frame() {
        let mut app = App::default();
        let si = app.create_sketch_on(SketchPlane::default());
        // the frame of reference as it exists in any sketch that has been dimensioned or constrained
        let origin = app.project.ensure_origin(si);
        let _ = app.project.ensure_axis(si, SketchAxis::X);
        let _ = app.project.ensure_axis(si, SketchAxis::Y);
        let frame = app.project.ensure_frame(si);
        // and the person's own line, drawn from zero
        app.project.add_line_entity(si, 0.0, 0.0, 20.0, 0.0, Purpose::Real);
        app.project.regen_sketch(si);
        app.chosen.sel = Sel::Sketch(si);

        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
        let at = (qymcad_ui_state::Sheet { view: app.viewing.view, rect }).at(qymcad_core::geom::Point2::new(0.0, 0.0));
        let got = crate::gui::pick::nearest_sketch_point(&app.pick_ctx(), rect, at, si).expect("a click at zero has to pick something");

        let guides = app.project.sketches[si].axis_pts;
        let mine: Vec<u64> = app.project.sketches[si].points.iter().map(|p| p.id).filter(|id| *id != origin && *id != frame && !guides.contains(id)).collect();
        eprintln!("picked {got}, origin {origin}, anchor {frame}, guides {guides:?}, the drawing's own {mine:?}");
        assert_ne!(got, frame, "the anchor of the frame was handed out - it is nobody's geometry and cannot be dimensioned to");
        assert!(!guides.contains(&got), "an axis guide was handed out - it marks an infinite line, it is not a vertex");
        assert_ne!(got, origin, "the origin was handed out while the person's own point stands in the same place");
    }
}
