//! CAM: tools, operations, setups, stock and the machine.
//!
//! The split is mechanical: a method belongs here because it touches the fields of this subsystem alone, not
//! because of what it is called. Names are chosen by whoever writes them, and searching by name finds only what
//! was expected to be found.

use super::*;
use super::tess::*; // 2D sketch geometry: profiles, tessellation, decomposition into regions

// ── CAM types ───────────────────────────────────────────────────────────────────
// Moved out of `model.rs` line by line, with no change of logic: CAM is to become a separate module enabled by
// a setting, and the CAD core does not need its definitions at all.

/// The target controller, that is the post-processor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PostKind {
    Mach3,
    Grbl,
    LinuxCnc,
}

impl Default for PostKind {
    fn default() -> Self {
        PostKind::Mach3
    }
}

impl PostKind {
    pub fn label(&self) -> &'static str {
        match self {
            PostKind::Mach3 => "Mach3",
            PostKind::Grbl => "GRBL",
            PostKind::LinuxCnc => "LinuxCNC",
        }
    }
}

/// Output settings of the post-processor, mirroring `PostOptions`.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct PostConfig {
    pub comments: bool,
    pub header: bool,
    pub line_numbers: bool,
    pub axis_precision: u8,
    pub feed_precision: u8,
    pub tlo: bool,
    pub translate_cycles: bool,
}

impl Default for PostConfig {
    fn default() -> Self {
        Self { comments: true, header: true, line_numbers: false, axis_precision: 3, feed_precision: 3, tlo: true, translate_cycles: false }
    }
}

/// A machine profile: working envelope, limits and post-processor.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Machine {
    pub name: String,
    pub work_min: [f64; 3],
    pub work_max: [f64; 3],
    pub max_rapid: f64,
    pub max_feed: f64,
    pub max_rpm: f64,
    pub post: PostKind,
    #[serde(default)]
    pub post_cfg: PostConfig,
}

impl Default for Machine {
    fn default() -> Self {
        Self {
            name: "Generic router 600×400".into(),
            work_min: [0.0, 0.0, -100.0],
            work_max: [600.0, 400.0, 0.0],
            max_rapid: 5000.0,
            max_feed: 3000.0,
            max_rpm: 24000.0,
            post: PostKind::Mach3,
            post_cfg: PostConfig::default(),
        }
    }
}

/// The stock: its extents and its zero. `auto` takes the extents from the geometry.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Stock {
    pub auto: bool,
    pub size: [f64; 3],
    pub origin: [f64; 3],
}

impl Default for Stock {
    fn default() -> Self {
        Self { auto: true, size: [100.0, 100.0, 10.0], origin: [0.0; 3] }
    }
}

/// The work coordinate system, that is the work offset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Wcs {
    G54,
    G55,
    G56,
    G57,
    G58,
    G59,
}

impl Default for Wcs {
    fn default() -> Self {
        Wcs::G54
    }
}

impl Wcs {
    pub fn code(&self) -> u8 {
        match self {
            Wcs::G54 => 54,
            Wcs::G55 => 55,
            Wcs::G56 => 56,
            Wcs::G57 => 57,
            Wcs::G58 => 58,
            Wcs::G59 => 59,
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            Wcs::G54 => "G54",
            Wcs::G55 => "G55",
            Wcs::G56 => "G56",
            Wcs::G57 => "G57",
            Wcs::G58 => "G58",
            Wcs::G59 => "G59",
        }
    }
    pub const ALL: [Wcs; 6] = [Wcs::G54, Wcs::G55, Wcs::G56, Wcs::G57, Wcs::G58, Wcs::G59];
}

/// A setup: a group of operations sharing one coordinate system and orientation.
///
/// Several setups give two-sided or multi-position machining, using G54 through G59.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Setup {
    pub name: String,
    #[serde(default)]
    pub wcs: Wcs,
}

impl Default for Setup {
    fn default() -> Self {
        Self { name: "name-setup#1".into(), wcs: Wcs::G54 }
    }
}

/// The side for a contour operation, with an automatic mode driven by nesting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SideMode {
    /// Outer contours are cut from the outside and holes from the inside, decided by nesting.
    Auto,
    Outside,
    Inside,
    On,
}

