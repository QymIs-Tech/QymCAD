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

use qymcad_core::feature::{EdgeEnds, EdgeFaces, FacePlace, FaceSplit, NamedAfter, Rename, SilentNaming};
use qymcad_core::model::Id;
use qymcad_core::names::{EdgeName, GeoName, NameMint, Role};
use std::collections::{BTreeMap, HashMap, HashSet};

/// NAME WHAT THE HISTORY OF `ask.body` WAS SILENT ABOUT, in the live body and in the answer.
pub(crate) fn silent(kernel: &crate::OcctKernel, ask: SilentNaming<'_>) -> NamedAfter {
    let SilentNaming { body, feature, listed, scale, names } = ask;
    let shapes = kernel.shapes.borrow();
    let Some(shape) = shapes.get(&body) else { return NamedAfter::default() };
    // the faces as the document lists them; for a body raised again from its file, as the kernel holds them
    let mut live: Vec<u32> = match listed {
        Some(list) => list.to_vec(),
        None => shape.face_places().iter().map(|p| p.id).collect(),
    };
    let was = live.clone();
    let mut named = NamedAfter::default();
    // THE ROLES FIRST: the faces the operation made under numbers awaiting their names take them from the table now,
    // and a piece of such a face is told after the name its whole took
    let roles = kernel.roles.borrow_mut().remove(&body).unwrap_or_default();
    let held: HashSet<u32> = live.iter().copied().chain(shape.absorbed_names().into_iter().flat_map(|a| [a.loser, a.winner])).chain(shape.face_splits().into_iter().map(|s| s.origin)).collect();
    let landed = land_roles(&roles, &held, names);
    land(shape, &landed, &mut live);
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
        let renames = pieces(&splits, &shape.edge_face_pairs(), &live, feature, names);
        land(shape, &renames, &mut live);
    }
    // in waves: a face in a corner has orphans for neighbours, and the faces the first wave names anchor the second
    // (measured: on a shell of a shell exactly four corner faces stayed nameless in one pass)
    for _ in 0..8 {
        let renames = seams(feature, &shape.edge_face_pairs(), &live, names);
        if renames.is_empty() {
            break;
        }
        land(shape, &renames, &mut live);
    }
    // asked of the list first, which costs nothing; a body raised from its file is always asked of the kernel
    if listed.is_none() || !live.iter().all(|d| names.knows(*d)) {
        let renames = placed(feature, scale, &shape.face_places(), names);
        land(shape, &renames, &mut live);
    }
    // what each face the document lists became, over all the waves: one renaming the document applies at once
    named.faces = was.iter().zip(&live).filter(|(w, n)| w != n).map(|(w, n)| Rename { was: *w, now: *n }).collect();
    let pairs = shape.edge_face_pairs();
    if !pairs.is_empty() {
        let renames = edges(&pairs, &shape.edge_end_faces(), &shape.edges_with_ids().1, names);
        // a name that told another pair yielded to the name of the pair: a reference to it, or to a piece of its pair,
        // follows the record in every later rebuild, not only in this one
        named.absorbed.extend(renames.iter().filter(|r| names.edge(r.was).is_some()).map(|r| qymcad_core::feature::Absorbed { loser: r.was, winner: r.now }));
        shape.rename_edges(&renames);
        named.edges = renames;
    }
    named
}

