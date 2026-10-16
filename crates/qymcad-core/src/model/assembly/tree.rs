//! THE TREE OF AN ASSEMBLY: colours and scales of bodies, the root and the context, imported and exported trees.

use super::*;

impl Project {
    /// Colour of a part by mesh index (RGB), keyed by lineage root: a manual colour from `part_colors`, or the
    /// palette entry for the stable index of that root, so the shade does not change with every operation.
    pub fn mesh_color(&self, index: usize) -> [u8; 3] {
        match self.mesh_id(index) {
            Some(body) => {
                let root = self.lineage_root(body);
                self.part_colors.get(&root).copied().unwrap_or_else(|| default_part_color(self.part_color_index(root)))
            }
            None => default_part_color(index),
        }
    }
    /// Set a manual part colour, keyed by lineage root, so it applies to the whole feature chain of that
    /// part.
    pub fn set_mesh_color(&mut self, index: usize, rgb: [u8; 3]) {
        if let Some(body) = self.mesh_id(index) {
            let root = self.lineage_root(body);
            self.part_colors.insert(root, rgb);
        }
    }
    /// The colour a file gave face `face` of `body` (by the face's persistent id), where it gave one: the face then
    /// shows it instead of the part's colour.
    pub fn face_color(&self, body: Id, face: u32) -> Option<[u8; 3]> {
        let root = self.lineage_root(body);
        self.face_colors.get(&root).and_then(|fs| fs.iter().find(|(id, _)| *id == face).map(|(_, c)| *c))
    }
    /// Clear a manual part colour and fall back to the palette entry for its lineage root.
    pub fn reset_mesh_color(&mut self, index: usize) {
        if let Some(body) = self.mesh_id(index) {
            let root = self.lineage_root(body);
            self.part_colors.remove(&root);
        }
    }
    /// The factor an import - a solid or a piece of a mesh - stands at from the file's own numbers; `None` where `body`
    /// came from no file.
    pub fn import_scale(&self, body: Id) -> Option<f64> {
        use crate::feature::FeatureKind;
        self.timeline.iter().find_map(|n| match n.kind {
            FeatureKind::Import { body: b, scale, .. } | FeatureKind::MeshPiece { body: b, scale, .. } if b == body => Some(scale),
            _ => None,
        })
    }
    /// Take an import at `factor` from the file's own numbers. The node keeps it: a solid is built at this size whenever
    /// it is read back from its source, and is marked for it; a mesh piece's mesh is its geometry, scaled already by
    /// the caller, so its factor is only remembered. Returns whether `body` is an import.
    pub fn set_import_scale(&mut self, body: Id, factor: f64) -> bool {
        use crate::feature::FeatureKind;
        let Some(n) = self.timeline.iter_mut().find(|n| matches!(n.kind, FeatureKind::Import { body: b, .. } | FeatureKind::MeshPiece { body: b, .. } if b == body)) else { return false };
        match &mut n.kind {
            FeatureKind::Import { scale, .. } => {
                *scale = factor;
                n.dirty = true;
            }
            FeatureKind::MeshPiece { scale, .. } => {
                // A PIECE OF A MESH IS ITS GEOMETRY: rescaled here, in the document, its mesh and the faces found on it
                // together, from the factor it stood at - nothing is found on it again
                let by = factor / *scale;
                *scale = factor;
                if let Some(mi) = self.mesh_index(body) {
                    self.bodies[mi].mesh.scale(0.0, 0.0, 0.0, by);
                    self.bodies[mi].faces = self.bodies[mi].faces.iter().map(|f| f.scaled(by)).collect();
                }
            }
            _ => {}
        }
        true
    }

