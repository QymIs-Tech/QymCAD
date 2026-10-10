//! ICON THEME CONFIGURATION AND MANAGEMENT.
//!
//! Provides discovery, priority-cascade configuration, and bundle packaging
//! for dynamic CAD vector icon packs.

pub(crate) mod appearance_section;
pub(crate) mod conflicts;
pub(crate) mod discovery;
pub(crate) mod gallery;
pub(crate) mod manager_window;
pub(crate) mod packager_dialog;
pub(crate) mod sidebar;

#[cfg(test)]
mod tests;

pub(crate) use appearance_section::{apply_icon_themes, icon_theme_section};
pub(crate) use manager_window::draw_icon_manager_window;
