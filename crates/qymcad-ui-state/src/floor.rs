//! THE FLOOR OF THE 3D VIEW: the plane Z = 0 under a grid, and the world axes, round what is looked at.
//!
//! Reported behaviour: "in the 3D view the table, the grid and the axes are not endless - one sees the plane end as the
//! view moves". The grid was a square of egui lines round the middle of the view, at most 160 lines each way, faded by
//! the number of a line: tilted 10 deg over the floor and taken out to 20 m it stopped 117 to 151 rows under the horizon
//! in perspective, and its far edge crossed the frame in orthographic. Run to the horizon instead, the grid became a
//! ripple over the whole frame - reported as hard on the eyes, the Z axis standing up to the top of the screen.
//!
//! Now every pixel casts its ray of sight onto the plane, as the floor of a CAD is drawn: the grid is drawn where the
//! point met lies near a line, a line's width is measured in pixels from the size of a pixel on the plane there, a level
//! of the grid fades out as its lines close in under 30 points with the next level, five times coarser, taking its
//! place, and the whole floor fades out softly round the point the view looks at (`FloorLook::reach`, three times the
//! view), the axes further out and the Z axis with its height. There is no edge to see: it is a fade that goes with the
//! view at any zoom. The floor lies behind every body: it is drawn only where no body is. The card does the same in
//! `fs_floor`; this is the arithmetic the software raster runs, and the description both take.

use super::*;
use egui::Color32;

/// WHAT THE FLOOR IS DRAWN WITH: the step of its finest lines, the colours from the scheme, and where it fades out.
#[derive(Clone, Copy, Debug)]
pub struct FloorLook {
    /// The finest step of the grid, mm: its lines 9 to 45 points apart at the camera's target, as before.
    pub step: f64,
    /// A line of the finest step, and a line of every fifth.
    pub minor: Color32,
    pub major: Color32,
    /// The world axes: X and Y lie on the floor, Z stands up from the origin.
    pub axis_x: Color32,
    pub axis_y: Color32,
    pub axis_z: Color32,
    /// The point of the floor under the camera's target: the floor fades out round it.
    pub centre: [f64; 2],
    /// How far from `centre` the grid is gone, mm: `FLOOR_REACH` times half the diagonal of the view at the target.
    pub reach: f64,
}

/// The widths of the lines, in points, as the egui strokes drew them.
pub const FLOOR_LINE: f64 = 1.0;
pub const FLOOR_AXIS: f64 = 1.6;
pub const FLOOR_AXIS_Z: f64 = 1.8;
/// A level of the grid is gone at 12 points between its lines and whole at 30. At 9 and 18 - the fade the minor lines
/// had at the switch of the step - the levels far off stood so close the floor read as a ripple.
pub const FLOOR_FADE_GONE: f64 = 12.0;
pub const FLOOR_FADE_WHOLE: f64 = 30.0;
/// How many levels of the grid, each five times coarser.
pub const FLOOR_LEVELS: usize = 4;
/// The grid fades out as the ray of sight comes within this sine of level (0.7 deg): there a pixel spans more of the
/// plane than any level can show.
pub const FLOOR_GRAZING: f64 = 0.012;
/// How far the floor reaches, in half diagonals of the view at the target: the grid is whole to a third of it and gone
/// at it, the axes X and Y go on to twice as far.
pub const FLOOR_REACH: f64 = 3.0;
pub const FLOOR_WHOLE_TO: f64 = 0.35;
/// The Z axis is gone at this share of the floor's reach, about a third of the frame: standing up the whole reach it ran
/// to the top of the frame.
pub const FLOOR_Z_REACH: f64 = 0.15;

/// The floor for the camera and the scheme of `pn`, in a view of `rect`.
pub fn floor_look(pn: &Painting, rect: Rect) -> FloorLook {
    let sc = (pn.cam.scale as f64).max(1e-9);
    let mut step = 10.0_f64;
    while step * sc < 9.0 {
        step *= 5.0;
    }
    let pal = &pn.scheme.pal;
    let half_diagonal = (rect.width() as f64).hypot(rect.height() as f64) * 0.5 / sc;
    FloorLook {
        step,
        minor: qymcad_scheme::a(pal.grid_minor(), 170),
        major: qymcad_scheme::a(pal.grid(), 230),
        axis_x: pal.grid_axis_x(),
        axis_y: pal.grid_axis_y(),
        axis_z: pal.grid_axis_z(),
        centre: [pn.cam.target[0], pn.cam.target[1]],
        reach: FLOOR_REACH * half_diagonal,
    }
}

