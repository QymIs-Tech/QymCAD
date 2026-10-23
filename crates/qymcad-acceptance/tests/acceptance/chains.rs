//! GROWN CHAINS, PLAYED: a person's sitting grown from a seed out of the descriptions of the tools, played through the
//! window with the oracles looking at every step and at the round trips after the last one. A chain that fails names
//! its seed, so it can be grown and played again.
use qymcad_acceptance::chains::{check, grow, play_grounded, seeds, Scene, Tier};
use qymcad_acceptance::probe;

probe! {
    /// A GROWN CHAIN IS PLAYED THROUGH: forty steps of the seed 1 on an open sketch, and most of them are steps a
    /// person could make there - a chain that aims at nothing proves nothing.
    fn a_grown_chain_is_played_through() {
        let steps = grow(1, 40, Scene::Sketch);
        let mut s = Scene::Sketch.start();
        // how much of a chain can be made is what is measured here: an undo that would take the sketch the chain draws
        // in away is aimed past it (two undos in a row reached it once empty steps stopped padding the list)
        match play_grounded(&mut s, &steps) {
            Ok(played) => assert!(played.done * 2 > steps.len(), "of {} steps of the seed 1 only {} could be made, skipped {:?}: the chain aims at nothing\n{steps:#?}", steps.len(), played.done, played.skipped),
            Err(stop) => panic!("the seed 1 stopped at step {} ({:?}):\n{}\nthe chain:\n{steps:#?}", stop.at, steps[stop.at], stop.said),
        }
    }
}

probe! {
    budget = 300;
    /// A CHAIN GIVEN BY THE ENVIRONMENT, played for the shrinking of another in a process of its own; run as one of the
    /// set, with nothing given, it has nothing to play.
    fn a_chain_given_by_the_environment() {
        qymcad_acceptance::chains::play_from_env();
    }
}

/// THE CHAINS OF A SLOT, at the level the run is asked for (`QYMCAD_TIER`), each held to every oracle.
fn a_slot(slot: u64) {
    for (seed, len) in seeds(Tier::asked(), slot) {
        if let Err(said) = check(seed, len, Scene::Sketch) {
            panic!("{said}");
        }
    }
}

probe! {
    budget = 1800;
    /// THE CHAINS OF THE SLOT 0 of a sketch hold under every oracle.
    fn the_chains_of_the_slot_0_hold() {
        a_slot(0);
    }
}

probe! {
    budget = 1800;
    /// THE CHAINS OF THE SLOT 1 of a sketch hold under every oracle.
    fn the_chains_of_the_slot_1_hold() {
        a_slot(1);
    }
}

probe! {
    budget = 1800;
    /// THE CHAINS OF THE SLOT 2 of a sketch hold under every oracle.
    fn the_chains_of_the_slot_2_hold() {
        a_slot(2);
    }
}

probe! {
    budget = 1800;
    /// THE CHAINS OF THE SLOT 3 of a sketch hold under every oracle.
    fn the_chains_of_the_slot_3_hold() {
        a_slot(3);
    }
}

probe! {
    budget = 1800;
    /// THE CHAINS OF THE SLOT 4 of a sketch hold under every oracle.
    fn the_chains_of_the_slot_4_hold() {
        a_slot(4);
    }
}

probe! {
    budget = 1800;
    /// THE CHAINS OF THE SLOT 5 of a sketch hold under every oracle.
    fn the_chains_of_the_slot_5_hold() {
        a_slot(5);
    }
}

probe! {
    budget = 1800;
    /// THE CHAINS OF THE SLOT 6 of a sketch hold under every oracle.
    fn the_chains_of_the_slot_6_hold() {
        a_slot(6);
    }
}

probe! {
    budget = 1800;
    /// THE CHAINS OF THE SLOT 7 of a sketch hold under every oracle.
    fn the_chains_of_the_slot_7_hold() {
        a_slot(7);
    }
}

probe! {
    /// THE CHAIN FOUND BY THE SEED 13, shrunk to the steps that matter: a spline, then a polygon of twelve sides and a
    /// second one whose corner is clicked on its own centre - the memory runs away.
    fn the_chain_of_the_seed_13() {
        use qymcad_acceptance::chains::{replay, Scene, Step};
        replay(Scene::Sketch, &[
            Step::Take { tool: "sketch.spline", entry: 1 },
            Step::Click { tool: "sketch.spline", at: 0 },
            Step::Finish { tool: "sketch.spline" },
            Step::Take { tool: "sketch.polygon", entry: 1 },
            Step::Value { tool: "sketch.polygon", field: 0, text: "12".into() },
            Step::Click { tool: "sketch.polygon", at: 0 },
            Step::Click { tool: "sketch.polygon", at: 1 },
            Step::Click { tool: "sketch.polygon", at: 1 },
            Step::Finish { tool: "sketch.polygon" },
        ]);
    }
}

