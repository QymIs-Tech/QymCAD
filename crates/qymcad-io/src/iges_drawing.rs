//! IGES AS A DRAWING: the curves of a flat drawing, read into sketch curves.
//!
//! Exact geometry - surfaces and solids - is the kernel's to read (`qymcad_kernel::import_exact`). A drawing is
//! not: it is lines, arcs and polylines, often wrapped in subfigures defined once and placed many times, and the
//! kernel's reader passes over the network subfigures and arrays such files are built from. Measured on a
//! reported file (a chip library cell): the kernel gave one 10 x 5 rectangle out of a cell of fourteen polylines
//! and eight contacts. So the drawing side of IGES is read here, the way DXF is.
use std::collections::{HashMap, HashSet};

use qymcad_core::geom::bspline::{Approximated, BSpline};
use qymcad_core::geom::{Point2, ProfEdge};

/// What a drawing gave: the curves, and what was passed over.
#[derive(Debug, Default)]
pub struct IgesDrawing {
    pub curves: Vec<ProfEdge>,
    /// Entities of kinds that are not drawn (a note, a dimension, a surface), as (IGES type, how many).
    pub skipped: Vec<(u32, usize)>,
    /// The file places nothing and only defines subfigures - a library - so the definitions are what is shown.
    pub definitions_only: bool,
}

/// Read the curves of an IGES drawing. The coordinates come out in millimetres whatever unit the file names.
pub fn read_iges_drawing(path: &str) -> Result<IgesDrawing, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("io-iges-read-failed#{e}"))?;
    let text = String::from_utf8_lossy(&bytes);
    let file = File::parse(&text).ok_or_else(|| "io-iges-not-iges".to_string())?;
    let drawing = file.draw();
    if drawing.curves.is_empty() {
        return Err("io-iges-no-curves".into());
    }
    Ok(drawing)
}

/// One entity: its directory entry and its parameters.
struct Entity {
    typ: u32,
    form: u32,
    /// The directory entry of a transformation (type 124), or 0.
    matrix: usize,
    blanked: bool,
    subordinate: Subordinate,
    annotation: bool,
    params: Vec<String>,
}

/// The subordinate entity switch of a directory entry, digits 3-4 of its status (IGES 5.3, 2.2.4.4.9.2).
enum Subordinate {
    /// 00: referred to by nothing, drawn on its own
    Independent,
    /// 01: exists only through the entity that refers to it - a member of a subfigure, a piece of a composite curve
    Physically,
    /// 02: exists on its own and is gathered by a grouping entity such as a 402, whose matrix does not move it
    Logically,
    /// 03: both
    Both,
}

impl Subordinate {
    fn of(digits: &str) -> Subordinate {
        match digits {
            "01" => Subordinate::Physically,
            "02" => Subordinate::Logically,
            "03" => Subordinate::Both,
            _ => Subordinate::Independent,
        }
    }
}

struct File {
    entities: HashMap<usize, Entity>,
    /// Directory entries in file order, for a stable order of the curves.
    order: Vec<usize>,
    to_mm: f64,
}

/// A 3x4 affine transform, row-major.
type M = [f64; 12];
const ONE: M = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];

fn mul(a: &M, b: &M) -> M {
    let mut r = [0.0; 12];
    for i in 0..3 {
        for j in 0..4 {
            let mut s = if j == 3 { a[i * 4 + 3] } else { 0.0 };
            for k in 0..3 {
                s += a[i * 4 + k] * b[k * 4 + j];
            }
            r[i * 4 + j] = s;
        }
    }
    r
}

fn apply(m: &M, p: [f64; 3]) -> [f64; 3] {
    [0, 1, 2].map(|i| m[i * 4] * p[0] + m[i * 4 + 1] * p[1] + m[i * 4 + 2] * p[2] + m[i * 4 + 3])
}

/// Where a subfigure instance (408, 420, or one position of a 412) puts what it places: its X, Y, Z and its scales.
struct Instance {
    at: [f64; 3],
    scale: [f64; 3],
}

