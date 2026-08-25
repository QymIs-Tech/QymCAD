//! A reference to geometry is a query, not a number.
//!
//! # What was there and what was missing
//!
//! Structural names already exist: [`crate::names::GeoName`] records which operation produced a face, in what
//! role and from which source entity. That is half the job, and a good half.
//!
//! The other half was missing: a reference itself remained one specific name. "Fillet this edge" — and if an
//! edit earlier in the timeline removed that edge while a similar one appeared nearby, matching by fingerprint,
//! a co-directed normal plus the nearest centroid, attached to the neighbour **silently**. Sometimes it guessed
//! right. Sometimes it did not, and then the fillet moved to a different edge while the cause looked like user
//! error.
//!
//! # What replaces it
//!
//! A reference describes an intent and is evaluated afresh on every rebuild:
//!
//! * `Id` — this specific one, a manual pick;
//! * `OfFeature` — everything this feature produced, optionally narrowed by role;
//! * `FromSource` — whatever grew out of this sketch entity;
//! * `Oriented`, `Extreme`, `Largest` — descriptive, from the live geometry;
//! * `Union` / `Minus` / `Filter` — built from sets.
//!
//! # The three rules this exists for
//!
//! 1. The result is a **set**, not an element. How many are expected is declared by the feature itself.
//! 2. **Ambiguity is a refusal, not a guess.** One face was expected and three were found, so the feature goes
//!    red with the count. Taking the first one and staying quiet is precisely what is being removed.
//! 3. **A loss is a refusal with an explanation**: what was sought and what it was at the moment it was picked.
//!    The fingerprint remains, but it serves the reader inside the error text rather than a matcher.

use crate::names::{NameTable, Role};
use serde::{Deserialize, Serialize};

/// A fingerprint taken at the moment of picking, kept for the reader of a refusal and for nothing else.
///
/// The old "nearest co-directed" matching lived on exactly these numbers and got them wrong silently. Here they
/// exist so that a refusal can read "the top face of the extrusion was sought and is gone; when it was picked it
/// was here and faced that way" rather than merely "the reference is lost".
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Fingerprint {
    pub centroid: [f64; 3],
    pub normal: [f64; 3],
}

/// How many elements the feature expects. The feature declares this itself: "one edge" and "every edge of this
/// face" are different intents and must not be confused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Cardinality {
    /// Exactly one. Two is a refusal.
    #[default]
    One,
    /// At least one; the exact count does not matter.
    Some,
    /// Any number, zero included, as in "every through hole".
    Any,
}

/// The axis used by `Extreme`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Axis {
    X,
    Y,
    Z,
}

/// How to find geometry, from the exact to the descriptive.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Query {
    /// One specific descriptor: a manual pick.
    Id(u32),
    /// Several descriptors in a single list: what was picked with the mouse.
    ///
    /// Flat, and deliberately so. A selection used to fold into a ladder of `Union(Union(...))` whose depth
    /// grew with the number of picked elements. At around fifty edges the document stopped being readable: it
    /// saved and would not open again. How many elements someone picked must not affect the structure of the
    /// file.
    Ids(Vec<u32>),
    /// Everything the feature produced; `role` narrows it, as in walls only or the cap only.
    OfFeature { feature: crate::model::Id, role: Option<Role> },
    /// Whatever grew out of this source entity: a sketch edge or a body edge.
    FromSource { src: crate::model::Id },
    /// By the direction of the normal: `dot(n, dir) >= cos(tol)`.
    Oriented { dir: [f64; 3], tol_deg: f64 },
    /// The extreme one along an axis: the topmost, the leftmost.
    Extreme { axis: Axis, max: bool },
    /// The largest by area.
    Largest,
    /// The edges touching these faces with at least one side.
    ///
    /// "Fillet every edge of this face" is the most common selection there is, and a list of numbers cannot
    /// express it: trim the face and there are more edges, while the list stays as it was.
    Adjacent(Box<Query>),
    /// A tangent chain: the edges that continue the seed smoothly.
    ///
    /// The most common selection when filleting: click one edge and the whole run around the part comes with
    /// it. A list cannot express it for the same reason as "every edge of this face": trim the shape and the
    /// chain is different while the list stays as it was. Elsewhere this is called selecting all tangentially
    /// connected edges.
    ///
    /// Smoothness is tested at the vertex and by tangents rather than by position: two edges join the chain if
    /// they share an endpoint and their tangents there are collinear within a tolerance.
    TangentChain { seed: Box<Query>, tol_deg: f64 },
    /// The edges where two sets of faces meet: one side of the edge in the first set, the other in the second.
    ///
    /// "Fillet where the boss meets the plate" is exactly this. Raise the boss and the junction changes in
    /// length and in edge count, while the description stays correct.
    Between(Box<Query>, Box<Query>),
    /// The union of two sets.
    Union(Box<Query>, Box<Query>),
    /// Subtraction: everything in the first set that is not in the second.
    Minus(Box<Query>, Box<Query>),
    /// Narrowing: the elements of the first set that also appear in the second.
    Filter(Box<Query>, Box<Query>),
}

