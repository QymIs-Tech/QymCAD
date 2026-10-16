//! FILLETS AND CHAMFERS through the door: what is their own - which edges, at what size, what the faces they make are
//! called - and nothing of what the door does for every operation.

use crate::door::{Bodies, Operation, Retreat, Roles, Sizing, StepBack};
use crate::kernel::{refused, OcctKernel};
use qymcad_core::errors::{CoreError, FilletEdgeIssue, Op};
use qymcad_core::feature::{BlendRoles, DroppedEdges};
use qymcad_core::geom::Built;
use qymcad_core::model::{ChamferShape, Id};
use qymcad_core::names::{GeoName, NameTable, Role};

/// A BLEND AS ASKED: the body it makes, the body it works on, its size (a radius or a leg), the edges (none - every
/// edge), and whether the faces it makes are named.
pub struct BlendAsk<'a> {
    pub body: Id,
    pub src: Id,
    pub size: f64,
    pub edges: &'a [u32],
    pub roles: BlendRoles,
}

/// AN EDGE TO BLEND and the number its surface is handed under.
#[derive(Clone, Copy)]
struct EdgeRole {
    edge: u32,
    surface: u32,
}

/// THE NUMBERS THE FACES OF A BLEND ARE HANDED UNDER, as the C++ side reads them: a surface per edge, a corner patch per
/// edge, and for every edge of the body the surface the blend makes if it continues across it.
struct BlendNumbers {
    surfaces: Vec<u32>,
    corners: Vec<u32>,
    all: Vec<u32>,
}

impl BlendNumbers {
    /// The numbers of the surfaces of `edges`, taken from those of the edges asked: an edge left out takes its number out
    /// with it (filtering the edges alone shifted every name after it onto the next edge).
    fn surfaces_of(&self, asked: &[EdgeRole], edges: &[u32]) -> Vec<u32> {
        if self.surfaces.is_empty() {
            return Vec::new();
        }
        edges.iter().map(|e| asked.iter().find(|a| a.edge == *e).map_or(0, |a| a.surface)).collect()
    }
}

/// THE ROLES OF A BLEND. The surface of a fillet or a chamfer is named after the edge it rounds, the patch closing a
/// corner after the edge too, there being no other source in the recipe. The surface continues across tangent
/// neighbours, which the recipe also names after their edges (measured: otherwise 2 to 4 nameless faces an operation)
/// - but only an edge that can be told apart: while two edges of a body share one number, naming their surfaces alike
///   makes a reference lead to two faces (measured: a mirror gave 7 such pairs of 20 edges).
fn blend_roles(roles: BlendRoles, edges: &[u32], body_edges: &[u32], given: &mut Roles) -> BlendNumbers {
    let BlendRoles::Named { feature } = roles else { return BlendNumbers { surfaces: Vec::new(), corners: Vec::new(), all: Vec::new() } };
    let surfaces = edges.iter().map(|e| given.give(GeoName::new(feature, Role::Blend, *e as Id))).collect();
    let corners = edges.iter().map(|e| given.give(GeoName::new(feature, Role::Corner, *e as Id))).collect();
    let once = |e: &u32| NameTable::is_named(*e) && body_edges.iter().filter(|x| *x == e).count() == 1;
    let mut all = Vec::new();
    for e in body_edges.iter().filter(|e| once(e)) {
        all.push(*e);
        all.push(given.give(GeoName::new(feature, Role::Blend, *e as Id)));
    }
    BlendNumbers { surfaces, corners, all }
}

/// The edges to blend with the smooth ones taken out - the tangent junctions of earlier blends have nothing left to
/// round, and the kernel honestly fails on them - and "every edge" of a body with a smooth one read as every sharp
/// edge by name. `None` when edges were asked and every one is smooth.
fn sharp(s: &crate::Shape, asked: &[u32]) -> Result<SharpEdges, CoreError> {
    let smooth = s.smooth_edge_ids();
    // AN EDGE THE BODY DOES NOT HAVE IS NOT HANDED TO OCCT: a number of another body, or one a rebuild above took away,
    // was told "the radius is too big", and the model filtered such numbers itself, OCCT having crashed on one
    let known: std::collections::HashSet<u32> = s.edges_with_ids().1.into_iter().collect();
    let unknown = asked.iter().filter(|e| !known.contains(e)).count();
    if !asked.is_empty() && unknown == asked.len() {
        return Err(CoreError::EdgesNotFound { asked: asked.len() });
    }
    let edges: Vec<u32> = if asked.is_empty() && !smooth.is_empty() { s.sharp_edge_ids() } else { asked.iter().copied().filter(|e| known.contains(e) && !smooth.contains(e)).collect() };
    // only when edges were asked for: an empty list asks for every edge, and a body with any smooth one - the seam of a
    // tube - was refused whole, its sharp edges with it
    if !asked.is_empty() && edges.is_empty() {
        return Err(CoreError::AllEdgesSmooth);
    }
    Ok(SharpEdges { edges, smooth: smooth.len(), unknown })
}

