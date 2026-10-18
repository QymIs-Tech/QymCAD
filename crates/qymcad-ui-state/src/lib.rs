//! THE STATE OF THE INTERFACE, in a crate of its own.
//!
//! Fifty-five records: the tools with parameters of their own, the selection, the view, the caches, the
//! settings. Every context of a workbench (`PartCtx`, `SketchCtx`, `JointCtx` and the rest) is built out
//! of these and out of nothing else.
//!
//! WHY A CRATE. A workbench cannot become a crate of its own while the records it edits are declared
//! beside the application: it would have to depend on the crate holding the god object, and that is the
//! very dependency the rebuild is undoing. Here they depend on the document, the kernel, the shell and
//! `egui` - which is what the manifest says and a guard below checks.
//!
//! THIS FILE NAMES `App` NOWHERE. That is the whole point, and it is guarded rather than promised.

pub mod icons;
pub use icons::*;
pub mod grab;
pub mod platform_keys;
/// Clipping a triangle by a plane for the SECTION view: a pure module with no `self`, so it can be unit-tested.
pub mod smallvec_tris {
    /// A clipped vertex: its world position plus the barycentric weights of the original vertices (for interpolating colour).
    #[derive(Clone, Copy)]
    pub struct ClipV {
        pub pos: [f64; 3],
        pub w: [f64; 3],
    }
    /// 0 to 2 triangles (indices into `verts`, as a fan); `whole = true` means take the original triangle as it is.
    pub struct ClipTris {
        pub whole: bool,
        pub verts: Vec<ClipV>,
    }
    impl ClipTris {
        pub fn whole() -> Self {
            ClipTris { whole: true, verts: Vec::new() }
        }
        pub fn empty() -> Self {
            ClipTris { whole: false, verts: Vec::new() }
        }
    }
    /// Sutherland-Hodgman: keep the side where dist <= 0. Three inputs give a fan of 0, 3 or 4 vertices.
    pub fn clip_by_dists(v: [[f64; 3]; 3], d: [f64; 3]) -> ClipTris {
        let vis = [d[0] <= 0.0, d[1] <= 0.0, d[2] <= 0.0];
        if vis[0] && vis[1] && vis[2] {
            return ClipTris::whole();
        }
        if !vis[0] && !vis[1] && !vis[2] {
            return ClipTris::empty();
        }
        let mut out: Vec<ClipV> = Vec::with_capacity(4);
        for i in 0..3 {
            let j = (i + 1) % 3;
            if vis[i] {
                let mut w = [0.0; 3];
                w[i] = 1.0;
                out.push(ClipV { pos: v[i], w });
            }
            if vis[i] != vis[j] {
                let t = d[i] / (d[i] - d[j]); // the point EXACTLY on the plane
                let lerp = |a: f64, b: f64| a + (b - a) * t;
                let mut w = [0.0; 3];
                w[i] = 1.0 - t;
                w[j] = t;
                out.push(ClipV { pos: [lerp(v[i][0], v[j][0]), lerp(v[i][1], v[j][1]), lerp(v[i][2], v[j][2])], w });
            }
        }
        ClipTris { whole: false, verts: out }
    }
}

pub mod settings_sections;

use egui::{Pos2, Rect, Vec2};
use qymcad_core::measure::MeasureItem;
use qymcad_core::part::PartManifest;
use qymcad_core::refs::Query;
use qymcad_core::geom::{Contour, MeshFace, Point2};
use qymcad_core::model::{Id, Project};

/// The four, taken from the application, where they live in two records.
#[macro_export]
macro_rules! scene_drag_of {
    ($x:expr) => {
        $crate::SceneDrag { body_giz: &$x.dragged.body_giz, comp_giz: $x.dragged.comp_giz, joint: &$x.side.joint, part_pull: &$x.dragged.part_pull }
    };
}

/// The current value of a command parameter, by key.
pub fn cmd_val(cmd: &FeatCommand, key: &str) -> f64 {
    cmd.params.iter().find(|p| p.key == key).map(|p| p.val).unwrap_or(0.0)
}

/// The command's fields for the current mode. A thread is specified the professional way — by standard
/// and size; the depth, the diameters and the profile are computed by the model's core from the
/// standard's formulas (the angle and the thread depth used to be typed by hand, and the result was
/// whatever came out). An auger has a flight of its own: the outer diameter, the pitch, the thickness.
pub fn set_thread_params(cmd: &mut FeatCommand, thread: ThreadParams) {
    let d = if thread.radius > 1e-6 { thread.radius * 2.0 } else { 10.0 };
    cmd.params = if thread.auger {
        vec![
            CmdParam::new("th-outer-d", "outer", (d * 3.0).max(d + 10.0), 0.5, 2000.0),
            CmdParam::new("th-pitch", "pitch", (d * 2.0).max(10.0), 0.1, 1000.0),
            CmdParam::new("th-length", "length", (d * 5.0).max(30.0), 0.1, 10000.0),
            CmdParam::new("th-flight-thickness", "thickness", (d * 0.25).max(1.0), 0.1, 100.0),
            CmdParam::new("th-edge-fillet", "edge_r", 0.0, 0.0, 50.0),
            // THE FLIGHT'S RUN-OUT: without it the turn breaks off square, like a stump. Half a pitch by default.
            CmdParam::new("th-taper-in", "lead_in", (d * 1.0).max(5.0), 0.0, 10000.0),
            CmdParam::new("th-taper-out", "lead_out", (d * 1.0).max(5.0), 0.0, 10000.0),
        ]
    } else {
        vec![
            CmdParam::new("th-nominal-d", "nominal", d, 0.5, 1000.0),
            // 0 means the standard coarse pitch for this diameter (ISO 261 / ISO 2901), so choosing the
            // size is enough.
            CmdParam::new("th-pitch-std", "pitch", 0.0, 0.0, 100.0),
            CmdParam::new("th-length", "length", (d * 1.5).max(10.0), 0.1, 10000.0),
            // THE FIT CLEARANCE is what printed threads are made for: a bolt and a nut screw together.
            CmdParam::new("th-fit-clearance", "fit", 0.2, 0.0, 5.0),
            CmdParam::new("th-lead-in", "lead_in", 1.5, 0.0, 10000.0),
            CmdParam::new("th-lead-out", "lead_out", 1.5, 0.0, 10000.0),
            // FILLETS on both the crest and the root. 0 means follow the standard; a value of one's own
            // overrides the table — for printing the crest is rounded more, so the layers do not tear.
            CmdParam::new("th-crest-fillet", "crest_r", 0.0, 0.0, 100.0),
            CmdParam::new("th-root-fillet", "root_r", 0.0, 0.0, 100.0),
        ]
    };
    // A "custom" form is a profile NOT described by a standard, so the angle and the depth are given by
    // hand. Without these fields choosing "custom" changed nothing: the angle silently stayed at 60 deg
    // and the depth at 0.6 of the pitch.
    if !thread.auger && thread_standard(thread.form) == qymcad_core::thread::ThreadStandard::Custom {
        cmd.params.push(CmdParam::new("th-profile-angle", "angle", 60.0, 5.0, 170.0));
        // THE DEPTH OPENS ON THE ONE THAT WILL BE CUT. Zero in this field does not mean a groove of no
        // depth: the core reads it as "nothing was typed" and cuts 0.6 of the pitch. And the pitch itself
        // is zero by default, meaning "the standard coarse pitch for this size" - so seeding the field from
        // the pitch typed in gave 0.00 over a groove 1.50 deep on a Ø20, a number describing nothing.
        cmd.params.push(CmdParam::new("th-depth", "depth", custom_depth_to_start_from(cmd, thread), 0.0, 1000.0));
    }
}

