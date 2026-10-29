//! WHAT A PERSON CAN REACH, LISTED THE WAY THEY REACH IT - every tool button by its hint, every item of the menus,
//! every command the search finds, every article of help about a function - and set against the descriptions of
//! tools. What has no description is not checked, and the lists say so by name.
use qymcad::{Key, Kind, Modifiers, Session};

use crate::build;
use crate::contract::{Entry, Tool};

/// The arrow the menus draw at the end of an item that opens a submenu.
const SUBMENU: &str = "\u{23f5}";

/// THE HINTS OF EVERY TOOL BUTTON a person meets: in the assembly of a first start, inside its part, and inside a
/// sketch on a plane of that part.
pub fn button_hints() -> Vec<String> {
    let mut s = Session::start();
    s.key(Key::Escape);
    let mut all = hints_here(&mut s);
    build::into_the_first_part(&mut s);
    all.extend(hints_here(&mut s));
    let xy = s.word("plane-xy-table");
    s.press_word(&xy);
    all.extend(hints_here(&mut s));
    // the buttons that come only with a body to work on - the pencil that opens a sketch on a face
    let mut with_a_body = crate::contract::fixtures::Fixture::Block.start();
    all.extend(hints_here(&mut with_a_body));
    // the cross that breaks a reference stands only beside a reference to a neighbour
    let mut referring = crate::contract::fixtures::Fixture::PartOnNeighbourFace.start();
    all.extend(hints_here(&mut referring));
    dedup(all)
}

/// The hints of the buttons on screen, read with the pointer first taken off whatever it rests on: a hint already up
/// when the reading starts is a word already on screen, and the button under it would read as having none. Measured on
/// the sketch: entered by its plane's row, the pointer left the hint of the point button up, and "Point (node) [P]"
/// went unread.
fn hints_here(s: &mut Session) -> Vec<String> {
    let c = s.canvas();
    s.move_to(c.min + (c.max - c.min) * 0.04);
    let mut all = s.button_hints();
    all.extend(s.icon_hints());
    all
}

/// THE ITEMS OF EVERY MENU OF THE MENU BAR, submenus opened by resting over their items, each as its path of words.
pub fn menu_paths() -> Vec<Vec<String>> {
    let mut s = Session::start();
    s.key(Key::Escape);
    let bar: Vec<(String, qymcad::Rect)> = s.widgets().into_iter().filter(|w| w.kind == Kind::Button && !w.label.is_empty() && w.rect.max.y < 22.0).map(|w| (w.label, w.rect)).collect();
    let mut paths = Vec::new();
    for (menu, at) in bar {
        let before = labels(&mut s);
        s.click(at.center());
        let items: Vec<(String, qymcad::Rect)> = buttons(&mut s).into_iter().filter(|(l, _)| !before.contains(l)).collect();
        for (item, rect) in &items {
            paths.push(vec![menu.clone(), item_name(item)]);
            // a submenu is marked by an arrow at the end of its row
            let words = s.words_at();
            if words.iter().any(|(w, r)| w == SUBMENU && (r.center().y - rect.center().y).abs() < 6.0) {
                let inside = labels(&mut s);
                let _ = s.hint_at(rect.center());
                for (sub, _) in buttons(&mut s).into_iter().filter(|(l, _)| !inside.contains(l)) {
                    paths.push(vec![menu.clone(), item_name(item), item_name(&sub)]);
                }
            }
        }
        s.key(Key::Escape).key(Key::Escape);
    }
    paths
}

/// THE COMMANDS THE SEARCH FINDS, by name. The search shows eight rows a query, so it is asked every pair of letters.
pub fn command_names() -> Vec<String> {
    let mut s = Session::start();
    s.key(Key::Escape);
    let title = s.word("cs-title");
    s.chord(Modifiers::COMMAND, Key::K);
    let mut names = Vec::new();
    for a in 'a'..='z' {
        for b in 'a'..='z' {
            s.chord(Modifiers::COMMAND, Key::A).type_text(&format!("{a}{b}"));
            let window = s.widgets().into_iter().find(|w| w.kind == Kind::Window && w.label == title).map(|w| w.rect);
            let Some(window) = window else { panic!("the command search closed while it was asked {a}{b}") };
            names.extend(s.widgets().into_iter().filter(|w| w.kind == Kind::Button && !w.label.is_empty() && window.contains(w.rect.center())).map(|w| w.label));
        }
    }
    dedup(names)
}

/// THE ARTICLES OF HELP ABOUT A FUNCTION - of the sketch, the part, the assembly and the program in general - by
/// their paths under `docs/help/<language>/` without `.md`.
pub fn articles(language: &str) -> Vec<String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/help").join(language);
    let mut out = Vec::new();
    for section in ["sketch", "part", "assembly", "general"] {
        let Ok(dir) = std::fs::read_dir(root.join(section)) else { panic!("the help has no section {section} in {language}") };
        for e in dir.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if let Some(stem) = name.strip_suffix(".md").filter(|s| *s != "index") {
                out.push(format!("{section}/{stem}"));
            }
        }
    }
    out.sort();
    out
}