impl FloorLook {
    /// How much of the floor is left with the distance from its centre, for a fade that is gone at `out` times the reach:
    /// whole to `FLOOR_WHOLE_TO` of that, then down to nothing.
    fn fade(&self, out: f64) -> Fade {
        Fade { gone: self.reach * out, whole: FLOOR_WHOLE_TO * self.reach * out }
    }
}

/// A ray of sight: where it starts and where it runs. In perspective it starts at the eye and runs one unit along the
/// line of sight per unit of `along`'s length on it, so the distance it has gone is the depth.
#[derive(Clone, Copy, Debug)]
pub struct Ray {
    pub from: [f64; 3],
    pub along: [f64; 3],
}

/// THE RAYS OF SIGHT OF A FRAME: the camera, the frame in points and in pixels, and its projection.
#[derive(Clone, Copy, Debug)]
pub struct ViewRays {
    pub target: [f64; 3],
    /// The camera's own directions.
    pub right: [f64; 3],
    pub up: [f64; 3],
    pub fwd: [f64; 3],
    /// Points per mm at the target.
    pub scale: f64,
    /// Half the frame, in points.
    pub half: egui::Vec2,
    /// The size of one pixel in the measure of `through`: 2 across the frame.
    pub pixel: egui::Vec2,
    /// Pixels per point.
    pub ppp: f64,
    pub proj: ProjParams,
}

impl ViewRays {
    /// The rays of a frame of `rect` points drawn at `ppp` pixels a point.
    pub fn new(cam: &Cam3, rect: Rect, ppp: f32, proj: ProjParams) -> Self {
        let (right, up, fwd) = cam.basis();
        let px = rect.size() * ppp;
        ViewRays {
            target: cam.target,
            right,
            up,
            fwd,
            scale: (cam.scale as f64).max(1e-9),
            half: rect.size() * 0.5,
            pixel: egui::vec2(2.0 / px.x.max(1.0), 2.0 / px.y.max(1.0)),
            ppp: ppp as f64,
            proj,
        }
    }

    /// The ray through a point of the frame given from its middle, x to the right and y up, -1..1 across.
    pub fn through(&self, ndc: (f64, f64)) -> Ray {
        let (right, up, fwd) = (self.right, self.up, self.fwd);
        let s = (ndc.0 * self.half.x as f64 / self.scale, ndc.1 * self.half.y as f64 / self.scale);
        let side: [f64; 3] = std::array::from_fn(|a| right[a] * s.0 + up[a] * s.1);
        let inv_d = self.proj.inv_d_eye;
        if inv_d > 0.0 {
            Ray { from: std::array::from_fn(|a| self.target[a] - fwd[a] / inv_d), along: std::array::from_fn(|a| fwd[a] + side[a] * inv_d) }
        } else {
            Ray { from: std::array::from_fn(|a| self.target[a] + side[a]), along: fwd }
        }
    }
}

/// A line `width` pixels wide.
struct Line {
    width: f64,
}

impl Line {
    /// How much of the line a pixel `off` pixels from its middle covers.
    fn covers(&self, off: f64) -> f64 {
        (self.width * 0.5 + 0.5 - off).clamp(0.0, 1.0)
    }
}

/// A fade from nothing at `gone` to the whole at `whole`, smooth at both ends.
struct Fade {
    gone: f64,
    whole: f64,
}

impl Fade {
    fn at(&self, x: f64) -> f64 {
        let t = ((x - self.gone) / (self.whole - self.gone)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }
}

/// A colour premultiplied by its alpha, 0..1.
#[derive(Clone, Copy)]
struct Paint {
    rgba: [f32; 4],
}

impl Paint {
    const NONE: Paint = Paint { rgba: [0.0; 4] };

    /// A colour of the scheme taken `k` of the way.
    fn of(c: Color32, k: f64) -> Paint {
        Paint { rgba: c.to_array().map(|v| (v as f64 / 255.0 * k) as f32) }
    }

