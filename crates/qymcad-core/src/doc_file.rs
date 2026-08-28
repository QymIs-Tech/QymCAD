//! The schema of the document file, kept separate from the in-memory model.
//!
//! `document.ron` used to be a direct serialisation of `Project`, so the model in memory and the format on disk
//! were one and the same object. The consequence is heavy: any restructuring of the model automatically broke
//! already saved projects, and conversely the format froze the internal design. Splitting the document into
//! aggregates is impossible in principle under that arrangement, consisting as it does entirely of
//! restructuring `Project`.
//!
//! The schema is therefore a type of its own. It describes exactly what lies in the file and changes only when
//! that is intended. The in-memory model is free after this.
//!
//! Two consequences visible at once:
//!
//! * there are no derived fields in the schema. `regen_faces`, `regen_edges`, `regen_errors` and
//!   `mates_conflict` were marked `serde(skip)` — already not stored, yet sitting in the same row as real data
//!   and looking like part of the document. The difference is now visible in the type rather than in an
//!   attribute;
//! * the `legacy_*` fields, the old parallel mesh lists, stayed here where they belong: in the schema for
//!   reading old files, not in the model.

use serde::{Deserialize, Serialize};

use crate::geom::Contour;
use crate::tool::Tool;
use crate::ir::Units;
use crate::model::{
    Body, DatumAxis, DatumPoint, Id, Machine, NamedDim, OperationDef, Param, Project, Setup, Sketch, SourceFile, Stock, WorkPlane,
};

/// THE MAPS OF THE FILE ARE ORDERED, and that is about the file rather than about speed.
///
/// A `HashMap` iterates in whatever order its hashing feels like, and that order changes between runs of
/// the SAME binary: saving one document twice gave two files differing in 6608 lines, byte for byte, with
/// nothing edited in between. For a format that lives in a version control system this is poison - every
/// save reads as a rewritten file, and a real change cannot be seen among the noise.
///
/// `BTreeMap` costs a little on lookup and pays it back in a file a person can diff. The document is
/// written a few times a minute at most; the ordering is worth far more than the nanoseconds.
type Map<K, V> = std::collections::BTreeMap<K, V>;

/// The document as it lies in `document.ron`.
#[derive(Serialize, Deserialize)]
pub(crate) struct DocumentFile {
    pub units: Units,
    /// Document properties: author, title, version, comment.
    ///
    /// They were absent from the schema entirely: the field was added to the model, the format is a separate
    /// type, and nothing checked the connection between them. Properties were filled in, saved and lost
    /// silently; the tests nearby checked the creation date and whether anything leaked into the settings, but
    /// not whether any of it survives the file.
    #[serde(default)]
    pub meta: crate::model::DocMeta,
    /// Geometric tolerance. A property of the document in substance: the contents of an STL depend on it, and
    /// one file has to produce one export for everybody.
    #[serde(default)]
    pub geom_quality: crate::model::GeomQuality,
    /// Component patterns. Also absent from the schema: an assembly with a pattern saved and reopened without
    /// it — the copies remained but stopped being a pattern, so editing and deleting went one at a time.
    #[serde(default)]
    pub comp_patterns: Vec<crate::model::comp_pattern::CompPattern>,
    #[serde(default)]
    pub next_id: Id,
    #[serde(default)]
    pub names: crate::names::NameTable,
    pub contours: Vec<Contour>,
    #[serde(default)]
    pub contour_ids: Vec<Id>,
    #[serde(default)]
    pub contour_ents: Map<Id, Vec<Id>>,
    #[serde(default)]
    pub contour_parent: Map<Id, Id>,
    #[serde(default)]
    pub bodies: Vec<Body>,
    #[serde(default)]
    pub imported_bodies: std::collections::HashSet<Id>,
    #[serde(default)]
    pub part_colors: Map<Id, [u8; 3]>,
    pub tools: Vec<Tool>,
    pub operations: Vec<OperationDef>,
    #[serde(default)]
    pub setups: Vec<Setup>,
    pub stock: Stock,
    #[serde(default)]
    pub machine: Machine,
    #[serde(default)]
    pub planes: Vec<WorkPlane>,
    #[serde(default)]
    pub sketches: Vec<Sketch>,
    #[serde(default)]
    pub timeline: Vec<crate::feature::FeatureNode>,
    #[serde(default)]
    pub components: Vec<crate::feature::Component>,
    #[serde(default)]
    pub root: Id,
    #[serde(default)]
    pub active_component: Option<Id>,
    #[serde(default)]
    pub datum_points: Vec<DatumPoint>,
    #[serde(default)]
    pub datum_axes: Vec<DatumAxis>,
    #[serde(default)]
    pub connectors: Vec<crate::feature::MateConnector>,
    #[serde(default)]
    pub joints: Vec<crate::feature::Joint>,
    /// Groups: sets of parts fastened to one another. Not joints, since there are no connectors.
    #[serde(default)]
    pub mate_constraints: Vec<crate::feature::MateConstraint>,
    /// Relations between mates: gear, rack and pinion, screw, linear.
    #[serde(default)]
    pub relations: Vec<crate::feature::MateRelation>,
    /// Bodies that were deleted: a joint referencing one goes red rather than disappearing.
    #[serde(default)]
    pub dead_bodies: Vec<Id>,
    #[serde(default)]
    pub external_refs: Vec<crate::feature::ExternalRef>,
    #[serde(default)]
    pub named_dims: Vec<NamedDim>,
    #[serde(default)]
    pub sources: Vec<SourceFile>,
    #[serde(default)]
    pub parameters: Vec<Param>,
    #[serde(default)]
    pub feat_dims: Map<Id, Map<String, String>>,
    #[serde(default)]
    pub edge_refs: Map<Id, Vec<(u32, [f64; 3], [f64; 3])>>,
    /// Face snapshots: the same as `edge_refs` but for face references. They travel in the file for the same
    /// reason — they witness a reference in case the id of a face changes, as a number becomes a name.
    #[serde(default)]
    pub face_refs: Map<Id, Vec<(u32, [f64; 3], [f64; 3])>>,
    #[serde(default)]
    pub rollback: Option<usize>,
}

