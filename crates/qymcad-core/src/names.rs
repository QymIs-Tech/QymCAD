//! The name of a piece of geometry is a structure, not a number.
//!
//! A reference to a face or an edge has to survive an edit of a feature earlier in the timeline. As long as the
//! name is derived from the traversal order of the result, that is impossible in principle: add one edge to a
//! contour and everything after it is called something else. The name is therefore derived from the recipe:
//! which operation produced this face, in what role, and from which source entity.
//!
//! Outwards — to the kernel, into references, into the file — a name travels as a numeric descriptor, which
//! avoided widening the id type in two hundred places and across the FFI. A descriptor is not an ordinal but an
//! index into the name table of the document; the table lives in the file, so a descriptor means the same thing
//! across sessions. A tag in the high bits separates a structural name from the positional number the kernel
//! hands out for geometry whose origin is not derived yet — those operations are moved onto recipes one at a
//! time.

use crate::model::Id;
use serde::{Deserialize, Serialize};

/// Tags inside a descriptor: no tag means a positional number from the kernel, `NAMED` a face name, `EDGE` an
/// edge name. The edge tag includes the `NAMED` bit, so "is this name structural?" is one and the same check
/// for both.
pub const NAMED: u32 = 0x4000_0000;
pub const EDGE: u32 = 0x6000_0000;
/// A vertex name. This tag includes `NAMED` too, so the structural check stays the same for faces, edges and
/// vertices, while `TAG` tells all three apart.
pub const VERTEX: u32 = 0x5000_0000;
const TAG: u32 = 0x7000_0000;

/// The role of a face in the recipe of the operation that produced it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    /// The cap at the start of the operation, the profile itself.
    CapStart,
    /// The cap at the end of the operation, the translated profile.
    CapEnd,
    /// A side face produced by an edge of the profile; `src` is the sketch entity.
    Wall,
    /// A surface of revolution from an edge of the profile.
    Revolved,
    /// A swept surface from an edge of the profile.
    Swept,
    /// A lofted surface from an edge of a section.
    Lofted,
    /// The side surface of a primitive: cylinder, cone, torus or sphere.
    Side,
    /// A face produced by the hole tool.
    Hole,
    /// A fillet or chamfer surface produced by an edge.
    Blend,
    /// A corner patch of a fillet: the face produced by a vertex where several fillets meet. `src` is the
    /// smallest of the names of the filleted edges at that vertex — a vertex is identified by its edges, the
    /// way an edge is identified by its faces.
    Corner,
    /// The inner wall of a shell.
    ShellWall,
    /// A face of a thread turn, or of an auger flight, produced by its own edge of the groove profile.
    /// `src` is the index of the edge within the profile — flank, crest or root. The profile is computed from
    /// the standard and is the same for any number of turns, so an edge index is a recipe rather than a
    /// traversal order.
    ThreadGroove,
    /// A face of a thread relief or lead-in. `src` says which tool and at which end (0 relief at the start,
    /// 1 relief at the end, 2 lead-in at the start, 3 lead-in at the end), and `split` says which surface of
    /// the tool produced it (near plane, far plane, cylinder, cone). Both are properties of a construction
    /// made here rather than of a traversal order.
    ThreadRelief,
    /// The faces of the drill itself apart from the wall: the point cone, the counterbore cylinder and its
    /// annular floor. `src` says which one (0 point, 1 counterbore, 2 floor). The drill is constructed here
    /// and what each surface is, is known, so the kind of surface is part of the recipe rather than a guess
    /// from the geometry.
    HoleTool,
    /// The side of a draft: a face produced by tilting a source face. A drafted face arrives in the result
    /// twice — as the same element and as a new face on the side; the second one has a recipe of its own, and
    /// `src` is the name of the source face.
    DraftSide,
    /// A seam: a face that was not any face in the sources — it is born where two parts meet, at the joint
    /// between pattern copies or where a plate is welded on. It has no origin of its own, just as an edge has
    /// none, and it is identified the same way an edge is: by its neighbours. `src` is the smallest name among
    /// the named neighbours, and `split` is its rank among the seams of that neighbour.
    Seam,
    /// A patch face: a surface stretched over the selected edges. It has no source beyond the feature
    /// itself — there is one patch per feature, so the role alone is enough for a name.
    Patch,
    /// A section face left by splitting a body: it is produced by the cutting plane itself and has no other
    /// source. There is one name per feature — the pieces of a split are separate bodies and names live inside
    /// a body, so they cannot collide.
    CutSection,
    /// The offset side of a thickened face. `src` is the name of the source face: the offset copy is produced
    /// by it and by nothing else, which is its entire recipe.
    Thickened,
    /// A side wall of a thickening, produced by a boundary edge. `src` is the name of that edge.
    ThickenWall,
    /// The image of a face in a copy, from a pattern or a mirror. `src` is the descriptor of the source face
    /// and `split` the instance number. Without this every copy carried the names of the original, and a
    /// reference to a face of the second copy silently resolved to the first.
    Instance,
}

