//! THE DOOR OF THE BRIDGE: every operation that builds a body goes through it, and what OCCT is known to get wrong is
//! handled here, once, for all of them - the step back of a size at the boundary of degeneracy, the cleaning and the
//! check of the result, the landing in the cache.
//!
//! An operation brings what is its own and nothing else: what it is (`Op`), where its result goes, whether it has a
//! size and how that steps back, and the work - the call of the OCCT algorithm at a factor on its size. Everything a
//! new operation would otherwise have to remember is done by the door.

mod land;
mod size;

pub use size::{sized, Retreat};

use qymcad_core::errors::{CoreError, Op};
use qymcad_core::geom::Built;
use qymcad_core::model::Id;

/// WHETHER AN OPERATION HAS A SIZE that is stepped back on a refusal, and how far.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sizing {
    /// No size the limiting case lies in: a refusal is the answer.
    Unsized,
    /// A size, stepped back by the steps of its kind.
    Sized(StepBack),
}

/// HOW AN OPERATION STEPS BACK: by the steps of its kind, from its largest length.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StepBack {
    pub retreat: Retreat,
    /// The largest length the work scales: a radius, a leg.
    pub size: f64,
}

impl StepBack {
    /// THE STRIPS A STEP BACK LEAVES. The asked size eats a face whole - a chamfer of 10 x 30 across the top of a step 30
    /// deep and 10 high - and is refused; a hair smaller it builds and leaves that face standing as a strip as wide as
    /// the step: 3e-4 mm at 1 - 1e-5 of 30. Such a strip is no face: nameless after no history, renamed by every
    /// rebuild, it lit a preview of the next tool all over the part, and a rounding beside it took no radius. Taken out,
    /// its neighbours meet where the asked size put them. The width allows four steps; a crack is never under 1e-5.
    pub fn strip(self, factor: f64) -> f64 {
        (4.0 * self.size.abs() * (1.0 - factor)).max(1e-5)
    }
}

/// THE NAMES AN OPERATION GIVES THE FACES IT MAKES, as values: the rule of the operation says what a face is (the
/// surface of a fillet after its edge, the patch of a corner after its vertex), the C++ side gets a number awaiting
/// the name (`names::ROLE`), and the table of the document gives the descriptor when the result lands - on the thread
/// of the document, in the order of its tree.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Roles {
    pub given: Vec<RoleName>,
}

/// A ROLE AND THE NUMBER IT WAS HANDED UNDER.
#[derive(Clone, Debug, PartialEq)]
pub struct RoleName {
    pub number: u32,
    pub recipe: RoleRecipe,
}

/// WHAT A ROLE IS CALLED: a name known when the operation is laid, or a name taken after other roles once they are
/// named - the cap of one region of several is named after the lowest name among the walls of its region, and that is
/// a descriptor the table gives only when the result lands.
#[derive(Clone, Debug, PartialEq)]
pub enum RoleRecipe {
    Named(qymcad_core::names::GeoName),
    AfterLowest(AfterLowest),
}

/// A NAME AFTER THE LOWEST OF THE NAMES OTHER ROLES TAKE: `role` of `feature`, its source the lowest descriptor the
/// roles numbered `of` are given (0 when there are none).
#[derive(Clone, Debug, PartialEq)]
pub struct AfterLowest {
    pub feature: Id,
    pub role: qymcad_core::names::Role,
    pub of: Vec<u32>,
}

impl Roles {
    /// The number a face of this role is handed to the C++ side under.
    pub fn give(&mut self, name: qymcad_core::names::GeoName) -> u32 {
        self.hand(RoleRecipe::Named(name))
    }

    /// The number a face named after other roles is handed under; those roles are given before it.
    pub fn give_after(&mut self, after: AfterLowest) -> u32 {
        self.hand(RoleRecipe::AfterLowest(after))
    }

    fn hand(&mut self, recipe: RoleRecipe) -> u32 {
        let number = qymcad_core::names::ROLE | self.given.len() as u32;
        self.given.push(RoleName { number, recipe });
        number
    }
}

/// A PIECE OF AN OPERATION THAT MAKES SEVERAL BODIES: what operation, the body it lands as, the roles of its faces, and
/// the shape the work made.
pub struct Piece {
    pub kind: Op,
    pub body: Id,
    pub roles: Roles,
    pub shape: crate::Shape,
}

/// AN OPERATION AS THE DOOR TAKES IT.
pub struct Operation<W: Fn(&Bodies, f64) -> Result<crate::Shape, CoreError>> {
    /// What it is: the name a refusal is told under.
    pub kind: Op,
    /// The body its result becomes.
    pub body: Id,
    pub sizing: Sizing,
    /// The names of the faces it makes, under the numbers its work hands the C++ side.
    pub roles: Roles,
    /// The work: the OCCT algorithm called at a factor on the size (1.0 first), on the bodies the door lends it. It
    /// scales every length of the operation by the factor and nothing else - an angle stays an angle; an unsized
    /// operation is called at 1.0 only.
    pub work: W,
}

