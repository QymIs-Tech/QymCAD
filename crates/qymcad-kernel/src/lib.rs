//! The B-rep kernel, through an FFI to the system OpenCASCADE.
//!
//! A STEP import gives a set of bodies, one per solid, and each body is a [`Mesh`] plus faces taken from the
//! real B-rep topology ([`MeshFace`]). From there on the B-rep is the source of truth for extrusions and
//! booleans.


/// The name of the C++ runtime to link, chosen by target - shared with `build.rs`.
pub mod cxx_runtime;
pub mod kernel;
pub use kernel::{kernel_gate, OcctKernel};

use std::ffi::CString;
use std::os::raw::{c_char, c_double};

use qymcad_core::geom::{Mesh, MeshFace, Point3};

#[repr(C)]
struct QymDoc {
    _private: [u8; 0],
}

extern "C" {
    fn qym_occt_step_read(path: *const c_char, defl: c_double) -> *mut QymDoc;
    fn qym_occt_box_doc(dx: c_double, dy: c_double, dz: c_double, defl: c_double) -> *mut QymDoc;
    fn qym_occt_extrude(xy: *const c_double, n: usize, height: c_double, defl: c_double) -> *mut QymDoc;
    fn qym_occt_revolve(xy: *const c_double, n: usize, axis: i32, angle_deg: c_double, defl: c_double) -> *mut QymDoc;
    fn qym_occt_extrude_bool(base_xy: *const c_double, nb: usize, base_h: c_double, tool_xy: *const c_double, nt: usize, tool_h: c_double, op: i32, defl: c_double) -> *mut QymDoc;
    fn qym_doc_body_count(d: *const QymDoc) -> usize;
    fn qym_body_vert_count(d: *const QymDoc, i: usize) -> usize;
    fn qym_body_tri_count(d: *const QymDoc, i: usize) -> usize;
    fn qym_body_face_count(d: *const QymDoc, i: usize) -> usize;
    fn qym_body_copy_verts(d: *const QymDoc, i: usize, out: *mut f32);
    fn qym_body_copy_tris(d: *const QymDoc, i: usize, out: *mut u32);
    fn qym_body_copy_face_starts(d: *const QymDoc, i: usize, out: *mut u32);
    fn qym_body_copy_face_counts(d: *const QymDoc, i: usize, out: *mut u32);
    fn qym_body_copy_face_ids(d: *const QymDoc, i: usize, out: *mut u32);
    fn qym_body_copy_face_anchors(d: *const QymDoc, i: usize, out: *mut f64);
    fn qym_doc_free(d: *mut QymDoc);
}

/// One body: a mesh plus the faces of the B-rep topology.
pub type Body = (Mesh, Vec<MeshFace>);

#[repr(C)]
struct QymShape {
    _private: [u8; 0],
}
#[repr(C)]
struct QymShapeList {
    _private: [u8; 0],
}
#[repr(C)]
struct QymEdges {
    _private: [u8; 0],
}

extern "C" {
    fn qym_shape_extrude(xy: *const c_double, n: usize, h: c_double) -> *mut QymShape;
    fn qym_shape_bezier_shell(pts: *const c_double, patches: usize, tol: c_double, make_solid: i32, free_edges: *mut u32) -> *mut QymShape;
    fn qym_shape_revolve(xy: *const c_double, n: usize, axis: i32, angle_deg: c_double) -> *mut QymShape;
    fn qym_shape_extrude_profile(data: *const c_double, n: usize, h: c_double, cap0: u32, cap1: u32) -> *mut QymShape;
    fn qym_shape_revolve_profile(data: *const c_double, n: usize, axis: i32, angle_deg: c_double) -> *mut QymShape;
    fn qym_shape_revolve_profile_axis(data: *const c_double, n: usize, origin: *const f64, dir: *const f64, angle_deg: c_double, cap0: u32, cap1: u32) -> *mut QymShape;
    fn qym_shape_sweep(prof: *const c_double, np: usize, prof_tf: *const c_double, path: *const c_double, npath: usize, path_tf: *const c_double, cap0: u32, cap1: u32) -> *mut QymShape;
    fn qym_shape_loft(data: *const c_double, ndata: usize, offsets: *const usize, nsec: usize, places: *const c_double, ruled: i32, solid: i32, cap0: u32, cap1: u32) -> *mut QymShape;
    fn qym_shape_extrude_profiles_fused(data: *const c_double, offsets: *const usize, nprof: usize, h: c_double, caps: *const u32, ncaps: usize) -> *mut QymShape;
    fn qym_shape_cylinder(r: c_double, h: c_double, cap0: u32, cap1: u32, side: u32) -> *mut QymShape;
    fn qym_shape_sphere(r: c_double, cap0: u32, cap1: u32, side: u32) -> *mut QymShape;
    fn qym_shape_cone(r1: c_double, r2: c_double, h: c_double, cap0: u32, cap1: u32, side: u32) -> *mut QymShape;
    fn qym_shape_torus(major: c_double, minor: c_double, cap0: u32, cap1: u32, side: u32) -> *mut QymShape;
    fn qym_shape_boolean(a: *const QymShape, b: *const QymShape, op: i32) -> *mut QymShape;
    fn qym_shape_fuse_many(parts: *const *const QymShape, n: i32) -> *mut QymShape;
    fn qym_shape_transform(s: *const QymShape, m: *const c_double) -> *mut QymShape;
    fn qym_shape_fillet_all(s: *const QymShape, r: c_double) -> *mut QymShape;
    fn qym_shape_chamfer_all(s: *const QymShape, d: c_double) -> *mut QymShape;
    fn qym_shape_edges(s: *const QymShape) -> *mut QymEdges;
    fn qym_edge_smooth(e: *const QymEdges, i: usize) -> i32;
    fn qym_edges_count(e: *const QymEdges) -> usize;
    fn qym_edge_id(e: *const QymEdges, i: usize) -> u32;
    fn qym_edge_point_count(e: *const QymEdges, i: usize) -> usize;
    fn qym_edge_copy_points(e: *const QymEdges, i: usize, out: *mut f32);
    fn qym_edge_circle(e: *const QymEdges, i: usize, out: *mut f64) -> i32;
    fn qym_edge_ref_dir(e: *const QymEdges, i: usize, out: *mut f64) -> i32;
    fn qym_edges_free(e: *mut QymEdges);
    fn qym_shape_fillet_edges(s: *const QymShape, r: c_double, idx: *const u32, n: usize, names: *const u32, corners: *const u32, all_names: *const u32, n_all: usize) -> *mut QymShape;
    fn qym_shape_fillet_var(s: *const QymShape, r1: c_double, r2: c_double, idx: *const u32, n: usize) -> *mut QymShape;
    fn qym_shape_fillet_at_vertices(s: *const QymShape, r_default: c_double, idx: *const u32, n: usize, vpts: *const f64, vrads: *const f64, m: usize, tol: c_double) -> *mut QymShape;
    fn qym_shape_chamfer_edges(s: *const QymShape, d: c_double, idx: *const u32, n: usize, names: *const u32, corners: *const u32, all_names: *const u32, n_all: usize) -> *mut QymShape;
    fn qym_shape_chamfer_edges_asym(s: *const QymShape, a: c_double, b: c_double, mode: i32, flip: i32, ref_face: u32, idx: *const u32, n: usize) -> *mut QymShape;
    fn qym_shape_shell(s: *const QymShape, t: c_double, idx: *const u32, n: usize, gfrom: *const u32, gto: *const u32, gn: usize) -> *mut QymShape;
    fn qym_shape_remove_faces(s: *const QymShape, ids: *const u32, n: usize, out_reason: *mut i32) -> *mut QymShape;
    fn qym_shape_push_face(s: *const QymShape, fid: u32, dist: c_double) -> *mut QymShape;
    fn qym_shape_thicken_face(s: *const QymShape, fid: u32, thickness: c_double, fmap: *const u32, nf: usize, emap: *const u32, ne: usize) -> *mut QymShape;
    fn qym_shape_thicken_face_join(s: *const QymShape, fid: u32, thickness: c_double, fmap: *const u32, nf: usize, emap: *const u32, ne: usize) -> *mut QymShape;
    fn qym_shape_face_cylinder(s: *const QymShape, fid: u32, origin: *mut c_double, dir: *mut c_double, radius: *mut c_double) -> i32;
    fn qym_shape_split_faces(s: *const QymShape, origin: *const c_double, normal: *const c_double) -> *mut QymShape;
    fn qym_shape_split_by_plane(s: *const QymShape, origin: *const c_double, normal: *const c_double, section: u32) -> *mut QymShapeList;
    fn qym_shape_draft(s: *const QymShape, idx: *const u32, n: usize, angle_deg: c_double, pull: *const f64, np_origin: *const f64, np_normal: *const f64, sides: *const u32, nsides: usize) -> *mut QymShape;
    fn qym_shape_face_splits(s: *const QymShape, out: *mut u32, cap: usize) -> usize;
    fn qym_shape_clear_face_splits(s: *mut QymShape);
    fn qym_shape_absorbed(s: *const QymShape, out: *mut u32, max_pairs: usize) -> usize;
    fn qym_shape_rename_faces(s: *mut QymShape, from: *const u32, to: *const u32, n: usize);
    fn qym_shape_edge_face_pairs(s: *const QymShape, out: *mut u32, cap: usize) -> usize;
    fn qym_shape_edge_end_faces(s: *const QymShape, out: *mut u32, cap: usize) -> usize;
    fn qym_shape_rename_edges(s: *mut QymShape, from: *const u32, to: *const u32, n: usize);
    fn qym_shape_hole_stepped(s: *const QymShape, kind: i32, pl: *const f64, dia: c_double, depth: c_double, dia2: c_double, depth2: c_double, bore: u32, extra: *const u32, n_extra: usize) -> *mut QymShape;
    fn qym_shape_holes_stepped(s: *const QymShape, kind: i32, pls: *const f64, n_holes: usize, dia: c_double, depth: c_double, dia2: c_double, depth2: c_double, bores: *const u32, extra: *const u32, n_extra: usize) -> *mut QymShape;
    fn qym_shape_shell_center(s: *const QymShape, t: c_double, idx: *const u32, n: usize) -> *mut QymShape;
    fn qym_shape_face_axis(s: *const QymShape, face_id: u32, origin: *mut f64, dir: *mut f64) -> i32;
    fn qym_shape_face_edge_ids(s: *const QymShape, face_id: u32, out: *mut u32, cap: usize) -> usize;
    fn qym_shape_mirror(s: *const QymShape, plane: i32) -> *mut QymShape;
    fn qym_shape_mirror_plane(s: *const QymShape, origin: *const f64, normal: *const f64) -> *mut QymShape;
    fn qym_shape_interference_volume(a: *const QymShape, b: *const QymShape, out: *mut c_double) -> i32;
    fn qym_why() -> *const std::os::raw::c_char;
    fn qym_why_clear();
    fn qym_why_set(text: *const std::os::raw::c_char);
    fn qym_shape_helical_profile(base: *const QymShape, origin: *const f64, dir: *const f64, radius: c_double, prof: *const f64, nprof: usize, length: c_double, lead: c_double, starts: i32, left: i32, mode: i32, lead_in: c_double, lead_out: c_double, gnames: *const u32, gn: usize, rnames: *const u32, rn: usize, crest_relief: c_double) -> *mut QymShape;
    fn qym_shape_thread(base: *const QymShape, origin: *const f64, dir: *const f64, radius: c_double, length: c_double, pitch: c_double, angle_deg: c_double, depth: c_double, starts: i32, left: i32, internal: i32, form: i32, clearance_crest: c_double, clearance_root: c_double, lead_in: c_double, lead_out: c_double) -> *mut QymShape;
    fn qym_shape_volume(s: *const QymShape) -> c_double;
    fn qym_shape_bbox(s: *const QymShape, out: *mut f64) -> i32;
    fn qym_shape_is_valid(s: *const QymShape) -> i32;
    fn qym_shape_min_round_radius(s: *const QymShape) -> f64;
    fn qym_shape_solid_count(s: *const QymShape) -> i32;
    fn qym_shape_shell_count(s: *const QymShape) -> i32;
    fn qym_shape_heal(s: *const QymShape) -> *mut QymShape;
    fn qym_shape_kind(s: *const QymShape) -> i32;
    fn qym_shape_copy_faces(s: *const QymShape, idx: *const u32, names: *const u32, n: usize) -> *mut QymShape;
    fn qym_shape_replace_faces(base: *const QymShape, idx: *const u32, n: usize, surf: *const QymShape, tol: c_double, out_free: *mut u32) -> *mut QymShape;
    fn qym_shape_stitch(parts: *const *const QymShape, n: usize, tol: c_double, out_free: *mut u32, out_joined: *mut u32) -> *mut QymShape;
    fn qym_shape_trim(s: *const QymShape, tool: *const QymShape, keep: *const c_double) -> *mut QymShape;
    fn qym_shape_patch(s: *const QymShape, idx: *const u32, n: usize, tangent: i32, name: u32) -> *mut QymShape;
    fn qym_shape_tessellate(s: *const QymShape, defl: c_double) -> *mut QymDoc;
    fn qym_shape_free(s: *mut QymShape);
    fn qym_shape_to_brep(s: *const QymShape, out_len: *mut usize) -> *mut u8;
    fn qym_shape_from_brep(data: *const u8, len: usize) -> *mut QymShape;
    fn qym_bytes_free(p: *mut u8);
    fn qym_shape_heal_pinched_faces(s: *mut QymShape) -> i32;
    fn qym_step_solids(path: *const c_char) -> *mut QymShapeList;
    fn qym_shapelist_count(l: *const QymShapeList) -> usize;
    fn qym_shapelist_get(l: *const QymShapeList, i: usize) -> *mut QymShape;
    fn qym_shapelist_free(l: *mut QymShapeList);
    fn qym_step_write(shapes: *const *const QymShape, mats: *const f64, n: usize, path: *const c_char) -> i32;
}

