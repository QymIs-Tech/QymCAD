//! THE ASSEMBLY WORKBENCH: joints and mates.
//!
//! Creating a joint, picking its anchors, dragging the gizmo, animating a degree of freedom, the list of
//! relations. Every function here works over `qymcad_ui_state::JointCtx` - the records the assembly edits - and asks the
//! application for nothing.

use egui::{Pos2, Rect};
use egui_phosphor::regular as ph;
use qymcad_core::model::{Id, Project};
use qymcad_ui_state::*;

mod slots;
pub use slots::*;
mod picks;
pub use picks::*;
mod gizmo;
pub use gizmo::*;
mod bars;
pub use bars::*;
mod popup;
pub use popup::*;
mod commands;
pub use commands::*;
