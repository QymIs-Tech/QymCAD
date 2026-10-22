//! THE OUTCOMES OF BLENDS KEPT: a fillet or a chamfer asked again of the same body - by its state, not its id - with the
//! same edges, sizes and names is answered with what it came to the first time, with no work of OCCT.
//!
//! Two askers need it. A trial build answers a value on a copy of the part, and Enter rebuilt the same node from the same
//! body again: a fillet of 66 edges whose edges would not take together is 8.3 s of edges tried one by one. And a rebuild
//! of the whole document asks every blend again of bodies that did not change.
//!
//! The key holds the state of the body read (`Shape::state` - its faces, edges and vertices by their names and by points
//! laid on them), so a body that changed by a hair is asked afresh. The outcome is kept whole: the body, or the refusal;
//! the edges left out; whether it came a hair short; the roles of the faces it makes. The body is kept as a copy of its
//! own and handed out as another: a kept body shares nothing with a body of a document.

use crate::door::Roles;
use qymcad_core::errors::{CoreError, Op};
use qymcad_core::feature::DroppedEdges;
use qymcad_core::geom::Built;
use qymcad_core::model::Id;

/// HOW MUCH IS KEPT, in the bytes of the bodies as the kernel writes them: a copy of a body held is some 2.5 times its
/// bytes in memory (measured on a propeller: 64 bodies of 0.1-0.5 MB, 74 MB more in the process). 8 MB of bytes is
/// some 20 MB of memory, and holds the blends of the propeller (seven a rebuild, 2.5 MB together) three rebuilds over. A
/// body heavier than the whole budget is not kept. Refusals weigh nothing; `HELD` bounds their number.
const HELD_BYTES: usize = 8 << 20;
const HELD: usize = 256;

/// WHAT A BLEND CAME TO.
struct Outcome {
    made: Made,
    dropped: Option<DroppedEdges>,
    short: Short,
    roles: Roles,
}

/// THE BODY A BLEND MADE, or why it made none.
enum Made {
    Body(crate::Shape),
    Refused(CoreError),
}

/// DID THE BODY COME AT THE SIZE ASKED, or a hair short of it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Short {
    AsAsked,
    SteppedBack,
}

thread_local! {
    /// How many blends of this thread were answered with an outcome kept: what a check counts.
    static TAKEN: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many blends this thread has had answered with an outcome kept.
#[cfg(test)]
pub(crate) fn taken() -> usize {
    TAKEN.with(|t| t.get())
}

/// AN OUTCOME KEPT under the key of what was asked, and what it weighs: the bytes of its body, none for a refusal.
struct Kept {
    key: u64,
    outcome: Outcome,
    bytes: usize,
}

/// KEEP `new` AMONG `kept`, the newest last, within `budget` bytes of bodies and `HELD` outcomes: the oldest go first. An
/// outcome heavier than the whole budget is not kept.
fn keep(kept: &mut Vec<Kept>, new: Kept, budget: usize) {
    kept.retain(|k| k.key != new.key);
    if new.bytes > budget {
        return;
    }
    kept.push(new);
    while kept.len() > HELD || kept.iter().map(|k| k.bytes).sum::<usize>() > budget {
        kept.remove(0);
    }
}

/// The outcomes kept, the newest last.
static KEPT: std::sync::Mutex<Vec<Kept>> = std::sync::Mutex::new(Vec::new());

/// EVERYTHING A BLEND WAS ASKED WITH besides the body it reads: what it is, the body it makes, its sizes, its edges, the
/// names of its faces and what else shapes it (the radii at the vertices, the form of a chamfer).
pub(crate) struct Asked<'a> {
    pub kind: Op,
    pub body: Id,
    pub src: Id,
    pub sizes: &'a [f64],
    pub edges: &'a [u32],
    pub roles: &'a qymcad_core::feature::BlendRoles,
    pub more: String,
}

impl Asked<'_> {
    /// Everything it was asked with, into `h`: the sizes by their bits, the names of the faces as their roles say them.
    fn hash_into(&self, h: &mut impl std::hash::Hasher) {
        use std::hash::Hash;
        format!("{:?}", self.kind).hash(h);
        self.body.hash(h);
        self.src.hash(h);
        self.sizes.iter().for_each(|v| v.to_bits().hash(h));
        self.edges.hash(h);
        format!("{:?}", self.roles).hash(h);
        self.more.hash(h);
    }
}