/// One edge of a body as the kernel gives it: a polyline, a persistent id and everything else known about it.
#[derive(Clone, Debug, Default)]
pub struct EdgeInfo {
    /// The polyline of the edge in the local frame of the body.
    pub poly: Vec<[f32; 3]>,
    /// The persistent id; zero means an unnamed edge.
    pub id: u32,
    /// A circle or arc, as centre, axis and radius. `None` is a straight edge or a spline.
    pub circle: Option<([f64; 3], [f64; 3], f64)>,
    /// A smooth, tangent junction of faces: no use for a fillet or a chamfer.
    pub smooth: bool,
    /// The normal of the adjacent face at the midpoint of the edge, which is the secondary axis of a
    /// connector. `None` means there is no adjacent face or its normal is undefined, leaving the roll about
    /// the edge unset.
    pub ref_dir: Option<[f64; 3]>,
}

/// A live B-rep shape. It owns the handle and frees it on drop, and it allows a general boolean over any
/// bodies, whatever built them — an extrusion, a revolve or a STEP import.
pub struct Shape {
    ptr: *mut QymShape,
}

impl Shape {
    /// A live body into bytes, together with the names of its faces and edges; see `qym_shape_to_brep`.
    ///
    /// This exists so that opening a file need not build everything again: a bundle holds meshes and faces
    /// but no live B-rep, and the first operation then pays for a full rebuild of the timeline.
    pub fn to_brep_bytes(&self) -> Option<Vec<u8>> {
        let mut len: usize = 0;
        let p = unsafe { qym_shape_to_brep(self.ptr, &mut len) };
        if p.is_null() || len == 0 {
            return None;
        }
        let v = unsafe { std::slice::from_raw_parts(p, len) }.to_vec();
        unsafe { qym_bytes_free(p) };
        Some(v)
    }

    /// The read back. `None` means the blob is foreign or damaged; there is nothing to guess at silently.
    pub fn from_brep_bytes(data: &[u8]) -> Option<Shape> {
        let p = unsafe { qym_shape_from_brep(data.as_ptr(), data.len()) };
        (!p.is_null()).then(|| Shape { ptr: p })
    }
}

impl Drop for Shape {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe { qym_shape_free(self.ptr) };
        }
    }
}

/// WHY THE KERNEL REFUSED the last operation on this thread, in the kernel's own words.
///
/// An operation that fails returns `None` and nothing more, and there are 250 places inside the kernel that
/// can produce that `None`. Finding which one meant bisecting five thousand lines of C++ - hours, on every
/// geometric defect. This says the place and, where the kernel spoke, what it said: `"boolean: BRepAlgoAPI:
/// the operands are self-intersecting"`.
///
/// FOR WHOEVER FIXES THE PROGRAM, not for whoever draws a part. The text names internals and arrives
/// untranslated from the kernel, so it belongs in a log, a test or a report - never in a window. What the
/// user is shown stays a `CoreError` worded from the catalogue.
///
/// The buffer belongs to the calling thread, and it holds until the next refusal on that thread, so read it
/// straight after the operation that returned `None`.
/// Refuse a request that never reaches OCCT, in the words of the kernel's own channel.
///
/// A guard on this side is a refusal like any other to whoever is looking for the cause, and a second channel
/// for it would be one nobody thinks to read. Always returns `None`, so a guard reads as one line.
fn refuse(where_: &str, what: &str) -> Option<Shape> {
    let text = std::ffi::CString::new(format!("{where_}: {what}")).unwrap_or_default();
    unsafe { qym_why_set(text.as_ptr()) }
    None
}

pub fn last_kernel_refusal() -> Option<String> {
    let p = unsafe { qym_why() };
    if p.is_null() {
        return None;
    }
    unsafe { std::ffi::CStr::from_ptr(p) }.to_str().ok().map(|s| s.to_owned())
}

/// Forget the last refusal on this thread, so that an old one is not read as the reason for a new failure.
pub fn clear_kernel_refusal() {
    unsafe { qym_why_clear() }
}

/// THE LAST REFUSAL, KEPT FOR A PROBLEM REPORT — one per process, across threads.
///
/// [`last_kernel_refusal`] reads a buffer belonging to the CALLING thread and is cleared as soon as the
/// failure is turned into a `CoreError`. The work runs on a background thread, the window reads on its
/// own, and the person decides to report the trouble minutes later — by then the words are gone three
/// times over. This copy survives all of that.
static KEPT: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// Called from the one place that turns a refusal into an error.
pub(crate) fn keep_for_report(op: qymcad_core::errors::Op, why: &str) {
    if let Ok(mut k) = KEPT.lock() {
        *k = Some(format!("{op:?}: {why}"));
    }
}

/// What the kernel last refused, for a report. `None` means it has refused nothing this run.
pub fn refusal_for_report() -> Option<String> {
    KEPT.lock().ok().and_then(|k| k.clone())
}

// A shape owns its kernel handle as a sole raw pointer. Moving that ownership to another thread is safe: a
// STEP import or export runs on a worker thread so the interface can show progress, and the object is never
// shared between threads at once, only moved. There is no concurrency over one shape, so `Send` is enough and
// `Sync` is not claimed.
unsafe impl Send for Shape {}

/// WHICH WAY A HELIX WINDS: the ordinary right hand, or the left one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hand {
    /// the usual thread, tightening clockwise
    Right,
    /// a left-hand thread
    Left,
}

/// WHERE A THREAD IS CUT: on the outside of a shaft, or inside a bore.
///
/// It decides which way the profile is taken from the surface, so a bore threaded as a shaft comes out with
/// the groove on the wrong side of the wall - a defect the caller cannot see in a bare `true`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Site {
    /// on the outer surface
    Shaft,
    /// inside a hole
    Bore,
}

