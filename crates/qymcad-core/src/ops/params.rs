//! Parameters shared by the operations: heights, passes, feed rates, and helpers.

use crate::ir::{CoolantMode, Move, OpMeta, SpindleDir, Toolpath, ToolRef, Units};
use crate::tool::Tool;
use serde::{Deserialize, Serialize};

/// The side to machine relative to a contour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    /// Outside the contour, the outer profile of the part.
    Outside,
    /// Inside the contour: an inner cut-out or the wall of a pocket.
    Inside,
    /// On the centreline: engraving, or a slot the width of the cutter.
    On,
}

/// The Z levels, all absolute in the coordinate system of the part, with `top` at the top of the material.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Heights {
    /// The safe height for rapid moves.
    pub clearance: f64,
    /// The retract between passes.
    pub retract: f64,
    /// The top of the material.
    pub top: f64,
    /// The bottom of the cut, the final depth.
    pub bottom: f64,
}

impl Default for Heights {
    fn default() -> Self {
        Self { clearance: 5.0, retract: 2.0, top: 0.0, bottom: -1.0 }
    }
}

impl Heights {
    /// The Z levels from top to bottom in steps of `stepdown`, the last one being exactly `bottom`.
    pub fn z_levels(&self, stepdown: f64) -> Vec<f64> {
        let sd = stepdown.max(0.01);
        let mut levels = Vec::new();
        let mut z = self.top - sd;
        while z > self.bottom + 1e-6 {
            levels.push(z);
            z -= sd;
        }
        levels.push(self.bottom);
        levels
    }
}

/// The removal strategy.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Passes {
    /// The depth of cut per Z pass.
    pub stepdown: f64,
    /// The stepover in XY, for pockets and facing, in mm.
    pub stepover: f64,
    /// The radial stock left for finishing, in mm.
    pub stock_to_leave: f64,
}

impl Default for Passes {
    fn default() -> Self {
        Self { stepdown: 1.0, stepover: 2.0, stock_to_leave: 0.0 }
    }
}

/// Feed rates and the spindle and coolant settings.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Feeds {
    pub rpm: f64,
    /// The cutting feed rate, in mm per minute.
    pub cut: f64,
    /// The plunge feed rate, in mm per minute.
    pub plunge: f64,
    pub spindle_dir: SpindleDir,
    pub coolant: CoolantMode,
}

impl Default for Feeds {
    fn default() -> Self {
        Self {
            rpm: 12000.0,
            cut: 600.0,
            plunge: 200.0,
            spindle_dir: SpindleDir::Cw,
            coolant: CoolantMode::Off,
        }
    }
}

/// A ramped entry: descending at an angle along the contour instead of plunging vertically.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Ramp {
    pub enabled: bool,
    /// The descent angle, in degrees.
    pub angle_deg: f64,
}

impl Default for Ramp {
    fn default() -> Self {
        Self { enabled: true, angle_deg: 3.0 }
    }
}

/// Tabs, which hold the part in place during a through cut.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Tabs {
    pub enabled: bool,
    /// The number of tabs, spaced evenly around the perimeter.
    pub count: u32,
    /// The width of a tab, in mm.
    pub width: f64,
    /// The height of a tab above the floor, in mm.
    pub height: f64,
}

impl Default for Tabs {
    fn default() -> Self {
        Self { enabled: false, count: 4, width: 6.0, height: 1.5 }
    }
}

/// Create a `Toolpath` with its metadata.
pub fn new_toolpath(name: &str, op_type: &str, tool: &Tool) -> Toolpath {
    Toolpath::new(
        Units::Mm,
        OpMeta {
            name: name.to_string(),
            op_type: op_type.to_string(),
            tool: Some(ToolRef { number: tool.number, name: tool.name.clone() }),
            est_seconds: None,
            wcs: 0,
        },
    )
}

/// The preamble of an operation: a comment, a tool change, the spindle and the coolant.
pub fn intro(tp: &mut Toolpath, name: &str, tool: &Tool, feeds: &Feeds) {
    tp.push(Move::Comment { text: name.to_string() });
    tp.push(Move::ToolChange {
        tool: ToolRef { number: tool.number, name: tool.name.clone() },
    });
    tp.push(Move::SpindleOn { rpm: feeds.rpm, dir: feeds.spindle_dir });
    if feeds.coolant != CoolantMode::Off {
        tp.push(Move::Coolant { mode: feeds.coolant });
    }
}
