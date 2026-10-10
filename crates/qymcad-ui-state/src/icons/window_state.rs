//! Icon theme manager and packager window states.

use super::bundle::ValidationReport;

/// Active tab in the Icon Theme Manager window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum IconManagerTab {
    #[default]
    Readme,
    Gallery,
}

/// Notice shown when a theme directory or file path was copied to clipboard.
#[derive(Clone, Debug, PartialEq)]
pub struct CopiedPathNotice {
    pub path: String,
    pub timestamp: f64,
}

/// Notice shown after running the SVG hygiene cleaner.
#[derive(Clone, Debug, PartialEq)]
pub struct CleanNotice {
    pub pack_id: String,
    pub text: String,
    pub is_error: bool,
    pub details: Vec<String>,
}

/// Persistent state of the Icon Theme Manager window.
#[derive(Clone, Debug, PartialEq)]
pub struct IconManagerState {
    pub selected_pack_id: String,
    pub active_tab: IconManagerTab,
    pub search_query: String,
    pub category_filter: String,
    pub clean_notice: Option<CleanNotice>,
    pub copied_path: Option<CopiedPathNotice>,
}

impl Default for IconManagerState {
    fn default() -> Self {
        Self { selected_pack_id: String::new(), active_tab: IconManagerTab::Readme, search_query: String::new(), category_filter: "all".into(), clean_notice: None, copied_path: None }
    }
}

/// Persistent state of the Developer Theme Packager modal dialog.
#[derive(Clone, Debug, PartialEq)]
pub struct PackagerDialogState {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub license: String,
    pub description: String,
    pub source_dir: String,
    pub output_file: String,
    pub message: Option<String>,
    pub is_error: bool,
    pub report: Option<ValidationReport>,
}

impl Default for PackagerDialogState {
    fn default() -> Self {
        Self {
            id: "my-cad-theme".into(),
            name: "My CAD Theme".into(),
            version: "1.0.0".into(),
            author: String::new(),
            license: "LGPL-2.1-or-later".into(),
            description: "Custom CAD vector icons".into(),
            source_dir: String::new(),
            output_file: String::new(),
            message: None,
            is_error: false,
            report: None,
        }
    }
}
