//! WHAT THE CURSOR CATCHES.
//!
//! Given the document, the view and a point on screen, these functions say what is under it: a vertex, an
//! edge, a face, a body, a datum plane, a contour. They change nothing and draw nothing - which is why
//! every workbench can call them and none of them owns the picking.

use egui::{Color32, Pos2, Rect};
use qymcad_core::geom::Point2;
use qymcad_core::feature::AnchorRef;
use qymcad_core::model::{Id, Project};
use qymcad_ui_state::grab::{grab, Grab};
use qymcad_ui_state::*;

#[cfg(test)]
mod arc_pick {
    use qymcad_core::feature::Purpose;
    use qymcad_core::model::Project;

    /// A CLICK WHERE AN ARC IS NOT DOES NOT TAKE IT: the upper half of a circle of radius 10 (from (10, 0) round to
    /// (-10, 0)) is caught on its top, and not at (0, -10), where the cut lower half had been.
    #[test]
    fn a_click_past_an_arc_does_not_take_it() {
        let mut p = Project::default();
        p.new_document();
        let si = p.new_sketch("S");
        let c = p.add_circle_entity(si, 0.0, 0.0, 10.0, Purpose::Real);
        let _ = p.add_line_entity(si, -20.0, 0.0, 20.0, 0.0, Purpose::Real);
        p.regen_sketch(si);
        assert!(p.trim_curve(si, c, 0.0, -10.0), "the lower half of the circle trimmed");
        let arc = p.sketches[si].entities.iter().find(|e| matches!(e.kind, qymcad_core::model::EntityKind::Arc { .. })).map(|e| e.id).expect("the upper arc");
        let (set, view) = (qymcad_ui_state::Settings::default(), qymcad_ui_state::View2d::default());
        let pick = qymcad_ui_state::PickCtx { project: &p, set: &set, view: &view };
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
        let sh = qymcad_ui_state::Sheet { view, rect };
        let at = |x: f64, y: f64| sh.at(qymcad_core::geom::Point2::new(x, y));
        assert_eq!(super::nearest_circle_entity(&pick, rect, at(0.0, 10.0), si), Some(arc), "the top of the arc");
        assert_eq!(super::nearest_circle_entity(&pick, rect, at(0.0, -10.0), si), None, "where the cut half had been");
    }
}

mod sketch_hits;
pub use sketch_hits::*;
mod body_hits;
pub use body_hits::*;
mod infer;
pub use infer::*;