/// The edges a blend takes, how many smooth ones it passed over, and how many asked the body does not have.
struct SharpEdges {
    edges: Vec<u32>,
    smooth: usize,
    unknown: usize,
}

impl SharpEdges {
    /// The edges asked, and how many of them were left out when `taken` of those it took were blended.
    fn left_out(&self, taken: usize) -> DroppedEdges {
        DroppedEdges { asked: self.edges.len() + self.unknown, dropped: self.edges.len() - taken + self.unknown }
    }
}

impl OcctKernel {
    /// A FILLET OF ONE RADIUS. At the limiting case the door steps it back; if the edges do not take together, each is
    /// tried alone and the answer says which refuse this radius and which radius they do accept, and the rest are
    /// rounded - as many as are possible, the ones left out said in yellow rather than a wholly red node.
    pub(crate) fn fillet_op(&self, ask: BlendAsk<'_>) -> Result<Built, CoreError> {
        let BlendAsk { body, src, size: radius, edges, roles } = ask;
        self.dropped.borrow_mut().remove(&body);
        let Prepared { sharp, numbers, given } = self.prepare(src, edges, roles)?;
        let asked: Vec<EdgeRole> = sharp.edges.iter().zip(numbers.surfaces.iter().chain(std::iter::repeat(&0))).map(|(edge, surface)| EdgeRole { edge: *edge, surface: *surface }).collect();
        let edges = &sharp.edges;
        let whole = |b: &Bodies, f: f64| -> Result<crate::Shape, CoreError> {
            let s = b.get(src)?;
            let r = if edges.is_empty() {
                s.fillet_all(radius * f)
            } else {
                s.fillet_edges_named(radius * f, edges, &numbers.surfaces_of(&asked, edges), &numbers.corners, &numbers.all)
            };
            r.ok_or(CoreError::OpFailed(Op::Fillet))
        };
        let first = self.run(Operation { kind: Op::Fillet, body, sizing: Sizing::Sized(StepBack { retreat: Retreat::Blend, size: radius }), roles: given.clone(), work: whole });
        if first.is_ok() && sharp.unknown > 0 {
            self.dropped.borrow_mut().insert(body, sharp.left_out(sharp.edges.len()));
        }
        match first {
            Err(CoreError::OpFailed(Op::Fillet)) if edges.is_empty() => Err(refused(Op::Fillet)),
            Err(CoreError::OpFailed(Op::Fillet)) => {
                let part = |b: &Bodies, _: f64| self.fillet_what_takes(b, FilletPart { body, src, radius, sharp: &sharp, numbers: &numbers, asked: &asked });
                self.run(Operation { kind: Op::Fillet, body, sizing: Sizing::Unsized, roles: given, work: part })
            }
            other => other,
        }
    }

    /// The edges that refuse told with data, and what does take built: the rest as a group, or one at a time on top of
    /// one another - the ids of untouched edges survive a fillet, and edge by edge on the running result often builds
    /// where the group fails at once.
    fn fillet_what_takes(&self, b: &Bodies, part: FilletPart<'_>) -> Result<crate::Shape, CoreError> {
        let FilletPart { body, src, radius, sharp, numbers, asked } = part;
        let s = b.get(src)?;
        let edges = &sharp.edges;
        let one = |base: &crate::Shape, e: u32, r: f64| base.fillet_edges_named(r, &[e], &numbers.surfaces_of(asked, &[e]), &numbers.corners, &numbers.all);
        let mut bad: Vec<FilletEdgeIssue> = Vec::new();
        for &e in edges {
            if one(s, e, radius).is_some() {
                continue;
            }
            // the largest radius it does accept, or none: data, not a phrase - the wording is the application's
            let takes_up_to = [0.5, 0.25, 0.1].into_iter().map(|f| radius * f).find(|r| one(s, e, *r).is_some());
            bad.push(FilletEdgeIssue { edge: e, takes_up_to });
        }
        if bad.is_empty() {
            // each takes alone and not together: neighbouring fillets intersect
            return Err(CoreError::FilletEdgesOneByOne { radius });
        }
        let good: Vec<u32> = edges.iter().copied().filter(|e| bad.iter().all(|x| x.edge != *e)).collect();
        let mut taken = good.len();
        let mut r = if good.is_empty() { None } else { s.fillet_edges_named(radius, &good, &numbers.surfaces_of(asked, &good), &numbers.corners, &numbers.all) };
        if r.is_none() && good.len() > 1 {
            let mut acc: Option<crate::Shape> = None;
            let mut done = 0usize;
            for &e in &good {
                // one that does not take on top of the rest - the rounds conflicting - is passed over, the most is built
                if let Some(ns) = one(acc.as_ref().unwrap_or(s), e, radius) {
                    acc = Some(ns);
                    done += 1;
                }
            }
            if done > 0 {
                r = acc;
                taken = done;
            }
        }
        let shape = r.ok_or(CoreError::FilletRadiusTooBig { radius, issues: bad.clone(), smooth_skipped: sharp.smooth })?;
        // what was left out goes on to the node in words
        self.dropped.borrow_mut().insert(body, sharp.left_out(taken));
        Ok(shape)
    }

