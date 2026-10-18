//! THE PANELS STAND WHERE THE LAYOUT PUTS THEM: every panel at every edge, alone or in a stack with another, shown or
//! hidden, in a sketch, a part and an assembly.
//!
//! The layout is the one an earlier run kept - the record of the settings edited between two runs - so that a cell
//! costs one start and not a dozen drags; dragging on the diagram of the layout is checked on its own.
//!
//! What is looked at is what a person sees: where the panel was laid out, that the view takes what is left, that no
//! panel lies over another and that every button of the workbench is seen whole on its bar or stands in its "More"
//! list. Failures accumulate over the cells and are reported together.
use qymcad::{Machine, Rect, Session};
use qymcad_acceptance::{build, probe};

/// The edges of the window a panel can stand at.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

const EDGES: [Edge; 4] = [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom];

/// Where the panel stands at its edge.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Company {
    /// The only panel at its edge.
    Alone,
    /// First in a stack, the partner after it along the edge.
    Stacked,
}

/// Shown or hidden.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Seen {
    Shown,
    Hidden,
}

/// The workbench the cell is looked at in.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Bench {
    Sketch,
    Part,
    Assembly,
}

const BENCHES: [Bench; 3] = [Bench::Sketch, Bench::Part, Bench::Assembly];

/// The panels of the matrix, by what a person calls them.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Panel {
    Tree,
    Properties,
    Toolbar,
    CommandBar,
    Status,
    WorkbenchTools,
}

/// One cell of the matrix.
#[derive(Clone, Copy, Debug)]
struct Cell {
    panel: Panel,
    edge: Edge,
    company: Company,
    seen: Seen,
    bench: Bench,
}

/// A panel and where the layout puts it, as the record of the settings says it.
struct Put {
    key: &'static str,
    edge: Edge,
    order: u32,
    seen: Seen,
}

/// THE STRIP THE CELL'S PANELS ARE PUT IN: one of their own, innermost at the edge, so that the panel stands alone or
/// with its partner and with nothing else.
const OWN_STRIP: u32 = 100;

/// The key a panel is registered under.
fn key(panel: Panel) -> &'static str {
    match panel {
        Panel::Tree => "tree",
        Panel::Properties => "props",
        Panel::Toolbar => "toolbar",
        Panel::CommandBar => "command_bar",
        Panel::Status => "status",
        Panel::WorkbenchTools => "wbtools",
    }
}

/// The panel that shares the edge with `panel` in a stack.
fn partner(panel: Panel) -> Panel {
    match panel {
        Panel::Properties => Panel::Toolbar,
        _ => Panel::Properties,
    }
}

/// THE STATUS LINE IS PINNED to the bottom and in sight, as the menu is at the top: a record that puts it elsewhere or
/// hides it is not followed, and it stands alone in its strip.
fn pinned(panel: Panel) -> bool {
    panel == Panel::Status
}

/// THE RECORD OF THE SETTINGS with the panels put as `puts` says, written over the factory one.
fn machine_with(puts: &[Put]) -> Machine {
    let mut m = Machine::default();
    let edge = |e: Edge| format!("{e:?}");
    let seen = |s: Seen| format!("{s:?}");
    let places: Vec<String> = puts.iter().map(|p| format!("{:?}:(edge:{},strip:{OWN_STRIP},order:{},seen:{})", p.key, edge(p.edge), p.order, seen(p.seen))).collect();
    let record = m.kept.record("settings").expect("the factory machine keeps its settings").to_string();
    let empty = "layout:{}";
    assert!(record.contains(empty), "the record of the settings holds no empty layout to replace: {record}");
    m.kept.set_record("settings", record.replacen(empty, &format!("layout:{{{}}}", places.join(",")), 1));
    m
}

