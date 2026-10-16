use super::*;

/// THE RADIUS OF A ROUNDING of the housing typed again, as a person reopens a node of the tree and types a size.
fn rounded_by(app: &mut App, node: usize, radius: f64) {
    let nid = app.project.timeline[node].id;
    app.tree_action(1, node, nid, None, None);
    // the hand lets the window go when it is done: a rebuild asked for further down the chain happens where it is asked
    Hand::new(app).set("radius", radius).enter().settle().close_window();
}

/// A DIMENSION EDITED IN THE MIDDLE BREAKS A NODE, AND THE STATUS LINE LEADS THERE: the first rounding of the housing
/// made wider than the housing goes red; the counter of the status line says how many nodes are red; a click on it
/// leads to the rounding - straight when it is the only one, through the window of faults when more went with it; the
/// radius put back, the badge goes.
pub(super) fn a_broken_node_is_counted_and_led_to(app: &mut App, problems: &mut Vec<String>) {
    let step = "a rounding in the middle widened past the housing";
    let Some(ti) = app.project.timeline.iter().position(|n| matches!(n.kind, qymcad_core::feature::FeatureKind::Fillet { .. })) else {
        problems.push(format!("[{step}] the housing has no rounding to widen"));
        return;
    };
    let nid = app.project.timeline[ti].id;
    // where the person stood: the click leads into the part of the rounding, and the chain goes on from here after it
    let here = qymcad_ui_state::current_ctx_id(&app.active_path, &app.project);
    rounded_by(app, ti, 45.0);
    let red = app.project.regen_errors.len();
    if !app.project.regen_errors.contains_key(&nid) {
        problems.push(format!("[{step}] the rounding 45 wide on a housing 40 wide stayed green: {:?}", app.project.regen_errors));
    }
    let mut hand = Hand::new(app);
    match hand.status_badge(qymcad_ui_state::Tone::Error) {
        None => problems.push(format!("[{step}] the status line shows no red badge for {red} red nodes")),
        Some(badge) => {
            let said = badge.text.trim_start_matches(|c: char| !c.is_ascii_digit()).trim().to_string();
            if said != red.to_string() {
                problems.push(format!("[{step}] the badge says {said:?}, the document holds {red} red nodes"));
            }
            hand.click_screen(badge.rect.center());
            // more than one red: the window lists them, and the row of the rounding leads there
            if let Some(window) = hand.window_titled(&crate::i18n::tr("win-faults")) {
                let path = qymcad_ui_state::path_in_words(&hand.app.project.feature_breadcrumbs(nid));
                match hand.painted_words().into_iter().find(|w| window.contains(w.rect.center()) && w.text == path) {
                    Some(row) => {
                        hand.click_screen(row.rect.center());
                    }
                    None => problems.push(format!("[{step}] the window of faults has no row {path:?}")),
                }
            }
            if hand.app.chosen.sel != crate::gui::Sel::Feature(ti) {
                problems.push(format!("[{step}] a click on the red badge did not lead to the rounding"));
            }
            // done with the list: its cross puts it away, as a person puts it away before working on
            hand.close_titled(&crate::i18n::tr("win-faults"));
        }
    }
    hand.close_window();
    if qymcad_ui_state::current_ctx_id(&app.active_path, &app.project) != here {
        app.enter_component(here); // back where the person stood, as a click on the path above the canvas takes them
    }
    rounded_by(app, ti, 3.0);
    let mut hand = Hand::new(app);
    let stayed = hand.status_badge(qymcad_ui_state::Tone::Error).is_some();
    hand.close_window();
    if stayed {
        problems.push(format!("[{step}] the red badge stayed after the radius was put back: {:?}", app.project.regen_errors));
    }
    check_all(app, &format!("{step}, counted, led to and put back"), problems);
}