/// The kind of an operation and the parameters specific to it.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum OpKind {
    Contour {
        side: SideMode,
        tabs: Tabs,
        ramp: Ramp,
        /// Climb milling: `true` for climb, `false` for conventional. It governs the direction in which the
        /// contour is traversed.
        #[serde(default)]
        climb: bool,
        /// A finish pass: leave `stock_to_leave` during roughing, then take one clean pass on size so the wall
        /// comes out to dimension.
        #[serde(default)]
        finish: bool,
    },
    Pocket { dogbone: bool },
    /// 2D adaptive, that is trochoidal, clearing over the selected contours.
    Adaptive2D,
    Drill { cycle: DrillKind, peck: Option<f64>, dwell: Option<f64> },
    /// Helical boring of holes, over circular contours.
    Bore,
    Face,
    Engrave,
    /// A slot along the centreline of the selected contours, one Z layer at a time.
    Slot,
    /// 3D finishing over a mesh by raster drop-cutter. `mesh` is the stable id of the mesh.
    Surface3D { mesh: Id },
    /// 3D roughing by Z levels, a level raster. `mesh` is the stable id of the mesh.
    Rough3D { mesh: Id },
    /// 3D finishing by Z levels, a waterline. `mesh` is the stable id of the mesh.
    Waterline3D { mesh: Id },
    /// A contour projected onto a surface, that is engraving over the part. `mesh` is the id of the mesh.
    Project3D { mesh: Id },
    /// Machining of horizontal flat areas. `mesh` is the id of the mesh.
    Flat3D { mesh: Id },
}

impl OpKind {
    pub fn label(&self) -> &'static str {
        match self {
            OpKind::Contour { .. } => "Contour",
            OpKind::Pocket { .. } => "Pocket",
            OpKind::Adaptive2D => "Adaptive 2D",
            OpKind::Drill { .. } => "Drill",
            OpKind::Bore => "Bore",
            OpKind::Face => "Face",
            OpKind::Engrave => "Engrave",
            OpKind::Slot => "Slot",
            OpKind::Surface3D { .. } => "Surface 3D",
            OpKind::Rough3D { .. } => "Rough 3D",
            OpKind::Waterline3D { .. } => "Waterline 3D",
            OpKind::Project3D { .. } => "Project 3D",
            OpKind::Flat3D { .. } => "Flat 3D",
        }
    }
}

/// The definition of an operation in the tree, referencing geometry and a tool.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OperationDef {
    pub name: String,
    pub enabled: bool,
    /// The tool number, a reference into `Project::tools`.
    pub tool: u32,
    /// Stable ids of the selected contours. Empty means every contour.
    pub selection: Vec<Id>,
    /// Stable ids of the selected bodies, for 3D operations. Empty means the body named in `kind`.
    #[serde(default)]
    pub bodies: Vec<Id>,
    /// The index of the setup in `Project::setups`; zero when there are no setups.
    #[serde(default)]
    pub setup: usize,
    pub heights: Heights,
    pub passes: Passes,
    pub feeds: Feeds,
    pub kind: OpKind,
}

impl OperationDef {
    /// A basic operation with sensible defaults.
    pub fn new(name: impl Into<String>, tool: u32, kind: OpKind) -> Self {
        Self {
            name: name.into(),
            enabled: true,
            tool,
            selection: Vec::new(),
            bodies: Vec::new(),
            setup: 0,
            heights: Heights::default(),
            passes: Passes::default(),
            feeds: Feeds::default(),
            kind,
        }
    }

}


impl Project {
    pub fn tool(&self, number: u32) -> Option<&Tool> {
        self.tools.iter().find(|t| t.number == number)
    }
    /// The coordinate system of an operation, taken from its setup, in the range 54 to 59.
    pub(super) fn op_wcs(&self, op: &OperationDef) -> u8 {
        self.setups.get(op.setup).map(|s| s.wcs.code()).unwrap_or(54)
    }
    /// The order of operations: grouped by setup, in setup order.
    pub(super) fn op_order_by_setup(&self) -> Vec<usize> {
        if self.setups.is_empty() {
            return (0..self.operations.len()).collect();
        }
        let mut order = Vec::new();
        for si in 0..self.setups.len() {
            for (oi, op) in self.operations.iter().enumerate() {
                if op.setup == si {
                    order.push(oi);
                }
            }
        }
        // operations with an invalid setup go last
        for (oi, op) in self.operations.iter().enumerate() {
            if op.setup >= self.setups.len() {
                order.push(oi);
            }
        }
        order
    }
}

// ── toolpath generation ─────────────────────────────────────────────────────────

impl Project {
    /// Build a program from the operations at the given indices only, for post-processing a single operation.
    /// The `enabled` flag is ignored.
    pub fn build_program_for(&self, name: &str, indices: &[usize]) -> Program {
        let mut toolpaths = Vec::new();
        for &idx in indices {
            if let Some(op) = self.operations.get(idx) {
                if let Some(tool) = self.tool(op.tool) {
                    if let Some(mut tp) = self.generate_op(op, tool) {
                        if tp.moves.len() > 4 {
                            tp.meta.wcs = self.op_wcs(op);
                            toolpaths.push(tp);
                        }
                    }
                }
            }
        }
        Program { name: name.to_string(), units: self.units, toolpaths }
    }

