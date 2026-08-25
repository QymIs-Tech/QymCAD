//! The toolpath intermediate representation: the neutral layer between geometry and the post-processor.
//!
//! Geometry and operations produce a `Toolpath` and nothing else. G-code is not printed here — that is the work
//! of the post-processor alone (`qymcad-cam`). This way Mach3, GRBL, LinuxCNC and any custom controller
//! receive the same toolpath and differ only in their output.

use crate::geom::Point3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Units {
    Mm,
    Inch,
}

impl Default for Units {
    fn default() -> Self {
        Units::Mm
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Plane {
    /// G17
    XY,
    /// G18
    XZ,
    /// G19
    YZ,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArcDir {
    /// G2
    Cw,
    /// G3
    Ccw,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpindleDir {
    /// M3
    Cw,
    /// M4
    Ccw,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoolantMode {
    /// M9
    Off,
    /// M7
    Mist,
    /// M8
    Flood,
}

/// A high-level drilling cycle. The post-processor decides whether to emit a real cycle, such as G81, G82 or
/// G83, or to expand it into G0 and G1 moves for controllers without cycles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrillKind {
    /// G81: a plain drill.
    Drill,
    /// G82: with a dwell at the bottom.
    DwellDrill,
    /// G83: peck drilling.
    Peck,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolRef {
    pub number: u32,
    pub name: String,
}

/// One element of a toolpath. The post-processor maps each variant onto a G or M code.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Move {
    /// G0: a rapid at maximum speed.
    Rapid { to: Point3 },
    /// G1: a linear cutting move.
    Linear { to: Point3, feed: f64 },
    /// G1 along Z: a plunge, kept separate for the plunge feed rate and for readability.
    Plunge { to: Point3, feed: f64 },
    /// G2 or G3: an arc with its centre given as IJK from the start, in the active plane.
    Arc {
        to: Point3,
        center: Point3,
        plane: Plane,
        dir: ArcDir,
        feed: f64,
    },
    /// A helical ramp entry.
    Helix {
        to: Point3,
        center: Point3,
        dir: ArcDir,
        pitch: f64,
        feed: f64,
    },
    /// G4: a dwell, in seconds.
    Dwell { secs: f64 },
    /// M3 or M4: spindle on.
    SpindleOn { rpm: f64, dir: SpindleDir },
    /// M5: spindle stop.
    SpindleOff,
    /// M6 with a T word: a tool change.
    ToolChange { tool: ToolRef },
    /// M7, M8 or M9: coolant.
    Coolant { mode: CoolantMode },
    /// A move along the rotary A axis, the fourth axis.
    RotaryTo { a: f64, feed: Option<f64> },
    /// A high-level drilling cycle over a list of points.
    DrillCycle {
        kind: DrillKind,
        points: Vec<Point3>,
        /// The retract plane, R.
        retract: f64,
        /// The drilling feed rate, F.
        feed: f64,
        /// The peck depth, Q, for G83.
        peck: Option<f64>,
        /// The dwell at the bottom, P, for G82.
        dwell: Option<f64>,
    },
    /// A comment, which the post-processor formats in its own style.
    Comment { text: String },
}

/// Metadata of an operation, which reaches the section header emitted by the post-processor.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct OpMeta {
    pub name: String,
    pub op_type: String,
    pub tool: Option<ToolRef>,
    pub est_seconds: Option<f64>,
    /// The work coordinate system, 54 to 59 for G54 to G59. Zero means unset, and G54 is used.
    #[serde(default)]
    pub wcs: u8,
}

/// The toolpath of a single operation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Toolpath {
    pub units: Units,
    pub meta: OpMeta,
    pub moves: Vec<Move>,
}

impl Toolpath {
    pub fn new(units: Units, meta: OpMeta) -> Self {
        Self { units, meta, moves: Vec::new() }
    }

    pub fn push(&mut self, m: Move) {
        self.moves.push(m);
    }
}

/// A program: the ordered toolpaths of several operations, one post-processor run giving one file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Program {
    pub name: String,
    pub units: Units,
    pub toolpaths: Vec<Toolpath>,
}
