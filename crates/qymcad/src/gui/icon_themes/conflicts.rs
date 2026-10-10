//! ICON THEME DUPLICATE ID CONFLICT VIEW.
//!
//! Renders diagnostics and resolution instructions when multiple packs claim the same theme identifier.

use egui_phosphor::regular as ph;
use qymcad_ui_state::icons::{DuplicateConflict, IconPack};

use super::appearance_section::draw_bundle_format_badge;

/// Draw the conflict details view for a pack that has an ID collision with another theme.
pub(crate) fn draw_duplicate_conflict_view(ui: &mut egui::Ui, pack: &IconPack, conflict: &DuplicateConflict, locale: &str) {
    let name = pack.manifest.name_for_locale(locale);
    let id = &pack.manifest.id;
    let source_path = pack.source_display();

    egui::ScrollArea::vertical().id_salt("mgr_conflict_scroll").auto_shrink([false, false]).show(ui, |ui| {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(48.0, 48.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 6.0, ui.visuals().error_fg_color.linear_multiply(0.18));
            ui.painter().rect_stroke(rect, 6.0, egui::Stroke::new(1.5, ui.visuals().error_fg_color), egui::StrokeKind::Inside);
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, ph::WARNING, egui::FontId::proportional(28.0), ui.visuals().error_fg_color);

            ui.vertical(|ui| {
                ui.label(egui::RichText::new(name).strong().size(18.0));
                ui.horizontal(|ui| {
                    draw_bundle_format_badge(ui, pack.format(), pack.is_tampered);
                    egui::Frame::NONE.fill(ui.visuals().error_fg_color.linear_multiply(0.20)).corner_radius(3.0).inner_margin(egui::Margin::symmetric(5, 2)).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 3.0;
                            ui.label(egui::RichText::new(ph::WARNING).color(ui.visuals().error_fg_color).small());
                            ui.label(egui::RichText::new(crate::i18n::tr("icontheme-mgr-conflict-badge")).color(ui.visuals().error_fg_color).small().strong());
                        });
                    });
                });
            });
        });

        ui.add_space(12.0);

        egui::Frame::NONE
            .fill(ui.visuals().error_fg_color.linear_multiply(0.12))
            .stroke(egui::Stroke::new(1.0, ui.visuals().error_fg_color))
            .corner_radius(6.0)
            .inner_margin(egui::Margin::symmetric(12, 10))
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::tr("icontheme-mgr-conflict-title")).strong().color(ui.visuals().error_fg_color));
                    ui.add_space(4.0);
                    let desc = if conflict.is_default_theme {
                        crate::i18n::tr1("icontheme-mgr-conflict-default-desc", "id", id)
                    } else {
                        crate::i18n::tr1("icontheme-mgr-conflict-desc", "id", id)
                    };
                    ui.label(egui::RichText::new(desc));
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new(format!("{}: {}", crate::i18n::tr("icontheme-mgr-conflict-path-label"), source_path)).small().monospace().weak());
                });
            });

        ui.add_space(14.0);

        egui::Frame::NONE.fill(ui.visuals().faint_bg_color).stroke(egui::Stroke::new(1.0, ui.visuals().weak_text_color())).corner_radius(6.0).inner_margin(egui::Margin::symmetric(12, 10)).show(
            ui,
            |ui| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::tr("icontheme-mgr-conflict-fix-title")).strong());
                    ui.add_space(6.0);
                    let fix_instructions = if pack.is_directory() {
                        crate::i18n::tr1("icontheme-mgr-conflict-fix-folder", "id", id)
                    } else {
                        crate::i18n::tr1("icontheme-mgr-conflict-fix-archive", "id", id)
                    };
                    ui.label(egui::RichText::new(fix_instructions));
                });
            },
        );

        ui.add_space(14.0);

        ui.label(egui::RichText::new(crate::i18n::tr("icontheme-mgr-meta-title")).strong());
        ui.add_space(4.0);
        ui.label(egui::RichText::new(crate::i18n::tr1("icontheme-mgr-meta-id", "value", id)).small().monospace());
        ui.label(egui::RichText::new(crate::i18n::tr1("icontheme-mgr-meta-version", "value", &pack.manifest.version)).small());
        if !pack.manifest.author.is_empty() {
            ui.label(egui::RichText::new(crate::i18n::tr1("icontheme-mgr-meta-author", "value", &pack.manifest.author)).small());
        }
        if !pack.manifest.license.is_empty() {
            ui.label(egui::RichText::new(crate::i18n::tr1("icontheme-mgr-meta-license", "value", &pack.manifest.license)).small());
        }
        let desc = pack.manifest.description_for_locale(locale);
        if !desc.is_empty() {
            ui.add_space(4.0);
            ui.label(egui::RichText::new(desc).small().weak());
        }
    });
}
