//! WHAT THE SNAP SHOWS IS WHAT IS PLACED: for every tool that puts points down, a first click on the end of a line, on
//! a line, on a circle, on a centre, on the X and Y axes and on the origin shows the glyph of the tie before the click
//! and lays that tie after it - the point stands on the target and is held there.
//!
//! Reported behaviour: the snap to other geometry was shown while a line or a shape was drawn - an end onto a line or a
//! circle, a centre - and the point was put down where the pointer was, without the tie; and there was no snap to the
//! X and Y axes or to the origin at all.
use qymcad::{Key, Session};
use qymcad_acceptance::{build, probe};

/// A PLACE TO CLICK, what the glyph there says, and the ties that hold a point put there.
struct Target {
    name: &'static str,
    at: (f64, f64),
    glyph: &'static str,
    /// a point put there may share the point standing there instead of being tied to it
    shared: bool,
    ties: &'static [&'static str],
}

/// The reference geometry: a line from (40, 30) to (80, 30) and a circle about (60, 80) of R 8.
const TARGETS: &[Target] = &[
    Target { name: "the end of a line", at: (40.0, 30.0), glyph: "Coincident", shared: true, ties: &["Coincident"] },
    Target { name: "a line", at: (50.0, 30.0), glyph: "PointOnLine", shared: false, ties: &["PointOnLine"] },
    Target { name: "a circle", at: (60.0, 88.0), glyph: "PointOnCircle", shared: false, ties: &["PointOnCircle"] },
    Target { name: "the centre of a circle", at: (60.0, 80.0), glyph: "Coincident", shared: true, ties: &["Coincident", "Concentric"] },
    Target { name: "the X axis", at: (90.0, 0.0), glyph: "PointOnLine", shared: false, ties: &["PointOnLine"] },
    Target { name: "the Y axis", at: (0.0, -40.0), glyph: "PointOnLine", shared: false, ties: &["PointOnLine"] },
    Target { name: "the origin", at: (0.0, 0.0), glyph: "Coincident", shared: true, ties: &["Coincident"] },
];

/// A TOOL THAT PUTS POINTS DOWN: its button, a mode of its bar if any, and how it ends after its clicks. The first click
/// goes on the target; the others go to free places beside it, (-12, -8) and (-12, 8) off it.
struct Tool {
    hint: &'static str,
    mode: Option<&'static str>,
    clicks: usize,
    ends: Ends,
    puts: Puts,
}

/// What a tool puts at its first click.
#[derive(Clone, Copy, PartialEq)]
enum Puts {
    /// a point of its own: an end, a corner, a centre, a node
    APoint,
    /// no point, a rim running through the click: a point clicked is held on it, and a curve clicked holds nothing
    ARim,
}

/// How a drawing is ended after its clicks.
#[derive(Clone, Copy, PartialEq)]
enum Ends {
    /// the last click ends it
    LastClick,
    /// a double click on the last place
    DoubleClick,
    /// Esc after the last click, a chain of lines going on otherwise
    Escape,
}

/// The sketch on XY with the auto constraints on, as a person finds it, and the line and the circle the targets are on.
fn the_sheet() -> Session {
    let mut s = Session::start();
    build::into_the_first_part(&mut s);
    let xy = s.word("plane-xy-table");
    s.press_word(&xy);
    build::line(&mut s, (40.0, 30.0), (80.0, 30.0));
    build::circle(&mut s, (60.0, 80.0), (68.0, 80.0));
    s
}

/// How many points of the sketch stand at `p`.
fn standing_at(s: &mut Session, p: (f64, f64)) -> usize {
    s.document().sketches[0].places.iter().filter(|q| (q[0] - p.0).hypot(q[1] - p.1) < 1e-6).count()
}

/// The kinds of the constraints of the sketch.
fn kinds(s: &mut Session) -> Vec<String> {
    s.document().sketches[0].constraint_kinds.clone()
}

/// The tool put down at every target in turn, a fresh sheet each time: what failed, together.
fn every_target(tool: &Tool) -> Vec<String> {
    let mut failed = Vec::new();
    for t in TARGETS.iter().filter(|t| tool.puts == Puts::APoint || t.shared) {
        let mut s = the_sheet();
        let (before_here, before_kinds) = (standing_at(&mut s, t.at), kinds(&mut s));
        let hint = s.word(tool.hint);
        s.press_hint(&hint);
        if let Some(mode) = tool.mode {
            let mode = s.word(mode);
            s.press_word(&mode);
        }
        let first = s.on_sketch(t.at.0, t.at.1);
        s.move_to(first);
        match s.snap_glyph() {
            Some(g) if g.glyph == t.glyph && (g.at.0 - t.at.0).hypot(g.at.1 - t.at.1) < 1e-6 => {}
            other => failed.push(format!("{}: before the click the glyph is {other:?}, not {} at {:?}", t.name, t.glyph, t.at)),
        }
        let places = [t.at, (t.at.0 - 12.0, t.at.1 - 8.0), (t.at.0 - 12.0, t.at.1 + 8.0)];
        for (k, p) in places.iter().take(tool.clicks).enumerate() {
            let last = k + 1 == tool.clicks;
            if last && tool.ends == Ends::DoubleClick {
                let at = s.on_sketch(p.0, p.1);
                s.double_click(at);
            } else {
                s.click_on_sketch(p.0, p.1);
            }
        }
        if tool.ends == Ends::Escape {
            s.key(Key::Escape);
        }
        s.key(Key::Escape);
        let here = standing_at(&mut s, t.at);
        let mut added = kinds(&mut s);
        for k in &before_kinds {
            if let Some(i) = added.iter().position(|a| a == k) {
                added.remove(i);
            }
        }
        let (shared, tied) = match tool.puts {
            Puts::APoint => (t.shared && here <= before_here, added.iter().any(|k| t.ties.contains(&k.as_str()))),
            Puts::ARim => (false, added.iter().any(|k| k == "PointOnCircle")),
        };
        if here == 0 && !shared && tool.puts == Puts::APoint {
            failed.push(format!("{}: no point stands at {:?} after the click", t.name, t.at));
        } else if !shared && !tied {
            failed.push(format!("{}: the point put at {:?} is held by nothing - the ties laid are {added:?}, one of {:?} was shown", t.name, t.at, t.ties));
        }
    }
    failed
}