pub fn seg_axis_intersect(a: Point2, b: Point2, axis: u8) -> Option<Point2> {
    let (pa, pb) = if axis == 0 { (a.x, b.x) } else { (a.y, b.y) };
    let d = pb - pa;
    if d.abs() < 1e-12 {
        return None; // parallel to the axis: either no crossing or the whole segment lies on it
    }
    let t = -pa / d;
    (0.0..=1.0).contains(&t).then(|| Point2::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t))
}

/// Where a circle crosses one of the sketch axes - none, or both points.
pub fn circle_axis_intersect(c: Point2, r: f64, axis: u8) -> Vec<Point2> {
    let off = if axis == 0 { c.x } else { c.y };
    if off.abs() > r {
        return Vec::new();
    }
    let h = (r * r - off * off).max(0.0).sqrt();
    if axis == 0 {
        vec![Point2::new(0.0, c.y - h), Point2::new(0.0, c.y + h)]
    } else {
        vec![Point2::new(c.x - h, 0.0), Point2::new(c.x + h, 0.0)]
    }
}

/// The five, taken from the application, where they live in four different records.
/// THE PIECES THE POPUPS OF A TEXT WORK ON, gathered from the application - see `TextCtx`.
#[macro_export]
macro_rules! text_ctx_of {
    ($x:expr) => {
        $crate::TextCtx {
            annot: &mut $x.tools.annot,
            inline: &mut $x.tools.inline,
            armed: &mut $x.tools.armed,
            tool: &mut $x.tools.tool,
            font: &mut $x.tool_prefs.font,
            text: &mut $x.tool_prefs.text,
            tool_text_height: $x.tool_prefs.text_h,
            writes_note: $x.tool_prefs.text_note,
        }
    };
}

/// THE FIVE PIECES OF AN EDIT, gathered from the application.
#[macro_export]
macro_rules! editing_of {
    ($x:expr) => {
        $crate::Editing { project: &mut $x.project, edits: &mut $x.disk.edits, regen: &mut $x.regen, sel: &mut $x.chosen.sel, status: &mut $x.status, view: &mut $x.viewing.view }
    };
}

/// The same, from a CONTEXT, where the fields are already borrows and are reborrowed.
#[macro_export]
macro_rules! editing_in {
    ($x:expr) => {
        $crate::Editing { project: &mut *$x.project, edits: &mut *$x.edits, regen: &mut *$x.regen, sel: &mut *$x.sel, status: &mut *$x.status, view: &mut *$x.view }
    };
}

/// TAKE THE MEASURING TOOL, or put it down.
///
/// LIKE EVERY OTHER TOOL: the others are released FIRST, and only then is this one taken. The reverse
/// order is not a style question - it is what the ruler button did, and it meant the button did nothing at
/// all. It set `measure.on` and pushed `BarAsk::SketchSelectMode`; the frame performs a request AFTER the
/// panel has drawn, that request calls `exit_draw_tools`, and `exit_draw_tools` clears the measuring tool
/// along with the rest. On, then off, in the same frame.
///
/// Reported behaviour: none - nobody had reported it, because a button that does nothing looks like a
/// button one has misunderstood. It was found by sweeping every pair of drawing tools.
pub fn set_measure(t: &mut Tools, on: bool) {
    exit_draw_tools(&mut t.reborrow());
    *t.armed = if on { Armed::Measure } else { Armed::None };
    t.measure.pts.clear();
}

/// THE TOOLS IN HAND, from an OWNER - the application, where these fields are held by value.
///
/// A MACRO AND NOT A METHOD, and the reason is the borrow checker rather than taste. A method takes the
/// WHOLE receiver, so `set_dim_tool(&mut self.tools(), &mut self.mode_3d, ...)` borrows the application
/// twice and does not compile. The macro expands to a struct literal of FOURTEEN SEPARATE FIELD BORROWS,
/// which are disjoint, so the neighbouring arguments can still be taken from the same owner.
#[macro_export]
macro_rules! tools_of {
    ($x:expr) => {
        $crate::Tools {
            armed: &mut $x.tools.armed,
            annot: &mut $x.tools.annot,
            cmd: &mut $x.tools.cmd,
            corner: &mut $x.tools.corner,
            dim: &mut $x.tools.dim,
            drag: &mut $x.tools.drag,
            gsel: &mut $x.tools.gsel,
            inline: &mut $x.tools.inline,
            measure: &mut $x.tools.measure,
            pat: &mut $x.tools.pat,
            pending_import: &mut $x.tools.pending_import,
            picking: &mut $x.tools.picking,
            place: &mut $x.tools.place,
            sel_sk: &mut $x.tools.sel_sk,
            tool: &mut $x.tools.tool,
        }
    };
}

/// THE TOOLS THE SIZE POPUPS TAKE, out of the application that holds them - one spelling, so that every caller
/// gathers the same four and the list is not written out again where a panel is.
#[macro_export]
macro_rules! popup_tools {
    ($x:expr) => {
        $crate::PopupTools { corner: &mut $x.tools.corner, place: &mut $x.tools.place, sel_sk: &mut $x.tools.sel_sk, armed: &$x.tools.armed }
    };
}

/// THE VALUES AND THE SCHEME THE SIZE POPUPS TAKE, out of the application that holds them: what the tools
/// remember between two corners, and the colours everything over the sheet is read from.
#[macro_export]
macro_rules! popup_looks {
    ($x:expr) => {
        $crate::PopupLooks { tool_prefs: &mut $x.tool_prefs, scheme: &$x.scheme, set: &$x.set }
    };
}