    /// This laid over `under`.
    fn over(self, under: Paint) -> Paint {
        Paint { rgba: std::array::from_fn(|k| self.rgba[k] + under.rgba[k] * (1.0 - self.rgba[3])) }
    }
}

/// THE FLOOR OF A FRAME, ROW BY ROW: what one picture of the floor needs besides its rays - where the Z axis can be seen.
///
/// A row is worked out in one pass: along a row of the picture the ray of sight changes by the same step from one pixel
/// to the next, so the point met on the floor and the size of a pixel there (what `fwidth` gives the card) come from
/// the row's first ray and two steps, with no ray cast per pixel. Cast three times a pixel, the floor cost a frame of
/// 1100 x 800 2.15 s of one thread in the build the checks run - the crate is not optimised there - and a person waiting
/// on an extrusion saw the frames of the preview go three times slower.
pub struct FloorRows<'a> {
    rays: &'a ViewRays,
    look: &'a FloorLook,
    /// The pixels the Z axis can fall on, when both its ends stand in front of the eye; none means every pixel.
    z_box: Option<egui::Rect>,
}

impl<'a> FloorRows<'a> {
    pub fn new(rays: &'a ViewRays, look: &'a FloorLook) -> Self {
        let on_screen = |q: [f64; 3]| {
            let rel = v_sub(q, rays.target);
            let depth = v_dot(rel, rays.fwd);
            let k = 1.0 + depth * rays.proj.inv_d_eye;
            (k > 1e-3).then(|| {
                let f = rays.scale / k;
                egui::pos2(((rays.half.x as f64 + v_dot(rel, rays.right) * f) * rays.ppp) as f32, ((rays.half.y as f64 - v_dot(rel, rays.up) * f) * rays.ppp) as f32)
            })
        };
        let top = look.reach * FLOOR_Z_REACH;
        let z_box = match (on_screen([0.0; 3]), on_screen([0.0, 0.0, top])) {
            (Some(a), Some(b)) => Some(egui::Rect::from_two_pos(a, b).expand((FLOOR_AXIS_Z * rays.ppp) as f32 + 2.0)),
            _ => None,
        };
        FloorRows { rays, look, z_box }
    }