/// WHAT A HELICAL PROFILE DOES TO THE BODY: takes material away or adds it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Helix {
    /// subtract, giving a thread
    Groove,
    /// unite, giving the flight of an auger
    Rib,
}

impl Shape {
    fn wrap(p: *mut QymShape) -> Option<Shape> {
        if p.is_null() {
            None
        } else {
            Some(Shape { ptr: p })
        }
    }
    /// A wrapper that checks validity, for operations the kernel does build — returning a non-null result —
    /// while the result may be geometrically broken: a fillet of a radius larger than the wall thickness, or a
    /// shell on unsuitable geometry. A broken result becomes `None`, so the feature falls into a red node and
    /// the part stays as it was, instead of showing half-transparent walls.
    fn wrap_valid(p: *mut QymShape) -> Option<Shape> {
        let s = Self::wrap(p)?;
        if unsafe { qym_shape_is_valid(s.ptr) } != 0 {
            return Some(s);
        }
        // An unsound shape is repaired rather than discarded. An operation that built a body with a minor
        // flaw used to return nothing from here, so "the operation failed" was reported instead of a part
        // that a standard repair puts right. If the repair fails, then nothing is returned.
        s.healed()
    }
    /// Extrude a profile into a shape.
    /// A shell of 4×4 Bezier patches: how the kernel receives a subdivision cage.
    ///
    /// It returns the shape and the number of unstitched edges. Zero means the shell closed on itself without
    /// a single hole, and that number is the whole point of the exercise. A non-zero one is not hidden: a cage
    /// with extraordinary points has holes by construction, and knowing how many matters more than seeing a
    /// picture.
    pub fn from_bezier_patches(patches: &[[[[f64; 3]; 4]; 4]], tol: f64, make_solid: bool) -> Option<(Shape, u32)> {
        if patches.is_empty() {
            refuse("bezier/asked", "there is not one patch to build a surface from")?;
        }
        let mut flat: Vec<f64> = Vec::with_capacity(patches.len() * 48);
        for p in patches {
            for row in p {
                for pt in row {
                    flat.extend_from_slice(pt);
                }
            }
        }
        let mut free = 0u32;
        let s = unsafe { Self::wrap(qym_shape_bezier_shell(flat.as_ptr(), patches.len(), tol, make_solid as i32, &mut free)) }?;
        Some((s, free))
    }

    pub fn extrude(xy: &[f64], h: f64) -> Option<Shape> {
        if xy.len() / 2 < 3 {
            return refuse("extrude/asked", "the contour has fewer than three points, so it encloses nothing");
        }
        unsafe { Self::wrap(qym_shape_extrude(xy.as_ptr(), xy.len() / 2, h)) }
    }
    /// Revolve a profile into a shape.
    pub fn revolve(xy: &[f64], axis: u8, angle_deg: f64) -> Option<Shape> {
        if xy.len() / 2 < 3 {
            return refuse("revolve/asked", "the contour has fewer than three points, so it encloses nothing");
        }
        let ang = if angle_deg.abs() < 1e-6 { 360.0 } else { angle_deg };
        unsafe { Self::wrap(qym_shape_revolve(xy.as_ptr(), xy.len() / 2, axis as i32, ang)) }
    }
    /// Extrude an exact profile — encoded by `Profile::encode`, with contours of real edges: lines, arcs and
    /// circles — to a height of `h`, giving a body with exact faces, so that a cylinder is three faces rather
    /// than a faceted prism. Without names, since the profile does not come from a sketch: format tests and
    /// auxiliary bodies.
    pub fn extrude_profile(data: &[f64], h: f64) -> Option<Shape> {
        Self::extrude_profile_named(data, h, [0, 0])
    }

    pub fn extrude_profiles_fused(profiles: &[Vec<f64>], h: f64) -> Option<Shape> {
        Self::extrude_profiles_fused_named(profiles, h, &[])
    }

    /// `caps` holds the name descriptors of the caps, at the start and the end; zero means a positional name
    /// from the kernel.
    pub fn extrude_profile_named(data: &[f64], h: f64, caps: [u32; 2]) -> Option<Shape> {
        if data.is_empty() {
            return refuse("extrude/asked", "there is no profile to extrude");
        }
        unsafe { Self::wrap(qym_shape_extrude_profile(data.as_ptr(), data.len(), h, caps[0], caps[1])) }
    }
    /// Extrude N exact profiles as a single 2D-fused face, so that touching ones give no seam edges. `caps`
    /// holds triples of region key, bottom and top, the key being the smallest wall name of the region.
    pub fn extrude_profiles_fused_named(profiles: &[Vec<f64>], h: f64, caps: &[u32]) -> Option<Shape> {
        if profiles.is_empty() || profiles.iter().any(|p| p.is_empty()) {
            return refuse("extrude/asked", "one of the profiles to extrude is empty");
        }
        let mut data: Vec<f64> = Vec::new();
        let mut offsets: Vec<usize> = vec![0];
        for pr in profiles {
            data.extend_from_slice(pr);
            offsets.push(data.len());
        }
        unsafe { Self::wrap(qym_shape_extrude_profiles_fused(data.as_ptr(), offsets.as_ptr(), profiles.len(), h, caps.as_ptr(), caps.len())) }
    }

    /// Revolve an exact profile about an axis — 0 for X, 1 for Y — through an angle, giving a body.
    pub fn revolve_profile(data: &[f64], axis: u8, angle_deg: f64) -> Option<Shape> {
        if data.is_empty() {
            return refuse("revolve/asked", "there is no profile to revolve");
        }
        unsafe { Self::wrap(qym_shape_revolve_profile(data.as_ptr(), data.len(), axis as i32, angle_deg)) }
    }
    /// Revolve an exact profile about an arbitrary axis, given as an origin and a direction in the local
    /// frame of the sketch, through an angle.
    pub fn revolve_profile_axis(data: &[f64], origin: [f64; 3], dir: [f64; 3], angle_deg: f64) -> Option<Shape> {
        Self::revolve_profile_axis_named(data, origin, dir, angle_deg, [0, 0])
    }

    /// `caps` holds the names of the end faces, present on a partial revolution; zero means positional names
    /// from the kernel.
    pub fn revolve_profile_axis_named(data: &[f64], origin: [f64; 3], dir: [f64; 3], angle_deg: f64, caps: [u32; 2]) -> Option<Shape> {
        if data.is_empty() {
            return refuse("revolve/asked", "there is no profile to revolve");
        }
        unsafe { Self::wrap(qym_shape_revolve_profile_axis(data.as_ptr(), data.len(), origin.as_ptr(), dir.as_ptr(), angle_deg, caps[0], caps[1])) }
    }
    /// A sweep: the exact profile `prof` along the path `path`, which is a single contour in the format
    /// `[1.0, loop_block]`. `prof_tf` and `path_tf` are the 3×4 placements of the two sketches into world
    /// coordinates. A profile face becomes a solid body; empty or insufficient data gives `None`.
    pub fn sweep_profile(prof: &[f64], prof_tf: &[f64; 12], path: &[f64], path_tf: &[f64; 12]) -> Option<Shape> {
        Self::sweep_profile_named(prof, prof_tf, path, path_tf, [0, 0])
    }

    /// `caps` holds the names of the end faces of the sweep, at the start and the end of the path.
    pub fn sweep_profile_named(prof: &[f64], prof_tf: &[f64; 12], path: &[f64], path_tf: &[f64; 12], caps: [u32; 2]) -> Option<Shape> {
        if prof.is_empty() {
            return refuse("sweep/asked", "there is no profile to sweep");
        }
        if path.is_empty() {
            return refuse("sweep/asked", "there is no path to sweep the profile along");
        }
        unsafe { Self::wrap(qym_shape_sweep(prof.as_ptr(), prof.len(), prof_tf.as_ptr(), path.as_ptr(), path.len(), path_tf.as_ptr(), caps[0], caps[1])) }
    }

    /// A loft through sections. `data` is the concatenation of the `loop_block`s of the sections, `offsets`,
    /// of length nsec+1, gives where each section starts in `data`, and `places`, of nsec×12, gives the 3×4
    /// placements of the section planes. `ruled` asks for ruled faces rather than a smooth surface, and
    /// `solid` closes the result into a body. The lengths are validated; otherwise `None`.
    pub fn loft_sections(data: &[f64], offsets: &[usize], places: &[f64], walls: qymcad_core::feature::LoftWalls, kind: qymcad_core::feature::LoftBody) -> Option<Shape> {
        Self::loft_sections_named(data, offsets, places, walls, kind, [0, 0])
    }

    /// `caps` holds the names of the end faces of the loft, at the first and last sections.
    pub fn loft_sections_named(data: &[f64], offsets: &[usize], places: &[f64], walls: qymcad_core::feature::LoftWalls, kind: qymcad_core::feature::LoftBody, caps: [u32; 2]) -> Option<Shape> {
        let (ruled, solid) = (walls == qymcad_core::feature::LoftWalls::Ruled, kind == qymcad_core::feature::LoftBody::Solid);
        let nsec = offsets.len().checked_sub(1).unwrap_or(0);
        if nsec < 2 {
            return refuse("loft/asked", "a loft needs two sections or more, and it was given fewer");
        }
        if data.is_empty() {
            return refuse("loft/asked", "the sections carry no points");
        }
        if places.len() != nsec * 12 {
            return refuse("loft/asked", "there is not one placement per section");
        }
        unsafe { Self::wrap(qym_shape_loft(data.as_ptr(), data.len(), offsets.as_ptr(), nsec, places.as_ptr(), ruled as i32, solid as i32, caps[0], caps[1])) }
    }
    /// An exact cylinder about Z with its base at z = 0: three faces, not a faceted one.
    pub fn cylinder(r: f64, h: f64) -> Option<Shape> {
        Self::cylinder_named(r, h, [0, 0, 0])
    }
    pub fn sphere(r: f64) -> Option<Shape> {
        Self::sphere_named(r, [0, 0, 0])
    }
    pub fn cone(r1: f64, r2: f64, h: f64) -> Option<Shape> {
        Self::cone_named(r1, r2, h, [0, 0, 0])
    }
    pub fn torus(major: f64, minor: f64) -> Option<Shape> {
        Self::torus_named(major, minor, [0, 0, 0])
    }

