//! GENERATED SESSIONS: chains of a person's steps, grown from a seed out of the descriptions of the tools.
//!
//! A chain is what a person does in a sitting, in the order they do it and with the slips they make: a tool taken
//! by one of its ways in, its clicks, a value typed into its field - one it takes or one it must refuse - Enter and
//! Esc in between, a click on nothing, a step taken back and put again, the document saved and opened in the middle.
//! The descriptions say what each tool is taken by, clicked at and typed into; the seed says which of them, in what
//! order. The same seed grows the same chain, so a chain that finds something can be played again.
//!
//! Playing a chain is done through the session alone, and every step is looked at by the oracles of the process. A
//! step the session cannot aim - the place of a click is covered, the field it types into is not open - is skipped,
//! as a person would not make it; a step after which the oracles find the program not right ends the chain with
//! the step, its number and their words.
use qymcad::{Key, Modifiers, Session};

use crate::build;
use crate::contract::fixtures::Fixture;
use crate::contract::run;
use crate::contract::{Flow, Tool};

/// A STREAM OF NUMBERS FROM A SEED, the same stream for the same seed on every machine (splitmix64). Written out here
/// rather than taken from a crate: the chains depend on the exact numbers, and a library free to change its stream
/// between versions would change every chain grown before.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    /// The next number of the stream.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A number below `n`; `n` must not be zero.
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }

    /// True `percent` times out of a hundred.
    pub fn chance(&mut self, percent: u64) -> bool {
        self.next_u64() % 100 < percent
    }
}

/// ONE STEP OF A PERSON, named by the tool it belongs to where it belongs to one - so a chain reads as what was done.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    /// The tool taken by the way into it numbered `entry` in its description.
    Take {
        tool: &'static str,
        entry: usize,
    },
    /// The ordinary click (or pick) numbered `at` of the tool.
    Click {
        tool: &'static str,
        at: usize,
    },
    /// The gesture that finishes the tool: the last click of a drawing, Enter of a command.
    Finish {
        tool: &'static str,
    },
    /// `text` typed into the field numbered `field` of the tool - a value it takes, or one it must refuse.
    Value {
        tool: &'static str,
        field: usize,
        text: String,
    },
    /// `text` typed into the field of words of the tool.
    Words {
        tool: &'static str,
        text: String,
    },
    Enter,
    Escape,
    /// A click on nothing - an empty corner of the canvas.
    Miss,
    Undo,
    Redo,
    /// The document saved under a name of its own and opened again from it.
    SaveAndOpen,
}

/// WHERE A CHAIN IS PLAYED, and the tools it takes there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Scene {
    /// A sketch open on XY, empty: every tool that draws or dimensions in a sketch.
    Sketch,
}

impl Scene {
    /// The tools of the scene - those whose descriptions start them in a sketch.
    pub fn tools(self) -> Vec<&'static Tool> {
        match self {
            Scene::Sketch => crate::tools::ALL
                .iter()
                .copied()
                .filter(|t| {
                    matches!(
                        t.fixture,
                        Fixture::SketchOnXy
                            | Fixture::RectangleInSketch
                            | Fixture::CircleInSketch
                            | Fixture::ArcInSketch
                            | Fixture::TwoLinesInSketch
                            | Fixture::TwoLinesOfTwoLengths
                            | Fixture::TwoCirclesInSketch
                    )
                })
                .collect(),
        }
    }

    /// The window the chain starts in.
    pub fn start(self) -> Session {
        match self {
            Scene::Sketch => Fixture::SketchOnXy.start(),
        }
    }
}

/// A LEVEL OF THE RUN OF THE CHECKS, and how much of the generated sessions it plays: none at every edit, short chains
/// before a commit, long ones before a release.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tier {
    Fast,
    Full,
    Release,
}

impl Tier {
    /// Every level, from the cheapest.
    pub const ALL: [Tier; 3] = [Tier::Fast, Tier::Full, Tier::Release];

    /// The level asked for by `QYMCAD_TIER` (`fast`, `full`, `release`); the full one when nothing is asked.
    pub fn asked() -> Tier {
        match std::env::var("QYMCAD_TIER").unwrap_or_default().trim() {
            "fast" => Tier::Fast,
            "release" => Tier::Release,
            _ => Tier::Full,
        }
    }

    /// How many chains each slot plays at this level, and how many steps each chain has.
    pub fn chains(self) -> (usize, usize) {
        match self {
            Tier::Fast => (0, 0),
            Tier::Full => (1, 40),
            Tier::Release => (6, 150),
        }
    }

