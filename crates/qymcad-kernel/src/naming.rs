//! THE NAMING OF THE BRIDGE, for what the history of an operation was silent about. The door carries the names of
//! faces and edges through the history of the algorithm (`qym_result` on the C++ side); what the history does not
//! name - the pieces of a split face beyond the first, the faces it made from nothing, the edges between faces - is
//! named here, by one rule for every operation, in this order:
//!
//! 1. the pieces of a split face, after the face they were cut from;
//! 2. the seams, in waves, after a named neighbour;
//! 3. what has no named neighbour - a body brought in from a file or made of a mesh - by where it lies;
//! 4. the edges, after the pair of faces they run between.
//!
//! The rules are functions of the facts of a body and of the table of the document (`NameMint`), and give the renames
//! as values; the table is the document's, and it gives the descriptors on the thread of the document, in the order of
//! its tree - so the same document gives the same names on every rebuild.
//!
//! The rules as a whole, with the contract that the bridge alone names: `crates/qymcad-kernel/NAMING.md`.

use qymcad_core::feature::{EdgeEnds, EdgeFaces, EdgeOnItsFace, FacePlace, FaceSplit, NamedAfter, OnItsFace, OnOneSurface, PieceSide, Rename, SilentNaming};
use qymcad_core::model::Id;
use qymcad_core::names::{CutOff, EdgeName, GeoName, NameMint, Role, Side};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

/// NAME WHAT THE HISTORY OF `ask.body` WAS SILENT ABOUT, in the live body and in the answer.
pub(crate) fn silent(kernel: &crate::OcctKernel, ask: SilentNaming<'_>) -> NamedAfter {
    let SilentNaming { body, feature, listed, scale, names } = ask;
    let shapes = kernel.shapes.borrow();
    let Some(shape) = shapes.get(&body) else { return NamedAfter::default() };
    // a name the history gave to several edges is no name of any of them: each is named by its pair below; unshared
    // before the facts are read, so the facts carry the numbers the edges wear to the end
    shape.unshare_edge_ids();
    let facts = shape.naming_facts();
    let mut moved = Moved::default();
    // the faces as the document lists them; for a body raised again from its file, as the kernel holds them
    let mut live: Vec<u32> = match listed {
        Some(list) => list.to_vec(),
        None => facts.places.iter().map(|p| p.id).collect(),
    };
    let was = live.clone();
    let mut named = NamedAfter::default();
    // THE ROLES FIRST: the faces the operation made under numbers awaiting their names take them from the table now,
    // and a piece of such a face is told after the name its whole took
    let roles = kernel.roles.borrow_mut().remove(&body).unwrap_or_default();
    let held: HashSet<u32> = live.iter().copied().chain(shape.absorbed_names().into_iter().flat_map(|a| [a.loser, a.winner])).chain(shape.face_splits().into_iter().map(|s| s.origin)).collect();
    let landed = land_roles(&roles, &held, names);
    land(shape, &landed, &mut live, &mut moved);
    let named_as: HashMap<u32, u32> = landed.iter().map(|r| (r.was, r.now)).collect();
    // the names faces merged into one yielded, in the names the roles took: a wall of a tool merged with a face of the
    // body yields under the number it was handed, and the table has to hear its name
    let as_named = |d: u32| named_as.get(&d).copied().unwrap_or(d);
    named.absorbed = shape.absorbed_names().into_iter().map(|a| qymcad_core::feature::Absorbed { loser: as_named(a.loser), winner: as_named(a.winner) }).collect();
    let splits: Vec<FaceSplit> = shape.face_splits().into_iter().map(|s| FaceSplit { origin: named_as.get(&s.origin).copied().unwrap_or(s.origin), ..s }).collect();
    if !splits.is_empty() {
        // the record is single-use: cleared before any name is given, so no early return leaves it alive for the next
        // operation to fold into its own group (measured: 26 faces instead of 56, a face pull no longer building)
        shape.clear_face_splits();
        let asked: Vec<u32> = splits.iter().flat_map(|s| [s.piece, s.origin]).collect();
        let split = Split { splits: &splits, pairs: &facts.pairs(&moved), sides: &shape.piece_sides(&asked), places: &facts.places(&moved) };
        let renames = pieces(&split, &live, feature, names);
        land(shape, &renames, &mut live, &mut moved);
    }
    // the pieces the history did not tell: a nameless face on the surface of a named face is a piece of it
    let untold = untold_pieces(&facts.surfaces(&moved), &live, names);
    if !untold.is_empty() {
        let asked: Vec<u32> = untold.iter().flat_map(|s| [s.piece, s.origin]).collect();
        let split = Split { splits: &untold, pairs: &facts.pairs(&moved), sides: &shape.piece_sides(&asked), places: &facts.places(&moved) };
        let renames = pieces(&split, &live, feature, names);
        land(shape, &renames, &mut live, &mut moved);
    }
    // in waves: a face in a corner has orphans for neighbours, and the faces the first wave names anchor the second
    // (measured: on a shell of a shell exactly four corner faces stayed nameless in one pass)
    for _ in 0..8 {
        let renames = seams(feature, &facts.pairs(&moved), &live, names);
        if renames.is_empty() {
            break;
        }
        land(shape, &renames, &mut live, &mut moved);
    }
    // asked of the list first, which costs nothing; a body raised from its file is always asked of the kernel
    if listed.is_none() || !live.iter().all(|d| names.knows(*d)) {
        let renames = placed(feature, scale, &facts.places(&moved), names);
        land(shape, &renames, &mut live, &mut moved);
    }
    // what each face the document lists became, over all the waves: one renaming the document applies at once
    named.faces = was.iter().zip(&live).filter(|(w, n)| w != n).map(|(w, n)| Rename { was: *w, now: *n }).collect();
    let pairs = facts.pairs(&moved);
    if !pairs.is_empty() {
        let renames = edges(&pairs, &ends_of(&facts.at_ends, &pairs, &moved), &facts.edges, &facts.on_face, names);
        // a name that told another pair yielded to the name of the pair: a reference to it, or to a piece of its pair,
        // follows the record in every later rebuild, not only in this one
        named.absorbed.extend(renames.iter().filter(|r| names.edge(r.was).is_some()).map(|r| qymcad_core::feature::Absorbed { loser: r.was, winner: r.now }));
        shape.rename_edges(&renames);
        named.edges = renames;
    }
    named
}