    /// A FILLET WITH RADII AT THE VERTICES: an edge takes the radii of its ends, matched to 1e-6 - the point comes from a
    /// live rebuild of the same body, so "nearly the same vertex" would be the neighbouring one. Every radius steps back
    /// by one factor, the default and those at the vertices alike.
    pub(crate) fn fillet_at_vertices_op(&self, ask: BlendAsk<'_>, verts: &[qymcad_core::feature::RadiusAt]) -> Result<Built, CoreError> {
        let BlendAsk { body, src, size: radius, edges, roles } = ask;
        self.dropped.borrow_mut().remove(&body);
        let Prepared { sharp, numbers, given } = self.prepare(src, edges, roles)?;
        let asked: Vec<EdgeRole> = sharp.edges.iter().zip(numbers.surfaces.iter().chain(std::iter::repeat(&0))).map(|(edge, surface)| EdgeRole { edge: *edge, surface: *surface }).collect();
        let edges = &sharp.edges;
        let surfaces = numbers.surfaces_of(&asked, edges);
        let work = |b: &Bodies, f: f64| {
            let scaled: Vec<qymcad_core::feature::RadiusAt> = verts.iter().map(|v| qymcad_core::feature::RadiusAt { at: v.at, radius: v.radius * f }).collect();
            let names = crate::BlendNames { surfaces: &surfaces, corners: &numbers.corners, all: &numbers.all };
            b.get(src)?.fillet_at_vertices(radius * f, edges, &scaled, names).ok_or(CoreError::OpFailed(Op::FilletVar))
        };
        let largest = verts.iter().map(|v| v.radius).fold(radius, f64::max);
        let done = self.run(Operation { kind: Op::FilletVar, body, sizing: Sizing::Sized(StepBack { retreat: Retreat::Blend, size: largest }), roles: given, work });
        if done.is_ok() && sharp.unknown > 0 {
            self.dropped.borrow_mut().insert(body, sharp.left_out(sharp.edges.len()));
        }
        done.map_err(|e| if e == CoreError::OpFailed(Op::FilletVar) { refused(Op::FilletVar) } else { e })
    }

    /// A SYMMETRIC CHAMFER, stepped back at the limiting case; a leg the faces cannot hold is said as such.
    pub(crate) fn chamfer_op(&self, ask: BlendAsk<'_>) -> Result<Built, CoreError> {
        let BlendAsk { body, src, size: dist, edges, roles } = ask;
        let Prepared { sharp, numbers, given } = self.prepare(src, edges, roles)?;
        let asked: Vec<EdgeRole> = sharp.edges.iter().zip(numbers.surfaces.iter().chain(std::iter::repeat(&0))).map(|(edge, surface)| EdgeRole { edge: *edge, surface: *surface }).collect();
        let edges = &sharp.edges;
        let work = |b: &Bodies, f: f64| {
            let s = b.get(src)?;
            let r = if edges.is_empty() { s.chamfer_all(dist * f) } else { s.chamfer_edges(dist * f, edges, &numbers.surfaces_of(&asked, edges), &numbers.corners, &numbers.all) };
            r.ok_or(CoreError::ChamferTooBig { dist })
        };
        let done = self.run(Operation { kind: Op::Chamfer, body, sizing: Sizing::Sized(StepBack { retreat: Retreat::Blend, size: dist }), roles: given, work });
        if done.is_ok() && sharp.unknown > 0 {
            self.dropped.borrow_mut().insert(body, sharp.left_out(sharp.edges.len()));
        }
        done
    }

