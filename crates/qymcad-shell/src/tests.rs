use super::*;

/// THE SHELL DOES NOT KNOW A SINGLE WORKBENCH BY NAME.
///
/// This is the whole rule of the crate, and it can only be held by reading its files: a shell that
/// mentions a workbench is a shell that will grow a second mention, and then the workbench cannot be
/// taken out without editing it. The keys travel through as opaque strings for exactly this reason.
#[test]
fn the_shell_names_no_workbench() {
    /// A file of the crate and its text.
    struct Source {
        file: &'static str,
        text: &'static str,
    }
    for Source { file, text: src } in [
        Source { file: "lib.rs", text: include_str!("lib.rs") },
        Source { file: "draw.rs", text: include_str!("draw.rs") },
        Source { file: "bar.rs", text: include_str!("bar.rs") },
    ] {
        // THE CODE, WITHOUT THE PROSE. The doc comments name a tree and a toolbar on purpose - as examples
        // of what MIGHT stand in a place - and forbidding that would forbid explaining the file.
        let code: String =
            src.split("\n#[cfg(test)]").next().expect("the working part is the file above its tests").lines().map(|l| l.trim_start()).filter(|l| !l.starts_with("//")).collect::<Vec<_>>().join("\n");
        // WHOLE WORDS. A plain `contains("part")` found it inside "apart" and reported the shell for a word
        // it had not said - a needle that fires on prose teaches everyone to ignore it.
        let words: Vec<String> = code.split(|c: char| !c.is_alphanumeric() && c != '_').map(|w| w.to_lowercase()).collect();
        for name in ["sketch", "part", "assembly", "cam", "joint", "tree", "viewport", "toolbar", "properties", "sketcher", "status", "props", "wbtools", "menubar"] {
            assert!(!words.iter().any(|w| w == name), "{file} names `{name}` - a place is a KIND of container, and what stands in it is not the shell's business");
        }
    }
}

/// Two places at the left and one at the right, as a workbench would register them, plus the menu and the middle.
fn four() -> Shell {
    let mut s = Shell::default();
    s.put(Place::new("menu", Slot::Menu));
    s.put(Place::new("alpha", Slot::Left).sized(200.0, true));
    s.put(Place::new("gamma", Slot::Left));
    s.put(Place::new("beta", Slot::Right));
    s.put(Place::new("middle", Slot::Centre));
    s
}

/// A place stands where it was registered until somebody moves it; a strip of its own, after the ones registered at
/// its edge before it.
#[test]
fn a_place_stands_where_it_was_registered() {
    let s = four();
    assert_eq!(s.slot_of("alpha"), Some(Slot::Left));
    assert_eq!(s.slot_of("beta"), Some(Slot::Right));
    assert_eq!(s.strips(Edge::Left), vec![vec!["alpha"], vec!["gamma"]], "two places registered at one edge stand side by side, the first nearer the edge");
    assert_eq!(s.slot_of("nobody"), None, "an unregistered key has no place, rather than a default one");
}

/// THE MENU AND THE MIDDLE ARE NOT IN THE LAYOUT: no place for them, no moving, nothing kept for them.
#[test]
fn the_menu_and_the_middle_are_not_in_the_layout() {
    let mut s = four();
    assert_eq!(s.movable(), vec!["alpha", "gamma", "beta"], "the layout lists the places at the edges only");
    assert_eq!(s.place_of("menu"), None);
    assert_eq!(s.slot_of("menu"), Some(Slot::Menu), "the menu still stands above everything");
    s.move_to("menu", Target::NewStrip { edge: Edge::Bottom, before: 0 });
    s.set_seen("menu", Seen::Hidden);
    assert!(s.saved().is_empty(), "the menu cannot be moved or hidden");
    let mut kept = BTreeMap::new();
    kept.insert("menu".to_string(), PanelPlace { edge: Edge::Bottom, ..PanelPlace::default() });
    s.restore(kept);
    assert_eq!(s.slot_of("menu"), Some(Slot::Menu), "a record of a file cannot move the menu either");
}