/// WHAT THE NAMING READS OF A BODY, asked of the bridge once (`Shape::naming_facts`), under the numbers the faces wore
/// before they took their names: each rule reads them through what the faces have become since (`Moved`). Asked one by
/// one after every wave, the naming of the 18 roundings of the case example took 752 ms in the bridge alone, of it 242 ms
/// the faces at the ends of edges and 183 ms the polylines of the edges; read once, 152 ms, and the whole naming 310 ms.
#[derive(Default)]
pub(crate) struct Facts {
    pub places: Vec<FacePlace>,
    pub surfaces: Vec<OnOneSurface>,
    pub pairs: Vec<EdgeFaces>,
    pub at_ends: Vec<FaceAtEnd>,
    pub edges: Vec<EdgeAt>,
    pub on_face: Vec<EdgeOnItsFace>,
}

impl Facts {
    /// The edges between two faces, under the names the faces wear now.
    fn pairs(&self, moved: &Moved) -> Vec<EdgeFaces> {
        self.pairs.iter().map(|p| EdgeFaces { edge: p.edge, faces: p.faces.map(|f| moved.now(f)) }).collect()
    }

    /// Where every face lies, under the name it wears now.
    fn places(&self, moved: &Moved) -> Vec<FacePlace> {
        self.places.iter().map(|p| FacePlace { id: moved.now(p.id), ..*p }).collect()
    }

    /// The faces on one surface of the kernel, under the names they wear now.
    fn surfaces(&self, moved: &Moved) -> Vec<OnOneSurface> {
        self.surfaces.iter().map(|s| OnOneSurface { face: moved.now(s.face), surface: moved.now(s.surface) }).collect()
    }
}

/// A FACE MEETING AN EDGE AT ONE OF ITS ENDS, not one of the faces the edge runs between.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FaceAtEnd {
    pub edge: u32,
    pub face: u32,
}

/// WHAT EACH FACE THE FACTS WERE READ UNDER HAS BECOME, over the waves of renaming. A face no wave renamed is not
/// in it.
#[derive(Default)]
struct Moved(HashMap<u32, u32>);

impl Moved {
    fn now(&self, face: u32) -> u32 {
        self.0.get(&face).copied().unwrap_or(face)
    }

    /// One wave renamed, all at once, as the body applies it: every face wearing a name it renames takes the new one.
    fn follow(&mut self, renames: &[Rename]) {
        let to: HashMap<u32, u32> = renames.iter().map(|r| (r.was, r.now)).collect();
        for now in self.0.values_mut() {
            if let Some(n) = to.get(now) {
                *now = *n;
            }
        }
        for r in renames {
            self.0.entry(r.was).or_insert(r.now);
        }
    }
}

/// THE TWO SMALLEST FACES NAMED BY RECIPE AT THE ENDS OF EACH EDGE, apart from its own two, under the names they wear
/// now (0 where there is none). A positional number does not count: it drifts itself.
fn ends_of(at_ends: &[FaceAtEnd], pairs: &[EdgeFaces], moved: &Moved) -> Vec<EdgeEnds> {
    use qymcad_core::names::{NAMED, ROLE};
    let own: HashMap<u32, [u32; 2]> = pairs.iter().map(|p| (p.edge, p.faces)).collect();
    let mut around: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
    for r in at_ends {
        let face = moved.now(r.face);
        if face & (NAMED | ROLE) != 0 && !own.get(&r.edge).is_some_and(|f| f.contains(&face)) {
            around.entry(r.edge).or_default().insert(face);
        }
    }
    around
        .into_iter()
        .map(|(edge, faces)| {
            let mut two = faces.into_iter();
            EdgeEnds { edge, around: [two.next().unwrap_or(0), two.next().unwrap_or(0)] }
        })
        .collect()
}

/// THE FACES OF ONE WAVE RENAMED, in the live body, in the list of live faces and in what the faces of the facts have
/// become - all at once: one wave gives a piece the name of its whole and the whole a name of its own, and applied one
/// after the other the second took both (measured: the two strips of a pinched rim under one name, nothing left to
/// select one with).
fn land(shape: &crate::Shape, renames: &[Rename], live: &mut [u32], moved: &mut Moved) {
    shape.rename_faces(renames);
    moved.follow(renames);
    let now: HashMap<u32, u32> = renames.iter().map(|r| (r.was, r.now)).collect();
    for d in live.iter_mut() {
        if let Some(n) = now.get(d) {
            *d = *n;
        }
    }
}

/// THE ROLES AN OPERATION GAVE, named by the table: the number each was handed under becomes its descriptor. A name
/// offered to a face the result may not hold (`RoleRecipe::Spare`) is written only when the result holds its number
/// (`held`): the caps every wall of several regions offers would otherwise fill the table with names no face wears.
pub(crate) fn land_roles(roles: &crate::door::Roles, held: &HashSet<u32>, names: &mut dyn NameMint) -> Vec<Rename> {
    use crate::door::RoleRecipe;
    let mut renames: Vec<Rename> = Vec::new();
    for r in &roles.given {
        let name = match r.recipe {
            RoleRecipe::Named(name) => name,
            RoleRecipe::Spare(name) if held.contains(&r.number) => name,
            RoleRecipe::Spare(_) => continue,
        };
        renames.push(Rename { was: r.number, now: names.intern_face(name) });
    }
    renames
}

