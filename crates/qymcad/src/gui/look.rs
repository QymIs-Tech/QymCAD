//! HOW THE WINDOW LOOKS: fonts, the language, the scale, the visuals of the scheme, the logo, the clock.

use qymcad_ui_state::Notice;
use super::*;

// `Hover` deliberately has NO shared reset: three places clear THEIR OWN kind of hover ("no constraint
// under the cursor here"), and the passes run every frame, so the mutual exclusion holds by itself. A
// shared reset would be wrong here — it would clear the hover a neighbouring pass had just computed.

pub(crate) fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    // the Phosphor icon font (otherwise icons render as "tofu" boxes)
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    // A BOLD FACE. The egui set has none at all — only the ordinary proportional and monospace ones.
    // Labels drawn OVER geometry (the X/Y/Z axes, dimensions) get lost in a thin font.
    // Liberation Sans Bold: OFL (the licence sits next to the file), Latin plus Cyrillic.
    fonts.font_data.insert(BOLD_FONT.to_string(), std::sync::Arc::new(egui::FontData::from_static(include_bytes!("../../../../assets/fonts/LiberationSans-Bold.ttf"))));
    fonts.families.insert(egui::FontFamily::Name(BOLD_FONT.into()), vec![BOLD_FONT.to_string()]);
    mac_key_symbols(&mut fonts);
    ctx.set_fonts(fonts);
    // SVG and image loaders for custom icon themes.
    egui_extras::install_image_loaders(ctx);
    // THE HINT SIZE, set here because this is the one place that already decides how text is drawn - and
    // because a size set in two places drifts.
    //
    // Reported behaviour: "the hint fonts are small at scale 1 and at 1.2 alike ... make them a bit larger
    // everywhere, and make them scale with the setting too." Measured: a hint was 10 points against 15 for
    // ordinary text, two thirds of it. The interface scale is a zoom factor, so it multiplies BOTH by the
    // same amount - the hint does grow on screen, but its share of the text around it never changes, and
    // that is what makes it unreadable at every scale.
    //
    // IN POINTS, not pixels. The scale turns points into pixels, so a size given here follows it; a size in
    // pixels would be readable at one scale and wrong at every other.
    // BOTH THEMES: egui keeps a style per theme, and setting one leaves the hints tiny for whoever
    // switches to the other.
    ctx.all_styles_mut(|st| {
        if let Some(f) = st.text_styles.get_mut(&egui::TextStyle::Small) {
            f.size = 11.0; // against a body of 12.5: 0.88 instead of 0.72
        }
    });
}