impl DocumentFile {
    /// Model to file schema. Derived fields are not carried across: the schema has none.
    pub(crate) fn from_model(p: &Project) -> Self {
        let parts = p.contours.parts();
        // STAMPED HERE, NOT IN THE MODEL: saving must not modify the open document. Were the field set on
        // `Project`, a save would change the document and mark it unsaved again the moment it was saved.
        let mut meta = p.meta.clone();
        meta.saved_by = crate::model::producer();
        Self {
            units: p.units.clone(),
            meta,
            geom_quality: p.geom_quality,
            comp_patterns: p.comp_patterns.clone(),
            next_id: p.next_id.clone(),
            names: p.names.clone(),
            contours: parts.0.to_vec(),
            contour_ids: parts.1.to_vec(),
            contour_ents: parts.2.iter().map(|(k, v)| (*k, v.clone())).collect(),
            contour_parent: parts.3.iter().map(|(k, v)| (*k, v.clone())).collect(),
            bodies: p.bodies.clone(),
            imported_bodies: p.imported_bodies.clone(),
            part_colors: p.part_colors.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            tools: p.tools.clone(),
            operations: p.operations.clone(),
            setups: p.setups.clone(),
            stock: p.stock.clone(),
            machine: p.machine.clone(),
            planes: p.planes.clone(),
            sketches: p.sketches.clone(),
            timeline: p.timeline.clone(),
            components: p.components.clone(),
            root: p.root.clone(),
            active_component: p.active_component.clone(),
            datum_points: p.datum_points.clone(),
            datum_axes: p.datum_axes.clone(),
            connectors: p.connectors.clone(),
            joints: p.joints.clone(),
            mate_constraints: p.mate_constraints.clone(),
            relations: p.relations.clone(),
            dead_bodies: p.dead_bodies.clone(),
            external_refs: p.external_refs.clone(),
            named_dims: p.named_dims.clone(),
            sources: p.sources.clone(),
            parameters: p.parameters.clone(),
            feat_dims: p.feat_dims.iter().map(|(k, v)| (*k, v.iter().map(|(a, b)| (a.clone(), b.clone())).collect())).collect(),
            edge_refs: p.edge_refs.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            face_refs: p.face_refs.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            rollback: p.rollback.clone(),
        }
    }

    /// File schema to model. The document restores its derived fields itself during a rebuild.
    pub(crate) fn into_model(self) -> Project {
        Project {
            units: self.units,
            meta: self.meta,
            geom_quality: self.geom_quality,
            comp_patterns: self.comp_patterns,
            next_id: self.next_id,
            names: self.names,
            contours: crate::model::contours::Contours::from_parts(self.contours, self.contour_ids, self.contour_ents.into_iter().collect(), self.contour_parent.into_iter().collect()),
            bodies: self.bodies,
            imported_bodies: self.imported_bodies,
            part_colors: self.part_colors.into_iter().collect(),
            tools: self.tools,
            operations: self.operations,
            setups: self.setups,
            stock: self.stock,
            machine: self.machine,
            planes: self.planes,
            sketches: self.sketches,
            timeline: self.timeline,
            components: self.components,
            root: self.root,
            active_component: self.active_component,
            datum_points: self.datum_points,
            datum_axes: self.datum_axes,
            connectors: self.connectors,
            joints: self.joints,
            mate_constraints: self.mate_constraints,
            relations: self.relations,
            dead_bodies: self.dead_bodies,
            external_refs: self.external_refs,
            named_dims: self.named_dims,
            sources: self.sources,
            parameters: self.parameters,
            feat_dims: self.feat_dims.into_iter().map(|(k, v)| (k, v.into_iter().collect())).collect(),
            edge_refs: self.edge_refs.into_iter().collect(),
            face_refs: self.face_refs.into_iter().collect(),
            rollback: self.rollback,
            ..Project::default()
        }
    }
}
