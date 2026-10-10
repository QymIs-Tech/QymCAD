//! Icon generation, priority cascade resolution, and installation into egui.

use std::path::PathBuf;
use std::sync::Arc;

pub use super::id::{BUILTIN_ICON_THEMES, DEFAULT_THEME_ID};
use super::id::{IconId, ALL_ICONS};
use super::pack::IconPack;
pub use super::runtime::ResolvedIcon;

/// Load all built-in icon themes embedded into the binary.
/// Embedded archives do not change across frames, so they are inflated and parsed once.
pub fn load_builtin_packs() -> &'static [IconPack] {
    static BUILTIN: std::sync::OnceLock<Vec<IconPack>> = std::sync::OnceLock::new();
    BUILTIN.get_or_init(|| BUILTIN_ICON_THEMES.iter().filter_map(|theme| IconPack::from_embedded_zip_bytes(theme.archive).ok()).collect())
}

/// Load a specific built-in icon pack by its manifest ID.
pub fn load_builtin_pack(id: &str) -> Option<IconPack> {
    load_builtin_packs().iter().find(|theme| theme.manifest.id == id).cloned()
}

/// Load the built-in default icon pack embedded into the binary.
pub fn load_default_pack() -> Option<IconPack> {
    load_builtin_pack(DEFAULT_THEME_ID)
}

/// Load active icon packs in priority order based on IDs and search directories.
pub fn load_active_icon_stack(active_ids: &[String], search_dirs: &[PathBuf]) -> Vec<IconPack> {
    let mut all_packs = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();
    seen_ids.insert(DEFAULT_THEME_ID.to_string());

    for d in search_dirs {
        for p in super::bundle::discover_packs_in(d) {
            if p.manifest.id != DEFAULT_THEME_ID && seen_ids.insert(p.manifest.id.clone()) {
                all_packs.push(p);
            }
        }
    }
    for b in load_builtin_packs() {
        if b.manifest.id != DEFAULT_THEME_ID && seen_ids.insert(b.manifest.id.clone()) {
            all_packs.push(b.clone());
        }
    }

    let mut stack = Vec::new();
    for id in active_ids {
        if id != DEFAULT_THEME_ID {
            if let Some(p) = all_packs.iter().find(|p| &p.manifest.id == id && !p.has_id_conflict()) {
                stack.push(p.clone());
            }
        }
    }

    if !stack.iter().any(|p| p.manifest.id == DEFAULT_THEME_ID) {
        if let Some(def) = load_default_pack() {
            stack.push(def);
        }
    }

    stack
}

/// Resolve an icon through the cascade stack (top active pack -> lower packs -> default pack).
/// Pure stateless generator function with no locks or global state.
pub fn resolve_icon(id: IconId, stack: &[IconPack], palette: &qymcad_scheme::Palette) -> ResolvedIcon {
    for pack in stack {
        if let Some(data) = pack.get_svg_for_id(id) {
            let data = Arc::from(resolve_icon_tokens(&data, palette));
            return ResolvedIcon::new(data, pack.manifest.id.clone(), id);
        }
    }

    if let Some(def) = load_default_pack() {
        if let Some(data) = def.get_svg_for_id(id) {
            let data = Arc::from(resolve_icon_tokens(&data, palette));
            return ResolvedIcon::new(data, DEFAULT_THEME_ID, id);
        }
    }

    let fallback_raw = b"<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\"><rect width=\"24\" height=\"24\" fill=\"none\" stroke=\"currentColor\"/></svg>";
    let data = Arc::from(resolve_icon_tokens(fallback_raw, palette));
    ResolvedIcon::new(data, "builtin-fallback", id)
}

/// Update a specific CAD icon directly into `egui::Context` loaders.
/// Replaces existing icon bytes and evicts stale GPU texture for this icon's canonical URI.
pub fn update_cad_icon(ctx: &egui::Context, icon: IconId, stack: &[IconPack], palette: &qymcad_scheme::Palette) {
    let resolved = resolve_icon(icon, stack, palette);
    let uri = icon.uri();
    ctx.forget_image(uri);
    ctx.include_bytes(uri, resolved.data.to_vec());
}

/// Install / update all CAD icons directly into `egui::Context` loaders.
/// Replaces existing icon bytes and evicts stale GPU textures so `egui::Image::new(icon.uri())`
/// immediately renders the newly resolved icons.
pub fn install_cad_icons(ctx: &egui::Context, stack: &[IconPack], palette: &qymcad_scheme::Palette) {
    for &icon in ALL_ICONS {
        update_cad_icon(ctx, icon, stack, palette);
    }
}

