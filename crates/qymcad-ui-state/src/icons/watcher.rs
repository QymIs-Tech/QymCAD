//! Live icon theme directory watching and background worker thread.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::id::IconId;
use super::manager::{
    active_icon_stack_id, active_palette_id, bump_icon_revision, icon_polling_active_id, install_cad_icons, update_cad_icon, watcher_id, watcher_thread_running_id, ActiveIconStack, ICON_STACK_LOCK,
};
use super::pack::FileSignature;

/// State for live directory watching stored inside `egui::Context::data()`.
#[derive(Clone, Default)]
pub struct IconWatcher {
    pub dev_watch_enabled: bool,
    pub watched_pack_ids: Vec<String>,
    pub last_seen_snapshots: HashMap<String, HashMap<PathBuf, FileSignature>>,
    pub removed_pack_ids: Vec<String>,
    pub last_poll_time: Option<std::time::Instant>,
}

/// Take IDs of active themes whose manifest ID changed while they were watched.
pub fn take_removed_icon_pack_ids(ctx: &egui::Context) -> Vec<String> {
    ctx.data_mut(|data| {
        let w_id = watcher_id();
        let mut watcher = data.get_temp::<IconWatcher>(w_id).unwrap_or_default();
        let removed = std::mem::take(&mut watcher.removed_pack_ids);
        data.insert_temp(w_id, watcher);
        removed
    })
}

