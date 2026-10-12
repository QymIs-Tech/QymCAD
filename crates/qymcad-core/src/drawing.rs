//! Associative drawing sheets: 2D documentation derived from the 3D model.
//!
//! A sheet holds views of components and dimensions of those views. The geometry is never copied into
//! the sheet: a view stores the id of its source component and the projection is computed on demand
//! from the live bodies, so an edit of the model moves the drawing with it. A dimension stores either
//! an expression over the project parameters (evaluated live, like any other field) or the two bodies
//! whose hole distance the workbench measures at draw time. Deleting what a view shows is refused at
//! creation; what disappears later reads as not live rather than as a dangling pointer.

use serde::{Deserialize, Serialize};

use crate::model::{Id, Project};

/// A sheet size in mm, width by height. Built only through [`SheetSize::of`], so a swapped pair cannot
/// compile: two neighbours of one type swap and still compile.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SheetSize {
    /// Width in mm.
    pub width: f64,
    /// Height in mm.
    pub height: f64,
}

/// Standard sheet formats (ISO 216), in mm.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SheetFormat {
    /// 297 x 210.
    A4Landscape,
    /// 210 x 297.
    A4Portrait,
    /// 420 x 297.
    A3Landscape,
    /// 297 x 420.
    A3Portrait,
}

/// A place on the sheet in mm from its lower-left corner. One struct rather than two `f64` neighbours.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SheetPos {
    /// Right in mm.
    pub x: f64,
    /// Up in mm.
    pub y: f64,
}

/// The viewing direction of a drawing view. An enum rather than angles: a direction has a name on the
/// sheet ("Front", "Top"), and three numbers would let a view look from nowhere in particular.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewDir {
    /// Along -Z.
    Front,
    /// Along -Y.
    Top,
    /// Along +X.
    Right,
    /// Isometric.
    Iso,
}

/// How a view is derived: a base view looks at a component, a projected view derives from another view
/// of the same sheet. The parent is an id rather than an index, like every other reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewKind {
    /// A first view of a component from a direction.
    Base {
        /// Where it looks from.
        dir: ViewDir,
    },
    /// A view projected from a base view of the same sheet.
    Projected {
        /// The view it is projected from.
        from: Id,
        /// Where it looks from.
        dir: ViewDir,
    },
}

/// One drawing view: what component it shows, how it is derived, at what scale and where on the sheet.
/// The projected edges are computed on demand from the live bodies, never stored here.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DrawingView {
    /// Stable id.
    pub id: Id,
    /// The component shown.
    pub source: Id,
    /// Base or projected.
    pub kind: ViewKind,
    /// Model mm per sheet mm.
    pub scale: f64,
    /// Centre of the view on the sheet.
    pub at: SheetPos,
}

/// What a dimension shows: an expression over the project parameters, evaluated live at every reading,
/// or the centre distance of two hole bodies, measured by the workbench at draw time.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum DrawingDimSource {
    /// An expression such as `h` or `w/2`, evaluated against the project parameters.
    Expression {
        /// The expression.
        expr: String,
    },
    /// The centre distance of two hole bodies: the dimension a drawing gives for a hole pattern.
    HoleDistance {
        /// The first hole body.
        first: Id,
        /// The second hole body.
        second: Id,
    },
}

/// One dimension on a view. The text is derived on every reading, never stored.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DrawingDim {
    /// Stable id.
    pub id: Id,
    /// The view it annotates.
    pub view: Id,
    /// What it shows.
    pub source: DrawingDimSource,
}

/// One drawing sheet: a named format with views and dimensions. Views and dims live inside the sheet
/// they annotate, so deleting a sheet deletes its views and dims with it and nothing dangles elsewhere.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DrawingSheet {
    /// Stable id.
    pub id: Id,
    /// Shown in the tree.
    pub name: String,
    /// The format.
    pub size: SheetSize,
    /// The views of the sheet.
    pub views: Vec<DrawingView>,
    /// The dimensions of the sheet.
    pub dims: Vec<DrawingDim>,
}

impl SheetSize {
    /// The size of a standard format in mm.
    pub fn of(format: SheetFormat) -> Self {
        match format {
            SheetFormat::A4Landscape => SheetSize { width: 297.0, height: 210.0 },
            SheetFormat::A4Portrait => SheetSize { width: 210.0, height: 297.0 },
            SheetFormat::A3Landscape => SheetSize { width: 420.0, height: 297.0 },
            SheetFormat::A3Portrait => SheetSize { width: 297.0, height: 420.0 },
        }
    }
}

impl Project {
    /// MAKE A DRAWING SHEET: a named sheet of a standard format. Returns its id.
    pub fn add_drawing_sheet(&mut self, name: impl Into<String>, format: SheetFormat) -> Id {
        let id = self.alloc_id();
        self.sheets.push(DrawingSheet { id, name: name.into(), size: SheetSize::of(format), views: Vec::new(), dims: Vec::new() });
        id
    }

    /// A drawing sheet by id.
    pub fn drawing_sheet(&self, id: Id) -> Option<&DrawingSheet> {
        self.sheets.iter().find(|s| s.id == id)
    }