    /// A BODY MADE OF A MESH, with no B-rep anywhere in its making: a piece of a mesh, or a move of one.
    pub fn mesh_born(&self, body: Id) -> bool {
        use crate::feature::FeatureKind;
        let mut b = body;
        for _ in 0..=self.timeline.len() {
            match self.timeline.iter().find(|n| n.kind.body() == Some(b)).map(|n| &n.kind) {
                Some(FeatureKind::MeshPiece { .. }) => return true,
                Some(FeatureKind::Move { src, .. }) => b = *src,
                _ => return false,
            }
        }
        false
    }
    /// Root assembly component of the document, created lazily. Returns its id.
    pub fn ensure_root(&mut self) -> Id {
        use crate::feature::{Component, ComponentKind, PLACE_IDENTITY};
        if self.root != 0 && self.components.iter().any(|c| c.id == self.root) {
            return self.root;
        }
        // A root component (`parent == None`) may already exist in the document.
        if let Some(c) = self.components.iter().find(|c| c.parent.is_none()) {
            self.root = c.id;
            return self.root;
        }
        let id = self.alloc_id();
        self.components.push(Component { id, name: "name-assembly".into(), kind: ComponentKind::Assembly, parent: None, transform: PLACE_IDENTITY, visible: true, grounded: false });
        self.root = id;
        id
    }
    /// Active context for creating nodes: `active_component`, or the root. It creates the root lazily, so the
    /// very first `add_*` is guaranteed to nest inside a real component rather than under id 0.
    pub fn active_ctx(&mut self) -> Id {
        self.ensure_root();
        self.active_component.filter(|id| *id != 0).unwrap_or(self.root)
    }
    /// Active context for queries taking `&self`: it does not create the root, and yields 0 when there is
    /// none yet.
    pub fn current_ctx(&self) -> Id {
        self.active_component.filter(|id| *id != 0).unwrap_or(self.root)
    }
    /// Add a component of the given kind to the active context. Returns its id.
    pub fn add_component_kind(&mut self, name: impl Into<String>, kind: crate::feature::ComponentKind) -> Id {
        use crate::feature::{Component, PLACE_IDENTITY};
        self.ensure_root();
        let id = self.alloc_id();
        // A part is never placed inside a part. A part is a body with a build history; a component inside it
        // would give a structure that does not exist in CAD: one tree row holding both nested parts and its own
        // construction. Attaching the component to the active context silently lets such a structure reach the
        // file. Instead the new component is lifted to the nearest assembly, which is what "new part" means
        // while standing inside a part.
        let mut ctx = self.active_ctx();
        while self.components.iter().any(|c| c.id == ctx && matches!(c.kind, crate::feature::ComponentKind::Part)) {
            match self.components.iter().find(|c| c.id == ctx).and_then(|c| c.parent) {
                Some(up) => ctx = up,
                None => break,
            }
        }
        let parent = Some(ctx);
        self.components.push(Component { id, name: name.into(), kind, parent, transform: PLACE_IDENTITY, visible: true, grounded: false });
        id
    }

    /// A NAME NO OTHER COMPONENT CARRIES: `name` itself, or `name (2)`, `name (3)`... - the way the professional systems
    /// number the instances of one part. Two rows reading "Part 1" in the tree, with nothing to tell them apart, came of
    /// bringing a part in from a file, pasting one and importing a format that names its parts. `except` is the
    /// component being named itself, which does not count against its own name.
    pub fn free_component_name(&self, name: &str, except: Id) -> String {
        self.free_component_name_shown(name, except, &|n| n.to_string())
    }

    /// As [`Project::free_component_name`], two names being the same when they READ the same: a part of this document
    /// is stored under a key of the catalogue (`name-part-n#1`) and one from a file under the words ("Part 1"), and it
    /// is the words a person tells rows apart by. `shown` turns a stored name into what is read.
    pub fn free_component_name_shown(&self, name: &str, except: Id, shown: &dyn Fn(&str) -> String) -> String {
        let taken = |n: &str| {
            let read = shown(n);
            self.components.iter().any(|c| c.id != except && self.instance_origin(c.id) == c.id && shown(&c.name) == read)
        };
        if !taken(name) {
            return name.to_string();
        }
        (2..).map(|k| format!("{name} ({k})")).find(|n| !taken(n)).unwrap_or_else(|| name.to_string())
    }

