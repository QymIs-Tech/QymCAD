// Build script for qymcad-ui-state:
// 1. Reads the clean `IconId` enum variants from `src/icons/id.rs`.
// 2. Maps each variant to its canonical `icons/<category>/<name>.svg` path.
// 3. Validates all base icons in `assets/icon-themes/shapr-alike/` (SVG validity, viewBox, no raster images).
// 4. Discovers all icon theme folders in `assets/icon-themes/` containing `manifest.ron`.
// 5. Packages all discovered themes into compressed `.qicons` bundles written to `OUT_DIR/<theme>.qicons`.
// 6. Generates `OUT_DIR/icon_generated.rs` implementing `relative_path(&self)`, `from_id_str(s)`, `ALL_ICONS`,
//    `DEFAULT_THEME_ID`, and `BUILTIN_ICON_THEMES`.

use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[allow(dead_code)]
#[path = "src/icons/sha256.rs"]
mod sha256;

fn variant_to_relative_path(var: &str) -> String {
    if var == "SketchPickPlane" {
        return "datum/sketch_pick_plane".to_string();
    }
    if var == "SketchCircle3Pt" {
        return "sketch/circle_3pt".to_string();
    }
    for cat in &["Sketch", "Constraint", "Datum", "Part", "Assembly"] {
        if let Some(rest) = var.strip_prefix(cat) {
            let mut s = String::new();
            for (i, c) in rest.chars().enumerate() {
                if c.is_uppercase() && i > 0 {
                    s.push('_');
                }
                s.push(c.to_ascii_lowercase());
            }
            return format!("{}/{}", cat.to_ascii_lowercase(), s);
        }
    }
    panic!("Unknown icon variant category for: {}", var);
}

fn extract_manifest_id(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("id:") {
            let part = trimmed.strip_prefix("id:")?.trim();
            let id = part.trim_matches(|c| c == '"' || c == ',' || c == ' ');
            return Some(id.to_string());
        }
    }
    None
}

struct DiscoveredTheme {
    dir_name: String,
    dir_path: PathBuf,
    manifest_id: String,
}

fn package_theme_archive(theme_dir: &Path, out_archive: &Path) {
    let manifest_path = theme_dir.join("manifest.ron");
    let manifest_content = fs::read_to_string(&manifest_path).expect("manifest.ron reads");

    let out_file = fs::File::create(out_archive).expect("creates output zip in OUT_DIR");
    let mut zip = zip::ZipWriter::new(out_file);
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // Write manifest.ron
    zip.start_file("manifest.ron", options).expect("writes manifest into zip");
    zip.write_all(manifest_content.as_bytes()).expect("writes manifest bytes");

    // Write icon.svg if present
    let icon_path = theme_dir.join("icon.svg");
    if icon_path.is_file() {
        let pack_icon = fs::read(&icon_path).expect("pack icon reads");
        zip.start_file("icon.svg", options).expect("writes pack icon into zip");
        zip.write_all(&pack_icon).expect("writes pack icon bytes");
    }

    // Include the base and localized descriptions, license, preview in the embedded archive.
    if let Ok(entries) = fs::read_dir(theme_dir) {
        let mut readmes = Vec::new();
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                    if name == "README.md" || (name.starts_with("README.") && name.ends_with(".md")) || name == "description.md" || name.starts_with("LICENSE") || name.starts_with("preview.") {
                        readmes.push((name.to_string(), p));
                    }
                }
            }
        }
        readmes.sort_by(|left, right| left.0.cmp(&right.0));
        for (name, path) in readmes {
            let content = fs::read(&path).expect("theme file reads");
            zip.start_file(name, options).expect("writes localized readme into zip");
            zip.write_all(&content).expect("writes localized readme bytes");
        }
    }

    // Walk and write all SVGs in icons/
    let icons_dir = theme_dir.join("icons");
    if icons_dir.is_dir() {
        walk_dir(&icons_dir, &icons_dir, &mut zip, options);
    }

    zip.finish().expect("finishes zip archive");

    // Append cryptographic verification trailer to ensure archive has valid provenance
    let mut zip_bytes = fs::read(out_archive).expect("reads generated zip");
    sha256::append_qicons_trailer(&mut zip_bytes);
    fs::write(out_archive, zip_bytes).expect("writes trailer to archive");
}