/// The same, from a CONTEXT, where the fields are already borrows and are reborrowed.
#[macro_export]
macro_rules! tools_in {
    ($x:expr) => {
        $crate::Tools {
            armed: $x.armed,
            annot: $x.annot,
            cmd: $x.cmd,
            corner: $x.corner,
            dim: $x.dim,
            drag: $x.drag,
            gsel: $x.gsel,
            inline: $x.inline,
            measure: $x.measure,
            pat: $x.pat,
            pending_import: $x.pending_import,
            picking: $x.picking,
            place: $x.place,
            sel_sk: $x.sel_sk,
            tool: $x.tool,
        }
    };
}

/// The visibility seven, taken from the application, where they live in four different records.
#[macro_export]
macro_rules! body_view_of {
    ($x:expr) => {
        $crate::BodyView { armed: &$x.tools.armed, active_path: &$x.active_path, cache: &$x.cache, cmd: &$x.tools.cmd, project: &$x.project, regen: &$x.regen, sketch_ses: $x.sketch_ses, win: &$x.win }
    };
}

/// The same, from a CONTEXT, where the fields are already borrows.
#[macro_export]
macro_rules! body_view_in {
    ($x:expr) => {
        $crate::BodyView { armed: $x.armed, active_path: $x.active_path, cache: &*$x.cache, cmd: &*$x.cmd, project: &*$x.project, regen: &*$x.regen, sketch_ses: *$x.sketch_ses, win: &*$x.win }
    };
}

impl<'a> Painting<'a> {
    /// What the interface is in the middle of, out of a whole frame's drawing state.
    pub fn doing(&self) -> Doing<'a> {
        Doing { armed: self.armed, cmd: self.cmd, gsel: self.gsel, joint: self.joint, picking: self.picking, sketch_ses: self.sketch_ses, workbench: self.workbench }
    }
}

impl<'a> Painting<'a> {
    /// The visibility seven out of a whole frame's drawing state.
    pub fn body_view(&self) -> BodyView<'a> {
        BodyView { armed: self.armed, active_path: self.active_path, cache: self.cache, cmd: self.cmd, project: self.project, regen: self.regen, sketch_ses: self.sketch_ses, win: self.win }
    }
}

/// The aiming, taken from the application, where the ten live in three different records.
#[macro_export]
macro_rules! feat_picks_of {
    ($x:expr) => {
        $crate::FeatPicks {
            arr: &mut $x.params.arr,
            datum: &mut $x.side.datum,
            draft: &mut $x.params.draft,
            loft: &mut $x.params.loft,
            mirror: &mut $x.params.mirror,
            picking: &mut $x.tools.picking,
            recognise: &mut $x.params.recognise,
            stitch_parts: &mut $x.params.stitch_parts,
            sweep: &mut $x.params.sweep,
            thread: &mut $x.params.thread,
            trim: &mut $x.side.trim,
        }
    };
}

/// The same, from a CONTEXT, where the fields are already borrows and are reborrowed.
#[macro_export]
macro_rules! feat_picks_in {
    ($x:expr) => {
        $crate::FeatPicks {
            arr: $x.arr,
            datum: $x.datum,
            draft: $x.draft,
            loft: $x.loft,
            mirror: $x.mirror,
            picking: $x.picking,
            recognise: $x.recognise,
            stitch_parts: $x.stitch_parts,
            sweep: $x.sweep,
            thread: $x.thread,
            trim: $x.trim,
        }
    };
}

impl Tools<'_> {
    /// LEND THE TOOLS ONWARD. A record of borrows cannot be copied, and a caller that has already unpacked
    /// it still needs to hand the whole set to a neighbour; this borrows every field afresh.
    pub fn reborrow(&mut self) -> Tools<'_> {
        Tools {
            armed: self.armed,
            annot: self.annot,
            cmd: self.cmd,
            corner: self.corner,
            dim: self.dim,
            drag: self.drag,
            gsel: self.gsel,
            inline: self.inline,
            measure: self.measure,
            pat: self.pat,
            pending_import: self.pending_import,
            picking: self.picking,
            place: self.place,
            sel_sk: self.sel_sk,
            tool: self.tool,
        }
    }
}

// HOW A KEY IS WRITTEN ON SCREEN lives in the dictionary crate, which writes every caption: a key typed into a
// sentence ("Copy (Ctrl+C)") has to follow the system just like a key in the table.
pub use qymcad_i18n::keys::{key_label, key_label_in, key_style, set_key_style, KeyStyle};

/// WHERE THE BREADCRUMB GOES. Opening an edit records what a person just did, so that a report about a
/// crash carries the last few steps. The record crate must not know what a crash report is, nor where it
/// is written - the application plugs a sink in at startup, and until it does, the breadcrumb goes
/// nowhere.
static STEP_SINK: std::sync::Mutex<Option<fn(&str)>> = std::sync::Mutex::new(None);

/// Install the sink for the breadcrumbs. Called once, at startup.
pub fn set_step_recorder(f: fn(&str)) {
    if let Ok(mut s) = STEP_SINK.lock() {
        *s = Some(f);
    }
}

/// Record what is being done, if anyone is listening.
pub fn note_step(name: &str) {
    let f = STEP_SINK.lock().ok().and_then(|s| *s);
    if let Some(f) = f {
        f(name);
    }
}

/// "IS THERE ANYTHING UNSAVED" — the same document fingerprint the operation-boundary guard uses.
///
/// The name is kept because the question is a different one (it is compared against `saved_key`, not
/// `committed_key`), but the COMPUTATION must be single: two identical keys would drift apart on an
/// edit to one of them, and would drift silently. Drifting here is expensive both ways — a needless
/// question on closing, or lost work.
pub fn edit_key(dc: &DrawCtx) -> u64 {
    doc_key(dc.project)
}