/// A reference in full: what to search with, how many are expected, and what it was.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ref {
    pub query: Query,
    pub expect: Cardinality,
    #[serde(default)]
    pub hint: Fingerprint,
}

impl Ref {
    /// A manual pick: one specific face, with exactly one expected.
    pub fn one(desc: u32, hint: Fingerprint) -> Ref {
        Ref { query: Query::Id(desc), expect: Cardinality::One, hint }
    }
    /// A set: however many are found is how many are taken.
    pub fn many(query: Query) -> Ref {
        Ref { query, expect: Cardinality::Any, hint: Fingerprint::default() }
    }

    /// A set of manual picks: what someone produces by clicking faces with the mouse.
    ///
    /// This is a union of `Id`s and behaves exactly like the old list of numbers: it does not grow with the
    /// model. For a set to grow it has to be described, as in "every top face of this feature"; separate
    /// selection commands will bring that into the interface. What matters here is that both are expressible
    /// by one type.
    pub fn picks(descs: &[u32]) -> Ref {
        // A flat list rather than a ladder. A selection used to fold into `Union(Union(...))`: 52 edges gave
        // 51 levels of nesting and the document reader hit its depth limit. The file saved and would not open
        // again, so picking 52 edges for a fillet and saving cost access to the project. Depth must not depend
        // on how many elements someone picked with the mouse.
        let query = match descs {
            [] => Query::Id(0),
            [one] => Query::Id(*one),
            many => Query::Ids(many.to_vec()),
        };
        Ref { query, expect: if descs.is_empty() { Cardinality::Any } else { Cardinality::Some }, hint: Fingerprint::default() }
    }
}

impl Query {
    /// Walk every descriptor of a query, used to carry names across when features are copied by a pattern or
    /// a mirror.
    ///
    /// A query has no single number: it may contain none at all, as `Oriented` does, or several, as a union of
    /// two `Id`s does. Carrying names across is therefore a traversal rather than an assignment.
    pub fn remap_descs(&mut self, f: &mut impl FnMut(&mut u32)) {
        match self {
            Query::Id(d) => f(d),
            Query::Ids(v) => v.iter_mut().for_each(f),
            Query::Union(a, b) | Query::Minus(a, b) | Query::Filter(a, b) | Query::Between(a, b) => {
                a.remap_descs(f);
                b.remap_descs(f);
            }
            Query::Adjacent(a) => a.remap_descs(f),
            Query::TangentChain { seed, .. } => seed.remap_descs(f),
            Query::OfFeature { .. } | Query::FromSource { .. } | Query::Oriented { .. } | Query::Extreme { .. } | Query::Largest => {}
        }
    }
}

impl Ref {
    /// The same for a reference as a whole.
    pub fn remap_descs(&mut self, f: &mut impl FnMut(&mut u32)) {
        self.query.remap_descs(f);
    }
}

impl Query {
    /// Whether a query returns edges or faces.
    ///
    /// What makes a query edge-valued is the question rather than the kind of element it names: "adjacent to
    /// these faces" and "where these two sets meet" ask about edges although they are phrased through faces.
    /// The interface needs this to show the result with the right highlight.
    pub fn yields_edges(&self) -> bool {
        matches!(self, Query::Adjacent(_) | Query::Between(_, _) | Query::TangentChain { .. })
    }
}

