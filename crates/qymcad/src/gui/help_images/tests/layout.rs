//! THE PICTURES OF THE LAYOUT AND OF THE BUTTONS OF A WORKBENCH: taken from the frames the hand makes in the window,
//! because what they show is a gesture - a panel carried on the diagram, a list unfolded by its heading - and a frame
//! drawn once from the fields would show a window no click reaches.

use super::*;
use crate::command_catalog::Named;
use crate::gui::hand::{Carry, Hand};

/// The words the interface shows for `key`.
fn word(key: &str) -> String {
    crate::i18n::tr(key)
}

/// Open the settings from the menu, if they are not open, and go to the section `key`; where the window stands.
fn settings_at(hand: &mut Hand, key: &str) -> Rect {
    let origin = egui::pos2(0.0, 0.0);
    if hand.window_at("win_settings").is_none() {
        hand.press_word(&word("menu-windows"), origin);
        hand.press_word(&word("win-settings"), origin);
    }
    // the list of the sections, found by its first one, and the name looked for down it: "Assembly" is also written in
    // the path at the top of the window
    let list = hand.written_at(&word("settings-sec-general")).map(|r| r.center() + egui::vec2(0.0, 70.0)).unwrap_or(egui::pos2(700.0, 200.0));
    hand.press_word(&word(key), list);
    // the window drawn out by its corner, so that the diagram stands whole in it
    let at = hand.window_at("win_settings").expect("the settings window is open");
    if at.height() < 600.0 {
        hand.drag_screen(at.max - egui::vec2(3.0, 3.0), at.max + egui::vec2(140.0, 200.0));
    }
    // the section from its top: a section opened after another keeps where that one was scrolled to
    hand.wheel3d(at.center(), 10);
    hand.frame(Vec::new());
    hand.window_at("win_settings").expect("the settings window is open")
}

/// Put the settings away the way they were opened: from the menu.
fn close_settings(hand: &mut Hand) {
    let origin = egui::pos2(0.0, 0.0);
    hand.press_word(&word("menu-windows"), origin);
    hand.press_word(&word("win-settings"), origin);
    hand.frame(Vec::new());
}

/// A PLATE IN A PART, the view drawn by the processor: a bar seen through needs a body under it.
fn scene() -> App {
    let mut app = plate(20.0);
    app.set.gpu_viewport = false;
    app
}

/// The hand at the scene: gone into the part by a double click in the tree, as a person goes, the plate in the middle.
fn into_the_part(app: &mut App) -> Hand<'_> {
    let mut hand = Hand::new(app);
    hand.frame(Vec::new());
    let part = hand.app.project.components.iter().find(|c| c.parent.is_some()).map(|c| crate::i18n::name(&c.name)).unwrap_or_default();
    let at = hand.written_at(&part);
    if let Some(at) = at {
        hand.double_click_screen(at.center());
    }
    hand.look_at([20.0, 15.0, 10.0], 7.0);
    hand.frame(Vec::new());
    hand
}

/// THE TREE CARRIED ONTO THE PROPERTIES on the diagram: the diagram as it comes, the tree held over the upper half of
/// the properties, the two in one strip at the right; and the window after it.
fn a_panel_carried() {
    let mut app = scene();
    let mut hand = into_the_part(&mut app);
    let win = settings_at(&mut hand, "settings-sec-layout");
    save("layout-carry/00.png", &hand.picture(win));
    let corner = hand.written_at(&word("settings-layout-menu")).expect("the diagram is drawn").center();
    let tree = hand.written_near(&word("panel-tree"), corner).expect("the tree stands on the diagram");
    let props = hand.written_near(&word("panel-props"), corner).expect("the properties stand on the diagram");
    let onto = props.center() - egui::vec2(0.0, 30.0);
    hand.carry(Carry { from: tree.center(), to: onto });
    save("layout-carry/01.png", &hand.picture(win));
    hand.let_go(onto);
    hand.frame(Vec::new());
    save("layout-carry/02.png", &hand.picture(win));
    close_settings(&mut hand);
    save("layout-window.png", &hand.picture(Rect::from_min_size(egui::pos2(0.0, 0.0), hand.screen())));
}

