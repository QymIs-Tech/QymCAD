//! THE HANDLES OF A COMMAND: its arrows and rings, grabbed and dragged.

use super::*;

/// HOW LONG THE ARROWS OF A POSITION ARE ON THE SCREEN, in pixels: a point or the origin of an axis is moved along X, Y and
/// Z by arrows of this length whatever the zoom.
pub(crate) const POSITION_ARROW_PX: f64 = 60.0;

/// HOW WIDE THE RINGS OF A DATUM ARE ON THE SCREEN, in pixels (the radius), whatever the zoom: inside the arrows of a
/// position (60 px), so the ball at an arrow's tip stands clear of every ring and the two are not taken for each other.
pub(crate) const DATUM_RING_PX: f64 = 42.0;

/// A HANDLE OF THE COMMAND IN HAND, by the value it drives: an arrow dragged along its direction, a ring turned about its
/// axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    Arrow(&'static str),
    Ring(&'static str),
}

/// WHAT TURNING A RING OF A COMMAND CHANGES.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RingTurns {
    /// The angle field the ring is called by, by the degrees it is turned: a datum plane's tilts and its turn.
    Field,
    /// The direction in the fields `dx`, `dy`, `dz`, turned about the ring's axis: a datum axis by hand.
    Direction,
}

/// ONE RING OF A COMMAND, as it is drawn and as it is grabbed: one geometry for both.
pub struct CmdRing {
    pub center: [f64; 3],
    /// The unit axis the ring turns about; the ring lies across it.
    pub axis: [f64; 3],
    pub radius: f64,
    /// The field it turns, or the name of the axis it turns the direction about.
    pub key: &'static str,
    pub turns: RingTurns,
}

impl CmdRing {
    /// The ring as points in the world, closed: 48 chords, as a ring of a body's gizmo.
    pub fn points(&self) -> Vec<[f64; 3]> {
        let (u, v) = perp_basis(self.axis);
        let (c, r) = (self.center, self.radius);
        (0..=48)
            .map(|k| {
                let (s, co) = (f64::from(k) / 48.0 * std::f64::consts::TAU).sin_cos();
                [c[0] + r * (u[0] * co + v[0] * s), c[1] + r * (u[1] * co + v[1] * s), c[2] + r * (u[2] * co + v[2] * s)]
            })
            .collect()
    }
}

/// EVERY HANDLE OF THE COMMAND IN HAND: its arrows and its rings.
pub struct Handles {
    pub arrows: Vec<CmdArrow>,
    pub rings: Vec<CmdRing>,
}

/// The handles of the command in hand (`handle_arrows`, `handle_rings`).
pub fn handles(pn: &Painting) -> Handles {
    Handles { arrows: handle_arrows(pn), rings: handle_rings(pn) }
}

/// THE HANDLE ARROWS OF THE COMMAND IN HAND, as they are drawn and as they are grabbed: the arrow of a value along a
/// direction (a face pushed, thickened, shelled, a split, a datum plane's offset - `face_arrow_geometry`), or the three
/// arrows of a position along X, Y and Z (a datum point by its coordinates, a datum axis by its origin). Reported
/// behaviour: the datum tools had no arrow to drag, their values were typed only.
pub fn handle_arrows(pn: &Painting) -> Vec<CmdArrow> {
    if let (Some(key), Some((o, tip, n))) = (face_arrow_key(pn.armed), face_arrow_geometry(pn)) {
        let len = (tip[0] - o[0]) * n[0] + (tip[1] - o[1]) * n[1] + (tip[2] - o[2]) * n[2];
        return vec![CmdArrow { base: o, dir: n, len, key }];
    }
    let keys: Option<[&'static str; 3]> = match pn.armed.cmd_kind() {
        21 if pn.datum.pt_mode == 0 => Some(["x", "y", "z"]),
        22 if pn.datum.axis_mode == 1 => Some(["ox", "oy", "oz"]),
        _ => None,
    };
    let Some(keys) = keys else { return Vec::new() };
    let base = keys.map(|k| cmd_val(pn.cmd, k));
    let len = POSITION_ARROW_PX / f64::from(pn.cam.scale).max(1e-9);
    let axes = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    keys.iter().zip(axes).map(|(&key, dir)| CmdArrow { base, dir, len, key }).collect()
}

/// THE DATUM PLANE THE COMMAND IN HAND WILL MAKE, as the core makes it (`Project::resolve_plane_into`): the preview and
/// the rings are drawn from it, so what is seen before Enter is what Enter makes.
pub struct DatumPlaneMade {
    /// Where it stands: the reference's origin, moved by the offset to the side the offset goes to.
    pub origin: [f64; 3],
    /// Its normal before the tilt, the reference's: the axes of the tilt are taken from it (`PlaneTilt::axes`).
    pub untilted: [f64; 3],
    /// Its normal tilted.
    pub normal: [f64; 3],
    /// Its turn about the normal, in degrees.
    pub turn: f64,
}

