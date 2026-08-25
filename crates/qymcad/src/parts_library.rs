//! The library of parts — the catalogue: a built-in one (baked into the binary) plus one of the user's
//! own (in the data directory of the system), merged into a tree of categories. Folders are categories,
//! `.qpart` files are parts, `category.ron` holds the metadata of a folder.
//!
//! The scan and the building of the tree are testable headless (they do not depend on egui). The window
//! and the insertion come later.
// The model and the scan are ready and under tests; they are not wired into the interface yet (the window
// and the tree come later). Until then the public items are formally "unused" — the warning is silenced
// narrowly on this module.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use include_dir::{include_dir, Dir, DirEntry};
use qymcad_core::part::{CategoryMeta, PartManifest};

/// The built-in catalogue: baked into the binary at build time (`build.rs` watches `../../library`).
/// It works on every target (a portable .exe, an AppImage, macOS) — no external files are needed
/// alongside.
static EMBEDDED_PARTS: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/../../library/parts");

/// The bytes of a built-in `.qpart` by its path inside the catalogue (as in `PartSource::Embedded`).
/// `None` means there is no such file.
pub fn embedded_bytes(rel: &str) -> Option<&'static [u8]> {
    EMBEDDED_PARTS.get_file(rel).map(|f| f.contents())
}

/// The path to the user's own (read-write) catalogue of parts. PORTABLE MODE takes priority: if a
/// `library/` folder lies beside the executable, that is where the work goes (a self-contained .exe on a
/// stick, with the parts beside it rather than in the data directory of the system). Otherwise it is
/// `<system data dir>/library/parts` (`%APPDATA%\qymis\qym-cad\data\...` on Windows). `None` means
/// neither could be determined.
pub fn user_parts_dir() -> Option<PathBuf> {
    if let Some(p) = portable_parts_dir() {
        return Some(p);
    }
    directories::ProjectDirs::from("tech", "qymis", "qym-cad").map(|d| d.data_dir().join("library").join("parts"))
}

/// Whether portable mode is active (a `library/` folder beside the executable). For marking the root of
/// the user's own parts in the interface.
pub fn user_tier_is_portable() -> bool {
    portable_parts_dir().is_some()
}

/// A portable `library/parts` beside the executable — if a `library/` folder IS there (the marker of the
/// mode). `None` means not portable, or that the path to the executable could not be determined. In
/// development (`cargo run`, with the executable under `target/`) there is no `library/` beside it, so
/// `None`, so the ordinary data directory.
fn portable_parts_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    portable_dir_from(exe.parent()?)
}

/// The portable catalogue relative to the folder `base` (beside the executable): `base/library/parts`, if
/// `base/library/` exists. Split out of `portable_parts_dir` for a headless test (with no tie to
/// `current_exe`).
fn portable_dir_from(base: &Path) -> Option<PathBuf> {
    let lib = base.join("library");
    lib.is_dir().then(|| lib.join("parts"))
}

/// The relative paths of ALL the user's categories (folders) under `user_parts_dir`, for example
/// `["Fasteners", "Fasteners/Bolts", "Profiles"]` — for choosing a category quickly when saving something
/// as a part. FOLDERS specifically (not the display title from `category.ron`), because the file is
/// written to disk by them. Sorted.
pub fn user_category_paths() -> Vec<String> {
    match user_parts_dir() {
        Some(root) => category_paths_in(&root),
        None => Vec::new(),
    }
}

/// The `.qpart` file name from the name of a part — characters not allowed on Windows or across
/// platforms (`<>:"/\|?*` and control ones) become `_`; leading and trailing dots and spaces are trimmed;
/// an empty result becomes `part`.
pub fn sanitize_part_stem(name: &str) -> String {
    let s: String = name.chars().map(|c| if "<>:\"/\\|?*".contains(c) || c.is_control() { '_' } else { c }).collect();
    let s = s.trim().trim_matches('.').trim().to_string();
    if s.is_empty() { "part".into() } else { s }
}

/// The relative paths of the category folders under `root` (recursively, sorted). Split out of
/// `user_category_paths` for a headless test (with no tie to the data directory of the system).
fn category_paths_in(root: &Path) -> Vec<String> {
    fn walk(dir: &Path, prefix: &str, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if !p.is_dir() {
                continue;
            }
            let Some(name) = p.file_name().and_then(|s| s.to_str()) else { continue };
            let rel = if prefix.is_empty() { name.to_string() } else { format!("{prefix}/{name}") };
            walk(&p, &rel, out);
            out.push(rel);
        }
    }
    let mut out = Vec::new();
    walk(root, "", &mut out);
    out.sort();
    out
}

