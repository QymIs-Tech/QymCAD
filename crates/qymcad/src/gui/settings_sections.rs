//! SECTIONS OF THE SETTINGS WINDOW — ONE SOURCE for the list on the left, for search and for reset.
//!
//! The window used to be one flat scroll where the sections were merely bold captions. That does not
//! scale: there will be three times as many settings (autosave, interface scale, pick precision,
//! recent files), and a flat list turns into a sheet that cannot be searched.
//!
//! WHY ONE TABLE AND NOT THREE LISTS. A section needs three things: what to show in it, what to
//! search, and what to reset. Split those across three places and they drift silently: a new setting
//! reaches the window but is not found by search, or is found but is not reset. So the row labels are
//! declared here in [`SettingsSection::row_keys`], and a guard cross-checks them against the SOURCE of
//! the window in both directions.
//!
//! The reset is spelled out field by field rather than "take the defaults wholesale": "reset the
//! section" must touch EXACTLY that section, otherwise the button in "Sketch" would wipe the chosen
//! language.
use super::Settings;

/// A section of the settings window. The order of the variants is the order in the list on the left.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum SettingsSection {
    General,
    Appearance,
    Viewport,
    Sketch,
    Part,
    Assembly,
    Cam,
}

impl SettingsSection {
    /// All the sections in display order.
    pub(crate) fn all() -> &'static [Self] {
        use SettingsSection::*;
        &[General, Appearance, Viewport, Sketch, Part, Assembly, Cam]
    }

    /// The catalogue key holding the section name.
    pub(crate) fn key(self) -> &'static str {
        use SettingsSection::*;
        match self {
            General => "settings-sec-general",
            Appearance => "settings-sec-appearance",
            Viewport => "settings-sec-viewport",
            Sketch => "settings-sec-sketch",
            Part => "settings-sec-part",
            Assembly => "settings-sec-assembly",
            Cam => "settings-sec-cam",
        }
    }

    /// The MACHINING section is shown only when the module is enabled: while the checkbox is off, no
    /// CAM internals should be visible at all.
    pub(crate) fn is_cam(self) -> bool {
        self == SettingsSection::Cam
    }

    /// THE ROW LABELS OF A SECTION — what search looks through. A guard cross-checks them against the window source.
    pub(crate) fn row_keys(self) -> &'static [&'static str] {
        use SettingsSection::*;
        match self {
            General => &["settings-language", "settings-help-lang", "settings-help-open", "settings-autosave", "settings-undo-cap", "settings-recent-limit", "settings-profile"],
            Appearance => &["settings-scheme", "settings-ui-scale"],
            Viewport => &["settings-engine", "settings-projection", "settings-shading", "settings-viewcube", "settings-pick-precision", "settings-ghost-alpha", "settings-fov", "settings-msaa"],
            Sketch => &["settings-snap-on", "settings-grid-step", "settings-rot-step", "settings-auto-constrain"],
            Part => &["settings-default-extrude", "settings-default-offset"],
            Assembly => &["settings-show-contours", "settings-show-joints", "settings-show-interference"],
            Cam => &["cam-tab-checkbox", "settings-rapids"],
        }
    }

    /// Whether a row matches the search query. An empty query matches everything.
    pub(crate) fn row_matches(key: &str, query: &str) -> bool {
        let q = query.trim().to_lowercase();
        q.is_empty() || crate::i18n::tr(key).to_lowercase().contains(&q)
    }

    /// Whether the section holds any row matching the query; that decides whether to show it at all.
    pub(crate) fn has_match(self, query: &str) -> bool {
        self.row_keys().iter().any(|k| Self::row_matches(k, query))
    }

    /// RESTORE THE FACTORY VALUES OF THIS SECTION ONLY. Nothing else is touched.
    pub(crate) fn reset(self, s: &mut Settings) {
        let d = Settings::default();
        use SettingsSection::*;
        match self {
            General => {
                s.language = d.language;
                s.help_lang = d.help_lang;
                s.help_external = d.help_external;
                s.autosave_secs = d.autosave_secs;
                s.undo_cap = d.undo_cap;
                s.recent_limit = d.recent_limit;
                // THE RECENT LIST ITSELF IS NOT TOUCHED BY A RESET: it is not a setting but a
                // history of work. "Reset the section" means "restore the factory values", not
                // "forget what I did"; the File menu has a separate item for the latter.
            }
            Appearance => {
                s.scheme = d.scheme;
                s.ui_scale = d.ui_scale;
            }
            Viewport => {
                s.gpu_viewport = d.gpu_viewport;
                s.cam_perspective = d.cam_perspective;
                s.smooth_shading = d.smooth_shading;
                s.viewcube_size = d.viewcube_size;
                s.pick_precision = d.pick_precision;
                s.ghost_alpha = d.ghost_alpha;
                s.persp_fov_deg = d.persp_fov_deg;
                s.msaa = d.msaa;
            }
            Sketch => {
                s.snap = d.snap;
                s.auto_constrain = d.auto_constrain;
            }
            Part => s.defaults = d.defaults,
            Assembly => {
                s.show_contours = d.show_contours;
                s.show_joints = d.show_joints;
                s.show_interference = d.show_interference;
            }
            Cam => {
                s.cam_tab_enabled = d.cam_tab_enabled;
                s.show_rapids = d.show_rapids;
            }
        }
    }
}