/// A STACK: a place taken into the strip of another stands after it along the edge, in one strip.
#[test]
fn a_place_is_taken_under_another() {
    let mut s = four();
    s.move_to("beta", Target::Into { edge: Edge::Left, strip: 0, before: 1 });
    assert_eq!(s.strips(Edge::Left), vec![vec!["alpha", "beta"], vec!["gamma"]]);
    assert!(s.strips(Edge::Right).is_empty(), "nothing is left at the right");
    s.move_to("gamma", Target::Into { edge: Edge::Left, strip: 0, before: 0 });
    assert_eq!(s.strips(Edge::Left), vec![vec!["gamma", "alpha", "beta"]], "a place goes before the panel it is dropped before");
}

/// A STRIP OF ITS OWN, before the strip named or innermost.
#[test]
fn a_place_gets_a_strip_of_its_own() {
    let mut s = four();
    s.move_to("beta", Target::NewStrip { edge: Edge::Left, before: 0 });
    assert_eq!(s.strips(Edge::Left), vec![vec!["beta"], vec!["alpha"], vec!["gamma"]]);
    s.move_to("alpha", Target::NewStrip { edge: Edge::Left, before: 9 });
    assert_eq!(s.strips(Edge::Left), vec![vec!["beta"], vec!["gamma"], vec!["alpha"]], "a count past the strips puts it innermost");
}

/// A place taken to another edge forgets its sizes: they were the sizes of where it stood.
#[test]
fn a_moved_place_forgets_its_sizes() {
    let mut s = four();
    s.resize("alpha", Resize::Across(333.0));
    assert_eq!(s.place_of("alpha").and_then(|p| p.across), Some(333.0));
    s.move_to("alpha", Target::NewStrip { edge: Edge::Bottom, before: 0 });
    let p = s.place_of("alpha").expect("alpha has a place");
    assert_eq!(p.edge, Edge::Bottom);
    assert_eq!(p.across, None);
}

/// THE THICKNESS OF A STRIP is every panel's of it: dragged on one, the other keeps it too when it is moved out.
#[test]
fn a_strip_is_resized_whole() {
    let mut s = four();
    s.move_to("gamma", Target::Into { edge: Edge::Left, strip: 0, before: 1 });
    s.resize("gamma", Resize::Across(250.0));
    assert_eq!(s.place_of("alpha").and_then(|p| p.across), Some(250.0));
    assert_eq!(s.place_of("gamma").and_then(|p| p.across), Some(250.0));
    s.resize("alpha", Resize::Along(120.0));
    assert_eq!(s.place_of("alpha").and_then(|p| p.along), Some(120.0));
    assert_eq!(s.place_of("gamma").and_then(|p| p.along), None, "the length along is the panel's own");
}

/// Hiding keeps the place: shown again, it comes back where it stood.
#[test]
fn a_hidden_place_comes_back_where_it_stood() {
    let mut s = four();
    s.move_to("beta", Target::Into { edge: Edge::Left, strip: 1, before: 0 });
    s.set_seen("beta", Seen::Hidden);
    assert_eq!(s.place_of("beta").map(|p| p.seen), Some(Seen::Hidden));
    s.set_seen("beta", Seen::Shown);
    assert_eq!(s.strips(Edge::Left), vec![vec!["alpha"], vec!["beta", "gamma"]]);
}

/// Putting back returns exactly the registration and leaves nothing to keep.
#[test]
fn putting_back() {
    let mut s = four();
    s.move_to("alpha", Target::NewStrip { edge: Edge::Bottom, before: 0 });
    s.set_seen("beta", Seen::Hidden);
    s.reset();
    assert_eq!(s.slot_of("alpha"), Some(Slot::Left), "putting back must give the registered place");
    assert_eq!(s.place_of("beta").map(|p| p.seen), Some(Seen::Shown));
    assert!(s.saved().is_empty(), "nothing was moved, so nothing is worth saving");
}