    /// What one slot may spend on its chains at this level, in seconds. The probe of a slot has 1800 seconds in all,
    /// and a chain that fails also spends the time of its shrinking - up to `SHRINK_SECONDS`.
    pub fn slot_budget(self) -> f64 {
        match self {
            Tier::Fast => 0.0,
            Tier::Full => 30.0,
            Tier::Release => 240.0,
        }
    }
}

/// THE SLOTS the chains of a level are played in: one probe each, the probes side by side.
pub const SLOTS: u64 = 8;

/// The seeds, and the length of each chain, that the slot `slot` plays at `tier`: the slots share out the seeds
/// 1, 2, 3 ... so that no two slots play the same one.
pub fn seeds(tier: Tier, slot: u64) -> Vec<(u64, usize)> {
    let (count, len) = tier.chains();
    (0..count as u64).map(|k| (1 + slot + SLOTS * k, len)).collect()
}

/// WHAT A CHAIN COSTS, measured on the machine the set is kept on (a debug build, one process a chain): the start of
/// the window and the three round trips once, 2.6 seconds, and 0.11 of a second for every step. A step of a short
/// chain comes cheaper (0.05 s over 40 steps) - the document grows as the chain goes on - so the price is the one of
/// the long chains: six of 150 steps took 112 seconds.
pub const CHAIN_SECONDS: f64 = 2.6;
pub const STEP_SECONDS: f64 = 0.11;

/// The tool of the scene called `id`.
fn tool(id: &str) -> &'static Tool {
    crate::tools::ALL.iter().copied().find(|t| t.id == id).unwrap_or_else(|| panic!("no tool is described as {id:?}"))
}

/// How many ordinary clicks a tool takes: the picks of a command, the places of a drawing.
fn clicks_of(t: &Tool) -> usize {
    match t.flow {
        Flow::Command => t.picks.len(),
        Flow::Drawing(places, _) => places.len(),
        Flow::Action => 0,
    }
}

/// GROW THE CHAIN OF `len` STEPS for `scene` from `seed`.
///
/// Mostly the steps of the tool in hand - its clicks, its fields, its finish - since that is what a person mostly
/// does; among them the slips: another tool taken halfway, Esc, a click on nothing, a value the field must refuse,
/// a step taken back and put again, the document saved and opened again.
pub fn grow(seed: u64, len: usize, scene: Scene) -> Vec<Step> {
    let tools = scene.tools();
    assert!(!tools.is_empty(), "the scene {scene:?} has no tool to take");
    let mut rng = Rng::new(seed);
    let mut hand: Option<&'static Tool> = None;
    let mut steps = Vec::with_capacity(len);
    while steps.len() < len {
        // every draw is made whatever is chosen, so the stream - and the chain - does not depend on the menu's length
        let other = tools[rng.below(tools.len())];
        let mut menu: Vec<(u64, Step)> = vec![
            (if hand.is_none() { 60 } else { 12 }, Step::Take { tool: other.id, entry: rng.below(other.entries.len()) }),
            (5, Step::Enter),
            (6, Step::Escape),
            (5, Step::Miss),
            (4, Step::Undo),
            (3, Step::Redo),
            (2, Step::SaveAndOpen),
        ];
        if let Some(h) = hand {
            let clicks = clicks_of(h);
            if clicks > 0 {
                menu.push((36, Step::Click { tool: h.id, at: rng.below(clicks) }));
            }
            menu.push((9, Step::Finish { tool: h.id }));
            if !h.fields.is_empty() {
                let at = rng.below(h.fields.len());
                let f = &h.fields[at];
                let text = if rng.chance(70) {
                    let good = run::valid_inputs(f);
                    good[rng.below(good.len())].0.clone()
                } else {
                    let bad = run::invalid_inputs(f);
                    bad[rng.below(bad.len())].clone()
                };
                menu.push((10, Step::Value { tool: h.id, field: at, text }));
            }
            if let Some(w) = h.words.first() {
                let pool: Vec<&str> = std::iter::once(w.typical).chain(w.valid.iter().copied()).chain(w.invalid.iter().copied()).collect();
                menu.push((6, Step::Words { tool: h.id, text: pool[rng.below(pool.len())].to_string() }));
            }
        }
        let total: u64 = menu.iter().map(|(w, _)| w).sum();
        let mut roll = rng.next_u64() % total;
        let step = menu
            .into_iter()
            .find_map(|(w, step)| {
                if roll < w {
                    Some(step)
                } else {
                    roll -= w;
                    None
                }
            })
            .expect("the roll falls on a step");
        match &step {
            Step::Take { tool: id, .. } => hand = Some(tool(id)),
            // the first Esc may only drop the shape being drawn, the second puts the tool down: which one this was
            // is not known ahead, and the chain goes on either way
            Step::Escape if rng.chance(50) => hand = None,
            _ => {}
        }
        steps.push(step);
    }
    steps
}