/// Where a part comes from: built in (read-only) or a file of the user's own on disk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PartSource {
    /// A path inside the built-in catalogue (relative to the root `library/parts`).
    Embedded(String),
    /// The absolute path of a `.qpart` on disk.
    User(PathBuf),
}

/// A leaf part in the tree of the catalogue. The manifest is loaded lazily (only `part.ron`, without
/// `document.ron`).
#[derive(Clone, Debug)]
pub struct PartEntry {
    /// The name from the manifest (or the file name as a stand-in, if the manifest does not read).
    pub name: String,
    /// The file name without the `.qpart` extension (the key for creating, reading and deleting).
    pub file_stem: String,
    pub source: PartSource,
    pub manifest: Option<PartManifest>,
}

/// A category node is a folder. It holds subcategories and parts.
#[derive(Clone, Debug, Default)]
pub struct CatNode {
    /// The name shown (`category.ron.title` or the name of the folder; for the roots, the built-in and
    /// the user's own captions).
    pub title: String,
    /// The order among siblings (from `category.ron`); equal ones go alphabetically by `title`.
    pub order: i32,
    /// The `ph::*` icon of the node (from `category.ron`), without the prefix.
    pub icon: Option<String>,
    pub subcats: Vec<CatNode>,
    pub parts: Vec<PartEntry>,
}

impl CatNode {
    /// The total number of parts in the subtree (for badges and empty states).
    pub fn total_parts(&self) -> usize {
        self.parts.len() + self.subcats.iter().map(CatNode::total_parts).sum::<usize>()
    }

    fn sort_recursive(&mut self) {
        self.subcats.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.title.cmp(&b.title)));
        self.parts.sort_by(|a, b| a.name.cmp(&b.name));
        for c in &mut self.subcats {
            c.sort_recursive();
        }
    }
}

/// The merged tree of the catalogue: two roots (the built-in one and the user's own). Clashes of names
/// between the tiers are NOT merged — each stays under its own root.
#[derive(Clone, Debug, Default)]
pub struct LibraryTree {
    pub embedded: CatNode,
    pub user: CatNode,
}

impl LibraryTree {
    /// Build the tree: the built-in catalogue from the baked-in bytes plus a scan of the user's data
    /// directory.
    pub fn load() -> Self {
        let mut embedded = build_embedded(&EMBEDDED_PARTS, &crate::i18n::tr("pl-builtin"));
        embedded.sort_recursive();

        // Portable mode (a `library/` folder beside the executable) — the root is marked so that where
        // the parts are written is visible.
        let user_title = if user_tier_is_portable() { crate::i18n::tr("pl-mine-portable") } else { crate::i18n::tr("pl-mine") };
        let mut user = match user_parts_dir() {
            Some(dir) => build_user(&dir, &user_title),
            None => CatNode { title: user_title, ..Default::default() },
        };
        user.sort_recursive();

        LibraryTree { embedded, user }
    }
}

/// Parse `category.ron` (as bytes) into metadata; on an error, the default.
fn parse_category_meta(bytes: &[u8]) -> CategoryMeta {
    std::str::from_utf8(bytes).ok().and_then(|s| ron::from_str::<CategoryMeta>(s).ok()).unwrap_or_default()
}

/// Apply the metadata of `category.ron` to a node (empty fields mean "not set" and what was there is
/// kept).
fn apply_meta(node: &mut CatNode, meta: CategoryMeta) {
    // THE KEY OUTRANKS A READY-MADE STRING: the built-in library must speak the language of the
    // interface. If there is no key, or it does not translate, `title` remains, and behind it the name of
    // the folder: that is exactly what a category of the user's own should be called — a person wrote its
    // name and it must not be translated.
    if !meta.title_key.is_empty() {
        let s = crate::i18n::tr(&meta.title_key);
        if s != meta.title_key {
            node.title = s;
        } else if !meta.title.is_empty() {
            node.title = meta.title;
        }
    } else if !meta.title.is_empty() {
        node.title = meta.title;
    }
    node.order = meta.order;
    if !meta.icon.is_empty() {
        node.icon = Some(meta.icon);
    }
}

