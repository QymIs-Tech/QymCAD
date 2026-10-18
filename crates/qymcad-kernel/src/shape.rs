//! THE SHAPE AND THE ONE DOOR TO C++: the entries of the C++ side are declared here, private to this module, and every
//! call of one stands here, wrapped - a refusal said, a result checked for validity, the memory owned. The door, the
//! operations, the kernel and every module beside them build through `Shape`; none of them can reach an entry of
//! C++ past it, the compiler refuses the call.

use std::ffi::CString;
use std::os::raw::{c_char, c_double};

use qymcad_core::geom::{Built, Mesh, Point3};

#[repr(C)]
pub(crate) struct QymDoc {
    _private: [u8; 0],
}

extern "C" {
    fn qym_occt_step_read(path: *const c_char, defl: c_double) -> *mut QymDoc;
    #[cfg(test)]
    fn qym_occt_box_doc(dx: c_double, dy: c_double, dz: c_double, defl: c_double) -> *mut QymDoc;
    #[cfg(test)]
    fn qym_occt_extrude(xy: *const c_double, n: usize, height: c_double, defl: c_double) -> *mut QymDoc;
    #[cfg(test)]
    fn qym_occt_revolve(xy: *const c_double, n: usize, axis: i32, angle_deg: c_double, defl: c_double) -> *mut QymDoc;
    #[cfg(test)]
    fn qym_occt_extrude_bool(base_xy: *const c_double, nb: usize, base_h: c_double, tool_xy: *const c_double, nt: usize, tool_h: c_double, op: i32, defl: c_double) -> *mut QymDoc;
    fn qym_doc_body_count(d: *const QymDoc) -> usize;
    fn qym_doc_face_count(d: *const QymDoc) -> usize;
    fn qym_set_parallel(on: i32);
    fn qym_set_threads(n: i32);
    #[cfg(test)]
    fn qym_shape_fix_shell(s: *const QymShape) -> *mut QymShape;
    #[cfg(test)]
    fn qym_shape_sew(s: *const QymShape, tol: c_double) -> *mut QymShape;
    fn qym_doc_unmeshed_faces(d: *const QymDoc) -> usize;
    fn qym_doc_meshed_faces(d: *const QymDoc) -> usize;
    fn qym_body_vert_count(d: *const QymDoc, i: usize) -> usize;
    fn qym_body_tri_count(d: *const QymDoc, i: usize) -> usize;
    fn qym_body_face_count(d: *const QymDoc, i: usize) -> usize;
    fn qym_body_copy_verts(d: *const QymDoc, i: usize, out: *mut f32);
    fn qym_body_copy_normals(d: *const QymDoc, i: usize, out: *mut f32);
    fn qym_body_copy_tris(d: *const QymDoc, i: usize, out: *mut u32);
    fn qym_body_copy_face_starts(d: *const QymDoc, i: usize, out: *mut u32);
    fn qym_body_copy_face_counts(d: *const QymDoc, i: usize, out: *mut u32);
    fn qym_body_copy_face_ids(d: *const QymDoc, i: usize, out: *mut u32);
    fn qym_body_copy_face_anchors(d: *const QymDoc, i: usize, out: *mut f64);
    fn qym_doc_free(d: *mut QymDoc);
}

#[repr(C)]
pub(crate) struct QymShape {
    _private: [u8; 0],
}
#[repr(C)]
pub(crate) struct QymShapeList {
    _private: [u8; 0],
}
#[repr(C)]
pub(crate) struct QymTree {
    _private: [u8; 0],
}
#[repr(C)]
pub(crate) struct QymEdges {
    _private: [u8; 0],
}