/// WHAT A PLAYED CHAIN CAME TO: how many steps were made, and which were skipped because the session could not aim
/// them.
#[derive(Debug)]
pub struct Played {
    pub done: usize,
    pub skipped: Vec<usize>,
}

/// WHY A CHAIN STOPPED: the number of the step after which the program was found not right, and the words that say
/// so.
#[derive(Debug)]
pub struct Stopped {
    pub at: usize,
    pub said: String,
}

/// Run `f` and answer where it failed, if it did: its words and the file the failure was raised in.
fn failure(f: impl FnOnce()) -> Option<(String, String)> {
    let raised = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let hook = std::panic::take_hook();
    let seen = raised.clone();
    std::panic::set_hook(Box::new(move |info| {
        if let Ok(mut f) = seen.lock() {
            *f = info.location().map(|l| l.file().to_string()).unwrap_or_default();
        }
    }));
    let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    std::panic::set_hook(hook);
    let e = out.err()?;
    let words = e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
    let file = raised.lock().map(|f| f.clone()).unwrap_or_default();
    Some((words, file))
}

/// Is a failure raised in `file` one of aiming - the session could not find what the step points at - rather than
/// of the program? Aiming is done by the door and by the gestures of the checks; everything else is the program.
/// The door is `gui/session.rs` and its modules in `gui/session/`: a miss raised in `session/finding.rs` read as the
/// program's failure stopped the kept chain of seed 43 at a step that a miss of aim only skips.
fn is_aiming(file: &str) -> bool {
    file.contains("gui/session.rs") || file.contains("gui/session/") || file.contains("qymcad-acceptance")
}

/// How much memory this process holds now, in megabytes; `None` where the system does not say (no `/proc`).
fn resident_mb() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let kb: u64 = status.lines().find_map(|l| l.strip_prefix("VmRSS:"))?.trim().trim_end_matches("kB").trim().parse().ok()?;
    Some(kb / 1024)
}

/// THE MEMORY A CHAIN MAY LEAVE THE PROGRAM HOLDING, in megabytes. A sketch of a sitting holds a few hundred; the runner
/// stops the whole probe at 2048 with no words of the chain's, so the chain stops itself first - and can then be
/// shrunk like any other failure.
const MEMORY_MB: u64 = 1536;

