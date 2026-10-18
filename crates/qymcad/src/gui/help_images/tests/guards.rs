//! The checks of the pictures of the help: each one an article names exists, decodes, shows something, and reaches
//! the window with its caption.

use super::*;

/// EVERY PICTURE MENTIONED IN AN ARTICLE EXISTS.
///
/// A broken picture looks like a fault in the program rather than like a forgotten file. And there is no
/// other way to notice it than by opening that very article in that very language.
#[test]
fn every_image_in_the_articles_exists() {
    let mut missing = Vec::new();
    for l in crate::help::languages() {
        for a in crate::help::articles(&l) {
            let md = crate::help::article(&a).expect("the article");
            for line in md.lines() {
                let t = line.trim();
                let Some(rest) = t.strip_prefix("![") else { continue };
                let Some((_, p)) = rest.split_once("](") else { continue };
                let Some(path) = p.strip_suffix(')') else { continue };
                let ok = if path.ends_with('/') { !crate::help::frames(path).is_empty() } else { crate::help::image(path).is_some() };
                if !ok {
                    missing.push(format!("{l}/{a} -> {path}"));
                }
            }
        }
    }
    assert!(missing.is_empty(), "an article refers to a picture that does not exist ({}):\n{}", missing.len(), missing.join("\n"));
}

/// AND EVERY PICTURE IS A REAL PNG OF A SENSIBLE SIZE.
///
/// "The file exists" is half a check: a truncated write, an empty file, an accidentally committed
/// placeholder all pass straight through it, while the window shows an empty space.
#[test]
fn every_image_decodes_and_is_big_enough() {
    let mut all: Vec<String> = Vec::new();
    collect_paths(&mut all);
    assert!(all.len() > 10, "suspiciously few pictures in the help: {}", all.len());
    for p in all {
        let bytes = crate::help::image(&p).unwrap_or_else(|| panic!("no file {p}"));
        let img = image::load_from_memory(bytes).unwrap_or_else(|e| panic!("{p} does not read as a picture: {e}"));
        let (w, h) = (image::GenericImageView::dimensions(&img).0, image::GenericImageView::dimensions(&img).1);
        // BY AREA AND BY EACH SIDE rather than "no narrower than 320". The rule was written for scenes
        // with a body - those are wide; a shot of a side panel is tall and narrow (300x420), and the old
        // measure rejected it although everything in it can be made out perfectly well.
        assert!(w >= 240 && h >= 200 && w * h >= 96_000, "picture {p} is too small ({w}x{h}) - it cannot be made out in an article");
    }
}

/// Every picture path (the frames of an animation are expanded by name).
fn collect_paths(out: &mut Vec<String>) {
    for l in crate::help::languages() {
        for a in crate::help::articles(&l) {
            let md = crate::help::article(&a).expect("the article");
            for line in md.lines() {
                let t = line.trim();
                let Some(rest) = t.strip_prefix("![") else { continue };
                let Some((_, p)) = rest.split_once("](") else { continue };
                let Some(path) = p.strip_suffix(')') else { continue };
                if path.ends_with('/') {
                    out.extend(crate::help::frames(path));
                } else {
                    out.push(path.to_string());
                }
            }
        }
    }
    out.sort();
    out.dedup();
}

/// AND THERE IS SOMETHING IN THE PICTURE: it is not one colour and not one flat blot.
///
/// Exactly the fault that was caught by eye and only by chance: the rasteriser stopped assembling the
/// viewport texture, and the body was drawn SOLID WHITE. The file was in place, the size was right, the
/// PNG decoded - every earlier guard stayed green. The colours are counted: a real scene has hundreds of
/// them (light, antialiasing, shadows), a flat blot has a handful.
#[test]
fn every_image_actually_shows_something() {
    let mut all: Vec<String> = Vec::new();
    collect_paths(&mut all);
    for p in all {
        let bytes = crate::help::image(&p).unwrap_or_else(|| panic!("no file {p}"));
        let img = image::load_from_memory(bytes).expect("the picture decodes").to_rgb8();
        let total = img.pixels().len();
        let mut seen: std::collections::HashMap<[u8; 3], usize> = std::collections::HashMap::new();
        for px in img.pixels() {
            *seen.entry(px.0).or_default() += 1;
        }
        let top = seen.values().copied().max().unwrap_or(total);
        // THE THRESHOLDS ON FILL ARE DELIBERATELY WEAK, and that is not laziness. The pictures differ
        // enormously: a scene with a body has hundreds of colours, while a frame of two lines in a sketch
        // has six, and 99 % of the area is background. A strict threshold would reject correct pictures,
        // and a guard that complains about the correct gets switched off altogether. 24 colours and 97 %
        // were tried - the array of posts and the line extension fell out.
        assert!(seen.len() >= 3, "{p}: only {} colours - the picture is empty", seen.len());
        assert!(top * 1000 / total <= 998, "{p}: {} per mille of the area is one colour - there is nothing in the picture", top * 1000 / total);
        // PURE WHITE NEVER OCCURS HERE: the scheme is dark and the background of the scenes is
        // transparent. A lot of white is exactly the case where the rasteriser lost the texture and
        // filled the body with it.
        let white = seen.get(&[255, 255, 255]).copied().unwrap_or(0);
        assert!(white * 100 / total <= 15, "{p}: {}% pure white - the texture seems to have been lost", white * 100 / total);
    }
}

/// A PICTURE REACHES THE SCREEN while its markup does not.
///
/// The parser can be mended and then left unwired from the drawing; the window would then show either
/// emptiness or a raw `![...](...)` - and both read as a fault in the program.
#[test]
fn the_window_draws_an_image_and_its_caption() {
    // THE LANGUAGE IS PINNED. The frame is drawn in the current language while the caption is taken
    // from the article - and between those two steps a neighbouring test managed to switch the language:
    // one language on screen, a word of another in the expectation. The test went red not from a fault
    // but from another run alongside it.
    let prev = crate::i18n::language();
    crate::i18n::set_language("ru");
    crate::help::set_lang("ru");
    let mut app = App::default();
    app.open_help("part/08-hole");
    let texts = crate::gui::screen_keys::tests::frame_text(&mut app, |a, c| a.help_window(c));
    assert!(!texts.iter().any(|t| t.contains("![") || t.contains("img/")), "the markup of a picture reached the screen raw: {:?}", texts.iter().filter(|t| t.contains("img/")).collect::<Vec<_>>());
    // THE CAPTION IS TAKEN FROM THE ARTICLE ITSELF rather than written out as a word: the language of the
    // help is shared across the run and neighbouring tests switch it - a string nailed down in one
    // language would go red every other time.
    let md = crate::help::article("part/08-hole").expect("the article");
    let caption = md.lines().find_map(|l| l.trim().strip_prefix("![").and_then(|r| r.split_once("](")).map(|(c, _)| c.to_string())).expect("the caption of the picture in the article");
    let word = caption.split_whitespace().max_by_key(|w| w.chars().count()).expect("a word of the caption");
    let found = texts.iter().any(|t| t.contains(word));
    crate::i18n::set_language(&prev);
    crate::help::set_lang("");
    assert!(found, "there is no caption under the picture (looked for \"{word}\"): {texts:?}");
}
