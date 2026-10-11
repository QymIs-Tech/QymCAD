//! WHAT MUST HOLD AFTER EVERY STEP, whatever the step was: the document whole, no red node without a reason, no key
//! of the catalogue or box in place of a letter on screen, nothing waiting for a click without its bar, no frame out
//! of its time. And three round trips a document must survive: undo and redo, save and open, rebuilding everything.
use std::time::Duration;

use qymcad::{Document, Key, Modifiers, Session};

use crate::build;

/// The longest a frame may take between two steps: a debug build on one core of a machine that is doing other things.
pub const STEP_FRAME_BUDGET: Duration = Duration::from_secs(3);

/// EVERYTHING THAT MUST HOLD AFTER A STEP, as problems in words; empty when it all holds.
pub fn after_every_step(s: &mut Session) -> Vec<String> {
    let doc = s.document();
    let mut problems = whole(&doc);
    // the history of the session's steps is its own: the copy a round trip opens in another start is not a step of it
    if !TRIPPING.get() {
        problems.extend(green_and_idle(&doc));
    }
    problems.extend(s.words_that_are_keys().into_iter().map(|w| format!("a key of the catalogue stands on screen instead of words: {w:?}")));
    problems.extend(s.words_without_glyphs().into_iter().map(|w| format!("drawn with a letter the font does not have, a box on screen: {w:?}")));
    if let Some(what) = s.waiting_without_bar() {
        problems.push(format!("{what} waits for a click, and no bar of options says so"));
    }
    if let Some(bar) = s.bar_with_nothing_in_hand() {
        problems.push(format!("a bar of options stands with nothing in hand behind it: {bar:?}"));
    }
    let frame = s.step_frame();
    if frame > STEP_FRAME_BUDGET {
        problems.push(format!("a frame took {frame:?}, over {STEP_FRAME_BUDGET:?}"));
    }
    problems.extend(round_trips(s, &doc));
    problems
}