    /// Build a program from every enabled operation, in order.
    pub fn build_program(&self, name: &str) -> Program {
        let mut toolpaths = Vec::new();
        for oi in self.op_order_by_setup() {
            let op = &self.operations[oi];
            if !op.enabled {
                continue;
            }
            let Some(tool) = self.tool(op.tool) else { continue };
            if let Some(mut tp) = self.generate_op(op, tool) {
                if tp.moves.len() > 4 {
                    // more than just the preamble
                    tp.meta.wcs = self.op_wcs(op);
                    toolpaths.push(tp);
                }
            }
        }
        Program { name: name.to_string(), units: self.units, toolpaths }
    }

    /// The mesh for a 3D operation: the union of the bodies selected in `op.bodies`, of which there may be
    /// several — small nested ones, for instance. An empty list falls back to the body named in `OpKind`. This
    /// is what makes an operation cut the selected bodies rather than everything at once.
    fn op_mesh(&self, op: &OperationDef, fallback: Id) -> Option<crate::geom::Mesh> {
        let ids: Vec<Id> = if op.bodies.is_empty() { vec![fallback] } else { op.bodies.clone() };
        let mut out: Option<crate::geom::Mesh> = None;
        for id in ids {
            if let Some(i) = self.mesh_index(id) {
                let m = &self.bodies[i].mesh;
                match &mut out {
                    None => out = Some(m.clone()),
                    Some(acc) => {
                        let base = acc.verts.len() as u32;
                        acc.verts.extend_from_slice(&m.verts);
                        acc.tris.extend(m.tris.iter().map(|t| [t[0] + base, t[1] + base, t[2] + base]));
                    }
                }
            }
        }
        out.filter(|m| !m.tris.is_empty())
    }

