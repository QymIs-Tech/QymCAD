//! THE FAULTS OF THE DOCUMENT, gathered in one list: what is red or yellow anywhere, in words, with the way there.
//!
//! Each kind of fault is made where it belongs and was shown only there: a node of the timeline that did not build in
//! the tree of its part, a mate that does not hold in the list of its assembly, a sketch whose constraints conflict
//! while it is open, a parameter whose formula does not evaluate in the table of parameters. Nothing counted them
//! together, so a document with a red cut in another part looked healthy from where a person stood. This list is the
//! one source for the counter in the status line and for the window of faults; it is derived from the document on
//! demand and remembered by a key, never kept in a field of the application.

use crate::{Caches, Notice, Sel, Tone, WinAsk, WinKind, Windows};
use qymcad_core::model::{Id, Project};

/// WHERE A FAULT IS - what a click on it leads to.
#[derive(Clone, Debug, PartialEq)]
pub enum FaultPlace {
    /// A node of the timeline, by its id.
    Node(Id),
    /// A mate, a mate constraint or a relation, by its id.
    Mate(Id),
    /// A sketch, by its id.
    Sketch(Id),
    /// A parameter of the document, by its name.
    Parameter(String),
    /// A named dimension (a driver) by its place in the list of drivers.
    Driver(usize),
}

/// ONE FAULT: how bad it is, what is wrong, where it sits in words, and the place a click leads to.
#[derive(Clone, Debug, PartialEq)]
pub struct Fault {
    pub tone: Tone,
    pub words: String,
    /// The path as the window of parameters writes it: part, sketch, operation; assembly and mate.
    pub path: String,
    pub place: FaultPlace,
    /// The component the fault belongs to: the window lists the current context first. `None` is the document.
    pub owner: Option<Id>,
}

/// HOW MANY of each tone the document holds now.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FaultCount {
    pub errors: usize,
    pub warnings: usize,
}

/// The remembered list and the key it was made for.
pub struct FaultMemo {
    key: u64,
    faults: std::rc::Rc<Vec<Fault>>,
}

/// THE KEY OF WHAT THE FAULTS ARE MADE FROM: the document itself and what its last rebuild and placement left.
///
/// `state_key` holds the document and leaves out the derived marks; those are what this list reads, so they are
/// added here. Measured cost: the state key is already taken once a frame for "is anything unsaved".
pub fn faults_key(project: &Project) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    project.state_key().hash(&mut h);
    // the marks of the last rebuild, in the order of their nodes: a map walks in no order of its own
    let mut marked: Vec<Id> = project.regen_errors.keys().chain(project.regen_warnings.keys()).copied().collect();
    marked.sort();
    for id in marked {
        id.hash(&mut h);
        format!("{:?}{:?}", project.regen_errors.get(&id), project.regen_warnings.get(&id)).hash(&mut h);
    }
    project.mates_violated.hash(&mut h);
    project.rollback.hash(&mut h);
    h.finish()
}

/// THE FAULTS OF THE DOCUMENT AS IT IS NOW, remembered until it changes.
pub fn document_faults(cache: &Caches, project: &Project) -> std::rc::Rc<Vec<Fault>> {
    let key = faults_key(project);
    if let Some(m) = &*cache.faults.borrow() {
        if m.key == key {
            return m.faults.clone();
        }
    }
    let faults = std::rc::Rc::new(gather(project));
    *cache.faults.borrow_mut() = Some(FaultMemo { key, faults: faults.clone() });
    faults
}

/// The count of each tone, for the badges.
pub fn fault_count(faults: &[Fault]) -> FaultCount {
    FaultCount { errors: faults.iter().filter(|f| f.tone == Tone::Error).count(), warnings: faults.iter().filter(|f| f.tone == Tone::Warning).count() }
}

/// THE FAULTS IN THE ORDER THE WINDOW SHOWS THEM: those of the context a person is working in first, then the rest,
/// each group errors before warnings and in the order of the document.
pub fn faults_in_order(faults: &[Fault], project: &Project, ctx: Id) -> Vec<Fault> {
    let here = |f: &Fault| f.owner.is_some_and(|o| o == ctx || project.component_is_within(o, ctx));
    let mut out: Vec<Fault> = faults.to_vec();
    out.sort_by_key(|f| (!here(f), f.tone != Tone::Error));
    out
}