std::thread_local! {
    /// The steps of undo the document held when the round trips were last made, in this session's thread.
    static TRIPPED_AT: std::cell::RefCell<Option<Vec<String>>> = const { std::cell::RefCell::new(None) };
    /// A round trip is being made: the other start of the program it opens a copy in is not one to make trips of.
    static TRIPPING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// This check makes no round trips at all (`no_round_trips_here`).
    static NO_TRIPS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// THIS CHECK MAKES NO ROUND TRIPS: for a check on a heavy sample whose steps - roll back, suppress, give back - have
/// their round trips made by a check on a light sample already. Measured on the sample PC case: 23 roundings, four
/// steps each, and after every step undo with redo, a copy opened and a rebuild from the start - past 1800 s, against
/// 578 s without them.
pub fn no_round_trips_here() {
    NO_TRIPS.set(true);
}

/// THE THREE ROUND TRIPS AFTER EVERY STEP THAT CHANGED THE DOCUMENT with nothing in hand - undo and redo, a copy saved
/// and opened in another start, everything rebuilt from the start - each giving the same document back. Midway through
/// a gesture undo puts the tool down, as it should, so a step with a tool in hand is not one to judge. Costly: made on
/// the release level only (`QYMCAD_TIER=release`, or `QYM_ROUND_TRIPS` for a run of a few checks), where the budget
/// is a day: the contract points with them took 1166 s at 6 threads.
fn round_trips(s: &mut Session, doc: &Document) -> Vec<String> {
    let release = std::env::var("QYMCAD_TIER").is_ok_and(|t| t.trim() == "release");
    if !release && std::env::var_os("QYM_ROUND_TRIPS").is_none() {
        return Vec::new();
    }
    let changed = TRIPPED_AT.with(|t| t.borrow().as_ref() != Some(&doc.undo));
    if TRIPPING.get() || NO_TRIPS.get() || !changed || doc.undo.is_empty() || s.asking_for_a_file() || !s.in_hand().is_empty() {
        return Vec::new();
    }
    TRIPPING.set(true);
    let mut problems = Vec::new();
    // one trip alone when `QYM_ROUND_TRIP_ONLY` names it (undo, save, rebuild): a failure told apart by its trip
    let only = std::env::var("QYM_ROUND_TRIP_ONLY").unwrap_or_default();
    // undo lets go of what is chosen, as it does in the professional systems, and rebuilding says so on the status
    // line: the trips are the check's own steps, and the next step of the person finds both as the last one left them
    s.aside(|s| {
        // with a field taking the keys Ctrl+Z is the field's: the text typed so far is the person's, not a step
        if (only.is_empty() || only == "undo") && !s.typing() {
            problems.extend(undo_redo(s).err());
        }
        let path = std::env::temp_dir().join(format!("qymcad-round-trip-{}-{:?}.qcad", std::process::id(), std::thread::current().id())).to_string_lossy().into_owned();
        if only.is_empty() || only == "save" {
            problems.extend(save_copy_open(s, &path).err());
        }
        if only.is_empty() || only == "rebuild" {
            problems.extend(rebuild_everything(s).err());
        }
    });
    TRIPPED_AT.with(|t| *t.borrow_mut() = Some(s.document().undo));
    TRIPPING.set(false);
    problems
}

/// A COPY SAVED AND OPENED IN ANOTHER START OF THE PROGRAM IS THE SAME DOCUMENT, the session keeping its own file.
pub fn save_copy_open(s: &mut Session, path: &str) -> Result<(), String> {
    let before = s.document();
    // the copy of an earlier step is not the person's file: writing an emptied document over it is refused as a loss
    let _ = std::fs::remove_file(path);
    s.save_copy(path).map_err(|e| format!("a copy of the document could not be written to {path}: {e}"))?;
    let mut other = Session::start_on(s.same_machine());
    build::open_project(&mut other, path);
    let opened = other.document();
    if shape(&opened) == shape(&before) {
        Ok(())
    } else {
        Err(format!("a copy saved and opened is {:?}, the document was {:?}", shape(&opened), shape(&before)))
    }
}

/// Nodes that may stand in the timeline without changing a body: a sketch, a plane, an axis, a point - and an
/// extrusion joined to the body, which changes nothing when its prism lies inside it and builds all the same in the
/// professional systems. A cut that misses the body is the extrusion's own matrix to refuse.
const NODES_WITHOUT_A_BODY: [&str; 5] = ["Sketch", "Plane", "DatumPoint", "DatumAxis", "Combine"];

/// The bodies of their own, as the eye tells them apart: how much each holds, its faces and edges, how far it reaches,
/// whether it is shown and in which part - not their names, and not the bodies taken up into others. A node rebuilds its
/// body under a new name and leaves the old one consumed: the same body under another name is not a body changed; the
/// same body in another part is - a piece made a part of its own.
type Bodies = Vec<(i64, usize, Option<usize>, [i64; 6], bool, Option<String>)>;

fn bodies(doc: &Document) -> Bodies {
    let mm = |v: f64| (v * 1e3).round() as i64;
    let mut out: Bodies = doc
        .bodies
        .iter()
        .filter(|b| !b.consumed)
        .map(|b| {
            let (lo, hi) = (b.min.map(mm), b.max.map(mm));
            (mm(b.volume), b.faces, b.edges, [lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]], b.visible, b.part.clone())
        })
        .collect();
    out.sort();
    out
}

