//! Guard on THE ONE VOICE OF THE PROGRAM: outside the theme and the tool of messages (`qymcad_ui_state::voice`), no
//! text is coloured by a warning or an error colour and no warning or error icon is put in front of words by hand.
//!
//! Measured on 2026-10-15, before the tool: the theme's error colour taken directly in 13 places, its milder error
//! colour in 23, its warning colour in 10, egui's copies of the same two colours in 34, and a hard-coded red and green
//! in one dialog - each place its own colour, its own icon or none. A message says its tone (`Notice`, `Tone`) and the
//! tool draws it; a place that picks a colour of its own is where the two drift apart again.
//!
//! The drawing of geometry is not a message: a constraint in conflict is drawn red on the canvas. Such files are named
//! below with the reason.

#[cfg(test)]
mod tests {
    /// A file allowed to name a tone's colour or icon, and why.
    struct Allowed {
        path: &'static str,
        why: &'static str,
    }

    const ALLOWED: [Allowed; 7] = [
        Allowed { path: "qymcad-scheme/src/lib.rs", why: "the theme: where the colours are defined and handed to egui's own widgets" },
        Allowed { path: "qymcad-ui-state/src/voice.rs", why: "the tool of messages: the one place a tone becomes a colour and an icon" },
        Allowed { path: "qymcad-render/src/sketch_overlay.rs", why: "the drawing of a sketch: a constraint in conflict or redundant is geometry painted on the canvas, not a message" },
        Allowed { path: "qymcad-render/src/sketch_marks.rs", why: "the marks of a sketch on the canvas: the glyph of a constraint in conflict is geometry, not a message" },
        Allowed { path: "qymcad-render/src/sketch_previews.rs", why: "the preview of a trim or an extension: the piece that goes is drawn red on the canvas, which is geometry" },
        Allowed { path: "qymcad-render/src/lib.rs", why: "the colours of the corners of a triangle in a check of the software rasteriser, not a message" },
        Allowed { path: "qymcad/src/gui/window/id_clash.rs", why: "egui's own red frame of a widget that changed its id, told by its colour in the window the checks drive, not a message" },
    ];

    /// What a place that says something must not name itself: the colours of a tone and the icons of a tone.
    const FORBIDDEN: [&str; 12] = [
        "pal.warning()", "pal.error()", "pal.error_mild()", "warn_fg_color", "error_fg_color", "Color32::RED", "Color32::GREEN", "Color32::YELLOW", "Color32::LIGHT_RED", "ph::WARNING",
        "regular::WARNING", "X_CIRCLE",
    ];

    #[test]
    fn every_exception_says_why() {
        for a in ALLOWED {
            assert!(a.why.split_whitespace().count() >= 8, "the exception of {} gives no reason", a.path);
        }
    }

    /// NO PLACE PICKS THE COLOUR OR THE ICON OF A TONE ITSELF: it says the tone, and the tool draws it.
    #[test]
    fn no_message_picks_its_own_colour_or_icon() {
        let root = qymcad_i18n::ratchet::crates_root();
        let mut stack = qymcad_i18n::ratchet::every_crate_src();
        let mut found = Vec::new();
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).expect("the sources read").flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                if p.extension().is_none_or(|x| x != "rs") || qymcad_i18n::ratchet::is_test_module(&p) {
                    continue;
                }
                let name = p.strip_prefix(&root).unwrap_or(&p).to_string_lossy().replace('\\', "/");
                if ALLOWED.iter().any(|a| a.path == name) || name.ends_with("one_voice_guard.rs") {
                    continue;
                }
                let text = std::fs::read_to_string(&p).unwrap_or_default();
                // the working half of the file: a module of checks below it compares against colours, it does not say
                let work = ["#[cfg(test)]\nmod ", "#[cfg(test)]\npub(crate) mod ", "#[cfg(test)]\npub(in crate::gui) mod ", "#[cfg(test)]\npub(super) mod "]
                    .iter()
                    .fold(text.as_str(), |t, cut| t.split(cut).next().unwrap_or(""));
                for (i, line) in work.lines().enumerate() {
                    if let Some(f) = FORBIDDEN.iter().find(|f| line.contains(**f)) {
                        found.push(format!("{name}:{}: {f}", i + 1));
                    }
                }
            }
        }
        assert!(
            found.is_empty(),
            "a place names the colour or the icon of a tone itself instead of saying the tone (`qymcad_ui_state::say`, `tone_colour`, `tone_mark`); {} of them:\n{}",
            found.len(),
            found.join("\n")
        );
    }
}