impl crate::kernel::OcctKernel {
    /// THE BLEND AS ASKED, as it came the last time it was asked of a body in the same state - or built by `build` and
    /// kept. A body not built yet is not asked about.
    pub(crate) fn blend_kept(&self, asked: Asked, build: impl FnOnce() -> Result<Built, CoreError>) -> Result<Built, CoreError> {
        use std::hash::{Hash, Hasher};
        let state = self.shapes.borrow().get(&asked.src).map(|s| s.state());
        let Some(state) = state.filter(|s| *s != 0) else { return build() };
        let mut h = std::collections::hash_map::DefaultHasher::new();
        state.hash(&mut h);
        asked.hash_into(&mut h);
        self.remembered(h.finish(), asked.kind, asked.body, build)
    }

    /// THE BLEND `kind` OF THE BODY `body`, as it came the last time it was asked with `key` - landed through the door as
    /// any result is - or built by `build` and kept. A refusal a stop asked for, or one the body not being built yet
    /// gives, is not an outcome and is not kept.
    fn remembered(&self, key: u64, kind: Op, body: Id, build: impl FnOnce() -> Result<Built, CoreError>) -> Result<Built, CoreError> {
        let found = KEPT.lock().ok().and_then(|kept| {
            kept.iter().rev().find(|k| k.key == key).map(|Kept { outcome: o, .. }| {
                let made = match &o.made {
                    Made::Body(s) => s.own_copy().map(Made::Body),
                    Made::Refused(e) => Some(Made::Refused(e.clone())),
                };
                made.map(|made| Outcome { made, dropped: o.dropped, short: o.short, roles: o.roles.clone() })
            })
        });
        if let Some(Some(o)) = found {
            TAKEN.with(|t| t.set(t.get() + 1));
            return self.land_kept(kind, body, o);
        }
        let out = build();
        let stopped = self.stop.load(std::sync::atomic::Ordering::Relaxed);
        let made = match &out {
            Ok(_) => self.shapes.borrow().get(&body).and_then(|s| s.own_copy()).map(Made::Body),
            Err(e) if !stopped && !e.retryable() && *e != CoreError::EmptyResult => Some(Made::Refused(e.clone())),
            Err(_) => None,
        };
        if let (Some(made), false) = (made, stopped) {
            let short = if self.stepped_back.borrow().contains(&body) { Short::SteppedBack } else { Short::AsAsked };
            // the weight by the bytes the kernel writes the body as: 1-9 ms a body on the propeller
            let bytes = match &made {
                Made::Body(s) => s.to_brep_bytes().map_or(usize::MAX, |b| b.len()),
                Made::Refused(_) => 0,
            };
            let o = Outcome { made, dropped: self.dropped.borrow().get(&body).copied(), short, roles: self.roles.borrow().get(&body).cloned().unwrap_or_default() };
            if let Ok(mut kept) = KEPT.lock() {
                keep(&mut kept, Kept { key, outcome: o, bytes }, HELD_BYTES);
            }
        }
        out
    }

    /// Land an outcome kept: its body through the door, its edges left out and its step back said as they were.
    fn land_kept(&self, kind: Op, body: Id, o: Outcome) -> Result<Built, CoreError> {
        let Outcome { made, dropped, short, roles } = o;
        self.dropped.borrow_mut().remove(&body);
        let shape = match made {
            Made::Body(s) => s,
            Made::Refused(e) => return Err(e),
        };
        let held = std::cell::Cell::new(Some(shape));
        let work = |_: &crate::door::Bodies, _: f64| held.take().ok_or(CoreError::OpFailed(kind));
        let done = self.run(crate::door::Operation { kind, body, sizing: crate::door::Sizing::Unsized, roles, work, limit: None })?;
        if let Some(d) = dropped {
            self.dropped.borrow_mut().insert(body, d);
        }
        if short == Short::SteppedBack {
            self.stepped_back.borrow_mut().insert(body);
        }
        Ok(done)
    }
}

#[cfg(test)]
mod tests {
    use qymcad_core::feature::{BlendRoles, Kernel};

    /// A kernel holding a block 40 x 30 x `high` as body 1.
    fn a_block(high: f64) -> crate::OcctKernel {
        let k = crate::OcctKernel::default();
        let block = crate::Shape::extrude(&[0.0, 0.0, 40.0, 0.0, 40.0, 30.0, 0.0, 30.0], high).expect("the block");
        k.shapes.borrow_mut().insert(1, block);
        k
    }

