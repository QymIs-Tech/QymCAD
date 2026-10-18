//! THE BAR OF TOOLS AND THE LAYOUT, SET UP BY HAND: the bar laid in rows at the top and gathered into "More" when the
//! window is narrow, its buttons of any size whole and sharp, the panels carried on the diagram of the settings and
//! hidden by their eye and from the menu "View", the buttons of a workbench put in order and into "More" - and all of
//! it kept for the next start.
use qymcad::{pos2, vec2, Arg, Key, Machine, Modifiers, PointerButton, Rect, Session};
use qymcad_acceptance::{build, probe};

/// What the record of the settings of a start says, written as the record writes it: the look of the bar of tools,
/// and where the panels stand.
struct Record<'a> {
    bar: &'a str,
    place: &'a str,
}

/// THE RECORD OF THE SETTINGS with the bar of tools looking and standing as `kept` says.
fn machine_with(kept: Record) -> Machine {
    let Record { bar, place } = kept;
    let mut m = Machine::default();
    let record = m.kept.record("settings").expect("the factory machine keeps its settings").to_string();
    let factory_bar = "bar:(size:34.0,lines:Two,align:Start,opacity:100,captions:Hidden)";
    assert!(record.contains(factory_bar) && record.contains("layout:{}"), "the record of the settings is not shaped as expected: {record}");
    let record = record.replacen(factory_bar, &format!("bar:({bar})"), 1).replacen("layout:{}", &format!("layout:{{{place}}}"), 1);
    m.kept.set_record("settings", record);
    m
}

/// Close the program, keeping what it keeps between runs; a question about unsaved work is answered "don't save".
fn quit_keeping(s: Session) -> qymcad::Kept {
    match s.quit() {
        Ok(kept) => kept,
        Err(mut s) => {
            let dont_save = s.word("nav-dont-save");
            s.press_word(&dont_save);
            s.quit().unwrap_or_else(|_| panic!("the window was closed and would not give what it keeps"))
        }
    }
}

/// The rows the buttons stand in, by the top of each: buttons whose tops differ by less than half a button share one.
fn rows_of(buttons: &[Rect]) -> Vec<f32> {
    let mut tops: Vec<f32> = Vec::new();
    for b in buttons {
        if !tops.iter().any(|t| (t - b.min.y).abs() < b.height() / 2.0) {
            tops.push(b.min.y);
        }
    }
    tops
}

probe! {
    /// THE BAR AT THE TOP IN TWO ROWS IN THE MIDDLE: the buttons of the part stand in two rows, as far from the left
    /// end of the bar as from the right; the window made narrow, what does not fit goes into "More", and a command
    /// pressed there starts as from its button.
    fn the_bar_at_the_top_lays_two_rows_in_the_middle_and_gathers_the_rest_in_more() {
        let mut s = Session::start_on(machine_with(Record { bar: "size:34.0,lines:Two,align:Centre,opacity:100", place: "\"wbtools\":(edge:Top,strip:100)" }));
        s.key(Key::Escape);
        build::into_the_first_part(&mut s);
        let bar = s.panel("wbtools").expect("the bar of tools is drawn");
        let buttons = s.bar_buttons();
        assert!(buttons.len() > 20, "the bar of the part shows {} buttons", buttons.len());
        assert_eq!(rows_of(&buttons).len(), 2, "the buttons stand in two rows: {:?}", rows_of(&buttons));
        assert!(buttons.iter().all(|b| bar.contains_rect(*b)), "a button stands out of its bar {bar:?}");
        let left = buttons.iter().map(|b| b.min.x).fold(f32::MAX, f32::min) - bar.min.x;
        let right = bar.max.x - buttons.iter().map(|b| b.max.x).fold(f32::MIN, f32::max);
        assert!((left - right).abs() < 12.0, "the buttons are not in the middle of the bar: {left} on the left, {right} on the right");
        assert!(s.more_buttons().is_empty(), "everything fits a window of 1280 and there is a More");

        s.resize(vec2(640.0, 800.0));
        let bar = s.panel("wbtools").expect("the bar of tools is drawn");
        let more = s.more_buttons();
        assert_eq!(more.len(), 1, "a narrow window gathers the rest of the bar into one More: {more:?}");
        assert!(bar.contains_rect(more[0]) && s.bar_buttons().iter().all(|b| bar.contains_rect(*b)), "a button or More is cut by the narrow bar");
        // the last command of the bar is in More now; found there by its hint and pressed, it starts or says why not
        let before = s.status();
        let pattern = s.word("tb-circ-array-body-hint");
        s.press_hint(&pattern);
        assert!(!s.in_hand().is_empty() || s.status() != before, "the circular pattern pressed in More neither started nor said why");
    }
}