thread_local! {
    /// The document as the last step left it: the kinds of its green nodes and its bodies.
    static LAST: std::cell::RefCell<Option<(Vec<String>, Bodies)>> = const { std::cell::RefCell::new(None) };
    /// The state before the last one, green kinds sorted: an undo goes back to it.
    static PREV: std::cell::RefCell<Option<(Vec<String>, Bodies)>> = const { std::cell::RefCell::new(None) };
    /// How many steps of undo the document held after the last step.
    static STEPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// A NODE THAT CAME IN GREEN CHANGED A BODY. A tool that is not built must say why in red; one that goes green and
/// leaves every body as it was has done nothing and said nothing. Nodes are counted by kind, not told by name: a node
/// renamed is not a node come in.
///
/// THE STATE THE DOCUMENT WAS IN JUST BEFORE THE LAST STEP is not a node come in: an undo of a delete brings the base
/// back green over the body that stayed at its last good state while its dependents stood red. Only that one state:
/// any state seen earlier also let a fillet that did nothing pass unsaid the second time it came. Nor is a step that
/// took steps of undo away: it brought a state back, it built nothing - a parameter deleted and brought back makes its
/// node green again over the body it kept, with steps of the table's window between.
///
/// Reported behaviour: a fillet and a chamfer stand green in the timeline, and the edge they name stays sharp.
pub fn green_and_idle(doc: &Document) -> Vec<String> {
    let mut green: Vec<String> = doc.features.iter().filter(|f| f.error.is_none() && !f.suppressed && !NODES_WITHOUT_A_BODY.contains(&f.kind.as_str())).map(|f| f.kind.clone()).collect();
    green.sort();
    let now = bodies(doc);
    let last = LAST.with(|l| l.replace(Some((green.clone(), now.clone()))));
    let back = PREV.with(|p| p.replace(last.clone())).is_some_and(|(g, b)| g == green && b == now);
    let undone = doc.undo.len() < STEPS.with(|n| n.replace(doc.undo.len()));
    let Some((before, was)) = last else { return Vec::new() };
    if now != was || back || undone {
        return Vec::new();
    }
    let count = |of: &[String], kind: &str| of.iter().filter(|k| *k == kind).count();
    let mut kinds: Vec<&String> = green.iter().collect();
    kinds.sort();
    kinds.dedup();
    kinds.into_iter().filter(|k| count(&green, k) > count(&before, k)).map(|k| format!("a {k} node came in green and left every body as it was")).collect()
}

/// THE DOCUMENT IS WHOLE: every node stands in a part, a part holds one body, no sketch is lost, no two parts share a
/// name, no node is red without a reason.
pub fn whole(doc: &Document) -> Vec<String> {
    let mut problems = Vec::new();
    for f in &doc.features {
        if f.part.is_none() {
            problems.push(format!("the {} node {:?} stands in no part", f.kind, f.name));
        }
        if f.error.as_deref().is_some_and(|e| e.trim().is_empty()) {
            problems.push(format!("the {} node {:?} is red without a reason", f.kind, f.name));
        }
    }
    for p in &doc.parts {
        // A PART IS ONE BODY - one SOLID body. Surfaces are not bodies of the part in that sense: a face taken out
        // as a sheet, a patch spanned across edges or a stitched skin stands beside the solid until it is handed
        // back, and counting those as a second body would make every surface tool a fault.
        let bodies = doc.bodies.iter().filter(|b| b.part_key == Some(p.key) && b.visible && !b.consumed && !b.sheet).count();
        // THE NAMED EXCEPTION: the pieces of "split body", and of a cut or an intersection that went through the body,
        // stay bodies of the one part, as the professional systems leave a body split into several; a boolean between
        // the pieces works on them there and may bring the part back to one body.
        let split = doc.features.iter().any(|f| f.part.as_deref() == Some(p.name.as_str()) && f.bodies > 1 && f.kind != "ComponentPattern");
        if !p.assembly && bodies > 1 && !split {
            problems.push(format!("the part {:?} holds {bodies} bodies", p.name));
        }
        // TWO PARTS OF ONE NAME are a person's own doing when one is a clone of the other or came in from a file
        // named as one already there; what must not happen is two parts of one name with nothing to tell them apart.
        let by_hand = doc.parts.iter().any(|q| q.name == p.name && (q.clone_of.is_some() || p.clone_of.is_some()));
        if doc.parts.iter().filter(|q| q.name == p.name).count() > 1 && !by_hand {
            problems.push(format!("two parts are named {:?}", p.name));
        }
    }
    for sk in &doc.sketches {
        if !sk.part.as_ref().is_some_and(|p| doc.parts.iter().any(|q| &q.name == p)) {
            problems.push(format!("the sketch {:?} belongs to no part", sk.name));
        }
    }
    problems.dedup();
    problems
}

/// WHAT A SKETCH HOLDS, counted: its name, then its points, lines, arcs, circles, ellipses, splines, texts and
/// constraints. The timeline keeps a node for a sketch whatever the sketch holds, so a sketch emptied by a round
/// trip would be seen nowhere else.
#[derive(Clone, Debug, PartialEq)]
pub struct SketchShape {
    pub name: String,
    pub counts: [usize; 8],
}

/// The sketches of the document, counted.
pub fn sketch_shapes(doc: &Document) -> Vec<SketchShape> {
    doc.sketches.iter().map(|sk| SketchShape { name: sk.name.clone(), counts: [sk.points, sk.lines, sk.arcs, sk.circles, sk.ellipses, sk.splines, sk.texts, sk.constraints] }).collect()
}

/// A node of the timeline as a check compares it: its name, its kind, and why it is red, if it is.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeShape {
    pub name: String,
    pub kind: String,
    pub error: Option<String>,
}