/// MARK THE CONSUMERS OF PARAMETERS WHOSE VALUES HAVE CHANGED SINCE THE LAST COMPLETED REBUILD.
///
/// Computed against the `params_seen` snapshot. The snapshot is advanced by [`settle_params_seen`] —
/// and by that alone, on the fact of a finished rebuild, whether synchronous or arriving from a thread.
///
/// This loop used to stand as THREE copies, and only the synchronous branch updated the snapshot. In
/// a live window the rebuild is asynchronous, which means the snapshot was NEVER updated: every
/// parameter counted as changed forever, the scheduler marked the whole parametric model dirty, the
/// frame asked for a rebuild — and round it went again. An open parametric document rebuilt WITHOUT
/// END, and to no purpose: it has no live B-rep yet, so every such rebuild failed with "the source
/// body has not been built" and left that error in the status line.
///
/// Answers whether the placement counts from a changed value (`Project::placement_counts_from`): a mate has no node to
/// mark, and its angle follows the parameter only through a rebuild asked for.
pub fn mark_changed_params_dirty(params_seen: &std::collections::HashMap<String, f64>, project: &mut Project) -> bool {
    let vars = project.param_map();
    let mut changed: Vec<String> = vars.iter().filter(|(k, v)| params_seen.get(*k).is_none_or(|old| (*old - **v).abs() > 1e-12)).map(|(k, _)| k.clone()).collect();
    // a name that was seen and is gone - a parameter deleted - changes whatever was counted from it
    changed.extend(params_seen.keys().filter(|k| !vars.contains_key(*k)).cloned());
    for name in &changed {
        project.mark_param_dependents_dirty_for(name);
    }
    project.placement_counts_from(&changed)
}

