//! ICON THEME MANAGER SIDEBAR AND THEME CARDS.
//!
//! Provides the sidebar shell, bottom action buttons, and individual theme card widgets.

use egui_phosphor::regular as ph;

use super::discovery::user_themes_dir;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ThemeCardResponse {
    pub clicked: bool,
    #[allow(dead_code, reason = "inspected in card geometry unit tests")]
    pub rect: egui::Rect,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct IndexSwap {
    pub from: usize,
    pub to: usize,
}

/// Render a selectable theme card item in the icon theme manager sidebar.
pub(crate) fn manager_theme_card(ui: &mut egui::Ui, salt: impl std::hash::Hash + std::fmt::Debug, selected: bool, content: impl FnOnce(&mut egui::Ui) -> Option<egui::Rect>) -> ThemeCardResponse {
    let fill = if selected { ui.visuals().selection.bg_fill.linear_multiply(0.22) } else { ui.visuals().faint_bg_color };
    let (rect, background) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 68.0), egui::Sense::click());
    ui.painter().rect_filled(rect, 6.0, fill);
    let inner = rect.shrink2(egui::vec2(8.0, 6.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).id_salt(&salt));
    let action_rect = content(&mut child);
    let mut selection_rect = rect;
    if let Some(action_rect) = action_rect {
        selection_rect.max.x = action_rect.left();
    }
    let foreground = ui.interact(selection_rect, ui.id().with(("theme_card", &salt)), egui::Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
    ThemeCardResponse { clicked: foreground.clicked() || background.clicked(), rect }
}

/// Draw a truncated title for a theme card with full tooltip on hover.
pub(crate) fn draw_manager_card_title(ui: &mut egui::Ui, text_width: f32, title: &str, hover_text: &str) {
    ui.allocate_ui_with_layout(egui::vec2(text_width, 26.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.add(egui::Label::new(title).truncate().halign(egui::Align::Min));
    })
    .response
    .on_hover_text(hover_text);
}

/// Sidebar container with pinned bottom actions.
pub(crate) fn manager_sidebar_shell(ui: &mut egui::Ui, actions: impl FnOnce(&mut egui::Ui)) -> egui::ScrollArea {
    egui::Panel::bottom("icon_manager_actions").resizable(false).exact_size(44.0).show(ui, actions);
    egui::ScrollArea::vertical().id_salt("mgr_sidebar_scroll").auto_shrink([false, false])
}

/// Action bar at the bottom of the manager sidebar.
pub(crate) fn draw_icon_manager_actions(ui: &mut egui::Ui) {
    ui.add_space(6.0);
    if let Some(user_dir) = user_themes_dir() {
        if ui.add_sized([ui.available_width(), 28.0], egui::Button::new(format!("{} {}", ph::FOLDER_OPEN, crate::i18n::tr("settings-icon-open-folder")))).clicked() {
            let _ = std::fs::create_dir_all(&user_dir);
            let (bin, args) = crate::gui::reveal_command(ui.ctx().os(), &user_dir);
            let _ = crate::system::start(bin, &args);
        }
    }
}