    /// `names` holds the face at the lower z, the face at the upper z, and the lateral surface.
    pub fn cylinder_named(r: f64, h: f64, names: [u32; 3]) -> Option<Shape> {
        unsafe { Self::wrap(qym_shape_cylinder(r, h, names[0], names[1], names[2])) }
    }
    /// An exact sphere centred at the origin: one face.
    pub fn sphere_named(r: f64, names: [u32; 3]) -> Option<Shape> {
        unsafe { Self::wrap(qym_shape_sphere(r, names[0], names[1], names[2])) }
    }
    /// An exact cone about Z, of radius `r1` at z = 0 and `r2` at z = h.
    pub fn cone_named(r1: f64, r2: f64, h: f64, names: [u32; 3]) -> Option<Shape> {
        unsafe { Self::wrap(qym_shape_cone(r1, r2, h, names[0], names[1], names[2])) }
    }
    /// An exact torus in the XY plane about Z.
    pub fn torus_named(major: f64, minor: f64, names: [u32; 3]) -> Option<Shape> {
        unsafe { Self::wrap(qym_shape_torus(major, minor, names[0], names[1], names[2])) }
    }
    /// A boolean against another shape: 0 subtracts, 1 unites, 2 intersects.
    pub fn boolean(&self, other: &Shape, op: u8) -> Option<Shape> {
        unsafe { Self::wrap(qym_shape_boolean(self.ptr, other.ptr, op as i32)) }
    }
    /// One union of every shape at once rather than a chain of pairwise ones: all the arguments are
    /// intersected a single time, sharing vertices and edges. The first shape carries the counter of
    /// positional numbers.
    pub fn fuse_many(parts: &[&Shape]) -> Option<Shape> {
        if parts.is_empty() {
            return refuse("fuse/asked", "there is not one body to fuse");
        }
        let ptrs: Vec<*const QymShape> = parts.iter().map(|s| s.ptr as *const QymShape).collect();
        unsafe { Self::wrap(qym_shape_fuse_many(ptrs.as_ptr(), ptrs.len() as i32)) }
    }
    /// A real thread on this body along the axis given by `origin` and `dir`, on a surface of radius
    /// `radius` taken from the geometry itself. A helical groove — of pitch `pitch`, profile angle
    /// `angle_deg`, depth `depth` and `starts` starts, `left` for a left hand and `internal` for one inside a
    /// hole — is cut out of the body. Genuine B-rep, by a helix, a pipe shell and a boolean cut, not
    /// cosmetics.
    #[allow(clippy::too_many_arguments)]
    /// A helical rib or groove from an exact profile (`geom::encode_profile`) computed by the model kernel
    /// from the thread standard. `mode` 0 subtracts, giving a thread, and 1 unites, giving the flight of an
    /// auger. The profile lies in the axial plane: x along the axis, y radially out from the surface of
    /// radius `radius`.
    #[allow(clippy::too_many_arguments)]
    pub fn helical_profile(&self, origin: [f64; 3], dir: [f64; 3], radius: f64, profile: &[f64], length: f64, lead: f64, starts: u32, hand: Hand, helix: Helix, lead_in: f64, lead_out: f64, gnames: &[u32], rnames: &[u32], crest_relief: f64) -> Option<Shape> {
        unsafe {
            Self::wrap(qym_shape_helical_profile(
                self.ptr,
                origin.as_ptr(),
                dir.as_ptr(),
                radius,
                profile.as_ptr(),
                profile.len(),
                length,
                lead,
                starts.max(1) as i32,
                (hand == Hand::Left) as i32,
                (helix == Helix::Rib) as i32,
                lead_in,
                lead_out,
                gnames.as_ptr(),
                gnames.len(),
                rnames.as_ptr(),
                rnames.len(),
                crest_relief,
            ))
        }
    }

    pub fn thread(&self, origin: [f64; 3], dir: [f64; 3], radius: f64, length: f64, pitch: f64, angle_deg: f64, depth: f64, starts: u32, hand: Hand, site: Site, form: u8, clearance_crest: f64, clearance_root: f64, lead_in: f64, lead_out: f64) -> Option<Shape> {
        unsafe { Self::wrap(qym_shape_thread(self.ptr, origin.as_ptr(), dir.as_ptr(), radius, length, pitch, angle_deg, depth, starts.max(1) as i32, (hand == Hand::Left) as i32, (site == Site::Bore) as i32, form as i32, clearance_crest, clearance_root, lead_in, lead_out)) }
    }
    /// The volume of the body, in mm³, through `GProp`, for tests and geometric checks.
    pub fn volume(&self) -> f64 {
        unsafe { qym_shape_volume(self.ptr) }
    }
    /// B-rep validity, through `BRepCheck_Analyzer`: true for a correct body, false for one broken, doubled
    /// or self-intersecting.
    /// Resolve pinched faces into real ones, a face lying in two places not being a face. It returns how
    /// many were resolved and edits the shape in place together with the name maps.
    pub fn heal_pinched_faces(&self) -> i32 {
        unsafe { qym_shape_heal_pinched_faces(self.ptr) }
    }
    /// A copy of faces as a separate sheet: `idx` says which to copy and `names` how to name the copies.
    pub fn copy_faces(&self, idx: &[u32], names: &[u32]) -> Option<Shape> {
        if idx.len() != names.len() {
            return refuse("face copy/asked", "there is not one name per face to copy");
        }
        unsafe { Self::wrap_valid(qym_shape_copy_faces(self.ptr, idx.as_ptr(), names.as_ptr(), idx.len())) }
    }

    /// A patch: a surface stretched over a chain of edges of the body. `tangent` asks it to meet the edges
    /// smoothly, tangent to the neighbouring faces, rather than merely coinciding with them in position.
    pub fn patch(&self, idx: &[u32], tangent: bool, name: u32) -> Option<Shape> {
        unsafe { Self::wrap_valid(qym_shape_patch(self.ptr, idx.as_ptr(), idx.len(), i32::from(tangent), name)) }
    }

    /// Trim a surface by another body, keeping the piece nearest the point `keep`.
    pub fn trim(&self, tool: &Shape, keep: [f64; 3]) -> Option<Shape> {
        unsafe { Self::wrap_valid(qym_shape_trim(self.ptr, tool.ptr, keep.as_ptr())) }
    }

    /// Stitch sheets into one surface. If it closes, a body comes back; if not, a sheet.
    ///
    /// `Err(())` means no edge joined: the sheets do not touch each other, and a stitch would give the same
    /// two islands under one name.
    pub fn stitch(parts: &[&Shape], tol: f64) -> Result<Shape, ()> {
        if parts.len() < 2 {
            return Err(());
        }
        let ptrs: Vec<*const QymShape> = parts.iter().map(|p| p.ptr as *const QymShape).collect();
        // The bridge counts the stitched and free edges itself and reports "nothing joined" as a zero, which
        // is the only case that has to be told apart here. The rest is visible in the result: if it closed, a
        // body arrived; if not, a sheet.
        let (mut free, mut joined) = (0u32, 0u32);
        let out = unsafe { Self::wrap_valid(qym_shape_stitch(ptrs.as_ptr(), ptrs.len(), tol, &mut free, &mut joined)) };
        out.ok_or(())
    }

    /// Replace faces of a body with a surface: the node where the design layer joins the timeline.
    ///
    /// `Err(n)` means the surface did not meet the opening, with `n` edges left unpaired; a zero means it
    /// failed for some other reason. That number has to reach the outside, since "it did not work" without it
    /// leaves nothing but guesswork.
    pub fn replace_faces(&self, idx: &[u32], surf: &Shape, tol: f64) -> Result<Shape, u32> {
        let mut free = 0u32;
        let out = unsafe { Self::wrap_valid(qym_shape_replace_faces(self.ptr, idx.as_ptr(), idx.len(), surf.ptr, tol, &mut free)) };
        out.ok_or(free)
    }

    /// A sheet, that is a surface rather than a body: there are faces but no solids. A sheet has no volume by
    /// nature, and the funnel of the kernel has to measure it differently from a solid.
    pub fn is_sheet(&self) -> bool {
        unsafe { qym_shape_kind(self.ptr) == 2 }
    }

    /// The shape is empty: neither solids nor faces.
    pub fn is_empty_shape(&self) -> bool {
        unsafe { qym_shape_kind(self.ptr) == 0 }
    }