/// ONLY THE CHANGES ARE SAVED, and they survive a restart. Saving the defaults too would freeze them: a place added
/// by a later version would never reach anybody who had opened the settings once.
#[test]
fn only_the_changes_are_saved() {
    let mut s = four();
    s.move_to("beta", Target::NewStrip { edge: Edge::Top, before: 0 });
    s.set_over("beta", Over::Over);
    assert_eq!(s.saved().keys().cloned().collect::<Vec<_>>(), vec!["beta".to_string()], "only the moved place is written");

    let mut fresh = four();
    fresh.restore(s.saved());
    assert_eq!(fresh.place_of("beta"), s.place_of("beta"), "the layout must survive a restart");
    assert_eq!(fresh.slot_of("alpha"), Some(Slot::Left));
}

/// A place put back by hand where it was registered leaves no record either.
#[test]
fn a_place_put_back_by_hand_leaves_no_record() {
    let mut s = four();
    s.move_to("beta", Target::NewStrip { edge: Edge::Top, before: 0 });
    s.move_to("beta", Target::NewStrip { edge: Edge::Right, before: 0 });
    assert!(s.saved().is_empty(), "beta stands where it was registered: {:?}", s.saved());
}

/// A saved place that nobody registers any more is dropped rather than kept as a ghost.
#[test]
fn a_saved_place_of_a_panel_that_is_gone_is_dropped() {
    let mut s = four();
    let mut kept = BTreeMap::new();
    kept.insert("alpha".to_string(), PanelPlace { edge: Edge::Top, ..PanelPlace::default() });
    kept.insert("gone".to_string(), PanelPlace::default());
    s.restore(kept);
    assert_eq!(s.slot_of("alpha"), Some(Slot::Top));
    assert_eq!(s.slot_of("gone"), None, "a place nobody registered must not come back from the file");
    assert_eq!(s.saved().len(), 1, "and it must not be written out again either");
}

/// Moving an unregistered key does nothing: the file cannot invent a panel.
#[test]
fn an_unknown_key_cannot_be_placed() {
    let mut s = four();
    s.move_to("nobody", Target::NewStrip { edge: Edge::Top, before: 0 });
    s.set_seen("nobody", Seen::Hidden);
    assert!(s.saved().is_empty(), "a key nobody registered must not enter the layout");
}

/// THE MIDDLE IS LAST. egui requires the central panel after every side one, and until now that was
/// held only by the order of statements in the frame.
#[test]
fn the_middle_comes_last() {
    assert_eq!(*Slot::ORDER.last().expect("the order is not empty"), Slot::Centre);
    assert_eq!(Slot::ORDER[0], Slot::Menu, "the menu is above everything");
}

/// A PLACE THAT MAY NOT BE HIDDEN stays shown: asked by the person, and asked by a record of a file.
#[test]
fn a_place_never_hidden_stays_shown() {
    let mut s = four();
    s.put(Place::new("delta", Slot::Top).never_hidden());
    s.set_seen("delta", Seen::Hidden);
    assert_eq!(s.place_of("delta").map(|p| p.seen), Some(Seen::Shown));
    assert!(!s.hideable("delta") && s.hideable("alpha") && !s.hideable("menu"));
    let mut kept = BTreeMap::new();
    kept.insert("delta".to_string(), PanelPlace { edge: Edge::Bottom, seen: Seen::Hidden, ..PanelPlace::default() });
    s.restore(kept);
    let p = s.place_of("delta").expect("delta has a place");
    assert_eq!(p.edge, Edge::Bottom, "the file moves it");
    assert_eq!(p.seen, Seen::Shown, "and cannot hide it");
}

/// The look of the bars in these checks: buttons of 30 points.
fn look(lines: BarLines, align: BarAlign) -> BarLook {
    BarLook { size: 30.0, lines, align, captions: Captions::Hidden }
}

/// `n` items of one group.
fn one_group(n: usize) -> Vec<BarItem> {
    vec![BarItem { group: 0 }; n]
}

/// A rectangle and the room it should lie in.
struct Within {
    rect: egui::Rect,
    room: egui::Rect,
}