/// THE NAMES OF THE ENDS OF THE EDGES. An end is one of the two vertices of the topology the edge runs between - the
/// nearer to that end, so a closed edge or one walked backwards is read right - and a vertex is named by the edges that
/// meet at it (`VertexName`: the three smallest). Told by the topology, not by where the ends stand: the model grouped the
/// ends of the edges of a mesh within a micrometre, and two vertices nearer than that were one.
pub(crate) fn name_edge_ends(edges: &mut [qymcad_core::geom::MeshEdge], vertices: &[crate::VertexAt], ends: &[crate::EdgeVertices]) {
    let gap = |p: [f64; 3], q: [f64; 3]| (p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2);
    let of: HashMap<u32, [u32; 2]> = ends.iter().map(|e| (e.edge, e.ends)).collect();
    let vertex = |k: u32| (k as usize).checked_sub(1).and_then(|i| vertices.get(i));
    for e in edges.iter_mut() {
        let Some(pair) = of.get(&e.id) else { continue };
        let two: Vec<&crate::VertexAt> = pair.iter().filter_map(|k| vertex(*k)).collect();
        for (k, end) in [e.a, e.b].into_iter().enumerate() {
            if let Some(v) = two.iter().min_by(|x, y| gap(x.at, end).total_cmp(&gap(y.at, end))) {
                e.ends[k] = qymcad_core::names::VertexName { edges: v.edges };
            }
        }
    }
}

/// WHAT AN OPERATION TELLS OF THE FACES IT SPLIT: the pieces and their originals, the edges between faces, the side of
/// each face beside a piece the piece lies on, and where each face stands.
#[derive(Clone, Copy)]
pub(crate) struct Split<'a> {
    pub splits: &'a [FaceSplit],
    pub pairs: &'a [EdgeFaces],
    pub sides: &'a [PieceSide],
    pub places: &'a [FacePlace],
}

/// THE PIECES THE HISTORY DID NOT TELL: a nameless face of `live` on one surface of the kernel with named faces that are
/// all the whole or pieces of one face, that face standing among them, is a piece of it. Measured on the filter: a window
/// through a tube widened, the history of the operation told only the upper half of a wall, the lower half was named as
/// a seam after the tube beside it, and the upper half took the name of the whole. A recorded fact of the kernel - the
/// pieces of a split face keep its surface - not a search by place.
fn untold_pieces(mates: &[OnOneSurface], live: &[u32], names: &dyn NameMint) -> Vec<FaceSplit> {
    let live: HashSet<u32> = live.iter().copied().collect();
    let mut on: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for m in mates.iter().filter(|m| live.contains(&m.face)) {
        on.entry(m.surface).or_default().push(m.face);
    }
    let mut out = Vec::new();
    for faces in on.values() {
        let named: Vec<u32> = faces.iter().copied().filter(|f| names.knows(*f)).collect();
        let nameless: Vec<u32> = faces.iter().copied().filter(|f| !names.knows(*f)).collect();
        let wholes: HashSet<GeoName> = named.iter().filter_map(|f| names.face(*f)).map(|g| g.whole()).collect();
        let wholes: Vec<GeoName> = wholes.into_iter().collect();
        let ([whole], false) = (wholes.as_slice(), nameless.is_empty()) else { continue };
        let Some(origin) = named.iter().copied().find(|f| names.face(*f) == Some(*whole)) else { continue };
        out.extend(nameless.iter().map(|piece| FaceSplit { piece: *piece, origin, index: 0 }));
    }
    out
}

/// The named faces beside each face, by the edges between them.
fn neighbours(pairs: &[EdgeFaces]) -> HashMap<u32, Vec<u32>> {
    let mut nb: HashMap<u32, Vec<u32>> = HashMap::new();
    for p in pairs {
        nb.entry(p.faces[0]).or_default().push(p.faces[1]);
        nb.entry(p.faces[1]).or_default().push(p.faces[0]);
    }
    nb
}

