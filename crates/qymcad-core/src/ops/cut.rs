//! Low-level helpers for cutting contours: offsets by side, and the emission of profile passes with ramped
//! entry and tabs.

use super::params::{Feeds, Heights, Passes, Ramp, Side, Tabs};
use crate::geom::{Contour, Point2, Point3};
use crate::ir::{Move, Toolpath};
use crate::offset::offset_to_side;

/// The paths to machine, derived from the contours by side and cutter radius, plus the stock allowance.
pub fn contour_paths(contours: &[Contour], side: Side, radius: f64) -> Vec<Contour> {
    let mut out = Vec::new();
    for c in contours {
        match side {
            Side::On => out.push(c.clone()),
            Side::Outside => out.extend(offset_to_side(c, radius, true)),
            Side::Inside => out.extend(offset_to_side(c, radius, false)),
        }
    }
    out
}

/// Emit the profile passes: a ramped descent, or a vertical plunge, over several Z passes, with tabs.
pub fn emit_profile(
    tp: &mut Toolpath,
    paths: &[Contour],
    heights: &Heights,
    passes: &Passes,
    feeds: &Feeds,
    tabs: Option<Tabs>,
    ramp: Ramp,
) {
    for path in paths {
        if path.points.len() < 2 {
            continue;
        }
        let start = path.points[0];
        let levels = heights.z_levels(passes.stepdown);
        let use_ramp = ramp.enabled && path.closed && ramp.angle_deg > 0.0 && path.length() > 1e-6;

        tp.push(Move::Rapid { to: Point3::new(start.x, start.y, heights.retract) });

        if use_ramp {
            // descend to the top of the material through air, then spiral down without retracting
            tp.push(Move::Rapid { to: Point3::new(start.x, start.y, heights.top) });
            let tan = ramp.angle_deg.to_radians().tan().max(1e-3);
            let mut z_prev = heights.top;
            for &z in &levels {
                let need = (z_prev - z) / tan;
                let ramp_len = need.min(path.length() * 0.5).max(0.0);
                let pts = profile_z_path(path, z_prev, z, ramp_len, heights.bottom, tabs);
                for p in &pts {
                    tp.push(Move::Linear { to: *p, feed: feeds.cut });
                }
                z_prev = z;
            }
        } else {
            for &z in &levels {
                let pts = profile_z_path(path, z, z, 0.0, heights.bottom, tabs);
                tp.push(Move::Rapid { to: Point3::new(start.x, start.y, heights.retract) });
                tp.push(Move::Plunge { to: pts[0], feed: feeds.plunge });
                for p in pts.iter().skip(1) {
                    tp.push(Move::Linear { to: *p, feed: feeds.cut });
                }
            }
        }

        tp.push(Move::Rapid { to: Point3::new(start.x, start.y, heights.retract) });
    }
}

/// The path of one pass along a contour with its Z profile: a ramped descent from `z_from` to `z_to` over the
/// first `ramp_len` mm, then `z_to`; over a tab the Z rises to `bottom + height`.
fn profile_z_path(path: &Contour, z_from: f64, z_to: f64, ramp_len: f64, bottom: f64, tabs: Option<Tabs>) -> Vec<Point3> {
    let mut pts = path.points.clone();
    if path.closed {
        pts.push(pts[0]);
    }

    let mut cum = vec![0.0];
    for i in 1..pts.len() {
        cum.push(cum[i - 1] + pts[i - 1].dist(pts[i]));
    }
    let perim = *cum.last().unwrap();

    // tab parameters, used only when the pass reaches their zone
    let (tab_on, tab_top, half, centers): (bool, f64, f64, Vec<f64>) = match tabs {
        Some(t) if t.enabled && t.count > 0 && path.closed && perim > 1e-6 && z_to < bottom + t.height - 1e-6 => {
            let tt = bottom + t.height;
            let h = (t.width / 2.0).min(perim / (2.0 * t.count as f64));
            let c = (0..t.count).map(|k| k as f64 * perim / t.count as f64).collect();
            (true, tt, h, c)
        }
        _ => (false, 0.0, 0.0, Vec::new()),
    };

    let z_at = |s: f64| -> f64 {
        let rz = if ramp_len > 1e-6 {
            z_from - (z_from - z_to) * (s / ramp_len).min(1.0)
        } else {
            z_to
        };
        if tab_on {
            for &c in &centers {
                let mut d = (s - c).abs();
                d = d.min(perim - d);
                if d <= half {
                    return rz.max(tab_top);
                }
            }
        }
        rz
    };
    let lerp = |a: Point2, b: Point2, f: f64| Point2::new(a.x + (b.x - a.x) * f, a.y + (b.y - a.y) * f);

    // the points where a vertex has to be inserted: the end of the ramp and the boundaries of the tabs
    let mut feats: Vec<f64> = Vec::new();
    if ramp_len > 1e-6 && ramp_len < perim {
        feats.push(ramp_len);
    }
    if tab_on {
        for &c in &centers {
            for e in [c - half, c + half] {
                for sh in [-perim, 0.0, perim] {
                    feats.push(e + sh);
                }
            }
        }
    }

    let mut out: Vec<Point3> = Vec::new();
    for i in 0..pts.len() - 1 {
        let (s0, s1) = (cum[i], cum[i + 1]);
        out.push(Point3::at(pts[i], z_at(s0)));
        let seglen = s1 - s0;
        if seglen <= 1e-9 {
            continue;
        }
        let mut cs: Vec<f64> = feats.iter().copied().filter(|&e| e > s0 + 1e-9 && e < s1 - 1e-9).collect();
        cs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        for e in cs {
            out.push(Point3::at(lerp(pts[i], pts[i + 1], (e - s0) / seglen), z_at(e)));
        }
    }
    out.push(Point3::at(*pts.last().unwrap(), z_at(*cum.last().unwrap())));
    out
}