/// Split a parameter record into fields. A Hollerith string (`5HPADIN`) is one field whatever it holds, since
/// the delimiters may appear inside it.
fn fields(text: &str, pd: char, rd: char) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    loop {
        while i < chars.len() && chars[i] == ' ' {
            i += 1;
        }
        let start = i;
        while i < chars.len() && chars[i].is_ascii_digit() {
            i += 1;
        }
        let field = if i > start && i < chars.len() && chars[i] == 'H' {
            let n: usize = chars[start..i].iter().collect::<String>().parse().unwrap_or(0);
            let from = i + 1;
            let to = (from + n).min(chars.len());
            i = to;
            chars[from..to].iter().collect::<String>()
        } else {
            i = start;
            while i < chars.len() && chars[i] != pd && chars[i] != rd {
                i += 1;
            }
            chars[start..i].iter().collect::<String>().trim().to_string()
        };
        out.push(field);
        while i < chars.len() && chars[i] == ' ' {
            i += 1;
        }
        if i >= chars.len() || chars[i] == rd {
            break;
        }
        i += 1; // the parameter delimiter
    }
    out
}

fn num(s: &str) -> f64 {
    s.trim().replace(['D', 'd'], "E").parse().unwrap_or(0.0)
}

fn int(s: &str) -> i64 {
    let t = s.trim();
    t.parse::<i64>().unwrap_or_else(|_| num(t) as i64)
}

/// The length of the unit the file names, in millimetres (IGES global parameters 14 and 15).
fn unit_mm(flag: i64, name: &str) -> f64 {
    match flag {
        1 => 25.4,
        4 => 304.8,
        5 => 1_609_344.0,
        6 => 1000.0,
        7 => 1_000_000.0,
        8 => 0.0254,
        9 => 0.001,
        10 => 10.0,
        11 => 0.000_025_4,
        3 => match name.trim().to_ascii_uppercase().as_str() {
            "IN" | "INCH" => 25.4,
            "FT" => 304.8,
            "M" => 1000.0,
            "CM" => 10.0,
            "UM" => 0.001,
            _ => 1.0,
        },
        _ => 1.0,
    }
}

impl File {
    fn parse(text: &str) -> Option<File> {
        let mut g = String::new();
        let mut d: Vec<&str> = Vec::new();
        let mut p: HashMap<usize, String> = HashMap::new();
        for line in text.lines() {
            let line = line.trim_end_matches('\r');
            if line.len() < 73 || !line.is_char_boundary(72) || !line.is_char_boundary(73) {
                continue;
            }
            match &line[72..73] {
                "G" => g.push_str(&line[..72]),
                "D" => d.push(line),
                "P" => {
                    let de: usize = line.get(64..72).map(|s| s.trim().parse().unwrap_or(0)).unwrap_or(0);
                    p.entry(de).or_default().push_str(&line[..64.min(line.len())]);
                }
                _ => {}
            }
        }
        if d.is_empty() || !d.len().is_multiple_of(2) {
            return None;
        }
        // the delimiters: a global section may redefine them in its first two fields
        let (mut pd, mut rd) = (',', ';');
        if let Some(rest) = g.trim_start().strip_prefix("1H") {
            pd = rest.chars().next().unwrap_or(',');
        }
        let head: Vec<String> = fields(&g, pd, char::from(0));
        if head.len() > 1 {
            rd = head[1].chars().next().unwrap_or(';');
        }
        let head = fields(&g, pd, rd);
        let to_mm = unit_mm(head.get(13).map(|s| int(s)).unwrap_or(2), head.get(14).map(String::as_str).unwrap_or(""));
        let field = |l: &str, k: usize| -> String { l.get(k * 8..k * 8 + 8).unwrap_or("").to_string() };
        let mut entities = HashMap::new();
        let mut order = Vec::new();
        for pair in d.chunks(2) {
            let (a, b) = (pair[0], pair[1]);
            let de: usize = a.get(73..).map(|s| s.trim().parse().unwrap_or(0)).unwrap_or(0);
            let status = format!("{:0>8}", field(a, 8).trim());
            let params = p.get(&de).map(|t| fields(t, pd, rd)).unwrap_or_default();
            entities.insert(
                de,
                Entity {
                    typ: int(&field(a, 0)) as u32,
                    form: int(&field(b, 4)) as u32,
                    matrix: int(&field(a, 6)).max(0) as usize,
                    blanked: &status[0..2] == "01",
                    subordinate: Subordinate::of(&status[2..4]),
                    annotation: &status[4..6] == "01",
                    params,
                },
            );
            order.push(de);
        }
        Some(File { entities, order, to_mm })
    }

