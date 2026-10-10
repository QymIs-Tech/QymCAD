//! HOW A KEY IS WRITTEN ON SCREEN.
//!
//! The settings and the catalogue keep the portable spelling (`Ctrl+Shift+W`, "Copy (Ctrl+C)"): a profile
//! means the same keys on every system, and a sentence is translated once. Only what is DRAWN follows the
//! system - on a Mac `⇧⌘W`. Here rather than beside the bindings because the dictionary writes every caption:
//! a key typed into a sentence has to change with the key in the table, or a Mac shows both spellings at once.

/// HOW A KEY IS WRITTEN ON SCREEN. The record always keeps the portable spelling (`Ctrl+Shift+W`), so a profile
/// carried from one system to another means the same keys; only the label follows the system.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyStyle {
    /// As stored: `Ctrl+Shift+W`.
    Plain,
    /// A Mac with a font that has the symbols: `⇧⌘W`, in Apple's order (Control, Option, Shift, Command).
    MacSymbols,
    /// A Mac without that font: `Shift+Cmd+W`. Words rather than boxes where a glyph would be missing.
    MacWords,
}

static KEY_STYLE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(if cfg!(target_os = "macos") { 2 } else { 0 });

/// Chosen once, where the fonts are installed: that is the one place that knows whether the symbols can be drawn.
pub fn set_key_style(style: KeyStyle) {
    let v = match style {
        KeyStyle::Plain => 0,
        KeyStyle::MacSymbols => 1,
        KeyStyle::MacWords => 2,
    };
    KEY_STYLE.store(v, std::sync::atomic::Ordering::Relaxed);
}

pub fn key_style() -> KeyStyle {
    match KEY_STYLE.load(std::sync::atomic::Ordering::Relaxed) {
        1 => KeyStyle::MacSymbols,
        2 => KeyStyle::MacWords,
        _ => KeyStyle::Plain,
    }
}

/// THE LABEL OF A STORED KEY in the style of this system. Every place that SHOWS a key goes through here; the
/// places that compare or store keys keep the stored spelling.
pub fn key_label(stored: &str) -> String {
    key_label_in(stored, key_style())
}

/// The same, with the style given - so the Mac forms can be checked on any system.
///
/// It reads the text rather than a `Chord`, because the general rows are written as text (`Ctrl+Z / Ctrl+Y`)
/// and must change with the rest. "Ctrl" in the stored form IS the command key (see `Chord`), so on a Mac it is
/// ⌘; "Control" is the Mac's own Control key, ⌃; Alt is Option.
pub fn key_label_in(stored: &str, style: KeyStyle) -> String {
    if style == KeyStyle::Plain {
        return stored.to_string();
    }
    stored.split(" / ").map(|part| part.split(", ").map(|chord| chord_label(chord, style)).collect::<Vec<_>>().join(", ")).collect::<Vec<_>>().join(" / ")
}

/// One chord of a key in a Mac style. AltGr is the right Option key there: the Option sign after an R, or
/// `Right Option+`.
fn chord_label(chord: &str, style: KeyStyle) -> String {
    let mut tokens: Vec<&str> = chord.split('+').collect();
    let key = tokens.pop().unwrap_or("");
    let (mut control, mut opt, mut ropt, mut shift, mut cmd, mut other) = (false, false, false, false, false, Vec::new());
    for t in tokens {
        match t {
            "Control" => control = true,
            "Ctrl" => cmd = true,
            "Shift" => shift = true,
            "Alt" => opt = true,
            "AltGr" => ropt = true,
            _ => other.push(t),
        }
    }
    let mods = [(control, "⌃", "Control"), (opt, "⌥", "Option"), (ropt, "R⌥", "Right Option"), (shift, "⇧", "Shift"), (cmd, "⌘", "Cmd")];
    let mut out: String = other.iter().map(|t| format!("{t}+")).collect();
    for (on, symbol, word) in mods {
        if on {
            match style {
                KeyStyle::MacSymbols => out.push_str(symbol),
                _ => {
                    out.push_str(word);
                    out.push('+');
                }
            }
        }
    }
    out.push_str(key);
    out
}

/// THE KEYS INSIDE A SENTENCE, written the way this system writes them: "Copy (Ctrl+C)" is "Copy (⌘C)" on a
/// Mac. A key is one or more of `Control+`, `Ctrl+`, `Shift+`, `Alt+` and then a key name (`C`, `Enter`, `F5`); the word
/// "Ctrl" alone - "Ctrl combinations" - is a word about the key, not a key, and stays.
pub fn keys_in_text(text: &str) -> String {
    keys_in_text_in(text, key_style())
}