thread_local! {
    /// Whether an undo that would take the scene itself away is skipped - see [`play_grounded`].
    static GROUNDED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// PLAY THE STEPS STANDING ON THE SCENE: as [`play`], but an undo that would take the scene itself away - the sketch the
/// chain draws in - is skipped as a step aimed past it. For what measures the chains themselves (how much of a chain a
/// program can make), not for the chains that look for faults: taking the sketch one stands in away is a fault's place.
///
/// # Errors
/// As [`play`].
pub fn play_grounded(s: &mut Session, steps: &[Step]) -> Result<Played, Stopped> {
    GROUNDED.with(|g| g.set(true));
    let result = play(s, steps);
    GROUNDED.with(|g| g.set(false));
    result
}

/// PLAY `steps` in `s`, the oracles of the process looking at every one.
///
/// # Errors
/// The step after which the program was found not right - by an oracle, or by a panic of its own.
pub fn play(s: &mut Session, steps: &[Step]) -> Result<Played, Stopped> {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;
    // A WATCH ON THE MEMORY WHILE A STEP RUNS: a step that runs the memory away does not come back to be looked at,
    // and the runner outside stops the whole probe with no words of the chain's. The watch names the step and ends
    // the process first, so the words that come back say which step it was.
    let (now, over) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicBool::new(false)));
    let watch = {
        let (now, over, steps) = (now.clone(), over.clone(), steps.to_vec());
        std::thread::spawn(move || {
            while !over.load(Ordering::Relaxed) {
                if let Some(held) = resident_mb().filter(|mb| *mb > MEMORY_MB) {
                    let at = now.load(Ordering::Relaxed);
                    eprintln!("the program's memory ran away past {MEMORY_MB} MB ({held} MB) during step {at}: {:?}", steps.get(at));
                    std::process::exit(101);
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        })
    };
    let result = play_watched(s, steps, &now);
    over.store(true, Ordering::Relaxed);
    let _ = watch.join();
    result
}

/// Play the steps, telling the watch of the memory which one runs.
fn play_watched(s: &mut Session, steps: &[Step], now: &std::sync::atomic::AtomicUsize) -> Result<Played, Stopped> {
    let mut played = Played { done: 0, skipped: Vec::new() };
    let ground = GROUNDED.with(|g| g.get()).then(|| s.document().undo.len());
    for (at, step) in steps.iter().enumerate() {
        now.store(at, std::sync::atomic::Ordering::Relaxed);
        if matches!(step, Step::Undo) && ground.is_some_and(|g| s.document().undo.len() <= g) {
            played.skipped.push(at);
            continue;
        }
        match failure(|| perform(s, step)) {
            None => played.done += 1,
            Some((said, _)) if said.contains("the program is not right") => return Err(Stopped { at, said }),
            // the door raises it, but it is the program that went on rebuilding or drawing without end
            Some((said, _)) if said.contains("did not come to rest") => return Err(Stopped { at, said }),
            Some((_, file)) if is_aiming(&file) => played.skipped.push(at),
            Some((said, file)) => return Err(Stopped { at, said: format!("the program panicked in {file}: {said}") }),
        }
        if let Some(held) = resident_mb().filter(|mb| *mb > MEMORY_MB) {
            return Err(Stopped { at, said: format!("the program's memory ran away: {held} MB after this step, over {MEMORY_MB}") });
        }
    }
    Ok(played)
}

/// PLAY `steps` IN A FRESH WINDOW OF `scene` AND HOLD THEM TO EVERY ORACLE: the ones of the process at every step, and
/// when the chain is played through, the three round trips a document must survive - undo and redo, save and open,
/// rebuilding all.
///
/// # Errors
/// What went wrong, in words.
fn hold(scene: Scene, steps: &[Step]) -> Result<Played, String> {
    let mut s = scene.start();
    // A STEP OF A PERSON COMES TO REST IN WELL UNDER A SECOND; one that has not in 20 has run away. The door's own
    // two minutes, spent again on every play of a shrinking, would make a chain that hangs unshrinkable.
    s.budget(std::time::Duration::from_secs(REST_SECONDS));
    let played = play(&mut s, steps).map_err(|stop| format!("stopped at step {} ({:?}):\n{}", stop.at, steps[stop.at], stop.said))?;
    // THE ROUND TRIPS, each named: they look at a program that may still not come to rest, and which of them it did
    // not come to rest in is the first thing to know about such a chain
    let path = format!("{}/qymcad-chain-{}-round.qcad", std::env::temp_dir().display(), std::process::id());
    let rounds: [Round; 3] = [
        Round { name: "undo and redo", run: &|s| crate::oracles::undo_redo(s) },
        Round { name: "save and open", run: &|s| crate::oracles::save_open(s, &path) },
        Round { name: "rebuilding everything", run: &|s| crate::oracles::rebuild_everything(s) },
    ];
    for Round { name, run: round } in rounds {
        let mut went = Ok(());
        if let Some((said, _)) = failure(|| went = round(&mut s)) {
            went = Err(said);
        }
        went.map_err(|e| format!("was played through, and then, in {name}: {e}"))?;
    }
    Ok(played)
}

/// A round trip a played chain is put through, by its name.
struct Round<'a> {
    name: &'static str,
    run: &'a dyn Fn(&mut Session) -> Result<(), String>,
}

/// How long a step may take to come to rest in a chain, in seconds.
const REST_SECONDS: u64 = 20;

/// How long a failing chain is shrunk for, in seconds, before the shortest one found so far is given: a play of 150
/// steps apart takes some 20 seconds, one that hangs 20 more for the step it hangs in, and the probe of a slot has
/// 1800 seconds in all. Only a chain that failed pays it - and a chain that failed is a finding.
const SHRINK_SECONDS: u64 = 1200;

