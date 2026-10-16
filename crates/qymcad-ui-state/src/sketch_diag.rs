//! THE STATE OF A SKETCH: its diagnostics, its keys of shape and place.

use super::*;

/// ALL of the sketch diagnostics in one pass and THROUGH A CACHE (recomputed only when the imprint
/// changes).
///
/// Every piece of it is a full Jacobian plus a Gaussian elimination (O(m*nv^2)). Only the degrees of
/// freedom and the free points used to be cached, while the conflicting set and the redundant
/// constraints were computed DIRECTLY and several times per frame: the panel, the list of constraints,
/// the dimension overlay, the glyphs and the model tree - five independent runs over the same sketch.
/// Now there is one source.
pub fn sketch_diag(cache: &Caches, project: &Project, si: usize) -> SketchDiag {
    let (shape, place) = (sketch_shape_key(project, si), sketch_place_key(project, si));
    if let Some(st) = &*cache.sk_status.borrow() {
        let dragged = cache.sk_dragged.get() == Some(si);
        if st.si == si && st.shape == shape && (dragged || st.place == place) {
            return st.diag.clone();
        }
    }
    // the degrees of freedom, the free points and the redundant from one elimination a part (`sketch_checks`)
    let checks = project.sketch_checks(si);
    let d = SketchDiag { dof: checks.dof, free: checks.free, conflicts: project.sketch_conflicts(si).into_iter().collect(), redundant: checks.redundant.into_iter().collect() };
    *cache.sk_status.borrow_mut() = Some(SketchStatus { si, shape, place, diag: d.clone() });
    d
}

/// WHICH REDUNDANT CONSTRAINTS TO MARK - one place for the list of constraints AND for the glyphs on the
/// canvas.
///
/// The rule used to be written down only in the list, while the glyphs on the canvas were painted from a
/// raw `diag.redundant`. The divergence looked like this: the slot is clean in the list, while orange
/// "redundant constraint" glyphs burn on its tangencies in the sketch. It was caught on a picture for the
/// help - on screen it had been visible for years to anyone who drew a slot.
///
/// WHAT IS EXCLUDED AND WHY:
/// * **dimensions** - consistent redundancy among dimensions is harmless (a reference dimension is not an
///   error);
/// * **tangencies** (`Tangent`/`CircleTangent`) - their Jacobian at the point of contact is PARALLEL to
///   the intrinsic of the arc, both measure the radius in one direction, and the rank analysis marks them
///   falsely. A tangency is the structural constraint of a fillet, a slot, a rounded rectangle;
///   professional CAD does not mark them red;
/// * **a virtual corner** - a `PointOnLine` on a point that belongs to no entity (the sharp corner under
///   a fillet, kept for the dimensions): the same lie of the rank;
/// * **everything geometric while the sketch contains tangencies** - the rank lies not only about the
///   tangencies themselves but about the constraints entangled with them (the horizontals and verticals
///   of a rectangle).
///
/// This hides no real contradictions: those are caught by `sketch_conflicts`, which counts by the
/// geometry.
pub fn flagged_redundant(cache: &Caches, project: &qymcad_core::model::Project, si: usize) -> std::collections::HashSet<usize> {
    use qymcad_core::model::Constraint;
    let diag = sketch_diag(cache, project, si);
    let Some(s) = project.sketches.get(si) else { return Default::default() };
    let tangency = |c: &Constraint| matches!(c, Constraint::Tangent { .. } | Constraint::CircleTangent { .. });
    // nothing redundant, nothing to mark - before the points of every entity are gathered: 7 ms a call on 70 000
    // segments, called by the list and by the canvas every frame
    if diag.redundant.is_empty() || s.constraints.iter().any(tangency) {
        return Default::default();
    }
    let entity_pts: std::collections::HashSet<Id> = s
        .entities
        .iter()
        .flat_map(|e| match e.kind {
            qymcad_core::model::EntityKind::Line { a, b } => vec![a, b],
            qymcad_core::model::EntityKind::Arc { center, a, b, .. } => vec![center, a, b],
            qymcad_core::model::EntityKind::Circle { center, .. } => vec![center],
            qymcad_core::model::EntityKind::Ellipse { c, ma, mi } => vec![c, ma, mi],
        })
        .collect();
    diag.redundant
        .iter()
        .copied()
        .filter(|ci| {
            let Some(c) = s.constraints.get(*ci) else { return false };
            let is_dim = matches!(
                c,
                Constraint::Distance { .. }
                    | Constraint::Angle { .. }
                    | Constraint::DistancePL { .. }
                    | Constraint::AngleLines { .. }
                    | Constraint::ArcLength { .. }
                    | Constraint::Diameter { .. }
                    | Constraint::EdgeDistance { .. }
            );
            let virtual_corner = matches!(c, Constraint::PointOnLine { p, .. } if !entity_pts.contains(p));
            !is_dim && !tangency(c) && !virtual_corner
        })
        .collect()
}