/// Does the rectangle lie inside its room, within a hair?
fn inside(w: Within) -> bool {
    w.room.expand(0.01).contains_rect(w.rect)
}

/// An edge and the room a bar has at it.
struct Room {
    edge: Edge,
    room: egui::Rect,
}

/// AT A SIDE IN TWO COLUMNS: two buttons a row, top to bottom, every one whole in the room.
#[test]
fn a_bar_at_a_side_lays_two_columns() {
    let room = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(63.0, 600.0));
    let plan = plan_bar(Edge::Left, &look(BarLines::Two, BarAlign::Start), room, &one_group(9), MoreSlot::WhenFull);
    assert_eq!(plan.on_bar.len(), 9);
    assert!(plan.more.is_none() && plan.in_more.is_empty(), "everything fits: no More");
    let xs: Vec<f32> = plan.on_bar.iter().map(|p| p.rect.min.x).collect();
    assert_eq!(xs[..4], [10.0, 43.0, 10.0, 43.0], "two columns, the first at the edge of the room");
    assert_eq!(plan.on_bar[2].rect.min.y, 20.0 + 33.0, "the third button starts the second row");
    assert!(plan.on_bar.iter().all(|p| inside(Within { rect: p.rect, room })));
    assert_eq!(plan.used.width(), 63.0, "the bar is two buttons and a gap wide");
}

/// AT THE TOP IN TWO ROWS: buttons stand in pairs one over the other, left to right; a group starts a pair of its
/// own after a line.
#[test]
fn a_bar_at_the_top_lays_two_rows_with_its_groups_apart() {
    let room = egui::Rect::from_min_size(egui::pos2(0.0, 30.0), egui::vec2(1000.0, 63.0));
    let items = [BarItem { group: 0 }, BarItem { group: 0 }, BarItem { group: 0 }, BarItem { group: 1 }, BarItem { group: 1 }];
    let plan = plan_bar(Edge::Top, &look(BarLines::Two, BarAlign::Start), room, &items, MoreSlot::WhenFull);
    let r: Vec<egui::Rect> = plan.on_bar.iter().map(|p| p.rect).collect();
    assert_eq!(r[1].min, egui::pos2(0.0, 63.0), "the second button stands under the first");
    assert_eq!(r[2].min, egui::pos2(33.0, 30.0), "the third starts the next pair");
    assert_eq!(r[3].min.x, r[2].max.x + bar::GROUP_GAP, "the second group starts a pair of its own after the gap of a group");
    assert_eq!(plan.lines.len(), 1, "one line between two groups");
    let line = plan.lines[0];
    assert!(line.min.x > r[2].max.x && line.min.x < r[3].min.x, "the line stands in the gap between the groups");
    assert_eq!(plan.used.height(), 63.0);
}

/// WHAT DOES NOT FIT GOES INTO "MORE" at the end of the bar, and nothing is cut: every button and "More" are whole
/// in the room, and every item is either on the bar or in "More", in its order.
#[test]
fn what_does_not_fit_goes_into_more() {
    for Room { edge, room } in [
        Room { edge: Edge::Top, room: egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(200.0, 63.0)) },
        Room { edge: Edge::Bottom, room: egui::Rect::from_min_size(egui::pos2(0.0, 700.0), egui::vec2(170.0, 30.0)) },
        Room { edge: Edge::Left, room: egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(63.0, 150.0)) },
        Room { edge: Edge::Right, room: egui::Rect::from_min_size(egui::pos2(900.0, 0.0), egui::vec2(30.0, 100.0)) },
    ] {
        let lines = if room.width().min(room.height()) > 40.0 { BarLines::Two } else { BarLines::One };
        let items: Vec<BarItem> = (0..30).map(|i| BarItem { group: i / 7 }).collect();
        let plan = plan_bar(edge, &look(lines, BarAlign::Start), room, &items, MoreSlot::WhenFull);
        let more = plan.more.unwrap_or_else(|| panic!("{edge:?}: thirty buttons do not fit {room:?} and there is no More"));
        assert!(inside(Within { rect: more, room }), "{edge:?}: More {more:?} is cut by the room {room:?}");
        assert!(plan.on_bar.iter().all(|p| inside(Within { rect: p.rect, room })), "{edge:?}: a button is cut by the room");
        assert!(plan.on_bar.iter().all(|p| !p.rect.intersects(more.shrink(0.5))), "{edge:?}: More lies over a button");
        let mut all: Vec<usize> = plan.on_bar.iter().map(|p| p.item).chain(plan.in_more.iter().copied()).collect();
        assert!(!plan.in_more.is_empty() && plan.on_bar.len() > 1, "{edge:?}: {} on the bar, {} in More", plan.on_bar.len(), plan.in_more.len());
        all.dedup();
        assert_eq!(all, (0..30).collect::<Vec<_>>(), "{edge:?}: every item is on the bar or in More, in its order");
    }
}