/// Synchronize watched pack IDs in `egui::Context`.
pub fn sync_watched_packs(ctx: &egui::Context, watched: &[String]) {
    ctx.data_mut(|d| {
        let id = watcher_id();
        let mut watcher = d.get_temp::<IconWatcher>(id).unwrap_or_default();
        watcher.watched_pack_ids = watched.to_vec();
        if let Some(stack) = d.get_temp::<ActiveIconStack>(active_icon_stack_id()) {
            for pack_id in watched {
                if !watcher.last_seen_snapshots.contains_key(pack_id) {
                    if let Some(pack) = stack.packs.iter().find(|p| &p.manifest.id == pack_id) {
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
        let id = watcher_id();
        let mut watcher = d.get_temp::<IconWatcher>(id).unwrap_or_default();
        watcher.dev_watch_enabled = enabled;
        d.insert_temp(id, watcher);
    });
}

/// Check if a specific pack is watched in `egui::Context`.
pub fn is_pack_watched(ctx: &egui::Context, pack_id: &str) -> bool {
    ctx.data(|d| d.get_temp::<IconWatcher>(watcher_id()).map(|w| w.dev_watch_enabled || w.watched_pack_ids.iter().any(|id| id == pack_id)).unwrap_or(false))
}

/// Set watch state for a pack in `egui::Context`.
pub fn set_pack_watching(ctx: &egui::Context, pack_id: &str, watch: bool) {
    ctx.data_mut(|d| {
        let id = watcher_id();
        let mut watcher = d.get_temp::<IconWatcher>(id).unwrap_or_default();
        if watch {
            if !watcher.watched_pack_ids.iter().any(|id| id == pack_id) {
                watcher.watched_pack_ids.push(pack_id.to_string());
            }
            if let Some(stack) = d.get_temp::<ActiveIconStack>(active_icon_stack_id()) {
                if let Some(pack) = stack.packs.iter().find(|p| p.manifest.id == pack_id) {
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
    ctx.data(|d| d.get_temp::<IconWatcher>(watcher_id()).map(|w| w.dev_watch_enabled || !w.watched_pack_ids.is_empty()).unwrap_or(false))
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
    let polling_lock_id = icon_polling_active_id();
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

    let w_id = watcher_id();
    let watcher = ctx.data(|d| d.get_temp::<IconWatcher>(w_id).unwrap_or_default());
    if !watcher.dev_watch_enabled && watcher.watched_pack_ids.is_empty() {
        return false;
    }

    let now = std::time::Instant::now();
    if let Some(last) = watcher.last_poll_time {
        if now.duration_since(last) < std::time::Duration::from_millis(200) {
            return false;
        }
    }

    let stack_id = active_icon_stack_id();
    let (mut stack, poll_generation) = ctx.data(|d| d.get_temp::<ActiveIconStack>(stack_id).map(|s| (s.packs, s.generation)).unwrap_or_default());
    let mut updated_snapshots = HashMap::new();
    let mut reloaded_packs = HashMap::new();
    let mut removed_ids = Vec::new();
    let mut removed_paths = HashSet::new();
    let mut affected_icons = HashSet::new();
    let mut manifest_changed = false;
    let mut other_file_changed = false;

    for pack in &mut stack {
        if pack.is_directory() && (watcher.dev_watch_enabled || watcher.watched_pack_ids.iter().any(|id| id == &pack.manifest.id)) {
            let old_id = pack.manifest.id.clone();
            if let Some(snap) = pack.directory_snapshot() {
                if let Some(prev) = watcher.last_seen_snapshots.get(&old_id) {
                    if prev != &snap {
                        let mut this_manifest_changed = false;
                        for (path, sig) in &snap {
                            if prev.get(path) != Some(sig) {
                                classify_changed_path(path, &mut this_manifest_changed, &mut affected_icons, &mut other_file_changed);
                            }
                        }
                        for path in prev.keys() {
                            if !snap.contains_key(path) {
                                classify_changed_path(path, &mut this_manifest_changed, &mut affected_icons, &mut other_file_changed);
                            }
                        }
                        if this_manifest_changed {
                            manifest_changed = true;
                            if pack.reload_manifest().is_ok() {
                                if pack.manifest.id != old_id {
                                    if let Some(path) = pack.source_path() {
                                        removed_paths.insert(path.to_path_buf());
                                    }
                                    removed_ids.push(old_id);
                                    continue;
                                } else {
                                    reloaded_packs.insert(pack.manifest.id.clone(), pack.clone());
                                }
                            }
                        }
                        updated_snapshots.insert(old_id, snap);
                    }
                } else {
                    updated_snapshots.insert(old_id, snap);
                }
            }
        }
    }

    let any_changed = manifest_changed || !affected_icons.is_empty() || other_file_changed;
    if !any_changed {
        ctx.data_mut(|d| {
            let mut w = d.get_temp::<IconWatcher>(w_id).unwrap_or_default();
            w.last_poll_time = Some(now);
            w.last_seen_snapshots.extend(updated_snapshots);
            d.insert_temp(w_id, w);
        });
        return false;
    }

    // Synchronize applying watched icon changes with set_active_icon_stack to eliminate race conditions.
    let _lock = ICON_STACK_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let (current_packs, current_gen) = ctx.data(|d| d.get_temp::<ActiveIconStack>(stack_id).map(|s| (s.packs, s.generation)).unwrap_or_default());

    if current_gen != poll_generation {
        // Active icon stack was modified concurrently (e.g. by set_active_icon_stack on UI thread).
        // Discard stale updates so we do not overwrite the user's latest theme selection.
        ctx.data_mut(|d| {
            let mut w = d.get_temp::<IconWatcher>(w_id).unwrap_or_default();
            w.last_poll_time = Some(now);
            w.last_seen_snapshots.extend(updated_snapshots);
            d.insert_temp(w_id, w);
        });
        return false;
    }

    let mut updated_stack = current_packs;
    if manifest_changed {
        for pack in &mut updated_stack {
            if let Some(reloaded) = reloaded_packs.get(&pack.manifest.id) {
                *pack = reloaded.clone();
            }
        }
        updated_stack.retain(|pack| pack.source_path().is_none_or(|path| !removed_paths.contains(path)));
    }

    let next_gen = current_gen.wrapping_add(1);
    ctx.data_mut(|d| {
        let mut w = d.get_temp::<IconWatcher>(w_id).unwrap_or_default();
        w.last_poll_time = Some(now);
        w.last_seen_snapshots.extend(updated_snapshots);
        for id in removed_ids {
            w.last_seen_snapshots.remove(&id);
            w.watched_pack_ids.retain(|watched| watched != &id);
            if !w.removed_pack_ids.contains(&id) {
                w.removed_pack_ids.push(id);
            }
        }
        d.insert_temp(w_id, w);
        if manifest_changed {
            d.insert_temp(stack_id, ActiveIconStack::new(updated_stack.clone(), next_gen));
        }
    });

    if manifest_changed {
        install_cad_icons(ctx, &updated_stack, palette);
    } else {
        for &icon in &affected_icons {
            update_cad_icon(ctx, icon, &updated_stack, palette);
        }
    }
    bump_icon_revision(ctx);
    ctx.request_repaint();

    true
}

/// Global fallback atomic handle to signal the running watcher thread even if Context is gone.
static RUNNING_WATCHER_SIGNAL: std::sync::RwLock<Option<Arc<AtomicBool>>> = std::sync::RwLock::new(None);

/// Ensure background watcher thread is running if there are watched icon packs.
pub fn ensure_watcher_thread(ctx: &egui::Context, palette: &qymcad_scheme::Palette) {
    ctx.data_mut(|d| d.insert_temp(active_palette_id(), palette.clone()));
    if !has_watched_icon_packs(ctx) {
        return;
    }
    let thread_flag_id = watcher_thread_running_id();
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
        ctx.data_mut(|d| d.insert_temp(thread_flag_id, Arc::clone(&flag)));

        let thread_ctx = ctx.clone();
        match std::thread::Builder::new().name("icon-theme-watcher".to_string()).spawn(move || {
            while flag_clone.load(Ordering::SeqCst) {
                std::thread::sleep(std::time::Duration::from_millis(250));
                if !flag_clone.load(Ordering::SeqCst) {
                    break;
                }
                if !has_watched_icon_packs(&thread_ctx) {
                    break;
                }
                let pal = thread_ctx.data(|d| d.get_temp::<qymcad_scheme::Palette>(active_palette_id())).unwrap_or_else(qymcad_scheme::dark);
                poll_watched_icon_packs(&thread_ctx, &pal);
            }
            flag_clone.store(false, Ordering::SeqCst);
        }) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Failed to spawn icon theme watcher thread: {e}");
                flag.store(false, Ordering::SeqCst);
                if let Ok(mut lock) = RUNNING_WATCHER_SIGNAL.write() {
                    *lock = None;
                }
                ctx.data_mut(|d| {
                    d.remove_temp::<Arc<AtomicBool>>(thread_flag_id);
                });
            }
        }
    }
}

/// Request background watcher thread to terminate.
pub fn stop_watcher_thread(ctx: &egui::Context) {
    let thread_flag_id = watcher_thread_running_id();
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