/// A QUICK HASH FOR THE KEYS OF THE STATUS CACHE: each word turned and multiplied in. The keys are taken several times
/// a frame (the panel, the points, the glyphs, the pick, the dimensions), over every point, entity and constraint of
/// the sketch; the hasher of a map, built to stand against an attacker, took most of a frame on 70 000 segments. A
/// key of a cache needs no such standing.
#[derive(Default)]
pub(crate) struct Mix(pub(crate) u64);

impl std::hash::Hasher for Mix {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write_u64(&mut self, x: u64) {
        self.0 = (self.0.rotate_left(5) ^ x).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }

    fn write(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(8) {
            let mut word = [0u8; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            self.write_u64(u64::from_le_bytes(word));
        }
    }
}

/// THE SHAPE OF A SKETCH, for the status cache: its points, entities, splines and constraints by what they are and
/// what they tie, and the values of its dimensions - everything the diagnostics depend on but where the points stand.
/// O(n) per frame is pennies against the Jacobian.
pub fn sketch_shape_key(project: &qymcad_core::model::Project, si: usize) -> u64 {
    use qymcad_core::model::EntityKind;
    use std::hash::{Hash, Hasher};
    let mut h = Mix::default();
    if let Some(s) = project.sketches.get(si) {
        for p in &s.points {
            p.id.hash(&mut h);
        }
        for e in &s.entities {
            e.id.hash(&mut h);
            e.construction.hash(&mut h);
            match e.kind {
                EntityKind::Line { a, b } => [0, a, b, 0].hash(&mut h),
                EntityKind::Arc { center, a, b, ccw } => [1, center, a, b, ccw as u64].hash(&mut h),
                EntityKind::Circle { center, .. } => [2, center].hash(&mut h),
                EntityKind::Ellipse { c, ma, mi } => [3, c, ma, mi].hash(&mut h),
            }
        }
        s.splines.len().hash(&mut h);
        for c in &s.constraints {
            std::mem::discriminant(c).hash(&mut h);
            c.points().hash(&mut h);
            c.is_driven().hash(&mut h);
            // the values of the dimensions (a number edited without the points moving - before solving)
            if let Some(d) = c.dim_value() {
                d.to_bits().hash(&mut h);
            }
        }
    }
    h.finish()
}

/// WHERE THE POINTS OF A SKETCH STAND AND HOW BIG ITS CIRCLES ARE, for the status cache: any move of a point or a
/// rim changes it.
pub fn sketch_place_key(project: &qymcad_core::model::Project, si: usize) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = Mix::default();
    if let Some(s) = project.sketches.get(si) {
        for p in &s.points {
            p.x.to_bits().hash(&mut h);
            p.y.to_bits().hash(&mut h);
        }
        for e in &s.entities {
            if let qymcad_core::model::EntityKind::Circle { r, .. } = e.kind {
                r.to_bits().hash(&mut h);
            }
        }
    }
    h.finish()
}

/// Whether a sketch (by id) is shown in the visibility context.
pub fn sketch_in_ctx(active_path: &[Id], project: &qymcad_core::model::Project, sketch_ses: SketchSession, sketch_id: Id) -> bool {
    // NESTED ONES BELONG. The rule used to be strict equality, and in the root of an assembly EVERY
    // sketch counted as someone else's: they belong to the parts, not to the root. Because of that the
    // contours tick box in an assembly showed nothing at all - reported as there being no contours in 3D
    // however much they were clicked.
    //
    // Bodies count differently (`component_is_within`, see `body_shown`), which is why the bodies of
    // nested parts are visible in an assembly. Sketches must follow the same rule: one's own means the
    // context AND its descendants. Neighbours (another branch of the tree) stay foreign, and the
    // isolation does not suffer from this.
    let ctx = viz_ctx_id(active_path, project, sketch_ses);
    match project.sketch_owner(sketch_id) {
        Some(owner) => owner == ctx || project.component_is_within(owner, ctx),
        None => false,
    }
}