extern "C" {
    fn qym_shape_extrude(xy: *const c_double, n: usize, h: c_double) -> *mut QymShape;
    #[cfg(test)]
    fn qym_shape_bezier_shell(pts: *const c_double, patches: usize, tol: c_double, make_solid: i32, free_edges: *mut u32) -> *mut QymShape;
    fn qym_shape_revolve(xy: *const c_double, n: usize, axis: i32, angle_deg: c_double) -> *mut QymShape;
    fn qym_shape_extrude_profile(data: *const c_double, n: usize, h: c_double, cap0: u32, cap1: u32) -> *mut QymShape;
    #[cfg(any(test, feature = "specimens"))]
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
    fn qym_shape_vertices(s: *const QymShape, pts: *mut f64, ids: *mut u32, cap: usize) -> usize;
    fn qym_shape_edge_vertices(s: *const QymShape, ids: *mut u32, ends: *mut u32, cap: usize) -> usize;
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
    #[cfg(test)]
    fn qym_shape_fillet_var(s: *const QymShape, r1: c_double, r2: c_double, idx: *const u32, n: usize) -> *mut QymShape;
    fn qym_shape_fillet_at_vertices(
        s: *const QymShape,
        r_default: c_double,
        idx: *const u32,
        n: usize,
        vpts: *const f64,
        vrads: *const f64,
        m: usize,
        tol: c_double,
        names: *const u32,
        corners: *const u32,
        all_names: *const u32,
        n_all: usize,
    ) -> *mut QymShape;
    fn qym_shape_chamfer_edges(s: *const QymShape, d: c_double, idx: *const u32, n: usize, names: *const u32, corners: *const u32, all_names: *const u32, n_all: usize) -> *mut QymShape;
    fn qym_shape_chamfer_edges_asym(
        s: *const QymShape,
        a: c_double,
        b: c_double,
        mode: i32,
        flip: i32,
        ref_face: u32,
        idx: *const u32,
        n: usize,
        names: *const u32,
        corners: *const u32,
        all_names: *const u32,
        n_all: usize,
    ) -> *mut QymShape;
    fn qym_shape_blend_limit(s: *const QymShape, how: *const BlendLimit, idx: *const u32, n: usize, names: *const u32) -> *mut QymShape;
    fn qym_shape_shell(s: *const QymShape, t: c_double, idx: *const u32, n: usize, gfrom: *const u32, gto: *const u32, gn: usize) -> *mut QymShape;
    fn qym_shape_remove_faces(s: *const QymShape, ids: *const u32, n: usize, out_reason: *mut i32) -> *mut QymShape;
    fn qym_shape_push_face(s: *const QymShape, fid: u32, dist: c_double) -> *mut QymShape;
    #[cfg(test)]
    fn qym_shape_thicken_face(s: *const QymShape, fid: u32, thickness: c_double, fmap: *const u32, nf: usize, emap: *const u32, ne: usize) -> *mut QymShape;
    fn qym_shape_thicken_face_join(s: *const QymShape, fid: u32, thickness: c_double, fmap: *const u32, nf: usize, emap: *const u32, ne: usize) -> *mut QymShape;
    fn qym_shape_face_cylinder(s: *const QymShape, fid: u32, origin: *mut c_double, dir: *mut c_double, radius: *mut c_double) -> i32;
    fn qym_shape_split_faces(s: *const QymShape, origin: *const c_double, normal: *const c_double) -> *mut QymShape;
    fn qym_shape_split_by_plane(s: *const QymShape, origin: *const c_double, normal: *const c_double, section: u32) -> *mut QymShapeList;
    fn qym_shape_draft(
        s: *const QymShape,
        idx: *const u32,
        n: usize,
        angle_deg: c_double,
        pull: *const f64,
        np_origin: *const f64,
        np_normal: *const f64,
        sides: *const u32,
        nsides: usize,
    ) -> *mut QymShape;
    fn qym_shape_face_splits(s: *const QymShape, out: *mut u32, cap: usize) -> usize;
    fn qym_shape_clear_face_splits(s: *mut QymShape);
    fn qym_shape_absorbed(s: *const QymShape, out: *mut u32, max_pairs: usize) -> usize;
    fn qym_shape_rename_faces(s: *mut QymShape, from: *const u32, to: *const u32, n: usize);
    fn qym_shape_edge_face_pairs(s: *const QymShape, out: *mut u32, cap: usize) -> usize;
    fn qym_shape_face_places(s: *const QymShape, ids: *mut u32, out: *mut f64, cap: usize) -> usize;
    fn qym_shape_edge_end_faces(s: *const QymShape, out: *mut u32, cap: usize) -> usize;
    fn qym_shape_rename_edges(s: *mut QymShape, from: *const u32, to: *const u32, n: usize);
    fn qym_shape_hole_stepped(
        s: *const QymShape,
        kind: i32,
        pl: *const f64,
        dia: c_double,
        depth: c_double,
        dia2: c_double,
        depth2: c_double,
        bore: u32,
        extra: *const u32,
        n_extra: usize,
    ) -> *mut QymShape;
    fn qym_shape_holes_stepped(
        s: *const QymShape,
        kind: i32,
        pls: *const f64,
        n_holes: usize,
        dia: c_double,
        depth: c_double,
        dia2: c_double,
        depth2: c_double,
        bores: *const u32,
        extra: *const u32,
        n_extra: usize,
    ) -> *mut QymShape;
    fn qym_shape_shell_center(s: *const QymShape, t: c_double, idx: *const u32, n: usize) -> *mut QymShape;
    fn qym_shape_face_axis(s: *const QymShape, face_id: u32, origin: *mut f64, dir: *mut f64) -> i32;
    fn qym_shape_face_edge_ids(s: *const QymShape, face_id: u32, out: *mut u32, cap: usize) -> usize;
    fn qym_shape_mirror(s: *const QymShape, plane: i32) -> *mut QymShape;
    fn qym_shape_mirror_plane(s: *const QymShape, origin: *const f64, normal: *const f64) -> *mut QymShape;
    fn qym_shape_interference_volume(a: *const QymShape, b: *const QymShape, out: *mut c_double) -> i32;
    fn qym_why() -> *const std::os::raw::c_char;
    fn qym_why_clear();
    fn qym_why_set(text: *const std::os::raw::c_char);
    fn qym_shape_helical_profile(
        base: *const QymShape,
        origin: *const f64,
        dir: *const f64,
        radius: c_double,
        prof: *const f64,
        nprof: usize,
        length: c_double,
        lead: c_double,
        starts: i32,
        left: i32,
        mode: i32,
        lead_in: c_double,
        lead_out: c_double,
        gnames: *const u32,
        gn: usize,
        rnames: *const u32,
        rn: usize,
        crest_relief: c_double,
    ) -> *mut QymShape;
    #[cfg(test)]
    fn qym_shape_thread(
        base: *const QymShape,
        origin: *const f64,
        dir: *const f64,
        radius: c_double,
        length: c_double,
        pitch: c_double,
        angle_deg: c_double,
        depth: c_double,
        starts: i32,
        left: i32,
        internal: i32,
        form: i32,
        clearance_crest: c_double,
        clearance_root: c_double,
        lead_in: c_double,
        lead_out: c_double,
    ) -> *mut QymShape;
    fn qym_shape_volume(s: *const QymShape) -> c_double;
    fn qym_shape_bbox(s: *const QymShape, out: *mut f64) -> i32;
    fn qym_shape_bbox_exact(s: *const QymShape, out: *mut f64) -> i32;
    fn qym_shape_is_valid(s: *const QymShape) -> i32;
    fn qym_shape_min_round_radius(s: *const QymShape) -> f64;
    fn qym_shape_solid_count(s: *const QymShape) -> i32;
    fn qym_shape_offset_faces(s: *const QymShape, idx: *const u32, names: *const u32, n: usize, dist: c_double) -> *mut QymShape;
    fn qym_shape_solids_info(s: *const QymShape, centres: *mut c_double, volumes: *mut c_double, max: i32) -> i32;
    fn qym_shape_solid_at(s: *const QymShape, index: i32) -> *mut QymShape;
    fn qym_shape_face_kinds(s: *const QymShape, out: *mut i32) -> i32;
    fn qym_shape_shell_count(s: *const QymShape) -> i32;
    fn qym_shapes_share(a: *const QymShape, b: *const QymShape) -> i32;
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
    fn qym_shape_drop_slivers(s: *mut QymShape, width: c_double) -> i32;
    fn qym_shape_sliver_count(s: *const QymShape) -> i32;
    fn qym_step_solids(path: *const c_char) -> *mut QymShapeList;
    fn qym_shapelist_count(l: *const QymShapeList) -> usize;
    fn qym_shapelist_get(l: *const QymShapeList, i: usize) -> *mut QymShape;
    fn qym_shapelist_free(l: *mut QymShapeList);
    fn qym_step_write(shapes: *const *const QymShape, mats: *const f64, n: usize, path: *const c_char) -> i32;
    fn qym_occt_iges_read(path: *const c_char, defl: f64) -> *mut QymDoc;
    fn qym_shape_from_mesh(verts: *const f64, nv: usize, tris: *const u32, nt: usize) -> *mut QymShape;
    fn qym_shape_from_faces(
        ns: usize,
        skind: *const i32,
        sparam: *const f64,
        nv: usize,
        vxyz: *const f64,
        ne: usize,
        ekind: *const i32,
        eparam: *const f64,
        eends: *const i64,
        epstart: *const usize,
        epts: *const f64,
        nf: usize,
        fsurface: *const i32,
        foutward: *const i32,
        finside: *const f64,
        floops: *const usize,
        lstart: *const usize,
        litems: *const i64,
        nloose: usize,
        loose: *const f64,
        tol: c_double,
        out_faces: *mut u32,
        out_area: *mut f64,
        out_free: *mut u32,
        out_free_at: *mut f64,
        cap_free: usize,
        out_dropped: *mut u32,
        faces_only: i32,
        fpstart: *const usize,
        fpts: *const f64,
    ) -> *mut QymShape;
    fn qym_faces_cache(on: i32);
    fn qym_exact_read(format: i32, path: *const c_char, defl: f64, doc: *mut *mut QymDoc, solids: *mut *mut QymShapeList) -> i32;
    fn qym_exact_read_tree(format: i32, path: *const c_char, defl: f64, doc: *mut *mut QymDoc, solids: *mut *mut QymShapeList, tree: *mut *mut QymTree) -> i32;
    fn qym_tree_count(t: *const QymTree) -> usize;
    fn qym_tree_parent(t: *const QymTree, i: usize) -> i64;
    fn qym_tree_name(t: *const QymTree, i: usize) -> *const c_char;
    fn qym_tree_place(t: *const QymTree, i: usize, out: *mut f64);
    fn qym_tree_solid(t: *const QymTree, i: usize) -> i64;
    fn qym_tree_repeat_of(t: *const QymTree, i: usize) -> i64;
    fn qym_tree_color(t: *const QymTree, i: usize, out: *mut f32) -> i32;
    fn qym_tree_face_colours(t: *const QymTree, i: usize, ids: *mut u32, rgb: *mut f32, cap: usize) -> usize;
    fn qym_tree_free(t: *mut QymTree);
    #[allow(clippy::too_many_arguments)] // the tree goes over as parallel arrays, one per field of a node
    fn qym_step_write_tree(
        n: usize,
        parents: *const i64,
        names: *const *const c_char,
        places: *const f64,
        shapes: *const *const QymShape,
        same_as: *const i64,
        rgb: *const f32,
        has_rgb: *const i32,
        face_starts: *const usize,
        face_ids: *const u32,
        face_rgb: *const f32,
        path: *const c_char,
    ) -> i32;
    fn qym_iges_solids(path: *const c_char) -> *mut QymShapeList;
    fn qym_iges_write(shapes: *const *const QymShape, mats: *const f64, n: usize, path: *const c_char, unit: *const c_char) -> i32;
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

/// THE TWO VERTICES OF AN EDGE: its id, and the places of its first and last vertex in the list of the vertices of the
/// shape, counted from 1 (0: none).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EdgeVertices {
    pub edge: u32,
    pub ends: [u32; 2],
}