    /// The transform an entity's own directory entry carries, chained through transforms of transforms.
    fn own_matrix(&self, de: usize, depth: usize) -> M {
        let Some(e) = self.entities.get(&de) else { return ONE };
        if e.matrix == 0 || depth > 16 {
            return ONE;
        }
        let Some(t) = self.entities.get(&e.matrix).filter(|t| t.typ == 124) else { return ONE };
        let v: Vec<f64> = t.params.iter().skip(1).take(12).map(|s| num(s)).collect();
        if v.len() < 12 {
            return ONE;
        }
        let mine: M = [v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7], v[8], v[9], v[10], v[11]];
        mul(&self.own_matrix(e.matrix, depth + 1), &mine)
    }

    /// The transform of instance `de` under `parent`: its scales first, then its own matrix, then its X, Y, Z, which
    /// its own matrix does not move (IGES 5.3, 3.6.2). The matrix carries the turn of a placed symbol; applied after
    /// the X, Y, Z it swung the position about the origin as well - a quarter turn took (100,0) to (0,100).
    fn instance(&self, de: usize, parent: &M, inst: &Instance) -> M {
        let [x, y, z] = inst.at;
        let [sx, sy, sz] = inst.scale;
        let shift = [1.0, 0.0, 0.0, x, 0.0, 1.0, 0.0, y, 0.0, 0.0, 1.0, z];
        let scale = [sx, 0.0, 0.0, 0.0, 0.0, sy, 0.0, 0.0, 0.0, 0.0, sz, 0.0];
        mul(parent, &mul(&shift, &mul(&self.own_matrix(de, 0), &scale)))
    }

    fn draw(&self) -> IgesDrawing {
        let mut out = Pen { to_mm: self.to_mm, curves: Vec::new(), skipped: HashMap::new() };
        // WHAT IS DRAWN AT THE TOP: visible, not an annotation, and independent - or a member of a group that no
        // subfigure, instance or composite curve places, since a group only gathers what exists on its own. Reported
        // behaviour: two lines of a 402 did not come in, and a drawing of grouped lines only was refused as empty.
        let placed: HashSet<usize> = self.order.iter().flat_map(|de| self.references(*de)).collect();
        let on_its_own = |de: &usize| match self.entities[de].subordinate {
            Subordinate::Independent => true,
            Subordinate::Logically => !placed.contains(de),
            Subordinate::Physically | Subordinate::Both => false,
        };
        let tops: Vec<usize> = self.order.iter().copied().filter(|de| on_its_own(de) && !self.entities[de].blanked).collect();
        let mut drawn_any = false;
        for &de in &tops {
            let e = &self.entities[&de];
            if e.annotation {
                *out.skipped.entry(e.typ).or_insert(0) += 1;
                continue;
            }
            if !matches!(e.typ, 308 | 320 | 124 | 402 | 406) {
                drawn_any |= self.entity(de, &ONE, &mut out, 0);
            }
        }
        // A LIBRARY: nothing is placed, only defined. What is shown then is every definition nobody else uses -
        // the cells, not the parts they are made of.
        let mut definitions_only = false;
        if !drawn_any && out.curves.is_empty() {
            for &de in &self.order {
                let e = &self.entities[&de];
                if matches!(e.typ, 308 | 320) && !placed.contains(&de) {
                    definitions_only |= self.entity(de, &ONE, &mut out, 0);
                }
            }
        }
        let mut skipped: Vec<(u32, usize)> = out.skipped.into_iter().collect();
        skipped.sort();
        IgesDrawing { curves: out.curves, skipped, definitions_only }
    }

    /// The entities a subfigure or an instance refers to.
    fn references(&self, de: usize) -> Vec<usize> {
        let Some(e) = self.entities.get(&de) else { return Vec::new() };
        let at = |k: usize| e.params.get(k).map(|s| int(s).max(0) as usize).unwrap_or(0);
        match e.typ {
            308 | 320 => (0..at(3)).map(|k| at(4 + k)).collect(),
            102 => (0..at(1)).map(|k| at(2 + k)).collect(),
            408 | 420 | 412 => vec![at(1)],
            _ => Vec::new(),
        }
    }

    /// Draw entity `de` under the transform `m`. Returns whether it drew anything.
    fn entity(&self, de: usize, parent: &M, pen: &mut Pen, depth: usize) -> bool {
        let Some(e) = self.entities.get(&de) else { return false };
        if depth > 32 {
            return false; // a subfigure that contains itself
        }
        let m = mul(parent, &self.own_matrix(de, 0));
        let v = |k: usize| e.params.get(k).map(|s| num(s)).unwrap_or(0.0);
        let or = |k: usize, default: f64| if e.params.get(k).is_some_and(|x| !x.trim().is_empty()) { v(k) } else { default };
        let before = pen.curves.len();
        match e.typ {
            110 => pen.line(&m, [v(1), v(2), v(3)], [v(4), v(5), v(6)]),
            100 => pen.arc(&m, v(1), [v(2), v(3)], [v(4), v(5)], [v(6), v(7)]),
            106 => {
                let (ip, n) = (int(&e.params.get(1).cloned().unwrap_or_default()), int(&e.params.get(2).cloned().unwrap_or_default()).max(0) as usize);
                let pts: Vec<[f64; 3]> = match (ip, e.form) {
                    (1, 11 | 63) => (0..n).map(|k| [v(4 + 2 * k), v(5 + 2 * k), v(3)]).collect(),
                    (2, 12) => (0..n).map(|k| [v(3 + 3 * k), v(4 + 3 * k), v(5 + 3 * k)]).collect(),
                    (3, 13) => (0..n).map(|k| [v(3 + 6 * k), v(4 + 6 * k), v(5 + 6 * k)]).collect(),
                    _ => {
                        *pen.skipped.entry(106).or_insert(0) += 1; // points, or a witness line of a dimension
                        return false;
                    }
                };
                for w in pts.windows(2) {
                    pen.line(&m, w[0], w[1]);
                }
                if e.form == 63 && pts.len() > 2 && pts.first() != pts.last() {
                    pen.line(&m, pts[pts.len() - 1], pts[0]);
                }
            }
            126 => pen.spline(&m, &e.params),
            102 => {
                for c in self.references(de) {
                    self.entity(c, &m, pen, depth + 1);
                }
            }
            308 | 320 => {
                for c in self.references(de) {
                    self.entity(c, &m, pen, depth + 1);
                }
            }
            408 => {
                // DE, X, Y, Z, the scale S (1 when not given)
                let s = or(5, 1.0);
                let at = self.instance(de, parent, &Instance { at: [v(2), v(3), v(4)], scale: [s, s, s] });
                self.entity(int(&e.params[1]).max(0) as usize, &at, pen, depth + 1);
            }
            420 => {
                // DE, X, Y, Z, the scales XS (1 when not given), YS and ZS (XS when not given)
                let xs = or(5, 1.0);
                let at = self.instance(de, parent, &Instance { at: [v(2), v(3), v(4)], scale: [xs, or(6, xs), or(7, xs)] });
                self.entity(int(&e.params[1]).max(0) as usize, &at, pen, depth + 1);
            }
            412 => {
                // base, scale, lower-left corner, columns, rows, column and row spacing, rotation
                let s = or(2, 1.0);
                let (cols, rows) = (v(6).max(1.0) as usize, v(7).max(1.0) as usize);
                let (dc, dr, a) = (v(8), v(9), v(10));
                let base = int(&e.params[1]).max(0) as usize;
                // 11 the count of the DO/DON'T list (0: every position), 12 DO (0) or DON'T (1), 13 on the positions,
                // numbered from 1 at the lower left up each column, then up the next (IGES 5.3, 4.136)
                let at = |k: usize| e.params.get(k).map(|s| int(s)).unwrap_or(0);
                let listed: HashSet<i64> = (0..(at(11).max(0) as usize).min(e.params.len())).map(|k| at(13 + k)).collect();
                let dont = at(12) != 0;
                for r in 0..rows {
                    for c in 0..cols {
                        if !listed.is_empty() && listed.contains(&((c * rows + r + 1) as i64)) == dont {
                            continue;
                        }
                        let (lx, ly) = (c as f64 * dc, r as f64 * dr);
                        let (x, y) = (v(3) + lx * a.cos() - ly * a.sin(), v(4) + lx * a.sin() + ly * a.cos());
                        self.entity(base, &self.instance(de, parent, &Instance { at: [x, y, v(5)], scale: [s, s, s] }), pen, depth + 1);
                    }
                }
            }
            124 | 402 | 406 | 132 => {}
            other => *pen.skipped.entry(other).or_insert(0) += 1,
        }
        pen.curves.len() > before
    }
}