    /// NAME THE COMPONENTS `ids` APART from every other one that reads the same (see `free_component_name_shown`):
    /// what came in from a file, a library or the clipboard.
    ///
    /// A CLONE KEEPS THE NAME OF ITS ORIGINAL - it is marked as one, and that is what tells it apart - so clones are
    /// neither renamed nor counted against.
    pub fn name_apart(&mut self, ids: &[Id], shown: &dyn Fn(&str) -> String) {
        for &id in ids {
            if self.instance_origin(id) != id {
                continue;
            }
            if let Some(ci) = self.components.iter().position(|c| c.id == id) {
                let free = self.free_component_name_shown(&self.components[ci].name.clone(), id, shown);
                self.components[ci].name = free;
            }
        }
    }
    /// Import a file's tree as its author built it: every node with children becomes a subassembly under the
    /// file's name for it, every node with a body a part holding an `Import` node of that body, and every one
    /// stands where the file places it in its parent - the bodies stay in their own coordinates, so a part can be
    /// moved and mated. A group with nothing in it does not come in. Several nodes at the top go into a
    /// subassembly named `group_name`; a single one stands alone. Lands in the active context and returns the
    /// created root. One method, because this is a topology operation.
    pub fn import_tree_as_parts(&mut self, nodes: Vec<ImportNode>, source: Id, group_name: &str) -> Option<Id> {
        let mut nodes: Vec<ImportNode> = nodes.into_iter().filter(ImportNode::carries_something).collect();
        if nodes.is_empty() {
            return None;
        }
        self.ensure_root();
        let saved = self.active_component;
        let mut parts = std::collections::HashMap::new(); // the part each body of the file came in as
        let created = if nodes.len() == 1 {
            nodes.pop().and_then(|n| self.import_node(n, source, &mut parts))
        } else {
            let asm = self.add_assembly(group_name);
            for n in nodes {
                self.set_active_component(Some(asm));
                self.import_node(n, source, &mut parts);
            }
            Some(asm)
        };
        self.set_active_component(saved);
        created
    }

