//! THE PART WORKBENCH: the feature commands.
//!
//! Extrude, revolve, sweep, loft, fillet, chamfer, shell, the array, the thread, the hole, the split -
//! their option bars, their popups at the geometry, their previews and what they do to the document. Every
//! function here works over `qymcad_ui_state::PartCtx` and asks the application for nothing.

mod trial;
pub use trial::{trial_refusal, Trial};

use egui::{Pos2, Rect};
use egui_phosphor::regular as ph;
use qymcad_core::model::{ArrayAxis, Id, Project};
use qymcad_ui_state::WinKind;

mod bars;
pub use bars::*;
mod popups;
pub use popups::*;
mod params;
pub use params::*;
mod picks;
pub use picks::*;
mod topology;
pub use topology::*;
mod reopen;
pub use reopen::*;
mod start;
pub use start::*;
mod apply;
pub use apply::*;