/// Monotonically increasing revision counter stored in `egui::Context::data()`.
pub fn get_icon_revision(ctx: &egui::Context) -> u64 {
    ctx.data(|d| d.get_temp(egui::Id::new("cad_icon_revision")).unwrap_or(0))
}

/// Bump the icon revision counter in `egui::Context::data()`.
pub fn bump_icon_revision(ctx: &egui::Context) -> u64 {
    ctx.data_mut(|d| {
        let id = egui::Id::new("cad_icon_revision");
        let rev = d.get_temp::<u64>(id).unwrap_or(0) + 1;
        d.insert_temp(id, rev);
        rev
    })
}

/// Active icon stack container stored inside `egui::Context::data()`.
#[derive(Clone, Default)]
pub struct ActiveIconStack(pub Vec<IconPack>);

/// Set the active icon stack on `egui::Context` and update icon textures.
pub fn set_active_icon_stack(ctx: &egui::Context, mut stack: Vec<IconPack>, palette: &qymcad_scheme::Palette) {
    if !stack.iter().any(|p| p.manifest.id == DEFAULT_THEME_ID) {
        if let Some(def) = load_default_pack() {
            stack.push(def);
        }
    }
    install_cad_icons(ctx, &stack, palette);
    bump_icon_revision(ctx);
    ctx.data_mut(|d| {
        let watcher_id = egui::Id::new("cad_icon_watcher");
        let mut watcher = d.get_temp::<IconWatcher>(watcher_id).unwrap_or_default();
        for pack in &stack {
            if (watcher.dev_watch_enabled || watcher.watched_pack_ids.iter().any(|id| id == &pack.manifest.id)) && pack.is_directory() {
                if let Some(snap) = pack.directory_snapshot() {
                    watcher.last_seen_snapshots.insert(pack.manifest.id.clone(), snap);
                }
            }
        }
        d.insert_temp(watcher_id, watcher);
        d.insert_temp(egui::Id::new("cad_active_icon_stack"), ActiveIconStack(stack));
        d.insert_temp(egui::Id::new("cad_active_palette"), palette.clone());
    });
}

/// Retrieve the active icon stack from `egui::Context`.
pub fn get_active_icon_stack(ctx: &egui::Context) -> Vec<IconPack> {
    ctx.data(|d| d.get_temp::<ActiveIconStack>(egui::Id::new("cad_active_icon_stack"))).map(|s| s.0).unwrap_or_else(|| load_default_pack().map(|p| vec![p]).unwrap_or_default())
}

pub use super::watcher::*;

/// Preprocess SVG bytes by resolving CSS color variables (`var(--token, fallback)`)
/// and `currentColor` using the active palette.
pub fn resolve_icon_tokens(data: &[u8], palette: &qymcad_scheme::Palette) -> Vec<u8> {
    let has_var = data.windows(4).any(|w| w == b"var(");
    let has_current_color = data.windows(12).any(|w| w.eq_ignore_ascii_case(b"currentcolor"));

    if !has_var && !has_current_color {
        return data.to_vec();
    }

    let Ok(text) = std::str::from_utf8(data) else {
        return data.to_vec();
    };

    let stroke_hex = palette.format_icon_color("icon-stroke").unwrap_or_else(|| "#E0E0E0".to_string());

    let mut result = if has_var { resolve_text_vars(text, palette, &stroke_hex, 0) } else { text.to_string() };

    if result.to_ascii_lowercase().contains("currentcolor") {
        let mut replaced = String::with_capacity(result.len());
        let mut search_from = 0;
        let lower = result.to_ascii_lowercase();
        while let Some(pos) = lower[search_from..].find("currentcolor") {
            let abs_pos = search_from + pos;
            let after_idx = abs_pos + 12;
            if is_color_context(&result, abs_pos, after_idx) {
                replaced.push_str(&result[search_from..abs_pos]);
                replaced.push_str(&stroke_hex);
            } else {
                replaced.push_str(&result[search_from..after_idx]);
            }
            search_from = after_idx;
        }
        replaced.push_str(&result[search_from..]);
        result = replaced;
    }

    result.into_bytes()
}

