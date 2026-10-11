//! THE TOOLS OF THE PART ACROSS BODIES, PICKS AND VALUES. A tool tried on one edge of one block says nothing about
//! the fourth edge of a face, an edge that ends on an earlier rounding, a round edge or a hollow one. A case here is
//! a body built through the window, the edges a person clicks on it, a value typed, and what must come of it: the
//! body cut or grown by what the geometry says, a face of the right kind for every edge picked, one correct solid no
//! larger than before - or a refusal in words, the body as it was.
//!
//! What must come of a case is worked out from the geometry (the areas below), never read off the program.
use std::f64::consts::PI;

use qymcad::{FaceKinds, Key, Session, Solid};

/// AN EDGE AS A PERSON CLICKS IT: a point on it, another point on it for when something of the window stands over
/// the first, the corner of the view cube it is seen from when the view as it stands hides both - and, for a short
/// edge, the end of it nearest the point, which a click too close to is taken for.
#[derive(Clone, Copy, Debug)]
pub struct Spot {
    pub at: [f64; 3],
    pub alt: [f64; 3],
    pub from: [i8; 3],
    pub end: Option<[f64; 3]>,
}

/// An edge through `at` and `alt`, seen from the front, the right and above - the view as the program opens it.
pub const fn spot(at: [f64; 3], alt: [f64; 3]) -> Spot {
    Spot { at, alt, from: [1, -1, 1], end: None }
}

/// A short edge through `at` and `alt` whose nearest end is `end`: the view is scaled up about it until that end
/// stands [`CLEAR`] px off, as a person scales up to click a short edge rather than its corner.
pub const fn short(at: [f64; 3], alt: [f64; 3], end: [f64; 3]) -> Spot {
    Spot { at, alt, from: [1, -1, 1], end: Some(end) }
}

/// How far on screen the click on an edge stands from its end, in px: three times the reach of a click that takes a
/// corner.
pub const CLEAR: f32 = 24.0;

/// A BODY TO WORK ON, built through the window by `build`.
pub struct Body {
    pub name: &'static str,
    pub build: fn(&mut Session),
}

/// FACES A TOOL ADDS, by kind. `blend` counts every curved face that is neither a cylinder nor a cone - the sphere,
/// torus or free form a kernel makes a corner or a round edge of.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Added {
    pub plane: i32,
    pub cylinder: i32,
    pub cone: i32,
    pub blend: i32,
}

impl Added {
    fn between(before: FaceKinds, after: FaceKinds) -> Added {
        let blend = |k: FaceKinds| (k.sphere + k.torus + k.free + k.other) as i32;
        Added {
            plane: after.plane as i32 - before.plane as i32,
            cylinder: after.cylinder as i32 - before.cylinder as i32,
            cone: after.cone as i32 - before.cone as i32,
            blend: blend(after) - blend(before),
        }
    }
}

/// WHAT A CASE MUST COME TO.
#[derive(Clone, Copy, Debug)]
pub enum Expect {
    /// Built: the volume changes by `change` mm^3 (negative when material is cut away) within `tol`, the faces grow
    /// by `added` - when the geometry says how - and the body stays one correct solid within the box it had.
    Built { change: f64, tol: f64, added: Option<Added> },
    /// Refused in words - the node red with its reason, the status line, or the words beside the field - and every
    /// body as it was.
    Refused,
    /// Built as [`Expect::Built`], the volume changing by `change` one way or the other: which way a sign of the
    /// value turns is the program's own convention (a draft outward or inward), the amount is the geometry's.
    Magnitude { change: f64, tol: f64, added: Option<Added> },
    /// Built as [`Expect::Built`] with no word on the faces, in `pieces` solids apart - the copies of a pattern
    /// with room between them.
    Pieces { change: f64, tol: f64, pieces: u32 },
    /// Whatever the tool makes of it, the program stays whole: built, one correct solid; refused, in words and every
    /// body as it was. What the part may hold is the oracles' after every step - one body.
    Whole,
    /// Either, where the professional systems differ - a rounding may overflow onto the faces next to it or be
    /// refused: built, the body one correct solid within the box it had and smaller than it was; refused, in words and
    /// the body as it was. Never a broken body, never a green node that did nothing.
    Either,
}