/// THE NAMES A BLEND GIVES WHAT IT MAKES, as the C++ side reads them: its surface after each edge asked, the patch at
/// each corner, and every edge of the body after itself (pairs laid flat).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct BlendNames<'a> {
    pub surfaces: &'a [u32],
    pub corners: &'a [u32],
    pub all: &'a [u32],
}

/// AN ASYMMETRIC CHAMFER: mode 1 takes two distances, `a` on the reference face and `b` on the adjacent one; mode 2 the
/// leg `a` and the angle `b` in degrees from the reference face. `flip` chooses which of the two adjacent faces is the
/// reference, `ref_face` is one picked by hand (zero: it follows `flip`).
#[derive(Clone, Copy, Debug)]
pub(crate) struct AsymChamfer {
    pub a: f64,
    pub b: f64,
    pub mode: i32,
    pub flip: bool,
    pub ref_face: u32,
}

/// A BLEND BUILT AS THE PRISMS IT TAKES OFF, as the C++ side reads it (`qym_shape_blend_limit`): a fillet (`kind` 0) of
/// radius `a`, or a chamfer (`kind` 1) of legs `a` on the first face and `b` on the second, the first face chosen as
/// the asymmetric chamfer chooses it.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct BlendLimit {
    pub kind: i32,
    pub a: f64,
    pub b: f64,
    pub flip: i32,
    pub ref_face: u32,
}