/// Where the curves go, in millimetres, flattened onto XY.
struct Pen {
    to_mm: f64,
    curves: Vec<ProfEdge>,
    skipped: HashMap<u32, usize>,
}

impl Pen {
    fn at(&self, m: &M, p: [f64; 3]) -> Point2 {
        let q = apply(m, p);
        Point2::new(q[0] * self.to_mm, q[1] * self.to_mm)
    }

    fn line(&mut self, m: &M, a: [f64; 3], b: [f64; 3]) {
        let (a, b) = (self.at(m, a), self.at(m, b));
        if (a.x - b.x).hypot(a.y - b.y) > 1e-12 {
            self.curves.push(ProfEdge::Line { a, b });
        }
    }

    /// An arc of entity 100: counter-clockwise from `start` to `end` about `centre`, at height `zt`, a full
    /// circle when the two ends meet. A transform that keeps shapes (a rotation, a uniform scale, a mirror) keeps
    /// it an arc; any other one - a stretch, a tilt out of the drawing plane - turns it into a polyline.
    fn arc(&mut self, m: &M, zt: f64, centre: [f64; 2], start: [f64; 2], end: [f64; 2]) {
        let (a0, a1, b0, b1) = (m[0], m[1], m[4], m[5]);
        let det = a0 * b1 - a1 * b0;
        let tol = 1e-9 * (1.0 + a0.abs() + a1.abs());
        let in_the_drawing_plane = m[8].abs() < 1e-9 && m[9].abs() < 1e-9;
        let turns_and_scales = (a0 - b1).abs() + (a1 + b0).abs() < tol;
        let mirrors = (a0 + b1).abs() + (a1 - b0).abs() < tol;
        let keeps = in_the_drawing_plane && (turns_and_scales || mirrors);
        let r = (start[0] - centre[0]).hypot(start[1] - centre[1]);
        let full = (start[0] - end[0]).hypot(start[1] - end[1]) < 1e-9 * (1.0 + r);
        let c3 = [centre[0], centre[1], zt];
        if keeps {
            let c = self.at(m, c3);
            if full {
                self.curves.push(ProfEdge::Circle { center: c, r: r * det.abs().sqrt() * self.to_mm });
            } else {
                let (a, b) = (self.at(m, [start[0], start[1], zt]), self.at(m, [end[0], end[1], zt]));
                self.curves.push(ProfEdge::Arc { a, b, center: c, ccw: det > 0.0 });
            }
            return;
        }
        let t0 = (start[1] - centre[1]).atan2(start[0] - centre[0]);
        let mut t1 = (end[1] - centre[1]).atan2(end[0] - centre[0]);
        if full || t1 <= t0 {
            t1 += std::f64::consts::TAU;
        }
        let n = 32;
        let pts: Vec<[f64; 3]> = (0..=n)
            .map(|k| {
                let t = t0 + (t1 - t0) * k as f64 / n as f64;
                [centre[0] + r * t.cos(), centre[1] + r * t.sin(), zt]
            })
            .collect();
        for w in pts.windows(2) {
            self.line(m, w[0], w[1]);
        }
    }