/// THE BAR OF TOOLS BY ITS PLACE: two columns at the left as it comes; two rows in the middle at the top, carried there
/// on the diagram; the same seen through, with the names of the groups.
fn the_bar_by_its_place() {
    let mut app = scene();
    let mut hand = into_the_part(&mut app);
    // the plate near and high in the view, so that the bar at the top lies over it
    hand.look_at([20.0, 15.0, -22.0], 11.0);
    hand.frame(Vec::new());
    let part = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1000.0, 560.0));
    save("layout-bar/00.png", &hand.picture(part));
    settings_at(&mut hand, "settings-sec-layout");
    let corner = hand.written_at(&word("settings-layout-menu")).expect("the diagram is drawn").center();
    let tools = hand.written_near(&word("panel-wbtools"), corner).expect("the bar of tools stands on the diagram");
    // onto the seam under the band of the menu, half of which stands below its middle: a strip of its own at the top
    hand.drag_screen(tools.center(), egui::pos2(corner.x + 200.0, corner.y + 10.0));
    hand.press_word(&word("bar-align-centre"), corner);
    close_settings(&mut hand);
    save("layout-bar/01.png", &hand.picture(part));
    hand.app.set.bar.opacity = 60;
    hand.app.set.bar.captions = qymcad_shell::Captions::Shown;
    for _ in 0..30 {
        hand.frame(Vec::new());
    }
    save("layout-bar/02.png", &hand.picture(part));
}

/// THE BUTTONS OF A WORKBENCH in its section: for the sketch the list folded under its heading, unfolded, and the
/// spline put into "More"; for each workbench its list unfolded.
fn the_buttons_of_a_workbench() {
    let mut app = scene();
    let mut hand = into_the_part(&mut app);
    let heading = word("settings-bar-buttons");
    let unfold = |hand: &mut Hand| {
        let at = hand.written_at(&heading).expect("the heading of the list is in the section").center();
        hand.press_word(&heading, at);
        hand.frame(Vec::new());
    };
    let win = settings_at(&mut hand, "settings-sec-sketch");
    save("bar-buttons/00.png", &hand.picture(win));
    unfold(&mut hand);
    save("bar-buttons/01.png", &hand.picture(win));
    save("bar-buttons-sketch.png", &hand.picture(win));
    let spline = crate::command_catalog::by_code("sketch.spline").map(|c| c.name()).expect("the spline is in the catalogue");
    let row = hand.written_at(&spline).expect("the spline is in the list");
    hand.press_word(&word("settings-bar-in-more"), egui::pos2(row.max.x + 300.0, row.center().y));
    hand.frame(Vec::new());
    save("bar-buttons/02.png", &hand.picture(win));
    for bench in ["part", "assembly"] {
        let win = settings_at(&mut hand, &format!("settings-sec-{bench}"));
        unfold(&mut hand);
        save(&format!("bar-buttons-{bench}.png"), &hand.picture(win));
    }
}

/// Every picture of the layout, in the language the other pictures speak.
pub(super) fn pictures() {
    crate::i18n::set_language("en");
    for dir in ["layout-carry", "layout-bar", "bar-buttons"] {
        std::fs::create_dir_all(img_dir().join(dir)).expect("the frame directory");
    }
    a_panel_carried();
    the_bar_by_its_place();
    the_buttons_of_a_workbench();
}

/// The pictures of the layout alone, without redrawing the rest of the help.
#[test]
#[ignore = "draws the pictures of the layout into docs/help/img"]
fn layout_pictures() {
    std::env::set_var("XDG_DATA_HOME", "/home/user/.local/share");
    pictures();
}