    /// How many shells the shape has: one for a solid body, two for a hollow one. More than that means the
    /// body is assembled from copies, by a pattern or a mirror, and the kernel cannot offset such a thing.
    pub fn shell_count(&self) -> u32 {
        unsafe { qym_shape_shell_count(self.ptr) as u32 }
    }
    /// How many bodies the shape has, zero for a sheet. More than one means the operation broke the part into
    /// pieces.
    pub fn solid_count(&self) -> u32 {
        unsafe { qym_shape_solid_count(self.ptr) as u32 }
    }
    /// Repair minor flaws of the shape, through `ShapeFix`. `None` means it did not help.
    pub fn healed(&self) -> Option<Shape> {
        unsafe { Self::wrap(qym_shape_heal(self.ptr)) }
    }
    pub fn is_valid(&self) -> bool {
        unsafe { qym_shape_is_valid(self.ptr) != 0 }
    }
    /// A rigid transformation, as a row-major 3×4 of the X, Y and N axes plus the origin, giving a placed
    /// shape.
    pub fn transformed(&self, m: &[f64; 12]) -> Option<Shape> {
        unsafe { Self::wrap(qym_shape_transform(self.ptr, m.as_ptr())) }
    }
    /// Fillet every edge with radius `r`, giving a new shape.
    pub fn fillet_all(&self, r: f64) -> Option<Shape> {
        unsafe { Self::wrap_valid(qym_shape_fillet_all(self.ptr, r)) }
    }
    /// Chamfer every edge by `d`, giving a new shape.
    pub fn chamfer_all(&self, d: f64) -> Option<Shape> {
        unsafe { Self::wrap_valid(qym_shape_chamfer_all(self.ptr, d)) }
    }
    /// The edges of the body as polylines, in the kernel's own edge order, for drawing and picking.
    pub fn edges(&self) -> Vec<Vec<[f32; 3]>> {
        self.edges_with_ids().0
    }
    /// The edges together with their persistent ids, in parallel. An id survives a rebuild, so a selection of
    /// edges does not slide off.
    pub fn edges_with_ids(&self) -> (Vec<Vec<[f32; 3]>>, Vec<u32>) {
        unsafe {
            let e = qym_shape_edges(self.ptr);
            if e.is_null() {
                return (Vec::new(), Vec::new());
            }
            let n = qym_edges_count(e);
            let mut out = Vec::with_capacity(n);
            let mut ids = Vec::with_capacity(n);
            for i in 0..n {
                let pc = qym_edge_point_count(e, i);
                let mut buf = vec![0f32; pc * 3];
                if pc > 0 {
                    qym_edge_copy_points(e, i, buf.as_mut_ptr());
                }
                out.push((0..pc).map(|k| [buf[3 * k], buf[3 * k + 1], buf[3 * k + 2]]).collect());
                ids.push(qym_edge_id(e, i));
            }
            qym_edges_free(e);
            (out, ids)
        }
    }
    /// The edges, their ids, and what is known about circular ones: for each edge `Some((centre, axis,
    /// radius))` if it is a circle or an arc, an analytic curve, and `None` otherwise. This is what a
    /// concentric anchor on a hole or a cylinder rests on.
    ///
    /// A slice of `edges_info`: a historical view of three fields out of five.
    #[allow(clippy::type_complexity)]
    pub fn edges_full(&self) -> (Vec<Vec<[f32; 3]>>, Vec<u32>, Vec<Option<([f64; 3], [f64; 3], f64)>>) {
        let (p, i, c, _s) = self.edges_full_smooth();
        (p, i, c)
    }

    /// As `edges_full`, but with a smoothness flag per edge: a tangent junction of faces near 180° has
    /// nothing left to round or to cut, so such edges are not offered for a fillet or a chamfer and are
    /// filtered out by the kernel.
    #[allow(clippy::type_complexity)]
    pub fn edges_full_smooth(&self) -> (Vec<Vec<[f32; 3]>>, Vec<u32>, Vec<Option<([f64; 3], [f64; 3], f64)>>, Vec<bool>) {
        let mut info = self.edges_info();
        let mut out = (Vec::with_capacity(info.len()), Vec::with_capacity(info.len()), Vec::with_capacity(info.len()), Vec::with_capacity(info.len()));
        for e in info.drain(..) {
            out.0.push(e.poly);
            out.1.push(e.id);
            out.2.push(e.circle);
            out.3.push(e.smooth);
        }
        out
    }

    /// Everything the kernel knows about the edges, in one pass. Walking the edges is expensive, a polyline
    /// per edge, and callers want different parts of the answer, so there is one source and `edges_full*` are
    /// slices of it.
    pub fn edges_info(&self) -> Vec<EdgeInfo> {
        unsafe {
            let e = qym_shape_edges(self.ptr);
            if e.is_null() {
                return Vec::new();
            }
            let n = qym_edges_count(e);
            let mut out = Vec::with_capacity(n);
            for i in 0..n {
                let pc = qym_edge_point_count(e, i);
                let mut buf = vec![0f32; pc * 3];
                if pc > 0 {
                    qym_edge_copy_points(e, i, buf.as_mut_ptr());
                }
                let mut c = [0f64; 7];
                let circle = (qym_edge_circle(e, i, c.as_mut_ptr()) != 0).then(|| ([c[0], c[1], c[2]], [c[3], c[4], c[5]], c[6]));
                let mut r = [0f64; 3];
                let ref_dir = (qym_edge_ref_dir(e, i, r.as_mut_ptr()) != 0).then_some(r);
                out.push(EdgeInfo {
                    poly: (0..pc).map(|k| [buf[3 * k], buf[3 * k + 1], buf[3 * k + 2]]).collect(),
                    id: qym_edge_id(e, i),
                    circle,
                    smooth: qym_edge_smooth(e, i) != 0,
                    ref_dir,
                });
            }
            qym_edges_free(e);
            out
        }
    }

    /// The ids of the smooth edges, the tangent junctions, for the fillet and chamfer filter.
    pub fn smooth_edge_ids(&self) -> std::collections::HashSet<u32> {
        let (_, ids, _, sm) = self.edges_full_smooth();
        ids.into_iter().zip(sm).filter(|&(id, s)| s && id != 0).map(|(id, _)| id).collect()
    }
    /// Fillet the selected edges, by zero-based index, with radius `r`.
    pub fn fillet_edges(&self, r: f64, idx: &[u32]) -> Option<Shape> {
        self.fillet_edges_named(r, idx, &vec![0u32; idx.len()], &vec![0u32; idx.len()], &[])
    }

    /// `names` gives the name of the fillet surface for each selected edge, in parallel with `idx`.
    pub fn fillet_edges_named(&self, r: f64, idx: &[u32], names: &[u32], corners: &[u32], all_names: &[u32]) -> Option<Shape> {
        unsafe { Self::wrap_valid(qym_shape_fillet_edges(self.ptr, r, idx.as_ptr(), idx.len(), Self::opt_ptr(names), Self::opt_ptr(corners), Self::opt_ptr(all_names), all_names.len() / 2)) }
    }
    /// A variable fillet of the selected edges: the radius runs linearly from `r1` at the start to `r2` at
    /// the end.
    /// A variable-radius fillet driven by a table of vertices: `verts` gives a point and the radius at it. An
    /// edge takes the radii of its ends and the kernel drives the law between them; an end with no entry takes
    /// `r_default`. The vertices travel as points, names being the model's business while geometry is enough
    /// for the kernel.
    pub fn fillet_at_vertices(&self, r_default: f64, idx: &[u32], verts: &[([f64; 3], f64)], tol: f64) -> Option<Shape> {
        let pts: Vec<f64> = verts.iter().flat_map(|(p, _)| *p).collect();
        let rads: Vec<f64> = verts.iter().map(|(_, r)| *r).collect();
        unsafe { Self::wrap_valid(qym_shape_fillet_at_vertices(self.ptr, r_default, idx.as_ptr(), idx.len(), pts.as_ptr(), rads.as_ptr(), verts.len(), tol)) }
    }

    pub fn fillet_var(&self, r1: f64, r2: f64, idx: &[u32]) -> Option<Shape> {
        unsafe { Self::wrap_valid(qym_shape_fillet_var(self.ptr, r1, r2, idx.as_ptr(), idx.len())) }
    }
    /// An empty slice is not a null pointer. `[].as_ptr()` returns an aligned but invalid address, and a
    /// `if (ptr)` check on the C++ side lets it through, after which rubbish is read. That is how a chamfer
    /// name check brought the process down with a segfault. An honest `null` is passed when there are no
    /// names.
    fn opt_ptr(v: &[u32]) -> *const u32 {
        if v.is_empty() { std::ptr::null() } else { v.as_ptr() }
    }

    /// Chamfer the selected edges, by zero-based index, by `d`.
    ///
    /// `names` gives the name of the chamfer surface for each selected edge, in parallel with `idx`, and
    /// `corners` the name of the corner patch. The same recipe as for a fillet: these faces have no source
    /// other than the edge and the vertex that produced them.
    pub fn chamfer_edges(&self, d: f64, idx: &[u32], names: &[u32], corners: &[u32], all_names: &[u32]) -> Option<Shape> {
        unsafe { Self::wrap_valid(qym_shape_chamfer_edges(self.ptr, d, idx.as_ptr(), idx.len(), Self::opt_ptr(names), Self::opt_ptr(corners), Self::opt_ptr(all_names), all_names.len() / 2)) }
    }
    /// An asymmetric chamfer. Mode 1 takes two distances, `a` on the reference face and `b` on the adjacent
    /// one; mode 2 takes the leg `a` and the angle `b` in degrees from the reference face. `flip` chooses
    /// which of the two adjacent faces is the reference, and `ref_face` is the persistent id of one picked by
    /// hand, zero meaning it follows `flip`.
    pub fn chamfer_edges_asym(&self, a: f64, b: f64, mode: i32, flip: bool, ref_face: u32, idx: &[u32]) -> Option<Shape> {
        unsafe { Self::wrap_valid(qym_shape_chamfer_edges_asym(self.ptr, a, b, mode, flip as i32, ref_face, idx.as_ptr(), idx.len())) }
    }
    /// A shell: remove the faces with the persistent ids `face_ids`, stable across a rebuild, and build walls
    /// by the signed `offset` — negative inward, which is the default, positive outward.
    ///
    /// `walls` seeds the names of the inner walls as pairs of a source face and the name of its image. Without
    /// it a wall takes the id of its outer face and becomes indistinguishable from it.
    /// The smallest fillet or chamfer on the body; zero means there are no rounded faces.
    pub fn min_round_radius(&self) -> f64 {
        unsafe { qym_shape_min_round_radius(self.ptr) }
    }
    pub fn shell(&self, offset: f64, face_ids: &[u32], walls: &[(u32, u32)]) -> Option<Shape> {
        let (gf, gt): (Vec<u32>, Vec<u32>) = walls.iter().copied().unzip();
        // No validity check here. It stands further on, in the shared `finish` funnel, where an unsound shape
        // is first repaired and only then called broken. Dropping the shape right here turned a clear fault
        // into an unnamed "the operation failed".
        unsafe { Self::wrap(qym_shape_shell(self.ptr, offset, face_ids.as_ptr(), face_ids.len(), gf.as_ptr(), gt.as_ptr(), gf.len())) }
    }
    /// A draft: tilt the faces `face_ids` by `angle_deg` relative to the neutral plane given by `np_origin`
    /// and `np_normal`, whose line of intersection with a face stays put, in the pull direction `pull`, which
    /// is usually the normal of the neutral plane. The faces keep their persistent ids.
    /// Remove faces and heal: the faces go, the neighbours are extended and the body stays closed. This is
    /// how a fillet, a chamfer or a boss is taken off without unpicking the timeline.
    ///
    /// Extending the neighbours is not always possible, a face sometimes carrying the whole shape, and then
    /// `None` comes back: better a refusal than a broken body.
    pub fn remove_faces(&self, face_ids: &[u32]) -> Option<Shape> {
        self.remove_faces_why(face_ids).ok()
    }

