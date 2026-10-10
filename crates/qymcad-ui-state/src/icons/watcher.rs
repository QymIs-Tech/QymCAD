//! Live icon theme directory watching and background worker thread.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::id::IconId;
use super::manager::{bump_icon_revision, install_cad_icons, update_cad_icon, ActiveIconStack};
use super::pack::FileSignature;

/// State for live directory watching stored inside `egui::Context::data()`.
#[derive(Clone, Default)]
pub struct IconWatcher {
    pub dev_watch_enabled: bool,
    pub watched_pack_ids: Vec<String>,
    pub last_seen_snapshots: HashMap<String, HashMap<PathBuf, FileSignature>>,
    pub last_poll_time: Option<std::time::Instant>,
}

/// Synchronize watched pack IDs in `egui::Context`.
pub fn sync_watched_packs(ctx: &egui::Context, watched: &[String]) {
    ctx.data_mut(|d| {
        let id = egui::Id::new("cad_icon_watcher");
        let mut watcher = d.get_temp::<IconWatcher>(id).unwrap_or_default();
        watcher.watched_pack_ids = watched.to_vec();
        if let Some(stack) = d.get_temp::<ActiveIconStack>(egui::Id::new("cad_active_icon_stack")) {
            for pack_id in watched {
                if !watcher.last_seen_snapshots.contains_key(pack_id) {
                    if let Some(pack) = stack.0.iter().find(|p| &p.manifest.id == pack_id) {
                        if let Some(snap) = pack.directory_snapshot() {
                            watcher.last_seen_snapshots.insert(pack_id.clone(), snap);
                        }
                    }
                }
            }
        }
        d.insert_temp(id, watcher);
    });
}

/// Set global dev watch mode in `egui::Context`.
pub fn set_dev_watch(ctx: &egui::Context, enabled: bool) {
    ctx.data_mut(|d| {
        let id = egui::Id::new("cad_icon_watcher");
        let mut watcher = d.get_temp::<IconWatcher>(id).unwrap_or_default();
        watcher.dev_watch_enabled = enabled;
        d.insert_temp(id, watcher);
    });
}

/// Check if a specific pack is watched in `egui::Context`.
pub fn is_pack_watched(ctx: &egui::Context, pack_id: &str) -> bool {
    ctx.data(|d| d.get_temp::<IconWatcher>(egui::Id::new("cad_icon_watcher")).map(|w| w.dev_watch_enabled || w.watched_pack_ids.iter().any(|id| id == pack_id)).unwrap_or(false))
}

/// Set watch state for a pack in `egui::Context`.
pub fn set_pack_watching(ctx: &egui::Context, pack_id: &str, watch: bool) {
    ctx.data_mut(|d| {
        let id = egui::Id::new("cad_icon_watcher");
        let mut watcher = d.get_temp::<IconWatcher>(id).unwrap_or_default();
        if watch {
            if !watcher.watched_pack_ids.iter().any(|id| id == pack_id) {
                watcher.watched_pack_ids.push(pack_id.to_string());
            }
            if let Some(stack) = d.get_temp::<ActiveIconStack>(egui::Id::new("cad_active_icon_stack")) {
                if let Some(pack) = stack.0.iter().find(|p| p.manifest.id == pack_id) {
                    if let Some(snap) = pack.directory_snapshot() {
                        watcher.last_seen_snapshots.insert(pack_id.to_string(), snap);
                    }
                }
            }
        } else {
            watcher.watched_pack_ids.retain(|id| id != pack_id);
            watcher.last_seen_snapshots.remove(pack_id);
        }
        d.insert_temp(id, watcher);
    });
}

/// Check if any folder packs currently have watching enabled in `egui::Context`.
pub fn has_watched_icon_packs(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<IconWatcher>(egui::Id::new("cad_icon_watcher")).map(|w| w.dev_watch_enabled || !w.watched_pack_ids.is_empty()).unwrap_or(false))
}

/// RAII guard to prevent overlapping concurrent polls from UI and worker threads.
struct PollingGuard(Arc<AtomicBool>);

impl Drop for PollingGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// Helper to classify a changed path from directory snapshot diff.
fn classify_changed_path(path: &Path, manifest_changed: &mut bool, affected_icons: &mut HashSet<IconId>, other_file_changed: &mut bool) {
    if path == Path::new("manifest.ron") {
        *manifest_changed = true;
    } else if let Ok(subpath) = path.strip_prefix("icons") {
        let clean = subpath.to_string_lossy().replace('\\', "/");
        if let Some(icon_id) = IconId::from_id_str(&clean) {
            affected_icons.insert(icon_id);
        } else {
            *other_file_changed = true;
        }
    } else {
        *other_file_changed = true;
    }
}