/// THE EDGES A BLEND TAKES and the number each one's surface is handed under, in the same order.
#[derive(Clone, Copy)]
pub(crate) struct NamedEdges<'a> {
    pub edges: &'a [u32],
    pub surfaces: &'a [u32],
}

/// A VERTEX OF A SHAPE: where it stands and the edges meeting at it, the three smallest ids (zero where fewer meet).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VertexAt {
    pub at: [f64; 3],
    pub edges: [u32; 3],
}

/// A live B-rep shape. It owns the handle and frees it on drop, and it allows a general boolean over any
/// bodies, whatever built them — an extrusion, a revolve or a STEP import.
pub struct Shape {
    ptr: *mut QymShape,
}

/// A HELICAL RIB OR GROOVE, as the kernel needs it: where the helix runs, what is swept along it, and
/// how its ends are eased.
///
/// Named after `qymcad_core::feature::Helical`, which says the same thing on the model's side, minus the
/// two body numbers the kernel does not need here.
pub struct HelicalCut<'a> {
    /// A point on the axis and the direction along it.
    pub axis: qymcad_core::feature::AxisLine,
    /// The radius of the surface the profile is swept on.
    pub radius: f64,
    /// The axial section, encoded as an exact profile.
    pub profile: &'a [f64],
    /// How far the helix runs along the axis.
    pub length: f64,
    /// The rise of one turn.
    pub lead: f64,
    pub starts: u32,
    pub hand: Hand,
    /// Unite (a rib) or subtract (a groove).
    pub kind: Helix,
    /// The run-out at the start and at the end.
    pub lead_in: f64,
    pub lead_out: f64,
    /// The names for the faces the groove produces, and for those the run-out produces.
    pub gnames: &'a [u32],
    pub rnames: &'a [u32],
    /// How far the whole profile is moved in radially, to take up the clearance the groove could not.
    pub crest_relief: f64,
}