/// WHERE A CLICK ON A FAULT GOES: the component to step into and what to select there - the same request the window
/// of parameters makes for a driver's path, so both roads are one. `None` for a parameter of the document, which
/// lives in the window of parameters and has nothing to select.
pub fn fault_goes_to(project: &Project, place: &FaultPlace) -> Option<GoTo> {
    match place {
        FaultPlace::Node(node) => {
            let ti = project.timeline.iter().position(|n| n.id == *node)?;
            Some(GoTo { owner: project.timeline[ti].parent, sel: Sel::Feature(ti) })
        }
        FaultPlace::Sketch(sketch) => Some(GoTo { owner: project.sketch_owner(*sketch), sel: Sel::Sketch(project.sketch_index(*sketch)?) }),
        FaultPlace::Mate(id) => Some(GoTo { owner: mate_home(project, *id), sel: Sel::Joint(*id) }),
        FaultPlace::Driver(k) => {
            let target = project.named_dims.get(*k)?.target.clone();
            match target {
                qymcad_core::model::DimTarget::Sketch { sketch, .. } => fault_goes_to(project, &FaultPlace::Sketch(sketch)),
                qymcad_core::model::DimTarget::Feature { node, .. } => fault_goes_to(project, &FaultPlace::Node(node)),
            }
        }
        FaultPlace::Parameter(_) => None,
    }
}

/// THE ROAD TO A PLACE: step into `owner` (when there is one) and select `sel` - one request, because stepping in
/// clears the selection.
#[derive(Clone, PartialEq)]
pub struct GoTo {
    pub owner: Option<Id>,
    pub sel: Sel,
}

/// GO TO A FAULT: step into its component and select it, as a click on a driver's path in the window of parameters
/// does - both push the same request. A parameter of the document has nothing to select: the window of parameters
/// opens with the table narrowed to its name.
///
/// A DRIVER WHOSE DIMENSION IS GONE leads to the table as well, narrowed to its name: the row there says the dimension
/// was not found and carries the bin that takes the driver away, while its sketch has nothing left to show. Reported
/// behaviour: the warning badge led into a sketch of the part, and nothing on screen said what was wrong.
///
/// Returns what to say on the status line: what is wrong and where, in the tone of the fault - arriving somewhere
/// without the words is arriving without knowing why. The step into a part writes its context on the line a frame
/// later, so the words go along with the request too and are said once the step is made. Reported behaviour: a click
/// on the one fault of another part left "Context: Part 1" on the line.
pub fn lead_to(fault: &Fault, project: &Project, win: &mut Windows, par_search: &mut String, ask: &mut Vec<WinAsk>) -> Notice {
    let to_table = match &fault.place {
        FaultPlace::Parameter(name) => Some(name.clone()),
        FaultPlace::Driver(k) => Some(project.named_dims.get(*k).map(|d| d.name.clone()).unwrap_or_default()),
        _ => None,
    };
    let words = if fault.path.is_empty() { fault.words.clone() } else { qymcad_i18n::tr2("fault-said", "where", &fault.path, "what", &fault.words) };
    let said = Notice { tone: fault.tone, words };
    match (to_table, fault_goes_to(project, &fault.place)) {
        (Some(name), _) => {
            *par_search = name;
            win.open(WinKind::Params);
        }
        (None, Some(g)) => ask.push(WinAsk::GoTo { owner: g.owner, sel: g.sel, said: Some(said.clone()) }),
        (None, None) => {}
    }
    said
}

/// A CLICK ON THE COUNTER OR ON THE WORDS OF A FAULT: with one fault it goes straight there, with more the window of
/// faults opens - a list to choose from is a step that one fault does not need.
/// Returns what to say on the status line when it went straight to the one fault.
pub fn lead_to_faults(faults: &[Fault], project: &Project, win: &mut Windows, par_search: &mut String, ask: &mut Vec<WinAsk>) -> Option<Notice> {
    match faults {
        [] => None,
        [one] => Some(lead_to(one, project, win, par_search, ask)),
        _ => {
            win.open(WinKind::Faults);
            None
        }
    }
}