/// THE BODIES OF THE CACHE, LENT TO THE WORK of an operation for its attempt and taken back before the result lands: a
/// work holding them while the result is cached would be a borrow of the cache across its own write.
pub struct Bodies<'a> {
    cache: &'a std::collections::HashMap<Id, crate::Shape>,
}

impl Bodies<'_> {
    /// The body `id`, or "the source body is not built".
    pub fn get(&self, id: Id) -> Result<&crate::Shape, CoreError> {
        self.cache.get(&id).ok_or(CoreError::SourceBodyNotBuilt)
    }
}

impl crate::kernel::OcctKernel {
    /// RUN AN OPERATION AND LAND ITS RESULT as the body `op.body`: the work at its size, stepped back on a refusal
    /// where it has one, the result cleaned and checked, tessellated and cached.
    pub fn run<W: Fn(&Bodies, f64) -> Result<crate::Shape, CoreError>>(&self, op: Operation<W>) -> Result<Built, CoreError> {
        // the words of a refusal are those of this operation or none: an old refusal turned into an error some other way
        // left its words behind, and a refusal here without words of its own read them as its reason
        crate::clear_kernel_refusal();
        let shape = self.attempt(&op)?;
        let built = self.finish(op.body, shape)?;
        // kept with the body until its result lands and the table names them
        self.roles.borrow_mut().insert(op.body, op.roles);
        Ok(built)
    }

    /// AN OPERATION WITH NO SIZE AND NO ROLES OF ITS OWN: its work on the bodies lent, the rest as for every operation.
    pub fn plain<W: Fn(&Bodies) -> Result<crate::Shape, CoreError>>(&self, kind: Op, body: Id, work: W) -> Result<Built, CoreError> {
        self.run(Operation { kind, body, sizing: Sizing::Unsized, roles: Roles::default(), work: |b: &Bodies, _: f64| work(b) })
    }

    /// The same, for an operation whose faces carry the roles `roles` gave.
    pub fn plain_named<W: Fn(&Bodies) -> Result<crate::Shape, CoreError>>(&self, kind: Op, body: Id, roles: Roles, work: W) -> Result<Built, CoreError> {
        self.run(Operation { kind, body, sizing: Sizing::Unsized, roles, work: |b: &Bodies, _: f64| work(b) })
    }

    /// ONE PIECE OF AN OPERATION THAT MAKES SEVERAL BODIES (a split): its work done once for all its pieces, each landed
    /// as every result is - cleaned, checked, tessellated, its roles kept for the names.
    pub fn land_piece(&self, piece: Piece) -> Result<Built, CoreError> {
        let Piece { body, roles, shape, .. } = piece;
        let landed = self.finish(body, shape)?;
        self.roles.borrow_mut().insert(body, roles);
        Ok(landed)
    }

    /// A BODY BROUGHT IN FROM A FILE THROUGH THE DOOR, without losing it: a copy is cleaned and checked as every result
    /// is and lands when it passes; refused, the body stays as it came and the refusal is said beside it. The file is
    /// all there is of such a body - no recipe to build it again - so a refusal must not take it away.
    pub fn land_as_it_came(&self, body: Id) -> Option<qymcad_core::feature::ImportLanded> {
        crate::clear_kernel_refusal();
        let copy = self.shapes.borrow().get(&body)?.transformed(&qymcad_core::feature::PLACE_IDENTITY)?;
        match self.clean(copy) {
            Ok(clean) => {
                let built = clean.tessellate_merged_auto(self.k())?;
                self.shapes.borrow_mut().insert(body, clean);
                Some(qymcad_core::feature::ImportLanded { built, kept: None })
            }
            Err(_) => {
                let built = self.shapes.borrow().get(&body)?.tessellate_merged_auto(self.k())?;
                Some(qymcad_core::feature::ImportLanded { built, kept: Some(CoreError::ImportKeptAsItCame) })
            }
        }
    }

    /// A TRIAL OF AN OPERATION: the same work, the same steps back, the same cleaning and checks - and nothing landed.
    /// What a tool asks before it lays its node, so the answer it gets is the one the node will get.
    pub fn trial<W: Fn(&Bodies, f64) -> Result<crate::Shape, CoreError>>(&self, op: Operation<W>) -> Result<crate::Shape, CoreError> {
        let shape = self.attempt(&op)?;
        self.clean(shape)
    }