/// A THREAD BY STANDARD, as the kernel needs it: the helix, the shape of the groove and the clearances.
///
/// The profile is computed inside from `form` rather than handed in - that is what tells this apart from
/// `HelicalCut`, where the section is given outright.
pub struct ThreadCut {
    pub axis: qymcad_core::feature::AxisLine,
    pub radius: f64,
    pub length: f64,
    pub pitch: f64,
    /// The included angle of the profile, in degrees, and how deep it cuts.
    pub angle_deg: f64,
    pub depth: f64,
    pub starts: u32,
    pub hand: Hand,
    /// On a shaft or in a bore.
    pub site: Site,
    /// The standard's own code.
    pub form: u8,
    /// Clearance at the crest and at the root, so that a pair screws together.
    pub clearance_crest: f64,
    pub clearance_root: f64,
    pub lead_in: f64,
    pub lead_out: f64,
}

/// Why a boolean gave no body (see `Shape::boolean_checked`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoolVerdict {
    /// The kernel refused; its words are in the refusal channel.
    Failed,
    /// The tool does not reach the base.
    CutRemovedNothing,
    /// The bodies share no volume: they are apart or only touch.
    NothingInCommon,
}

/// A stitch that joined no edge: the sheets do not touch each other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NothingJoined;

/// THE IMAGE OF A FACE an operation makes from it - the inner wall a shell makes from an outer face - and the name it is
/// handed under.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageOf {
    pub source: u32,
    pub name: u32,
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
    pub(crate) fn from_brep_bytes(data: &[u8]) -> Option<Shape> {
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

/// How many numbers describe a surface to the bridge, the most any kind takes: a helix's seventeen.
const SURFACE_NUMBERS: usize = 20;

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
/// A surface a face recognised on a mesh lies on; see [`Shape::from_faces`].
#[derive(Clone, Debug, PartialEq)]
pub enum FaceSurface {
    Plane {
        point: [f64; 3],
        normal: [f64; 3],
    },
    Cylinder {
        point: [f64; 3],
        axis: [f64; 3],
        radius: f64,
    },
    /// `axis` runs from the tip into the cone.
    Cone {
        apex: [f64; 3],
        axis: [f64; 3],
        half_angle: f64,
    },
    Sphere {
        center: [f64; 3],
        radius: f64,
    },
    Torus {
        center: [f64; 3],
        axis: [f64; 3],
        major: f64,
        minor: f64,
    },
    /// A free form: the face fills its border and runs through its `points`; `normal` is where it faces out.
    Free {
        normal: [f64; 3],
    },
    /// A bicubic B-spline surface on clamped uniform knots, `nu` x `nv` poles given as the face's `points`, row by row
    /// (`nu` along u in a row); u x v points out of the body. `frame` lays a point of the face on its parameters: u =
    /// (q.frame[0..3] - frame[6]) / frame[7], v = (q.frame[3..6] - frame[8]) / frame[9] - its edges are laid so, not
    /// searched for.
    Spline {
        nu: usize,
        nv: usize,
        frame: [f64; 10],
    },
    /// A straight profile screwed about the axis through `point` along `axis`: a point at distance r, height z and angle
    /// t from `reference` lies on it where `radial * r + axial * (z - rise * t) = offset`, `axial` positive; its own
    /// normal points the way (`radial`, `axial`) does. The face lies within the angles `turn` (whole turns counted) and
    /// the distances `reach`.
    Helix {
        point: [f64; 3],
        axis: [f64; 3],
        reference: [f64; 3],
        rise: f64,
        radial: f64,
        axial: f64,
        offset: f64,
        turn: [f64; 2],
        reach: [f64; 2],
    },
    /// A round wire wound about the axis through `point` along `axis`: its middle runs `radius` from the axis at angle t
    /// from `reference`, `lift + rise * t` along it, and the surface is every point `wire` from that line; its own normal
    /// points away from the line. The face lies within the angles `turn` (whole turns counted).
    Coil {
        point: [f64; 3],
        axis: [f64; 3],
        reference: [f64; 3],
        rise: f64,
        radius: f64,
        lift: f64,
        wire: f64,
        turn: [f64; 2],
    },
    /// A circle screwed about the axis through `point` along `axis`: a point at distance r, height z and angle t from
    /// `reference` lies on it where (r, z - rise * t) lies `round` from (`middle`, `height`), whole turns counted; its own
    /// normal points away from the circle's middle. The face lies within the angles `turn` and the angles round the
    /// circle `reach`, from the way out from the axis towards the axis's own way.
    RoundHelix {
        point: [f64; 3],
        axis: [f64; 3],
        reference: [f64; 3],
        rise: f64,
        middle: f64,
        height: f64,
        round: f64,
        turn: [f64; 2],
        reach: [f64; 2],
    },
}

/// The curve an edge of such a face runs along.
#[derive(Clone, Debug, PartialEq)]
pub enum EdgeCurve {
    /// `dir` is a unit vector the way the edge's points run.
    Line {
        point: [f64; 3],
        dir: [f64; 3],
    },
    Circle {
        center: [f64; 3],
        axis: [f64; 3],
        radius: f64,
    },
    /// In the plane through `center` square to `normal`, half-axis `major` along `major_dir` and `minor` across it.
    Ellipse {
        center: [f64; 3],
        normal: [f64; 3],
        major_dir: [f64; 3],
        major: f64,
        minor: f64,
    },
    /// A curve through the edge's points: within one tolerance of them where `close`, and within four otherwise.
    Points {
        close: bool,
    },
    /// A polyline through the edge's points as they are: an edge beside a region left as mesh, meeting the sides of its
    /// triangles.
    Polyline,
}

/// An edge of faces recognised on a mesh.
#[derive(Clone, Debug, PartialEq)]
pub struct FaceEdge {
    pub curve: EdgeCurve,
    /// The corners it runs between, as indices into the corners; `None` for a closed edge.
    pub ends: Option<[usize; 2]>,
    /// Its points in order; where it has corners, the first and the last are theirs.
    pub points: Vec<[f64; 3]>,
}

/// What came of building a body from faces recognised on a mesh; see [`Shape::from_faces`].
pub struct BuiltBody {
    pub shape: Shape,
    /// For every face, the area it took of its surface; `None` where it could not be built.
    pub areas: Vec<Option<f64>>,
    /// How many sides no second face met in the sewing: none means the shell closed.
    pub free_edges: usize,
    /// Where those sides are, up to 256 of them: the middle of each.
    pub free_at: Vec<[f64; 3]>,
    /// How many triangles of the regions left as mesh made no face: each is a hole of its own size.
    pub loose_dropped: usize,
}

/// A face recognised on a mesh, bounded by loops of edges.
#[derive(Clone, Debug, PartialEq)]
pub struct BoundedFace {
    pub surface: FaceSurface,
    /// Whether the surface's own normal - a plane's `normal`, away from the axis or the centre on the rest - points out
    /// of the body.
    pub outward: bool,
    /// A point amid the face: the seam of a round surface is turned away from it, and a sphere's poles aside.
    pub inside: [f64; 3],
    /// Its loops: edges in order, each with whether the loop walks it the way its points run. The face lies on the
    /// left of every loop seen from outside the body. A face with no loop is its whole surface.
    pub loops: Vec<Vec<(usize, bool)>>,
    /// Points a free form runs through, amid its border; empty for the other surfaces.
    pub points: Vec<[f64; 3]>,
}

pub(crate) fn refuse(where_: &str, what: &str) -> Option<Shape> {
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

// A shape owns its kernel handle as a sole raw pointer, so moving that ownership to another thread moves the
// pointer. It is sound only while no shape left behind, or sent to another thread, shares a face or an edge with
// it: two shapes that share a sub-shape (the pieces of a split share the cut face) would otherwise be meshed or
// healed on two threads at once, which rewrites that sub-shape and crashes. The batch rebuild holds to that
// condition by keeping bodies that share geometry on one thread (`Kernel::shares`, model/regen.rs). `Sync` is not
// claimed: there is never concurrent access to one shape.
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

impl Shape {}

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
/// a minimum number of segments in any case. The ceiling of 1 mm holds up to a diagonal of 10 m and rises past it
/// as 1e-4 of the diagonal: a ball's triangles go as the square of the segments around it, and under a fixed 1 mm
/// a ball of r = 100 m came out 1006106 triangles against 1982 of r = 10 mm - a window past 2 GB and over a
/// minute to tessellate - while a cylinder of the same radius, its triangles going as the segments alone, did not.
/// A `diag` of zero or less, the box not having been computed, falls back to the former 0.5 mm.
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
    (diag * k).clamp(MIN, MAX.max(diag * 1e-4))
}

/// WHETHER THE KERNEL'S BOOLEANS RUN ON SEVERAL CORES, and on how many.
///
/// Measured on the reference documents: with the flag the scenario document rebuilds in 17.2 s against 20.3 s,
/// and its threads in 8.5 s against 12.3 s. `threads` of zero or less means "all the cores but one", which
/// leaves the machine usable while a heavy rebuild runs. The pool is shared with every other parallel pass of
/// the kernel, so this is the ceiling for all of them together.
pub fn set_parallel(on: bool, threads: i32) {
    let want = if !on {
        1
    } else if threads > 0 {
        threads as usize
    } else {
        // all the cores but one, so the machine stays usable while a heavy rebuild runs
        std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).saturating_sub(1).max(1)
    };
    WORKERS.store(want, std::sync::atomic::Ordering::Relaxed);
    qymcad_core::solver::set_threads(want); // the solve of a sketch takes the same number
    unsafe {
        qym_set_threads(threads);
        qym_set_parallel(if on { 1 } else { 0 });
    }
}