/// THE PICTURE'S LIGHTNESS at a point of the screen, 0 to 255.
fn light(p: &qymcad::Picture, at: qymcad::Pos2) -> i32 {
    let i = 4 * (at.y as usize * p.width + at.x as usize);
    (i32::from(p.rgba[i]) + i32::from(p.rgba[i + 1]) + i32::from(p.rgba[i + 2])) / 3
}

/// HOW SHARP AN ICON IS DRAWN: the steepest step of lightness between two neighbouring pixels inside `at`. An icon
/// drawn for its size steps from its line to the ground in one pixel; one stretched from a smaller picture fades over
/// several, and its steepest step is a fraction of that.
fn sharpness(p: &qymcad::Picture, at: Rect) -> i32 {
    let mut best = 0;
    for y in at.min.y as usize..at.max.y as usize {
        for x in at.min.x as usize..(at.max.x as usize).saturating_sub(1) {
            best = best.max((light(p, pos2(x as f32, y as f32)) - light(p, pos2(x as f32 + 1.0, y as f32))).abs());
        }
    }
    best
}

probe! {
    budget = 600;
    /// BUTTONS OF 20, 32 AND 48 POINTS at a side and at the top: every button of that size, whole in its bar, and its
    /// icon drawn sharp - as sharp at 48 as at 20.
    fn buttons_of_every_size_are_whole_and_sharp() {
        let mut faults = Vec::new();
        for size in [20.0, 32.0, 48.0] {
            for edge in ["Left", "Top"] {
                let mut s = Session::start_on(machine_with(Record { bar: &format!("size:{size:.1},lines:Two,align:Start,opacity:100"), place: &format!("\"wbtools\":(edge:{edge},strip:100)") }));
                s.key(Key::Escape);
                build::into_the_first_part(&mut s);
                let bar = s.panel("wbtools").expect("the bar of tools is drawn");
                let buttons = s.bar_buttons();
                if buttons.iter().any(|b| (b.width() - size).abs() > 0.5) {
                    faults.push(format!("{size} at {edge}: a button is {:?}", buttons.iter().find(|b| (b.width() - size).abs() > 0.5)));
                }
                if let Some(cut) = buttons.iter().chain(s.more_buttons().iter()).find(|b| !bar.expand(0.5).contains_rect(**b)) {
                    faults.push(format!("{size} at {edge}: the button {cut:?} is cut by the bar {bar:?}"));
                }
                let picture = s.snapshot();
                let steep = buttons.iter().take(6).map(|b| sharpness(&picture, b.shrink(size * 0.15))).max().unwrap_or(0);
                if steep < 90 {
                    faults.push(format!("{size} at {edge}: the icons are blurred, the steepest step of lightness is {steep}"));
                }
            }
        }
        assert!(faults.is_empty(), "the bar of tools at sizes of its buttons:\n{}", faults.join("\n"));
    }
}

/// Open the settings at the section `key`.
fn settings_at(s: &mut Session, key: &str) {
    let windows = s.word("menu-windows");
    let settings = s.word("win-settings");
    s.menu(&[&windows, &settings]);
    // the list of sections, found by its first one: the name of a section may be written elsewhere too - "Sketch" heads
    // the properties of an open sketch
    let list = s.find(&s.word("settings-sec-general"), pos2(700.0, 200.0)).map(|r| r.center()).unwrap_or(pos2(700.0, 200.0));
    let section = s.word(key);
    s.press_word_near(&section, list);
}

/// WHERE `text` IS WRITTEN IN THE SETTINGS, the section scrolled down by the wheel until it comes into sight, as a
/// person scrolls.
fn in_settings(s: &mut Session, text: &str) -> Option<Rect> {
    // the wheel turned over the heading of the list of buttons, inside the section that scrolls
    let over = s.find(&s.word("settings-bar-buttons"), pos2(700.0, 400.0)).map(|r| r.center()).unwrap_or(pos2(700.0, 400.0));
    for _ in 0..20 {
        if let Some(r) = s.find(text, pos2(700.0, 300.0)) {
            return Some(r);
        }
        s.wheel(over, vec2(0.0, -120.0), Modifiers::NONE);
    }
    None
}

/// Put the settings away.
fn close_settings(s: &mut Session) {
    let title = s.word("win-settings");
    s.close_window(&title);
}