    fn generate_op(&self, op: &OperationDef, tool: &Tool) -> Option<crate::ir::Toolpath> {
        // 3D operations work over a mesh rather than over contours
        if let OpKind::Surface3D { mesh } = op.kind {
            let m = self.op_mesh(op, mesh)?;
            return Some(
                SurfaceOp {
                    name: op.name.clone(),
                    tool: tool.clone(),
                    mesh: m.clone(),
                    heights: op.heights,
                    passes: op.passes,
                    feeds: op.feeds,
                    boundary: self.boundary_contours(op),
                }
                .generate(),
            );
        }
        if let OpKind::Rough3D { mesh } = op.kind {
            let m = self.op_mesh(op, mesh)?;
            return Some(
                Rough3DOp {
                    name: op.name.clone(),
                    tool: tool.clone(),
                    mesh: m.clone(),
                    heights: op.heights,
                    passes: op.passes,
                    feeds: op.feeds,
                    boundary: self.boundary_contours(op),
                }
                .generate(),
            );
        }
        if let OpKind::Waterline3D { mesh } = op.kind {
            let m = self.op_mesh(op, mesh)?;
            return Some(
                WaterlineOp {
                    name: op.name.clone(),
                    tool: tool.clone(),
                    mesh: m.clone(),
                    heights: op.heights,
                    passes: op.passes,
                    feeds: op.feeds,
                    boundary: self.boundary_contours(op),
                }
                .generate(),
            );
        }
        if let OpKind::Project3D { mesh } = op.kind {
            let m = self.op_mesh(op, mesh)?;
            // the contours are the selected ones, or all of them when nothing is selected
            let contours: Vec<Contour> = self.resolve_selection(op).iter().map(|(_, c)| (*c).clone()).collect();
            if contours.is_empty() {
                return None;
            }
            return Some(
                ProjectOp {
                    name: op.name.clone(),
                    tool: tool.clone(),
                    mesh: m.clone(),
                    contours,
                    heights: op.heights,
                    passes: op.passes,
                    feeds: op.feeds,
                }
                .generate(),
            );
        }
        if let OpKind::Flat3D { mesh } = op.kind {
            let m = self.op_mesh(op, mesh)?;
            return Some(
                FlatOp {
                    name: op.name.clone(),
                    tool: tool.clone(),
                    mesh: m.clone(),
                    heights: op.heights,
                    passes: op.passes,
                    feeds: op.feeds,
                }
                .generate(),
            );
        }

        let selected = self.resolve_selection(op);
        if selected.is_empty() {
            return None;
        }
        let contours: Vec<Contour> = selected.iter().map(|(_, c)| (*c).clone()).collect();

        match op.kind {
            OpKind::Contour { side, tabs, ramp, climb, finish } => {
                let mut tp = new_toolpath(&op.name, "contour", tool);
                intro(&mut tp, &op.name, tool, &op.feeds);
                tp.push(crate::ir::Move::Rapid {
                    to: crate::geom::Point3::new(0.0, 0.0, op.heights.clearance),
                });
                // helper: build the paths at radius r, honouring the side and the direction
                let build = |this: &Self, r: f64| -> Vec<Contour> {
                    let mut paths = Vec::new();
                    for (idx, c) in &selected {
                        let s = this.resolve_side(side, *idx);
                        paths.extend(contour_paths(std::slice::from_ref(*c), s, r));
                    }
                    if !climb {
                        for p in &mut paths {
                            p.points.reverse();
                        }
                    }
                    paths
                };
                // roughing passes, leaving `stock_to_leave` behind
                let rough = build(self, tool.radius() + op.passes.stock_to_leave);
                emit_profile(&mut tp, &rough, &op.heights, &op.passes, &op.feeds, Some(tabs), ramp);
                // the finish pass on size, a single pass at the floor, when enabled and stock is left
                if finish && op.passes.stock_to_leave > 1e-6 {
                    let fin = build(self, tool.radius());
                    let total = (op.heights.top - op.heights.bottom).abs() + 1.0;
                    let fin_passes = Passes { stepdown: total, stock_to_leave: 0.0, ..op.passes };
                    emit_profile(&mut tp, &fin, &op.heights, &fin_passes, &op.feeds, Some(tabs), ramp);
                }
                tp.push(crate::ir::Move::Rapid {
                    to: crate::geom::Point3::new(0.0, 0.0, op.heights.clearance),
                });
                Some(tp)
            }
            OpKind::Pocket { dogbone } => Some(
                PocketOp {
                    name: op.name.clone(),
                    tool: tool.clone(),
                    boundary: contours,
                    heights: op.heights,
                    passes: op.passes,
                    feeds: op.feeds,
                    dogbone: if dogbone { tool.radius() * 0.5 } else { 0.0 },
                }
                .generate(),
            ),
            OpKind::Adaptive2D => Some(
                AdaptiveOp {
                    name: op.name.clone(),
                    tool: tool.clone(),
                    boundary: contours,
                    heights: op.heights,
                    passes: op.passes,
                    feeds: op.feeds,
                }
                .generate(),
            ),
            OpKind::Bore => {
                let holes: Vec<(Point2, f64)> = contours
                    .iter()
                    .filter_map(|c| c.as_circle().map(|(ctr, r)| (ctr, r * 2.0)))
                    .collect();
                Some(
                    BoreOp {
                        name: op.name.clone(),
                        tool: tool.clone(),
                        holes,
                        heights: op.heights,
                        passes: op.passes,
                        feeds: op.feeds,
                    }
                    .generate(),
                )
            }
            OpKind::Drill { cycle, peck, dwell } => {
                let points: Vec<Point2> = contours.iter().map(|c| c.centroid()).collect();
                Some(
                    DrillOp {
                        name: op.name.clone(),
                        tool: tool.clone(),
                        points,
                        kind: cycle,
                        heights: op.heights,
                        peck,
                        dwell,
                        feeds: op.feeds,
                    }
                    .generate(),
                )
            }
            OpKind::Face => {
                let area = bbox_of(&contours)?;
                Some(
                    FaceOp {
                        name: op.name.clone(),
                        tool: tool.clone(),
                        area,
                        heights: op.heights,
                        passes: op.passes,
                        feeds: op.feeds,
                    }
                    .generate(),
                )
            }
            OpKind::Engrave => Some(
                EngraveOp {
                    name: op.name.clone(),
                    tool: tool.clone(),
                    contours,
                    heights: op.heights,
                    passes: op.passes,
                    feeds: op.feeds,
                }
                .generate(),
            ),
            OpKind::Slot => {
                // a slot along the centreline: the contours are cut as they are, one Z layer at a time
                let mut tp = new_toolpath(&op.name, "slot", tool);
                intro(&mut tp, &op.name, tool, &op.feeds);
                tp.push(crate::ir::Move::Rapid { to: crate::geom::Point3::new(0.0, 0.0, op.heights.clearance) });
                emit_profile(&mut tp, &contours, &op.heights, &op.passes, &op.feeds, None, crate::ops::params::Ramp::default());
                tp.push(crate::ir::Move::Rapid { to: crate::geom::Point3::new(0.0, 0.0, op.heights.clearance) });
                Some(tp)
            }
            OpKind::Surface3D { .. } | OpKind::Rough3D { .. } | OpKind::Waterline3D { .. } | OpKind::Project3D { .. } | OpKind::Flat3D { .. } => None, // handled above
        }
    }
}