/// The datum plane the command in hand will make, once its reference is picked.
pub fn datum_plane_made(pn: &Painting) -> Option<DatumPlaneMade> {
    use qymcad_core::feature::{PlaneTilt, SketchPlane};
    if pn.armed.cmd_kind() != 20 {
        return None;
    }
    let sp = pn.datum.plane_pick.as_ref()?;
    let (o, side) = mirror_plane_world(&DrawCtx { cam: &pn.cam, set: pn.set, scheme: pn.scheme, project: pn.project, active_path: pn.active_path }, sp)?;
    let l = (side[0] * side[0] + side[1] * side[1] + side[2] * side[2]).sqrt();
    if l < 1e-9 {
        return None;
    }
    let side = [side[0] / l, side[1] / l, side[2] / l];
    // a base plane is offset to its positive side, while its normal is its frame's (XZ: +Y against -Y)
    let untilted = match sp {
        SketchPlane::World(b) => b.frame().normal(),
        SketchPlane::Datum(_) | SketchPlane::Face(..) => side,
    };
    let d = cmd_val(pn.cmd, "dist");
    let tilt = PlaneTilt { about_x: cmd_val(pn.cmd, "tilt_x"), about_y: cmd_val(pn.cmd, "tilt_y") };
    Some(DatumPlaneMade { origin: [o[0] + side[0] * d, o[1] + side[1] * d, o[2] + side[2] * d], untilted, normal: tilt.apply(untilted), turn: cmd_val(pn.cmd, "turn") })
}

/// THE RINGS OF THE COMMAND IN HAND: a datum plane is tilted about its X and its Y and turned about its normal, a datum
/// axis by hand has its direction turned about X, Y and Z. Reported behaviour: the axis by hand and the datum plane had
/// no rings, their direction was typed only.
pub fn handle_rings(pn: &Painting) -> Vec<CmdRing> {
    let radius = DATUM_RING_PX / f64::from(pn.cam.scale).max(1e-9);
    match pn.armed.cmd_kind() {
        20 => {
            let Some(m) = datum_plane_made(pn) else { return Vec::new() };
            let f = qymcad_core::feature::PlaneTilt::axes(m.untilted);
            let field = |axis: [f64; 3], key: &'static str| CmdRing { center: m.origin, axis, radius, key, turns: RingTurns::Field };
            vec![field(f.x, "tilt_x"), field(f.y, "tilt_y"), field(m.normal, "turn")]
        }
        22 if pn.datum.axis_mode == 1 => {
            let center = ["ox", "oy", "oz"].map(|k| cmd_val(pn.cmd, k));
            let axes = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
            ["about_x", "about_y", "about_z"].iter().zip(axes).map(|(&key, axis)| CmdRing { center, axis, radius, key, turns: RingTurns::Direction }).collect()
        }
        _ => Vec::new(),
    }
}

/// THE HANDLE UNDER THE POINTER, grabbed: a press on it takes the value as it stands, and the field lets go of the
/// keyboard.
pub fn grab_handle(ctx: &egui::Context, pn: &Painting, scr: &Screen, pos: Pos2) -> Option<Handle> {
    let hit = handle_at(pn, scr, pos)?;
    ctx.memory_mut(|m| m.stop_text_input());
    Some(hit)
}