fn is_qpart(name: &str) -> bool {
    name.rsplit('.').next().map(|e| e.eq_ignore_ascii_case("qpart")).unwrap_or(false)
}

fn folder_name(path: &Path) -> String {
    path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string()
}

/// Build the node of a built-in category from a baked-in folder. `title_override` is for the root.
fn build_embedded(dir: &Dir, title_override: &str) -> CatNode {
    let mut node = CatNode { title: title_override.to_string(), ..Default::default() };
    if title_override.is_empty() {
        node.title = folder_name(dir.path());
    }
    for e in dir.entries() {
        match e {
            DirEntry::Dir(d) => node.subcats.push(build_embedded(d, "")),
            DirEntry::File(f) => {
                let fname = f.path().file_name().and_then(|s| s.to_str()).unwrap_or("");
                if fname.eq_ignore_ascii_case("category.ron") {
                    apply_meta(&mut node, parse_category_meta(f.contents()));
                } else if is_qpart(fname) {
                    let path = f.path().to_string_lossy().to_string();
                    let manifest = qymcad_io::load_part_manifest_bytes(f.contents()).ok();
                    let stem = fname.rsplit_once('.').map(|(s, _)| s).unwrap_or(fname).to_string();
                    node.parts.push(PartEntry {
                        name: manifest.as_ref().map(|m| m.name.clone()).unwrap_or_else(|| stem.clone()),
                        file_stem: stem,
                        source: PartSource::Embedded(path),
                        manifest,
                    });
                }
            }
        }
    }
    node
}

/// Build the node of a user category by scanning a folder on disk. `title_override` is for the root.
fn build_user(dir: &Path, title_override: &str) -> CatNode {
    let mut node = CatNode {
        title: if title_override.is_empty() { folder_name(dir) } else { title_override.to_string() },
        ..Default::default()
    };
    // the metadata of THIS folder
    if let Ok(bytes) = std::fs::read(dir.join("category.ron")) {
        apply_meta(&mut node, parse_category_meta(&bytes));
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return node };
    for entry in rd.flatten() {
        let path = entry.path();
        if path.is_dir() {
            node.subcats.push(build_user(&path, ""));
        } else if path.extension().and_then(|s| s.to_str()).map(|e| e.eq_ignore_ascii_case("qpart")).unwrap_or(false) {
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
            let manifest = qymcad_io::load_part_manifest(&path.to_string_lossy()).ok();
            node.parts.push(PartEntry {
                name: manifest.as_ref().map(|m| m.name.clone()).unwrap_or_else(|| stem.clone()),
                file_stem: stem,
                source: PartSource::User(path),
                manifest,
            });
        }
    }
    node
}

#[cfg(test)]
mod tests {
    use super::*;
    use qymcad_core::geom::{Contour, Point2};
    use qymcad_core::model::Project;

    // A miniature part on disk: a square extruded, packed into a `.qpart`.
    fn write_qpart(path: &Path, name: &str) {
        let mut p = Project::default();
        let part = p.new_document();
        let sq = vec![Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), Point2::new(10.0, 10.0), Point2::new(0.0, 10.0)];
        let sk = p.add_sketch("s", vec![Contour::closed(sq)], None);
        p.add_sketch_node(sk, "Sketch");
        p.add_extrude(sk, 10.0);
        let sub = p.subproject_of(part).unwrap();
        let man = PartManifest::new(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        qymcad_io::save_part(&sub, &man, &[], None, &path.to_string_lossy()).unwrap();
    }

    fn temp_root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("qym_lib_test_{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn embedded_catalog_builds_without_panic() {
        // the root of the built-in catalogue exists (`library/parts` is baked in) and nothing panics
        let mut node = build_embedded(&EMBEDDED_PARTS, "Built-in");
        node.sort_recursive();
        assert_eq!(node.title, "Built-in");
        // THE NAME OF A CATEGORY IS TRANSLATED NOW, so it must not be compared with a literal word: the
        // test would turn red when a neighbouring test changed the language. The caption is asked of the
        // catalogue, by the same key `category.ron` names.
        let want = crate::i18n::tr("lib-cat-profiles");
        assert!(node.subcats.iter().any(|c| c.title == want), "the category \"{want}\" was not assembled: {:?}", node.subcats.iter().map(|c| &c.title).collect::<Vec<_>>());
        assert!(node.subcats.len() >= 3, "there must be at least three built-in categories: {:?}", node.subcats.iter().map(|c| &c.title).collect::<Vec<_>>());
    }