/// The same, with the style given - so the Mac forms can be checked on any system.
pub fn keys_in_text_in(text: &str, style: KeyStyle) -> String {
    if style == KeyStyle::Plain || !text.contains('+') {
        return text.to_string();
    }
    const MODS: [&str; 4] = ["Control+", "Ctrl+", "Shift+", "Alt+"];
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        let rest = &text[i..];
        // a key starts at a word boundary: "XCtrl+C" is not one
        let boundary = text[..i].chars().next_back().is_none_or(|c| !c.is_alphanumeric());
        if boundary && MODS.iter().any(|m| rest.starts_with(m)) {
            let mut j = 0;
            while let Some(m) = MODS.iter().find(|m| rest[j..].starts_with(*m)) {
                j += m.len();
            }
            let name = rest[j..].chars().take_while(char::is_ascii_alphanumeric).count();
            if name > 0 {
                out.push_str(&key_label_in(&rest[..j + name], style));
                i += j + name;
                continue;
            }
        }
        let c = rest.chars().next().unwrap_or_default();
        out.push(c);
        i += c.len_utf8();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mac_writes_the_keys_of_a_sentence_with_symbols() {
        let mac = KeyStyle::MacSymbols;
        assert_eq!(keys_in_text_in("Copy (Ctrl+C)", mac), "Copy (⌘C)");
        assert_eq!(keys_in_text_in("pick the target component and press Ctrl+V", mac), "pick the target component and press ⌘V");
        assert_eq!(keys_in_text_in("Go one level up — Ctrl+Enter", mac), "Go one level up — ⌘Enter");
        assert_eq!(keys_in_text_in("Ctrl+Shift+S saves as, Alt+U from a field", mac), "⇧⌘S saves as, ⌥U from a field");
        assert_eq!(keys_in_text_in("Ctrl combinations belong to the system", mac), "Ctrl combinations belong to the system", "a word about the key is not a key");
        assert_eq!(keys_in_text_in("C++ and 2+2", mac), "C++ and 2+2");
        // a key after a word in another script: the Russian caption of the same command, read from the catalogue
        let ru = crate::tr_in("ru", "act-copy-ctrl-c").expect("the Russian caption of Copy");
        assert_eq!(keys_in_text_in(&ru, mac), ru.replace("Ctrl+C", "⌘C"));
    }

    /// THE MAC'S CONTROL KEY IS ITS OWN MODIFIER, written first as Apple writes it, and never mixed up with Ctrl.
    #[test]
    fn a_mac_writes_its_control_key_apart_from_cmd() {
        assert_eq!(key_label_in("Control+J", KeyStyle::MacSymbols), "⌃J");
        assert_eq!(key_label_in("Control+Ctrl+Shift+J", KeyStyle::MacSymbols), "⌃⇧⌘J");
        assert_eq!(key_label_in("Control+Shift+J", KeyStyle::MacWords), "Control+Shift+J");
        assert_eq!(keys_in_text_in("press Control+A in a field", KeyStyle::MacSymbols), "press ⌃A in a field");
        assert_eq!(key_label_in("Control+J", KeyStyle::Plain), "Control+J", "off a Mac the record reads as it is");
    }

    /// A SEQUENCE IS WRITTEN CHORD BY CHORD, and the right Option key apart from the left one.
    #[test]
    fn a_mac_writes_a_sequence_chord_by_chord() {
        assert_eq!(key_label_in("Ctrl+T, Ctrl+Shift+F", KeyStyle::MacSymbols), "⌘T, ⇧⌘F");
        assert_eq!(key_label_in("G, G", KeyStyle::MacSymbols), "G, G");
        assert_eq!(key_label_in("Alt+G, G", KeyStyle::MacSymbols), "⌥G, G");
        assert_eq!(key_label_in("AltGr+F", KeyStyle::MacSymbols), "R⌥F");
        assert_eq!(key_label_in("Shift+AltGr+F", KeyStyle::MacWords), "Right Option+Shift+F");
        assert_eq!(key_label_in("Ctrl+T, F", KeyStyle::Plain), "Ctrl+T, F");
    }

    #[test]
    fn off_a_mac_a_sentence_is_left_as_written() {
        assert_eq!(keys_in_text_in("Copy (Ctrl+C)", KeyStyle::Plain), "Copy (Ctrl+C)");
    }
}