probe! {
    /// THE PROPERTIES CARRIED UNDER THE TREE on the diagram of the settings, the bar of the path hidden by its eye and
    /// the tree from the menu "View" - each seen in the window at once, and the layout there after a restart.
    fn the_panels_are_carried_and_hidden_by_hand() {
        let mut s = Session::start();
        s.key(Key::Escape);
        build::into_the_first_part(&mut s);
        settings_at(&mut s, "settings-sec-layout");
        let corner = s.find(&s.word("settings-layout-menu"), pos2(640.0, 300.0)).expect("the diagram of the window is drawn").center();
        let props = s.word("panel-props");
        let tree = s.word("panel-tree");
        let from = s.find(&props, corner).expect("the properties stand on the diagram").center();
        let to = s.find(&tree, corner).expect("the tree stands on the diagram").center() + vec2(0.0, 40.0);
        s.drag(from, to, PointerButton::Primary, Modifiers::NONE);
        let path = s.word("panel-toolbar");
        let eye = s.word_with("settings-layout-hide", Arg { name: "name", value: &path });
        s.press_hint(&eye);
        close_settings(&mut s);
        let t = s.panel("tree").expect("the tree is drawn");
        let p = s.panel("props").expect("the properties are drawn");
        assert!(p.min.y >= t.max.y - 1.0 && (p.min.x - t.min.x).abs() < 1.0, "the properties do not stand under the tree: {t:?} and {p:?}");
        assert!(s.panel("toolbar").is_none(), "the bar of the path hidden by its eye is still drawn");
        let view = s.word("menu-view");
        let panels = s.word("menu-panels");
        s.menu(&[&view, &panels, &tree]);
        s.key(Key::Escape);
        assert!(s.panel("tree").is_none(), "the tree unticked in the menu View is still drawn");
        let kept = quit_keeping(s);
        let mut s = Session::start_on(Machine { kept, ..Machine::default() });
        s.key(Key::Escape);
        build::into_the_first_part(&mut s);
        assert!(s.panel("tree").is_none() && s.panel("toolbar").is_none(), "after a restart the hidden panels came back");
        let p = s.panel("props").expect("the properties are drawn");
        assert!(p.min.x < 200.0, "after a restart the properties left the left edge: {p:?}");
    }
}

probe! {
    /// THE BUTTONS OF THE SKETCH SET UP BY HAND: the circle carried before the line in the list of its section, the
    /// spline put into "More" - on the bar the circle stands before the line and the spline is gone, in "More" the
    /// spline is there and starts from it, and after a restart the set is the same.
    fn the_buttons_of_the_sketch_are_put_in_order_and_into_more() {
        let mut s = build::empty_sketch();
        settings_at(&mut s, "settings-sec-sketch");
        // the list lies folded under its heading, as the colours of a scheme do
        let heading = s.word("settings-bar-buttons");
        let line_name = s.command_name("sketch.line");
        assert!(s.find(&line_name, pos2(700.0, 300.0)).is_none(), "the list of the buttons is open before its heading is pressed");
        s.press_word_near(&heading, pos2(700.0, 400.0));
        let circle_hint = s.word("tb-circle-hint");
        let line_hint = s.word("tb-line-hint");
        let spline_hint = s.word("tb-spline-hint");
        // the rows of the list are the names of the commands, as the search shows them
        let row = |s: &mut Session, code: &str| -> Rect {
            let name = s.command_name(code);
            in_settings(s, &name).unwrap_or_else(|| panic!("{name:?} is not in the list of the buttons; on screen: {:?}", s.words()))
        };
        let circle = row(&mut s, "sketch.circle");
        let line = row(&mut s, "sketch.line");
        s.drag(circle.center(), pos2(circle.center().x, line.min.y + 2.0), PointerButton::Primary, Modifiers::NONE);
        let spline = row(&mut s, "sketch.spline");
        let in_more = s.word("settings-bar-in-more");
        s.press_word_near(&in_more, pos2(spline.max.x + 300.0, spline.center().y));
        close_settings(&mut s);
        let check = |s: &mut Session, when: &str| {
            let at_circle = s.find_hint(&circle_hint).unwrap_or_else(|| panic!("{when}: no circle on the bar"));
            let at_line = s.find_hint(&line_hint).unwrap_or_else(|| panic!("{when}: no line on the bar"));
            let before = at_circle.y < at_line.y - 1.0 || ((at_circle.y - at_line.y).abs() <= 1.0 && at_circle.x < at_line.x);
            assert!(before, "{when}: the circle {at_circle:?} does not stand before the line {at_line:?}");
            let on_bar: Vec<qymcad::Pos2> = s.bar_buttons().iter().map(|r| r.center()).collect();
            let spline_at = s.find_hint(&spline_hint).unwrap_or_else(|| panic!("{when}: the spline is neither on the bar nor in More"));
            assert!(!on_bar.contains(&spline_at), "{when}: the spline is still on the bar");
            s.click(spline_at);
            assert_eq!(s.drawing_tool(), 9, "{when}: the spline pressed in More is not in hand");
            s.key(Key::Escape);
        };
        check(&mut s, "set by hand");
        let kept = quit_keeping(s);
        let mut s = Session::start_on(Machine { kept, ..Machine::default() });
        s.key(Key::Escape);
        build::into_the_first_part(&mut s);
        let xy = s.word("plane-xy-table");
        s.press_word(&xy);
        check(&mut s, "after a restart");
    }
}
