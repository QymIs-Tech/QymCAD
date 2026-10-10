//! THEME DISCOVERY AND DIRECTORY SCANNING.
//!
//! Locates user and bundled icon themes across platform search directories,
//! computes content signatures, and manages the theme discovery cache.

use qymcad_ui_state::icons::{discover_packs_detailed, load_builtin_packs, DiscoveryError, DuplicateConflict, IconPack, DEFAULT_THEME_ID};
use std::path::{Path, PathBuf};

/// The directory for user-installed icon themes in the application config folder.
pub(crate) fn user_themes_dir() -> Option<PathBuf> {
    qymcad_paths::config("icon_themes")
}

/// The directory for bundled icon themes.
pub(crate) fn bundled_themes_dir() -> PathBuf {
    static BUNDLED: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    BUNDLED.get_or_init(|| bundled_themes_dir_with(Some(Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/icon-themes"))), std::env::current_exe().ok().as_deref())).clone()
}

pub(crate) fn bundled_themes_dir_with(dev_candidate: Option<&Path>, exe_path: Option<&Path>) -> PathBuf {
    if let Some(dev) = dev_candidate {
        if dev.exists() {
            return dev.to_path_buf();
        }
    }
    if let Some(exe) = exe_path {
        if let Some(parent) = exe.parent() {
            let beside = parent.join("assets/icon-themes");
            if beside.exists() {
                return beside;
            }
            if let Some(contents) = parent.parent() {
                let mac_resources = contents.join("Resources/assets/icon-themes");
                if mac_resources.exists() {
                    return mac_resources;
                }
                let mac_resources_flat = contents.join("Resources/icon-themes");
                if mac_resources_flat.exists() {
                    return mac_resources_flat;
                }
            }
        }
    }
    PathBuf::from("assets/icon-themes")
}

/// All search directories for icon themes (user directories followed by bundled directory).
pub(crate) fn all_theme_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(user_dir) = user_themes_dir() {
        if !dirs.contains(&user_dir) {
            dirs.push(user_dir);
        }
    }
    if let Some(data_dir) = qymcad_paths::data("icon_themes") {
        if !dirs.contains(&data_dir) {
            dirs.push(data_dir);
        }
    }
    if let Some(d) = qymcad_paths::dirs() {
        let xdg_data = d.data_dir().join("icon_themes");
        if !dirs.contains(&xdg_data) {
            dirs.push(xdg_data);
        }
        let xdg_config = d.config_dir().join("icon_themes");
        if !dirs.contains(&xdg_config) {
            dirs.push(xdg_config);
        }
    }
    let bundled = bundled_themes_dir();
    if !dirs.contains(&bundled) {
        dirs.push(bundled);
    }
    dirs
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ThemeEntrySignature {
    pub path: PathBuf,
    pub modified: Option<std::time::SystemTime>,
    pub len: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ThemeDirSignature {
    pub dir: PathBuf,
    pub entries: Vec<ThemeEntrySignature>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct DiscoveredThemes {
    pub packs: Vec<IconPack>,
    pub errors: Vec<DiscoveryError>,
}

#[derive(Clone, Debug)]
struct CachedDiscovery {
    signatures: Vec<ThemeDirSignature>,
    #[cfg(test)]
    themes: DiscoveredThemes,
    all_themes: Option<DiscoveredThemes>,
}

static DISCOVERY_CACHE: std::sync::RwLock<Option<std::collections::HashMap<Vec<PathBuf>, CachedDiscovery>>> = std::sync::RwLock::new(None);
static DISCOVERY_WORKER_SIGNAL: std::sync::RwLock<Option<std::sync::Arc<std::sync::atomic::AtomicBool>>> = std::sync::RwLock::new(None);

#[cfg(test)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct DiscoveryCacheStats {
    pub hits: usize,
    pub misses: usize,
}

#[cfg(test)]
static DISCOVERY_HITS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
#[cfg(test)]
static DISCOVERY_MISSES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

#[cfg(test)]
pub(crate) fn discovery_cache_stats() -> DiscoveryCacheStats {
    DiscoveryCacheStats { hits: DISCOVERY_HITS.load(std::sync::atomic::Ordering::Relaxed), misses: DISCOVERY_MISSES.load(std::sync::atomic::Ordering::Relaxed) }
}

#[cfg(test)]
pub(crate) fn clear_discovery_cache_for_test() {
    if let Ok(mut guard) = DISCOVERY_CACHE.write() {
        if let Some(map) = guard.as_mut() {
            map.clear();
        }
    }
    DISCOVERY_HITS.store(0, std::sync::atomic::Ordering::Relaxed);
    DISCOVERY_MISSES.store(0, std::sync::atomic::Ordering::Relaxed);
}

/// Invalidate cached discovery results across all searched theme directories.
pub(crate) fn invalidate_theme_discovery_cache() {
    if let Ok(mut guard) = DISCOVERY_CACHE.write() {
        if let Some(map) = guard.as_mut() {
            map.clear();
        }
    }
}

pub(crate) fn compute_dir_signature(dir: &Path) -> ThemeDirSignature {
    let mut entries = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for entry in rd.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let manifest_path = path.join("manifest.ron");
                if let Ok(meta) = std::fs::metadata(&manifest_path) {
                    entries.push(ThemeEntrySignature { path: manifest_path, modified: meta.modified().ok(), len: meta.len() });
                }
            } else if path.extension().is_some_and(|ext| ext == "qicons" || ext == "zip") {
                if let Ok(meta) = std::fs::metadata(&path) {
                    entries.push(ThemeEntrySignature { path, modified: meta.modified().ok(), len: meta.len() });
                }
            }
        }
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    ThemeDirSignature { dir: dir.to_path_buf(), entries }
}

#[cfg(test)]
pub(crate) fn discover_theme_packs_in_dirs(dirs: &[PathBuf]) -> DiscoveredThemes {
    if let Ok(guard) = DISCOVERY_CACHE.read() {
        if let Some(map) = guard.as_ref() {
            if let Some(cached) = map.get(dirs) {
                #[cfg(test)]
                DISCOVERY_HITS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                return cached.themes.clone();
            }
        }
    }

    #[cfg(test)]
    DISCOVERY_MISSES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    let current_signatures: Vec<ThemeDirSignature> = dirs.iter().map(|d| compute_dir_signature(d)).collect();
    let mut packs = Vec::new();
    let mut errors = Vec::new();
    for dir in dirs {
        let report = discover_packs_detailed(dir);
        packs.extend(report.packs);
        errors.extend(report.errors);
    }

    let themes = DiscoveredThemes { packs, errors };

    if let Ok(mut guard) = DISCOVERY_CACHE.write() {
        let map = guard.get_or_insert_with(std::collections::HashMap::new);
        map.insert(
            dirs.to_vec(),
            CachedDiscovery {
                signatures: current_signatures,
                #[cfg(test)]
                themes: themes.clone(),
                all_themes: None,
            },
        );
    }

    themes
}

pub(crate) fn is_pack_from_bundled_dir(pack: &IconPack, bundled_dir: &Path) -> bool {
    let Some(source_path) = pack.source_path() else {
        return false;
    };
    if source_path.starts_with(bundled_dir) {
        return true;
    }
    static CANONICAL_BUNDLED: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    let canon_bundled = CANONICAL_BUNDLED.get_or_init(|| std::fs::canonicalize(bundled_dir).ok());
    if let (Ok(canon_pack), Some(canon_bundled)) = (std::fs::canonicalize(source_path), canon_bundled.as_deref()) {
        return canon_pack.starts_with(canon_bundled);
    }
    false
}

fn assemble_all_themes(discovered: DiscoveredThemes, bundled_dir: &Path) -> DiscoveredThemes {
    let builtin_packs = load_builtin_packs();
    let mut packs = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();

    for pack in discovered.packs {
        let is_bundled = is_pack_from_bundled_dir(&pack, bundled_dir);
        if pack.manifest.id == DEFAULT_THEME_ID {
            if is_bundled {
                if let Some(builtin) = builtin_packs.iter().find(|b| b.manifest.id == DEFAULT_THEME_ID) {
                    if seen_ids.insert(DEFAULT_THEME_ID.to_string()) {
                        packs.push(builtin.clone());
                    }
                } else if seen_ids.insert(DEFAULT_THEME_ID.to_string()) {
                    packs.push(pack);
                }
            } else {
                let mut conflict_pack = pack;
                conflict_pack.duplicate_conflict = Some(DuplicateConflict { conflicting_id: DEFAULT_THEME_ID.to_string(), is_default_theme: true });
                packs.push(conflict_pack);
            }
        } else if let Some(builtin) = builtin_packs.iter().find(|b| b.manifest.id == pack.manifest.id) {
            if is_bundled {
                if seen_ids.insert(builtin.manifest.id.clone()) {
                    packs.push(builtin.clone());
                }
            } else if seen_ids.insert(pack.manifest.id.clone()) {
                packs.push(pack);
            } else {
                let mut conflict_pack = pack;
                conflict_pack.duplicate_conflict = Some(DuplicateConflict { conflicting_id: conflict_pack.manifest.id.clone(), is_default_theme: false });
                packs.push(conflict_pack);
            }
        } else if seen_ids.insert(pack.manifest.id.clone()) {
            packs.push(pack);
        } else {
            let mut conflict_pack = pack;
            conflict_pack.duplicate_conflict = Some(DuplicateConflict { conflicting_id: conflict_pack.manifest.id.clone(), is_default_theme: false });
            packs.push(conflict_pack);
        }
    }

    for builtin in builtin_packs {
        if seen_ids.insert(builtin.manifest.id.clone()) {
            packs.push(builtin.clone());
        }
    }

    DiscoveredThemes { packs, errors: discovered.errors }
}

/// Polls directory signatures in the background thread.
/// Returns true and updates cache if any directory signature changed on disk.
pub(crate) fn poll_theme_discovery(dirs: &[PathBuf]) -> bool {
    let current_signatures: Vec<ThemeDirSignature> = dirs.iter().map(|d| compute_dir_signature(d)).collect();

    let unchanged = if let Ok(guard) = DISCOVERY_CACHE.read() {
        guard.as_ref().and_then(|map| map.get(dirs)).is_some_and(|c| c.signatures == current_signatures && c.all_themes.is_some())
    } else {
        false
    };

    if unchanged {
        return false;
    }

    let mut packs = Vec::new();
    let mut errors = Vec::new();
    for dir in dirs {
        let report = discover_packs_detailed(dir);
        packs.extend(report.packs);
        errors.extend(report.errors);
    }
    let themes = DiscoveredThemes { packs, errors };
    let bundled_dir = bundled_themes_dir();
    let all_themes = assemble_all_themes(themes.clone(), &bundled_dir);

    if let Ok(mut guard) = DISCOVERY_CACHE.write() {
        let map = guard.get_or_insert_with(std::collections::HashMap::new);
        map.insert(
            dirs.to_vec(),
            CachedDiscovery {
                signatures: current_signatures,
                #[cfg(test)]
                themes,
                all_themes: Some(all_themes),
            },
        );
    }

    true
}

/// Ensure background discovery watcher thread is running to check theme directories periodically (1s).
pub(crate) fn ensure_discovery_worker(ctx: &egui::Context, dirs: &[PathBuf]) {
    let thread_flag_id = egui::Id::new("theme_discovery_worker_running");
    let needs_spawn = ctx.data(|d| match d.get_temp::<std::sync::Arc<std::sync::atomic::AtomicBool>>(thread_flag_id) {
        Some(flag) => !flag.load(std::sync::atomic::Ordering::SeqCst),
        None => true,
    });

    if needs_spawn {
        let flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let flag_clone = std::sync::Arc::clone(&flag);
        if let Ok(mut lock) = DISCOVERY_WORKER_SIGNAL.write() {
            *lock = Some(std::sync::Arc::clone(&flag));
        }
        ctx.data_mut(|d| d.insert_temp(thread_flag_id, flag));

        let thread_ctx = ctx.clone();
        let thread_dirs = dirs.to_vec();
        let _ = std::thread::Builder::new().name("theme-discovery-watcher".to_string()).spawn(move || {
            while flag_clone.load(std::sync::atomic::Ordering::SeqCst) {
                std::thread::sleep(std::time::Duration::from_millis(1000));
                if !flag_clone.load(std::sync::atomic::Ordering::SeqCst) {
                    break;
                }
                if poll_theme_discovery(&thread_dirs) {
                    thread_ctx.request_repaint();
                }
            }
            flag_clone.store(false, std::sync::atomic::Ordering::SeqCst);
        });
    }
}

/// Stop background discovery watcher thread.
pub(crate) fn stop_discovery_worker(ctx: &egui::Context) {
    let thread_flag_id = egui::Id::new("theme_discovery_worker_running");
    ctx.data(|d| {
        if let Some(flag) = d.get_temp::<std::sync::Arc<std::sync::atomic::AtomicBool>>(thread_flag_id) {
            flag.store(false, std::sync::atomic::Ordering::SeqCst);
        }
    });
    if let Ok(mut lock) = DISCOVERY_WORKER_SIGNAL.write() {
        if let Some(flag) = lock.take() {
            flag.store(false, std::sync::atomic::Ordering::SeqCst);
        }
    }
}

/// Stop any running discovery watcher thread globally without requiring an egui::Context.
pub(crate) fn stop_all_discovery_workers() {
    if let Ok(mut lock) = DISCOVERY_WORKER_SIGNAL.write() {
        if let Some(flag) = lock.take() {
            flag.store(false, std::sync::atomic::Ordering::SeqCst);
        }
    }
}

/// Discovers all icon theme packs across given directories.
/// Fast path: reads directly from in-memory cache with zero filesystem IO on the UI thread.
/// Updates from disk are handled asynchronously by the background discovery worker.
pub(crate) fn discover_all_theme_packs(dirs: &[PathBuf]) -> DiscoveredThemes {
    if let Ok(guard) = DISCOVERY_CACHE.read() {
        if let Some(map) = guard.as_ref() {
            if let Some(cached) = map.get(dirs) {
                if let Some(all) = &cached.all_themes {
                    return all.clone();
                }
            }
        }
    }

    let current_signatures: Vec<ThemeDirSignature> = dirs.iter().map(|d| compute_dir_signature(d)).collect();
    let mut packs = Vec::new();
    let mut errors = Vec::new();
    for dir in dirs {
        let report = discover_packs_detailed(dir);
        packs.extend(report.packs);
        errors.extend(report.errors);
    }
    let themes = DiscoveredThemes { packs, errors };
    let bundled_dir = bundled_themes_dir();
    let all_themes = assemble_all_themes(themes.clone(), &bundled_dir);

    if let Ok(mut guard) = DISCOVERY_CACHE.write() {
        let map = guard.get_or_insert_with(std::collections::HashMap::new);
        map.insert(
            dirs.to_vec(),
            CachedDiscovery {
                signatures: current_signatures,
                #[cfg(test)]
                themes,
                all_themes: Some(all_themes.clone()),
            },
        );
    }

    all_themes
}