/// Check watched folder packs for file changes.
/// If any file changed, granularly updates changed icons into `egui::Context` loader cache,
/// bumps revision, requests repaint, and returns true.
pub fn poll_watched_icon_packs(ctx: &egui::Context, palette: &qymcad_scheme::Palette) -> bool {
    let polling_lock_id = egui::Id::new("cad_icon_polling_active");
    let guard = ctx.data_mut(|d| {
        let flag = d.get_temp::<Arc<AtomicBool>>(polling_lock_id).unwrap_or_else(|| {
            let a = Arc::new(AtomicBool::new(false));
            d.insert_temp(polling_lock_id, Arc::clone(&a));
            a
        });
        if flag.swap(true, Ordering::SeqCst) {
            None
        } else {
            Some(PollingGuard(flag))
        }
    });

    let Some(_guard) = guard else {
        return false;
    };

    let watcher_id = egui::Id::new("cad_icon_watcher");
    let watcher = ctx.data(|d| d.get_temp::<IconWatcher>(watcher_id).unwrap_or_default());
    if !watcher.dev_watch_enabled && watcher.watched_pack_ids.is_empty() {
        return false;
    }

    let now = std::time::Instant::now();
    if let Some(last) = watcher.last_poll_time {
        if now.duration_since(last) < std::time::Duration::from_millis(200) {
            return false;
        }
    }

    let stack_id = egui::Id::new("cad_active_icon_stack");
    let mut stack = ctx.data(|d| d.get_temp::<ActiveIconStack>(stack_id)).map(|s| s.0).unwrap_or_default();
    let mut updated_snapshots = Vec::new();
    let mut affected_icons = HashSet::new();
    let mut manifest_changed = false;
    let mut other_file_changed = false;

    for pack in &mut stack {
        if pack.is_directory() && (watcher.dev_watch_enabled || watcher.watched_pack_ids.iter().any(|id| id == &pack.manifest.id)) {
            if let Some(snap) = pack.directory_snapshot() {
                if let Some(prev) = watcher.last_seen_snapshots.get(&pack.manifest.id) {
                    if prev != &snap {
                        for (path, sig) in &snap {
                            if prev.get(path) != Some(sig) {
                                classify_changed_path(path, &mut manifest_changed, &mut affected_icons, &mut other_file_changed);
                            }
                        }
                        for path in prev.keys() {
                            if !snap.contains_key(path) {
                                classify_changed_path(path, &mut manifest_changed, &mut affected_icons, &mut other_file_changed);
                            }
                        }
                        if manifest_changed {
                            let _ = pack.reload_manifest();
                        }
                        updated_snapshots.push((pack.manifest.id.clone(), snap));
                    }
                } else {
                    updated_snapshots.push((pack.manifest.id.clone(), snap));
                }
            }
        }
    }

    ctx.data_mut(|d| {
        let mut w = d.get_temp::<IconWatcher>(watcher_id).unwrap_or_default();
        w.last_poll_time = Some(now);
        for (id, snap) in updated_snapshots {
            w.last_seen_snapshots.insert(id, snap);
        }
        d.insert_temp(watcher_id, w);
    });

    let any_changed = manifest_changed || !affected_icons.is_empty() || other_file_changed;
    if any_changed {
        if manifest_changed {
            install_cad_icons(ctx, &stack, palette);
        } else {
            for &icon in &affected_icons {
                update_cad_icon(ctx, icon, &stack, palette);
            }
        }
        bump_icon_revision(ctx);
        ctx.data_mut(|d| d.insert_temp(stack_id, ActiveIconStack(stack)));
        ctx.request_repaint();
    }

    any_changed
}

/// Global fallback atomic handle to signal the running watcher thread even if Context is gone.
static RUNNING_WATCHER_SIGNAL: std::sync::RwLock<Option<Arc<AtomicBool>>> = std::sync::RwLock::new(None);

/// Ensure background watcher thread is running if there are watched icon packs.
pub fn ensure_watcher_thread(ctx: &egui::Context, palette: &qymcad_scheme::Palette) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("cad_active_palette"), palette.clone()));
    if !has_watched_icon_packs(ctx) {
        return;
    }
    let thread_flag_id = egui::Id::new("cad_icon_watcher_thread_running");
    let needs_spawn = ctx.data(|d| match d.get_temp::<Arc<AtomicBool>>(thread_flag_id) {
        Some(flag) => !flag.load(Ordering::SeqCst),
        None => true,
    });

    if needs_spawn {
        let flag = Arc::new(AtomicBool::new(true));
        let flag_clone = Arc::clone(&flag);
        if let Ok(mut lock) = RUNNING_WATCHER_SIGNAL.write() {
            *lock = Some(Arc::clone(&flag));
        }
        ctx.data_mut(|d| d.insert_temp(thread_flag_id, flag));

        let thread_ctx = ctx.clone();
        let _ = std::thread::Builder::new().name("icon-theme-watcher".to_string()).spawn(move || {
            while flag_clone.load(Ordering::SeqCst) {
                std::thread::sleep(std::time::Duration::from_millis(250));
                if !flag_clone.load(Ordering::SeqCst) {
                    break;
                }
                if !has_watched_icon_packs(&thread_ctx) {
                    break;
                }
                let pal = thread_ctx.data(|d| d.get_temp::<qymcad_scheme::Palette>(egui::Id::new("cad_active_palette"))).unwrap_or_else(qymcad_scheme::dark);
                poll_watched_icon_packs(&thread_ctx, &pal);
            }
            flag_clone.store(false, Ordering::SeqCst);
        });
    }
}

/// Request background watcher thread to terminate.
pub fn stop_watcher_thread(ctx: &egui::Context) {
    let thread_flag_id = egui::Id::new("cad_icon_watcher_thread_running");
    ctx.data(|d| {
        if let Some(flag) = d.get_temp::<Arc<AtomicBool>>(thread_flag_id) {
            flag.store(false, Ordering::SeqCst);
        }
    });
    if let Ok(mut lock) = RUNNING_WATCHER_SIGNAL.write() {
        if let Some(flag) = lock.take() {
            flag.store(false, Ordering::SeqCst);
        }
    }
}

/// Stop any running watcher thread globally without requiring an egui::Context.
pub fn stop_all_watcher_threads() {
    if let Ok(mut lock) = RUNNING_WATCHER_SIGNAL.write() {
        if let Some(flag) = lock.take() {
            flag.store(false, Ordering::SeqCst);
        }
    }
}
