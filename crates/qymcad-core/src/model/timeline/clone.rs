//! A SUBTREE COPIED into another document, with its names.

use super::*;

impl Project {
    /// The single engine for deep-cloning the subtree of component `id` from source project `from` into project
    /// `dest` under `target_parent`.
    ///
    /// It serves both a clone within one project (`clone_component`, with `from` a snapshot of self) and a clone
    /// between projects (`graft` for inserting a product, `subproject_of` for extracting one). Every output id —
    /// components, sketches, planes, points, axes, bodies, nodes and `feat_dims` — is remapped through
    /// `dest.alloc_id` and therefore cannot collide with the ids of the host. References leading outside the
    /// subtree are cut, as they are on removal. Bodies are marked dirty and are rebuilt by `regenerate(kernel)`.
    /// Returns the id of the cloned root in `dest`.
    pub(crate) fn clone_subtree_into(dest: &mut Project, from: &Project, id: Id, target_parent: Id) -> Option<Id> {
        use crate::feature::{Component, FeatureKind, FeatureNode};
        use std::collections::HashMap;
        if id == from.root || !from.components.iter().any(|c| c.id == id) || !dest.components.iter().any(|c| c.id == target_parent) {
            return None;
        }
        if dest.component_is_part(target_parent) {
            return None; // A part inside a part is not allowed: a component goes into an assembly only.
        }
        // The component subtree: the component itself plus its descendants.
        let mut subtree: Vec<Id> = from.descendants(id);
        subtree.insert(0, id);
        let sub_set: std::collections::HashSet<Id> = subtree.iter().copied().collect();

        // Id map: the components plus every output of the timeline nodes of the subtree (sketches, planes,
        // points, axes, bodies).
        let mut map: HashMap<Id, Id> = HashMap::new();
        for &c in &subtree {
            map.insert(c, dest.alloc_id());
        }
        let sub_nodes: Vec<FeatureNode> = from.timeline.iter().filter(|n| n.parent.is_some_and(|p| sub_set.contains(&p))).cloned().collect();
        for n in &sub_nodes {
            for out in from.node_output_ids(&n.kind) {
                map.entry(out).or_insert_with(|| dest.alloc_id());
            }
            map.entry(n.id).or_insert_with(|| dest.alloc_id());
        }

        // Clone the components; a parent inside the subtree maps to its clone and the subtree root maps to
        // `target_parent`.
        for &c in &subtree {
            let Some(src) = from.components.iter().find(|x| x.id == c).cloned() else { continue };
            let parent = if c == id { Some(target_parent) } else { src.parent.and_then(|p| map.get(&p).copied()) };
            dest.components.push(Component { id: map[&c], name: src.name.clone(), kind: src.kind, parent, transform: src.transform, visible: src.visible, grounded: src.grounded });
        }

        // Topological name map: a cloned feature has a different id, so its faces carry different names.
        //
        // Measured on a thread: a copied part went red with "thread crest not found" and drifted, because
        // `Thread.edge` in the copy still pointed at an edge of the original. Ids have been remapped for a long
        // time, but a face name is derived from the recipe (which feature, in what role, from which entity)
        // rather than from an id, and on a copy that recipe reads differently. Shell, draft, fillet and a hole
        // on a face hold references of the same kind.
        //
        // The names depend on each other: an edge is named by its pair of faces and a fillet by its edge. The
        // map is therefore built in passes to a fixed point rather than in one list: the order in the table
        // guarantees nothing, and a missed name would silently leave a reference to somebody else's
        // geometry.
        let mut nmap: HashMap<u32, u32> = HashMap::new();
        loop {
            let mut progress = false;
            for (i, gn) in from.names.faces().iter().enumerate() {
                let desc = crate::names::NAMED | i as u32;
                if nmap.contains_key(&desc) {
                    continue;
                }
                // A name belonging to a feature outside the subtree is not this map's business: an outward
                // reference is cut anyway.
                let Some(&nfeat) = map.get(&gn.feature) else { continue };
                // `src` comes in two kinds: a sketch entity, whose id is the same in the clone, and the name of
                // another face or edge (a fillet, a corner patch, a pattern image), which waits for its own
                // mapping.
                let nsrc = if gn.src <= u32::MAX as Id && crate::names::NameTable::is_named(gn.src as u32) {
                    match nmap.get(&(gn.src as u32)) {
                        Some(&v) => v as Id,
                        None => continue,
                    }
                } else {
                    gn.src
                };
                let nd = dest.names.intern_face(crate::names::GeoName { feature: nfeat, role: gn.role, src: nsrc, split: gn.split });
                nmap.insert(desc, nd);
                progress = true;
            }
            for (i, en) in from.names.edges().iter().enumerate() {
                let desc = crate::names::EDGE | i as u32;
                if nmap.contains_key(&desc) {
                    continue;
                }
                let (Some(&a), Some(&b)) = (nmap.get(&en.faces[0]), nmap.get(&en.faces[1])) else { continue };
                let nd = dest.names.intern_edge(crate::names::EdgeName::new(a, b, en.index));
                nmap.insert(desc, nd);
                progress = true;
            }
            if !progress {
                break;
            }
        }

        // Clone the sketches (remapping `source` and `plane`) and build the contour map used by `profile`.
        let mut cmap: HashMap<Id, Id> = HashMap::new();
        for n in &sub_nodes {
            if let FeatureKind::Sketch { sketch } = n.kind {
                if let Some(si) = from.sketch_index(sketch) {
                    let old_cids = from.sketches[si].contour_ids.clone();
                    let mut sk = from.sketches[si].clone();
                    sk.id = map[&sketch];
                    sk.contour_ids.clear();
                    if let Some(src) = sk.source {
                        sk.source = Some(map.get(&src).copied().unwrap_or(src));
                    }
                    sk.plane = match sk.plane {
                        crate::feature::SketchPlane::Datum(p) => crate::feature::SketchPlane::Datum(map.get(&p).copied().unwrap_or(p)),
                        // The anchor face is addressed by name rather than by id, and on a copy that name
                        // differs (see `nmap`).
                        crate::feature::SketchPlane::Face(b, mut f) => {
                            if let Some(&nd) = nmap.get(&f.id) {
                                f.id = nd;
                            }
                            crate::feature::SketchPlane::Face(map.get(&b).copied().unwrap_or(b), f)
                        }
                        other => other,
                    };
                    dest.sketches.push(sk);
                    let ni = dest.sketches.len() - 1;
                    dest.regen_sketch(ni);
                    let new_cids = dest.sketches[ni].contour_ids.clone();
                    for (k, oc) in old_cids.iter().enumerate() {
                        if let Some(nc) = new_cids.get(k) {
                            cmap.insert(*oc, *nc);
                        }
                    }
                }
            }
        }

        // Clone the datum planes, remapping their definitions.
        for n in &sub_nodes {
            if let FeatureKind::Plane { plane } = n.kind {
                if let Some(pl) = from.planes.iter().find(|p| p.id == plane).cloned() {
                    let mut np = pl;
                    np.id = map[&plane];
                    np.def = match np.def {
                        // The anchor face goes by name, which differs for a cloned feature.
                        PlaneDef::OffsetFace { body, mut face, dist } => {
                            if let Some(&nd) = nmap.get(&face.id) {
                                face.id = nd;
                            }
                            PlaneDef::OffsetFace { body: map.get(&body).copied().unwrap_or(body), face, dist }
                        }
                        PlaneDef::OffsetPlane { plane, dist } => PlaneDef::OffsetPlane { plane: map.get(&plane).copied().unwrap_or(plane), dist },
                        other => other,
                    };
                    dest.planes.push(np);
                }
            }
        }
        // Clone the datum points.
        for n in &sub_nodes {
            if let FeatureKind::DatumPoint { point } = n.kind {
                if let Some(dp) = from.datum_points.iter().find(|p| p.id == point).cloned() {
                    dest.datum_points.push(DatumPoint { id: map[&point], ..dp });
                }
            }
        }
        // Clone the datum axes, remapping their definitions.
        for n in &sub_nodes {
            if let FeatureKind::DatumAxis { axis } = n.kind {
                if let Some(da) = from.datum_axes.iter().find(|a| a.id == axis).cloned() {
                    let mut na = da;
                    na.id = map[&axis];
                    na.def = match na.def {
                        AxisDef::TwoPoints { a, b } => AxisDef::TwoPoints { a: map.get(&a).copied().unwrap_or(a), b: map.get(&b).copied().unwrap_or(b) },
                        AxisDef::FromEdge { body, edge } => AxisDef::FromEdge { body: map.get(&body).copied().unwrap_or(body), edge: nmap.get(&edge).copied().unwrap_or(edge) },
                        AxisDef::FromFace { body, face } => AxisDef::FromFace { body: map.get(&body).copied().unwrap_or(body), face: nmap.get(&face).copied().unwrap_or(face) },
                        other => other,
                    };
                    dest.datum_axes.push(na);
                }
            }
        }

        // Clone the timeline nodes in their original order, remapping the references and the profiles and
        // marking the bodies dirty.
        let base = dest.timeline.len();
        for (offset, n) in sub_nodes.iter().enumerate() {
            let mut kind = n.kind.clone();
            kind.remap_ids(&map);
            kind.remap_names(&nmap);
            kind.remap_profile(&cmap);
            let parent = n.parent.and_then(|p| map.get(&p).copied());
            let dirty = kind.body().is_some();
            dest.timeline.insert(base + offset, FeatureNode { id: map[&n.id], name: n.name.clone(), kind, parent, dirty, suppressed: n.suppressed });
        }

        // Clone the parametric body dimensions (`feat_dims` under the new body ids).
        let dim_clones: Vec<(Id, std::collections::HashMap<String, String>)> = from.feat_dims.iter().filter_map(|(k, v)| map.get(k).map(|nk| (*nk, v.clone()))).collect();
        for (nk, v) in dim_clones {
            dest.feat_dims.insert(nk, v);
        }

        Some(map[&id])
    }

    /// Owning component of a sketch, by id, through its timeline node. `None` means the sketch has no node
    /// yet.
    pub fn sketch_owner(&self, sketch: Id) -> Option<Id> {
        self.timeline.iter().find(|n| matches!(n.kind, crate::feature::FeatureKind::Sketch { sketch: s } if s == sketch)).and_then(|n| n.parent)
    }

    /// Owning component of a datum plane, through its `Plane` node; `None` when it was not found.
    pub fn plane_owner(&self, plane: Id) -> Option<Id> {
        self.timeline.iter().find(|n| matches!(n.kind, crate::feature::FeatureKind::Plane { plane: p } if p == plane)).and_then(|n| n.parent)
    }
}