/// The live wireframe preview of the active command (the prism of the profile along its extent) plus the
/// arrow gizmo of the length.
/// The geometry of the LIVE SWEEP PREVIEW: the contour of the profile carried along the path with a
/// parallel-transported frame (the same idea as the corrected Frenet frame in the core: the minimal
/// rotation between tangents, with no false twist). Returns the polyline of the path in world
/// coordinates and the section loops at the stations along it. It agrees with the core at the start and
/// the finish (the same frame perpendicular to the tangent); the middle is a stable approximation.
pub fn sweep_preview(project: &Project, sweep: SweepParams) -> Option<SweepPreview> {
    let (prof_sid, path_sid) = (sweep.prof_sid, sweep.path_sid);
    if prof_sid == 0 || path_sid == 0 {
        return None;
    }
    let prof_cid = if sweep.prof_cid != 0 { sweep.prof_cid } else { *project.sweep_profile_contours(prof_sid).first()? };
    let path_cid = if sweep.path_cid != 0 { sweep.path_cid } else { *project.sweep_path_contours(path_sid).first()? };
    let pxy = project.contour_profile_xy(prof_cid)?; // the flat contour of the profile, closed
    let np = pxy.len() / 2;
    if np < 3 {
        return None;
    }
    let pf = project.sketch_frame_by_id(path_sid)?;
    let pidx = project.contour_index(path_cid)?;
    let pts2 = &project.contours[pidx].points;
    if pts2.len() < 2 {
        return None;
    }
    let path: Vec<[f64; 3]> = pts2
        .iter()
        .map(|p| {
            let w = pf.lift(*p);
            [w.x, w.y, w.z]
        })
        .collect();
    let n = path.len();
    let sub = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let add = |a: [f64; 3], b: [f64; 3]| [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let cross = |a: [f64; 3], b: [f64; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let norm = |v: [f64; 3]| {
        let l = dot(v, v).sqrt();
        if l < 1e-9 {
            [0.0, 0.0, 1.0]
        } else {
            [v[0] / l, v[1] / l, v[2] / l]
        }
    };
    let tangent = |i: usize| -> [f64; 3] {
        if i == 0 {
            norm(sub(path[1], path[0]))
        } else if i == n - 1 {
            norm(sub(path[n - 1], path[n - 2]))
        } else {
            norm(add(norm(sub(path[i], path[i - 1])), norm(sub(path[i + 1], path[i]))))
        }
    };
    // the starting frame (as in the C++ core): refX perpendicular to Z, Y = Z x refX, X = Y x Z
    let z0 = tangent(0);
    let refx = if dot(z0, [1.0, 0.0, 0.0]).abs() > 0.9 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
    let mut y = norm(cross(z0, refx));
    let mut x = norm(cross(y, z0));
    let mut zprev = z0;
    // rotating vector v about the unit axis a by an angle given as (sin, cos) - Rodrigues
    let rot = |v: [f64; 3], a: [f64; 3], s: f64, c: f64| {
        let av = cross(a, v);
        let ad = dot(a, v) * (1.0 - c);
        [v[0] * c + av[0] * s + a[0] * ad, v[1] * c + av[1] * s + a[1] * ad, v[2] * c + av[2] * s + a[2] * ad]
    };
    let mut sections = Vec::with_capacity(n);
    for (i, org) in path.iter().enumerate() {
        let zi = tangent(i);
        // parallel transport of the frame from zprev to zi (the minimal rotation)
        let axc = cross(zprev, zi);
        let sn = dot(axc, axc).sqrt();
        if sn > 1e-9 {
            let a = [axc[0] / sn, axc[1] / sn, axc[2] / sn];
            let cs = dot(zprev, zi).clamp(-1.0, 1.0);
            x = norm(rot(x, a, sn, cs));
            y = norm(rot(y, a, sn, cs));
        }
        let sec: Vec<[f64; 3]> = (0..np)
            .map(|k| {
                let (px, py) = (pxy[2 * k], pxy[2 * k + 1]);
                [org[0] + px * x[0] + py * y[0], org[1] + px * x[1] + py * y[1], org[2] + px * x[2] + py * y[2]]
            })
            .collect();
        sections.push(sec);
        zprev = zi;
    }
    Some(SweepPreview { path, sections })
}

/// WHICH WAY THE TOOL GROWS, in the word the model uses.
///
/// The window holds this as two separate switches - the extent mode of the top bar and the flip of the
/// gizmo - while the model holds one word of three. The translation lives here alone, so the preview and
/// the rebuild cannot come to different answers about the same two switches.
pub fn cmd_reach(cmd: &FeatCommand, feat: FeatTarget) -> qymcad_core::feature::Reach {
    use qymcad_core::feature::Reach;
    if cmd.extent.symmetric() {
        Reach::BothWays
    } else if feat.flip {
        Reach::Backward
    } else {
        Reach::Forward
    }
}

/// THE RESULT TEXT — the only place where the numbers turn into a string (the status line and the
/// plate at the geometry say the same thing: two wordings of one measurement drift apart
/// silently).
pub fn measure_text(pn: &Painting) -> String {
    let Some(r) = measure_result(pn.m3) else { return qymcad_i18n::tr("m3-hint-short") };
    let names: Vec<&str> = pn.m3.picks.iter().map(|p| p.what.as_str()).collect();
    let mut parts: Vec<String> = Vec::new();
    if let Some((label, v)) = r.value {
        // the label is a code of the catalogue (`m3-length`) or a sign (`Ø`): translated where it is a code
        parts.push(qymcad_i18n::tr2("m3-value-mm", "label", &qymcad_i18n::name(label), "v", &qymcad_i18n::num(v, 3)));
    }
    if let Some(d) = r.distance {
        parts.push(qymcad_i18n::tr1("m3-distance", "v", &qymcad_i18n::num(d, 3)));
    }
    if let Some(a) = r.angle_deg {
        parts.push(qymcad_i18n::tr1("m3-angle", "v", &qymcad_i18n::num(a, 3)));
    }
    if let Some(d) = r.delta {
        parts.push(format!("Δ {:.3} / {:.3} / {:.3}", d[0], d[1], d[2]));
    }
    if parts.is_empty() {
        // HONESTLY: there is no meaningful number for this pair (converging faces, for one — the
        // distance between them depends on where it is measured). Silence is worse: a person will
        // decide the tool is broken.
        return qymcad_i18n::tr1("m3-not-parallel", "what", &names.join(" - "));
    }
    format!("{}: {}", names.join(" - "), parts.join(" · "))
}

/// The sign of a gizmo ring's rotation. `axis_depth` is the component of the rotation axis along the
/// INTO-THE-SCREEN direction (`basis.2`, where depth grows AWAY from the camera). With the axis pointing
/// TOWARDS the viewer (depth < 0), a visually counter-clockwise drag is a POSITIVE right-handed angle. The
/// sign used to be inverted — dragging to the right gave negative degrees and turned the part the other way.
pub fn ring_drag_sign(axis_depth: f64) -> f64 {
    if axis_depth >= 0.0 {
        -1.0
    } else {
        1.0
    }
}

/// THE CHORD PRESSED THIS FRAME that a tool key may answer to, or `None`.
///
/// FOCUS IN A FIELD MUST NOT KILL EVERY KEY - and must not let a letter through either. A bare letter in a
/// field types itself (expressions hold both `w` and `len`); Alt plus a letter types nothing, so it goes to the
/// command. The rule is one: with no focus, the bare key; with focus, Alt.
///
/// CHORDS: a Ctrl or Control chord that can be bound types nothing in a field - the letters a field edits with
/// under either are refused at binding (`platform_keys`) - so it is heard as it is, focus or not. Without them the
/// rule above holds, Shift included: Shift+W in a field is a capital letter.
///
/// "CTRL" IS THE COMMAND KEY: Cmd on a Mac. The Mac's own Control key is a modifier of its own (`Chord::of_press`).
///
/// ANY BINDABLE KEY, NOT A FIXED LIST. Twenty-three letters were listed by hand in the handler, and a tool
/// rebound to W or Z was saved, shown in the window, and never heard: the press did not reach the table.
pub fn pressed_chord(ctx: &egui::Context) -> Option<Chord> {
    // asked BEFORE `ctx.input`: inside it `wants_keyboard_input` deadlocks on the input lock
    let typing = ctx.egui_wants_keyboard_input();
    ctx.input(|i| {
        let held = Chord::of_press(i.modifiers, egui::Key::Escape);
        let chord_key = held.ctrl || held.control;
        // Alt and Ctrl together reach nothing: Alt is the way out of a field, Ctrl and Control do not need one
        let ok = if typing { i.modifiers.alt != chord_key } else { !i.modifiers.alt };
        if !ok {
            return None;
        }
        i.events.iter().find_map(|e| match e {
            egui::Event::Key { key, pressed: true, repeat: false, .. } => Some(Chord { key: *key, ..held }),
            _ => None,
        })
    })
    .filter(Chord::bindable_key)
}

/// Which action a key press means in this area. It reads the settings and nothing else - the whole
/// application was never needed for a table lookup.
pub fn hotkey_action(set: &Settings, area: &str, key: impl Into<Chord>) -> Option<&'static str> {
    let pressed = key.into();
    // A BINDING THIS SYSTEM REFUSES IS NOT RUN. The settings travel with the profile: Cmd+W bound on a Mac arrives
    // on Linux as Ctrl+W, which a field erases a word with - running the tool there would do both.
    HOTKEYS.iter().filter(|r| r.area == area).find(|r| Chord::parse(&hotkey_key(set, r.action)) == Some(pressed)).map(|r| r.action).filter(|a| hotkey_refusal(a, &pressed).is_none())
}

pub fn fit(project: &Project, view: &mut View2d, rect: Rect) {
    let mut b = bounds(&project.contours);
    for body in &project.bodies {
        if let Some(mb) = body.mesh.bounds() {
            let lo = Point2::new(mb.min.x, mb.min.y);
            let hi = Point2::new(mb.max.x, mb.max.y);
            b = Some(match b {
                None => (lo, hi),
                Some((bl, bh)) => (Point2::new(bl.x.min(lo.x), bl.y.min(lo.y)), Point2::new(bh.x.max(hi.x), bh.y.max(hi.y))),
            });
        }
    }
    let Some(b) = b else { return };
    // a sketch under 1 mm is framed at its own size, as the 3D view frames a model; only one with no extent at all, a
    // single point, takes 1 mm, and a side of no length (a lone line) divides nothing
    let raw = (b.1.x - b.0.x).max(b.1.y - b.0.y);
    let (w, h) = if raw > 1e-9 { ((b.1.x - b.0.x).max(raw * 1e-9) as f32, (b.1.y - b.0.y).max(raw * 1e-9) as f32) } else { (1.0, 1.0) };
    view.scale = (rect.width() / w).min(rect.height() / h) * 0.85;
    view.fit = view.scale;
    view.center = Vec2::new(((b.0.x + b.1.x) / 2.0) as f32, ((b.0.y + b.1.y) / 2.0) as f32);
    view.initialized = true;
}

/// Draw the header. Returns the new name if a person touched it.
pub fn props_header(ui: &mut egui::Ui, icon: &str, kind_key: &str, name: NameSlot, lin: &Lineage) -> Option<String> {
    ui.heading(format!("{icon} {}", qymcad_i18n::tr(kind_key)));
    let mut renamed = None;
    match name {
        NameSlot::Editable(mut stored) => {
            ui.horizontal(|ui| {
                ui.label(qymcad_i18n::tr("pp-name"));
                if name_edit(ui, &mut stored).changed() {
                    renamed = Some(stored);
                }
            });
        }
        NameSlot::Fixed(s) if !s.is_empty() => {
            ui.label(egui::RichText::new(qymcad_i18n::name(&s)).weak().small());
        }
        _ => {}
    }
    let mut chain = |key: &str, items: &[String]| {
        if items.is_empty() {
            return;
        }
        ui.label(egui::RichText::new(qymcad_i18n::tr(key)).small().weak());
        for it in items {
            ui.label(format!("  · {it}"));
        }
    };
    chain("fp-built-on", &lin.built_on);
    chain("fp-dependents", &lin.dependents);
    ui.separator();
    renamed
}

/// INSTALL THE FONTS INTO A CONTEXT — IN ONE PLACE.
///
/// This used to be done right inside the application start-up, and the `qym-bold` family existed only
/// there. Any other `egui` context (and tests build their own) panicked on the first attempt to draw a
/// bold label: "FontFamily::Name(\"qym-bold\") is not bound to any fonts". That is, a test checking the
/// drawing was impossible precisely because the font setup was hidden inside `main`.
///
/// Now there is one setup, and everyone who needs a full context calls it.
/// THE NODE NAME FIELD: show it IN WORDS, store WHAT WAS TYPED.
///
/// A node's name is both document data and text on screen: the core stores a KEY (`name-plane`), and a
/// person may rename it to anything. A field bound straight to the stored value showed the key —
/// `name-plane` and `name-assembly` appeared in the right-hand panel.
///
/// The rule is simple and reversible: while the name has not been touched, the key stays in the
/// document (and the label follows a change of language); the very first edit stores the WORDS typed,
/// and they must not be translated any more.
pub fn name_edit(ui: &mut egui::Ui, stored: &mut String) -> egui::Response {
    let mut shown = qymcad_i18n::name(stored);
    let r = ui.text_edit_singleline(&mut shown);
    if r.changed() {
        *stored = shown;
    }
    r
}

thread_local! {
    /// The keys of the bar fields holding a value that cannot be taken, as last drawn - for the tools that act on a
    /// click rather than on Enter, where no frame context is at hand (the offset of a sketch).
    static BAR_BAD_KEYS: std::cell::RefCell<std::collections::HashSet<&'static str>> = std::cell::RefCell::new(std::collections::HashSet::new());
}

pub use icons::{forget_all_cad_icons, icon_button, icon_image, icon_label, icon_small_button, ResolvedIcon};

/// The six, taken from a CONTEXT, where the fields are already borrows.
#[macro_export]
macro_rules! doing_in {
    ($x:expr) => {
        $crate::Doing { armed: $x.armed, cmd: &*$x.cmd, gsel: &*$x.gsel, joint: $x.joint, picking: *$x.picking, sketch_ses: *$x.sketch_ses, workbench: $x.workbench }
    };
}

#[cfg(test)]
mod state_is_free_of_the_god_object {
    /// The working part of a file: everything before the first `#[cfg(test)]` module. Copied here rather
    /// than borrowed from the dictionary crate - a record of the interface must not depend on the words
    /// shown to a person.
    fn working_part(text: &str) -> &str {
        text.split("#[cfg(test)]").next().unwrap_or(text)
    }

    /// THE STATE OF THE INTERFACE KNOWS NOTHING ABOUT THE APPLICATION THAT HOLDS IT.
    ///
    /// This is the whole reason the file exists, and the last thing standing between a workbench and a
    /// crate of its own. A record that names `App` - in a signature, in a body, anywhere but a comment -
    /// drags the god object behind it, and the crate would have to depend on the crate declaring it.
    ///
    /// COMMENTS ARE STRIPPED FIRST. The header of this very file names `App` four times while explaining
    /// why it must not; a check that read the raw text would forbid the explanation along with the
    /// dependency.
    #[test]
    pub(crate) fn the_state_names_no_application() {
        // THE WORKING PART ONLY. The check below spells the forbidden word out to look for it, so a scan
        // of the whole file finds its own needle and goes red on itself for ever.
        let src = working_part(concat!(
            include_str!("contexts.rs"),
            "\n",
            include_str!("corners.rs"),
            "\n",
            include_str!("dims.rs"),
            "\n",
            include_str!("edits.rs"),
            "\n",
            include_str!("fields.rs"),
            "\n",
            include_str!("fonts.rs"),
            "\n",
            include_str!("geom.rs"),
            "\n",
            include_str!("gizmos.rs"),
            "\n",
            include_str!("handles.rs"),
            "\n",
            include_str!("hotkeys.rs"),
            "\n",
            include_str!("jobs.rs"),
            "\n",
            include_str!("joints.rs"),
            "\n",
            include_str!("navigation.rs"),
            "\n",
            include_str!("painting.rs"),
            "\n",
            include_str!("rebuild.rs"),
            "\n",
            concat!(
                include_str!("records/commands.rs"),
                "\n",
                include_str!("records/frame.rs"),
                "\n",
                include_str!("records/gizmos.rs"),
                "\n",
                include_str!("records/pieces.rs"),
                "\n",
                include_str!("records/scene.rs"),
                "\n",
                include_str!("records/tools.rs"),
                "\n",
                include_str!("records.rs")
            ),
            "\n",
            include_str!("scene.rs"),
            "\n",
            include_str!("section.rs"),
            "\n",
            include_str!("selection.rs"),
            "\n",
            include_str!("settings.rs"),
            "\n",
            include_str!("sketch_diag.rs"),
            "\n",
            include_str!("sketch_edit.rs"),
            "\n",
            include_str!("text.rs"),
            "\n",
            include_str!("tool_steps.rs"),
            "\n",
            include_str!("tree.rs"),
            "\n",
            include_str!("lib.rs")
        ));
        let mut sins: Vec<String> = Vec::new();
        for (i, line) in src.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            if code.split(|c: char| !c.is_alphanumeric() && c != '_').any(|w| w == "App") {
                sins.push(format!("lib.rs:{}: {}", i + 1, line.trim()));
            }
        }
        assert!(sins.is_empty(), "the state of the interface names the application ({}):\n{}", sins.len(), sins.join("\n"));
    }
}