/// Start the program with the layout of `cell` and go to its workbench, with its command in hand for the command bar.
fn open(cell: Cell) -> Session {
    let mut puts = vec![Put { key: key(cell.panel), edge: cell.edge, order: 0, seen: cell.seen }];
    if cell.company == Company::Stacked {
        puts.push(Put { key: key(partner(cell.panel)), edge: cell.edge, order: 1, seen: Seen::Shown });
    }
    let mut s = Session::start_on(machine_with(&puts));
    s.key(qymcad::Key::Escape);
    // WITH THE BAR OF TOOLS HIDDEN a person goes by keys and by the table of planes: N makes a new part
    if cell.panel == Panel::WorkbenchTools && cell.seen == Seen::Hidden {
        s.key(qymcad::Key::N);
        if cell.bench == Bench::Sketch {
            let xy = s.word("plane-xy-table");
            s.press_word(&xy);
        }
        return s;
    }
    match cell.bench {
        Bench::Assembly => {
            if cell.panel == Panel::CommandBar {
                let mirror = s.word("tb-mirror-part-hint");
                s.press_hint(&mirror);
            }
        }
        Bench::Part => {
            build::into_the_first_part(&mut s);
            if cell.panel == Panel::CommandBar {
                let extrude = s.word("tb-extrude-hint");
                s.press_hint(&extrude);
            }
        }
        Bench::Sketch => {
            // by the pencil and the plane in the view: the table of planes stands in the properties, which a cell
            // may hide
            build::into_the_first_part(&mut s);
            let pencil = s.word("g-sketch-pick-hint");
            s.press_hint(&pencil);
            let table = s.in_space([0.0, 0.0, 0.0]);
            s.click(table);
        }
    }
    s
}

/// A panel and its partner in a stack, where they were laid out.
struct Pair {
    mine: Rect,
    other: Rect,
}

impl Pair {
    /// How far the two share across `edge`, against the narrower of them: all of it for two panels of one strip.
    fn shared_across(&self, edge: Edge) -> f32 {
        let (a, b) = if matches!(edge, Edge::Left | Edge::Right) { (self.mine.x_range(), self.other.x_range()) } else { (self.mine.y_range(), self.other.y_range()) };
        let common = (a.max.min(b.max) - a.min.max(b.min)).max(0.0);
        common / a.span().min(b.span()).max(1.0)
    }

    /// Does the panel stand before its partner along `edge`?
    fn first_along(&self, edge: Edge) -> bool {
        if matches!(edge, Edge::Left | Edge::Right) {
            self.mine.max.y <= self.other.min.y + 1.5
        } else {
            self.mine.max.x <= self.other.min.x + 1.5
        }
    }
}

/// Does one rectangle lie over another by more than a hairline?
fn overlap(pair: &Pair) -> bool {
    pair.mine.shrink(1.5).intersects(pair.other.shrink(1.5))
}

/// What is wrong in the window of `cell`, in words; empty when the cell holds.
fn faults(cell: Cell) -> Vec<String> {
    let mut s = open(cell);
    let mut out = Vec::new();
    let screen = Rect::from_min_size(qymcad::pos2(0.0, 0.0), s.screen());
    let canvas = s.canvas();
    let all = [Panel::Tree, Panel::Properties, Panel::Toolbar, Panel::CommandBar, Panel::Status, Panel::WorkbenchTools];
    let drawn: Vec<Laid> = all.iter().filter_map(|p| s.panel(key(*p)).map(|rect| Laid { panel: *p, rect })).collect();
    let mine = drawn.iter().find(|l| l.panel == cell.panel).map(|l| l.rect);
    // the bar of the command in hand is never hidden: a tool with its bar hidden asks for what nobody can see; nor is
    // the status line, where the program says what it waits for
    let seen = if cell.panel == Panel::CommandBar || pinned(cell.panel) { Seen::Shown } else { cell.seen };
    match (seen, mine) {
        (Seen::Hidden, Some(r)) => out.push(format!("hidden, it is drawn at {r:?}")),
        (Seen::Shown, None) => out.push("shown, it is not drawn".to_string()),
        (Seen::Shown, Some(r)) => {
            let edge = if pinned(cell.panel) { Edge::Bottom } else { cell.edge };
            let at_edge = match edge {
                Edge::Left => r.max.x <= canvas.min.x + 1.5,
                Edge::Right => r.min.x >= canvas.max.x - 1.5,
                Edge::Top => r.max.y <= canvas.min.y + 1.5,
                Edge::Bottom => r.min.y >= canvas.max.y - 1.5,
            };
            if !at_edge {
                out.push(format!("not at its edge: the panel {r:?}, the view {canvas:?}"));
            }
            if !screen.expand(0.5).contains_rect(r) {
                out.push(format!("out of the window: {r:?}"));
            }
            if cell.panel == Panel::WorkbenchTools {
                out.extend(buttons_cut(&mut s, edge, r));
            }
        }
        (Seen::Hidden, None) => {}
    }
    if cell.company == Company::Stacked && !pinned(cell.panel) {
        let other = partner(cell.panel);
        match (drawn.iter().find(|l| l.panel == other).map(|l| l.rect), mine.filter(|_| seen == Seen::Shown)) {
            (None, _) => out.push(format!("its partner {other:?} is not drawn")),
            (Some(q), Some(r)) => {
                let pair = Pair { mine: r, other: q };
                if pair.shared_across(cell.edge) < 0.8 || !pair.first_along(cell.edge) {
                    out.push(format!("not in one stack with {other:?}: the panel {r:?}, the partner {q:?}"));
                }
            }
            (Some(_), None) => {}
        }
    }
    for (i, a) in drawn.iter().enumerate() {
        if overlap(&Pair { mine: a.rect, other: canvas }) {
            out.push(format!("{:?} lies over the view: {:?} and {canvas:?}", a.panel, a.rect));
        }
        for b in &drawn[i + 1..] {
            if overlap(&Pair { mine: a.rect, other: b.rect }) {
                out.push(format!("{:?} and {:?} lie over each other: {:?} and {:?}", a.panel, b.panel, a.rect, b.rect));
            }
        }
    }
    // THE VIEW TAKES WHAT IS LEFT: every point of the window is in the menu, a panel or the view, and the view is
    // left room to work in
    if canvas.width() < 200.0 || canvas.height() < 200.0 {
        out.push(format!("the view is left {canvas:?} of the window {screen:?}"));
    }
    // the middle with its margin round the canvas
    let mut taken: Vec<Rect> = drawn.iter().map(|l| l.rect).collect();
    taken.push(s.panel("viewport").unwrap_or(canvas));
    taken.extend(s.panel("menubar"));
    let points: Vec<qymcad::Pos2> = (0..64).flat_map(|i| (0..40).map(move |j| qymcad::pos2((i as f32 + 0.5) * screen.width() / 64.0, (j as f32 + 0.5) * screen.height() / 40.0))).collect();
    let holes: Vec<&qymcad::Pos2> = points.iter().filter(|p| !taken.iter().any(|r| r.expand(2.0).contains(**p))).collect();
    if holes.len() * 100 > points.len() * 2 {
        out.push(format!("{} of {} points of the window are in no panel and not in the view, the first at {:?}", holes.len(), points.len(), holes[0]));
    }
    out
}

