//! Assembly solver: the positional problem of placing bodies so that their mates hold.
//!
//! What is hand-written here matters more than how much. Hand-written linear algebra (Gaussian
//! elimination without rank detection) and hand-written derivatives both fail silently: a degenerate
//! system is solved arbitrarily, and a wrong derivative shows up as "the body landed in the wrong
//! place", which is indistinguishable from bad geometry.
//!
//! Everything easy to get wrong is therefore taken from established libraries:
//!
//! * `nalgebra` — linear algebra, SVD for rank and null space, quaternions, `Isometry3`;
//! * `levenberg-marquardt` — damped Gauss-Newton (a MINPACK port);
//! * `num-dual` — derivatives by automatic differentiation, from the same code that computes the
//!   residual;
//! * `petgraph` — the mate graph: connected components, traversal from grounded bodies.
//!
//! What stays local is the only thing no library can know: what each mate means.

pub mod bridge;
pub mod connector;
pub mod decompose;
pub mod frame;
pub mod iterate;
pub mod joint;
pub mod problem;
pub mod solve;

pub use frame::{Anchor, Pose};
pub use problem::{Body, BodyId, Constraint, Problem};