#[cfg(test)]
mod picking {
    /// A SHEET COPIED FROM A FACE, AT THE SAME DEPTH, IS WHAT A SURFACE TOOL GETS - and only a surface tool.
    #[test]
    fn a_sheet_on_its_face_wins_only_for_the_tools_that_want_a_surface() {
        // the solid met first, the sheet at its depth after it
        assert!(super::nearer_hit(10.0, true, Some((10.0, false)), true), "a surface tool must take the sheet lying on the face");
        assert!(!super::nearer_hit(10.0, true, Some((10.0, false)), false), "a tool for bodies must keep the face of the part");
        // the sheet met first, the solid after it
        assert!(!super::nearer_hit(10.0, false, Some((10.0, true)), true), "a surface tool must not give the sheet up for the face under it");
        // a nearer body is nearer, whatever it is
        assert!(super::nearer_hit(9.0, false, Some((10.0, true)), true), "what stands in front must be taken");
        assert!(!super::nearer_hit(11.0, true, Some((10.0, false)), true), "a sheet behind the face must not be reached through it");
    }
}

#[cfg(test)]
mod tool_buttons {
    use super::{take_or_drop, BarAsk};

    /// A TOOL BUTTON TAKES ITS TOOL WHEN THE HAND IS FREE OF IT AND PUTS IT DOWN WHEN IT IS HELD: a sketch tool back to
    /// the arrow, a command cancelled.
    #[test]
    fn a_tool_button_takes_or_puts_down() {
        assert!(matches!(take_or_drop(false, BarAsk::FeatCmd(4)), BarAsk::FeatCmd(4)), "a free hand takes the command");
        assert!(matches!(take_or_drop(true, BarAsk::FeatCmd(4)), BarAsk::CancelAllTools(crate::Then::Nothing)), "a held command is put down");
        assert!(matches!(take_or_drop(true, BarAsk::SketchTool(3)), BarAsk::SketchSelectMode), "a held sketch tool gives the arrow back");
        assert!(matches!(take_or_drop(false, BarAsk::SketchTool(3)), BarAsk::SketchTool(3)), "a free hand takes the sketch tool");
    }
}