/// GROW THE CHAIN OF `seed` AND HOLD IT TO EVERY ORACLE.
///
/// # Errors
/// Words that begin with the seed, so the chain can be grown again; then the chain shrunk to the fewest steps that
/// still fail the same way, and that chain written out as a probe ready to be put into the set for good.
pub fn check(seed: u64, len: usize, scene: Scene) -> Result<(), String> {
    let steps = grow(seed, len, scene);
    let all: Vec<usize> = (0..steps.len()).collect();
    hold_apart(seed, len, scene, &all).map_err(|failed| {
        let kind = kind_of(&failed);
        let short = shrink_within(&all, |fewer| hold_apart(seed, len, scene, fewer).err().is_some_and(|e| kind_of(&e) == kind), std::time::Duration::from_secs(SHRINK_SECONDS));
        let kept: Vec<Step> = short.iter().map(|&i| steps[i].clone()).collect();
        format!(
            "the chain of the seed {seed} ({len} steps, {scene:?}) {failed}\n\nshrunk to {} steps that fail the same way; kept for good, it is this probe:\n\n{}",
            kept.len(),
            as_probe(seed, scene, &kept)
        )
    })
}

/// THE VARIABLE a chain is handed to a process of its own by: the seed, the length, the scene and the numbers of the
/// steps kept - the child grows the same chain from the same seed and plays the steps named.
pub const CHAIN_VAR: &str = "QYMCAD_CHAIN";

/// The probe of the set that plays a chain given by [`CHAIN_VAR`], by its path in the test binary.
const CHAIN_PROBE: &str = "chains::a_chain_given_by_the_environment";

/// How long one play of a chain apart may take, in seconds: 150 steps come to about 20, a chain that hangs to 20 more
/// for every step it hangs in.
const APART_SECONDS: u64 = 240;

/// PLAY THE STEPS `keep` OF THE CHAIN OF `seed` IN A PROCESS OF ITS OWN, held to every oracle; answer what went wrong.
/// A chain that runs the memory away, never comes to rest or takes the process down comes back as words like any
/// other failure - and so can be shrunk like any other.
fn hold_apart(seed: u64, len: usize, scene: Scene, keep: &[usize]) -> Result<(), String> {
    let given = format!("{seed} {len} {scene:?} {}", keep.iter().map(usize::to_string).collect::<Vec<_>>().join(","));
    crate::isolation::apart(CHAIN_PROBE, std::time::Duration::from_secs(APART_SECONDS), 2048, &[(CHAIN_VAR, given)])
}

/// PLAY THE CHAIN GIVEN BY [`CHAIN_VAR`], if one is: the probe [`CHAIN_PROBE`] calls this, and with nothing given it
/// has nothing to play.
///
/// # Panics
/// With what went wrong in the chain, and on a variable that does not read as a chain.
pub fn play_from_env() {
    let Ok(given) = std::env::var(CHAIN_VAR) else { return };
    let parts: Vec<&str> = given.split(' ').collect();
    let [seed, len, scene, keep] = parts.as_slice() else { panic!("{CHAIN_VAR} does not read as a chain: {given:?}") };
    let (seed, len): (u64, usize) = (seed.parse().expect("the seed of the chain"), len.parse().expect("the length of the chain"));
    let scene = match *scene {
        "Sketch" => Scene::Sketch,
        other => panic!("{CHAIN_VAR} names no scene {other:?}"),
    };
    let steps = grow(seed, len, scene);
    let kept: Vec<Step> = keep.split(',').filter(|s| !s.is_empty()).map(|i| steps[i.parse::<usize>().expect("a number of a step")].clone()).collect();
    eprintln!("playing {} steps of the chain of the seed {seed} ({len} steps, {scene:?})", kept.len());
    if let Err(failed) = hold(scene, &kept) {
        panic!("{failed}");
    }
}

/// PLAY A CHAIN KEPT IN THE SET for good, held to every oracle.
///
/// # Panics
/// With what went wrong, when it does.
pub fn replay(scene: Scene, steps: &[Step]) {
    if let Err(failed) = hold(scene, steps) {
        panic!("the kept chain of {} steps ({scene:?}) {failed}", steps.len());
    }
}

/// WHAT WENT WRONG, without what only this chain says - the step it happened at, the gesture, the path of a file - so
/// a shorter chain failing for the same reason is known as the same failure.
fn kind_of(failed: &str) -> String {
    if failed.contains("did not come to rest") {
        return "did not come to rest".to_string();
    }
    if failed.contains("memory ran away") {
        return "memory ran away".to_string();
    }
    if let Some((_, problems)) = failed.split_once("the program is not right:") {
        return problems.trim().to_string();
    }
    if let Some((_, panic)) = failed.split_once("the program panicked in") {
        return format!("panicked in{}", panic.lines().next().unwrap_or_default());
    }
    for round in ["undo and redo", "saved as", "rebuilt from the start"] {
        if failed.contains(round) {
            return round.to_string();
        }
    }
    failed.lines().next().unwrap_or_default().to_string()
}