    /// The same, but with the cause of a refusal: a face missing from the body and neighbours that cannot be
    /// extended call for different remedies, and one message for both leaves nothing but guesswork.
    pub fn remove_faces_why(&self, face_ids: &[u32]) -> Result<Shape, String> {
        if face_ids.is_empty() {
            return Err("cad-no-faces-picked".into());
        }
        let mut reason: i32 = 0;
        let p = unsafe { qym_shape_remove_faces(self.ptr, face_ids.as_ptr(), face_ids.len(), &mut reason) };
        match Self::wrap(p) {
            Some(sh) => Ok(sh),
            None if reason == -1 => Err("cad-faces-not-in-body".into()),
            None => Err("cad-neighbours-not-extendable".into()),
        }
    }

    /// Push and pull a face: the planar face `face_id` travels along its own normal by `dist`, a positive
    /// value adding material and a negative one cutting it away. This is direct modelling, which the
    /// application did not have at all — a body could only be built from a sketch or a primitive.
    ///
    /// Curved faces are deliberately not supported: moving a cylinder or a sphere is a different operation, a
    /// surface offset, with different behaviour at the junctions, and doing it here as well would give a
    /// silently wrong result on the very first rounded part. Such a face gives `None`.
    pub fn push_face(&self, face_id: u32, dist: f64) -> Option<Shape> {
        unsafe { Self::wrap(qym_shape_push_face(self.ptr, face_id, dist)) }
    }

    /// Split faces by a plane without cutting the body: the body stays one, while the faces the plane crossed
    /// fall into parts. This is how an area for paint, a pad for machining or a zone for a future feature is
    /// marked out without breaking the part apart.
    ///
    /// `None` means the plane passed clear of it, leaving nothing to split, or the kernel did not manage.
    pub fn split_faces(&self, origin: [f64; 3], normal: [f64; 3]) -> Option<Shape> {
        unsafe { Self::wrap_valid(qym_shape_split_faces(self.ptr, origin.as_ptr(), normal.as_ptr())) }
    }

    /// Split a body by a plane: the body is cut into pieces and each piece becomes a separate body.
    ///
    /// There can be more than two pieces — a U-shaped part is cut into three by one plane — so a list comes
    /// back: two halves is a special case, and counting on it means losing pieces silently. The names of faces
    /// and edges are carried into every piece, or the references of fillets and chamfers would slide after the
    /// cut.
    ///
    /// `None` if the plane passed clear of the body, leaving nothing to cut, or the kernel did not manage:
    /// better an honest refusal than an operation that succeeded and changed nothing.
    ///
    /// `section` is the name of the section face, which the cutting plane itself produces; zero means no name
    /// was given.
    pub fn split_by_plane(&self, origin: [f64; 3], normal: [f64; 3], section: u32) -> Option<Vec<Shape>> {
        unsafe {
            let l = qym_shape_split_by_plane(self.ptr, origin.as_ptr(), normal.as_ptr(), section);
            if l.is_null() {
                return None;
            }
            let n = qym_shapelist_count(l);
            let mut out = Vec::with_capacity(n);
            for i in 0..n {
                if let Some(sh) = Shape::wrap(qym_shapelist_get(l, i)) {
                    out.push(sh);
                }
            }
            qym_shapelist_free(l);
            (out.len() >= 2).then_some(out)
        }
    }

    /// `sides` holds pairs of a source face name and the name of the draft side it produced, as a flat list.
    pub fn draft_faces(&self, face_ids: &[u32], angle_deg: f64, pull: [f64; 3], np_origin: [f64; 3], np_normal: [f64; 3], sides: &[u32]) -> Option<Shape> {
        if face_ids.is_empty() {
            return refuse("draft/asked", "not one face was named to tilt");
        }
        unsafe {
            Self::wrap(qym_shape_draft(
                self.ptr,
                face_ids.as_ptr(),
                face_ids.len(),
                angle_deg,
                pull.as_ptr(),
                np_origin.as_ptr(),
                np_normal.as_ptr(),
                Self::opt_ptr(sides),
                sides.len() / 2,
            ))
        }
    }
    /// A stepped hole: a tool — the main cylinder plus a counterbore or countersink — cut in the frame `pl`,
    /// which maps local to world. `kind` is 0 for a plain hole, 1 for a counterbore and 2 for a countersink.
    /// The body with the cut comes back.
    pub fn hole_stepped(&self, kind: u8, pl: [f64; 12], dia: f64, depth: f64, dia2: f64, depth2: f64, _extra: &[u32]) -> Option<Shape> {
        self.hole_stepped_named(kind, pl, dia, depth, dia2, depth2, 0, &[])
    }

    /// `bore` is the name of the wall of the main bore, the one fillets and sketches are placed against.
    pub fn hole_stepped_named(&self, kind: u8, pl: [f64; 12], dia: f64, depth: f64, dia2: f64, depth2: f64, bore: u32, extra: &[u32]) -> Option<Shape> {
        unsafe { Self::wrap(qym_shape_hole_stepped(self.ptr, kind as i32, pl.as_ptr(), dia, depth, dia2, depth2, bore, Self::opt_ptr(extra), extra.len())) }
    }
    /// Many holes at once, at the placements `pls`, each a 3×4 matrix. All the tools are fused and a single
    /// cut is taken.
    pub fn holes_stepped(&self, kind: u8, pls: &[[f64; 12]], dia: f64, depth: f64, dia2: f64, depth2: f64) -> Option<Shape> {
        self.holes_stepped_named(kind, pls, dia, depth, dia2, depth2, &vec![0u32; pls.len()], &[])
    }

    /// `bores` gives the wall name for each hole, one per placement.
    pub fn holes_stepped_named(&self, kind: u8, pls: &[[f64; 12]], dia: f64, depth: f64, dia2: f64, depth2: f64, bores: &[u32], extra: &[u32]) -> Option<Shape> {
        if pls.is_empty() {
            return None;
        }
        let flat: Vec<f64> = pls.iter().flatten().copied().collect();
        unsafe { Self::wrap(qym_shape_holes_stepped(self.ptr, kind as i32, flat.as_ptr(), pls.len(), dia, depth, dia2, depth2, bores.as_ptr(), Self::opt_ptr(extra), extra.len())) }
    }
    /// A shell centred on the surface: a wall of thickness `t` is centred on the original surface, with the
    /// faces `face_ids` left open. The body grows by +t/2 and is then hollowed by −t. `None` means the offset
    /// failed.
    pub fn shell_center(&self, t: f64, face_ids: &[u32]) -> Option<Shape> {
        unsafe { Self::wrap_valid(qym_shape_shell_center(self.ptr, t, face_ids.as_ptr(), face_ids.len())) }
    }
    /// The axis of a cylindrical or conical face by its persistent id, as an origin and a direction in the
    /// local frame of the body. `None` means the face was not found or is not round, being a plane or a
    /// spline. This is what picking the axis of a circular pattern rests on.
    pub fn face_axis(&self, face_id: u32) -> Option<([f64; 3], [f64; 3])> {
        let (mut origin, mut dir) = ([0.0f64; 3], [0.0f64; 3]);
        let ok = unsafe { qym_shape_face_axis(self.ptr, face_id, origin.as_mut_ptr(), dir.as_mut_ptr()) };
        (ok == 1).then_some((origin, dir))
    }
    /// Thicken a face: the face `face_id` becomes a plate of thickness `thickness`, the sign choosing which
    /// side grows. This is how a part is made from a curved surface — take a face of the body, thicken it by
    /// 2 mm and the skin is ready.
    ///
    /// `None` means the face was not found, the thickness is zero, or the offset failed by intersecting
    /// itself.
    ///
    /// The names come from above as pairs of before and after: `fmap` maps the name of a source face to the
    /// name of its offset side, and `emap` the name of an edge to the name of the wall it produced. The naming
    /// scheme is known to the model layer; the kernel only carries it.
    pub fn thicken_face(&self, face_id: u32, thickness: f64, fmap: &[u32], emap: &[u32]) -> Option<Shape> {
        unsafe { Self::wrap_valid(qym_shape_thicken_face(self.ptr, face_id, thickness, fmap.as_ptr(), fmap.len() / 2, emap.as_ptr(), emap.len() / 2)) }
    }

