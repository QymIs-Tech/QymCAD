//! SPECIMENS: bodies built for a probe to measure - a cylinder to tessellate into a mesh the recognition is asked to
//! find again, a block to write into a STEP file an import is asked to read. They are built by the operations of the
//! bridge directly, past the door and its names, which is right for a specimen and wrong for a document: the module
//! exists only in the probes of this crate and in the builds that ask for the `specimens` feature, which only the
//! dev-dependencies do. A build of the application has no such module, so a tool of the window cannot build here.

use crate::{HelicalCut, Shape};
use qymcad_core::feature::{LoftBody, LoftWalls};

/// A prism of the polygon `xy` (x, y pairs), `h` tall.
pub fn extrude(xy: &[f64], h: f64) -> Option<Shape> {
    Shape::extrude(xy, h)
}

/// A prism of an encoded profile, `h` tall.
pub fn extrude_profile(data: &[f64], h: f64) -> Option<Shape> {
    Shape::extrude_profile(data, h)
}

/// The polygon `xy` turned by `angle_deg` about world axis `axis`.
pub fn revolve(xy: &[f64], axis: u8, angle_deg: f64) -> Option<Shape> {
    Shape::revolve(xy, axis, angle_deg)
}

/// An encoded profile turned by `angle_deg` about world axis `axis`.
pub fn revolve_profile(data: &[f64], axis: u8, angle_deg: f64) -> Option<Shape> {
    Shape::revolve_profile(data, axis, angle_deg)
}

/// A loft through the encoded sections (see `Shape::loft_sections`).
pub fn loft(data: &[f64], offsets: &[usize], places: &[f64], walls: LoftWalls, kind: LoftBody) -> Option<Shape> {
    Shape::loft_sections(data, offsets, places, walls, kind)
}

pub fn cylinder(r: f64, h: f64) -> Option<Shape> {
    Shape::cylinder(r, h)
}

pub fn sphere(r: f64) -> Option<Shape> {
    Shape::sphere(r)
}

pub fn cone(r1: f64, r2: f64, h: f64) -> Option<Shape> {
    Shape::cone(r1, r2, h)
}

pub fn torus(major: f64, minor: f64) -> Option<Shape> {
    Shape::torus(major, minor)
}

/// One union of every shape.
pub fn fuse(parts: &[&Shape]) -> Option<Shape> {
    Shape::fuse_many(parts)
}

/// A body sewn of the flat faces of a mesh.
pub fn from_mesh(mesh: &qymcad_core::geom::Mesh) -> Option<Shape> {
    Shape::from_mesh(mesh)
}

/// WHAT A SPECIMEN IS WORKED ON WITH, under names of their own: the operations of the bridge stay closed, and a method
/// of the same name would be shadowed by them.
pub trait Specimen {
    /// Every edge rounded by `r`.
    fn rounded(&self, r: f64) -> Option<Shape>;
    /// With `other`: 0 subtracts, 1 unites, 2 intersects.
    fn combined(&self, other: &Shape, op: u8) -> Option<Shape>;
    /// The edges `edges` chamfered by `d`, the faces it makes left without names.
    fn chamfered(&self, d: f64, edges: &[u32]) -> Option<Shape>;
    /// A helical groove or rib.
    fn grooved(&self, cut: HelicalCut) -> Option<Shape>;
}

impl Specimen for Shape {
    fn rounded(&self, r: f64) -> Option<Shape> {
        self.fillet_all(r)
    }
    fn combined(&self, other: &Shape, op: u8) -> Option<Shape> {
        self.boolean(other, op)
    }
    fn chamfered(&self, d: f64, edges: &[u32]) -> Option<Shape> {
        self.chamfer_edges(d, edges, &[], &[], &[])
    }
    fn grooved(&self, cut: HelicalCut) -> Option<Shape> {
        self.helical_profile(cut)
    }
}

/// WHAT A PROBE ASKS OF A BODY that the program never asks outside the bridge: its triangles at a deflection, at a
/// deflection fitted to its size, the body placed elsewhere, read back from saved bytes, the places of its faces.
pub fn mesh(s: &Shape, deflection: f64) -> Vec<qymcad_core::geom::Built> {
    s.tessellate(deflection)
}

/// The triangles at a deflection fitted to the size of the body.
pub fn mesh_auto(s: &Shape, k: f64) -> Vec<qymcad_core::geom::Built> {
    s.tessellate_auto(k)
}

/// The triangles of the body as one mesh.
pub fn mesh_merged(s: &Shape, deflection: f64) -> Option<qymcad_core::geom::Built> {
    s.tessellate_merged(deflection)
}

/// The body moved by the 3x4 matrix `m`, its names kept.
pub fn placed(s: &Shape, m: &[f64; 12]) -> Option<Shape> {
    s.transformed(m)
}

/// A body read back from the bytes it was saved as.
pub fn read(bytes: &[u8]) -> Option<Shape> {
    Shape::from_brep_bytes(bytes)
}

/// The places of the faces of the body, by their names.
pub fn face_places(s: &Shape) -> Vec<qymcad_core::feature::FacePlace> {
    s.face_places()
}

/// The common volume of two bodies; `None` when it cannot be measured.
pub fn common_volume(a: &Shape, b: &Shape) -> Option<f64> {
    a.interference_volume(b)
}