fn walk_dir(base: &Path, current: &Path, zip: &mut zip::ZipWriter<fs::File>, options: zip::write::SimpleFileOptions) {
    let mut entries: Vec<_> = fs::read_dir(current).expect("reads dir").flatten().collect();
    entries.sort_by_key(|e| e.path());
    for entry in entries {
        let p = entry.path();
        if p.is_dir() {
            walk_dir(base, &p, zip, options);
        } else if p.extension().is_some_and(|e| e == "svg") {
            let rel = p.strip_prefix(base).expect("strip prefix");
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            let data = fs::read(&p).expect("read file");
            zip.start_file(format!("icons/{rel_str}"), options).expect("zip start file");
            zip.write_all(&data).expect("zip write file");
        }
    }
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let repo_root = manifest_dir.join("../..");
    let icon_themes_dir = repo_root.join("assets/icon-themes");

    // Cargo recursively scans directories specified in `rerun-if-changed`.
    // Watching the root `icon_themes_dir` ensures Cargo's dirty-tracker walks the entire
    // subtree and re-runs this build script whenever any nested SVG, manifest, or document changes.
    println!("cargo:rerun-if-changed={}", icon_themes_dir.display());

    if !icon_themes_dir.is_dir() {
        panic!("Icon themes root directory not found at: {}", icon_themes_dir.display());
    }

    // 1. Discover all theme subdirectories containing manifest.ron
    let mut theme_entries = Vec::new();
    for entry in fs::read_dir(&icon_themes_dir).expect("icon themes dir reads").flatten() {
        let p = entry.path();
        if p.is_dir() {
            let manifest_path = p.join("manifest.ron");
            if manifest_path.is_file() {
                println!("cargo:rerun-if-changed={}", p.display());
                let dir_name = entry.file_name().to_string_lossy().to_string();
                let manifest_content = fs::read_to_string(&manifest_path).expect("manifest.ron reads");
                let _: ron::Value = ron::from_str(&manifest_content).expect("manifest.ron is valid RON");
                let manifest_id = extract_manifest_id(&manifest_content).expect("manifest has id field");
                theme_entries.push(DiscoveredTheme { dir_name, dir_path: p, manifest_id });
            }
        }
    }

    // Sort themes: ensure "shapr-alike" is first, others sorted alphabetically by directory name
    theme_entries.sort_by(|a, b| {
        if a.manifest_id == "shapr-alike" {
            std::cmp::Ordering::Less
        } else if b.manifest_id == "shapr-alike" {
            std::cmp::Ordering::Greater
        } else {
            a.dir_name.cmp(&b.dir_name)
        }
    });

    let default_theme = theme_entries.iter().find(|t| t.manifest_id == "shapr-alike").or_else(|| theme_entries.first()).expect("At least one built-in icon theme must exist in assets/icon-themes");

    // 2. Extract variants from `src/icons/id.rs`
    let id_rs_path = manifest_dir.join("src/icons/id.rs");
    println!("cargo:rerun-if-changed={}", id_rs_path.display());
    let id_rs_content = fs::read_to_string(&id_rs_path).expect("id.rs reads");

    let mut in_enum = false;
    let mut variants = Vec::new();
    for line in id_rs_content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("pub enum IconId") {
            in_enum = true;
            continue;
        }
        if in_enum {
            if trimmed.starts_with('}') {
                break;
            }
            if trimmed.starts_with("//") || trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let var = trimmed.trim_end_matches(',').trim();
            if !var.is_empty() && var.chars().next().unwrap().is_ascii_uppercase() {
                variants.push(var.to_string());
            }
        }
    }

    if variants.is_empty() {
        panic!("Failed to parse IconId enum variants from id.rs");
    }

    // 3. Map variants to relative paths and check uniqueness
    let mut var_to_path = Vec::new();
    let mut path_to_var = HashMap::new();
    let mut expected_icons = HashSet::new();

    for v in &variants {
        let rel_path = variant_to_relative_path(v);
        var_to_path.push((v.clone(), rel_path.clone()));
        path_to_var.insert(rel_path.clone(), v.clone());
        expected_icons.insert(format!("{rel_path}.svg"));
    }

    // 4. Verify all expected default icons exist and pass SVG validation
    let default_icons_dir = default_theme.dir_path.join("icons");
    for rel_path in &expected_icons {
        let full_path = default_icons_dir.join(rel_path);
        if !full_path.is_file() {
            panic!("COMPILE ERROR: Missing default vector SVG icon: {}\nExpected at: {}", rel_path, full_path.display());
        }

        let svg_data = fs::read(&full_path).unwrap_or_else(|e| {
            panic!("Failed to read {}: {e}", full_path.display());
        });

        let text = match std::str::from_utf8(&svg_data) {
            Ok(t) => t,
            Err(_) => panic!("SVG data in {} is not valid UTF-8", full_path.display()),
        };

        if !text.contains("<svg") {
            panic!("Missing <svg> root element in {}", full_path.display());
        }
        if !text.contains("viewBox") {
            panic!("Missing viewBox attribute in {}", full_path.display());
        }
        if text.contains("<image") || text.contains("data:image/") {
            panic!("Prohibited raster image found in {}", full_path.display());
        }
    }

    // 5. Generate OUT_DIR/icon_generated.rs
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let mut gen = String::new();
    gen.push_str("// Generated by build.rs. Do not edit directly.\n\n");
    gen.push_str("impl IconId {\n");
    gen.push_str("    /// Relative path inside the `icons/` folder, without `.svg` extension.\n");
    gen.push_str("    pub fn relative_path(&self) -> &'static str {\n");
    gen.push_str("        match self {\n");
    for (v, p) in &var_to_path {
        gen.push_str(&format!("            Self::{v} => \"{p}\",\n"));
    }
    gen.push_str("        }\n");
    gen.push_str("    }\n\n");
    gen.push_str("    /// Canonical egui image URI for this icon.\n");
    gen.push_str("    pub fn uri(&self) -> &'static str {\n");
    gen.push_str("        match self {\n");
    for (v, p) in &var_to_path {
        gen.push_str(&format!("            Self::{v} => \"bytes://qicons/{p}.svg\",\n"));
    }
    gen.push_str("        }\n");
    gen.push_str("    }\n\n");

    gen.push_str("    /// Resolve an `IconId` from a relative path or dotted identifier.\n");
    gen.push_str("    pub fn from_id_str(s: &str) -> Option<Self> {\n");
    gen.push_str("        let clean = s.trim().trim_end_matches(\".svg\").replace('.', \"/\");\n");
    gen.push_str("        match clean.as_str() {\n");
    for (v, p) in &var_to_path {
        gen.push_str(&format!("            \"{p}\" => Some(Self::{v}),\n"));
    }
    gen.push_str("            _ => None,\n");
    gen.push_str("        }\n");
    gen.push_str("    }\n");
    gen.push_str("}\n\n");

    gen.push_str("/// All known `IconId` values. Generated automatically by build script.\n");
    gen.push_str("pub const ALL_ICONS: &[IconId] = &[\n");
    for v in &variants {
        gen.push_str(&format!("    IconId::{v},\n"));
    }
    gen.push_str("];\n\n");

    gen.push_str("/// The default built-in icon theme ID.\n");
    gen.push_str(&format!("pub const DEFAULT_THEME_ID: &str = \"{}\";\n\n", default_theme.manifest_id));

    gen.push_str("/// All built-in icon themes embedded into the binary.\n");
    gen.push_str("pub const BUILTIN_ICON_THEMES: &[BuiltinTheme] = &[\n");
    for theme in &theme_entries {
        gen.push_str(&format!("    BuiltinTheme {{ id: \"{}\", archive: include_bytes!(concat!(env!(\"OUT_DIR\"), \"/{}.qicons\")) }},\n", theme.manifest_id, theme.dir_name));
    }
    gen.push_str("];\n");

    let gen_path = out_dir.join("icon_generated.rs");
    fs::write(&gen_path, gen).expect("writes icon_generated.rs");

    // 6. Package all discovered themes into OUT_DIR/<theme_dir_name>.qicons
    for theme in &theme_entries {
        let out_archive = out_dir.join(format!("{}.qicons", theme.dir_name));
        package_theme_archive(&theme.dir_path, &out_archive);
    }

    println!("cargo:info=Successfully validated {} default icons and packaged {} built-in themes", variants.len(), theme_entries.len());
}