    /// A rational B-spline curve (entity 126), placed and approximated by the cubic B-spline of the sketch within
    /// 0.001 mm (`BSpline::approximate`), its knots begun at the file's: laid out as 16 segments a span, a curve of the
    /// drawing was a run of lines a person could not drag, and a body had a face for each.
    fn spline(&mut self, m: &M, p: &[String]) {
        let at = |k: usize| p.get(k).map(|s| num(s)).unwrap_or(0.0);
        let (k, deg) = (at(1) as usize, at(2) as usize);
        let nk = k + deg + 2;
        let knots: Vec<f64> = (0..nk).map(|i| at(7 + i)).collect();
        let w: Vec<f64> = (0..=k).map(|i| at(7 + nk + i)).collect();
        let cp: Vec<[f64; 3]> = (0..=k).map(|i| [at(8 + nk + k + 3 * i), at(9 + nk + k + 3 * i), at(10 + nk + k + 3 * i)]).collect();
        let (v0, v1) = (at(8 + nk + k + 3 * (k + 1)), at(9 + nk + k + 3 * (k + 1)));
        if knots.len() != nk || v1 <= v0 {
            return;
        }
        let eval = |t: f64| -> [f64; 3] {
            // de Boor on homogeneous points
            let mut span = deg;
            while span < k && knots[span + 1] <= t {
                span += 1;
            }
            let mut dp: Vec<[f64; 4]> = (0..=deg)
                .map(|j| {
                    let i = span - deg + j;
                    let ww = if w[i] == 0.0 { 1.0 } else { w[i] };
                    [cp[i][0] * ww, cp[i][1] * ww, cp[i][2] * ww, ww]
                })
                .collect();
            for r in 1..=deg {
                for j in (r..=deg).rev() {
                    let i = span - deg + j;
                    let den = knots[i + deg + 1 - r] - knots[i];
                    let a = if den.abs() < 1e-15 { 0.0 } else { (t - knots[i]) / den };
                    let prev = dp[j - 1];
                    for (v, p) in dp[j].iter_mut().zip(prev) {
                        *v = (1.0 - a) * p + a * *v;
                    }
                }
            }
            let h = dp[deg];
            [h[0] / h[3], h[1] / h[3], h[2] / h[3]]
        };
        let along = |u: f64| self.at(m, eval(v0 + (v1 - v0) * u));
        let bends: Vec<f64> = knots[deg + 1..=k].iter().map(|x| (x - v0) / (v1 - v0)).collect();
        let off = |q: Point2, u: f64| q.dist(along(u));
        let curve = Approximated { at: &along, bends: &bends, ends: qymcad_core::feature::Ends::Open, off: &off, tolerance: 1e-3, through: None };
        match BSpline::approximate(&curve) {
            Some(b) => self.curves.push(ProfEdge::Spline(b)),
            None => {
                let n = 16 * (k + 1 - deg).max(1);
                let pts: Vec<[f64; 3]> = (0..=n).map(|i| eval(v0 + (v1 - v0) * i as f64 / n as f64)).collect();
                for s in pts.windows(2) {
                    self.line(m, s[0], s[1]);
                }
            }
        }
    }
}
