//! THE DRAWING LAYER.
//!
//! Given the document, the view and a painter, these functions draw: bodies, sketches, dimensions,
//! constraints, joints, previews, the gizmos. They change nothing - each takes a shared borrow of the
//! state and hands egui a list of shapes. That is why all three workbenches can call them and none owns
//! the drawing.

use egui::{Color32, Pos2, Rect, Stroke};
use egui_phosphor::regular as ph;
use qymcad_core::geom::Point2;
use qymcad_core::model::{Id, Project};
use qymcad_ui_state::*;

/// A tool button with a drawn glyph (instead of raw unicode that renders as tofu).
pub fn sym_button(ui: &mut egui::Ui, g: Gly, tip: &str, active: bool) -> bool {
    // The size is THE SAME as `icon_tool`'s (40x34), otherwise a mismatch of widths breaks the wrap onto two columns.
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(40.0, 34.0), egui::Sense::click());
    let vis = ui.style().interact_selectable(&resp, active);
    ui.painter().rect(rect, 3.0, vis.bg_fill, vis.bg_stroke, egui::StrokeKind::Middle);
    paint_gly(ui.painter(), rect.center(), 9.0, g, vis.fg_stroke.color);
    resp.on_hover_text(tip).clicked()
}

#[cfg(test)]
mod blend_tests {
    use super::interp_color;
    use egui::Color32;

    /// A POINT BETWEEN OPAQUE CORNERS IS OPAQUE: the weights of a triangle's corners add up to one only to the float,
    /// and a blend cut down to the byte below made a point of an opaque body 254 see-through and every channel half a
    /// step darker - every smoothly lit point of the software picture. Walked over a triangle the way `raster_band`
    /// walks it.
    #[test]
    fn a_point_between_opaque_corners_is_opaque() {
        let cols = [Color32::RED, Color32::GREEN, Color32::BLUE];
        let (v0, v1, v2) = ([0.0f32, 0.0], [97.0f32, 13.0], [31.0f32, 71.0]);
        let e = qymcad_ui_state::edge;
        let inv = 1.0 / e(v0[0], v0[1], v1[0], v1[1], v2[0], v2[1]);
        let (mut inside, mut short) = (0usize, 0usize);
        for py in 0..80 {
            for px in 0..100 {
                let (fx, fy) = (px as f32 + 0.5, py as f32 + 0.5);
                let w = [e(v1[0], v1[1], v2[0], v2[1], fx, fy) * inv, e(v2[0], v2[1], v0[0], v0[1], fx, fy) * inv, e(v0[0], v0[1], v1[0], v1[1], fx, fy) * inv];
                if w.iter().any(|x| *x < 0.0) {
                    continue;
                }
                inside += 1;
                short += usize::from(interp_color(&cols, w[0], w[1], w[2]).a() != 255);
            }
        }
        assert!(inside > 1000, "the triangle covers {inside} points");
        assert_eq!(short, 0, "{short} of {inside} points between opaque corners come out see-through");
    }
}

mod cards;
pub use cards::*;
mod dim_text;
pub use dim_text::*;
mod glyphs;
pub use glyphs::*;
mod raster;
mod floor;
pub use floor::*;
pub use raster::*;
mod gizmos;
pub use gizmos::*;
mod sketch_overlay;
pub use sketch_overlay::*;
mod sketch_previews;
pub use sketch_previews::*;
mod sketch_marks;
pub use sketch_marks::*;
mod part_previews;
pub use part_previews::*;
mod pickers;
pub use pickers::*;