/// WHAT THE LAST REBUILD LEFT, as the line it ends with: the first node that did not build, in the order of the
/// timeline, and how many did not; failing that, the first mate the placement after it could not hold; failing that,
/// the first node that built only in part. `None` when all of it built and holds.
///
/// Reported behaviour: a parameter made a cut fail, the cut and every node after it went red in the tree, and the
/// status line said so in the plain colour of "Ready".
pub fn rebuild_left(project: &Project) -> Option<Notice> {
    let say = |key: &str, marks: &std::collections::HashMap<Id, qymcad_core::errors::CoreError>| {
        let n = marks.len().to_string();
        project.timeline.iter().find_map(|node| {
            let e = marks.get(&node.id)?;
            // the count and the place, then why: the reason closes the line in every language of the catalogue
            let head = qymcad_i18n::tr2(key, "n", &n, "where", &path_in_words(&project.feature_breadcrumbs(node.id)));
            Some(format!("{head} {}", qymcad_i18n::error_words::error_text(e)))
        })
    };
    let mates = || {
        let faulty = mate_faults(project);
        faulty.first().map(|f| Notice::error(format!("{} {}", qymcad_i18n::tr2("io-placement-left-errors", "n", &faulty.len().to_string(), "where", &f.path), f.words)))
    };
    say("io-rebuild-left-errors", &project.regen_errors).map(Notice::error).or_else(mates).or_else(|| say("io-rebuild-left-warnings", &project.regen_warnings).map(Notice::warning))
}

/// WHAT AN EDIT OF THE PARAMETERS LEFT: the first parameter whose formula gives no value, and how many give none.
/// `None` when every formula gives a value.
pub fn parameters_left(project: &Project) -> Option<Notice> {
    let broken: Vec<Fault> = parameter_faults(project).into_iter().filter(|f| f.tone == Tone::Error).collect();
    broken.first().map(|f| Notice::error(format!("{} {}", qymcad_i18n::tr2("par-left-errors", "n", &broken.len().to_string(), "where", &f.path), f.words)))
}

/// The assembly a mate, a mate constraint or a relation lives in.
fn mate_home(project: &Project, id: Id) -> Option<Id> {
    if let Some(j) = project.joints.iter().find(|j| j.id == id) {
        return project.joint_home(j).or(Some(project.root));
    }
    if let Some(r) = project.relations.iter().find(|r| r.id == id) {
        return project.joints.iter().find(|j| j.id == r.a || j.id == r.b).and_then(|j| project.joint_home(j)).or(Some(project.root));
    }
    // a mate constraint lives where all it holds meet, as a mate lives where its two parts meet
    let g = project.mate_constraints.iter().find(|g| g.id == id)?;
    let held: Vec<Id> = g.members.iter().copied().chain(g.anchors.iter().filter_map(|c| project.connector(*c).map(|x| x.owner))).chain(g.faces.iter().map(|(o, _)| *o)).collect();
    held.split_first().and_then(|(first, rest)| rest.iter().try_fold(*first, |home, m| project.common_ancestor(home, *m))).or(Some(project.root))
}

/// The breadcrumbs of a component with a name after them, as the parameters window writes a path.
fn path_to(project: &Project, owner: Option<Id>, name: &str) -> String {
    let own = owner.map(|c| project.component_breadcrumbs(c)).unwrap_or_default();
    let path = match (own.is_empty(), name.is_empty()) {
        (true, _) => name.to_string(),
        (false, true) => own,
        (false, false) => format!("{own}.{name}"),
    };
    path_in_words(&path)
}

/// A PATH OF THE DOCUMENT IN THE LANGUAGE OF THE INTERFACE: the document stores the name of a node it named itself as a
/// key of the catalogue ("feat-name-combine-cut"), and the tree says it in words; each link of the path is said as the
/// tree says it. Found by the check of the window of faults: the path of a cut read "Housing.feat-name-combine-cut".
pub fn path_in_words(path: &str) -> String {
    path.split('.').map(qymcad_i18n::name).collect::<Vec<_>>().join(".")
}

fn gather(project: &Project) -> Vec<Fault> {
    let mut out = Vec::new();
    // THE NODES, in the order of the timeline: a node that did not build is an error, one that built a part of what
    // it was asked is a warning. A node past the rollback bar is not built and has neither.
    for n in &project.timeline {
        let (tone, e) = match (project.regen_errors.get(&n.id), project.regen_warnings.get(&n.id)) {
            (Some(e), _) => (Tone::Error, e),
            (None, Some(w)) => (Tone::Warning, w),
            (None, None) => continue,
        };
        out.push(Fault { tone, words: qymcad_i18n::error_words::error_text(e), path: path_in_words(&project.feature_breadcrumbs(n.id)), place: FaultPlace::Node(n.id), owner: n.parent });
    }
    out.extend(mate_faults(project));
    // THE SKETCHES whose constraints cannot hold at once. Redundancy that agrees with itself is not an error and is not
    // counted (see `flagged_redundant`); a conflict is measured by the geometry and remembered by each sketch.
    for (si, s) in project.sketches.iter().enumerate() {
        let n = project.sketch_conflicts(si).len();
        if n > 0 {
            let owner = project.sketch_owner(s.id);
            out.push(Fault {
                tone: Tone::Error,
                words: qymcad_i18n::tr1("fault-sketch-conflict", "n", &n.to_string()),
                path: path_in_words(&project.sketch_breadcrumbs(s.id)),
                place: FaultPlace::Sketch(s.id),
                owner,
            });
        }
    }
    out.extend(parameter_faults(project));
    out
}