    /// The same, but joined to the body, keeping to the rule that a part is one body.
    ///
    /// The names of the source body are carried over, so references to its faces and edges survive the
    /// operation, while the new geometry of the plate gets its own. As a separate plate the part became two
    /// bodies — a differently coloured piece on screen instead of one part.
    pub fn thicken_face_join(&self, face_id: u32, thickness: f64, fmap: &[u32], emap: &[u32]) -> Option<Shape> {
        unsafe { Self::wrap_valid(qym_shape_thicken_face_join(self.ptr, face_id, thickness, fmap.as_ptr(), fmap.len() / 2, emap.as_ptr(), emap.len() / 2)) }
    }

    /// A cylindrical face: the axis and the radius in the local frame of the body. `face_axis` gives no
    /// radius, and a measurement wants exactly that — the wall of a hole is measured for its diameter, not for
    /// where its axis lies. `None` means the face was not found or is not a cylinder, being a plane, a cone or
    /// a spline.
    pub fn face_cylinder(&self, face_id: u32) -> Option<([f64; 3], [f64; 3], f64)> {
        let (mut origin, mut dir, mut r) = ([0.0f64; 3], [0.0f64; 3], 0.0f64);
        let ok = unsafe { qym_shape_face_cylinder(self.ptr, face_id, origin.as_mut_ptr(), dir.as_mut_ptr(), &mut r) };
        (ok == 1).then_some((origin, dir, r))
    }

    /// The persistent ids of every edge of the face `face_id`, so that picking a face selects all its edges
    /// for a chamfer or a fillet. Empty means the face was not found or has no named edges.
    /// Absorbed names, as pairs of a former face name and the name of the common face. Fusing coplanar faces
    /// into a monolith collapses two named faces into one, and one of the names has to give way. The pair
    /// tells the model layer which name it gave way to, so a reference to the former one finds the common face
    /// rather than getting lost. Without this a fillet was lost wherever two walls honestly became one.
    /// What tells two edges of the same pair of faces apart: `(edge, name1, name2)`, the two smallest names
    /// of named faces meeting at its ends, excluding its own two; a zero means there is no such face. This
    /// exists so that the rank of an edge within a pair is not assigned by traversal order; see
    /// `name_edges_of`.
    pub fn edge_end_faces(&self) -> Vec<(u32, u32, u32)> {
        unsafe {
            let n = qym_shape_edge_end_faces(self.ptr, std::ptr::null_mut(), 0);
            if n == 0 {
                return Vec::new();
            }
            let mut buf = vec![0u32; n * 3];
            let got = qym_shape_edge_end_faces(self.ptr, buf.as_mut_ptr(), buf.len());
            buf.truncate(got * 3);
            buf.chunks_exact(3).map(|c| (c[0], c[1], c[2])).collect()
        }
    }

    pub fn absorbed_names(&self) -> Vec<(u32, u32)> {
        unsafe {
            let n = qym_shape_absorbed(self.ptr, std::ptr::null_mut(), 0);
            if n == 0 {
                return Vec::new();
            }
            let mut buf = vec![0u32; n * 2];
            let got = qym_shape_absorbed(self.ptr, buf.as_mut_ptr(), n);
            buf.truncate(got * 2);
            buf.chunks_exact(2).map(|c| (c[0], c[1])).collect()
        }
    }

    /// The split faces, as the id of the piece, the name of the source face and the number of the piece.
    /// Clear the records of the pieces. They are single-use: the model layer has read them and handed out the
    /// names, and records left behind would be folded into the next operation's group, which would then elect
    /// a different keeper of the name.
    pub fn clear_face_splits(&self) {
        unsafe { qym_shape_clear_face_splits(self.ptr) }
    }

    pub fn face_splits(&self) -> Vec<(u32, u32, u32)> {
        unsafe {
            let n = qym_shape_face_splits(self.ptr, std::ptr::null_mut(), 0);
            if n == 0 {
                return Vec::new();
            }
            let mut buf = vec![0u32; n * 3];
            let got = qym_shape_face_splits(self.ptr, buf.as_mut_ptr(), buf.len());
            buf.truncate(got.min(n) * 3);
            buf.chunks_exact(3).map(|c| (c[0], c[1], c[2])).collect()
        }
    }

    /// Rewrite the face names, as pairs of before and after.
    pub fn rename_faces(&self, pairs: &[(u32, u32)]) {
        if pairs.is_empty() {
            return;
        }
        let (from, to): (Vec<u32>, Vec<u32>) = pairs.iter().copied().unzip();
        unsafe { qym_shape_rename_faces(self.ptr, from.as_ptr(), to.as_ptr(), from.len()) }
    }

    /// Pairs of an edge and its two faces, by name: the id of the edge and the names of faces A and B. Only
    /// edges with exactly two adjacent faces appear, those being the ones an edge name is derived from.
    pub fn edge_face_pairs(&self) -> Vec<(u32, u32, u32)> {
        unsafe {
            let n = qym_shape_edge_face_pairs(self.ptr, std::ptr::null_mut(), 0);
            if n == 0 {
                return Vec::new();
            }
            let mut buf = vec![0u32; n * 3];
            let got = qym_shape_edge_face_pairs(self.ptr, buf.as_mut_ptr(), buf.len());
            buf.truncate(got.min(n) * 3);
            buf.chunks_exact(3).map(|c| (c[0], c[1], c[2])).collect()
        }
    }

    /// Rewrite the edge names, as pairs of before and after.
    pub fn rename_edges(&self, pairs: &[(u32, u32)]) {
        if pairs.is_empty() {
            return;
        }
        let (from, to): (Vec<u32>, Vec<u32>) = pairs.iter().copied().unzip();
        unsafe { qym_shape_rename_edges(self.ptr, from.as_ptr(), to.as_ptr(), from.len()) }
    }

    pub fn face_edge_ids(&self, face_id: u32) -> Vec<u32> {
        let mut buf = vec![0u32; 256];
        let n = unsafe { qym_shape_face_edge_ids(self.ptr, face_id, buf.as_mut_ptr(), buf.len()) };
        buf.truncate(n);
        buf
    }
    /// Mirror about a plane through the origin — 0 for XY, 1 for XZ, 2 for YZ — giving a new shape.
    /// Mirror about an arbitrary plane, given as an origin and a normal: a datum or a face.
    pub fn mirrored_plane(&self, origin: [f64; 3], normal: [f64; 3]) -> Option<Shape> {
        unsafe { Self::wrap(qym_shape_mirror_plane(self.ptr, origin.as_ptr(), normal.as_ptr())) }
    }
    pub fn mirrored(&self, plane: u8) -> Option<Shape> {
        unsafe { Self::wrap(qym_shape_mirror(self.ptr, plane as i32)) }
    }
    /// The volume of intersection with another body, in mm³, which is how interference in an assembly is
    /// detected. `Some(0.0)` means they do not penetrate, their bounding boxes being apart or the contact
    /// being only a touch. Both bodies have to be in the same frame; see `transformed`.
    ///
    /// `None` means the kernel could not measure — which is NOT the same as "they are clear of each other",
    /// and must never be treated as one. Whoever reads this decides what an unknown is worth; for an
    /// assembly it is worth a warning, because an unmeasurable pair is exactly the pair worth looking at.
    pub fn interference_volume(&self, other: &Shape) -> Option<f64> {
        let mut v = 0.0f64;
        (unsafe { qym_shape_interference_volume(self.ptr, other.ptr, &mut v) } == 1).then_some(v)
    }
    /// The bounding box of the body in its own frame, as `[xmin, ymin, zmin, xmax, ymax, zmax]` in mm.
    /// `None` means an empty or broken shape.
    pub fn bbox(&self) -> Option<[f64; 6]> {
        let mut out = [0.0f64; 6];
        (unsafe { qym_shape_bbox(self.ptr, out.as_mut_ptr()) } == 1).then_some(out)
    }

    /// The diagonal of the bounding box in mm: the characteristic size of the body. A zero means the box
    /// could not be computed.
    pub fn bbox_diag(&self) -> f64 {
        let Some(b) = self.bbox() else { return 0.0 };
        let (dx, dy, dz) = (b[3] - b[0], b[4] - b[1], b[5] - b[2]);
        let d = (dx * dx + dy * dy + dz * dz).sqrt();
        if d.is_finite() { d } else { 0.0 }
    }

    /// Tessellate into bodies, for drawing and for machining.
    pub fn tessellate(&self, deflection: f64) -> Vec<Body> {
        let defl = if deflection > 0.0 { deflection } else { 0.5 };
        unsafe { doc_to_bodies(qym_shape_tessellate(self.ptr, defl)) }
    }

    /// Tessellation with a deflection adapted to the size of the body; see [`adaptive_deflection`].
    pub fn tessellate_auto(&self, k: f64) -> Vec<Body> {
        self.tessellate(adaptive_deflection(self.bbox_diag(), k))
    }

    /// [`Shape::tessellate_merged`] with an adaptive deflection: the working path for drawing.
    pub fn tessellate_merged_auto(&self, k: f64) -> Option<Body> {
        self.tessellate_merged(adaptive_deflection(self.bbox_diag(), k))
    }

    /// Tessellation of the whole shape into one mesh: the solids of a compound are merged — a pattern, a
    /// mirror together with its original, a cut that fell into pieces. Otherwise one body node would show only
    /// the first solid. `None` means empty.
    pub fn tessellate_merged(&self, deflection: f64) -> Option<Body> {
        let bodies = self.tessellate(deflection);
        if bodies.is_empty() {
            return None;
        }
        let (mut verts, mut tris, mut faces) = (Vec::new(), Vec::new(), Vec::new());
        for (m, fs) in bodies {
            let voff = verts.len() as u32;
            let toff = tris.len() as u32;
            verts.extend(m.verts);
            tris.extend(m.tris.into_iter().map(|t| [t[0] + voff, t[1] + voff, t[2] + voff]));
            for mut f in fs {
                f.triangles.iter_mut().for_each(|ti| *ti += toff);
                faces.push(f);
            }
        }
        Some((qymcad_core::geom::Mesh { verts, tris }, faces))
    }
}