/// What a description reaches its tool by, in the words of `s`.
pub fn described(s: &Session, tools: &[&Tool]) -> (Vec<String>, Vec<Vec<String>>, Vec<String>, Vec<String>) {
    let (mut hints, mut menus, mut commands, mut help) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for t in tools {
        for e in t.entries {
            match e {
                Entry::Button(k) => hints.push(s.word(k)),
                Entry::Menu(path) => menus.push(path.iter().map(|k| s.word(k)).collect()),
                Entry::Search(k) => commands.push(s.word(k)),
                Entry::SearchByArticle => commands.push(article_title(&s.language(), t.help)),
                Entry::Key(..) | Entry::KeyAgain(..) | Entry::Label(_) => {}
            }
        }
        // a tool the help does not write about names no article: that absence is a finding, not a description
        if !t.help.is_empty() {
            help.push(t.help.to_string());
        }
    }
    (hints, menus, commands, help)
}

/// THE TITLE OF AN ARTICLE OF HELP, by its path under `docs/help/<language>/` without `.md`: the first heading of the
/// file. The command search names a command without a name of its own by this title, so a description that says "the
/// search finds it" has to know it too.
pub fn article_title(lang: &str, path: &str) -> String {
    let file = format!("{}/../../docs/help/{lang}/{path}.md", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("the article {file} cannot be read: {e}"));
    let head = text.lines().find(|l| l.starts_with("# ")).unwrap_or_else(|| panic!("the article {file} has no heading"));
    head.trim_start_matches("# ").trim().to_string()
}

/// THE NAME OF A MENU ITEM without what stands beside it on its row: the arrow of a submenu, and the keys that do the
/// same (`Save Ctrl+S` is the item `Save`).
pub fn item_name(label: &str) -> String {
    let words: Vec<&str> = label.split_whitespace().filter(|w| *w != SUBMENU).collect();
    let keys = |w: &str| ["Ctrl+", "Shift+", "Alt+", "Cmd+"].iter().any(|k| w.starts_with(k)) || (w.starts_with('F') && w.len() <= 3 && w[1..].chars().all(|c| c.is_ascii_digit()) && w.len() > 1);
    let end = words.iter().rposition(|w| !keys(w)).map_or(0, |i| i + 1);
    words[..end].join(" ")
}

/// The labels of the buttons on screen.
fn labels(s: &mut Session) -> Vec<String> {
    buttons(s).into_iter().map(|(l, _)| l).collect()
}

/// The buttons on screen with words on them, and where they stand.
fn buttons(s: &mut Session) -> Vec<(String, qymcad::Rect)> {
    s.widgets().into_iter().filter(|w| w.kind == Kind::Button && !w.label.is_empty()).map(|w| (w.label, w.rect)).collect()
}

/// A list without repeats, in the order first met.
fn dedup(items: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for i in items {
        if !i.is_empty() && !out.contains(&i) {
            out.push(i);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    /// A MENU ITEM IS NAMED WITHOUT ITS KEYS AND ITS ARROW, and a name that only looks like keys is kept.
    #[test]
    fn a_menu_item_is_named_without_its_keys() {
        for (label, name) in [
            ("Save Ctrl+S", "Save"),
            ("Redo Ctrl+Shift+Z", "Redo"),
            ("Export project \u{23f5}", "Export project"),
            ("STL\u{2026}", "STL\u{2026}"),
            ("Help F1", "Help"),
            ("F1 lesson", "F1 lesson"),
        ] {
            assert_eq!(super::item_name(label), name, "the item {label:?}");
        }
    }

    /// EVERY DESCRIPTION IS RUN THROUGH ITS CONTRACT, and every point it leaves out says why.
    #[test]
    fn every_description_is_run_through_its_contract() {
        let contract = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/acceptance/contract.rs")).expect("the contract checks read");
        let mut problems = Vec::new();
        for t in crate::tools::ALL {
            let name = t.id.rsplit('.').next().unwrap_or(t.id);
            // `sketch.dim-radius` is written `DIM_RADIUS`: a hyphen of a code is an underscore of a name
            let named = name.to_uppercase().replace('-', "_");
            if !contract.lines().any(|l| l.trim_start().starts_with("contract!(") && l.contains(&format!("::{named}"))) {
                problems.push(format!("{}: no contract! runs it", t.id));
            }
            let mut seen = Vec::new();
            for (point, why) in t.not_applicable {
                if !crate::contract::POINTS.iter().any(|(n, _)| n == point) || seen.contains(point) || why.trim().is_empty() {
                    problems.push(format!("{}: point {point} is left out wrongly (not a point of the contract, twice, or without why)", t.id));
                }
                seen.push(*point);
            }
        }
        let ids: Vec<&str> = crate::tools::ALL.iter().map(|t| t.id).collect();
        let unique: std::collections::HashSet<&&str> = ids.iter().collect();
        if unique.len() != ids.len() {
            problems.push(format!("two descriptions share a name: {ids:?}"));
        }
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }
}