/// THE MATES THAT DO NOT HOLD, as the list of each assembly marks them; a global mate is listed at the root as well, and
/// once here.
fn mate_faults(project: &Project) -> Vec<Fault> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for c in project.components.iter().filter(|c| !project.component_is_part(c.id)) {
        for e in project.mate_timeline(c.id) {
            let words = match e.state {
                qymcad_core::feature::MateState::Ok => continue,
                qymcad_core::feature::MateState::Faulty(why) => qymcad_i18n::tr(why),
                qymcad_core::feature::MateState::Violated => qymcad_i18n::tr("jp-dof-conflict"),
            };
            if !seen.insert(e.id) {
                continue;
            }
            let owner = mate_home(project, e.id);
            let name = if e.name.is_empty() { qymcad_i18n::tr(e.kind_label) } else { e.name.clone() };
            out.push(Fault { tone: Tone::Error, words, path: path_to(project, owner, &name), place: FaultPlace::Mate(e.id), owner });
        }
    }
    out
}

/// THE PARAMETERS whose formula gives no value or whose name a formula reads as a constant, and the drivers whose
/// dimension is gone.
fn parameter_faults(project: &Project) -> Vec<Fault> {
    let mut out = Vec::new();
    for (i, p) in project.parameters.iter().enumerate().filter(|(_, p)| !p.expr.trim().is_empty()) {
        // asked as the table asks it, so a parameter in a circle is a fault with its chain, not a stale number
        if let Err(e) = project.check_param_text(i, &p.expr) {
            out.push(Fault { tone: Tone::Error, words: qymcad_i18n::error_words::expr_error_text(&e), path: p.name.clone(), place: FaultPlace::Parameter(p.name.clone()), owner: None });
        }
    }
    // A PARAMETER NAMED `e`, `pi` OR `tau`: naming refuses such a name, but a document read from disk may hold one. A
    // formula reads the constant there, so the parameter has a value in the table and no formula reaches it.
    for p in project.parameters.iter().filter(|p| qymcad_core::expr::is_constant(&p.name)) {
        out.push(Fault { tone: Tone::Warning, words: qymcad_i18n::tr("par-name-is-constant"), path: p.name.clone(), place: FaultPlace::Parameter(p.name.clone()), owner: None });
    }
    // A DIMENSION OF A SKETCH OR A NUMBER OF A FEATURE NAMED SO, the same; a click leads to where it stands - its sketch,
    // its node - as a click on a parameter leads to its row.
    for d in project.named_dims.iter().filter(|d| qymcad_core::expr::is_constant(&d.name)) {
        let place = match &d.target {
            qymcad_core::model::DimTarget::Sketch { sketch, .. } => FaultPlace::Sketch(*sketch),
            qymcad_core::model::DimTarget::Feature { node, .. } => FaultPlace::Node(*node),
        };
        let at = project.driver_path(&d.target);
        let path = if at.is_empty() { d.name.clone() } else { format!("{}.{}", path_in_words(&at), d.name) };
        let owner = fault_goes_to(project, &place).and_then(|g| g.owner);
        out.push(Fault { tone: Tone::Warning, words: qymcad_i18n::tr("par-name-is-constant"), path, place, owner });
    }
    for (k, d) in project.named_dims.iter().enumerate() {
        if project.named_dim_value(d).is_none() {
            // the path is where the dimension stood: the name is in the words already
            let path = path_in_words(&project.driver_path(&d.target));
            out.push(Fault { tone: Tone::Warning, words: qymcad_i18n::tr1("par-dim-missing", "name", &d.name), path, place: FaultPlace::Driver(k), owner: None });
        }
    }
    out
}
