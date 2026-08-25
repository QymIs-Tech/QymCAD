//! `qymcad-verify`: lightweight verification of G-code.
//!
//! It re-parses the finished output of any post-processor and computes the bounds of the toolpath, an estimate
//! of the time from the feed rates and rapids, and any excursion beyond the limits of the table. It is
//! independent of the controller and works for any post-processor.

/// Verification options.
#[derive(Clone, Copy, Debug)]
pub struct VerifyOptions {
    /// The rapid speed in mm per minute, used to estimate the time.
    pub rapid_rate: f64,
    /// The limits of the table for the excursion check, as a minimum and a maximum in XYZ. `None` skips the
    /// check.
    pub limits: Option<([f64; 3], [f64; 3])>,
}

impl Default for VerifyOptions {
    fn default() -> Self {
        Self { rapid_rate: 5000.0, limits: None }
    }
}

/// The result of a run over the G-code.
#[derive(Clone, Debug)]
pub struct VerifyResult {
    pub seconds: f64,
    pub bounds_min: [f64; 3],
    pub bounds_max: [f64; 3],
    /// The length of the cutting path, in mm.
    pub cut_length: f64,
    /// The length of the rapids, in mm.
    pub rapid_length: f64,
    pub errors: Vec<String>,
}

impl Default for VerifyResult {
    fn default() -> Self {
        Self {
            seconds: 0.0,
            bounds_min: [f64::INFINITY; 3],
            bounds_max: [f64::NEG_INFINITY; 3],
            cut_length: 0.0,
            rapid_length: 0.0,
            errors: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Words {
    g: Option<i32>,
    x: Option<f64>,
    y: Option<f64>,
    z: Option<f64>,
    i: Option<f64>,
    j: Option<f64>,
    f: Option<f64>,
}

/// Run over the G-code: bounds, lengths, an estimate of the time and a check against the limits.
pub fn verify_gcode(gcode: &str, opts: &VerifyOptions) -> VerifyResult {
    let mut r = VerifyResult::default();
    let (mut x, mut y, mut z) = (0.0_f64, 0.0_f64, 0.0_f64);
    let mut feed = 0.0_f64;
    let mut motion: i32 = 0; // the modal motion mode
    let mut seen_any = false;

    for raw in gcode.lines() {
        let line = strip_comment(raw);
        if line.trim().is_empty() {
            continue;
        }
        let w = parse_words(&line);
        if let Some(g) = w.g {
            if (0..=3).contains(&g) {
                motion = g;
            }
        }
        if let Some(f) = w.f {
            feed = f;
        }

        let has_xyz = w.x.is_some() || w.y.is_some() || w.z.is_some();
        if !has_xyz {
            continue;
        }

        let (nx, ny, nz) = (w.x.unwrap_or(x), w.y.unwrap_or(y), w.z.unwrap_or(z));

        // update the bounds with the end point
        if !seen_any {
            // the first point sets the start and counts too
            update_bounds(&mut r, x, y, z);
            seen_any = true;
        }
        update_bounds(&mut r, nx, ny, nz);

        let dist = match motion {
            2 | 3 => arc_len(x, y, w.i.unwrap_or(0.0), w.j.unwrap_or(0.0), nx, ny, motion == 2)
                + (nz - z).abs(),
            _ => ((nx - x).powi(2) + (ny - y).powi(2) + (nz - z).powi(2)).sqrt(),
        };

        let rate = if motion == 0 { opts.rapid_rate } else { feed };
        if rate > 0.0 {
            r.seconds += dist / rate * 60.0;
        }
        if motion == 0 {
            r.rapid_length += dist;
        } else {
            r.cut_length += dist;
        }

        x = nx;
        y = ny;
        z = nz;
    }

    if !seen_any {
        r.bounds_min = [0.0; 3];
        r.bounds_max = [0.0; 3];
    }

    if let Some((lo, hi)) = opts.limits {
        let (mn, mx) = (r.bounds_min, r.bounds_max);
        for (k, axis) in ["X", "Y", "Z"].iter().enumerate() {
            if mn[k] < lo[k] - 1e-6 || mx[k] > hi[k] + 1e-6 {
                r.errors.push(format!(
                    "verify-axis-out-of-table#{axis}: [{:.3}..{:.3}] / [{:.3}..{:.3}]",
                    mn[k], mx[k], lo[k], hi[k]
                ));
            }
        }
    }

    r
}

fn update_bounds(r: &mut VerifyResult, x: f64, y: f64, z: f64) {
    for (k, v) in [x, y, z].iter().enumerate() {
        r.bounds_min[k] = r.bounds_min[k].min(*v);
        r.bounds_max[k] = r.bounds_max[k].max(*v);
    }
}

/// The length of an arc given its centre, where I and J are the offset of the centre from the start; `cw`
/// selects a clockwise arc.
fn arc_len(sx: f64, sy: f64, i: f64, j: f64, ex: f64, ey: f64, cw: bool) -> f64 {
    let (cx, cy) = (sx + i, sy + j);
    let radius = (i * i + j * j).sqrt();
    if radius < 1e-9 {
        return ((ex - sx).powi(2) + (ey - sy).powi(2)).sqrt();
    }
    let a0 = (sy - cy).atan2(sx - cx);
    let a1 = (ey - cy).atan2(ex - cx);
    let mut sweep = a1 - a0;
    use std::f64::consts::TAU;
    if cw {
        while sweep >= 0.0 {
            sweep -= TAU;
        }
    } else {
        while sweep <= 0.0 {
            sweep += TAU;
        }
    }
    radius * sweep.abs()
}

/// Strip the comments: `( ... )` and `; ...`.
fn strip_comment(line: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for c in line.chars() {
        match c {
            '(' => depth += 1,
            ')' => {
                if depth > 0 {
                    depth -= 1
                }
            }
            ';' if depth == 0 => break,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

/// Parse the words of a block: a letter followed by a number.
fn parse_words(line: &str) -> Words {
    let mut w = Words::default();
    let bytes: Vec<char> = line.chars().collect();
    let mut k = 0;
    while k < bytes.len() {
        let c = bytes[k].to_ascii_uppercase();
        if c.is_ascii_alphabetic() {
            // collect the number after the letter
            let start = k + 1;
            let mut e = start;
            while e < bytes.len() {
                let d = bytes[e];
                if d.is_ascii_digit() || d == '.' || d == '-' || d == '+' {
                    e += 1;
                } else {
                    break;
                }
            }
            let num: String = bytes[start..e].iter().collect();
            match c {
                'G' => w.g = num.trim().parse::<f64>().ok().map(|v| v as i32),
                'X' => w.x = num.trim().parse().ok(),
                'Y' => w.y = num.trim().parse().ok(),
                'Z' => w.z = num.trim().parse().ok(),
                'I' => w.i = num.trim().parse().ok(),
                'J' => w.j = num.trim().parse().ok(),
                'F' => w.f = num.trim().parse().ok(),
                _ => {}
            }
            k = e;
        } else {
            k += 1;
        }
    }
    w
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn straight_moves_bounds_and_time() {
        // a rapid to (0,0,5), a cut to (100,0) at a feed of 1000, and back
        let g = "G21\nG0 X0 Y0 Z5\nG1 Z-1 F200\nG1 X100 F1000\nG1 X0\nG0 Z5\n";
        let r = verify_gcode(g, &VerifyOptions::default());
        assert!((r.bounds_max[0] - 100.0).abs() < 1e-6);
        assert!((r.bounds_min[2] + 1.0).abs() < 1e-6);
        // the cutting path: a plunge from Z5 to Z−1 of 6, plus 100 and 100, giving 206
        assert!((r.cut_length - 206.0).abs() < 1e-3, "cut={}", r.cut_length);
        assert!(r.seconds > 0.0);
        assert!(r.errors.is_empty());
    }

    #[test]
    fn detects_out_of_limits() {
        let g = "G0 X600 Y0\n";
        let opts = VerifyOptions {
            rapid_rate: 5000.0,
            limits: Some(([0.0, 0.0, -100.0], [500.0, 400.0, 0.0])),
        };
        let r = verify_gcode(g, &opts);
        assert!(!r.errors.is_empty(), "X600 has to violate a limit of 500");
    }

    #[test]
    fn arc_quarter_length() {
        // a quarter circle of r = 10 from (10,0) to (0,10) about (0,0): I = −10, J = 0, counter-clockwise
        let g = "G1 X10 Y0 F500\nG3 X0 Y10 I-10 J0\n";
        let r = verify_gcode(g, &VerifyOptions::default());
        let quarter = std::f64::consts::PI * 10.0 / 2.0; // ~15.708
        // the cut includes the first move of 10 plus the arc of about 15.708
        assert!((r.cut_length - (10.0 + quarter)).abs() < 0.1, "cut={}", r.cut_length);
    }
}
