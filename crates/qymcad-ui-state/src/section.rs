//! THE SECTION: its caps and the bodies it crosses.

use super::*;

/// THE SECTION CAPS: an exact planar cut of every visible body, in world coordinates, cached by (plane,
/// geom_rev, visible bodies).
pub fn section_caps_for_frame(pn: &Painting) -> std::rc::Rc<Vec<qymcad_core::geom::Mesh>> {
    let Some((o, n)) = section_eff(pn.section) else {
        return std::rc::Rc::new(Vec::new());
    };
    let key = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for v in o.iter().chain(n.iter()) {
            v.to_bits().hash(&mut h);
        }
        view_rev(pn.regen).hash(&mut h); // the section cuts IN THE WORLD, so it depends on the layout too
        for SceneMesh { index: mi, ghost, .. } in visible_mesh_items(pn) {
            (mi as u32, ghost).hash(&mut h);
        }
        h.finish()
    };
    {
        let c = pn.cache.section_caps.borrow();
        if c.rev == key {
            return c.value.clone();
        }
    }
    // THE CAP IS COMPUTED FROM THE MESH rather than by a kernel boolean. Reported behaviour: no amber cap
    // and hollow bodies inside - no CAD cuts that way. A Common boolean needed a live B-rep, which is not
    // there right after opening from a bundle, will never be there for an imported STL, and cost minutes on
    // a thousand bodies. A mesh is ALWAYS there and cuts orders of magnitude faster, so the cap is now drawn
    // during a gizmo drag as well - the section looks like a closed body at any moment.
    let mut caps: Vec<qymcad_core::geom::Mesh> = Vec::new();
    for SceneMesh { index: mi, ghost, world: wt, .. } in visible_mesh_items(pn) {
        if ghost {
            continue; // ghosts get no cap
        }
        // A CHEAP REJECT by extent - the plane cuts a handful of bodies out of a thousand
        if !mesh_crosses_plane(pn, mi, o, n) {
            continue;
        }
        let Some(mesh) = pn.project.bodies.get(mi).map(|b| &b.mesh) else { continue };
        // the body's mesh is in its local frame, so the plane is carried there too (wt is rigid: p_l = R^T (p - t))
        let r = |v: [f64; 3]| [wt[0] * v[0] + wt[4] * v[1] + wt[8] * v[2], wt[1] * v[0] + wt[5] * v[1] + wt[9] * v[2], wt[2] * v[0] + wt[6] * v[1] + wt[10] * v[2]];
        let po = [o[0] - wt[3], o[1] - wt[7], o[2] - wt[11]];
        let (ol, nl) = (r(po), r(n));
        let tris = qymcad_core::geom::mesh_section_cap(mesh, ol, nl);
        if tris.is_empty() {
            continue;
        }
        let mut cap = qymcad_core::geom::Mesh::default();
        for t in tris {
            let base = cap.verts.len() as u32;
            cap.verts.extend(t);
            cap.tris.push([base, base + 1, base + 2]);
        }
        if !qymcad_core::feature::is_identity12(&wt) {
            cap.transform(&wt); // the cap into world coordinates
        }
        caps.push(cap);
    }
    let rc = std::rc::Rc::new(caps);
    pn.cache.section_caps.borrow_mut().put(key, rc.clone());
    rc
}

/// Whether the PLANE crosses the world bounding box of mesh `mi`. A cheap rejection before the
/// expensive boolean for the section cap: a body lying entirely on one side has no cap by definition.
pub fn mesh_crosses_plane(pn: &Painting, mi: usize, o: [f64; 3], n: [f64; 3]) -> bool {
    let Some(b) = mesh_world_bounds(pn, mi) else { return true }; // the box is unknown, so assume it cuts
                                                                  // the box's support point along the normal: if the WHOLE box is on one side, there is no intersection
    let (mut lo, mut hi) = (0.0, 0.0);
    for k in 0..3 {
        let (a, b2) = ((b.min[k] - o[k]) * n[k], (b.max[k] - o[k]) * n[k]);
        lo += a.min(b2);
        hi += a.max(b2);
    }
    lo <= 0.0 && hi >= 0.0
}

/// The world bounding box of mesh `mi` (cached by `geom_rev` — computed once per change of the scene).
pub fn mesh_world_bounds(pn: &Painting, mi: usize) -> Option<WorldBox> {
    // the cache key accounts for VISIBILITY too (the map is built from the visible bodies): the key
    // used to be `geom_rev` alone, and a hidden body switched back on missed the cache.
    let key = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        // `geom_rev` IS IN ITS RIGHT PLACE HERE — and this is not the same thing as the document key.
        // A bounding box is derived from THE MESHES: rebuild a body and the old box lies, while a cache
        // miss is silent (the section cap simply does not get drawn). The drawing revision was taken out
        // of "is there anything unsaved" precisely because there it answered the wrong question; in a
        // picture cache it is exactly the right one.
        view_rev(pn.regen).hash(&mut h);
        for b in &pn.project.bodies {
            b.visible.hash(&mut h);
        }
        h.finish()
    };
    {
        let c = pn.cache.mesh_bounds.borrow();
        if c.rev == key {
            return c.value.get(&mi).copied();
        }
    }
    let mut map: std::collections::HashMap<usize, WorldBox> = std::collections::HashMap::new();
    for SceneMesh { index: i, world: wt, .. } in visible_mesh_items(pn) {
        let Some(m) = pn.project.bodies.get(i).map(|b| &b.mesh) else { continue };
        let Some(bb) = m.bounds() else { continue };
        // the 8 corners of the local box taken into world space (wt is rigid), giving a world AABB
        let (mut mn, mut mx) = ([f64::MAX; 3], [f64::MIN; 3]);
        for cx in [bb.min.x, bb.max.x] {
            for cy in [bb.min.y, bb.max.y] {
                for cz in [bb.min.z, bb.max.z] {
                    let p = qymcad_core::feature::apply12(&wt, [cx, cy, cz]);
                    for k in 0..3 {
                        mn[k] = mn[k].min(p[k]);
                        mx[k] = mx[k].max(p[k]);
                    }
                }
            }
        }
        map.insert(i, WorldBox { min: mn, max: mx });
    }
    let out = map.get(&mi).copied();
    pn.cache.mesh_bounds.borrow_mut().put(key, map);
    out
}