/// THE PIECES OF A SPLIT FACE, after the face they were cut from, told by what cut each off: the face of this node
/// beside the piece first in the order of its recipe (`InRecipe`), and the side of it the piece lies on (`sides`, asked
/// of the body). The number of a piece is its place among the pieces cut off by the same face on the same side, by
/// what else of the node stands beside them and where they stand - so taking one piece away renames no other. Measured before: the number was the place
/// among all the pieces, and an edit that closed a gap took one piece away and renumbered the rest (the filter, the
/// case and the vacuum cleaner: 8 faces that did not move renamed over the matrix of their edits). The name of the whole
/// stays with the piece first by the faces of the node and of its own node beside it (`PieceKey`), what cut it off, and
/// where it stands. Measured: kept only on a piece no face of the node
/// stands beside, it renamed more than it saved (17 faults against 15 over the matrix) - a face of the node coming to
/// stand beside such a piece, the piece not moving, took the name of the whole from it.
/// `crates/qymcad-kernel/NAMING.md`, "A face split in pieces".
pub(crate) fn pieces(split: &Split, live: &[u32], feature: Id, names: &mut dyn NameMint) -> Vec<Rename> {
    let Split { splits, pairs, sides, places } = *split;
    let nb = neighbours(pairs);
    let live: HashSet<u32> = live.iter().copied().collect();
    // a piece beside a piece of another face split by the same operation, still under a number, is beside that face:
    // measured on the filter, the lower halves of the walls of a window through a tube bordered only one another, had no
    // named face of the node beside them, and came first in the order of the recipe
    let origin_of: HashMap<u32, u32> = splits.iter().map(|s| (s.piece, s.origin)).collect();
    let name_of = |f: u32, names: &dyn NameMint| names.face(f).or_else(|| origin_of.get(&f).and_then(|o| names.face(*o)));
    // where a piece stands, to the micrometre: what orders pieces the recipe cannot tell apart
    let at: HashMap<u32, [i64; 3]> = places.iter().map(|p| (p.id, p.at.map(|v| (v * 1e3).round() as i64))).collect();
    let side_of: HashMap<Beside, Side> = sides.iter().map(|s| (Beside { piece: s.piece, face: s.beside }, s.side)).collect();
    // in the order of the originals, not of a hash: a step back takes the names it gave away and a step forward gives
    // them again - in hash order the same name came to the half of another side
    let mut groups: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for s in splits {
        groups.entry(s.origin).or_default().push(s.piece);
    }
    let mut renames: Vec<Rename> = Vec::new();
    for (origin, mut members) in groups {
        let Some(base) = names.face(origin) else { continue }; // the original is nameless: nothing to inherit
        if live.contains(&origin) {
            members.push(origin); // the piece holding the name of the whole takes part in the split too
        }
        let own: HashSet<u32> = members.iter().copied().collect();
        let keyed = |f: u32| -> Keyed {
            let mut key = PieceKey::default();
            let mut cutters: Vec<Cutter> = Vec::new();
            for n in nb.get(&f).into_iter().flatten().filter(|n| !own.contains(n)) {
                let Some(g) = name_of(*n, &*names) else { continue };
                let at = InRecipe { feature: g.feature, role: g.role as u8, src: g.src, split: g.split };
                if g.feature == feature {
                    key.cut_by.push(at);
                    cutters.push(Cutter { at, name: g, face: *n });
                } else if g.feature == base.feature {
                    key.of_its_node.push(at);
                }
            }
            let first = cutters.into_iter().min_by(|a, b| a.at.cmp(&b.at).then(a.face.cmp(&b.face)));
            let side = first.map_or(Side::On, |c| side_of.get(&Beside { piece: f, face: c.face }).copied().unwrap_or(Side::On));
            for list in [&mut key.cut_by, &mut key.of_its_node] {
                list.sort_unstable();
                list.dedup();
            }
            let by = first.map(|c| CutBy { face: c.at, side });
            let cut = first.map(|c| CutOff { feature: c.name.feature, role: c.name.role, src: c.name.src, split: c.name.split, side });
            Keyed { key, by, cut, face: f }
        };
        let mut all: Vec<Keyed> = members.iter().map(|f| keyed(*f)).collect();
        // pieces the recipe cannot tell apart - the two halves of a wall of a window through a tube - by what cut them off
        // and where they stand, never by the number the kernel gave them: the kernel handing the other half as the
        // original after an edit moved the name of the whole across the part
        all.sort_by(|a, b| a.key.cmp(&b.key).then(a.by.cmp(&b.by)).then(at.get(&a.face).cmp(&at.get(&b.face))).then(a.face.cmp(&b.face)));
        let mut in_group: HashMap<Option<CutBy>, u16> = HashMap::new();
        for (place, k) in all.iter().enumerate() {
            let (cut, f) = (k.cut, &k.face);
            let split = if place == 0 {
                0
            } else {
                let counted = in_group.entry(k.by).or_insert(u16::from(k.by.is_none()));
                let split = *counted;
                *counted = counted.saturating_add(1);
                split
            };
            // the split ordinal of a copy of a pattern holds its copy number, of a seam its seam number: written over,
            // five pieces of a copied face came under one name; so their whole is the face itself, "piece k of origin"
            let name = if place == 0 {
                base
            } else if base.split != 0 || base.role == Role::Seam {
                GeoName { feature: base.feature, role: base.role, src: origin as Id, split, cut }
            } else {
                GeoName { split, cut, ..base }
            };
            let d = names.intern_face(name);
            if *f != d {
                renames.push(Rename { was: *f, now: d });
            }
        }
    }
    renames
}

/// THE ORDER OF THE PIECES OF A SPLIT FACE, which tells the one keeping the name of the whole: the faces the node made
/// beside it, then the faces of the node the split face comes from. The faces of other nodes beside a piece do not take
/// part: they come and go with edits beside it (measured on the filter: an edit brought the walls of another window
/// beside the halves of a wall, and told by them the name of the whole went to the other half). The faces of its own
/// node stay (measured on the same filter: without them the ring of a cap beside its own wall came first one time and
/// not the next, the rings on the axis having one centre).
#[derive(Default, PartialEq, Eq, PartialOrd, Ord)]
struct PieceKey {
    cut_by: Vec<InRecipe>,
    of_its_node: Vec<InRecipe>,
}

/// A PIECE AND WHAT TELLS IT: its place in the order of the recipe, what cut it off, and the name of that.
struct Keyed {
    key: PieceKey,
    by: Option<CutBy>,
    cut: Option<CutOff>,
    face: u32,
}

/// A PIECE AND A FACE BESIDE IT.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Beside {
    piece: u32,
    face: u32,
}

/// A FACE OF THE NODE BESIDE A PIECE: in the order of its recipe, its name, and the face it is.
#[derive(Clone, Copy)]
struct Cutter {
    at: InRecipe,
    name: GeoName,
    face: u32,
}

/// THE FACE OF THE NODE THAT CUT A PIECE OFF, and the side of it the piece lies on.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct CutBy {
    face: InRecipe,
    side: Side,
}

/// A NAME IN THE ORDER OF ITS RECIPE: the node, the role, the source, the piece - not the place the table holds it at.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct InRecipe {
    feature: Id,
    role: u8,
    src: Id,
    split: u16,
}

/// ONE WAVE OF SEAMS: an orphan with a named neighbour is named after the smallest of them, its number among the orphans
/// of that neighbour told by its other named neighbours. The node is part of the name: two operations both making "a
/// seam at face N" met in one body would lead a reference to two places at once.
pub(crate) fn seams(feature: Id, pairs: &[EdgeFaces], live: &[u32], names: &mut dyn NameMint) -> Vec<Rename> {
    let orphans: Vec<u32> = live.iter().copied().filter(|d| !names.knows(*d)).collect();
    if orphans.is_empty() {
        return Vec::new();
    }
    let nb = neighbours(pairs);
    // the named neighbours of every orphan, asked before any name of this wave is given
    let named_nb: HashMap<u32, Vec<u32>> = orphans
        .iter()
        .map(|f| {
            let mut v: Vec<u32> = nb.get(f).map(|ns| ns.iter().copied().filter(|n| names.knows(*n)).collect()).unwrap_or_default();
            v.sort_unstable();
            v.dedup();
            (*f, v)
        })
        .collect();
    let mut groups: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for f in &orphans {
        if let Some(&host) = named_nb.get(f).and_then(|v| v.first()) {
            groups.entry(host).or_default().push(*f);
        }
    }
    let taken: HashSet<u32> = live.iter().copied().collect();
    let mut renames: Vec<Rename> = Vec::new();
    for (host, mut members) in groups {
        members.sort_by(|a, b| named_nb.get(a).cmp(&named_nb.get(b)).then(a.cmp(b)));
        for (k, f) in members.iter().enumerate() {
            let d = names.intern_face(GeoName { feature, role: Role::Seam, src: host as Id, split: k.min(u16::MAX as usize) as u16, cut: None });
            if *f != d && !taken.contains(&d) {
                renames.push(Rename { was: *f, now: d });
            }
        }
    }
    renames
}