/// The handle whose line passes within 9 px of `pos`, the nearest: the arrows before the rings, as on a body's gizmo -
/// an arrow's ball stands inside the rings; none elsewhere.
pub fn handle_at(pn: &Painting, scr: &Screen, pos: Pos2) -> Option<Handle> {
    let hs = handles(pn);
    let nearest = |hits: Vec<(&'static str, f32)>| hits.into_iter().filter(|(_, d)| *d <= 9.0).min_by(|x, y| x.1.total_cmp(&y.1)).map(|(k, _)| k);
    if let Some(k) = nearest(hs.arrows.iter().map(|a| (a.key, screen_dist_seg(pos, scr.at(a.base).0, scr.at(a.tip()).0))).collect()) {
        return Some(Handle::Arrow(k));
    }
    let ring_dist = |r: &CmdRing| {
        let pts: Vec<Pos2> = r.points().into_iter().map(|p| scr.at(p).0).collect();
        pts.windows(2).map(|w| screen_dist_seg(pos, w[0], w[1])).fold(f32::MAX, f32::min)
    };
    nearest(hs.rings.iter().map(|r| (r.key, ring_dist(r))).collect()).map(Handle::Ring)
}

/// WHERE THE POINTER IS AND HOW FAR IT WENT this frame, on the screen.
#[derive(Clone, Copy)]
pub struct PointerStep {
    pub at: Pos2,
    pub by: egui::Vec2,
}

/// THE HANDLE `held` DRAGGED BY THE POINTER: an arrow adds the pointer's travel along its line, in the units of the world,
/// to its field; a ring adds the angle the pointer turned about its centre on the screen. The field and the handle are
/// one value. `hs` are the command's (`handles`).
pub fn drag_handle(cmd: &mut FeatCommand, regen: &mut Rebuilding, hs: &Handles, held: Option<Handle>, scr: &Screen, step: PointerStep) {
    let set = |cmd: &mut FeatCommand, key: &str, v: f64, digits: usize| {
        if let Some(p) = cmd.params.iter_mut().find(|p| p.key == key) {
            p.val = v;
            p.txt = format!("{v:.digits$}"); // the field and the handle are one value, not two independent ones
        }
    };
    match held {
        Some(Handle::Arrow(key)) => {
            let Some(a) = hs.arrows.iter().find(|a| a.key == key) else { return };
            let l = POSITION_ARROW_PX / f64::from(scr.cam.scale).max(1e-9);
            let (s0, s1) = (scr.at(a.base).0, scr.at([a.base[0] + a.dir[0] * l, a.base[1] + a.dir[1] * l, a.base[2] + a.dir[2] * l]).0);
            let pd = s1 - s0;
            let denom = f64::from(pd.x * pd.x + pd.y * pd.y);
            if denom < 1e-6 {
                return;
            }
            set(cmd, a.key, cmd_val(cmd, a.key) + f64::from(step.by.x * pd.x + step.by.y * pd.y) * l / denom, 2);
        }
        Some(Handle::Ring(key)) => {
            let Some(r) = hs.rings.iter().find(|r| r.key == key) else { return };
            // the turn about the centre on the screen, counter-clockwise as seen (y runs down), signed by which way
            // the axis faces the eye - as a ring of a body's gizmo turns
            let radial = step.at - scr.at(r.center).0;
            let r2 = f64::from(radial.x * radial.x + radial.y * radial.y);
            if r2 < 4.0 {
                return;
            }
            let ccw = -f64::from(radial.x * step.by.y - radial.y * step.by.x) / r2;
            let b = scr.basis.2;
            let deg = ccw.to_degrees() * ring_drag_sign(b[0] * r.axis[0] + b[1] * r.axis[1] + b[2] * r.axis[2]);
            match r.turns {
                RingTurns::Field => set(cmd, r.key, cmd_val(cmd, r.key) + deg, 2),
                RingTurns::Direction => {
                    let keys = ["dx", "dy", "dz"];
                    let d = qymcad_core::feature::turn_about(keys.map(|k| cmd_val(cmd, k)), r.axis, deg);
                    for (k, v) in keys.iter().zip(d) {
                        set(cmd, k, v, 4); // a component of a unit direction: two digits would lose a turn of a degree
                    }
                }
            }
        }
        None => return,
    }
    invalidate(regen);
}

/// THE PLACE THE HANDLES OF THE COMMAND TOOK ON THE SCREEN in a frame, as they were drawn.
#[derive(Clone, Copy)]
pub(crate) struct HandlesSeen {
    pub(crate) frame: u64,
    pub(crate) rect: Rect,
}

pub(crate) fn handles_seen_id() -> egui::Id {
    egui::Id::new("command_handles_seen")
}

/// The place `hs` take on the screen: their lines and the balls at the arrows' tips, none when there are no handles.
pub fn handles_screen_rect(hs: &Handles, scr: &Screen) -> Option<Rect> {
    let pts = hs.arrows.iter().flat_map(|a| [a.base, a.tip()]).chain(hs.rings.iter().flat_map(|r| r.points())).map(|p| scr.at(p).0);
    pts.fold(None, |acc: Option<Rect>, p| Some(acc.map_or(Rect::from_min_max(p, p), |r| r.union(Rect::from_min_max(p, p))))).map(|r| r.expand(7.0))
}

/// Remember where the handles were drawn this frame (`handles_screen_rect`): the fields of the command stand clear of them.
pub fn remember_handles(ctx: &egui::Context, rect: Option<Rect>) {
    let frame = ctx.cumulative_frame_nr();
    ctx.data_mut(|d| match rect {
        Some(rect) => {
            d.insert_temp(handles_seen_id(), HandlesSeen { frame, rect });
        }
        None => d.remove::<HandlesSeen>(handles_seen_id()),
    });
}

/// Where the handles of the command were drawn, this frame or the one before; none when none were.
pub fn handles_seen(ctx: &egui::Context) -> Option<Rect> {
    let seen = ctx.data(|d| d.get_temp::<HandlesSeen>(handles_seen_id()))?;
    (seen.frame + 1 >= ctx.cumulative_frame_nr()).then_some(seen.rect)
}