    /// ADD A BASE VIEW of component `source` to a sheet. `None` when the sheet is missing, the source is
    /// no component, or the scale is not positive: a view of nothing, or at no scale, is not made.
    pub fn sheet_add_base_view(&mut self, sheet: Id, source: Id, dir: ViewDir, scale: f64, at: SheetPos) -> Option<Id> {
        if !scale.is_finite() || scale <= 0.0 || !self.components.iter().any(|c| c.id == source) {
            return None;
        }
        let id = self.alloc_id();
        self.sheets.iter_mut().find(|s| s.id == sheet)?.views.push(DrawingView { id, source, kind: ViewKind::Base { dir }, scale, at });
        Some(id)
    }

    /// ADD A PROJECTED VIEW derived from view `from` onto the same sheet. `None` when the sheet or the
    /// parent view is missing, or the scale is not positive.
    pub fn sheet_add_projected_view(&mut self, sheet: Id, from: Id, dir: ViewDir, scale: f64, at: SheetPos) -> Option<Id> {
        if !scale.is_finite() || scale <= 0.0 {
            return None;
        }
        let id = self.alloc_id();
        let s = self.sheets.iter_mut().find(|s| s.id == sheet)?;
        let source = s.views.iter().find(|v| v.id == from)?.source;
        s.views.push(DrawingView { id, source, kind: ViewKind::Projected { from, dir }, scale, at });
        Some(id)
    }

    /// ADD A DIMENSION to a view of a sheet. `None` when the sheet or the view is missing. An empty
    /// expression is refused: a dimension showing nothing is not made.
    pub fn sheet_add_dim(&mut self, sheet: Id, view: Id, source: DrawingDimSource) -> Option<Id> {
        if matches!(&source, DrawingDimSource::Expression { expr } if expr.trim().is_empty()) {
            return None;
        }
        let id = self.alloc_id();
        let s = self.sheets.iter_mut().find(|s| s.id == sheet)?;
        if !s.views.iter().any(|v| v.id == view) {
            return None;
        }
        s.dims.push(DrawingDim { id, view, source });
        Some(id)
    }

    /// IS THIS VIEW LIVE: its source component still exists, and for a projected view its parent view is
    /// on the same sheet and live. The projection itself is computed on demand from the live bodies, so
    /// liveness of the references is all there is to check.
    pub fn drawing_view_is_live(&self, sheet: Id, view: Id) -> bool {
        let Some(s) = self.sheets.iter().find(|s| s.id == sheet) else { return false };
        let Some(v) = s.views.iter().find(|v| v.id == view) else { return false };
        if !self.components.iter().any(|c| c.id == v.source) {
            return false;
        }
        match v.kind {
            ViewKind::Base { .. } => true,
            ViewKind::Projected { from, .. } => from != view && s.views.iter().any(|p| p.id == from && p.source == v.source) && self.drawing_view_is_live_inner(s, from),
        }
    }

    /// The text of a dimension, derived on every reading: an expression evaluates against the current
    /// project parameters, so editing a parameter moves the dimension with it. A hole distance is
    /// measured from live geometry by the workbench at draw time, so the core answers `None` rather
    /// than a stale number. `None` also when the sheet, the view or the dimension is missing, or the
    /// expression does not evaluate.
    pub fn drawing_dim_text(&self, sheet: Id, dim: Id) -> Option<String> {
        let s = self.sheets.iter().find(|s| s.id == sheet)?;
        let d = s.dims.iter().find(|d| d.id == dim)?;
        if !s.views.iter().any(|v| v.id == d.view) {
            return None;
        }
        match &d.source {
            DrawingDimSource::Expression { expr } => self.eval_expr(expr).ok().map(crate::expr::fmt_num),
            DrawingDimSource::HoleDistance { .. } => None,
        }
    }

    /// IS THIS HOLE-DISTANCE DIMENSION GROUNDED: both bodies exist and neither is consumed by a feature
    /// above it, so the workbench has live geometry to measure at draw time.
    pub fn drawing_dim_is_grounded(&self, sheet: Id, dim: Id) -> bool {
        let Some(s) = self.sheets.iter().find(|s| s.id == sheet) else { return false };
        let Some(d) = s.dims.iter().find(|d| d.id == dim) else { return false };
        let DrawingDimSource::HoleDistance { first, second } = d.source else { return true };
        if first == second {
            return false;
        }
        let consumed = self.consumed_bodies();
        [first, second].iter().all(|b| self.body_owner(*b).is_some() && !consumed.contains(b))
    }

    fn drawing_view_is_live_inner(&self, s: &DrawingSheet, view: Id) -> bool {
        let Some(v) = s.views.iter().find(|v| v.id == view) else { return false };
        if !self.components.iter().any(|c| c.id == v.source) {
            return false;
        }
        match v.kind {
            ViewKind::Base { .. } => true,
            ViewKind::Projected { from, .. } => from != view && s.views.iter().any(|p| p.id == from && p.source == v.source),
        }
    }
}