/// Run a tool over every target and say what failed.
fn check(tool: Tool) {
    let failed = every_target(&tool);
    assert!(failed.is_empty(), "{}: {} of {} targets not taken:\n{}", tool.hint, failed.len(), TARGETS.len(), failed.join("\n"));
}

probe! {
    budget = 300;
    /// THE LINE: its first end put on each target.
    fn a_line_takes_the_snap_it_shows() {
        check(Tool { hint: "tb-line-hint", mode: None, clicks: 2, ends: Ends::Escape, puts: Puts::APoint });
    }
}

probe! {
    budget = 300;
    /// THE RECTANGLE BY TWO CORNERS: its first corner put on each target.
    fn a_rectangle_takes_the_snap_it_shows() {
        check(Tool { hint: "tb-rect-hint", mode: None, clicks: 2, ends: Ends::LastClick, puts: Puts::APoint });
    }
}

probe! {
    budget = 300;
    /// THE RECTANGLE FROM ITS CENTRE: its centre put on each target.
    fn a_rectangle_from_its_centre_takes_the_snap_it_shows() {
        check(Tool { hint: "tb-rect-hint", mode: Some("opt-rect-centre"), clicks: 2, ends: Ends::LastClick, puts: Puts::APoint });
    }
}

probe! {
    budget = 300;
    /// THE CIRCLE: its centre put on each target.
    fn a_circle_takes_the_snap_it_shows() {
        check(Tool { hint: "tb-circle-hint", mode: None, clicks: 2, ends: Ends::LastClick, puts: Puts::APoint });
    }
}

probe! {
    budget = 300;
    /// THE CIRCLE BY THREE POINTS: its first click on each point of the sketch - the point held on the circle.
    fn a_circle_by_three_points_takes_the_snap_it_shows() {
        check(Tool { hint: "tb-circle-3pt", mode: None, clicks: 3, ends: Ends::LastClick, puts: Puts::ARim });
    }
}

probe! {
    budget = 300;
    /// THE ARC: its centre put on each target.
    fn an_arc_takes_the_snap_it_shows() {
        check(Tool { hint: "tb-arc-hint", mode: None, clicks: 3, ends: Ends::LastClick, puts: Puts::APoint });
    }
}

probe! {
    budget = 300;
    /// THE POLYGON: its centre put on each target.
    fn a_polygon_takes_the_snap_it_shows() {
        check(Tool { hint: "tb-polygon-hint", mode: None, clicks: 2, ends: Ends::LastClick, puts: Puts::APoint });
    }
}

probe! {
    budget = 300;
    /// THE SLOT: its first end put on each target.
    fn a_slot_takes_the_snap_it_shows() {
        check(Tool { hint: "tb-slot-hint", mode: None, clicks: 3, ends: Ends::LastClick, puts: Puts::APoint });
    }
}

probe! {
    budget = 300;
    /// THE ELLIPSE: its centre put on each target.
    fn an_ellipse_takes_the_snap_it_shows() {
        check(Tool { hint: "tb-ellipse-hint", mode: None, clicks: 3, ends: Ends::LastClick, puts: Puts::APoint });
    }
}

probe! {
    budget = 300;
    /// THE SPLINE: its first node put on each target.
    fn a_spline_takes_the_snap_it_shows() {
        check(Tool { hint: "tb-spline-hint", mode: None, clicks: 3, ends: Ends::DoubleClick, puts: Puts::APoint });
    }
}

probe! {
    budget = 300;
    /// THE POINT: put on each target.
    fn a_point_takes_the_snap_it_shows() {
        check(Tool { hint: "tb-point-hint", mode: None, clicks: 1, ends: Ends::LastClick, puts: Puts::APoint });
    }
}

probe! {
    /// THE TIE TO THE WORLD SHOWS ITS BADGE: a rectangle from its centre, the centre clicked on the Y axis - the badge of
    /// the point on the axis stands by the centre, though the rectangle's own ties, which hold its shape, show none.
    /// Reported behaviour: the centre was put on the axis and no badge said it was held there.
    fn the_centre_of_a_rectangle_put_on_an_axis_shows_the_badge_of_its_tie() {
        let mut s = the_sheet();
        let hint = s.word("tb-rect-hint");
        s.press_hint(&hint);
        let mode = s.word("opt-rect-centre");
        s.press_word(&mode);
        s.click_on_sketch(0.0, -40.0).click_on_sketch(20.0, -30.0);
        s.key(Key::Escape);
        let centre = s.on_sketch(0.0, -40.0);
        let badges = s.badges();
        assert!(
            badges.iter().any(|b| b.glyph == "PointOnLine" && b.at.distance(centre) < 40.0),
            "no badge of a point on a line by the centre at {centre:?}; the badges drawn: {badges:?}"
        );
    }
}
