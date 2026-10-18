use super::*;

/// The words a person reads for the catalogue entry `key`.
fn word(key: &str) -> String {
    crate::i18n::tr(key)
}

/// THE LAYOUT SET BY HAND IN THE MIDDLE OF THE WORK, in the settings window: the tree carried onto the properties at
/// the right, so the two stand one under the other; the bar of tools carried to the top, in two rows in the middle;
/// the button of a new sketch put into "More". The window shows each of it, the pencil is found in "More" and taken,
/// and the chain goes on with its own clicks in this layout. The step before it, the extrusion of the housing, is
/// checked first.
pub(super) fn set_by_hand(app: &mut App, problems: &mut Vec<String>) {
    check_all(app, "the housing was extruded", problems);
    let mut hand = Hand::new(app);
    let origin = egui::pos2(0.0, 0.0);
    hand.press_word(&word("menu-windows"), origin);
    hand.press_word(&word("win-settings"), origin);
    hand.press_word(&word("settings-sec-layout"), egui::pos2(700.0, 200.0));
    let Some(corner) = hand.written_at(&word("settings-layout-menu")).map(|r| r.center()) else {
        problems.push("[the layout] the diagram of the window is not in the settings".into());
        return;
    };
    // the tree onto the upper half of the properties: into their strip, before them
    let tree = hand.written_near(&word("panel-tree"), corner);
    let props = hand.written_near(&word("panel-props"), corner);
    if let Some(t) = tree {
        if let Some(p) = props {
            hand.drag_screen(t.center(), p.center() - egui::vec2(0.0, 30.0));
        }
    }
    if tree.is_none() || props.is_none() {
        problems.push("[the layout] the tree or the properties are not on the diagram".into());
    }
    // the bar of tools onto the seam at the top of the diagram: a strip of its own there
    if let Some(tools) = hand.written_near(&word("panel-wbtools"), corner) {
        let top_seam = egui::pos2(corner.x + 200.0, corner.y + 9.0 + 2.0);
        hand.drag_screen(tools.center(), top_seam);
    } else {
        problems.push("[the layout] the bar of tools is not on the diagram".into());
    }
    hand.press_word(&word("bar-align-centre"), corner);
    // the pencil of a new sketch into "More", in the section of the part
    hand.press_word(&word("settings-sec-part"), egui::pos2(700.0, 200.0));
    hand.press_word(&word("settings-bar-buttons"), egui::pos2(700.0, 400.0));
    let pencil = word("cmdname-new-sketch");
    match hand.written_at(&pencil) {
        Some(row) => {
            hand.press_word(&word("settings-bar-in-more"), egui::pos2(row.max.x + 300.0, row.center().y));
        }
        None => problems.push("[the layout] the new sketch is not in the list of the buttons of the part".into()),
    }
    hand.press_word(&word("menu-windows"), origin);
    hand.press_word(&word("win-settings"), origin);
    hand.frame(Vec::new());

    let tree_at = hand.laid_out("tree");
    let props_at = hand.laid_out("props");
    let under = match tree_at {
        Some(t) => props_at.is_some_and(|p| t.max.y <= p.min.y + 1.0 && (t.min.x - p.min.x).abs() < 1.0 && t.min.x > 600.0),
        None => false,
    };
    if !under {
        problems.push(format!("[the layout] the tree does not stand over the properties at the right: {tree_at:?} and {props_at:?}"));
    }
    let bar = hand.laid_out("wbtools");
    let buttons = hand.bar_buttons();
    let rows: std::collections::BTreeSet<i32> = buttons.iter().map(|b| b.min.y.round() as i32).collect();
    match bar {
        Some(b) if b.max.y < 120.0 && rows.len() == 2 => {
            let left = buttons.iter().map(|r| r.min.x).fold(f32::MAX, f32::min) - b.min.x;
            let right = b.max.x - buttons.iter().map(|r| r.max.x).fold(f32::MIN, f32::max);
            if (left - right).abs() > 40.0 {
                problems.push(format!("[the layout] the buttons at the top are not in the middle: {left} on the left, {right} on the right"));
            }
        }
        _ => problems.push(format!("[the layout] the bar of tools is not at the top in two rows: {bar:?}, rows at {rows:?}")),
    }
    // the pencil, gone from the bar, is found in "More" and taken
    if !hand.press_hint(&word("g-sketch-pick-hint")) || !hand.app.tools.picking.is_sketch_plane() {
        problems.push("[the layout] the pencil put into More is not taken from there".into());
    }
    hand.key(egui::Key::Escape);
    check_all(app, "the layout was set by hand", problems);
}