impl Role {
    /// A short tag for the role, used in the debug rendering of a name. Not a label in the interface: face
    /// roles never reach it, and the core carries no language of its own — the application picks the wording
    /// if it is ever needed.
    pub fn tag(&self) -> &'static str {
        match self {
            Role::CapStart => "cap_start",
            Role::CapEnd => "cap_end",
            Role::Wall => "wall",
            Role::Revolved => "revolved",
            Role::Swept => "swept",
            Role::Lofted => "lofted",
            Role::Side => "side",
            Role::Hole => "hole",
            Role::Blend => "blend",
            Role::Corner => "corner",
            Role::ShellWall => "shell_wall",
            Role::ThreadGroove => "thread_groove",
            Role::ThreadRelief => "thread_relief",
            Role::HoleTool => "hole_tool",
            Role::DraftSide => "draft_side",
            Role::Patch => "patch",
            Role::CutSection => "cut_section",
            Role::Thickened => "thickened",
            Role::ThickenWall => "thicken_wall",
            Role::Instance => "instance",
            Role::Seam => "seam",
        }
    }
}

/// The name of a face: who produced it, in what role, from what, and as which piece.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GeoName {
    /// The timeline node that produced this face: its creator, not the last node that touched it.
    pub feature: Id,
    pub role: Role,
    /// The source entity of the recipe: a sketch edge or a body edge. Zero means the role stands alone, as
    /// for a cap.
    pub src: Id,
    /// The piece number, when one source entity produced several faces through a split.
    pub split: u16,
}

impl GeoName {
    pub fn new(feature: Id, role: Role, src: Id) -> Self {
        Self { feature, role, src, split: 0 }
    }
}

/// The name of an edge is the pair of its faces. An edge has no recipe of its own: it is where two surfaces
/// meet, and that is the only thing objectively known about it. The name is therefore derived from the face
/// names, which are already sound, rather than from a traversal order. `index` separates several edges between
/// the same pair of faces — within a pair the order is deterministic, and the pair itself does not depend on
/// edits to neighbours.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EdgeName {
    /// descriptors of the two faces, sorted, so the name does not depend on which side it is seen from
    pub faces: [u32; 2],
    pub index: u16,
}

impl EdgeName {
    pub fn new(a: u32, b: u32, index: u16) -> Self {
        Self { faces: [a.min(b), a.max(b)], index }
    }
}

/// The name of a vertex is its edges. A vertex has no recipe of its own, just as an edge has none: it is where
/// edges meet, and that is the only thing objectively known about it. The same principle is already stated at
/// `Role::Corner` — a vertex is identified by its edges the way an edge is identified by its faces — and here
/// it becomes the name.
///
/// Three edges, not all of them. A vertex of a solid almost always has three; more appear where chamfers and
/// intersections meet. Taking all of them would change the name as soon as a fourth edge appeared nearby, that
/// is, lose the reference at exactly the moment the part is being edited. The three smallest descriptors are
/// taken instead: the set is deterministic, independent of traversal order, and any missing slots are zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VertexName {
    /// descriptors of the edges of the vertex, sorted, with the tail zeroed
    pub edges: [u32; 3],
}

impl VertexName {
    /// A name from the edges of a vertex: sort, then take the three smallest.
    pub fn new(mut edges: Vec<u32>) -> Self {
        edges.sort_unstable();
        edges.dedup();
        let mut e = [0u32; 3];
        for (i, d) in edges.into_iter().take(3).enumerate() {
            e[i] = d;
        }
        Self { edges: e }
    }
}