    /// The long top front edge of body 1 of `k`, `high` up.
    fn top_front(k: &crate::OcctKernel, high: f64) -> u32 {
        let shapes = k.shapes.borrow();
        let e = shapes[&1].edges_info().into_iter().find(|e| e.circle.is_none() && e.poly.iter().all(|p| p[1].abs() < 1e-6 && (f64::from(p[2]) - high).abs() < 1e-5));
        e.expect("setup: the block has no top front edge").id
    }

    /// An outcome of `bytes` under `key`: a refusal, which weighs nothing by itself, given a weight for the store.
    fn weighing(key: u64, bytes: usize) -> super::Kept {
        let outcome = super::Outcome {
            made: super::Made::Refused(qymcad_core::errors::CoreError::OpFailed(qymcad_core::errors::Op::Fillet)),
            dropped: None,
            short: super::Short::AsAsked,
            roles: Default::default(),
        };
        super::Kept { key, outcome, bytes }
    }

    /// THE STORE IS BOUNDED BY THE BYTES OF ITS BODIES, not by their number: the oldest go first once the budget is
    /// passed, and a body heavier than the whole budget is not kept. Measured with the store bounded by number alone: 64
    /// bodies of a propeller, 74 MB more in the process after ten edits.
    #[test]
    fn the_store_holds_bodies_within_its_bytes() {
        let mut kept = Vec::new();
        for (key, bytes) in [(1, 10), (2, 10), (3, 10)] {
            super::keep(&mut kept, weighing(key, bytes), 25);
        }
        assert_eq!(kept.iter().map(|k| k.key).collect::<Vec<_>>(), [2, 3], "the oldest body is not let go of over the budget");
        super::keep(&mut kept, weighing(4, 30), 25);
        assert_eq!(kept.iter().map(|k| k.key).collect::<Vec<_>>(), [2, 3], "a body heavier than the budget is kept, or threw the others out");
        super::keep(&mut kept, weighing(2, 10), 25);
        assert_eq!(kept.iter().map(|k| k.key).collect::<Vec<_>>(), [3, 2], "an outcome kept again does not stand twice");
    }

    /// A FILLET ASKED AGAIN OF A BODY IN THE SAME STATE IS THE ONE IT CAME TO: no work of the kernel, the same body.
    /// Reported behaviour: Enter rebuilt a fillet its trial had just built, 8.3 s for one of 66 edges.
    #[test]
    fn a_fillet_asked_again_is_taken_as_it_came() {
        let k = a_block(10.0);
        let e = top_front(&k, 10.0);
        let first = k.fillet(2, 1, 3.0, &[e], BlendRoles::Unnamed).expect("R3 rounds an edge of a block");
        let was = super::taken();
        // another kernel, the source carried through its bytes: as a trial hands it to a rebuild
        let again = a_block(10.0);
        let second = again.fillet(2, 1, 3.0, &[e], BlendRoles::Unnamed).expect("R3 again");
        assert_eq!(super::taken(), was + 1, "the fillet asked again of the same block was built again");
        assert_eq!(first.faces.len(), second.faces.len(), "the fillet taken is not the one built");
        let v = again.shapes.borrow()[&2].volume();
        let want = 12000.0 - (9.0 - std::f64::consts::PI * 9.0 / 4.0) * 40.0;
        assert!((v - want).abs() < 1e-6 * want, "the fillet taken holds {v}, not {want}");
    }

    /// A SOURCE THAT CHANGED IS ASKED AFRESH: the same edge by its name, the same radius, a block 2 higher - the outcome
    /// kept for the lower block is not taken.
    #[test]
    fn a_fillet_of_a_changed_source_is_built_again() {
        let low = a_block(10.0);
        let e = top_front(&low, 10.0);
        low.fillet(2, 1, 3.0, &[e], BlendRoles::Unnamed).expect("R3 on the block 10 high");
        let was = super::taken();
        let high = a_block(12.0);
        assert_eq!(top_front(&high, 12.0), e, "setup: the edge of the higher block has another name, and the key would differ by it alone");
        high.fillet(2, 1, 3.0, &[e], BlendRoles::Unnamed).expect("R3 on the block 12 high");
        assert_eq!(super::taken(), was, "the fillet of the block 12 high was taken from the one 10 high");
        let v = high.shapes.borrow()[&2].volume();
        let want = 14400.0 - (9.0 - std::f64::consts::PI * 9.0 / 4.0) * 40.0;
        assert!((v - want).abs() < 1e-6 * want, "the fillet of the block 12 high holds {v}, not {want}");
    }
}
