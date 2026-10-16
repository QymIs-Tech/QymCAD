//! Lightweight runtime icon access and egui texture cache bridge.
//!
//! Icons are pre-installed directly into egui's loaders via `ctx.include_bytes`
//! under canonical URIs (`bytes://qicons/...`). Rendering widgets display them with
//! `egui::Image::new(icon.uri())` with ZERO frame-time allocations or token replacement.

use std::sync::Arc;

use super::id::{IconId, ALL_ICONS};

/// The result of resolving an icon through the priority stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedIcon {
    /// SVG file bytes.
    pub data: Arc<[u8]>,
    /// Identifier of the pack supplying the icon.
    pub pack_id: String,
    /// Canonical egui image URI.
    pub uri: &'static str,
}

impl ResolvedIcon {
    pub fn new(data: Arc<[u8]>, pack_id: impl Into<String>, id: IconId) -> Self {
        Self { data, pack_id: pack_id.into(), uri: id.uri() }
    }
}

/// Ensure default CAD icons are loaded into `ctx` if not yet installed.
pub fn ensure_cad_icons_installed(ctx: &egui::Context) {
    let id_key = egui::Id::new("cad_icons_installed");
    let is_installed = ctx.data(|d| d.get_temp::<bool>(id_key).unwrap_or(false));
    if !is_installed {
        ctx.data_mut(|d| d.insert_temp(id_key, true));
        super::manager::install_cad_icons(ctx, &[], &qymcad_scheme::dark());
    }
}

/// Fast-path runtime render for CAD icons.
/// Renders from pre-installed bytes in egui's loader under canonical `icon.uri()`.
pub fn icon_image(ui: &egui::Ui, icon: IconId, size: f32) -> egui::Image<'static> {
    ensure_cad_icons_installed(ui.ctx());
    egui::Image::new(icon.uri()).fit_to_exact_size(egui::vec2(size, size))
}

/// Button widget displaying a CAD icon.
pub fn icon_button(ui: &mut egui::Ui, icon: IconId, size: f32, selected: bool) -> egui::Response {
    let img = icon_image(ui, icon, size);
    ui.add(egui::Button::image(img).selected(selected))
}

/// Small button widget displaying a CAD icon (14px).
pub fn icon_small_button(ui: &mut egui::Ui, icon: IconId, selected: bool) -> egui::Response {
    let img = icon_image(ui, icon, 14.0);
    ui.add(egui::Button::image(img).small().selected(selected))
}

/// Label widget displaying an icon next to text.
pub fn icon_label(ui: &mut egui::Ui, icon: IconId, size: f32, text: impl Into<egui::WidgetText>) -> egui::Response {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.add(icon_image(ui, icon, size));
        ui.label(text)
    })
    .response
}

/// Evict all previously rendered CAD icon textures from the egui image cache.
pub fn forget_all_cad_icons(ctx: &egui::Context) {
    for &id in ALL_ICONS {
        ctx.forget_image(id.uri());
    }
}
