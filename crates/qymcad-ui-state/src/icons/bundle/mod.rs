//! Pack discovery, SVG validation, diagnostic inspection, and .qicons bundle packaging.
//!
//! Submodules organize the specific domains:
//! - `hygiene`: SVG validation, security sanitization, and junk element repair.
//! - `discovery`: directory scanning, diagnostic reports, and coverage counting.
//! - `packaging`: `.qicons` bundle compression and cryptographic trailer signing.

pub(crate) use super::{id, manifest, pack, sha256};

pub mod discovery;
pub mod hygiene;
pub mod packaging;

pub use discovery::{discover_packs_detailed, discover_packs_in, inspect_pack_directory, CategoryCoverage, CoverageCount, DiscoveryError, DiscoveryReport, RejectedArchive, ValidationReport};
pub use hygiene::{
    clean_directory_icon, clean_directory_icons, clean_svg, cleanup_residual_tmp_files, directory_has_cleanable_icons, find_svg_junk_issues, is_cleaner_temp_file, parse_viewbox_values,
    validate_icon_svg, validate_icon_tokens, validate_svg, validate_svg_structural, CleanFileFailure, CleanIconResult, CleanPackReport,
};
pub use packaging::{package_bundle, package_bundle_to_bytes, package_bundle_to_writer, PackagedBundle};