/// BY WHERE IT LIES, the last rule: a face with no history and no named neighbour - a body brought in from a file or
/// made of a mesh (measured: a cube made of a mesh held 6 of 6 faces on positional numbers). The place is told in the
/// frame of the file, divided by the factor the body came in at (rescaled, saved and opened, a fillet on it found no
/// edge otherwise). Two faces at one place, which two faces of a sound body are not, take the order of their numbers.
pub(crate) fn placed(feature: Id, scale: f64, places: &[FacePlace], names: &mut dyn NameMint) -> Vec<Rename> {
    let mut by_key: BTreeMap<Id, Vec<u32>> = BTreeMap::new();
    for place in places.iter().filter(|p| !names.knows(p.id)) {
        let in_file = FacePlace { at: place.at.map(|v| v / scale), ..*place };
        by_key.entry(place_key(&in_file)).or_default().push(place.id);
    }
    let taken: HashSet<u32> = places.iter().map(|p| p.id).collect();
    let mut renames: Vec<Rename> = Vec::new();
    for (key, mut faces) in by_key {
        faces.sort_unstable();
        for (k, f) in faces.iter().enumerate() {
            let d = names.intern_face(GeoName { feature, role: Role::Placed, src: key, split: k.min(u16::MAX as usize) as u16, cut: None });
            if *f != d && !taken.contains(&d) {
                renames.push(Rename { was: *f, now: d });
            }
        }
    }
    renames
}

/// THE KEY OF A PLACE: the point to a micrometre and the normal to a thousandth, folded into one number (FNV-1a over the
/// six rounded values). The same place gives the same key on every machine and every rebuild.
fn place_key(place: &FacePlace) -> Id {
    let rounded = [place.at[0] * 1e3, place.at[1] * 1e3, place.at[2] * 1e3, place.normal[0] * 1e3, place.normal[1] * 1e3, place.normal[2] * 1e3].map(|v| v.round() as i64);
    rounded.iter().flat_map(|v| v.to_le_bytes()).fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3))
}

/// THE EDGES, after the pair of faces they run between. An edge between two named faces is named whatever the rest of
/// the body is (measured: one nameless face cancelling the whole body left 190 edges positional). The one edge of its
/// pair is named by the pair alone. Edges of one pair are told apart by the faces at their ends - the upper and the lower
/// edge of a slot between the same walls, the cap at one and the floor at the other - never by their place in an order
/// of the pair: numbered within the pair, an edit that took one of them away renumbered the others (measured on the case
/// example: a chamfer on "the wall with the end cap, 2" took the edge beside its own and refused). Edges of one pair
/// with the same ends are numbered by where they stand (`EdgeAt`); a number a neighbour wears through the history
/// passes to the next, since a newcomer given it made two edges one address (measured: 8 of 24 edges of a shelled
/// pocket).
///
/// AN EDGE NAMED BESIDE A FACE GONE FROM THE BODY is named again by the pair it runs between. A strip taken out between a
/// rounding and a wall makes its two long sides one edge, and whichever of the two names it kept, that name says "beside
/// the strip". The renaming goes to the document with the others, the old name recorded as yielded to the new one.
/// Measured on a sample: with the smaller name kept, the rounding above the strip lost two of its four edges. A name of
/// another pair whose faces are all in the body - a piece for its whole, a wall a cut put along a seam - stays: a
/// reference to it is read through its pair (`NameTable::canonical_edge`), and renaming those too left 28 edges of a
/// rounding of a sample unfound after an edit.
pub(crate) fn edges(pairs: &[EdgeFaces], ends: &[EdgeEnds], body_edges: &[EdgeAt], on_face: &[EdgeOnItsFace], names: &mut dyn NameMint) -> Vec<Rename> {
    // the names worn on the whole body, not only by the edges with a pair: a seam and a degenerate edge have none, and
    // their names stayed free to be handed out twice
    let mut worn: HashSet<u32> = body_edges.iter().map(|e| e.edge).chain(pairs.iter().map(|p| p.edge)).filter(|e| names.knows(*e)).collect();
    let mut renames: Vec<Rename> = Vec::new();
    let taken = edges_on_their_face(pairs, on_face, names, &mut renames);
    worn.extend(taken.iter().copied());
    let mut by_pair: BTreeMap<[u32; 2], Vec<u32>> = BTreeMap::new();
    let faces: HashSet<u32> = pairs.iter().flat_map(|p| p.faces).collect();
    for p in pairs {
        let [a, b] = p.faces;
        // nowhere to take a name from; or an edge with one face on both sides, numbered above
        if !names.knows(a) || !names.knows(b) || a == b {
            continue;
        }
        // named through the history, the name telling no face gone from the body and not taken by a seam
        let yields = taken.contains(&p.edge);
        if !yields && names.knows(p.edge) && names.edge(p.edge).is_none_or(|n| n.faces.iter().all(|f| faces.contains(f))) {
            continue;
        }
        by_pair.entry([a.min(b), a.max(b)]).or_default().push(p.edge);
    }
    let around: HashMap<u32, [u32; 2]> = ends.iter().map(|e| (e.edge, e.around)).collect();
    let at: HashMap<u32, [i64; 3]> = body_edges.iter().map(|e| (e.edge, e.at.map(|v| (v * 1e4).round() as i64))).collect();
    // how many edges of the body run between each pair, the ones keeping a name through the history among them
    let mut of_pair: HashMap<[u32; 2], usize> = HashMap::new();
    for p in pairs {
        *of_pair.entry([p.faces[0].min(p.faces[1]), p.faces[0].max(p.faces[1])]).or_default() += 1;
    }
    for ([a, b], es) in by_pair {
        // the one edge of its pair is named by the pair alone: the faces at its ends change with any edit beside it
        // (measured: told by its ends, 901 edges of the examples that did not move were renamed over the matrix of edits)
        let twins = of_pair.get(&[a, b]).copied().unwrap_or(0) > 1;
        let mut by_ends: BTreeMap<[u32; 2], Vec<u32>> = BTreeMap::new();
        for e in es {
            let told = if twins { around.get(&e).copied().unwrap_or([0, 0]) } else { [0, 0] };
            let blank = EdgeName::new(a, b, 0).at_ends(told);
            by_ends.entry(blank.ends).or_default().push(e);
        }
        for (ends, mut twins) in by_ends {
            twins.sort_unstable_by_key(|e| (at.get(e).copied(), *e));
            let mut next = 0u16;
            for e in twins {
                let name = loop {
                    let n = names.intern_edge(EdgeName::new(a, b, next).at_ends(ends));
                    next = next.saturating_add(1);
                    if !worn.contains(&n) {
                        break n;
                    }
                };
                worn.insert(name);
                renames.push(Rename { was: e, now: name });
            }
        }
    }
    renames
}