/// SHRINK A FAILING CHAIN to the fewest steps `fails` still holds for: halves, quarters and so down to single steps
/// are left out while the chain goes on failing. `steps` must fail to begin with.
pub fn shrink_by<T: Clone>(steps: &[T], fails: impl Fn(&[T]) -> bool) -> Vec<T> {
    shrink_within(steps, fails, std::time::Duration::MAX)
}

/// SHRINK AS [`shrink_by`] DOES, but no longer than `within`: past it the shortest chain found so far is given.
pub fn shrink_within<T: Clone>(steps: &[T], fails: impl Fn(&[T]) -> bool, within: std::time::Duration) -> Vec<T> {
    let began = std::time::Instant::now();
    let mut kept: Vec<T> = steps.to_vec();
    let mut chunk = kept.len().div_ceil(2).max(1);
    loop {
        let mut cut = false;
        let mut from = 0;
        while from < kept.len() {
            if began.elapsed() > within {
                return kept;
            }
            let to = (from + chunk).min(kept.len());
            let fewer: Vec<T> = kept[..from].iter().chain(&kept[to..]).cloned().collect();
            if !fewer.is_empty() && fails(&fewer) {
                kept = fewer;
                cut = true; // the same place is tried again: what slid into it may go as well
            } else {
                from = to;
            }
        }
        if chunk == 1 && !cut {
            return kept;
        }
        if !cut {
            chunk = chunk.div_ceil(2).max(1);
        }
    }
}

/// A STEP WRITTEN AS RUST, the way a probe spells it.
fn as_rust(step: &Step) -> String {
    match step {
        Step::Take { tool, entry } => format!("Step::Take {{ tool: {tool:?}, entry: {entry} }}"),
        Step::Click { tool, at } => format!("Step::Click {{ tool: {tool:?}, at: {at} }}"),
        Step::Finish { tool } => format!("Step::Finish {{ tool: {tool:?} }}"),
        Step::Value { tool, field, text } => format!("Step::Value {{ tool: {tool:?}, field: {field}, text: {text:?}.into() }}"),
        Step::Words { tool, text } => format!("Step::Words {{ tool: {tool:?}, text: {text:?}.into() }}"),
        Step::Enter => "Step::Enter".into(),
        Step::Escape => "Step::Escape".into(),
        Step::Miss => "Step::Miss".into(),
        Step::Undo => "Step::Undo".into(),
        Step::Redo => "Step::Redo".into(),
        Step::SaveAndOpen => "Step::SaveAndOpen".into(),
    }
}

/// A SHRUNK CHAIN AS A PROBE OF ITS OWN, ready to be put into `tests/acceptance/chains.rs` as it is - it brings its
/// own `use` - and it plays the same steps every run, whatever the generator grows later.
pub fn as_probe(seed: u64, scene: Scene, steps: &[Step]) -> String {
    let lines: Vec<String> = steps.iter().map(|s| format!("            {},", as_rust(s))).collect();
    format!(
        "probe! {{\n    /// THE CHAIN FOUND BY THE SEED {seed}, shrunk to the steps that matter.\n    fn the_chain_of_the_seed_{seed}() {{\n        use qymcad_acceptance::chains::{{replay, Scene, Step}};\n        replay(Scene::{scene:?}, &[\n{}\n        ]);\n    }}\n}}",
        lines.join("\n")
    )
}

/// The file a chain saves the document into, one per process.
fn scratch() -> String {
    format!("{}/qymcad-chain-{}.qcad", std::env::temp_dir().display(), std::process::id())
}