/// The name table of a document: descriptor to name and back. It lives in the file, since otherwise a
/// descriptor inside a reference would lose its meaning on the next open. It only ever grows: a descriptor once
/// issued is never reused, even after its feature is deleted, or a reference to unrelated geometry could land
/// on the freed number.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct NameTable {
    /// face names; the descriptor is `NAMED | index`
    list: Vec<GeoName>,
    /// edge names; the descriptor is `EDGE | index`. A separate list rather than one shared with faces: this
    /// way the document format grows by addition, and a file saved before edge names existed opens as it is,
    /// with no need to rewrite already stored face descriptors.
    #[serde(default)]
    edges: Vec<EdgeName>,
    /// vertex names; the descriptor is `VERTEX | index`. A separate list for the same reason as edges: the
    /// document format grows by addition, and face and edge descriptors are never rewritten.
    #[serde(default)]
    verts: Vec<VertexName>,
    /// Absorbed names: former face name to the name of the shared face.
    ///
    /// Merging coplanar faces into a monolith legitimately collapses two named faces into one, and one of the
    /// names has to give way. It used to simply disappear, taking everything that stood on it: a single sketch
    /// edit could lose six walls and eighteen edge names, leaving a fillet without its edges. The yielding name
    /// is now remembered, so a reference to it finds that shared face. This is a record of the merge having
    /// happened, not a fallback that guesses from geometry.
    #[serde(default)]
    absorbed: std::collections::HashMap<u32, u32>,
    /// reverse indices are derived and are not written to the file; they are rebuilt on load
    #[serde(skip)]
    index: std::collections::HashMap<GeoName, u32>,
    #[serde(skip)]
    eindex: std::collections::HashMap<EdgeName, u32>,
    #[serde(skip)]
    vindex: std::collections::HashMap<VertexName, u32>,
}

impl NameTable {
    /// The descriptor of a face name, existing or newly issued.
    pub fn intern_face(&mut self, name: GeoName) -> u32 {
        if let Some(&d) = self.index.get(&name) {
            return d;
        }
        let d = NAMED | self.list.len() as u32;
        self.list.push(name);
        self.index.insert(name, d);
        d
    }

    /// The descriptor of an edge name, existing or newly issued.
    pub fn intern_edge(&mut self, name: EdgeName) -> u32 {
        if let Some(&d) = self.eindex.get(&name) {
            return d;
        }
        let d = EDGE | self.edges.len() as u32;
        self.edges.push(name);
        self.eindex.insert(name, d);
        d
    }

    /// All face names in order; the descriptor of the i-th is `NAMED | i`. Needed when cloning a subtree: a
    /// copy of a part gets new feature ids and therefore new face names, and the references of the copied
    /// features — a thread on an edge, a shell on faces — have to move onto them. Without this a copy of a
    /// threaded part reported a missing crest, because the reference led to a name belonging to the original.
    pub fn faces(&self) -> &[GeoName] {
        &self.list
    }

    /// The descriptor of a vertex name, existing or newly issued.
    pub fn intern_vertex(&mut self, name: VertexName) -> u32 {
        if let Some(&d) = self.vindex.get(&name) {
            return d;
        }
        let d = VERTEX | self.verts.len() as u32;
        self.verts.push(name);
        self.vindex.insert(name, d);
        d
    }

    /// The descriptor of an already known vertex name, without interning.
    ///
    /// Needed where the caller asks rather than records: rendering holds `&self` and has no right to create a
    /// name. Absent from the table means nobody has named this vertex yet, so there was nowhere a reference to
    /// it could have come from.
    pub fn vertex_desc(&self, name: &VertexName) -> Option<u32> {
        self.vindex.get(name).copied()
    }

    /// The vertex name for a descriptor.
    pub fn vertex(&self, desc: u32) -> Option<VertexName> {
        if desc & TAG != VERTEX {
            return None;
        }
        self.verts.get((desc & !TAG) as usize).copied()
    }

    /// Is this descriptor a vertex name?
    pub fn is_vertex(desc: u32) -> bool {
        desc & TAG == VERTEX
    }