/// AN EDGE OF THE BODY AND WHERE IT STANDS: the point of its curve at the middle of its range, which tells apart edges
/// of one pair with the same faces at their ends.
#[derive(Clone, Copy, Debug)]
pub(crate) struct EdgeAt {
    pub edge: u32,
    pub at: [f64; 3],
}

/// THE EDGES WITH ONE FACE ON BOTH SIDES, numbered by where they come from and where they stand - never by the order
/// the kernel gives them in, never as the first number free: the seam where the face closes along U from 0, along V
/// from 1000, an edge lying inside the face from 2000; several of one kind in the order of where they stand on the face
/// (`EdgeOnItsFace`). Measured on the vacuum cleaner: numbered as the first number free, the seam of a revolved wall went
/// from 1 to 0 when an edge of another pair, wearing "the wall with itself, 0" through the history, took the name of its
/// own pair after an edit. A name a seam takes from an edge of another pair is that edge's to yield: it is named by its
/// own pair. Returns the names the seams took; the renamings go into `renames`.
fn edges_on_their_face(pairs: &[EdgeFaces], on_face: &[EdgeOnItsFace], names: &mut dyn NameMint, renames: &mut Vec<Rename>) -> HashSet<u32> {
    let on: HashMap<u32, &EdgeOnItsFace> = on_face.iter().map(|e| (e.edge, e)).collect();
    let mut by_face: BTreeMap<u32, Vec<EdgeOnItsFace>> = BTreeMap::new();
    for p in pairs.iter().filter(|p| p.faces[0] == p.faces[1] && names.knows(p.faces[0])) {
        let told = on.get(&p.edge).map_or(EdgeOnItsFace { edge: p.edge, kind: OnItsFace::Inside, along: 0, across: 0 }, |e| **e);
        by_face.entry(p.faces[0]).or_default().push(told);
    }
    let mut taken: HashSet<u32> = HashSet::new();
    for (face, mut es) in by_face {
        es.sort_by_key(|e| (e.kind, e.along, e.across, e.edge));
        let mut in_kind: HashMap<OnItsFace, u16> = HashMap::new();
        for e in es {
            let rank = in_kind.entry(e.kind).or_insert(0);
            let base: u16 = match e.kind {
                OnItsFace::SeamAlongU => 0,
                OnItsFace::SeamAlongV => 1000,
                OnItsFace::Inside => 2000,
            };
            let name = names.intern_edge(EdgeName::new(face, face, base.saturating_add((*rank).min(999))));
            *rank += 1;
            taken.insert(name);
            if name != e.edge {
                renames.push(Rename { was: e.edge, now: name });
            }
        }
    }
    taken
}

#[cfg(test)]
mod tests {
    use super::*;
    use qymcad_core::names::NameTable;

    /// Edges standing nowhere in particular: where they stand tells none of them apart.
    fn bare(edges: &[u32]) -> Vec<EdgeAt> {
        edges.iter().map(|e| EdgeAt { edge: *e, at: [0.0; 3] }).collect()
    }

    /// A SEAM IS NUMBERED BY WHERE IT COMES FROM, never as the first number free. Measured on the vacuum cleaner (an edit
    /// of 10 to 11): an edge between a revolved wall F and a wall G of the next node kept through the history the name
    /// "F with itself, 0" - the seam of F once stood there - and the seam of F, named afresh, took the first number free,
    /// 1; the edit brought that history no more, the edge took the name of its pair, and the seam went from 1 to 0. The
    /// seam takes the number of the seam where F closes along U, and the edge wearing it for another pair yields it.
    #[test]
    fn a_seam_is_numbered_by_where_it_comes_from() {
        let mut table = NameTable::default();
        let f = table.intern_face(GeoName::new(5, Role::Wall, 96));
        let g = table.intern_face(GeoName::new(6, Role::Wall, 157));
        let worn = table.intern_edge(EdgeName::new(f, f, 0)); // worn by the edge running between F and G
        let seam = 7u32;
        let pairs = [EdgeFaces { edge: worn, faces: [f, g] }, EdgeFaces { edge: seam, faces: [f, f] }];
        let on = [EdgeOnItsFace { edge: seam, kind: OnItsFace::SeamAlongU, along: 500_000, across: 0 }];
        let renames = edges(&pairs, &[], &bare(&[worn, seam]), &on, &mut table);
        let now = |e: u32| renames.iter().find(|r| r.was == e).map_or(e, |r| r.now);
        assert_eq!(table.edge(now(seam)), Some(EdgeName::new(f, f, 0)), "the seam is not the seam of F along U");
        assert_eq!(table.edge(now(worn)).map(|n| n.faces), Some(EdgeName::new(f, g, 0).faces), "the edge between F and G kept the name of the seam");
    }