#[cfg(test)]
mod blend_outline {
    use super::{edge_blend_outline, Blend};
    use qymcad_core::geom::{Mesh, Point3};

    /// The top front edge of a 40 x 30 x 10 block: one triangle of the top face, one of the front face, sharing it.
    fn corner() -> Mesh {
        let p = |x, y, z| Point3::new(x, y, z);
        Mesh { verts: vec![p(0.0, 0.0, 10.0), p(40.0, 0.0, 10.0), p(40.0, 30.0, 10.0), p(0.0, 0.0, 0.0)], tris: vec![[0, 1, 2], [1, 0, 3]], ..Default::default() }
    }

    fn near(a: [f64; 3], b: [f64; 3]) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < 1e-9)
    }

    /// A FILLET OF RADIUS r ON A SQUARE EDGE meets the top face r back from the edge and the front face r down, and
    /// the lines move with the radius: the preview follows the value before Enter.
    #[test]
    fn a_fillet_outline_follows_its_radius() {
        let edge = [[0.0f32, 0.0, 10.0], [40.0, 0.0, 10.0]];
        for r in [2.0, 4.0] {
            let out = edge_blend_outline(&corner(), &edge, Blend::Round(r));
            assert_eq!(out.len(), 4, "two lines along the faces and two end sections");
            assert!(near(out[0][0], [0.0, r, 10.0]) && near(out[0][1], [40.0, r, 10.0]), "on the top face, r = {r} back: {:?}", out[0]);
            assert!(near(out[1][0], [0.0, 0.0, 10.0 - r]) && near(out[1][1], [40.0, 0.0, 10.0 - r]), "on the front face, r = {r} down: {:?}", out[1]);
            let mid = out[2][4];
            let d = ((mid[1] - r).powi(2) + (mid[2] - (10.0 - r)).powi(2)).sqrt();
            assert!((d - r).abs() < 1e-9, "the section is an arc of radius {r} about the centre: {mid:?}");
        }
    }

    /// THE EDGE IS CUT FINER THAN THE MESH: a block's edge comes in 24 pieces against one side of a triangle, and every
    /// piece still finds its two faces.
    #[test]
    fn an_edge_cut_finer_than_the_mesh_finds_its_faces() {
        let out = edge_blend_outline(&corner(), &[[0.0, 0.0, 10.0], [20.0, 0.0, 10.0], [40.0, 0.0, 10.0]], Blend::Round(2.0));
        assert_eq!(out.first().map(|l| l.len()), Some(3), "a point of the line for every vertex of the edge: {out:?}");
        assert!(near(out[0][1], [20.0, 2.0, 10.0]), "{:?}", out[0]);
    }

    /// A HOLE'S RIMS ARE AS WIDE AS ITS DIAMETER and its bottom as deep as its depth, down from the face.
    #[test]
    fn a_hole_outline_follows_its_diameter_and_depth() {
        let pl = qymcad_core::feature::PlaneFrame::from_origin_normal([20.0, 15.0, 10.0], [0.0, 0.0, 1.0], 0.0).matrix12();
        for (d, depth) in [(6.0, 15.0), (12.0, 5.0)] {
            let out = super::hole_outline(&pl, qymcad_core::model::HoleTool { kind: 0, diameter: d, depth, dia2: 0.0, depth2: 0.0 });
            let p = out[0][0];
            assert!((((p[0] - 20.0).powi(2) + (p[1] - 15.0).powi(2)).sqrt() - d / 2.0).abs() < 1e-9 && (p[2] - 10.0).abs() < 1e-9, "the rim at the face, {d} across: {p:?}");
            assert!((out[1][0][2] - (10.0 - depth)).abs() < 1e-9, "the bottom {depth} down: {:?}", out[1][0]);
        }
    }

    /// A DRAFTED SIDE LEANS OUT BELOW THE NEUTRAL FACE by its depth times tan(angle), and its top edge, on the neutral
    /// face, stays where it is.
    #[test]
    fn a_draft_outline_leans_by_its_angle() {
        let p = |x, y, z| Point3::new(x, y, z);
        let mesh = Mesh { verts: vec![p(0.0, 0.0, 0.0), p(40.0, 0.0, 0.0), p(40.0, 0.0, 10.0), p(0.0, 0.0, 10.0)], tris: vec![[0, 1, 2], [0, 2, 3]], ..Default::default() };
        let face = qymcad_core::geom::MeshFace { triangles: vec![0, 1], normal: [0.0, -1.0, 0.0], centroid: p(20.0, 0.0, 5.0), area: 400.0, id: 1 };
        for angle in [3.0f64, 6.0] {
            let out = super::draft_outline(&mesh, &face, [20.0, 15.0, 10.0], [0.0, 0.0, 1.0], angle);
            assert_eq!(out.len(), 4, "the four sides of the face: {out:?}");
            let lean = 10.0 * angle.to_radians().tan();
            let pts: Vec<[f64; 3]> = out.iter().flatten().copied().collect();
            assert!(pts.iter().any(|q| near(*q, [0.0, -lean, 0.0])), "the bottom leans out {lean} at {angle} deg: {pts:?}");
            assert!(pts.iter().any(|q| near(*q, [0.0, 0.0, 10.0])), "the edge on the neutral face stays: {pts:?}");
        }
    }

    /// THE SECOND CLICK OF A POLYGON LIES ON IT in every mode: a vertex (inscribed), the middle of an edge
    /// (circumscribed), the end of the edge begun at the first click (by edge).
    #[test]
    fn a_polygon_passes_through_its_second_click() {
        use qymcad_core::geom::Point2;
        let (a, b) = (Point2::new(0.0, 0.0), Point2::new(15.0, 0.0));
        let vertices = |(c, v): (Point2, Point2)| -> Vec<[f64; 2]> {
            let (r, a0) = (((v.x - c.x).powi(2) + (v.y - c.y).powi(2)).sqrt(), (v.y - c.y).atan2(v.x - c.x));
            (0..6)
                .map(|k| {
                    let t = a0 + std::f64::consts::TAU * k as f64 / 6.0;
                    [c.x + r * t.cos(), c.y + r * t.sin()]
                })
                .collect()
        };
        let on = |p: [f64; 2], q: [f64; 2]| (p[0] - q[0]).abs() < 1e-9 && (p[1] - q[1]).abs() < 1e-9;
        assert!(vertices(super::polygon_from_clicks(a, b, 6, 0)).iter().any(|v| on(*v, [15.0, 0.0])), "inscribed: a vertex at the click");
        let vs = vertices(super::polygon_from_clicks(a, b, 6, 1));
        assert!(
            (0..6).any(|k| {
                let (p, q) = (vs[k], vs[(k + 1) % 6]);
                on([(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0], [15.0, 0.0])
            }),
            "circumscribed: the middle of an edge at the click: {vs:?}"
        );
        let vs = vertices(super::polygon_from_clicks(a, b, 6, 2));
        assert!(vs.iter().any(|v| on(*v, [0.0, 0.0])) && vs.iter().any(|v| on(*v, [15.0, 0.0])), "by edge: both clicks are vertices: {vs:?}");
    }

    /// A FILLET IN AN INNER CORNER lies in the corner, not beyond it. Reported behaviour: with the tool open on a frame
    /// with bosses in its corners, the arcs of the preview lay on the far side of the edge, inside the part, while the
    /// fillet itself built outwards. The floor z = 0 runs on in +y and the wall y = 0 rises in +z from their common
    /// edge; R2 meets the floor 2 out along it and the wall 2 up it, about the centre (y 2, z 2) in the air.
    #[test]
    fn a_fillet_in_an_inner_corner_lies_in_the_corner() {
        let p = |x, y, z| Point3::new(x, y, z);
        let m = Mesh { verts: vec![p(0.0, 0.0, 0.0), p(40.0, 0.0, 0.0), p(40.0, 30.0, 0.0), p(0.0, 0.0, 10.0)], tris: vec![[0, 1, 2], [1, 0, 3]], ..Default::default() };
        let edge = [[0.0f32, 0.0, 0.0], [40.0, 0.0, 0.0]];
        let out = edge_blend_outline(&m, &edge, Blend::Round(2.0));
        assert_eq!(out.len(), 4, "{out:?}");
        let lines: Vec<[f64; 3]> = vec![out[0][0], out[1][0]];
        assert!(lines.iter().any(|q| near(*q, [0.0, 2.0, 0.0])) && lines.iter().any(|q| near(*q, [0.0, 0.0, 2.0])), "on the floor 2 out and on the wall 2 up: {lines:?}");
        let mid = out[2][4];
        let k = 2.0 - 2.0 / 2f64.sqrt();
        assert!(near(mid, [0.0, k, k]), "the arc bows into the corner, about the centre in the air: {mid:?}");
        let cut = edge_blend_outline(&m, &edge, Blend::Cut(1.5, 1.5, None));
        let legs: Vec<[f64; 3]> = vec![cut[0][0], cut[1][0]];
        assert!(legs.iter().any(|q| near(*q, [0.0, 1.5, 0.0])) && legs.iter().any(|q| near(*q, [0.0, 0.0, 1.5])), "a chamfer's legs on the floor and the wall: {legs:?}");
    }

    /// A CHAMFER cuts straight across from leg to leg.
    #[test]
    fn a_chamfer_outline_is_cut_straight_at_its_legs() {
        let out = edge_blend_outline(&corner(), &[[0.0, 0.0, 10.0], [40.0, 0.0, 10.0]], Blend::Cut(1.5, 3.0, None));
        assert!(near(out[0][0], [0.0, 1.5, 10.0]) && near(out[1][0], [0.0, 0.0, 7.0]), "{out:?}");
        assert_eq!(out[2], vec![out[0][0], out[1][0]], "the end section is the straight cut");
    }

    /// A CHAMFER BY TWO LEGS lays its first leg on the reference face, as the kernel does: named the upright face, the
    /// 1.5 runs down it and the 3 across the top. Reported behaviour: the preview drew the first leg on both faces.
    #[test]
    fn a_chamfer_by_two_legs_lays_the_first_on_the_reference_face() {
        let out = edge_blend_outline(&corner(), &[[0.0, 0.0, 10.0], [40.0, 0.0, 10.0]], Blend::Cut(1.5, 3.0, Some([0.0, 1.0, 0.0])));
        let legs = [out[0][0], out[1][0]];
        assert!(legs.iter().any(|q| near(*q, [0.0, 0.0, 8.5])) && legs.iter().any(|q| near(*q, [0.0, 3.0, 10.0])), "the legs of a chamfer 1.5 on the upright face and 3 on the top lie at {legs:?}");
    }

    /// An edge with one face found has nothing to be set back from, and draws nothing rather than a guess.
    #[test]
    fn an_edge_with_one_face_draws_nothing() {
        let mut m = corner();
        m.tris.pop();
        assert!(edge_blend_outline(&m, &[[0.0, 0.0, 10.0], [40.0, 0.0, 10.0]], Blend::Round(2.0)).is_empty());
    }
}

mod selection;
pub use selection::*;
mod jobs;
pub use jobs::*;
mod settings;
pub use settings::*;
mod records;
pub use records::*;
mod scene;
pub use floor::*;
mod floor;
pub use scene::*;
mod contexts;
pub use contexts::*;
mod painting;
pub use painting::*;
mod tree;
pub use tree::*;
mod navigation;
pub use navigation::*;
mod hotkeys;
pub use hotkeys::*;
mod geom;
pub use geom::*;
mod dims;
pub use dims::*;
mod gizmos;
pub use gizmos::*;
mod handles;
pub use handles::*;
mod joints;
pub use joints::*;
mod sketch_diag;
pub use sketch_diag::*;
mod section;
pub use section::*;
mod rebuild;
pub use rebuild::*;
mod edits;
pub use edits::*;
mod fields;
pub use fields::*;
mod fonts;
pub use fonts::*;
mod text;
pub use text::*;
mod sketch_edit;
pub use sketch_edit::*;
mod corners;
pub use corners::*;
mod tool_steps;
pub use tool_steps::*;
mod voice;
pub use voice::*;
mod faults;
pub use faults::*;
pub mod catalogue;