/// The nodes of the timeline, in its order.
pub fn node_shapes(doc: &Document) -> Vec<NodeShape> {
    doc.features.iter().map(|f| NodeShape { name: f.name.clone(), kind: f.kind.clone(), error: f.error.clone() }).collect()
}

/// A named face of a body and where it lies, to a hundredth of a mm.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct NamedFace {
    name: u32,
    at: [i64; 3],
}

/// A body as a round trip compares it: its name, its volume in thousandths of a mm^3, its faces and edges, and the
/// names of its faces.
#[derive(Debug, PartialEq)]
struct BodyShape {
    name: String,
    volume: i64,
    faces: usize,
    edges: Option<usize>,
    named_faces: Vec<NamedFace>,
}

/// What a round trip compares: the timeline with its reasons, the bodies with their numbers and the names of their
/// faces, and the sketches with what they hold.
#[derive(Debug, PartialEq)]
struct Shape {
    nodes: Vec<NodeShape>,
    bodies: Vec<BodyShape>,
    sketches: Vec<SketchShape>,
}

/// The names of a body's faces, each with where its face lies (to a hundredth of a mm), in the order of the names: the
/// order the program lists faces in is its own business, a name that moves to another face is not.
fn named_faces(b: &qymcad::Solid) -> Vec<NamedFace> {
    let mut out: Vec<NamedFace> = b.face_names.iter().zip(&b.face_centres).map(|(n, c)| NamedFace { name: *n, at: c.map(|v| (v * 100.0).round() as i64) }).collect();
    out.sort();
    out
}

fn shape(doc: &Document) -> Shape {
    Shape {
        nodes: node_shapes(doc),
        bodies: doc.bodies.iter().map(|b| BodyShape { name: b.name.clone(), volume: (b.volume * 1e3).round() as i64, faces: b.faces, edges: b.edges, named_faces: named_faces(b) }).collect(),
        sketches: sketch_shapes(doc),
    }
}

/// UNDO AND THEN REDO GIVE BACK THE SAME DOCUMENT.
pub fn undo_redo(s: &mut Session) -> Result<(), String> {
    let before = s.document();
    if before.undo.is_empty() {
        return Ok(());
    }
    s.chord(Modifiers::COMMAND, Key::Z).chord(Modifiers::COMMAND, Key::Y);
    let after = s.document();
    if shape(&after) == shape(&before) && after.undo == before.undo {
        Ok(())
    } else {
        Err(format!("undo and redo changed the document: {:?} became {:?}; the steps {:?} became {:?}", shape(&before), shape(&after), before.undo, after.undo))
    }
}

/// SAVED AND OPENED IN ANOTHER START OF THE PROGRAM, THE DOCUMENT IS THE SAME.
pub fn save_open(s: &mut Session, path: &str) -> Result<(), String> {
    let before = s.document();
    build::save_as(s, path);
    let mut other = Session::start_on(s.same_machine());
    build::open_project(&mut other, path);
    let opened = other.document();
    if shape(&opened) == shape(&before) {
        Ok(())
    } else {
        Err(format!("saved as {path} and opened, the document is {:?}, it was {:?}", shape(&opened), shape(&before)))
    }
}

