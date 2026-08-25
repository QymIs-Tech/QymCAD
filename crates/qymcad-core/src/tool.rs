//! The tool library, in outline. It grows later with geometry, the holder, feeds and speeds, and a binding to
//! the slots of a tool changer by `T` number.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolType {
    FlatEnd,
    BallNose,
    BullNose,
    VBit,
    Engraver,
    Drill,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tool {
    /// The tool number, T, compatible with an M6 tool change and with changer slots.
    pub number: u32,
    pub name: String,
    pub kind: ToolType,
    pub diameter: f64,
    /// The corner radius: used by a bull-nose cutter, zero for a flat end and equal to the radius for a ball
    /// end.
    pub corner_radius: f64,
    pub flutes: u32,
    /// The included angle of a V cutter in degrees, for V bits and engravers only.
    pub v_angle: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ToolLibrary {
    pub tools: Vec<Tool>,
}

impl Tool {
    pub fn radius(&self) -> f64 {
        self.diameter / 2.0
    }
}

impl ToolLibrary {
    pub fn get(&self, number: u32) -> Option<&Tool> {
        self.tools.iter().find(|t| t.number == number)
    }
}