/// The linear tessellation deflection, taken from the characteristic size of the body — the diagonal of its
/// bounding box, in mm.
///
/// A fixed 0.5 mm for everything left a 5 mm part faceted, the deflection being larger than the part itself,
/// and a 3 m frame unmanageable in triangles. With the deflection a fraction of the size, the triangle count of
/// a body is roughly constant whatever the scale.
///
/// The fraction is 0.15% of the diagonal, about 30 segments around a circle spanning the box, clamped to
/// `[0.002, 1.0]` mm: from below so that a tiny part does not produce millions of triangles, from above so that
/// the holes of a huge frame do not degenerate into triangles — the angular deflection in the tessellator keeps
/// a minimum number of segments in any case. A `diag` of zero or less, the box not having been computed, falls
/// back to the former 0.5 mm.
///
/// `k`, the fraction of the diagonal, is set by the document through `GeomQuality::deflection_k` rather than by
/// a constant here: the geometric tolerance has to travel with the file, or one project would give two people
/// different STLs.
pub fn adaptive_deflection(diag: f64, k: f64) -> f64 {
    const MIN: f64 = 0.002;
    const MAX: f64 = 1.0;
    if !diag.is_finite() || diag <= 0.0 {
        return 0.5;
    }
    (diag * k).clamp(MIN, MAX)
}

/// Read a STEP file and return a shape per solid, in parallel with [`import_step`].
pub fn step_solids(path: &str) -> Result<Vec<Shape>, String> {
    let c = CString::new(path).map_err(|e| e.to_string())?;
    unsafe {
        let l = qym_step_solids(c.as_ptr());
        if l.is_null() {
            return Err("cad-step-no-shapes".into());
        }
        let n = qym_shapelist_count(l);
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            if let Some(s) = Shape::wrap(qym_shapelist_get(l, i)) {
                out.push(s);
            }
        }
        qym_shapelist_free(l);
        Ok(out)
    }
}

/// Write bodies into a single STEP file as exact B-rep, each with its own 3×4 world transform, which for an
/// assembly comes from `body_world_transform`. The units are millimetres. `Err` on an empty set or a kernel
/// failure.
pub fn write_step(bodies: &[(&Shape, [f64; 12])], path: &str) -> Result<(), String> {
    if bodies.is_empty() {
        return Err("cad-step-nothing-to-export".into());
    }
    let c = CString::new(path).map_err(|e| e.to_string())?;
    let ptrs: Vec<*const QymShape> = bodies.iter().map(|(s, _)| s.ptr as *const QymShape).collect();
    let mats: Vec<f64> = bodies.iter().flat_map(|(_, m)| *m).collect();
    let rc = unsafe { qym_step_write(ptrs.as_ptr(), mats.as_ptr(), ptrs.len(), c.as_ptr()) };
    if rc == 0 {
        Ok(())
    } else {
        Err(format!("cad-step-write-failed#{rc}"))
    }
}

/// Copy every body out of an opaque handle and free it.
///
/// # Safety
/// `d` is a valid non-null pointer from `qym_occt_*` and is not used afterwards.
unsafe fn doc_to_bodies(d: *mut QymDoc) -> Vec<Body> {
    let nb = qym_doc_body_count(d);
    let mut out = Vec::with_capacity(nb);
    for i in 0..nb {
        let vc = qym_body_vert_count(d, i);
        let tc = qym_body_tri_count(d, i);
        let fc = qym_body_face_count(d, i);

        let mut vbuf = vec![0f32; vc * 3];
        let mut tbuf = vec![0u32; tc * 3];
        if vc > 0 {
            qym_body_copy_verts(d, i, vbuf.as_mut_ptr());
        }
        if tc > 0 {
            qym_body_copy_tris(d, i, tbuf.as_mut_ptr());
        }
        let verts = vbuf.chunks_exact(3).map(|c| Point3::new(c[0] as f64, c[1] as f64, c[2] as f64)).collect();
        let tris = tbuf.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();
        let mesh = Mesh { verts, tris };

        // faces from the B-rep: each is a contiguous range of triangles plus a persistent id
        let mut starts = vec![0u32; fc];
        let mut counts = vec![0u32; fc];
        let mut ids = vec![0u32; fc];
        if fc > 0 {
            qym_body_copy_face_starts(d, i, starts.as_mut_ptr());
            qym_body_copy_face_counts(d, i, counts.as_mut_ptr());
            qym_body_copy_face_ids(d, i, ids.as_mut_ptr());
        }
        let mut anchors = vec![0f64; fc * 7];
        if fc > 0 {
            qym_body_copy_face_anchors(d, i, anchors.as_mut_ptr());
        }
        let faces = starts
            .iter()
            .zip(counts.iter())
            .zip(ids.iter())
            .enumerate()
            .map(|(k, ((&s, &c), &fid))| {
                let mut f = mesh.meshface_from_triangles((s..s + c).collect());
                f.id = fid;
                // An exact anchor from the B-rep rather than from the tessellation: the normal is analytic,
                // and for a plane the centroid is projected onto the exact plane. Otherwise a sketch on a face
                // landed with an error of about 1e-5, bosses came out nearly coplanar, the unifier did not
                // merge the seams, and the result was fragmentation and red fillets.
                let a = &anchors[k * 7..k * 7 + 7];
                let n2 = a[3] * a[3] + a[4] * a[4] + a[5] * a[5];
                if n2 > 0.5 {
                    f.normal = [a[3], a[4], a[5]];
                    if a[6] > 0.5 {
                        let d = (f.centroid.x - a[0]) * a[3] + (f.centroid.y - a[1]) * a[4] + (f.centroid.z - a[2]) * a[5];
                        f.centroid.x -= d * a[3];
                        f.centroid.y -= d * a[4];
                        f.centroid.z -= d * a[5];
                    }
                }
                f
            })
            .collect();

        out.push((mesh, faces));
    }
    qym_doc_free(d);
    out
}

/// Import a STEP file into bodies, one per solid, each with its B-rep faces. `deflection` is the linear
/// tessellation deflection in mm.
pub fn import_step(path: &str, deflection: f64) -> Result<Vec<Body>, String> {
    let c = CString::new(path).map_err(|e| e.to_string())?;
    let defl = if deflection > 0.0 { deflection } else { 0.5 };
    unsafe {
        let d = qym_occt_step_read(c.as_ptr(), defl);
        if d.is_null() {
            return Err("cad-step-read-failed".into());
        }
        let bodies = doc_to_bodies(d);
        if bodies.is_empty() {
            return Err("cad-step-empty-tessellation".into());
        }
        Ok(bodies)
    }
}

/// Extrude a closed profile — XY points at z = 0 — to a height of `height`, giving bodies. `xy` holds the
/// coordinates in pairs, as `[x0, y0, x1, y1, ...]`.
pub fn extrude(xy: &[f64], height: f64, deflection: f64) -> Result<Vec<Body>, String> {
    let n = xy.len() / 2;
    if n < 3 {
        return Err("cad-extrude-needs-3-points".into());
    }
    let defl = if deflection > 0.0 { deflection } else { 0.5 };
    unsafe {
        let d = qym_occt_extrude(xy.as_ptr(), n, height, defl);
        if d.is_null() {
            return Err("cad-extrude-failed".into());
        }
        let bodies = doc_to_bodies(d);
        if bodies.is_empty() {
            return Err("cad-extrude-empty".into());
        }
        Ok(bodies)
    }
}

/// Revolve a closed profile about an axis — `axis` 0 for X, 1 for Y — through `angle_deg`, giving bodies.
pub fn revolve(xy: &[f64], axis: u8, angle_deg: f64, deflection: f64) -> Result<Vec<Body>, String> {
    let n = xy.len() / 2;
    if n < 3 {
        return Err("cad-revolve-needs-3-points".into());
    }
    let defl = if deflection > 0.0 { deflection } else { 0.5 };
    let ang = if angle_deg.abs() < 1e-6 { 360.0 } else { angle_deg };
    unsafe {
        let d = qym_occt_revolve(xy.as_ptr(), n, axis as i32, ang, defl);
        if d.is_null() {
            return Err("cad-revolve-failed".into());
        }
        let bodies = doc_to_bodies(d);
        if bodies.is_empty() {
            return Err("cad-revolve-empty".into());
        }
        Ok(bodies)
    }
}

/// A boolean over two extruded profiles. `op` is 0 to subtract the tool from the base, 1 to unite and 2 to
/// intersect.
pub fn extrude_bool(base: &[f64], base_h: f64, tool: &[f64], tool_h: f64, op: u8, deflection: f64) -> Result<Vec<Body>, String> {
    if base.len() / 2 < 3 || tool.len() / 2 < 3 {
        return Err("cad-boolean-needs-3-points".into());
    }
    let defl = if deflection > 0.0 { deflection } else { 0.5 };
    unsafe {
        let d = qym_occt_extrude_bool(base.as_ptr(), base.len() / 2, base_h, tool.as_ptr(), tool.len() / 2, tool_h, op as i32, defl);
        if d.is_null() {
            return Err("cad-boolean-failed".into());
        }
        let bodies = doc_to_bodies(d);
        if bodies.is_empty() {
            return Err("cad-boolean-empty".into());
        }
        Ok(bodies)
    }
}

/// A tessellated box from the kernel, for checking that the linking and the pipeline work.
pub fn box_mesh(dx: f64, dy: f64, dz: f64, deflection: f64) -> Mesh {
    let defl = if deflection > 0.0 { deflection } else { 0.5 };
    unsafe {
        let d = qym_occt_box_doc(dx, dy, dz, defl);
        let mut bodies = doc_to_bodies(d);
        if bodies.is_empty() {
            return Mesh { verts: Vec::new(), tris: Vec::new() };
        }
        bodies.remove(0).0
    }
}