    /// All edge names in order; the descriptor of the i-th is `EDGE | i`. See [`NameTable::faces`].
    pub fn edges(&self) -> &[EdgeName] {
        &self.edges
    }

    /// The face name for a descriptor; `None` for a positional number or an edge name.
    pub fn get(&self, desc: u32) -> Option<GeoName> {
        if desc & TAG != NAMED {
            return None;
        }
        self.list.get((desc & !TAG) as usize).copied()
    }

    /// The edge name for a descriptor.
    pub fn edge(&self, desc: u32) -> Option<EdgeName> {
        if desc & TAG != EDGE {
            return None;
        }
        self.edges.get((desc & !TAG) as usize).copied()
    }

    /// Record an absorption: `loser` has yielded its face to the name `winner`.
    ///
    /// No chain is built up: if the winner itself once yielded, the record points straight at the last one.
    /// A lookup then stays a single step, and a cycle is impossible by construction.
    pub fn absorb(&mut self, loser: u32, winner: u32) {
        if loser == winner || !Self::is_named(loser) || !Self::is_named(winner) {
            return;
        }
        let target = self.absorbed_face(winner).unwrap_or(winner);
        if target == loser {
            return; // mutual absorption leaves nobody to record a trail for
        }
        self.absorbed.insert(loser, target);
        // whoever yielded to the loser now points at the same target
        let stale: Vec<u32> = self.absorbed.iter().filter(|(_, v)| **v == loser).map(|(k, _)| *k).collect();
        for k in stale {
            self.absorbed.insert(k, target);
        }
    }

    /// Forget an absorption that no longer holds.
    ///
    /// The record "name A yielded to name B" is true exactly as long as there is nothing left to carry A. A
    /// rebuild with different parameters can bring the face A back to life, and then the old record starts
    /// misleading: a live name is mapped onto someone else's and the reference looks for itself in the wrong
    /// place. A measurement caught this in its pure form — both candidates for an edge came out 80 mm away from
    /// the snapshot, because both sides of the pair had been reduced to absorbers that no longer exist.
    ///
    /// So on every rebuild of a body the records whose names are alive again are dropped.
    pub fn forget_absorbed(&mut self, live: &[u32]) {
        if self.absorbed.is_empty() {
            return;
        }
        let live: std::collections::HashSet<u32> = live.iter().copied().collect();
        self.absorbed.retain(|loser, _| !live.contains(loser));
    }

    /// All recorded absorptions, for measurements and diagnostics.
    pub fn absorbed_pairs(&self) -> Vec<(u32, u32)> {
        let mut v: Vec<(u32, u32)> = self.absorbed.iter().map(|(k, t)| (*k, *t)).collect();
        v.sort_unstable();
        v
    }

    /// Which name the face with this name yielded to, if it yielded at all.
    ///
    /// Two cases, and both are a recorded fact rather than a search for something similar:
    ///
    /// 1. An explicit absorption when coplanar faces merged, reported by the kernel.
    /// 2. A piece returned to the whole. "Piece k of face F" is by construction part of F. If the operation
    ///    stopped splitting that face, the piece is gone while F is there, and everything that stood on the
    ///    piece stands on F. This follows from the name itself, so no geometry needs checking. The case is not
    ///    hypothetical: a sketch edit collapsed four pieces back and left a fillet without four of its edges.
    ///
    /// The two cases chain together: a piece returns to its whole, and that whole may itself have yielded to a
    /// neighbour. So the walk continues while there is somewhere to go, with an explicit limit so that a
    /// damaged table cannot produce an endless loop.
    pub fn absorbed_face(&self, desc: u32) -> Option<u32> {
        let mut cur = desc;
        for _ in 0..8 {
            let next = self.absorbed.get(&cur).copied().or_else(|| {
                let n = self.get(cur)?;
                (n.split != 0).then(|| self.index.get(&GeoName { split: 0, ..n }).copied())?
            });
            match next {
                Some(d) if d != cur => cur = d,
                _ => break,
            }
        }
        (cur != desc).then_some(cur)
    }

