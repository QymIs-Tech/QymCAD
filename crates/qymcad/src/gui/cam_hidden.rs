//! WITH THE MACHINING MODULE OFF, NOT ONE OF ITS WORDS IS ON THE SCREEN.
//!
//! The requirement: CAM is an addition people switch on themselves; while the box is unticked, none of
//! the CAM innards may be visible. A list of words cannot check that — such a test lies: it catches
//! the strings it was told about and stays silent about the one added tomorrow.
//!
//! The requirement became checkable precisely because the CAM strings live in a SEPARATE catalogue
//! (`cam.ftl`). So the question can be put exactly: build a frame of every surface and compare all its
//! text against the WHOLE contents of that catalogue. A new CAM string falls under the guard by
//! itself, with no edit to the test.
#[cfg(test)]
mod tests {
    use super::super::App;
    use crate::i18n;

    /// The words PROPER to machining: the values from `cam.ftl` MINUS everything that is in the CAD
    /// dictionary too.
    ///
    /// The subtraction carries weight. Words like "add", "name" and "select" lie in both catalogues,
    /// and meeting them on screen is no evidence: the CAD shows them. A guard that counts such a thing
    /// as a leak reddens the build for nothing, and then gets switched off wholesale — and the real
    /// check leaves along with the noise.
    ///
    /// The translations are taken rather than the keys: it is the text that reaches the screen. Short
    /// ones are dropped — a match on "X" or "mm" means nothing.
    fn cam_words(lang: &str) -> Vec<String> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().and_then(|p| p.parent()).expect("the root of the repository").join("i18n").join(lang);
        let values = |file: &str| -> Vec<String> {
            std::fs::read_to_string(dir.join(file))
                .unwrap_or_default()
                .lines()
                .filter_map(|l| l.split_once(" = "))
                .map(|(_, v)| v.trim().to_lowercase())
                .collect()
        };
        let cad: std::collections::HashSet<String> = values("main.ftl").into_iter().chain(values("errors.ftl")).collect();
        let mut out: Vec<String> = std::fs::read_to_string(dir.join("cam.ftl"))
            .expect("the CAM catalogue reads")
            .lines()
            .filter_map(|l| l.split_once(" = "))
            .map(|(_, v)| v.trim().to_string())
            .filter(|v| v.chars().count() > 3 && v.chars().any(|c| c.is_alphabetic()) && !v.contains('{') && !cad.contains(&v.to_lowercase()))
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// THE WHOLE CAPTION, NOT A SUBSTRING.
    ///
    /// The requirement is that not one STRING from the CAM dictionary is on the screen, and that is
    /// exactly what must be compared: "Pick" sits inside the CAD's own "Pick a face...", and by
    /// substring the guard would find a leak where there is none. The icon at the start is stripped: a
    /// caption is drawn as "{glyph} Word".
    fn same_label(screen: &str, word: &str) -> bool {
        let norm = |s: &str| s.trim_start_matches(|c: char| !c.is_alphanumeric()).trim().to_lowercase();
        norm(screen) == norm(word)
    }

    /// A scene the panels have something to show on, plus the open windows.
    fn app_with_everything_open(cam_on: bool) -> App {
        let mut app = super::super::screen_keys::tests::populated();
        app.win.settings = true;
        app.win.params = true;
        app.win.parts_library = true;
        app.win.hotkeys = true;
        app.win.about = true;
        app.win.tools = true;
        app.set_show_start_for_test(true);
        app.set_cam_tab_for_test(cam_on);
        // THE HELP IS A SURFACE TOO: its table of contents is built from files, and the machining
        // section would leak into it by itself, without a single line of code about CAM.
        app.open_help("index");
        app
    }

    type Surface = (&'static str, fn(&mut App, &egui::Context));
    const SURFACES: &[Surface] = &[
        ("tree", |a, c| a.tree_panel(c)),
        ("properties", |a, c| a.properties_panel(c)),
        ("menu", |a, c| a.menu_bar(c)),
        ("context bar", |a, c| a.toolbar(c)),
        ("tool bar", |a, c| a.tool_options_bar(c)),
        ("settings", |a, c| a.settings_window(c)),
        ("parameters", |a, c| a.params_window(c)),
        ("parts library", |a, c| a.parts_library_window(c)),
        ("hotkeys", |a, c| a.hotkeys_window(c)),
        ("about", |a, c| a.about_dialog(c)),
        ("tools (CAM)", |a, c| a.tools_window(c)),
        ("start screen", |a, c| a.start_screen(c)),
        ("help", |a, c| a.help_window(c)),
    ];

    /// THE POINT: with the box unticked there is no machining word anywhere on screen.
    #[test]
    fn with_the_module_off_no_machining_word_reaches_the_screen() {
        let prev = i18n::language();
        let mut leaks: Vec<String> = Vec::new();
        for code in ["ru", "en"] {
            i18n::set_language(code);
            let words = cam_words(code);
            assert!(words.len() > 100, "{code}: suspiciously few CAM words were collected: {}", words.len());
            for (name, draw) in SURFACES {
                let mut app = app_with_everything_open(false);
                for t in super::super::screen_keys::tests::frame_text(&mut app, *draw) {
                    if let Some(w) = words.iter().find(|w| same_label(&t, w)) {
                        let msg = format!("{code}: \"{name}\" showed the machining word \"{w}\"");
                        if !leaks.contains(&msg) {
                            leaks.push(msg);
                        }
                    }
                }
            }
        }
        i18n::set_language(&prev);
        assert!(leaks.is_empty(), "the machining module is off and its words are on screen ({}):\n{}", leaks.len(), leaks.join("\n"));
    }

    /// AND THE REVERSE: switch it on and it comes back. Without this half the guard would pass on a
    /// program where CAM is switched off FOR GOOD — that is, it would prove the wrong thing.
    #[test]
    fn switching_the_module_on_brings_machining_back() {
        let prev = i18n::language();
        i18n::set_language("ru");
        let words = cam_words("ru");
        let mut seen = false;
        for (_, draw) in SURFACES {
            let mut app = app_with_everything_open(true);
            if super::super::screen_keys::tests::frame_text(&mut app, *draw).iter().any(|t| words.iter().any(|w| same_label(t, w))) {
                seen = true;
                break;
            }
        }
        i18n::set_language(&prev);
        assert!(seen, "the machining module was switched on and still not one of its words is on screen — the guard is checking emptiness");
    }
}
