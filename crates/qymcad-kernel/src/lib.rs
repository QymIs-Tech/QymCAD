//! The B-rep kernel, through an FFI to the system OpenCASCADE.
//!
//! A STEP import gives a set of bodies, one per solid, and each body is a [`Mesh`] plus faces taken from the
//! real B-rep topology ([`MeshFace`]). From there on the B-rep is the source of truth for extrusions and
//! booleans.

/// The name of the C++ runtime to link, chosen by target - shared with `build.rs`.
pub mod cxx_runtime;
pub mod door;
pub mod ask;
pub mod imports;
#[cfg(test)]
mod inside;
pub mod kernel;
mod workers;
mod naming;
pub mod ops;
pub mod recognise;
mod shape;
#[cfg(any(test, feature = "specimens"))]
pub mod specimen;
pub use recognise::{recognise, Recognised};
pub use kernel::{kernel_gate, OcctKernel};
pub use workers::{wait_for_workers, worker};
pub use shape::*;