    /// THE FLOOR ALONG ROW `y` of the picture, into `out` - the row's pixels, premultiplied: the grid and the axes X and Y
    /// where a ray meets the plane, the Z axis where a ray passes by it; transparent where neither is.
    pub fn row(&self, y: usize, out: &mut [Color32]) {
        let (rays, look) = (self.rays, self.look);
        let ppp = rays.ppp;
        let ndc_y = 1.0 - (y as f64 + 0.5) * rays.pixel.y as f64;
        let ndc_x = |x: f64| (x + 0.5) * rays.pixel.x as f64 - 1.0;
        let first = rays.through((ndc_x(0.0), ndc_y));
        let step = |r: Ray| Ray { from: v_sub(r.from, first.from), along: v_sub(r.along, first.along) };
        let right = step(rays.through((ndc_x(1.0), ndc_y)));
        let down = step(rays.through((ndc_x(0.0), ndc_y - rays.pixel.y as f64)));
        let grazing = Fade { gone: 0.0, whole: FLOOR_GRAZING };
        let lod = Fade { gone: FLOOR_FADE_GONE, whole: FLOOR_FADE_WHOLE };
        let line = Line { width: FLOOR_LINE * ppp };
        let axis = Line { width: FLOOR_AXIS * ppp };
        let persp = rays.proj.inv_d_eye > 0.0;
        // THE LOOP IS WRITTEN OUT BY HAND, without closures or iterators: this crate is not optimised in the build the
        // checks and a debug run use, and there every small closure is a call - per pixel of a full frame.
        let (fade_in, fade_out) = (look.fade(1.0), look.fade(2.0));
        let level_fade = Fade { gone: 0.0, whole: 0.01 };
        let (cx, cy, reach2) = (look.centre[0], look.centre[1], 2.0 * look.reach);
        for (x, px) in out.iter_mut().enumerate() {
            let k = x as f64;
            let from = [first.from[0] + right.from[0] * k, first.from[1] + right.from[1] * k, first.from[2] + right.from[2] * k];
            let along = [first.along[0] + right.along[0] * k, first.along[1] + right.along[1] * k, first.along[2] + right.along[2] * k];
            let mut colour = Paint::NONE;
            let az = along[2];
            // where the ray meets the floor: never if it runs level, and in perspective only in front of the eye
            let t = if az.abs() < 1e-12 { 0.0 } else { -from[2] / az };
            if az.abs() >= 1e-12 && (!persp || t > rays.proj.z_near) {
                let p = [from[0] + along[0] * t, from[1] + along[1] * t];
                let r = ((p[0] - cx) * (p[0] - cx) + (p[1] - cy) * (p[1] - cy)).sqrt();
                if r < reach2 {
                    // the size of a pixel on the plane: how far the point met moves a pixel to the right and a pixel down
                    let dtx = -(right.from[2] * az - from[2] * right.along[2]) / (az * az);
                    let dty = -(down.from[2] * az - from[2] * down.along[2]) / (az * az);
                    let fw = [
                        ((right.from[0] + right.along[0] * t + along[0] * dtx).abs() + (down.from[0] + down.along[0] * t + along[0] * dty).abs()).max(1e-12),
                        ((right.from[1] + right.along[1] * t + along[1] * dtx).abs() + (down.from[1] + down.along[1] * t + along[1] * dty).abs()).max(1e-12),
                    ];
                    let level = (az / (along[0] * along[0] + along[1] * along[1] + az * az).sqrt()).abs();
                    let fade = fade_in.at(r) * grazing.at(level);
                    // the strongest line of the levels here, and whether it is of the finest one
                    let (mut best, mut finest) = (0.0f64, false);
                    // every line of a coarser level is a line of the finest one: a pixel off the lines of the finest level
                    // is off every line, and most pixels are
                    let off_finest = |a: usize| (p[a] - (p[a] / look.step).round() * look.step).abs() / fw[a];
                    let on_a_line = line.covers(off_finest(0)) > 0.0 || line.covers(off_finest(1)) > 0.0;
                    if fade > 0.0 && on_a_line {
                        let mut spacing = look.step;
                        for lv in 0..FLOOR_LEVELS {
                            // the lines across X and the lines across Y each fade by their own spacing on screen: looked at
                            // low, the lines running away from the eye stay wide apart while the lines across the view close in
                            for a in 0..2 {
                                let off = (p[a] - (p[a] / spacing).round() * spacing).abs() / fw[a];
                                let cov = line.covers(off);
                                if cov > 0.0 {
                                    let c = cov * lod.at(spacing / (fw[a] * ppp));
                                    if c > best {
                                        (best, finest) = (c, lv == 0);
                                    }
                                }
                            }
                            spacing *= 5.0;
                        }
                    }
                    if best > 0.0 {
                        colour = Paint::of(if finest { look.minor } else { look.major }, best * fade);
                    }
                    let near_level = level_fade.at(level) * fade_out.at(r);
                    if near_level > 0.0 {
                        let ax = axis.covers(p[1].abs() / fw[1]);
                        if ax > 0.0 {
                            colour = Paint::of(look.axis_x, ax * near_level).over(colour);
                        }
                        let ay = axis.covers(p[0].abs() / fw[0]);
                        if ay > 0.0 {
                            colour = Paint::of(look.axis_y, ay * near_level).over(colour);
                        }
                    }
                }
            }
            if self.z_box.is_none_or(|b| b.contains(egui::pos2(x as f32 + 0.5, y as f32 + 0.5))) {
                colour = self.z_axis(Ray { from, along }).over(colour);
            }
            *px = if colour.rgba[3] >= 0.5 / 255.0 {
                let c = colour.rgba.map(|v| (v * 255.0).round().clamp(0.0, 255.0) as u8);
                Color32::from_rgba_premultiplied(c[0], c[1], c[2], c[3])
            } else {
                Color32::TRANSPARENT
            };
        }
    }

    /// THE Z AXIS where `ray` passes closest to the line standing up from the origin, fading with its height.
    fn z_axis(&self, ray: Ray) -> Paint {
        let (rays, look) = (self.rays, self.look);
        let (o, d) = (ray.from, ray.along);
        let aa = d.iter().map(|v| v * v).sum::<f64>();
        let b = d[2];
        let den = aa - b * b;
        if den <= 1e-12 * aa {
            return Paint::NONE;
        }
        let dd = (0..3).map(|k| d[k] * o[k]).sum::<f64>();
        let e = o[2];
        let sc = (b * e - dd) / den; // along the ray
        let tc = (aa * e - b * dd) / den; // up the axis
        let persp = rays.proj.inv_d_eye > 0.0;
        if tc < 0.0 || (persp && sc <= rays.proj.z_near) {
            return Paint::NONE;
        }
        let gap = ((o[0] + d[0] * sc).powi(2) + (o[1] + d[1] * sc).powi(2) + (o[2] + d[2] * sc - tc).powi(2)).sqrt();
        let per_point = if persp { sc * rays.proj.inv_d_eye / rays.scale } else { 1.0 / rays.scale };
        Paint::of(look.axis_z, Line { width: FLOOR_AXIS_Z * rays.ppp }.covers(gap / per_point * rays.ppp) * look.fade(FLOOR_Z_REACH).at(tc))
    }
}