/// A panel and where it was laid out.
struct Laid {
    panel: Panel,
    rect: Rect,
}

/// THE BUTTONS OF THE WORKBENCH not seen whole on their bar `bar` at `edge`: every button and its "More" whole in the
/// bar and in the window; a bar at the top or the bottom is one or two rows.
fn buttons_cut(s: &mut Session, edge: Edge, bar: Rect) -> Vec<String> {
    let mut out = Vec::new();
    let screen = Rect::from_min_size(qymcad::pos2(0.0, 0.0), s.screen());
    let mut icons = s.bar_buttons();
    icons.extend(s.more_buttons());
    if icons.len() < 3 {
        out.push(format!("the bar shows {} buttons", icons.len()));
    }
    let cut: Vec<&Rect> = icons.iter().filter(|r| !bar.expand(0.5).contains_rect(**r) || !screen.contains_rect(**r)).collect();
    if !cut.is_empty() {
        out.push(format!("{} of {} buttons are not seen whole, the first {:?} on the bar {bar:?}", cut.len(), icons.len(), cut[0]));
    }
    if matches!(edge, Edge::Top | Edge::Bottom) {
        // rows by the tops of the buttons: tops apart by less than half a button are one row
        let mut rows: Vec<f32> = Vec::new();
        for b in &icons {
            if !rows.iter().any(|t| (t - b.min.y).abs() < b.height() / 2.0) {
                rows.push(b.min.y);
            }
        }
        if rows.len() > 2 {
            out.push(format!("the buttons at the top or the bottom stand in {} rows", rows.len()));
        }
    }
    out
}

/// Every cell of `panel`, checked; the faults of all of them at once.
fn check_panel(panel: Panel) {
    let mut red = Vec::new();
    let mut cells = 0;
    for bench in BENCHES {
        // the tree gives way to the tools of a sketch while one is edited: that is a rule of the sketch, not a place
        if panel == Panel::Tree && bench == Bench::Sketch {
            continue;
        }
        for edge in EDGES {
            for company in [Company::Alone, Company::Stacked] {
                for seen in [Seen::Shown, Seen::Hidden] {
                    let cell = Cell { panel, edge, company, seen, bench };
                    cells += 1;
                    // a cell that breaks the program is a red cell, and the next one is still looked at
                    let f = std::panic::catch_unwind(|| faults(cell))
                        .unwrap_or_else(|e| vec![e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|t| t.to_string())).unwrap_or_default()]);
                    if !f.is_empty() {
                        red.push(format!("{cell:?}: {}", f.join("; ")));
                    }
                }
            }
        }
    }
    assert!(red.is_empty(), "{} of {cells} cells of {panel:?} are red:\n{}", red.len(), red.join("\n"));
}

