//! THE WINDOW OF SUPPORT, opened from the menu of the help and from the About window: each way of the resource
//! (`assets/support/support.ron`) stands as its link, its address in the hint, and over it its own QR code. That a
//! picture holds its address is checked when it is made (`zbarimg`); here, that the window shows the picture of the list.
use crate::gui::hand::Hand;
use crate::gui::{App, WinKind};

/// Open the menu "Help" and press its item `item`, as a person does.
fn press_in_help(hand: &mut Hand, item: &str) -> bool {
    let origin = egui::pos2(0.0, 0.0);
    let menu = crate::i18n::tr("menu-help");
    hand.press_word(&menu, origin);
    let heading = hand.written_at(&menu).map_or(origin, |r| r.center());
    hand.press_word(item, heading + egui::vec2(20.0, 40.0))
}

/// The share of the pixels of `shown` - a picture of the window, a point a pixel - that differ in light from the PNG
/// `bytes` shown at half its size: each point is the middle of a square of four pixels of the PNG.
fn differs_from(shown: &egui::ColorImage, bytes: &[u8]) -> f64 {
    let Ok(png) = image::load_from_memory(bytes).map(|i| i.to_luma8()) else { return 1.0 };
    let [w, h] = shown.size;
    let mut off = 0;
    for y in 0..h {
        for x in 0..w {
            let px = shown.pixels[y * w + x];
            let light = (px.r() as u32 + px.g() as u32 + px.b() as u32) / 3 > 127;
            let own = png.get_pixel_checked(2 * x as u32 + 1, 2 * y as u32 + 1).is_some_and(|l| l.0[0] > 127);
            off += usize::from(light != own);
        }
    }
    off as f64 / (w * h).max(1) as f64
}

/// THE PICTURES THE LAST FRAME DREW IN `window`: the rectangles filled with a texture of their own.
fn pictures_in(hand: &Hand, window: egui::Rect) -> Vec<egui::Rect> {
    fn walk(s: &egui::Shape, window: egui::Rect, out: &mut Vec<egui::Rect>) {
        match s {
            egui::Shape::Rect(r) if r.fill_texture_id() != egui::TextureId::default() && window.contains_rect(r.rect) && r.rect.width() > 100.0 => out.push(r.rect),
            egui::Shape::Vec(v) => v.iter().for_each(|x| walk(x, window, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for cs in hand.shapes_drawn() {
        walk(&cs.shape, window, &mut out);
    }
    out
}

#[test]
fn the_window_of_support_opens_from_the_help_and_from_about() {
    let mut app = App::default();
    let mut hand = Hand::new(&mut app);
    let mut problems = Vec::new();

    // the menu of the help
    if !press_in_help(&mut hand, &crate::i18n::tr("help-support")) {
        problems.push("the menu of the help has no item to support the author".to_string());
    }
    hand.frame(Vec::new());
    if !hand.app.win.is(WinKind::Support) || hand.window_at("win_support").is_none() {
        problems.push("the item of the help opened no window of support".to_string());
    }
    hand.close_titled(&crate::i18n::tr("win-support"));
    if hand.app.win.is(WinKind::Support) {
        hand.app.win.close(WinKind::Support);
        problems.push("the window of support does not close by its cross".to_string());
    }

    // the About window, its button
    press_in_help(&mut hand, &crate::i18n::tr("help-about"));
    if !hand.app.win.is(WinKind::About) {
        problems.push("the item of the help opened no About window".to_string());
    }
    let about = hand.window_at("win_about").map_or(egui::pos2(0.0, 0.0), |r| r.center());
    if !hand.press_word(&crate::i18n::tr("about-support"), about) {
        problems.push("the About window has no button to support the author".to_string());
    }
    hand.frame(Vec::new());
    if !hand.app.win.is(WinKind::Support) {
        problems.push("the button of the About window opened no window of support".to_string());
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn each_way_of_the_resource_shows_its_link_its_address_and_its_code() {
    let ways = crate::support::ways();
    assert_eq!(ways.len(), 3, "the resource lists the three ways: {ways:?}");
    let mut app = App::default();
    let mut hand = Hand::new(&mut app);
    press_in_help(&mut hand, &crate::i18n::tr("help-support"));
    hand.frame(Vec::new()).frame(Vec::new());
    let window = hand.window_at("win_support").expect("the window of support is open");
    let pictures = pictures_in(&hand, window);
    // the hand comes into the window before it rests on a link
    hand.hint_at(window.center());
    let mut problems = Vec::new();
    for way in &ways {
        let Some(label) = hand.written_near(&way.label, window.center()).filter(|r| window.contains_rect(*r)) else {
            problems.push(format!("{}: no link in the window", way.label));
            continue;
        };
        let hint = hand.hint_at(label.center());
        if !hint.contains(&way.url) {
            problems.push(format!("{}: the link names {hint:?}, the resource says {:?}", way.label, way.url));
        }
        // its picture stands right above it
        let Some(picture) =
            pictures.iter().filter(|p| p.max.y <= label.min.y && p.x_range().contains(label.center().x)).min_by(|a, b| (label.min.y - a.max.y).total_cmp(&(label.min.y - b.max.y))).copied()
        else {
            problems.push(format!("{}: no picture above the link", way.label));
            continue;
        };
        let shown = hand.picture(picture);
        for other in &ways {
            let off = differs_from(&shown, crate::support::picture(&other.qr).unwrap_or_default());
            let own = other.qr == way.qr;
            // measured: the own picture differs by its edge row alone, 0.13 %; another code by 24-26 %
            if own && off > 0.03 {
                problems.push(format!("{}: the picture above the link differs from {} by {:.1} %", way.label, other.qr, off * 100.0));
            }
            if !own && off < 0.15 {
                problems.push(format!("{}: the picture above the link is as close to {} ({:.1} %) as to its own", way.label, other.qr, off * 100.0));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