    /// Where a former name leads, for faces and for edges alike.
    ///
    /// An edge has no recipe of its own: it is named by the pair of its faces. So once a face is absorbed, the
    /// edge is automatically named differently — by the new pair. An edge name is therefore translated
    /// component by component, and only if such a name already exists in the table: there is nothing to create
    /// a new one from, since nothing carries it.
    pub fn absorbed_target(&self, desc: u32) -> Option<u32> {
        if let Some(e) = self.edge(desc) {
            let a = self.absorbed_face(e.faces[0]).unwrap_or(e.faces[0]);
            let b = self.absorbed_face(e.faces[1]).unwrap_or(e.faces[1]);
            if a == e.faces[0] && b == e.faces[1] {
                return None; // neither face was absorbed, so there is nothing to translate
            }
            return self.eindex.get(&EdgeName::new(a, b, e.index)).copied();
        }
        self.absorbed_face(desc)
    }

    /// The canonical form of an edge name: the pair of face names reduced to their wholes, plus the index
    /// within the pair.
    ///
    /// An edge is named by a pair of faces, and a face can be split by a later operation — the edge then lies
    /// between a piece and a neighbour and is named differently, although it is the same edge of the same body.
    /// The converse holds too: the split may have disappeared and the piece returned to the whole. Reducing
    /// both faces to their wholes equates these records, and a reference survives either change.
    ///
    /// This is a translation by the structure of the name, not a search for something nearby: there is not a
    /// single coordinate here.
    pub fn canonical_edge(&self, desc: u32) -> Option<([u32; 2], u16)> {
        let e = self.edge(desc)?;
        // One rule for both sides. A piece used to be reduced only to its whole, and a whole only to its
        // absorber, so "piece 177" and "177" were canonicalised differently and one and the same edge compared
        // against itself as different. `absorbed_face` walks both rules to the end, so both sides arrive at the
        // same point.
        let whole = |d: u32| -> u32 { self.absorbed_face(d).unwrap_or(d) };
        let (a, b) = (whole(e.faces[0]), whole(e.faces[1]));
        Some(([a.min(b), a.max(b)], e.index))
    }

    /// Is this descriptor a structural name rather than a positional number?
    pub fn is_named(desc: u32) -> bool {
        desc & NAMED != 0
    }

    /// Is this descriptor an edge name?
    pub fn is_edge(desc: u32) -> bool {
        desc & TAG == EDGE
    }

    pub fn len(&self) -> usize {
        self.list.len() + self.edges.len()
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty() && self.edges.is_empty()
    }

    /// Rebuild the reverse index after loading from a file, where it is `serde(skip)`.
    ///
    /// Without it, interning would issue a new descriptor for an already known name, and references written in
    /// a previous session would stop matching the geometry just built.
    pub fn rebuild_index(&mut self) {
        self.index = self.list.iter().enumerate().map(|(i, n)| (*n, NAMED | i as u32)).collect();
        self.eindex = self.edges.iter().enumerate().map(|(i, n)| (*n, EDGE | i as u32)).collect();
        self.vindex = self.verts.iter().enumerate().map(|(i, n)| (*n, VERTEX | i as u32)).collect();
    }

    /// A rendering of a name for debugging, not a label in the interface.
    ///
    /// Face roles never reach the interface by any path: they are read by whoever is repairing topological
    /// names, in the message of a failing test or in a log. So this is not prose in one language — the core
    /// carries no language — but a dense technical record: the role as a tag and everything else as numbers.
    /// `wall f12 src7 p1`, `edge[cap_end f3 | wall f3]:2`, `pos 41`.
    pub fn describe(&self, desc: u32) -> String {
        if let Some(e) = self.edge(desc) {
            let tail = if e.index > 0 { format!(":{}", e.index) } else { String::new() };
            return format!("edge[{} | {}]{tail}", self.describe(e.faces[0]), self.describe(e.faces[1]));
        }
        match self.get(desc) {
            None => format!("pos {desc}"),
            Some(n) => {
                let mut s = format!("{} f{}", n.role.tag(), n.feature);
                if n.src != 0 {
                    s.push_str(&format!(" src{}", n.src));
                }
                if n.split != 0 {
                    s.push_str(&format!(" p{}", n.split));
                }
                s
            }
        }
    }
}
