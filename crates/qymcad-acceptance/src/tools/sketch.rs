//! THE TOOLS OF THE SKETCH.
use qymcad::{Key, Modifiers};

use crate::contract::fixtures::Fixture;
use crate::contract::{When, Class, Context, Entry, Field, Finish, Flow, Mode, Outcome, Pick, Tool, Under, Words};

/// Why the points of the contract that belong to a command do not apply to a tool that draws.
const DRAWS: &[(u8, &str)] = &[
    (3, "a drawing takes no reference: a click puts a point down, it does not pick something already there"),
    (4, "a drawing takes no reference, so there is no wrong one to refuse"),
    (13, "a drawn shape has no node in the tree to double-click: its dimensions are edited on the sheet (see the dimensions of B1)"),
    (14, "nothing of the timeline stands above a drawing but the plane of its sketch, which is checked with the sketch itself"),
    (15, "a drawing stands on its sketch alone: deleting the sketch takes the drawing with it, and that is checked with the sketch"),
    (17, "the kernel builds nothing here: a sketch is drawn in the plane, and the geometry holds whatever was clicked"),
];

/// The points above, and those of a tool with no field of its own.
const DRAWS_WITHOUT_FIELDS: &[(u8, &str)] = &[
    (3, DRAWS[0].1),
    (4, DRAWS[1].1),
    (5, "the tool has no field: what it draws is given by its clicks, and sized afterwards by a dimension"),
    (6, "the tool has no field to refuse a value in"),
    (13, DRAWS[2].1),
    (14, DRAWS[3].1),
    (15, DRAWS[4].1),
    (17, DRAWS[5].1),
];

mod draws;
pub use draws::*;
mod dims;
pub use dims::*;
mod cuts;
pub use cuts::*;
mod corners;
pub use corners::*;
mod projects;
pub use projects::*;
mod acts;
pub use acts::*;
mod constraints;
pub use constraints::*;
mod edits;
pub use edits::*;
mod reads;
pub use reads::*;