/// THE FACES OF ONE WAVE RENAMED, in the live body and in the list of live faces - all at once: one wave gives a piece
/// the name of its whole and the whole a name of its own, and applied one after the other the second took both
/// (measured: the two strips of a pinched rim under one name, nothing left to select one with).
fn land(shape: &crate::Shape, renames: &[Rename], live: &mut [u32]) {
    shape.rename_faces(renames);
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

/// The named faces beside each face, by the edges between them.
fn neighbours(pairs: &[EdgeFaces]) -> HashMap<u32, Vec<u32>> {
    let mut nb: HashMap<u32, Vec<u32>> = HashMap::new();
    for p in pairs {
        nb.entry(p.faces[0]).or_default().push(p.faces[1]);
        nb.entry(p.faces[1]).or_default().push(p.faces[0]);
    }
    nb
}

/// THE PIECES OF A SPLIT FACE, after the face they were cut from. Which piece keeps the name of the whole is told by the
/// recipe, not by the order of the history, which shifts with any edit above: by what cut it - the faces beside it the
/// node itself made, the walls of the tool of a cut, the blends of a rounding - and then by the other named faces beside
/// it, each list in the order of the recipe (`InRecipe`), not of the table. Measured with every neighbour by its
/// descriptor: an edit above took a rounding of an older node away from one of two pieces of a wall, and the piece that
/// did not move gave its name to the other (16 of 36 fillet references once landed on the wrong edges with the order of
/// the history). The number of a face is the last separator, where the recipe is silent.
pub(crate) fn pieces(splits: &[FaceSplit], pairs: &[EdgeFaces], live: &[u32], feature: Id, names: &mut dyn NameMint) -> Vec<Rename> {
    let nb = neighbours(pairs);
    let live: HashSet<u32> = live.iter().copied().collect();
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
        let key = |f: u32| -> PieceKey {
            let mut key = PieceKey::default();
            for n in nb.get(&f).into_iter().flatten().filter(|n| !own.contains(n)) {
                let Some(g) = names.face(*n) else { continue };
                let at = InRecipe { feature: g.feature, role: g.role as u8, src: g.src, split: g.split };
                if g.feature == feature {
                    key.cut_by.push(at)
                } else {
                    key.beside.push(at)
                }
            }
            for list in [&mut key.cut_by, &mut key.beside] {
                list.sort_unstable();
                list.dedup();
            }
            key
        };
        members.sort_by(|a, b| key(*a).cmp(&key(*b)).then(a.cmp(b)));
        for (k, f) in members.iter().enumerate() {
            let split = k.min(u16::MAX as usize) as u16;
            // the split ordinal of a copy of a pattern holds its copy number, of a seam its seam number: written over,
            // five pieces of a copied face came under one name; so their whole is the face itself, "piece k of origin"
            let name = if base.split != 0 || base.role == Role::Seam {
                if k == 0 {
                    base
                } else {
                    GeoName { feature: base.feature, role: base.role, src: origin as Id, split }
                }
            } else {
                GeoName { split, ..base }
            };
            let d = names.intern_face(name);
            if *f != d {
                renames.push(Rename { was: *f, now: d });
            }
        }
    }
    renames
}

/// WHAT TELLS A PIECE OF A SPLIT FACE: the faces the node made beside it, then the other named faces beside it.
#[derive(Default, PartialEq, Eq, PartialOrd, Ord)]
struct PieceKey {
    cut_by: Vec<InRecipe>,
    beside: Vec<InRecipe>,
}

/// A NAME IN THE ORDER OF ITS RECIPE: the node, the role, the source, the piece - not the place the table holds it at.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
            let d = names.intern_face(GeoName { feature, role: Role::Seam, src: host as Id, split: k.min(u16::MAX as usize) as u16 });
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
            let d = names.intern_face(GeoName { feature, role: Role::Placed, src: key, split: k.min(u16::MAX as usize) as u16 });
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

