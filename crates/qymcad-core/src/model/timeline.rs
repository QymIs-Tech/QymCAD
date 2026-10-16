//! Feature timeline: the nodes, their order, their owners, the rollback bar and editing the feature tree.
//!
//! The split is mechanical: methods that touch `timeline` and `rollback` and nothing else in the model live
//! here. Those interwoven with sketches, assemblies or datums stay where they are, since moving them would
//! relocate the entanglement rather than untangle it.
//!
//! The rebuild itself lives separately (`model::regen`); this module is about the structure of the tree rather
//! than about building geometry from it.

use super::*;

impl Project {}

mod delete;
mod nodes;
mod clone;
mod add_ops;
mod state;
mod order;