/// HOW MANY THREADS A REBUILD MAY BE DIVIDED INTO. The same number the booleans inside the kernel are given,
/// because the pool is one: divided twice over, the threads multiply and the machine stops.
///
/// Zero means nobody has said: then it is all the cores but one, which is what the application asks for by
/// default and what a check gets without saying anything. A setting of one is the single-threaded answer.
static WORKERS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

pub fn workers() -> usize {
    match WORKERS.load(std::sync::atomic::Ordering::Relaxed) {
        0 => std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).saturating_sub(1).max(1),
        n => n,
    }
}

/// THE FACES SEWN TOGETHER with the given tolerance, and closed into a solid where they close.
#[cfg(test)]
pub(crate) fn sew(shape: &Shape, tolerance: f64) -> Option<Shape> {
    Shape::wrap(unsafe { qym_shape_sew(shape.ptr, tolerance) })
}

/// THE SHELL PUT RIGHT: the orientation of the faces and of the shell. `None` means the kernel refused.
#[cfg(test)]
pub(crate) fn fix_shell(shape: &Shape) -> Option<Shape> {
    Shape::wrap(unsafe { qym_shape_fix_shell(shape.ptr) })
}

/// HOW MANY FACES OF THIS SHAPE DO NOT TESSELLATE, out of how many.
///
/// A face without a triangulation is skipped when the mesh is assembled and leaves a hole in the shell - the
/// part is see-through there. The number is meant for a measurement and for a report to the person: an import
/// that lost faces must say so rather than hand over a body with gaps.
pub fn unmeshed_faces(shape: &Shape, deflection: f64) -> (usize, usize) {
    unsafe {
        let d = qym_shape_tessellate(shape.ptr, deflection);
        if d.is_null() {
            return (0, 0);
        }
        let out = (qym_doc_unmeshed_faces(d), qym_doc_face_count(d));
        qym_doc_free(d);
        out
    }
}