/// Check if the match of `currentColor` at `[start..end]` is in a valid SVG color attribute
/// or CSS color property context, rather than in an element id, class, or text node.
fn is_color_context(full_text: &str, start: usize, end: usize) -> bool {
    let before_str = &full_text[..start];
    let after_str = &full_text[end..];

    let next_char = after_str.chars().next();
    if !matches!(next_char, None | Some('"' | '\'' | ';' | ')' | '}' | '>' | '/' | '!' | ' ' | '\t' | '\r' | '\n')) {
        return false;
    }

    let before_trimmed = before_str.trim_end_matches(|c: char| c.is_ascii_whitespace());

    // Case 1: Attribute assignment: fill="currentColor" or stroke='currentColor'
    let (has_quote, quote_char, before_attr) = if let Some(stripped) = before_trimmed.strip_suffix('"') {
        (true, '"', stripped)
    } else if let Some(stripped) = before_trimmed.strip_suffix('\'') {
        (true, '\'', stripped)
    } else {
        (false, ' ', before_trimmed)
    };

    let before_eq = before_attr.trim_end_matches(|c: char| c.is_ascii_whitespace());
    if let Some(before_eq_name) = before_eq.strip_suffix('=') {
        let attr_name = before_eq_name.trim_end_matches(|c: char| c.is_ascii_whitespace()).rsplit(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != ':').next().unwrap_or("");
        let attr_name = attr_name.rsplit(':').next().unwrap_or(attr_name);
        let is_color_attr = matches!(attr_name.to_ascii_lowercase().as_str(), "fill" | "stroke" | "stop-color" | "flood-color" | "lighting-color" | "color");
        if is_color_attr {
            if has_quote {
                let next_non_ws = after_str.trim_start_matches(|c: char| c.is_ascii_whitespace()).chars().next();
                return next_non_ws == Some(quote_char);
            }
            return true;
        }
        return false;
    }

    // Case 2: CSS property declaration: fill: currentColor;
    if let Some(before_colon_name) = before_trimmed.strip_suffix(':') {
        let prop_name = before_colon_name.trim_end_matches(|c: char| c.is_ascii_whitespace()).rsplit(|c: char| !c.is_ascii_alphanumeric() && c != '-').next().unwrap_or("");
        let is_color_prop = matches!(prop_name.to_ascii_lowercase().as_str(), "fill" | "stroke" | "stop-color" | "flood-color" | "lighting-color" | "color" | "outline-color" | "border-color");
        return is_color_prop;
    }

    // Case 3: Fallback value inside var(--token, currentColor)
    if before_trimmed.ends_with(',') {
        let after_trimmed = after_str.trim_start_matches(|c: char| c.is_ascii_whitespace());
        if after_trimmed.starts_with(')') {
            return true;
        }
    }

    false
}

/// Recursively expand CSS `var(--token, fallback)` expressions in `text`.
fn resolve_text_vars(text: &str, palette: &qymcad_scheme::Palette, stroke_hex: &str, depth: usize) -> String {
    if depth > 10 || !text.contains("var(") {
        return text.to_string();
    }

    let mut result = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(start_idx) = rest.find("var(") {
        result.push_str(&rest[..start_idx]);
        let after_var = &rest[start_idx + 4..];
        let mut paren_depth = 0usize;
        let mut close_idx = None;
        for (idx, ch) in after_var.char_indices() {
            if ch == '(' {
                paren_depth += 1;
            } else if ch == ')' {
                if paren_depth == 0 {
                    close_idx = Some(idx);
                    break;
                }
                paren_depth -= 1;
            }
        }

        if let Some(end_idx) = close_idx {
            let inner = &after_var[..end_idx];
            let replaced_color = if let Some((tok, fallback)) = inner.split_once(',') {
                let tok_trimmed = tok.trim();
                let fallback_trimmed = fallback.trim();
                if let Some(val) = palette.format_icon_color(tok_trimmed) {
                    val
                } else {
                    resolve_text_vars(fallback_trimmed, palette, stroke_hex, depth + 1)
                }
            } else {
                let tok_trimmed = inner.trim();
                palette.format_icon_color(tok_trimmed).unwrap_or_else(|| stroke_hex.to_string())
            };
            result.push_str(&replaced_color);
            rest = &after_var[end_idx + 1..];
        } else {
            result.push_str("var(");
            rest = after_var;
        }
    }
    result.push_str(rest);
    result
}