/// REBUILDING EVERYTHING FROM THE START GIVES THE SAME AS THE STEPS THAT MADE IT.
pub fn rebuild_everything(s: &mut Session) -> Result<(), String> {
    let before = s.document();
    let (edit, rebuild) = (s.word("menu-edit"), s.word("menu-rebuild"));
    s.menu(&[&edit, &rebuild]);
    let after = s.document();
    if shape(&after) == shape(&before) {
        Ok(())
    } else {
        Err(format!("rebuilt from the start, the document is {:?}, step by step it was {:?}", shape(&after), shape(&before)))
    }
}

#[cfg(test)]
mod tests {
    use qymcad::{Document, Feature, Part, SketchInfo, Solid};

    /// A document of parts and bodies, named, for the oracle of wholeness.
    fn doc(parts: &[&str], bodies: &[(&str, &str)], features: &[(&str, Option<&str>, Option<&str>)], sketches: &[(&str, Option<&str>)]) -> Document {
        Document {
            path: None,
            unsaved: false,
            editing: None,
            context: String::new(),
            datums: vec![],
            parts: parts
                .iter()
                .enumerate()
                .map(|(i, n)| Part {
                    name: n.to_string(),
                    key: i as u64 + 1,
                    assembly: false,
                    parent: None,
                    visible: true,
                    grounded: false,
                    clone_of: None,
                    at: [0.0; 3],
                    axes: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                })
                .collect(),
            sketches: sketches
                .iter()
                .map(|(n, p)| SketchInfo {
                    name: n.to_string(),
                    part: p.map(str::to_string),
                    points: 0,
                    lines: 0,
                    construction: 0,
                    arcs: 0,
                    circles: 0,
                    ellipses: 0,
                    splines: 0,
                    texts: 0,
                    notes: 0,
                    text_fonts: vec![],
                    constraint_kinds: vec![],
                    places: vec![],
                    picked: 0,
                    seat: "XY".into(),
                    min: [0.0; 2],
                    max: [0.0; 2],
                    constraints: 0,
                    dof: 0,
                    redundant: 0,
                })
                .collect(),
            features: features
                .iter()
                .map(|(n, p, e)| Feature {
                    name: n.to_string(),
                    kind: "Extrude".into(),
                    part: p.map(str::to_string),
                    suppressed: false,
                    error: e.map(str::to_string),
                    warning: None,
                    key: 0,
                    bodies: 1,
                })
                .collect(),
            bodies: bodies
                .iter()
                .map(|(n, p)| Solid {
                    name: n.to_string(),
                    part: Some(p.to_string()),
                    part_key: parts.iter().position(|q| q == p).map(|i| i as u64 + 1),
                    colour: [200, 200, 200],
                    volume: 1.0,
                    area: 1.0,
                    faces: 6,
                    face_names: vec![],
                    face_centres: vec![],
                    edges: Some(12),
                    min: [0.0; 3],
                    max: [1.0; 3],
                    visible: true,
                    consumed: false,
                    sheet: false,
                    lit_by_the_surface: 0,
                })
                .collect(),
            parameters: vec![],
            joints: vec![],
            mates: vec![],
            section: false,
            undo: vec![],
            redo: vec![],
        }
    }

    /// A WHOLE DOCUMENT PASSES, and each way of not being whole is named.
    #[test]
    fn each_way_of_not_being_whole_is_named() {
        let fine = doc(&["Part 1"], &[("Body 1", "Part 1")], &[("Extrusion", Some("Part 1"), None)], &[("Sketch 1", Some("Part 1"))]);
        assert!(super::whole(&fine).is_empty(), "a whole document: {:?}", super::whole(&fine));
        let cases = [
            (doc(&["Part 1"], &[], &[("Extrusion", None, None)], &[]), "stands in no part"),
            (doc(&["Part 1"], &[], &[("Extrusion", Some("Part 1"), Some(" "))], &[]), "red without a reason"),
            (doc(&["Part 1"], &[("Body 1", "Part 1"), ("Body 2", "Part 1")], &[], &[]), "holds 2 bodies"),
            (doc(&["Part 1", "Part 1"], &[], &[], &[]), "two parts are named"),
            (doc(&["Part 1"], &[], &[], &[("Sketch 1", Some("Part 9"))]), "belongs to no part"),
        ];
        for (d, words) in cases {
            let said = super::whole(&d);
            assert!(said.iter().any(|p| p.contains(words)), "{words:?} was not said of a document that deserves it: {said:?}");
        }
    }
}

