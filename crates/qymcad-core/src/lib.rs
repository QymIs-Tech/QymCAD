//! `qymcad-core`: the headless core.
//!
//! It holds the document model, the basic geometry and the solver. It does not depend on the interface and
//! builds and tests without one.

pub mod asm;
mod doc_file;
pub mod drivers;
pub mod errors;
pub mod expr;
pub mod feature;
pub mod geom;
pub mod measure;
pub mod model;
pub mod names;
pub mod offset;
pub mod part;
pub mod refs;
pub mod solver;
pub mod subdiv;
pub mod text;
pub mod thread;