    /// TWO PIECES OF ONE SEAM are numbered by where each stands along it, whatever order they come in.
    #[test]
    fn the_pieces_of_a_seam_are_numbered_by_where_they_stand() {
        let mut table = NameTable::default();
        let f = table.intern_face(GeoName::new(5, Role::Wall, 96));
        let name = |first: u32, second: u32, table: &mut NameTable| -> [EdgeName; 2] {
            let pairs = [EdgeFaces { edge: first, faces: [f, f] }, EdgeFaces { edge: second, faces: [f, f] }];
            // the edge numbered 8 by the kernel stands higher along the seam than the edge numbered 9
            let on = [
                EdgeOnItsFace { edge: 8, kind: OnItsFace::SeamAlongU, along: 700_000, across: 0 },
                EdgeOnItsFace { edge: 9, kind: OnItsFace::SeamAlongU, along: 200_000, across: 0 },
            ];
            let r = edges(&pairs, &[], &bare(&[first, second]), &on, table);
            [9, 8].map(|e| table.edge(r.iter().find(|x| x.was == e).map_or(e, |x| x.now)).expect("named"))
        };
        let one_way = name(8, 9, &mut table);
        let other_way = name(9, 8, &mut table);
        assert_eq!(one_way, other_way, "the order the pieces came in numbered them");
        assert!(one_way[0].index < one_way[1].index, "the piece lower along the seam does not come first: {one_way:?}");
    }

    /// A FRESH EDGE NEVER TAKES A NAME ALREADY WORN. An edge between faces A and B wears "pair A-B, number 0"; a second
    /// edge of the same pair comes without a name. Numbered from zero, it took the very same address: highlighting took
    /// both, a fillet cut both, and nothing was left to pick one of them - what a through shell with a 2 mm wall made.
    #[test]
    fn a_fresh_edge_never_takes_a_name_already_worn() {
        let mut table = NameTable::default();
        let fa = table.intern_face(GeoName::new(5, Role::Wall, 1));
        let fb = table.intern_face(GeoName::new(5, Role::CapEnd, 0));
        let worn = table.intern_edge(EdgeName::new(fa, fb, 0));
        let pairs = [EdgeFaces { edge: worn, faces: [fa, fb] }, EdgeFaces { edge: 7, faces: [fa, fb] }];
        let renames = edges(&pairs, &[], &bare(&[worn, 7]), &[], &mut table);
        let got = renames.iter().find(|r| r.was == 7).map(|r| r.now).expect("the new edge is named");
        assert_ne!(got, worn, "the new edge took the name of its neighbour");
        assert_eq!(table.edge(got).map(|n| n.index), Some(1), "the name is not the next free number within the pair");
    }

    /// The edges of one pair are told apart by the faces at their ends, not by their numbers: swapped numbers give the
    /// same names to the same edges.
    #[test]
    fn edges_of_one_pair_are_told_by_their_ends() {
        let mut table = NameTable::default();
        let [wall, cap, floor, top] =
            [GeoName::new(5, Role::Wall, 1), GeoName::new(5, Role::CapEnd, 0), GeoName::new(6, Role::CapStart, 0), GeoName::new(6, Role::CapEnd, 0)].map(|n| table.intern_face(n));
        // the names of the upper and of the lower edge, numbered as given
        let name_of = |upper: u32, lower: u32, table: &mut NameTable| -> [u32; 2] {
            let pairs = [EdgeFaces { edge: upper, faces: [wall, cap] }, EdgeFaces { edge: lower, faces: [wall, cap] }];
            let ends = [EdgeEnds { edge: upper, around: [top, 0] }, EdgeEnds { edge: lower, around: [floor, 0] }];
            let r = edges(&pairs, &ends, &bare(&[upper, lower]), &[], table);
            [upper, lower].map(|e| r.iter().find(|x| x.was == e).map(|x| x.now).expect("named"))
        };
        let first = name_of(7, 8, &mut table);
        let swapped = name_of(8, 7, &mut table);
        assert_eq!(first, swapped, "the upper and the lower edge of one pair swapped names with their numbers");
    }

    /// EDGES OF ONE PAIR WITH THE SAME ENDS are numbered by where they stand, whatever numbers the kernel gives them.
    #[test]
    fn twins_with_the_same_ends_are_numbered_by_where_they_stand() {
        let mut table = NameTable::default();
        let [wall, cap] = [GeoName::new(5, Role::Wall, 1), GeoName::new(5, Role::CapEnd, 0)].map(|n| table.intern_face(n));
        // the edge on the left, numbered as given, and the names of the left and of the right edge
        let name_of = |left: u32, right: u32, table: &mut NameTable| -> [EdgeName; 2] {
            let pairs = [EdgeFaces { edge: left, faces: [wall, cap] }, EdgeFaces { edge: right, faces: [wall, cap] }];
            let at = [EdgeAt { edge: left, at: [-5.0, 0.0, 0.0] }, EdgeAt { edge: right, at: [5.0, 0.0, 0.0] }];
            let r = edges(&pairs, &[], &at, &[], table);
            [left, right].map(|e| table.edge(r.iter().find(|x| x.was == e).map_or(e, |x| x.now)).expect("named"))
        };
        let one_way = name_of(8, 7, &mut table);
        let other_way = name_of(7, 8, &mut table);
        assert_eq!(one_way, other_way, "the numbers of the kernel numbered the twins");
    }

    /// AN EDGE WHOSE NAME TELLS ANOTHER PAIR is named after the pair it runs between: the two long sides of a strip taken
    /// out become one edge between a rounding and a wall, and the name it kept says "beside the strip", a face gone.
    #[test]
    fn an_edge_named_beside_a_face_gone_takes_the_name_of_its_pair() {
        let mut table = NameTable::default();
        let [strip, wall, blend] = [GeoName::new(5, Role::Wall, 1), GeoName::new(5, Role::Wall, 2), GeoName::new(6, Role::Blend, 1)].map(|n| table.intern_face(n));
        let kept = table.intern_edge(EdgeName::new(strip, wall, 0));
        let renames = edges(&[EdgeFaces { edge: kept, faces: [blend, wall] }], &[], &bare(&[kept]), &[], &mut table);
        let now = renames.iter().find(|r| r.was == kept).and_then(|r| table.edge(r.now));
        assert_eq!(now, Some(EdgeName::new(blend, wall, 0)), "the edge kept a name beside a face it does not touch");
    }