#[cfg(test)]
mod green_and_idle_tests {
    use super::green_and_idle;
    use qymcad::{Document, Feature, Solid};

    fn node(name: &str, kind: &str, error: Option<&str>) -> Feature {
        Feature { name: name.into(), kind: kind.into(), part: Some("Part 1".into()), suppressed: false, error: error.map(Into::into), warning: None, key: 0, bodies: 1 }
    }

    fn block(volume: f64, faces: usize) -> Solid {
        Solid {
            name: "Body 1".into(),
            part: Some("Part 1".into()),
            part_key: Some(1),
            colour: [0; 3],
            volume,
            area: 0.0,
            faces,
            face_names: Vec::new(),
            face_centres: Vec::new(),
            edges: Some(12),
            min: [0.0; 3],
            max: [40.0, 30.0, 10.0],
            visible: true,
            consumed: false,
            sheet: false,
            lit_by_the_surface: 0,
        }
    }

    fn doc(features: Vec<Feature>, body: Solid) -> Document {
        Document {
            path: None,
            unsaved: true,
            parts: Vec::new(),
            sketches: Vec::new(),
            editing: None,
            context: String::new(),
            datums: Vec::new(),
            features,
            bodies: vec![body],
            parameters: Vec::new(),
            joints: Vec::new(),
            mates: Vec::new(),
            section: false,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    /// A fillet that came in green over the very body there was before is told; one that changed it, one that came
    /// in red and a node renamed are not.
    #[test]
    fn a_green_node_that_changed_nothing_is_told() {
        let base = || vec![node("Sketch 1", "Sketch", None), node("Extrude 1", "Extrude", None)];
        let with = |f: Feature| base().into_iter().chain([f]).collect::<Vec<_>>();
        assert!(green_and_idle(&doc(base(), block(12000.0, 6))).is_empty(), "the first reading has nothing to compare with");
        let idle = green_and_idle(&doc(with(node("Fillet 1", "Fillet", None)), block(12000.0, 6)));
        assert!(idle.len() == 1 && idle[0].contains("Fillet"), "a green fillet that left the block as it was is not told: {idle:?}");
        let _ = green_and_idle(&doc(base(), block(12000.0, 6)));
        let made = green_and_idle(&doc(with(node("Fillet 1", "Fillet", None)), block(11965.6, 7)));
        assert!(made.is_empty(), "a fillet that rounded the edge is told: {made:?}");
        let _ = green_and_idle(&doc(base(), block(12000.0, 6)));
        let red = green_and_idle(&doc(with(node("Fillet 1", "Fillet", Some("the radius is too large"))), block(12000.0, 6)));
        assert!(red.is_empty(), "a fillet that came in red with its reason is told: {red:?}");
        let _ = green_and_idle(&doc(base(), block(12000.0, 6)));
        let renamed = green_and_idle(&doc(vec![node("Sketch 1", "Sketch", None), node("Base", "Extrude", None)], block(12000.0, 6)));
        assert!(renamed.is_empty(), "a node renamed is told as a node come in: {renamed:?}");
        // the node rebuilds the body under a new name and leaves the old one taken up: the same body all the same
        let _ = green_and_idle(&doc(base(), block(12000.0, 6)));
        let mut again = doc(with(node("Fillet 1", "Fillet", None)), block(12000.0, 6));
        again.bodies[0].consumed = true;
        again.bodies.push(Solid { name: "Body 2".into(), ..block(12000.0, 6) });
        let renamed_body = green_and_idle(&again);
        assert!(renamed_body.len() == 1, "a green fillet that left the same block under a new name is not told: {renamed_body:?}");
    }
}
