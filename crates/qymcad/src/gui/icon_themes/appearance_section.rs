//! ICON THEME APPEARANCE SETTINGS SECTION AND APPLICATION.
//!
//! Provides the live settings appearance section and priority cascade applicator.

use egui_phosphor::regular as ph;
use qymcad_ui_state::icons::{load_default_pack, BundleFormat, DEFAULT_THEME_ID};
use qymcad_ui_state::{Settings, WinCtx};

use super::discovery::{all_theme_dirs, discover_all_theme_packs, ensure_discovery_worker};
use super::manager_window::open_icon_manager;

/// Apply the active icon packs from settings into the egui context.
pub(crate) fn apply_icon_themes(set: &Settings, ctx: &egui::Context, palette: &qymcad_scheme::Palette) {
    let dirs = all_theme_dirs();
    let stack = qymcad_ui_state::icons::load_active_icon_stack(&set.active_icon_packs, &dirs);
    qymcad_ui_state::icons::sync_watched_packs(ctx, &set.watched_icon_packs);
    qymcad_ui_state::icons::set_dev_watch(ctx, set.icon_dev_watch);
    qymcad_ui_state::icons::set_active_icon_stack(ctx, stack, palette);
}

/// Render a compact, colored visual badge indicating the format and provenance of an icon bundle.
pub(crate) fn draw_bundle_format_badge(ui: &mut egui::Ui, format: BundleFormat, is_tampered: bool) {
    let visuals = ui.visuals();
    let (icon, label_key, bg, fg) = if is_tampered {
        (ph::WARNING, "bundle-format-tampered", visuals.error_fg_color.linear_multiply(0.20), visuals.error_fg_color)
    } else {
        match format {
            BundleFormat::Directory => (ph::FOLDER_OPEN, "bundle-format-folder", visuals.warn_fg_color.linear_multiply(0.18), visuals.warn_fg_color),
            BundleFormat::Package => (ph::PACKAGE, "bundle-format-package", visuals.selection.bg_fill.linear_multiply(0.22), visuals.selection.bg_fill),
            BundleFormat::Embedded => (ph::GEAR, "bundle-format-embedded", visuals.faint_bg_color, visuals.weak_text_color()),
        }
    };

    egui::Frame::NONE.fill(bg).corner_radius(3.0).inner_margin(egui::Margin::symmetric(5, 2)).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            ui.label(egui::RichText::new(icon).color(fg).small());
            ui.label(egui::RichText::new(crate::i18n::tr(label_key)).color(fg).small().strong());
        });
    });
}

/// Draw the Icon Themes section in the Settings window (Appearance tab).
pub(crate) fn icon_theme_section(wc: &mut WinCtx, ui: &mut egui::Ui, ctx: &egui::Context) {
    ui.label(egui::RichText::new(crate::i18n::tr("settings-icon-themes-title")).strong());
    ui.label(egui::RichText::new(crate::i18n::tr("settings-icon-themes-desc")).small().weak());
    ui.add_space(4.0);

    let dirs = all_theme_dirs();
    ensure_discovery_worker(ctx, &dirs);
    let mut all_packs = discover_all_theme_packs(&dirs).packs;
    if !all_packs.iter().any(|p| p.manifest.id == DEFAULT_THEME_ID) {
        if let Some(def) = load_default_pack() {
            all_packs.push(def);
        }
    }

    // Single-line active cascade representation
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.label(egui::RichText::new(crate::i18n::tr("settings-icon-active-chain")).strong());

        for id in &wc.set.active_icon_packs {
            if let Some(pack) = all_packs.iter().find(|p| &p.manifest.id == id) {
                ui.label(egui::RichText::new(pack.manifest.name_for_locale(&crate::i18n::language())).strong());
                draw_bundle_format_badge(ui, pack.format(), pack.is_tampered);
                ui.label(egui::RichText::new(ph::ARROW_RIGHT).weak());
            } else {
                ui.label(egui::RichText::new(id).weak());
                ui.label(egui::RichText::new(ph::ARROW_RIGHT).weak());
            }
        }

        // Base fallback is always the built-in SVG bundle
        let base_name = all_packs
            .iter()
            .find(|pack| pack.manifest.id == DEFAULT_THEME_ID)
            .map(|pack| pack.manifest.name_for_locale(&crate::i18n::language()).to_string())
            .unwrap_or_else(|| crate::i18n::tr("settings-icon-themes-base"));
        ui.label(egui::RichText::new(base_name).strong());
        draw_bundle_format_badge(ui, BundleFormat::Embedded, false);
    });

    ui.add_space(6.0);
    if ui.button(format!("{} {}", ph::PALETTE, crate::i18n::tr("settings-open-icon-manager"))).clicked() {
        open_icon_manager(ctx);
    }
}