probe! {
    budget = 900;
    /// THE TREE at every edge, alone and under another panel, shown and hidden.
    fn the_tree_stands_where_the_layout_puts_it() {
        check_panel(Panel::Tree);
    }
}

probe! {
    budget = 900;
    /// THE PROPERTIES at every edge, alone and over another panel, shown and hidden.
    fn the_properties_stand_where_the_layout_puts_them() {
        check_panel(Panel::Properties);
    }
}

probe! {
    budget = 900;
    /// THE TOOLBAR of the path and the switches at every edge.
    fn the_toolbar_stands_where_the_layout_puts_it() {
        check_panel(Panel::Toolbar);
    }
}

probe! {
    budget = 900;
    /// THE BAR OF THE COMMAND IN HAND at every edge: an extrusion in a part, a mirrored part in an assembly, the
    /// options of the tool in a sketch. Asked to hide, it stays.
    fn the_command_bar_stands_where_the_layout_puts_it() {
        check_panel(Panel::CommandBar);
    }
}

probe! {
    budget = 900;
    /// THE STATUS LINE at every edge.
    fn the_status_line_stands_where_the_layout_puts_it() {
        check_panel(Panel::Status);
    }
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

probe! {
    /// THE PROPERTIES UNDER THE TREE: both seen in one strip at the left, the border between them dragged down by
    /// the mouse, the strip dragged wider by its edge - and both sizes are there after a restart.
    fn the_properties_under_the_tree_keep_their_border() {
        let puts = [Put { key: "tree", edge: Edge::Left, order: 0, seen: Seen::Shown }, Put { key: "props", edge: Edge::Left, order: 1, seen: Seen::Shown }];
        let mut s = Session::start_on(machine_with(&puts));
        s.key(qymcad::Key::Escape);
        build::into_the_first_part(&mut s);
        let (tree, props) = (s.panel("tree").expect("the tree is drawn"), s.panel("props").expect("the properties are drawn"));
        assert!(tree.max.y <= props.min.y + 1.0 && (tree.min.x - props.min.x).abs() < 1.0, "the properties do not stand under the tree: {tree:?} and {props:?}");
        // the border between them, dragged 120 points down
        let border = qymcad::pos2(tree.center().x, (tree.max.y + props.min.y) / 2.0);
        s.drag(border, border + qymcad::vec2(0.0, 120.0), qymcad::PointerButton::Primary, qymcad::Modifiers::NONE);
        let tree_after = s.panel("tree").expect("the tree is drawn");
        assert!((tree_after.height() - tree.height() - 120.0).abs() < 6.0, "the tree is {} tall after the border went 120 down; it was {}", tree_after.height(), tree.height());
        // the right edge of the strip, dragged 80 points wider
        let edge = qymcad::pos2(tree_after.max.x, tree_after.center().y);
        s.drag(edge, edge + qymcad::vec2(80.0, 0.0), qymcad::PointerButton::Primary, qymcad::Modifiers::NONE);
        let wide = s.panel("props").expect("the properties are drawn");
        assert!((wide.width() - props.width() - 80.0).abs() < 6.0, "the strip is {} wide after its edge went 80 out; it was {}", wide.width(), props.width());
        let kept = quit_keeping(s);
        let mut s = Session::start_on(Machine { kept, ..Machine::default() });
        s.key(qymcad::Key::Escape);
        build::into_the_first_part(&mut s);
        let (tree_again, props_again) = (s.panel("tree").expect("the tree is drawn"), s.panel("props").expect("the properties are drawn"));
        assert!((tree_again.height() - tree_after.height()).abs() < 2.0, "after a restart the tree is {} tall, it was left {}", tree_again.height(), tree_after.height());
        assert!((props_again.width() - wide.width()).abs() < 2.0, "after a restart the strip is {} wide, it was left {}", props_again.width(), wide.width());
    }
}

probe! {
    budget = 900;
    /// THE TOOLS OF THE WORKBENCH at every edge, every button seen whole or in "More".
    fn the_workbench_tools_stand_where_the_layout_puts_them() {
        check_panel(Panel::WorkbenchTools);
    }
}