/// ⌃ ⌘ ⇧ ⌥ FOR THE KEYS OF A MAC, where people read keys as symbols and `Ctrl+W` names the wrong key.
///
/// The fonts the program carries cannot draw them: ⌥ and ⌃ are in none of them, and ⇧ only in the monospace one, so
/// the symbols alone would be boxes. Every Mac has Apple Symbols, and it is taken from the system rather than
/// carried - it is drawn on a Mac only. Second in each family, right after the main face: the emoji fonts
/// further down have a ⌘ of a different look. Without the file the keys are written in words (`Shift+Cmd+W`).
pub(crate) fn mac_key_symbols(fonts: &mut egui::FontDefinitions) {
    if !cfg!(target_os = "macos") {
        return;
    }
    let Ok(bytes) = std::fs::read("/System/Library/Fonts/Apple Symbols.ttf") else {
        qymcad_ui_state::set_key_style(qymcad_ui_state::KeyStyle::MacWords);
        return;
    };
    const NAME: &str = "mac-key-symbols";
    fonts.font_data.insert(NAME.to_string(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        let list = fonts.families.entry(family).or_default();
        list.insert(list.len().min(1), NAME.to_string());
    }
    qymcad_ui_state::set_key_style(qymcad_ui_state::KeyStyle::MacSymbols);
}

/// The name of the BOLD font family. One place: family names spelled out separately drift apart and
/// give a silent fallback to the default font — the text still draws, only not bold, and that is
/// invisible to the eye in the code.
pub(crate) const BOLD_FONT: &str = "qym-bold";

/// The bold font at a given size.
pub(crate) fn bold(size: f32) -> egui::FontId {
    egui::FontId::new(size, egui::FontFamily::Name(BOLD_FONT.into()))
}

/// "hh:mm" for the autosave line, on the wall clock of the person at the screen.
///
/// Reported behaviour: at 14:28 in UTC+3 the line read "Auto-saved (11:27)" - the hour of UTC. The zone comes
/// from chrono's `Local`, which reads the zone database itself; the local offset of `time` gives no answer on
/// Linux once a second thread runs, and the window always runs several.
pub(crate) fn clock_hh_mm() -> String {
    hh_mm_in(unix_secs(), &chrono::Local)
}

/// "hh:mm" of an instant in a given zone - the part a test can pin, since the current time and the zone of
/// the machine running the test are not known in advance.
pub(crate) fn hh_mm_in<Z: chrono::TimeZone>(secs: u64, zone: &Z) -> String
where
    Z::Offset: std::fmt::Display,
{
    let at = chrono::DateTime::from_timestamp(secs as i64, 0).unwrap_or_default();
    at.with_timezone(zone).format("%H:%M").to_string()
}

/// APPLY THE THEME TO `egui`. The one setting for which sitting in the record is not enough: the
/// library does not remember its palette between runs, so it has to be assigned explicitly — at
/// start-up and on every change. The theme used to be assigned ONLY on a click and stored nowhere:
/// pick the light one, restart, and it is dark again.
/// APPLY THE LANGUAGE. An empty setting means "never chosen", and then the system locale decides.
/// The resolution lives in `i18n`, and this only passes it on: two places deciding "which language"
/// would drift apart at the very first edit.
pub(crate) fn apply_language(set: &Settings) {
    let code = if set.language.is_empty() { crate::i18n::system_default() } else { set.language.clone() };
    crate::i18n::set_language(&code);
    // THE HELP LANGUAGE TRAVELS WITH THE INTERFACE LANGUAGE — otherwise "same as the interface" would
    // lie after a switch: the help would stay on the previous one until its own setting was touched.
    crate::help::set_lang(&set.help_lang);
}

/// Test facades for the language setting: a test must take the same path as the settings window.
#[cfg(test)]
pub(crate) fn settings_language_is_empty(set: &Settings) -> bool {
    set.language.is_empty()
}

/// The look of the egui panels according to the current scheme. Kept apart from [`Self::apply_theme`],
/// because that one REBUILDS the scheme from the list — which, while a custom scheme is being edited
/// live, would wipe out the unsaved work.
pub(crate) fn sync_visuals(scheme: &SchemeUi, ctx: &egui::Context) {
    qymcad_ui_state::put_look(ctx, &scheme.pal); // both of egui's looks: the system's theme must not swap it out
                                                 // THE INTERFACE SCALE IS NOT APPLIED HERE. It has nothing to do with the theme, and the coupling
                                                 // was a hidden one: because of it "adopt the settings" worked even without its own call to the
                                                 // scale, and the guard stayed silent about that. The scale is applied by those whose business it
                                                 // is: `adopt_settings` and the slider in the window.
    let stack = qymcad_ui_state::icons::get_active_icon_stack(ctx);
    qymcad_ui_state::icons::set_active_icon_stack(ctx, stack, &scheme.pal);
}

/// APPLY THE INTERFACE SCALE TO `egui`.
///
/// A method of its own, called from BOTH places where the appearance is applied (`apply_theme` at
/// start-up and on a change of scheme, `sync_visuals` while a custom scheme is edited live). Exactly
/// the lesson the theme taught: it was called on a click and applied nowhere at start-up, so the
/// choice silently rolled back.
///
/// The bounds are strict: with a zero or negative factor `egui` draws an interface one can neither
/// leave nor use to put the setting back.
pub(crate) fn apply_ui_scale(set: &Settings, ctx: &egui::Context) {
    ctx.set_zoom_factor(set.ui_scale.clamp(0.5, 3.0));
}

/// Load the logo lazily (a 256x256 PNG embedded in the binary) into a texture for the splash screen.
pub(crate) fn ensure_logo(logo_tex: &mut Option<egui::TextureHandle>, ctx: &egui::Context) {
    if logo_tex.is_some() {
        return;
    }
    const LOGO: &[u8] = include_bytes!("../../../../assets/icons/linux/256x256.png");
    if let Ok(img) = image::load_from_memory(LOGO) {
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        let color = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], rgba.as_raw());
        *logo_tex = Some(ctx.load_texture("app-logo", color, egui::TextureOptions::LINEAR));
    }
}

/// Re-read the schemes from disk (at start-up and after a custom one is edited).
pub(crate) fn reload_schemes(scheme: &mut SchemeUi, status: &mut qymcad_ui_state::Notice) {
    let (all, errs) = crate::palette::all();
    scheme.all = all;
    if !errs.is_empty() {
        *status = Notice::error(crate::i18n::tr1("scheme-load-failed", "error", &errs.join("; ")));
    }
}

/// ADOPT A WHOLE SET OF SETTINGS — the only path by which they take effect.
///
/// The record alone is not enough: the theme, the language and the interface scale live not only in
/// it but also in `egui`'s state, which is not remembered between runs. While this was done as a
/// list of calls at start-up, the second path (importing a profile) would have had to repeat that
/// same list — and would have drifted from it at the very first new setting, silently at that: the
/// record is right while the screen shows the old thing.
pub(crate) fn adopt_settings(regen: &mut Rebuilding, scheme: &mut SchemeUi, set: &mut Settings, status: &mut qymcad_ui_state::Notice, s: Settings, ctx: &egui::Context) {
    *set = s;
    // CUSTOM SCHEMES COME FROM DISK BEFORE THE THEME IS APPLIED: otherwise the chosen custom scheme
    // will not be found in the list and will silently fall back to the dark one.
    reload_schemes(scheme, status);
    apply_theme(scheme, set, ctx);
    apply_language(set);
    apply_ui_scale(set, ctx);
    icon_themes::apply_icon_themes(set, ctx, &scheme.pal);
    invalidate(regen); // the colours and the scale are part of the picture caches' keys
}