/// HOW MANY FACES A TESSELLATION OF `shape` HAS TO MESH - the ones that come to it with no triangulation of their own.
/// A flat face bounded by straight edges meshes the same at any deflection, so once it has its triangles it keeps them.
pub fn remeshed_faces(shape: &Shape, deflection: f64) -> usize {
    unsafe {
        let d = qym_shape_tessellate(shape.ptr, deflection);
        if d.is_null() {
            return 0;
        }
        let out = qym_doc_meshed_faces(d);
        qym_doc_free(d);
        out
    }
}

/// Extrude a closed profile — XY points at z = 0 — to a height of `height`, giving bodies. `xy` holds the
/// coordinates in pairs, as `[x0, y0, x1, y1, ...]`.
#[cfg(test)]
pub(crate) fn extrude(xy: &[f64], height: f64, deflection: f64) -> Result<Vec<Built>, String> {
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
#[cfg(test)]
pub(crate) fn revolve(xy: &[f64], axis: u8, angle_deg: f64, deflection: f64) -> Result<Vec<Built>, String> {
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
#[cfg(test)]
pub(crate) fn extrude_bool(base: &[f64], base_h: f64, tool: &[f64], tool_h: f64, op: u8, deflection: f64) -> Result<Vec<Built>, String> {
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
#[cfg(test)]
pub(crate) fn box_mesh(dx: f64, dy: f64, dz: f64, deflection: f64) -> Mesh {
    let defl = if deflection > 0.0 { deflection } else { 0.5 };
    unsafe {
        let d = qym_occt_box_doc(dx, dy, dz, defl);
        let mut bodies = doc_to_bodies(d);
        if bodies.is_empty() {
            return Mesh { verts: Vec::new(), tris: Vec::new(), ..Default::default() };
        }
        bodies.remove(0).mesh
    }
}

mod build;
mod asks;
mod edges;
mod ops;
mod faces;
mod exchange;
pub use exchange::*;