/// MAKE ONE STEP in `s`, as a person makes it.
fn perform(s: &mut Session, step: &Step) {
    match step {
        Step::Take { tool: id, entry } => run::enter_for(s, tool(id), &tool(id).entries[*entry]),
        Step::Click { tool: id, at } => {
            let t = tool(id);
            match t.flow {
                Flow::Command => run::click(s, &t.picks[*at]),
                Flow::Drawing(places, _) => {
                    let (x, y) = places[*at];
                    s.click_on_sketch(x, y);
                }
                Flow::Action => panic!("{id}: an action takes no click of its own"),
            }
        }
        Step::Finish { tool: id } => match tool(id).flow {
            Flow::Command => {
                s.key(Key::Enter);
            }
            Flow::Drawing(places, last) => {
                if let Some((x, y)) = places.last() {
                    run::finish_click(s, *x, *y, last);
                }
            }
            Flow::Action => {}
        },
        Step::Value { tool: id, field, text } => run::type_into_field(s, &tool(id).fields[*field], text),
        Step::Words { tool: id, text } => {
            if let Some(w) = tool(id).words.first() {
                run::type_words(s, w, text);
            }
        }
        Step::Enter => {
            s.key(Key::Enter);
        }
        Step::Escape => {
            s.key(Key::Escape);
        }
        Step::Miss => {
            let at = run::empty_space(s);
            s.click(at);
        }
        Step::Undo => {
            s.chord(Modifiers::COMMAND, Key::Z);
        }
        Step::Redo => {
            s.chord(Modifiers::COMMAND, Key::Y);
        }
        Step::SaveAndOpen => {
            let path = scratch();
            build::save_as(s, &path);
            build::open_project(s, &path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{grow, Scene, Step};

    /// The kind of a step, for counting them.
    fn kind(s: &Step) -> &'static str {
        match s {
            Step::Take { .. } => "take",
            Step::Click { .. } => "click",
            Step::Finish { .. } => "finish",
            Step::Value { .. } => "value",
            Step::Words { .. } => "words",
            Step::Enter => "enter",
            Step::Escape => "escape",
            Step::Miss => "miss",
            Step::Undo => "undo",
            Step::Redo => "redo",
            Step::SaveAndOpen => "save and open",
        }
    }

    /// THE SAME SEED GROWS THE SAME CHAIN, and another seed another one.
    #[test]
    fn a_miss_of_aim_is_told_by_the_session_and_its_modules() {
        for aiming in ["crates/qymcad/src/gui/session.rs", "crates/qymcad/src/gui/session/finding.rs", "crates/qymcad-acceptance/src/contract/run/steps.rs"] {
            assert!(super::is_aiming(aiming), "a failure raised in {aiming} is a miss of aim");
        }
        for program in ["crates/qymcad/src/gui/sketching.rs", "crates/qymcad-ui-state/src/session_tools.rs"] {
            assert!(!super::is_aiming(program), "a failure raised in {program} is the program's");
        }
    }

    #[test]
    fn a_seed_grows_the_same_chain() {
        assert_eq!(grow(7, 60, Scene::Sketch), grow(7, 60, Scene::Sketch), "two chains of the seed 7 came out different");
        assert_ne!(grow(7, 60, Scene::Sketch), grow(8, 60, Scene::Sketch), "the seeds 7 and 8 grew the same chain");
        assert_eq!(grow(7, 60, Scene::Sketch).len(), 60, "a chain of 60 steps came out of another length");
    }

    /// EVERY KIND OF STEP IS GROWN over a few seeds - the slips as well as the work - and so is every tool of the
    /// scene.
    #[test]
    fn every_kind_of_step_and_every_tool_is_grown() {
        let chains: Vec<Vec<Step>> = (0..40).map(|seed| grow(seed, 60, Scene::Sketch)).collect();
        let kinds: std::collections::BTreeSet<&str> = chains.iter().flatten().map(kind).collect();
        for want in ["take", "click", "finish", "value", "words", "enter", "escape", "miss", "undo", "redo", "save and open"] {
            assert!(kinds.contains(want), "no chain of forty seeds holds a step of the kind {want:?}: {kinds:?}");
        }
        let taken: std::collections::BTreeSet<&str> = chains.iter().flatten().filter_map(|s| if let Step::Take { tool, .. } = s { Some(*tool) } else { None }).collect();
        for t in Scene::Sketch.tools() {
            assert!(taken.contains(t.id), "no chain of forty seeds takes the tool {:?}", t.id);
        }
    }

    /// A CHAIN TAKES ONLY THE TOOLS OF ITS SCENE, and a step of a tool comes only after that tool was taken.
    #[test]
    fn a_step_of_a_tool_comes_after_the_tool_is_taken() {
        let scene: Vec<&str> = Scene::Sketch.tools().iter().map(|t| t.id).collect();
        for seed in 0..40 {
            let chain = grow(seed, 80, Scene::Sketch);
            let mut taken: Vec<&str> = Vec::new();
            for step in &chain {
                match step {
                    Step::Take { tool, .. } => {
                        assert!(scene.contains(tool), "seed {seed}: the tool {tool:?} is not of the sketch");
                        taken.push(tool);
                    }
                    Step::Click { tool, .. } | Step::Finish { tool } | Step::Value { tool, .. } | Step::Words { tool, .. } => {
                        assert!(taken.contains(tool), "seed {seed}: a step of {tool:?} before the tool was ever taken");
                    }
                    _ => {}
                }
            }
        }
    }

    /// A FAILING CHAIN SHRINKS TO THE STEPS THE FAILURE NEEDS, in their order: here, a polygon taken and then clicked.
    #[test]
    fn a_failing_chain_shrinks_to_what_the_failure_needs() {
        let needs = |steps: &[Step]| {
            let taken = steps.iter().position(|s| matches!(s, Step::Take { tool: "sketch.polygon", .. }));
            taken.is_some_and(|at| steps[at..].iter().any(|s| matches!(s, Step::Click { tool: "sketch.polygon", .. })))
        };
        let mut shrunk_any = false;
        for seed in 0..40 {
            let chain = grow(seed, 60, Scene::Sketch);
            if !needs(&chain) {
                continue;
            }
            let short = super::shrink_by(&chain, needs);
            assert!(needs(&short), "seed {seed}: the shrunk chain no longer fails: {short:?}");
            assert!(short.len() == 2, "seed {seed}: a failure that needs two steps shrank to {} of {}: {short:?}", short.len(), chain.len());
            shrunk_any = true;
        }
        assert!(shrunk_any, "no chain of forty seeds took a polygon and clicked it: nothing was shrunk");
    }

    /// A SHRUNK CHAIN IS WRITTEN AS A PROBE that names its seed and spells every step as Rust.
    #[test]
    fn a_shrunk_chain_is_written_as_a_probe() {
        let steps = vec![Step::Take { tool: "sketch.polygon", entry: 1 }, Step::Value { tool: "sketch.polygon", field: 0, text: "1\"2".into() }, Step::Undo];
        let probe = super::as_probe(7, Scene::Sketch, &steps);
        for want in [
            "probe! {",
            "fn the_chain_of_the_seed_7()",
            "use qymcad_acceptance::chains::{replay, Scene, Step};",
            "replay(Scene::Sketch, &[",
            "Step::Take { tool: \"sketch.polygon\", entry: 1 },",
            "Step::Value { tool: \"sketch.polygon\", field: 0, text: \"1\\\"2\".into() },",
            "Step::Undo,",
        ] {
            assert!(probe.contains(want), "the probe does not hold {want:?}:\n{probe}");
        }
    }

    /// EVERY LEVEL FITS ITS BUDGET: what the chains of a slot cost by the measured price is within what the level may
    /// spend on them, and the cheapest level plays none.
    #[test]
    fn every_level_fits_its_budget() {
        for tier in super::Tier::ALL {
            let (count, len) = tier.chains();
            let costs = count as f64 * (super::CHAIN_SECONDS + len as f64 * super::STEP_SECONDS);
            assert!(costs <= tier.slot_budget(), "{tier:?}: the chains of a slot cost {costs} s, over the {} s the level gives them", tier.slot_budget());
        }
        assert!(super::Tier::Fast.chains().0 == 0, "the level of every edit plays chains");
        assert!(super::Tier::Release.chains().0 * super::Tier::Release.chains().1 > super::Tier::Full.chains().0 * super::Tier::Full.chains().1, "the release plays no more steps than the commit");
    }

    /// THE SLOTS SHARE OUT THE SEEDS: no seed is played twice at a level, and every level beyond the cheapest plays
    /// the seeds from 1 up without a gap.
    #[test]
    fn the_slots_share_out_the_seeds() {
        for tier in super::Tier::ALL {
            let mut all: Vec<u64> = (0..super::SLOTS).flat_map(|slot| super::seeds(tier, slot)).map(|(seed, _)| seed).collect();
            all.sort_unstable();
            let want: Vec<u64> = (1..=all.len() as u64).collect();
            assert!(all == want, "{tier:?}: the slots play the seeds {all:?}");
        }
    }

    /// SOME OF THE VALUES TYPED ARE ONES THE FIELD MUST REFUSE, and some it takes.
    #[test]
    fn values_to_take_and_values_to_refuse_are_both_typed() {
        let values: Vec<String> = (0..40).flat_map(|seed| grow(seed, 80, Scene::Sketch)).filter_map(|s| if let Step::Value { text, .. } = s { Some(text) } else { None }).collect();
        let refused = values.iter().filter(|t| t.is_empty() || t.parse::<f64>().is_err() || t.starts_with('-')).count();
        assert!(refused > 0 && refused < values.len(), "of {} values typed, {refused} are of the kind to refuse", values.len());
    }
}