    /// One node of an imported tree into the active context (see `import_tree_as_parts`). `parts` holds the part each
    /// body of the file came in as, so a repeat of it comes in as a clone of that part.
    pub(crate) fn import_node(&mut self, node: ImportNode, source: Id, parts: &mut std::collections::HashMap<u32, Id>) -> Option<Id> {
        use crate::feature::{FeatureKind, FeatureNode};
        let here = self.active_component;
        let created = match node.body {
            None if node.children.is_empty() => {
                // a repeat of a product met before: a clone of its part; the body it repeats came in earlier, the
                // file being walked in its own order
                let of = node.repeat_of.and_then(|k| parts.get(&k).copied())?;
                let clone = self.clone_part(of, self.current_ctx())?;
                self.rename_component(clone, node.name.clone());
                let body = self.timeline.iter().find(|n| n.parent == Some(clone)).and_then(|n| n.kind.body());
                if let (Some(rgb), Some(body)) = (node.color, body) {
                    let root = self.lineage_root(body); // the clone's body has no mesh until it is built
                    self.part_colors.insert(root, rgb);
                }
                // the faces of a colour of their own are the original's: the file holds the product once
                let original = self.timeline.iter().find(|n| n.parent == Some(of)).and_then(|n| n.kind.body()).map(|b| self.lineage_root(b));
                if let (Some(body), Some(original)) = (body, original) {
                    let root = self.lineage_root(body);
                    if let Some(faces) = self.face_colors.get(&original).filter(|_| original != root).cloned() {
                        self.face_colors.insert(root, faces);
                    }
                }
                clone
            }
            Some(body) if node.children.is_empty() => {
                let part = self.add_part(node.name);
                // a piece of a mesh is not an imported solid: it has no B-rep to raise again or to export as one
                let (name, kind) = if node.mesh {
                    ("name-mesh-piece", FeatureKind::MeshPiece { body, source, piece: node.solid, scale: 1.0 })
                } else {
                    ("name-import", FeatureKind::Import { body, source, solid: node.solid, scale: 1.0 })
                };
                // a mesh piece arrives built: its mesh and faces are already there, and it has no recipe to rebuild. A
                // dirty one started a rebuild of nothing that ran past Esc and overwrote "import cancelled".
                self.push_timeline(FeatureNode { id: body, name: name.into(), kind, parent: Some(part), dirty: !node.mesh, suppressed: false });
                if let (Some(rgb), Some(i)) = (node.color, self.mesh_index(body)) {
                    self.set_mesh_color(i, rgb); // the author's colour, kept by the part's lineage like one chosen by hand
                }
                if !node.face_colors.is_empty() {
                    let root = self.lineage_root(body);
                    self.face_colors.insert(root, node.face_colors.clone());
                }
                if !node.tri_colors.is_empty() {
                    // a piece of a mesh coloured triangle by triangle: its palette, and each triangle's place in it
                    let root = self.lineage_root(body);
                    self.tri_colors.insert(root, crate::model::palette_of(&node.tri_colors));
                }
                parts.insert(node.solid, part);
                part
            }
            _ => {
                let asm = self.add_assembly(node.name.clone());
                // a node that carries a body AND children: the body is a part of its own inside the group
                let own = node.body.map(|body| ImportNode {
                    name: node.name.clone(),
                    body: Some(body),
                    solid: node.solid,
                    color: node.color,
                    face_colors: node.face_colors.clone(),
                    tri_colors: node.tri_colors.clone(),
                    mesh: node.mesh,
                    ..Default::default()
                });
                for child in own.into_iter().chain(node.children.into_iter().filter(ImportNode::carries_something)) {
                    self.set_active_component(Some(asm));
                    self.import_node(child, source, parts);
                }
                asm
            }
        };
        self.set_component_transform(created, node.place);
        self.set_active_component(here);
        Some(created)
    }
    /// A CLONE OF PART `src` IN ASSEMBLY `target`: the same part once more, not a copy of it. Its one node repeats the
    /// original's body (`PartInstance`), so an edit of the original rebuilds it, while it stands, moves and is mated
    /// as a part of its own. A clone of a clone repeats the original. It comes in under the original's name and
    /// stands where the original stands in its parent. Only a part is cloned, and only into an assembly. One method,
    /// because this is a topology operation.
    pub fn clone_part(&mut self, src: Id, target: Id) -> Option<Id> {
        use crate::feature::{ComponentKind, FeatureKind, FeatureNode};
        let origin = self.instance_origin(src);
        if self.component_kind(origin) != Some(ComponentKind::Part) || self.component_kind(target) != Some(ComponentKind::Assembly) {
            return None;
        }
        let name = self.components.iter().find(|c| c.id == src).map(|c| c.name.clone())?;
        let place = self.free_place_beside(src, target);
        let saved = self.active_component;
        self.set_active_component(Some(target));
        let comp = self.add_part(name);
        self.set_active_component(saved);
        self.set_component_transform(comp, place);
        let body = self.alloc_id();
        self.push_timeline(FeatureNode { id: body, name: "name-instance".into(), kind: FeatureKind::PartInstance { src_comp: origin, body }, parent: Some(comp), dirty: true, suppressed: false });
        Some(comp)
    }
    /// WHERE A CLONE OF `src` STANDS IN `target`: beside the original along X, in the first place where its box meets
    /// no body of the assembly, so it is seen and taken apart from the original and does not sink into a neighbour.
    /// Reported behaviour: a clone stood exactly where its original stood, and a click on it took the original. The
    /// gap is a fifth of the original's length along X, 5 mm at least; a part with no body yet keeps the original's
    /// place, there being nothing to measure.
    pub(crate) fn free_place_beside(&self, src: Id, target: Id) -> [f64; 12] {
        use crate::feature::apply12;
        let consumed = self.consumed_bodies();
        // the box of a body in the frame of `target`, from the eight corners of its own box
        let boxed = |b: &crate::model::Body| -> Option<([f64; 3], [f64; 3])> {
            let bb = b.mesh.bounds()?;
            let m = self.body_display_transform(b.id, target);
            let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
            for k in 0..8 {
                let c = [if k & 1 == 0 { bb.min.x } else { bb.max.x }, if k & 2 == 0 { bb.min.y } else { bb.max.y }, if k & 4 == 0 { bb.min.z } else { bb.max.z }];
                let w = apply12(&m, c);
                for i in 0..3 {
                    lo[i] = lo[i].min(w[i]);
                    hi[i] = hi[i].max(w[i]);
                }
            }
            Some((lo, hi))
        };
        let live = |b: &&crate::model::Body| !consumed.contains(&b.id) && self.body_owner(b.id).is_some();
        let own: Vec<([f64; 3], [f64; 3])> = self.bodies.iter().filter(live).filter(|b| self.body_owner(b.id) == Some(src)).filter_map(boxed).collect();
        let mut place = self.relative_transform(src, target);
        let Some((lo, hi)) = own.into_iter().reduce(|a, b| ([a.0[0].min(b.0[0]), a.0[1].min(b.0[1]), a.0[2].min(b.0[2])], [a.1[0].max(b.1[0]), a.1[1].max(b.1[1]), a.1[2].max(b.1[2])])) else {
            return place;
        };
        let others: Vec<([f64; 3], [f64; 3])> = self.bodies.iter().filter(live).filter_map(boxed).collect();
        let len = hi[0] - lo[0];
        let step = len + (len / 5.0).max(5.0);
        let meets = |dx: f64| {
            others.iter().any(|(a, b)| {
                (0..3).all(|i| {
                    let (l, h) = if i == 0 { (lo[0] + dx, hi[0] + dx) } else { (lo[i], hi[i]) };
                    l < b[i] - 1e-6 && a[i] < h - 1e-6
                })
            })
        };
        let dx = (1..=64).map(|k| k as f64 * step).find(|dx| !meets(*dx)).unwrap_or(64.0 * step);
        place[3] += dx;
        place
    }