/// THE EDGES, after the pair of faces they run between and their number within the pair. An edge between two named
/// faces is named whatever the rest of the body is (measured: one nameless face cancelling the whole body left 190
/// edges positional). Edges of one pair are ordered by the named faces at their ends - the upper and the lower edge of
/// a slot between the same walls, the cap at one and the floor at the other - and the number of an edge is the last
/// separator. The number within a pair is the first one free on the body: a newcomer of a pair given the number a
/// neighbour wears made two edges one address (measured: 8 of 24 edges of a shelled pocket).
///
/// AN EDGE NAMED BESIDE A FACE GONE FROM THE BODY is named again by the pair it runs between. A strip taken out between a
/// rounding and a wall makes its two long sides one edge, and whichever of the two names it kept, that name says "beside
/// the strip". The renaming goes to the document with the others, the old name recorded as yielded to the new one.
/// Measured on a sample: with the smaller name kept, the rounding above the strip lost two of its four edges. A name of
/// another pair whose faces are all in the body - a piece for its whole, a wall a cut put along a seam - stays: a
/// reference to it is read through its pair (`NameTable::canonical_edge`), and renaming those too left 28 edges of a
/// rounding of a sample unfound after an edit.
pub(crate) fn edges(pairs: &[EdgeFaces], ends: &[EdgeEnds], body_edges: &[u32], names: &mut dyn NameMint) -> Vec<Rename> {
    // the names worn on the whole body, not only by the edges with a pair: a seam and a degenerate edge have none, and
    // their names stayed free to be handed out twice
    let mut worn: HashSet<u32> = body_edges.iter().chain(pairs.iter().map(|p| &p.edge)).copied().filter(|e| names.knows(*e)).collect();
    let mut by_pair: BTreeMap<[u32; 2], Vec<u32>> = BTreeMap::new();
    let faces: HashSet<u32> = pairs.iter().flat_map(|p| p.faces).collect();
    for p in pairs {
        let [a, b] = p.faces;
        // nowhere to take a name from; or named through the history, and the name tells no face gone from the body
        if !names.knows(a) || !names.knows(b) {
            continue;
        }
        if names.knows(p.edge) && names.edge(p.edge).is_none_or(|n| n.faces.iter().all(|f| faces.contains(f))) {
            continue;
        }
        by_pair.entry([a.min(b), a.max(b)]).or_default().push(p.edge);
    }
    let around: HashMap<u32, [u32; 2]> = ends.iter().map(|e| (e.edge, e.around)).collect();
    let mut renames: Vec<Rename> = Vec::new();
    for ([a, b], mut es) in by_pair {
        es.sort_unstable_by(|x, y| around.get(x).copied().unwrap_or([0, 0]).cmp(&around.get(y).copied().unwrap_or([0, 0])).then(x.cmp(y)));
        let mut next = 0u16;
        for e in es {
            let name = loop {
                let n = names.intern_edge(EdgeName::new(a, b, next));
                next = next.saturating_add(1);
                if !worn.contains(&n) {
                    break n;
                }
            };
            worn.insert(name);
            renames.push(Rename { was: e, now: name });
        }
    }
    renames
}

#[cfg(test)]
mod tests {
    use super::*;
    use qymcad_core::names::NameTable;

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
        let renames = edges(&pairs, &[], &[worn, 7], &mut table);
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
            let r = edges(&pairs, &ends, &[upper, lower], table);
            [upper, lower].map(|e| r.iter().find(|x| x.was == e).map(|x| x.now).expect("named"))
        };
        let first = name_of(7, 8, &mut table);
        let swapped = name_of(8, 7, &mut table);
        assert_eq!(first, swapped, "the upper and the lower edge of one pair swapped names with their numbers");
    }

    /// AN EDGE WHOSE NAME TELLS ANOTHER PAIR is named after the pair it runs between: the two long sides of a strip taken
    /// out become one edge between a rounding and a wall, and the name it kept says "beside the strip", a face gone.
    #[test]
    fn an_edge_named_beside_a_face_gone_takes_the_name_of_its_pair() {
        let mut table = NameTable::default();
        let [strip, wall, blend] = [GeoName::new(5, Role::Wall, 1), GeoName::new(5, Role::Wall, 2), GeoName::new(6, Role::Blend, 1)].map(|n| table.intern_face(n));
        let kept = table.intern_edge(EdgeName::new(strip, wall, 0));
        let renames = edges(&[EdgeFaces { edge: kept, faces: [blend, wall] }], &[], &[kept], &mut table);
        let now = renames.iter().find(|r| r.was == kept).and_then(|r| table.edge(r.now));
        assert_eq!(now, Some(EdgeName::new(blend, wall, 0)), "the edge kept a name beside a face it does not touch");
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