    /// AN ASYMMETRIC CHAMFER - two legs, or a leg and an angle - of edges picked: asymmetry is not defined for "every
    /// edge", which falls back to the symmetric form. Both legs step back; an angle is no length and stays as asked.
    pub(crate) fn chamfer_asym_op(&self, ask: BlendAsk<'_>, shape: ChamferShape) -> Result<Built, CoreError> {
        use qymcad_core::feature::ChamferMode;
        let ChamferShape { mode, d2, flip, ref_face } = shape;
        if ask.edges.is_empty() || mode == ChamferMode::Symmetric {
            return self.chamfer_op(ask);
        }
        let BlendAsk { body, src, size: d1, edges, roles } = ask;
        self.dropped.borrow_mut().remove(&body);
        let Prepared { sharp, numbers, given } = self.prepare(src, edges, roles)?;
        let asked: Vec<EdgeRole> = sharp.edges.iter().zip(numbers.surfaces.iter().chain(std::iter::repeat(&0))).map(|(edge, surface)| EdgeRole { edge: *edge, surface: *surface }).collect();
        let edges = &sharp.edges;
        let surfaces = numbers.surfaces_of(&asked, edges);
        let mode = if mode == ChamferMode::DistAngle { 2 } else { 1 };
        // an angle does not step back with the size; a second leg does
        let second = |f: f64| if mode == 2 { d2 } else { d2 * f };
        let work = |b: &Bodies, f: f64| {
            let names = crate::BlendNames { surfaces: &surfaces, corners: &numbers.corners, all: &numbers.all };
            b.get(src)?.chamfer_edges_asym(crate::AsymChamfer { a: d1 * f, b: second(f), mode, flip, ref_face }, edges, names).ok_or(CoreError::OpFailed(Op::ChamferAsym))
        };
        let largest = if mode == 2 { d1.max(d1 * d2.to_radians().tan().abs()) } else { d1.max(d2) };
        let done = self.run(Operation { kind: Op::ChamferAsym, body, sizing: Sizing::Sized(StepBack { retreat: Retreat::Blend, size: largest }), roles: given, work });
        if done.is_ok() && sharp.unknown > 0 {
            self.dropped.borrow_mut().insert(body, sharp.left_out(sharp.edges.len()));
        }
        done.map_err(|e| if e == CoreError::OpFailed(Op::ChamferAsym) { refused(Op::ChamferAsym) } else { e })
    }
}

/// What a blend has worked out before its work: the edges it takes, the numbers of its faces, and their roles.
struct Prepared {
    sharp: SharpEdges,
    numbers: BlendNumbers,
    given: Roles,
}

impl OcctKernel {
    /// The sharp edges of `src` among those asked, and the roles of the faces a blend of them makes.
    fn prepare(&self, src: Id, edges: &[u32], roles: BlendRoles) -> Result<Prepared, CoreError> {
        let shapes = self.shapes.borrow();
        let s = shapes.get(&src).ok_or(CoreError::SourceBodyNotBuilt)?;
        let sharp = sharp(s, edges)?;
        let mut given = Roles::default();
        let numbers = blend_roles(roles, &sharp.edges, &s.edges_with_ids().1, &mut given);
        Ok(Prepared { sharp, numbers, given })
    }
}

/// What the partial build of a fillet works with.
struct FilletPart<'a> {
    body: Id,
    src: Id,
    radius: f64,
    sharp: &'a SharpEdges,
    numbers: &'a BlendNumbers,
    asked: &'a [EdgeRole],
}

#[cfg(test)]
mod tests {
    use super::*;
    use qymcad_core::feature::Kernel;

    /// A kernel holding a block 10 x 10 x 10 as body 1.
    fn block() -> OcctKernel {
        let k = OcctKernel::default();
        k.extrude(1, &[0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0], 10.0, qymcad_core::feature::PLACE_IDENTITY).expect("the block");
        k
    }

    /// AN EDGE THE BODY DOES NOT HAVE IS NOT HANDED TO OCCT: asked alone it is refused in words, asked beside edges the
    /// body has the rest are blended and the one left out is said. The model used to filter such numbers itself, the
    /// kernel "crashing on an edge it does not have".
    #[test]
    fn an_edge_the_body_does_not_have_is_said_not_blended() {
        let k = block();
        let real = k.shapes.borrow().get(&1).map(|s| s.edges_with_ids().1).expect("the edges of the block")[0];
        let alone = k.fillet_op(BlendAsk { body: 2, src: 1, size: 1.0, edges: &[987_654], roles: BlendRoles::Unnamed });
        assert_eq!(alone.err(), Some(CoreError::EdgesNotFound { asked: 1 }), "an edge the body does not have, asked alone");
        let chamfer = k.chamfer_op(BlendAsk { body: 3, src: 1, size: 1.0, edges: &[987_654], roles: BlendRoles::Unnamed });
        assert_eq!(chamfer.err(), Some(CoreError::EdgesNotFound { asked: 1 }), "a chamfer of an edge the body does not have");
        k.fillet_op(BlendAsk { body: 4, src: 1, size: 1.0, edges: &[real, 987_654], roles: BlendRoles::Unnamed }).expect("the edge the body has is rounded");
        assert_eq!(k.dropped.borrow().get(&4).copied(), Some(DroppedEdges { asked: 2, dropped: 1 }), "the edge left out is not said");
    }
}