    #[test]
    fn embedded_catalog_contains_shipped_parts() {
        // The standard parts shipped in `library/parts` really do land in the binary (include_dir).
        // This is also a rebuild guard: editing this file forces include_dir to re-bake when a `.qpart` is
        // added.
        let mut node = build_embedded(&EMBEDDED_PARTS, "Built-in");
        node.sort_recursive();
        assert!(node.total_parts() >= 2, "the built-in parts are baked in: {}", node.total_parts());
        // the manifests of the baked-in `.qpart` files read (the name comes from `part.ron`, not from the
        // file name as a stand-in)
        let names: Vec<String> = {
            fn collect(n: &CatNode, out: &mut Vec<String>) {
                out.extend(n.parts.iter().map(|p| p.name.clone()));
                n.subcats.iter().for_each(|c| collect(c, out));
            }
            let mut v = Vec::new();
            collect(&node, &mut v);
            v
        };
        assert!(names.iter().any(|n| n.contains("Type-C")), "the manifest of the Type-C charger was read: {names:?}");
    }

    #[test]
    fn user_scan_builds_category_tree_with_parts() {
        let root = temp_root("scan");
        write_qpart(&root.join("Profiles").join("profile_2020.qpart"), "Profile 20x20");
        write_qpart(&root.join("Fasteners").join("Bolts").join("bolt_m8.qpart"), "Bolt M8");
        // a `category.ron` with an order and a name
        std::fs::write(root.join("Profiles").join("category.ron"), "(title: \"Profiles\", order: 1, icon: \"cube\")".as_bytes()).unwrap();

        let mut node = build_user(&root, "My parts");
        node.sort_recursive();

        assert_eq!(node.total_parts(), 2, "two parts in the subtree");
        let profiles = node.subcats.iter().find(|c| c.title == "Profiles").expect("the Profiles category");
        assert_eq!(profiles.order, 1, "the order from category.ron");
        assert_eq!(profiles.icon.as_deref(), Some("cube"));
        assert_eq!(profiles.parts.len(), 1);
        assert_eq!(profiles.parts[0].name, "Profile 20x20", "the name from the manifest");
        // the nested subcategory Fasteners/Bolts
        let fasteners = node.subcats.iter().find(|c| c.title == "Fasteners").expect("the Fasteners category");
        let bolts = fasteners.subcats.iter().find(|c| c.title == "Bolts").expect("the Bolts subcategory");
        assert_eq!(bolts.parts.len(), 1);
        assert_eq!(bolts.parts[0].name, "Bolt M8");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_user_dir_yields_empty_node() {
        let root = temp_root("missing").join("nope");
        let node = build_user(&root, "My parts");
        assert_eq!(node.total_parts(), 0);
        assert!(node.subcats.is_empty());
    }

    #[test]
    fn portable_dir_detected_by_library_folder_next_to_exe() {
        // Portable mode: a `library/` folder beside the executable puts the parts there
        // (`base/library/parts`), otherwise `None`.
        let root = temp_root("portable");
        std::fs::create_dir_all(&root).unwrap();
        assert_eq!(portable_dir_from(&root), None, "no library/ beside it, so not portable");
        std::fs::create_dir_all(root.join("library")).unwrap();
        assert_eq!(portable_dir_from(&root), Some(root.join("library").join("parts")), "library/ is there, so portable = library/parts");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn sanitize_stem_handles_invalid_and_empty() {
        // The non-ASCII name is deliberate test data: valid characters, whatever alphabet they are
        // written in, must pass through untouched — only the separators get cleaned.
        assert_eq!(sanitize_part_stem("Профиль 20×20"), "Профиль 20×20");
        assert_eq!(sanitize_part_stem("a/b:c*?"), "a_b_c__"); // the disallowed ones become _
        assert_eq!(sanitize_part_stem("  ...  "), "part"); // empty after trimming, so the default
        assert_eq!(sanitize_part_stem(""), "part");
    }

    #[test]
    fn category_paths_lists_nested_folders_relative() {
        // The category picker used when saving something as a part — the relative paths of ALL the
        // folders (for writing to disk).
        let root = temp_root("catpaths");
        std::fs::create_dir_all(root.join("Profiles").join("Aluminium")).unwrap();
        std::fs::create_dir_all(root.join("Fasteners").join("Bolts")).unwrap();
        // a `.qpart` file in a folder does not count as a category (only folders do)
        write_qpart(&root.join("Profiles").join("p.qpart"), "P");
        let paths = category_paths_in(&root);
        assert_eq!(paths, vec!["Fasteners", "Fasteners/Bolts", "Profiles", "Profiles/Aluminium"], "the relative paths of the folders, sorted");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn categories_sorted_by_order_then_title() {
        let root = temp_root("sort");
        for (cat, ord) in [("Alpha", 3), ("Beta", 1), ("Gamma", 1)] {
            std::fs::create_dir_all(root.join(cat)).unwrap();
            std::fs::write(root.join(cat).join("category.ron"), format!("(order: {ord})").as_bytes()).unwrap();
        }
        let mut node = build_user(&root, "root");
        node.sort_recursive();
        let titles: Vec<&str> = node.subcats.iter().map(|c| c.title.as_str()).collect();
        // order 1 (Beta and Gamma alphabetically), then order 3 (Alpha)
        assert_eq!(titles, vec!["Beta", "Gamma", "Alpha"]);
        let _ = std::fs::remove_dir_all(&root);
    }
}

/// THE BUILT-IN LIBRARY SPEAKS THE LANGUAGE OF THE INTERFACE.
///
/// The names of the categories arrive AS DATA out of the manifests, so the guard against a catalogue key
/// reaching the screen does not see them: it checks the language catalogue, and these are files of the
/// library. In an English interface the categories stood in another language — and only an eye could
/// notice that.
///
/// The names of the PARTS themselves are deliberately not translated: "Type-C charger" is the name of a
/// thing, not a word.
#[cfg(test)]
mod i18n_tests {
    use super::*;

    /// Every built-in category except the root, as a flat list.
    fn embedded_titles() -> Vec<String> {
        fn walk(n: &CatNode, out: &mut Vec<String>) {
            for c in &n.subcats {
                out.push(c.title.clone());
                walk(c, out);
            }
        }
        let mut out = Vec::new();
        walk(&LibraryTree::load().embedded, &mut out);
        out
    }

    /// NOT ONE WORD OF ANOTHER LANGUAGE IN THE ENGLISH INTERFACE.
    ///
    /// The check goes by the alphabet rather than by a list of words: a list catches the names it was
    /// told about and stays silent about what is added tomorrow. Cyrillic in an English build is evidence
    /// in itself.
    #[test]
    fn the_builtin_library_speaks_the_interface_language() {
        let prev = crate::i18n::language();
        crate::i18n::set_language("en");
        let titles = embedded_titles();
        crate::i18n::set_language(&prev);
        assert!(titles.len() >= 3, "suspiciously few built-in categories were found: {titles:?}");
        for t in &titles {
            assert!(!t.chars().any(|c| ('а'..='я').contains(&c.to_lowercase().next().unwrap_or(c))), "in the English interface the category \"{t}\" stayed in another language");
        }
    }

    /// AND IN THE OTHER LANGUAGE THEY ARE WORDS TOO, not keys.
    #[test]
    fn and_in_russian_they_are_words_not_keys() {
        let prev = crate::i18n::language();
        crate::i18n::set_language("ru");
        let titles = embedded_titles();
        crate::i18n::set_language(&prev);
        for t in &titles {
            assert!(!t.starts_with("lib-cat-"), "a category showed up as a catalogue key: \"{t}\"");
            assert!(!t.trim().is_empty(), "a category has an empty name");
        }
    }

    /// THE NAME OF A USER CATEGORY IS NOT TOUCHED.
    ///
    /// The essential second half: "translate everything" would break somebody's library — their folder is
    /// called what they called it, and substituting that is not allowed.
    #[test]
    fn a_user_category_keeps_its_own_name() {
        let mut node = CatNode { title: "My brackets".into(), ..Default::default() };
        // there is no manifest, so the name stays as it is
        apply_meta(&mut node, CategoryMeta::default());
        assert_eq!(node.title, "My brackets", "the name of a user category was substituted");
        // and even with a `title` in the manifest but no key, it is that title that is taken
        let mut node2 = CatNode::default();
        apply_meta(&mut node2, CategoryMeta { title: "Fasteners".into(), ..Default::default() });
        assert_eq!(node2.title, "Fasteners", "the name from the user's manifest was not applied");
    }
}