    /// The work at the asked size; on a refusal of a sized operation, at each step back in turn - on the bodies of the
    /// cache, lent for the attempt only. The refusal told is the one at the asked size: a step back is the door's
    /// business, the person asked for the size.
    fn attempt<W: Fn(&Bodies, f64) -> Result<crate::Shape, CoreError>>(&self, op: &Operation<W>) -> Result<crate::Shape, CoreError> {
        let cache = self.shapes.borrow();
        let bodies = Bodies { cache: &cache };
        let asked = (op.work)(&bodies, 1.0);
        match (asked, op.sizing) {
            (Ok(shape), _) => Ok(shape),
            (Err(e), Sizing::Unsized) => Err(e),
            (Err(e), Sizing::Sized(back)) => back
                .retreat
                .steps()
                .iter()
                .find_map(|f| {
                    let shape = (op.work)(&bodies, *f).ok()?;
                    shape.drop_slivers_under(back.strip(*f));
                    Some(shape)
                })
                .ok_or(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A block 10 x 10 x 10, built at `f` times its side.
    fn block(f: f64) -> Result<crate::Shape, CoreError> {
        let s = 10.0 * f;
        crate::Shape::extrude(&[0.0, 0.0, s, 0.0, s, s, 0.0, s], s).ok_or(CoreError::OpFailed(Op::Extrude))
    }

    /// A work refusing exactly at its size and building below it: the door steps back and lands the result.
    #[test]
    fn a_sized_operation_refused_at_its_size_lands_stepped_back() {
        let k = crate::OcctKernel::default();
        let op = Operation {
            kind: Op::Fillet,
            body: 7,
            sizing: Sizing::Sized(StepBack { retreat: Retreat::Blend, size: 10.0 }),
            roles: Roles::default(),
            work: |_: &Bodies, f: f64| if f < 1.0 { block(f) } else { Err(CoreError::OpFailed(Op::Fillet)) },
        };
        let built = k.run(op).expect("landed stepped back");
        assert!(!built.faces.is_empty(), "the result landed with no faces");
        assert!(k.shapes.borrow().get(&7).is_some_and(|s| (s.volume() - 1000.0).abs() < 1e-3), "the body is not in the cache at the step back");
    }

    /// THE WORDS OF AN OLD REFUSAL DO NOT OUTLIVE THE START OF A NEW OPERATION. A refusal that turned into an error
    /// some other way left its words in the buffer, and the next operation refused without words of its own read them
    /// as its reason: an offset of a round face past its radius was told "remove face: nothing was removed".
    #[test]
    fn an_operation_starts_with_no_words_of_an_old_refusal() {
        let k = crate::OcctKernel::default();
        let _ = crate::refuse("remove face/result", "nothing was removed");
        let op = Operation { kind: Op::Extrude, body: 9, sizing: Sizing::Unsized, roles: Roles::default(), work: |_: &Bodies, _: f64| block(1.0) };
        k.run(op).expect("the block landed");
        assert_eq!(crate::last_kernel_refusal(), None, "the words of the old refusal are still there for the next one to take");
    }

    /// A BODY BROUGHT IN IS NOT LOST AT THE DOOR: a clean one is cleaned and landed with nothing said; one the door refuses
    /// (here the rebuild is asked to stop on the way, the door breaking off its cleaning) stays in the cache as it came,
    /// and the refusal is said.
    #[test]
    fn a_body_brought_in_passes_the_door_or_stays_as_it_came() {
        let k = crate::OcctKernel::default();
        k.shapes.borrow_mut().insert(5, block(1.0).expect("a block"));
        let clean = k.land_as_it_came(5).expect("the block landed");
        assert_eq!(clean.kept, None, "a clean block was said to be kept as it came");
        k.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let before = k.shapes.borrow().get(&5).and_then(|s| s.to_brep_bytes());
        let kept = k.land_as_it_came(5).expect("the body is still answered");
        assert_eq!(kept.kept, Some(CoreError::ImportKeptAsItCame), "a body the door refused is not said to be kept as it came");
        assert!(!kept.built.faces.is_empty(), "a body kept as it came gave no faces");
        assert_eq!(k.shapes.borrow().get(&5).and_then(|s| s.to_brep_bytes()), before, "the body the door refused was taken away or changed");
    }

    /// An unsized operation refused is refused: no step back, and the refusal told is its own.
    #[test]
    fn an_unsized_operation_is_not_stepped_back() {
        let k = crate::OcctKernel::default();
        let asked = std::cell::Cell::new(0);
        let op = Operation {
            kind: Op::Boolean,
            body: 7,
            sizing: Sizing::Unsized,
            roles: Roles::default(),
            work: |_: &Bodies, f: f64| {
                asked.set(asked.get() + 1);
                if f < 1.0 {
                    block(f)
                } else {
                    Err(CoreError::CutRemovedNothing)
                }
            },
        };
        assert_eq!(k.run(op).err(), Some(CoreError::CutRemovedNothing), "the refusal is not the operation's own");
        assert_eq!(asked.get(), 1, "an unsized operation was asked again");
        assert!(k.shapes.borrow().is_empty(), "a refused operation landed a body");
    }

    /// A trial runs the same door and lands nothing.
    #[test]
    fn a_trial_lands_nothing() {
        let k = crate::OcctKernel::default();
        let op = Operation { kind: Op::Extrude, body: 7, sizing: Sizing::Unsized, roles: Roles::default(), work: |_: &Bodies, f: f64| block(f) };
        let shape = k.trial(op).expect("the trial builds");
        assert!((shape.volume() - 1000.0).abs() < 1e-3, "the trial did not give the block");
        assert!(k.shapes.borrow().is_empty(), "a trial landed a body in the cache");
    }
}