    /// THE TWO HALVES OF A WALL OF A WINDOW CUT THROUGH A TUBE are named by where they stand, whichever of them the kernel
    /// hands as the original. Measured on the filter: the walls of a window through a tube each in two, the lower halves
    /// bordering only one another, the tube and its bore; they had no named face of the node beside them, came first in
    /// the order of the recipe and took the name of the whole, and the kernel handing the other half as the original
    /// after an edit moved the name of the whole across the part.
    #[test]
    fn the_halves_of_a_wall_are_named_by_where_they_stand() {
        // the bottom of the window (the wall of the cut from line 167) and its side (line 168), each in an upper and a
        // lower half; the halves of one side meet at a corner; the tube beside all of them
        let name = |origin_up: bool| -> [GeoName; 4] {
            let mut table = NameTable::default();
            let tube = table.intern_face(GeoName::new(35, Role::Wall, 30));
            let [bottom, side] = [167, 168].map(|src| table.intern_face(GeoName::new(206, Role::Wall, src)));
            // the original of each wall is the half the kernel says it is; the other half comes under a number
            let [bottom_up, bottom_down, side_up, side_down] = if origin_up { [bottom, 65, side, 66] } else { [65, bottom, 66, side] };
            let splits = [FaceSplit { piece: 65, origin: bottom, index: 0 }, FaceSplit { piece: 66, origin: side, index: 0 }];
            let pairs = [
                EdgeFaces { edge: 1, faces: [bottom_up, side_up] },
                EdgeFaces { edge: 2, faces: [bottom_down, side_down] },
                EdgeFaces { edge: 3, faces: [bottom_up, tube] },
                EdgeFaces { edge: 4, faces: [bottom_down, tube] },
                EdgeFaces { edge: 5, faces: [side_up, tube] },
                EdgeFaces { edge: 6, faces: [side_down, tube] },
            ];
            let sides = [
                PieceSide { piece: bottom_up, beside: side_up, side: Side::Back },
                PieceSide { piece: bottom_down, beside: side_down, side: Side::Back },
                PieceSide { piece: side_up, beside: bottom_up, side: Side::Back },
                PieceSide { piece: side_down, beside: bottom_down, side: Side::Back },
            ];
            let live = [tube, bottom, side, 65, 66];
            let place = |id: u32, y: f64| FacePlace { id, at: [0.0, y, 30.0], normal: [0.0, 0.0, 1.0] };
            let places = [place(bottom_up, 24.0), place(bottom_down, -24.0), place(side_up, 24.0), place(side_down, -24.0)];
            let r = pieces(&Split { splits: &splits, pairs: &pairs, sides: &sides, places: &places }, &live, 206, &mut table);
            let now = |f: u32| r.iter().find(|x| x.was == f).map_or(f, |x| x.now);
            // by where each half stands: the bottom up and down, the side up and down
            [bottom_up, bottom_down, side_up, side_down].map(|f| table.face(now(f)).expect("named"))
        };
        assert_eq!(name(true), name(false), "the half the kernel handed as the original took the name of the whole");
    }

    /// THE NAME OF THE WHOLE STAYS ON ITS HALF WHATEVER BORDERS IT. Measured on the filter: the halves of a wall of a
    /// window through a tube bordered one another when the document opened, and the walls of another window after an edit
    /// brought the two windows together; told by those other walls, the name of the whole went to the upper half, and a
    /// rounding's references to the halves were lost. The faces of other nodes beside a piece change with edits beside
    /// it; the name of the whole goes by what of the node cut the pieces off, then by where they stand.
    #[test]
    fn the_whole_stays_on_its_half_whatever_borders_it() {
        let mut table = NameTable::default();
        let tube = table.intern_face(GeoName::new(35, Role::Wall, 30));
        let side = table.intern_face(GeoName::new(206, Role::Wall, 168));
        // the walls of the other window, the upper half beside one, the lower beside another
        let [upper_wall, lower_wall] = [194, 196].map(|src| table.intern_face(GeoName::new(205, Role::Wall, src)));
        let (up, down) = (side, 69);
        let splits = [FaceSplit { piece: down, origin: side, index: 0 }];
        let pairs = [
            EdgeFaces { edge: 1, faces: [up, tube] },
            EdgeFaces { edge: 2, faces: [down, tube] },
            EdgeFaces { edge: 3, faces: [up, upper_wall] },
            EdgeFaces { edge: 4, faces: [down, lower_wall] },
        ];
        let place = |id: u32, y: f64| FacePlace { id, at: [11.0, y, 60.0], normal: [1.0, 0.0, 0.0] };
        let places = [place(up, 16.0), place(down, -16.0)];
        let live = [tube, side, upper_wall, lower_wall, down];
        let r = pieces(&Split { splits: &splits, pairs: &pairs, sides: &[], places: &places }, &live, 206, &mut table);
        let now = |f: u32| r.iter().find(|x| x.was == f).map_or(f, |x| x.now);
        assert_eq!(table.face(now(down)), Some(GeoName::new(206, Role::Wall, 168)), "the name of the whole went to the upper half, told by the walls of another node beside it");
    }

    /// A face with no history and no named neighbour is named by where it lies, and the same place at twice the scale
    /// of its import gives the same name.
    #[test]
    fn a_place_is_told_in_the_frame_of_the_file() {
        let mut table = NameTable::default();
        let at = |f: f64| [FacePlace { id: 3, at: [1.0 * f, 2.0 * f, 3.0 * f], normal: [0.0, 0.0, 1.0] }];
        let once = placed(9, 1.0, &at(1.0), &mut table);
        let twice = placed(9, 2.0, &at(2.0), &mut table);
        assert!(once.len() == 1 && once == twice, "the same face at twice the scale took another name: {once:?} / {twice:?}");
    }
}