impl Query {
    /// Is this a plain list of picks, that is an `Id` or a union of `Id`s?
    ///
    /// The distinction has to be drawn by the kind of query, not by whether numbers appear inside it.
    /// `Adjacent(Id(face))` does carry a number, but that number names a face while the query itself is about
    /// edges. Confusing the two feeds the kernel a face number where an edge was expected, which ends in a
    /// segmentation fault: the program crashed on "shell, then fillet one of its faces".
    pub fn is_pick_list(&self) -> bool {
        match self {
            Query::Id(_) | Query::Ids(_) => true,
            Query::Union(a, b) => a.is_pick_list() && b.is_pick_list(),
            _ => false,
        }
    }

    /// The explicit descriptors of a query: the ones picked with the mouse.
    ///
    /// Note that for a descriptive query these are the numbers of what it is phrased through — faces, in the
    /// case of `Adjacent` — and not of what it will return. Deciding between a pick and a description is the
    /// job of [`Query::is_pick_list`], not of whether this list is empty.
    ///
    /// Used by the interface to highlight the selected faces. A descriptive query such as "every top face" has
    /// none, and the empty list is honest: there is nothing to highlight from it until the body is rebuilt.
    pub fn picked_descs(&self) -> Vec<u32> {
        let mut out = Vec::new();
        let mut q = self.clone();
        q.remap_descs(&mut |d| {
            if *d != 0 {
                out.push(*d);
            }
        });
        out
    }
}

/// Why a reference did not resolve. A refusal always names the reason; the silent guess is gone.
#[derive(Clone, Debug, PartialEq)]
pub enum RefError {
    /// Nothing was found where at least something was expected.
    Lost { what: String, was: Fingerprint },
    /// More was found than expected.
    Ambiguous { what: String, found: usize },
}

impl RefError {
    /// A key into the language catalogue rather than a finished phrase.
    ///
    /// The core has no language: the application supplies the words. An earlier version wrote localised text
    /// straight into `Display`, and the translation ratchet went red over it, which is exactly what it is for.
    /// Same approach as `ExprError`: the core returns the kind of error and the data, the application the
    /// sentence.
    pub fn key(&self) -> &'static str {
        match self {
            RefError::Lost { .. } => "ref-lost",
            RefError::Ambiguous { .. } => "ref-ambiguous",
        }
    }

    /// What the reference itself is called: a key the application translates, such as the edge of a fillet.
    pub fn what(&self) -> &str {
        match self {
            RefError::Lost { what, .. } | RefError::Ambiguous { what, .. } => what,
        }
    }
}

/// The geometry an edge has and a face does not: its endpoints and, if the edge is circular, its circle.
///
/// Needed by the queries about junctions. Whether one edge continues another smoothly is decided at the vertex,
/// and the midpoint data (`centroid` and `normal`) cannot answer it: the tangent of an arc at its middle points
/// elsewhere than at its end, and two perfectly tangent arcs would look like a kink.
#[derive(Clone, Copy, Debug, Default)]
pub struct EdgeGeom {
    pub a: [f64; 3],
    pub b: [f64; 3],
    /// Centre and axis of the circle; `radius == 0` means the edge is not circular, being straight or a
    /// spline.
    pub center: [f64; 3],
    pub axis: [f64; 3],
    pub radius: f64,
}

impl EdgeGeom {
    /// The unit tangent at point `p`; its sign does not matter.
    ///
    /// For an arc it is computed exactly, from the axis and the radius vector; for a straight edge it is the
    /// direction of the edge. Between them these two cases cover practically all the geometry this system
    /// builds.
    pub fn tangent_at(&self, p: [f64; 3]) -> [f64; 3] {
        let norm = |v: [f64; 3]| {
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            if l < 1e-12 {
                [0.0, 0.0, 0.0]
            } else {
                [v[0] / l, v[1] / l, v[2] / l]
            }
        };
        if self.radius > 1e-9 {
            let r = [p[0] - self.center[0], p[1] - self.center[1], p[2] - self.center[2]];
            return norm([
                self.axis[1] * r[2] - self.axis[2] * r[1],
                self.axis[2] * r[0] - self.axis[0] * r[2],
                self.axis[0] * r[1] - self.axis[1] * r[0],
            ]);
        }
        norm([self.b[0] - self.a[0], self.b[1] - self.a[1], self.b[2] - self.a[2]])
    }