    /// WHOSE PART THIS IS: the original a clone - or a pattern's copy - repeats, or the part itself. A part repeats
    /// another when its body is an instance of the other's.
    pub fn instance_origin(&self, id: Id) -> Id {
        use crate::feature::FeatureKind;
        let mut cur = id;
        // a guard against a loop of instances in a damaged document, not a depth anyone reaches
        for _ in 0..64 {
            match self.timeline.iter().find_map(|n| match &n.kind {
                FeatureKind::PartInstance { src_comp, .. } if n.parent == Some(cur) => Some(*src_comp),
                FeatureKind::ComponentPattern { src, copies, .. } if copies.contains(&cur) => Some(*src),
                _ => None,
            }) {
                Some(src) if src != cur => cur = src,
                _ => break,
            }
        }
        cur
    }
    /// THE TREE UNDER `root` AS IT GOES OUT to an exact file: every subassembly and part, where it stands in its
    /// parent, a part with its body and colour. A clone of a part already gone out repeats it (`same_as`), so the file
    /// holds the product once and places it twice. A part whose body `has_shape` does not answer for - a mesh, a
    /// failed rebuild - does not go out, nor a subassembly left empty by it. The root stands at the file's zero.
    pub fn export_tree(&self, root: Id, has_shape: impl Fn(Id) -> bool) -> Vec<ExportNode> {
        let mut out = Vec::new();
        let mut written = std::collections::HashMap::new();
        self.export_node(root, None, crate::feature::PLACE_IDENTITY, &has_shape, &mut out, &mut written);
        out
    }

    /// One component of `export_tree` and what is under it; returns whether anything went out. `written` holds the
    /// node every part went out as, so a clone of it repeats it.
    pub(crate) fn export_node(
        &self,
        id: Id,
        parent: Option<usize>,
        place: [f64; 12],
        has_shape: &impl Fn(Id) -> bool,
        out: &mut Vec<ExportNode>,
        written: &mut std::collections::HashMap<Id, usize>,
    ) -> bool {
        use crate::feature::ComponentKind;
        let Some(c) = self.components.iter().find(|c| c.id == id) else { return false };
        let at = out.len();
        if c.kind == ComponentKind::Part {
            let origin = self.instance_origin(id);
            if let Some(&k) = written.get(&origin).filter(|_| origin != id) {
                out.push(ExportNode { name: c.name.clone(), parent, place, body: None, same_as: Some(k), color: None, face_colors: Vec::new() });
                return true;
            }
            let Some(body) = self.active_body(id).filter(|b| has_shape(*b)) else { return false };
            let color = self.mesh_index(body).map(|i| self.mesh_color(i));
            let face_colors = self.face_colors.get(&self.lineage_root(body)).cloned().unwrap_or_default();
            out.push(ExportNode { name: c.name.clone(), parent, place, body: Some(body), same_as: None, color, face_colors });
            if origin == id {
                written.insert(id, at);
            }
            return true;
        }
        out.push(ExportNode { name: c.name.clone(), parent, place, body: None, same_as: None, color: None, face_colors: Vec::new() });
        let mut any = false;
        for child in self.component_children(id) {
            let t = self.component_transform(child);
            any |= self.export_node(child, Some(at), t, has_shape, out, written);
        }
        if !any {
            out.truncate(at);
        }
        any
    }
}