/// A STEP OF PICKING: a click on an edge or on a face through a spot of it, or a word pressed on the bar of options
/// between clicks (the neutral face of a draft).
#[derive(Clone, Copy, Debug)]
pub enum Pick {
    Edge(Spot),
    Face(Spot),
    /// A click on the corner at the spot.
    Vertex(Spot),
    Bar(&'static str),
    /// A letter pressed on the bar - the name of an axis, the same in every language and so not a word of the
    /// catalogue.
    Letter(&'static str),
    /// A click on this point of space, whatever is drawn there - a region of a finished sketch. When the tool shows
    /// the sketch flat, the point is read on its sheet, in the sketch's own axes (the world's, for a sketch on XY).
    Space([f64; 3]),
    /// Enter, where the tool takes a step with it: the contours picked, on to their size.
    Enter,
    /// A switch of the bar ticked or cleared by its word (a pattern spread over the whole turn).
    Toggle(&'static str),
    /// The row of the sketch numbered so in the tree clicked: the path of a sweep, the next section of a loft.
    SketchRow(usize),
}

/// ONE CASE: a body, what is clicked on it, the words pressed on the bar of options, the value typed into the tool's
/// own field and into others, what must come of it, how it is told - and a change made above the tool in the timeline
/// afterwards, with what must come of that.
pub struct Case {
    pub body: &'static Body,
    pub picks: Vec<Pick>,
    pub bar: Vec<&'static str>,
    pub value: String,
    pub more: Vec<(&'static str, String)>,
    pub expect: Expect,
    pub what: String,
    pub then: Option<Then>,
    /// Steps taken once the fields are filled, before Enter: a switch a person ticks after the count.
    pub after: Vec<Pick>,
}

/// A CHANGE ABOVE THE TOOL: the node whose tree row begins with the word `row` reopened with a double click, `value`
/// typed under `caption`, Enter. The body the tool stood on is now `base` mm^3, and the tool must take `change` of it
/// (within `tol`) and add the faces it added before - every edge it was given still done, or the node red with its
/// reason.
pub struct Then {
    pub row: &'static str,
    pub caption: &'static str,
    pub value: &'static str,
    pub base: f64,
    pub change: f64,
    pub tol: f64,
}

/// A TOOL OF THE PART: taken by its hint, its own value typed under its caption, the node it makes - and whether its
/// status line counts what is clicked, as the tools of edges do.
pub struct PartTool {
    pub hint: &'static str,
    pub caption: &'static str,
    pub node: &'static str,
    pub counts: bool,
}

/// No body at all: what a part holds before its first body is made.
fn nothing() -> Solid {
    Solid {
        name: String::new(),
        part: None,
        part_key: None,
        colour: [0; 3],
        volume: 0.0,
        area: 0.0,
        faces: 0,
        face_names: Vec::new(),
        face_centres: Vec::new(),
        edges: None,
        min: [f64::NAN; 3],
        max: [f64::NAN; 3],
        visible: true,
        consumed: false,
        sheet: false,
        lit_by_the_surface: 0,
    }
}

/// How far a point of the mesh may lie from the exact surface, in mm.
const MESH: f64 = 0.05;

/// The body the tools of the part work on, with its place among the bodies: the last solid that is its own.
pub fn working(s: &mut Session) -> Option<(usize, Solid)> {
    s.document().bodies.into_iter().enumerate().rfind(|(_, b)| !b.consumed && !b.sheet)
}

/// BRING `spot` INTO VIEW as a person does, and answer the point of it to click: one of its two points the eye
/// sees; when the view hides both, a click on the corner of the cube it is seen from first - by way of the top of the
/// cube when that corner is on the far side.
pub fn show(s: &mut Session, spot: Spot) -> [f64; 3] {
    let p = seen_point(s, spot);
    if let Some(end) = spot.end {
        for _ in 0..12 {
            match (s.seen_at(p), s.seen_at(end)) {
                (Some(a), Some(b)) if (a - b).length() >= CLEAR => break,
                (Some(a), _) => {
                    s.wheel(a, qymcad::vec2(0.0, 120.0), qymcad::Modifiers::NONE);
                }
                _ => break,
            }
        }
        return seen_point(s, spot);
    }
    p
}

/// The point of `spot` the eye sees, turning the view to it first when it must.
fn seen_point(s: &mut Session, spot: Spot) -> [f64; 3] {
    if let Some(p) = [spot.at, spot.alt].into_iter().find(|p| s.sees(*p)) {
        return p;
    }
    let corner = match s.cube_toward(spot.from) {
        Some(at) => at,
        None => {
            let top = s.cube_toward([0, 0, 1]).unwrap_or_else(|| panic!("the top of the view cube is out of reach"));
            s.click(top);
            s.cube_toward(spot.from).unwrap_or_else(|| panic!("the corner {:?} of the view cube is out of reach even from the top", spot.from))
        }
    };
    s.click(corner);
    [spot.at, spot.alt].into_iter().find(|p| s.sees(*p)).unwrap_or_else(|| panic!("{:?} is hidden even from the corner {:?} of the view cube", spot.at, spot.from))
}

/// RUN ONE CASE of `tool` in a session of its own; `round_trips` adds undo and redo, save and open, and rebuilding
/// everything to what is checked of a body built. The problems, in words; empty when the case holds.
pub fn run(tool: &PartTool, case: &Case, round_trips: bool) -> Vec<String> {
    let mut s = Session::start();
    (case.body.build)(&mut s);
    // a tool that makes the first body of the part starts from nothing
    let empty = working(&mut s).is_none();
    let (base, was) = match working(&mut s) {
        None => (nothing(), qymcad::Inspection { valid: true, solids: 0, shells: 0, kinds: FaceKinds::default() }),
        Some((i, base)) => {
            let Some(was) = s.inspect(i) else { return vec!["the body to work on has no exact body to read".into()] };
            if !was.valid || was.solids != 1 {
                return vec![format!("the body to work on is not one correct solid before the tool is taken: {was:?}")];
            }
            (base, was)
        }
    };
    let nodes = |s: &mut Session| s.document().features.iter().filter(|f| f.kind == tool.node).count();
    let before = nodes(&mut s);
    let hint = s.word(tool.hint);
    s.press_hint(&hint);
    for word in &case.bar {
        let word = s.word(word);
        s.press_word_near(&word, qymcad::pos2(640.0, 0.0));
    }
    let instruction = s.status();
    if let Some(problem) = take_steps(&mut s, tool, &case.picks) {
        return vec![problem];
    }
    // what a click was answered with: a refusal said at the click counts as said, a count of what is picked does not
    let at_the_click = s.status();
    let said_at_the_click = at_the_click != instruction && !at_the_click.chars().any(|c| c.is_ascii_digit());
    // the tool's own field first, as a person fills it - a pattern shows the field of its pitch once it has a count -
    // then the others; a tool with no value of its own (removing faces) has no field and nothing beside it
    let status_before = s.status();
    if !tool.caption.is_empty() {
        let caption = s.word(tool.caption);
        s.fill(&caption, &case.value);
    }
    for (caption, text) in &case.more {
        let caption = s.word(caption);
        s.fill(&caption, text);
    }
    let beside = if tool.caption.is_empty() {
        Vec::new()
    } else {
        let caption = s.word(tool.caption);
        let field = s.field(&caption);
        s.hint_at(qymcad::pos2(field.rect.max.x + 10.0, field.rect.center().y))
    };
    if let Some(problem) = take_steps(&mut s, tool, &case.after) {
        return vec![problem];
    }
    let status = if tool.caption.is_empty() { status_before } else { s.status() };
    s.key(Key::Enter);
    let doc = s.document();
    let node = (doc.features.iter().filter(|f| f.kind == tool.node).count() > before).then(|| doc.features.iter().rfind(|f| f.kind == tool.node).cloned()).flatten();
    let (i, now) = match working(&mut s) {
        Some(found) => found,
        None if empty => (usize::MAX, nothing()),
        None => return vec!["no body is left".into()],
    };
    let mut problems = Vec::new();
    let refused = match case.expect {
        Expect::Either | Expect::Whole => node.as_ref().is_none_or(|n| n.error.is_some()),
        Expect::Refused => true,
        Expect::Built { .. } | Expect::Magnitude { .. } | Expect::Pieces { .. } => false,
    };
    let expect = match case.expect {
        Expect::Either if !refused => Expect::Built { change: now.volume.min(base.volume) - base.volume, tol: f64::INFINITY, added: None },
        Expect::Whole if !refused => Expect::Built { change: now.volume - base.volume, tol: f64::INFINITY, added: None },
        Expect::Either | Expect::Whole => Expect::Refused,
        Expect::Magnitude { change, tol, added } => Expect::Built { change: change.abs() * (now.volume - base.volume).signum(), tol, added },
        Expect::Pieces { change, tol, .. } => Expect::Built { change, tol, added: None },
        other => other,
    };
    if matches!(case.expect, Expect::Either) && !refused && now.volume >= base.volume {
        problems.push(format!("built, and the body did not lose anything: {} mm^3, it was {}", now.volume, base.volume));
    }
    match expect {
        Expect::Either | Expect::Whole | Expect::Magnitude { .. } | Expect::Pieces { .. } => unreachable!("told apart above"),
        Expect::Refused => {
            let said = match &node {
                Some(n) => n.error.as_deref().is_some_and(|e| !e.trim().is_empty()),
                None => s.status() != status || !beside.is_empty() || said_at_the_click,
            };
            if !said {
                problems.push(format!(
                    "refused without a word: {}; the status line says {:?}",
                    node.map_or("nothing was made".to_string(), |n| format!("the node {:?} came in green", n.name)),
                    s.status()
                ));
            }
            if (now.volume - base.volume).abs() > 1e-6 || now.faces != base.faces {
                problems.push(format!("the body changed: {} mm^3 over {} faces, it was {} over {}", now.volume, now.faces, base.volume, base.faces));
            }
        }
        Expect::Built { change, tol, added } => {
            match &node {
                None => problems.push(format!("nothing was made; the status line says {:?}", s.status())),
                Some(n) if n.error.is_some() => problems.push(format!("the node came in red: {:?}", n.error)),
                _ => {}
            }
            match s.inspect(i) {
                None => problems.push("the body left has no exact body to read".into()),
                Some(is) => {
                    let pieces = if let Expect::Pieces { pieces, .. } = case.expect { pieces } else { 1 };
                    if !is.valid || is.solids != pieces || is.shells != pieces {
                        problems.push(format!("the body is not one correct solid: valid {}, {} solids, {} shells", is.valid, is.solids, is.shells));
                    }
                    let got = Added::between(was.kinds, is.kinds);
                    if added.is_some_and(|want| got != want) {
                        problems.push(format!("the faces added are {got:?}, not {added:?}"));
                    }
                }
            }
            let d = now.volume - base.volume;
            if (d - change).abs() > tol {
                problems.push(format!("the volume changed by {d:.4} mm^3, not {change:.4} (within {tol})"));
            }
            // a tool that takes material away leaves the body within the box it had; the box is read off the mesh,
            // whose points on a curve move by up to its deflection when it is made anew
            if change <= 0.0 && (0..3).any(|k| now.min[k] < base.min[k] - MESH || now.max[k] > base.max[k] + MESH) {
                problems.push(format!("the body reaches out of the box it had: {:?}..{:?}, it was {:?}..{:?}", now.min, now.max, base.min, base.max));
            }
            if let (Some(then), true) = (&case.then, problems.is_empty()) {
                problems.extend(change_above(&mut s, tool, then, was.kinds, added));
            }
            if round_trips && problems.is_empty() {
                let path = std::env::temp_dir().join(format!("qymcad-matrix-{}.qcad", std::process::id())).to_string_lossy().into_owned();
                for trip in [crate::oracles::undo_redo(&mut s), crate::oracles::save_open(&mut s, &path), crate::oracles::rebuild_everything(&mut s)] {
                    if let Err(e) = trip {
                        problems.push(e);
                    }
                }
            }
        }
    }
    problems
}

/// MAKE THE CHANGE ABOVE THE TOOL and see the tool follow it: green, every edge still done - the faces it added
/// before, the volume the geometry gives now - the body one correct solid.
fn change_above(s: &mut Session, tool: &PartTool, then: &Then, kinds: FaceKinds, added: Option<Added>) -> Vec<String> {
    let lead = s.word(then.row);
    let left = s.canvas().min.x;
    let rows: Vec<qymcad::Rect> = s.words_at().into_iter().filter(|(w, r)| r.max.x < left && w.starts_with(&lead)).map(|(_, r)| r).collect();
    let [row] = rows.as_slice() else { return vec![format!("the row of the node to change, beginning {lead:?}, is not one row of the tree: {rows:?}")] };
    s.double_click(row.center());
    let caption = s.word(then.caption);
    s.fill(&caption, then.value);
    s.key(Key::Enter);
    let mut problems = Vec::new();
    let what = format!("after {:?} = {} above it", caption, then.value);
    let doc = s.document();
    if let Some(n) = doc.features.iter().rfind(|f| f.kind == tool.node) {
        if n.error.is_some() {
            problems.push(format!("{what}: the node went red: {:?}", n.error));
        }
    }
    let Some((i, now)) = working(s) else { return vec![format!("{what}: no body is left")] };
    let d = now.volume - then.base;
    if (d - then.change).abs() > then.tol {
        problems.push(format!("{what}: the tool takes {d:.4} mm^3 of the body, not {:.4} (within {})", then.change, then.tol));
    }
    match s.inspect(i) {
        None => problems.push(format!("{what}: the body has no exact body to read")),
        Some(is) => {
            if !is.valid || is.solids != 1 || is.shells != 1 {
                problems.push(format!("{what}: the body is not one correct solid: valid {}, {} solids, {} shells", is.valid, is.solids, is.shells));
            }
            let got = Added::between(kinds, is.kinds);
            if added.is_some_and(|want| got != want) {
                problems.push(format!("{what}: the faces the tool added are {got:?}, not {added:?}"));
            }
        }
    }
    problems
}

/// TAKE THE STEPS OF PICKING in turn - clicks, words and letters on the bar, switches, Enter; for a tool whose status
/// line counts what it takes, every click must add one. The problem, when a click did not.
pub fn take_steps(s: &mut Session, tool: &PartTool, steps: &[Pick]) -> Option<String> {
    let mut clicks = 0;
    for pick in steps {
        let p = match *pick {
            Pick::Edge(p) | Pick::Face(p) | Pick::Vertex(p) => p,
            Pick::Bar(word) => {
                let word = s.word(word);
                s.press_word_near(&word, qymcad::pos2(640.0, 0.0));
                continue;
            }
            Pick::Letter(letter) => {
                s.press_word_near(letter, qymcad::pos2(640.0, 0.0));
                continue;
            }
            Pick::Space(p) => {
                let at = if s.flat() { s.on_sheet(p[0], p[1]) } else { s.in_space(p) };
                s.click(at);
                continue;
            }
            Pick::Enter => {
                s.key(Key::Enter);
                continue;
            }
            Pick::Toggle(word) => {
                let word = s.word(word);
                s.toggle(&word);
                continue;
            }
            Pick::SketchRow(i) => {
                let name = s.document().sketches.get(i).map(|k| k.name.clone()).unwrap_or_default();
                let Some(row) = s.find(&name, qymcad::pos2(0.0, 300.0)) else { return Some(format!("the sketch {name:?} is not in the tree")) };
                s.click(row.center());
                continue;
            }
        };
        let point = show(s, p);
        let at = match pick {
            Pick::Edge(_) => s.edge_at(point),
            Pick::Vertex(_) => s.vertex_at(point),
            _ => s.face_at(point),
        };
        s.click(at);
        if !tool.counts {
            continue;
        }
        // the status line counts what is picked, as the person reads it
        let status = s.status();
        let counted = status.split(|c: char| !c.is_ascii_digit()).rfind(|w| !w.is_empty()).and_then(|w| w.parse::<usize>().ok());
        clicks += 1;
        if counted != Some(clicks) {
            return Some(format!("the click on {pick:?} did not add it: the status line says {status:?}"));
        }
    }
    None
}

/// RUN EVERY CASE and fail with all that did not hold, each told by its body and its words.
pub fn run_all(tool: &PartTool, cases: &[Case], round_trips: bool) {
    let mut failed = Vec::new();
    for case in cases {
        let problems = crate::refusal(|| {
            let p = run(tool, case, round_trips);
            assert!(p.is_empty(), "{}", p.join("; "));
        });
        if !problems.is_empty() {
            failed.push(format!("{} / {} / {:?}: {problems}", case.body.name, case.what, case.value));
        }
    }
    assert!(failed.is_empty(), "{} of {} cases did not hold:\n{}", failed.len(), cases.len(), failed.join("\n"));
}

/// THE AREA A ROUNDING OF RADIUS `r` TAKES FROM THE SECTION OF A STRAIGHT EDGE whose faces turn by `turn` radians
/// (pi/2 on the edge of a block): the corner between the faces less the arc. A hollow edge gains the same area.
pub fn fillet_area(r: f64, turn: f64) -> f64 {
    r * r * ((turn / 2.0).tan() - turn / 2.0)
}

/// How far the centre of that area lies from the corner, along either face, on an edge of a block.
pub fn fillet_centre(r: f64) -> f64 {
    r * (5.0 / 6.0 - PI / 4.0) / (1.0 - PI / 4.0)
}

/// THE AREA A CHAMFER OF LEG `d` TAKES FROM THE SECTION OF A STRAIGHT EDGE between faces meeting at `angle` radians
/// inside the body (pi/2 on the edge of a block).
pub fn chamfer_area(d: f64, angle: f64) -> f64 {
    0.5 * d * d * angle.sin()
}

/// WHERE TWO ROUNDED EDGES OF A BLOCK MEET at a corner whose third edge stays sharp, what the two roundings share:
/// the waste of both, counted twice by their sum.
pub fn fillet_mitre(r: f64) -> f64 {
    r * r * r * (5.0 / 3.0 - PI / 2.0)
}

/// WHERE THREE ROUNDED EDGES OF A BLOCK MEET, what a ball of the radius leaves of the corner's cube of side `r`, less
/// the three wastes that run into it: the corner takes `r^3 (1 - pi/6)` in all.
pub fn fillet_corner(r: f64) -> f64 {
    r * r * r * (1.0 - PI / 6.0)
}

/// Where two chamfered edges of a block meet, one of leg `a` and one of leg `b`, what the two chamfers share.
pub fn chamfer_mitre(a: f64, b: f64) -> f64 {
    let m = a.min(b);
    a * b * m - (a + b) * m * m / 2.0 + m * m * m / 3.0
}

/// WHERE THREE CHAMFERED EDGES OF A BLOCK MEET, what the chamfers take of the corner's cube of side `d`: the three
/// strips take 3/4 of it, and the corner is closed by a triangle through the points where the edges of the strips
/// meet on each face - (d, d, 0) and its turns - which takes the tetrahedron of d^3 / 12 beyond them.
pub fn chamfer_corner(d: f64) -> f64 {
    (0.75 + 1.0 / 12.0) * d * d * d
}