probe! {
    /// THE CHAIN FOUND BY THE SEED 43, shrunk to the steps that matter (as far as its shrinking had time to go): after
    /// it, rebuilding everything never comes to rest.
    fn the_chain_of_the_seed_43() {
        use qymcad_acceptance::chains::{replay, Scene, Step};
        replay(Scene::Sketch, &[
            Step::Take { tool: "sketch.slot", entry: 2 },
            Step::Click { tool: "sketch.slot", at: 1 },
            Step::Enter,
            Step::Click { tool: "sketch.slot", at: 0 },
            Step::Click { tool: "sketch.slot", at: 2 },
            Step::Take { tool: "sketch.polygon", entry: 2 },
            Step::Enter,
            Step::Miss,
            Step::Value { tool: "sketch.polygon", field: 0, text: "nosuchname".into() },
            Step::Click { tool: "sketch.polygon", at: 1 },
            Step::Click { tool: "sketch.polygon", at: 0 },
            Step::SaveAndOpen,
            Step::Click { tool: "sketch.polygon", at: 1 },
            Step::Miss,
            Step::Enter,
            Step::Take { tool: "sketch.arc", entry: 0 },
            Step::Take { tool: "sketch.rect", entry: 0 },
            Step::Click { tool: "sketch.rect", at: 1 },
            Step::Click { tool: "sketch.rect", at: 0 },
            Step::Click { tool: "sketch.rect", at: 1 },
            Step::Click { tool: "sketch.rect", at: 1 },
            Step::Escape,
            Step::Undo,
            Step::Enter,
            Step::Take { tool: "sketch.circle", entry: 1 },
            Step::Click { tool: "sketch.circle", at: 1 },
            Step::Click { tool: "sketch.circle", at: 0 },
            Step::Finish { tool: "sketch.circle" },
            Step::Click { tool: "sketch.circle", at: 1 },
            Step::Click { tool: "sketch.circle", at: 1 },
            Step::Click { tool: "sketch.spline", at: 2 },
            Step::Click { tool: "sketch.spline", at: 1 },
            Step::Click { tool: "sketch.spline", at: 2 },
            Step::Enter,
            Step::Take { tool: "sketch.dim-radius", entry: 0 },
            Step::Click { tool: "sketch.dim-radius", at: 0 },
            Step::Take { tool: "sketch.polygon", entry: 2 },
            Step::Take { tool: "sketch.dim", entry: 0 },
            Step::Enter,
            Step::Finish { tool: "sketch.dim" },
            Step::Value { tool: "sketch.dim", field: 0, text: "40".into() },
            Step::Finish { tool: "sketch.dim" },
            Step::Click { tool: "sketch.dim", at: 0 },
            Step::Enter,
            Step::Redo,
            Step::Finish { tool: "sketch.dim" },
            Step::Click { tool: "sketch.dim", at: 1 },
            Step::Undo,
            Step::Click { tool: "sketch.dim", at: 1 },
            Step::Take { tool: "sketch.text", entry: 1 },
            Step::Value { tool: "sketch.text", field: 0, text: "inf".into() },
            Step::Finish { tool: "sketch.text" },
            Step::Value { tool: "sketch.text", field: 0, text: "0.1".into() },
            Step::SaveAndOpen,
            Step::Click { tool: "sketch.arc", at: 2 },
            Step::Take { tool: "sketch.dim", entry: 0 },
            Step::Enter,
            Step::Click { tool: "sketch.dim", at: 0 },
            Step::Take { tool: "sketch.polygon", entry: 0 },
            Step::Click { tool: "sketch.polygon", at: 0 },
            Step::Value { tool: "sketch.dim-radius", field: 0, text: "20.5".into() },
            Step::Take { tool: "sketch.line", entry: 1 },
            Step::Miss,
            Step::Click { tool: "sketch.line", at: 1 },
            Step::Enter,
            Step::Enter,
        ]);
    }
}

probe! {
    /// THE CHAIN FOUND BY THE SEED 40, shrunk to the steps that matter.
    fn the_chain_of_the_seed_40() {
        use qymcad_acceptance::chains::{replay, Scene, Step};
        replay(Scene::Sketch, &[
            Step::Take { tool: "sketch.polygon", entry: 0 },
            Step::Finish { tool: "sketch.polygon" },
            Step::Click { tool: "sketch.polygon", at: 0 },
            Step::Click { tool: "sketch.spline", at: 2 },
            Step::Click { tool: "sketch.spline", at: 1 },
            Step::Escape,
            Step::Take { tool: "sketch.arc", entry: 1 },
            Step::Click { tool: "sketch.arc", at: 1 },
            Step::Click { tool: "sketch.arc", at: 1 },
            Step::Miss,
            Step::Take { tool: "sketch.dim", entry: 0 },
            Step::Click { tool: "sketch.dim", at: 0 },
            Step::Click { tool: "sketch.dim", at: 0 },
            Step::Escape,
            Step::Undo,
            Step::Click { tool: "sketch.spline", at: 1 },
        ]);
    }
}

probe! {
    /// THE CHAIN FOUND BY THE SEED 12, shrunk to the steps that matter.
    fn the_chain_of_the_seed_12() {
        use qymcad_acceptance::chains::{replay, Scene, Step};
        replay(Scene::Sketch, &[
            Step::Take { tool: "sketch.spline", entry: 0 },
            Step::Click { tool: "sketch.spline", at: 2 },
            Step::Click { tool: "sketch.spline", at: 0 },
            Step::Click { tool: "sketch.spline", at: 2 },
            Step::Click { tool: "sketch.spline", at: 0 },
            Step::Click { tool: "sketch.spline", at: 1 },
            Step::Miss,
            Step::Click { tool: "sketch.spline", at: 0 },
            Step::Click { tool: "sketch.spline", at: 2 },
        ]);
    }
}