    /// The vertex shared with another edge, or `None`.
    pub fn shared_vertex(&self, other: &EdgeGeom) -> Option<[f64; 3]> {
        let close = |x: [f64; 3], y: [f64; 3]| {
            (x[0] - y[0]).abs() < 1e-6 && (x[1] - y[1]).abs() < 1e-6 && (x[2] - y[2]).abs() < 1e-6
        };
        for p in [self.a, self.b] {
            for q in [other.a, other.b] {
                if close(p, q) {
                    return Some(p);
                }
            }
        }
        None
    }
}

/// One face or edge as the resolver sees it: a descriptor plus what the live geometry says about it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Candidate {
    pub desc: u32,
    pub centroid: [f64; 3],
    pub normal: [f64; 3],
    pub area: f64,
    /// Filled in for edges only: a face has no endpoints.
    pub edge: Option<EdgeGeom>,
}

/// Resolve a query against the live geometry.
///
/// `pool` holds every face, or every edge, of the body as it is right now, and `names` is the name table of the
/// document. No state is carried between calls: the query is evaluated afresh every time, which is the whole
/// point.
pub fn resolve(query: &Query, pool: &[Candidate], names: &NameTable, face_pool: &[Candidate]) -> Vec<u32> {
    match query {
        Query::Id(d) => {
            let hit: Vec<u32> = pool.iter().filter(|c| c.desc == *d).map(|c| c.desc).collect();
            if !hit.is_empty() {
                return hit;
            }
            // An absorbed name leads to the shared face. Merging coplanar faces collapses two named faces
            // into one and makes one name yield. A name that yielded is not lost: the kernel reported which
            // name it yielded to, and the reference follows. This is a translation of the name by a recorded
            // merge, not a search for something similar in the geometry.
            match names.absorbed_target(*d) {
                Some(t) => pool.iter().filter(|c| c.desc == t).map(|c| c.desc).collect(),
                None => Vec::new(),
            }
        }
        Query::OfFeature { feature, role } => pool
            .iter()
            .filter(|c| match names.get(c.desc) {
                Some(n) => n.feature == *feature && role.is_none_or(|r| n.role == r),
                None => false,
            })
            .map(|c| c.desc)
            .collect(),
        Query::FromSource { src } => pool
            .iter()
            .filter(|c| names.get(c.desc).is_some_and(|n| n.src == *src))
            .map(|c| c.desc)
            .collect(),
        Query::Oriented { dir, tol_deg } => {
            let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt().max(1e-12);
            let cos_tol = tol_deg.to_radians().cos();
            pool.iter()
                .filter(|c| {
                    let d = (c.normal[0] * dir[0] + c.normal[1] * dir[1] + c.normal[2] * dir[2]) / len;
                    d >= cos_tol
                })
                .map(|c| c.desc)
                .collect()
        }
        Query::Extreme { axis, max } => {
            let k = match axis {
                Axis::X => 0,
                Axis::Y => 1,
                Axis::Z => 2,
            };
            // The extreme is a set rather than a single element: a box can have several top faces, and
            // claiming one of them is the silent guess all over again. Let `expect` decide.
            let best = pool.iter().fold(None::<f64>, |acc, c| {
                let v = c.centroid[k];
                Some(match acc {
                    None => v,
                    Some(b) => {
                        if *max {
                            b.max(v)
                        } else {
                            b.min(v)
                        }
                    }
                })
            });
            match best {
                None => Vec::new(),
                Some(b) => pool.iter().filter(|c| (c.centroid[k] - b).abs() < 1e-6).map(|c| c.desc).collect(),
            }
        }
        Query::Largest => {
            let best = pool.iter().map(|c| c.area).fold(f64::MIN, f64::max);
            pool.iter().filter(|c| (c.area - best).abs() < 1e-9 * best.abs().max(1.0)).map(|c| c.desc).collect()
        }
        // Edges through their own faces. An edge has no recipe of its own: it is where two surfaces meet, and
        // its name is the pair of their names (`names::EdgeName`). A query about edges is therefore always
        // phrased through faces rather than through a feature of the edge, which does not exist.
        Query::Adjacent(faces) => {
            let want = resolve(faces, face_pool, names, face_pool);
            pool.iter()
                .filter(|c| names.edge(c.desc).is_some_and(|e| e.faces.iter().any(|f| want.contains(f))))
                .map(|c| c.desc)
                .collect()
        }
        // The chain grows from its seeds while there is anywhere to grow, as a wave across shared vertices.
        // The traversal walks the edge pool itself, so the query works even where no names exist yet.
        Query::TangentChain { seed, tol_deg } => {
            let start = resolve(seed, pool, names, face_pool);
            let cos_tol = tol_deg.to_radians().cos();
            let mut out: Vec<u32> = start.clone();
            let mut queue: Vec<u32> = start;
            while let Some(d) = queue.pop() {
                let Some(cur) = pool.iter().find(|c| c.desc == d) else { continue };
                let Some(ge) = cur.edge else { continue };
                for other in pool {
                    if out.contains(&other.desc) {
                        continue;
                    }
                    let Some(go) = other.edge else { continue };
                    let Some(v) = ge.shared_vertex(&go) else { continue };
                    let (t1, t2) = (ge.tangent_at(v), go.tangent_at(v));
                    let dot = (t1[0] * t2[0] + t1[1] * t2[1] + t1[2] * t2[2]).abs();
                    if dot >= cos_tol {
                        out.push(other.desc);
                        queue.push(other.desc);
                    }
                }
            }
            out
        }
        Query::Between(a, b) => {
            let (sa, sb) = (resolve(a, face_pool, names, face_pool), resolve(b, face_pool, names, face_pool));
            pool.iter()
                .filter(|c| {
                    names.edge(c.desc).is_some_and(|e| {
                        let (f0, f1) = (e.faces[0], e.faces[1]);
                        (sa.contains(&f0) && sb.contains(&f1)) || (sa.contains(&f1) && sb.contains(&f0))
                    })
                })
                .map(|c| c.desc)
                .collect()
        }
        Query::Ids(v) => {
            let mut out: Vec<u32> = Vec::new();
            for &d in v {
                for r in resolve(&Query::Id(d), pool, names, face_pool) {
                    if !out.contains(&r) {
                        out.push(r);
                    }
                }
            }
            out
        }
        Query::Union(a, b) => {
            let mut out = resolve(a, pool, names, face_pool);
            for d in resolve(b, pool, names, face_pool) {
                if !out.contains(&d) {
                    out.push(d);
                }
            }
            out
        }
        Query::Minus(a, b) => {
            let drop = resolve(b, pool, names, face_pool);
            resolve(a, pool, names, face_pool).into_iter().filter(|d| !drop.contains(d)).collect()
        }
        Query::Filter(a, b) => {
            let keep = resolve(b, pool, names, face_pool);
            resolve(a, pool, names, face_pool).into_iter().filter(|d| keep.contains(d)).collect()
        }
    }
}

impl Ref {
    /// Resolve a reference, checking the count and refusing honestly.
    ///
    /// `what` is the catalogue key naming the reference itself, such as `ref-what-fillet-edge`. The core stores
    /// the key and the application supplies the sentence.
    pub fn resolve(&self, what: &str, pool: &[Candidate], names: &NameTable, face_pool: &[Candidate]) -> Result<Vec<u32>, RefError> {
        let found = resolve(&self.query, pool, names, face_pool);
        match (self.expect, found.len()) {
            (Cardinality::One, 1) => Ok(found),
            (Cardinality::One, 0) | (Cardinality::Some, 0) => Err(RefError::Lost { what: what.into(), was: self.hint }),
            (Cardinality::One, n) => Err(RefError::Ambiguous { what: what.into(), found: n }),
            _ => Ok(found),
        }
    }
}

#[cfg(test)]
mod tests;