/// THE BUTTONS STAND AT THE START, IN THE MIDDLE OR AT THE END of a bar they do not fill.
#[test]
fn a_bar_is_set_to_its_alignment() {
    let room = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1000.0, 30.0));
    let first = |align| plan_bar(Edge::Top, &look(BarLines::One, align), room, &one_group(4), MoreSlot::WhenFull).on_bar[0].rect.min.x;
    let width = 4.0 * 30.0 + 3.0 * bar::GAP;
    assert_eq!(first(BarAlign::Start), 0.0);
    assert_eq!(first(BarAlign::Centre), (1000.0 - width) / 2.0);
    assert_eq!(first(BarAlign::End), 1000.0 - width);
}

/// "MORE" STANDS ON A BAR THAT HAS ROOM when the person put commands into it: after the last button, in its free lane.
#[test]
fn more_stands_when_the_person_put_commands_into_it() {
    let room = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(63.0, 600.0));
    let plan = plan_bar(Edge::Left, &look(BarLines::Two, BarAlign::Start), room, &one_group(3), MoreSlot::Always);
    assert_eq!(plan.on_bar.len(), 3, "everything fits and stays on the bar");
    assert!(plan.in_more.is_empty());
    let more = plan.more.expect("More stands although everything fits");
    assert_eq!(more.min, egui::pos2(33.0, 33.0), "More takes the free lane after the third button");
}

/// THE CAPTIONS OF THE GROUPS: at a side a line across the bar before each group, the buttons after it; at the top a line
/// above each group, the buttons under it and the bar that much thicker.
#[test]
fn the_captions_of_the_groups_stand_before_and_above_them() {
    let items = [BarItem { group: 0 }, BarItem { group: 0 }, BarItem { group: 1 }];
    let with = BarLook { captions: Captions::Shown, ..look(BarLines::One, BarAlign::Start) };
    let side = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(30.0, 600.0));
    let plan = plan_bar(Edge::Left, &with, side, &items, MoreSlot::WhenFull);
    assert_eq!(plan.captions.len(), 2, "a caption a group");
    assert_eq!(plan.captions[0].rect.min.y, 0.0);
    assert_eq!(plan.on_bar[0].rect.min.y, bar::CAPTION + bar::GAP, "the first button stands under the caption of its group");
    assert!(plan.captions[1].rect.min.y >= plan.on_bar[1].rect.max.y && plan.captions[1].rect.max.y <= plan.on_bar[2].rect.min.y, "the second caption stands between the groups");
    assert!(plan.lines.is_empty(), "at a side the caption stands in place of the line");
    let top = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(600.0, 60.0));
    let plan = plan_bar(Edge::Top, &with, top, &items, MoreSlot::WhenFull);
    assert_eq!(plan.on_bar[0].rect.min.y, bar::CAPTION + bar::GAP, "at the top the buttons stand under the line of captions");
    assert_eq!(plan.captions[1].rect.min.x, plan.on_bar[2].rect.min.x, "a caption starts over its group");
    assert_eq!(with.across(Edge::Top), 30.0 + bar::CAPTION + bar::GAP);
    assert_eq!(with.across(Edge::Left), 30.0, "at a side the captions take no room across");
}
