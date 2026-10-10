//! Dynamic vector icon themes and bundles.
//!
//! A bundle is a directory or a `.qicons` archive (ZIP) carrying an `icons/` folder and a `manifest.ron`.
//! Tools resolve their icons through a cascade stack: top active pack -> lower packs -> built-in
//! default SVG pack.
//!
//! Strict folder paths determine icon identity (`icons/sketch/line.svg` -> `IconId::SketchLine`),
//! eliminating redundant mapping tables.

pub mod bundle;
pub mod id;
pub mod manager;
pub mod manifest;
pub mod pack;
pub mod runtime;
pub mod sha256;
pub mod watcher;

#[cfg(test)]
mod tests;

pub use bundle::{
    clean_directory_icon, clean_directory_icons, clean_svg, cleanup_residual_tmp_files, directory_has_cleanable_icons, discover_packs_detailed, discover_packs_in, find_svg_junk_issues,
    inspect_pack_directory, is_cleaner_temp_file, package_bundle, package_bundle_to_bytes, package_bundle_to_writer, validate_icon_svg, validate_svg, CategoryCoverage, CleanFileFailure,
    CleanIconResult, CleanPackReport, CoverageCount, DiscoveryError, DiscoveryReport, PackagedBundle, RejectedArchive, ValidationReport,
};
pub use id::{BuiltinTheme, IconId, ALL_ICONS};
pub use manager::{
    bump_icon_revision, ensure_watcher_thread, get_active_icon_stack, get_icon_revision, has_watched_icon_packs, install_cad_icons, is_pack_watched, load_active_icon_stack, load_builtin_pack,
    load_builtin_packs, load_default_pack, poll_watched_icon_packs, resolve_icon, resolve_icon_tokens, set_active_icon_stack, set_dev_watch, set_pack_watching, stop_all_watcher_threads,
    stop_watcher_thread, sync_watched_packs, update_cad_icon, ActiveIconStack, IconWatcher, BUILTIN_ICON_THEMES, DEFAULT_THEME_ID,
};
pub use manifest::{
    has_zalgo, is_combining_mark, IconManifest, LocalizedThemeText, PackageType, MAX_MANIFEST_AUTHOR_LEN, MAX_MANIFEST_DESCRIPTION_LEN, MAX_MANIFEST_ID_LEN, MAX_MANIFEST_LICENSE_LEN,
    MAX_MANIFEST_LOCALE_LEN, MAX_MANIFEST_NAME_LEN, MAX_MANIFEST_TRANSLATIONS, MAX_MANIFEST_VERSION_LEN,
};
pub use pack::{BundleFormat, DuplicateConflict, FileSignature, IconPack, PackSource, ThemePreviewAsset};
pub use runtime::{forget_all_cad_icons, icon_button, icon_image, icon_label, icon_small_button, ResolvedIcon};
