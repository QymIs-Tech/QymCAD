//! THE LIST OF FONTS A PERSON CAN WRITE WITH.
//!
//! A file dialog is not a font chooser: it asks a person to know where the file lies. The list is built by
//! walking the font directories of the system - and of the sandboxes, where the host's fonts are mounted
//! somewhere else entirely.
use std::io::Write;

/// A directory of our own with a font in it, so the walk is measured on something that is certainly there.
fn a_directory_with_a_font() -> (std::path::PathBuf, Vec<u8>) {
    let bytes = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/fonts/LiberationSans-Bold.ttf")).expect("the font shipped with the repository");
    let dir = std::env::temp_dir().join(format!("qym-fonts-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("deeper")).expect("the directory is made");
    // one font, one file that only looks like one, and one that is not a font at all
    std::fs::write(dir.join("deeper").join("Ours.ttf"), &bytes).expect("the font is written");
    let mut junk = std::fs::File::create(dir.join("notes.txt")).expect("the file is made");
    junk.write_all(b"not a font").expect("written");
    std::fs::write(dir.join("broken.ttf"), b"neither is this").expect("written");
    (dir, bytes)
}

#[test]
fn the_walk_finds_a_font_by_its_own_name_and_ignores_what_is_not_one() {
    let (dir, _) = a_directory_with_a_font();
    let found = qymcad_ui_state::installed_fonts_in(std::slice::from_ref(&dir));
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(found.len(), 1, "one font lies there, and the walk found {}: {found:?}", found.len());
    let f = &found[0];
    assert_eq!(f.family, "Liberation Sans", "the family is read from the file, not from the file name");
    assert_eq!(f.style, "Bold", "the style is read from the file");
    assert_eq!(f.index, 0, "an ordinary font holds one face");
    assert!(f.path.ends_with("Ours.ttf"), "the path leads to the file that was found: {}", f.path);
}

/// AND THE SANDBOXES ARE IN THE LIST OF PLACES TO LOOK.
///
/// Inside Flatpak the host's fonts are mounted read-only under `/run/host`, and `/usr/share/fonts` holds the
/// runtime's own few. A list that knows only the ordinary paths shows a person three fonts and none of their
/// own. Nothing needs to be granted for those paths - they are there for every application.
#[test]
#[cfg(target_os = "linux")]
fn the_places_to_look_include_the_ones_a_sandbox_uses() {
    let dirs: Vec<String> = qymcad_ui_state::font_directories().iter().map(|d| d.to_string_lossy().into_owned()).collect();
    for want in ["/run/host/fonts", "/run/host/local-fonts", "/run/host/user-fonts"] {
        assert!(dirs.iter().any(|d| d == want), "the fonts of the host inside Flatpak are not looked for: {want} is missing from {dirs:?}");
    }
    assert!(dirs.iter().any(|d| d.contains("hostfs")), "the fonts of the host inside Snap are not looked for: nothing under /var/lib/snapd/hostfs in {dirs:?}");
}

/// THE SEARCH NARROWS THE LIST BY FAMILY AND BY STYLE, whatever the case.
#[test]
fn the_search_finds_by_family_and_by_style() {
    let faces = vec![
        qymcad_ui_state::FontFace { family: "Liberation Sans".into(), style: "Bold".into(), path: "/a.ttf".into(), index: 0, note: String::new() },
        qymcad_ui_state::FontFace { family: "DejaVu Serif".into(), style: "Italic".into(), path: "/b.ttf".into(), index: 0, note: String::new() },
        qymcad_ui_state::FontFace { family: "Noto Sans".into(), style: "Regular".into(), path: "/c.ttf".into(), index: 0, note: String::new() },
    ];
    let names = |q: &str| qymcad_ui_state::fonts_matching(&faces, q).into_iter().map(|f| f.family).collect::<Vec<_>>();
    assert_eq!(names(""), vec!["Liberation Sans", "DejaVu Serif", "Noto Sans"], "an empty search hides nothing");
    assert_eq!(names("  sans "), vec!["Liberation Sans", "Noto Sans"], "the family is searched, and the spaces around are not part of the word");
    assert_eq!(names("ITALIC"), vec!["DejaVu Serif"], "the style is searched too, whatever the case");
    assert!(names("no such family").is_empty(), "a search that matches nothing shows nothing");
}

/// THE SAME FONT IN TWO PLACES IS ONE ROW, not two.
///
/// Measured on the reporter's machine: 254 faces and thirteen pairs of rows saying exactly the same thing -
/// one `NotoSansCJK-Regular.ttc` lying in `~/.local/share/fonts`, `/usr/local/share/fonts` and
/// `/usr/share/fonts` at once, and `CaskaydiaCove` copied into a folder inside its own folder. Real copies,
/// not symlinks, so canonicalising the directories does not help. A person picking a font sees three
/// identical lines and no way to tell what the difference is - there is none.
#[test]
fn one_font_lying_in_two_places_is_shown_once() {
    let bytes = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/fonts/LiberationSans-Bold.ttf")).expect("the font shipped with the repository");
    let root = std::env::temp_dir().join(format!("qym-fonts-twice-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (a, b) = (root.join("one"), root.join("two"));
    std::fs::create_dir_all(&a).expect("made");
    std::fs::create_dir_all(&b).expect("made");
    std::fs::write(a.join("Ours.ttf"), &bytes).expect("written");
    std::fs::write(b.join("SameFontOtherName.ttf"), &bytes).expect("written");

    let found = qymcad_ui_state::installed_fonts_in(&[a, b]);
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(found.len(), 1, "the same font in two folders came out as {} rows: {found:?}", found.len());
}

/// THE WALK DOES NOT HOLD UP THE FRAME.
///
/// Measured: reading the names out of every file under the font folders takes 536 ms for 232 faces. Done
/// inside a frame that is half a second of a frozen window each time the list is opened, and a machine with
/// a few thousand fonts is worse. So the walk starts and returns, and the window says it is looking.
#[test]
fn opening_the_list_does_not_freeze_the_window() {
    let mut picker = qymcad_ui_state::FontPicker::default();
    let started = std::time::Instant::now();
    picker.start_scan();
    let took = started.elapsed();
    assert!(took < std::time::Duration::from_millis(50), "starting the walk took {took:?} - it is being done in the frame, and the whole walk is around half a second");
    assert!(picker.scanning() || !picker.faces.is_empty(), "the walk was neither started nor already done");

    let waiting = std::time::Instant::now();
    while !picker.poll_scan() && waiting.elapsed() < std::time::Duration::from_secs(30) {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(!picker.faces.is_empty(), "the walk came back with nothing after {:?}", waiting.elapsed());
    assert!(!picker.scanning(), "the walk is over and the picker still says it is going");
}

/// TWO DIFFERENT FILES CALLING THEMSELVES THE SAME THING ARE TOLD APART.
///
/// Measured on the reporter's machine: `CaskaydiaCove Nerd Font Regular` appears twice, once as a `.ttf` and
/// once as an `.otf` "Complete" edition. They are not copies - different formats, different sizes, possibly
/// different coverage - so hiding one of them silently would be a lie. Two identical lines are no better: a
/// person cannot tell which is which, and after choosing has no idea what they took.
#[test]
fn two_files_with_the_same_name_are_told_apart_in_the_list() {
    let bytes = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/fonts/LiberationSans-Bold.ttf")).expect("the font shipped with the repository");
    let root = std::env::temp_dir().join(format!("qym-fonts-namesake-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("made");
    // the same names inside, different files: one carries a byte of padding, as a second edition would
    std::fs::write(root.join("First.ttf"), &bytes).expect("written");
    let mut other = bytes.clone();
    other.extend_from_slice(&[0u8; 16]);
    std::fs::write(root.join("Second edition.ttf"), &other).expect("written");

    let found = qymcad_ui_state::installed_fonts_in(std::slice::from_ref(&root));
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(found.len(), 2, "two different files came out as {} rows", found.len());
    let lines: Vec<String> = found.iter().map(qymcad_ui_state::font_row_text).collect();
    assert_ne!(lines[0], lines[1], "the two rows read exactly the same: {lines:?}");
    assert!(lines.iter().all(|l| l.contains("Liberation Sans")), "the family stopped being the first thing said: {lines:?}");
}
