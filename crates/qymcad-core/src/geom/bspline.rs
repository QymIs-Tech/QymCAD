//! A CUBIC B-SPLINE: its poles and its knots - the one form a spline of the sketch has, as pro-CAD keeps one and as the
//! kernel takes it. Degree 3, clamped - the curve starts at its first pole and ends at its last, each end knot four
//! times - its knots running from 0 to 1. A point and its derivatives are read by de Boor's rule; a piece is cut out by
//! inserting knots at its ends (Boehm) until the curve passes through them; a knot of a joint where the curve is as
//! smooth as the knot allows is taken away (Piegl and Tiller, The NURBS Book, A5.8), so a spline through nodes keeps one
//! knot a node and goes to the kernel as it is. A spline drawn through nodes is interpolated with continuous curvature
//! (`BSpline::through`); an offset is fitted to the true one by least squares (`BSpline::fit`); both systems are solved
//! by the sparse solver of the core (`linalg`).
use crate::feature::Ends;
use crate::geom::Point2;
use crate::model::Handle;
use faer::sparse::Triplet;

/// The degree of every B-spline of a sketch.
pub const DEGREE: usize = 3;

/// How far a pole moved by the removal of a knot may stand from where the curve needs it: a knot is taken away only
/// where the curve stays the same within this, in millimetres.
const REMOVAL: f64 = 1e-9;

/// A CUBIC B-SPLINE: `poles.len() + 4` knots. Clamped, as every curve of the sketch is, the first four are 0 and the
/// last four 1.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BSpline {
    pub knots: Vec<f64>,
    pub poles: Vec<Point2>,
}

/// A POINT AND ITS FIRST TWO DERIVATIVES by the parameter.
#[derive(Clone, Copy, Debug)]
pub struct Derivs {
    pub at: Point2,
    pub velocity: Point2,
    pub bend: Point2,
}

/// A CUBIC SPAN BY ITS ENDS AND THE DERIVATIVES THERE, over the stretch `run` of the parameter of the B-spline it is a
/// span of: the derivatives by that parameter, so spans of one curve meet at its pace.
#[derive(Clone, Copy, Debug)]
pub struct Span {
    pub from: Point2,
    pub leave: Point2,
    pub to: Point2,
    pub arrive: Point2,
    pub run: [f64; 2],
}

/// THE TWO TANGENTS OF A NODE OF A SPLINE, as a spline entity keeps them in its handles: the derivative the span before
/// the node ends with and the span after it starts with, each span a unit of its own parameter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NodeTangents {
    pub arrive: Point2,
    pub leave: Point2,
}

/// A SPLINE AS A SKETCH HOLDS IT: its nodes and the tangents of each - one node a knot of the B-spline.
#[derive(Clone, Debug, PartialEq)]
pub struct Hermite {
    pub nodes: Vec<Point2>,
    pub tangents: Vec<NodeTangents>,
}

/// A POINT A CURVE IS FITTED TO, and the place of the curve it should stand at.
#[derive(Clone, Copy, Debug)]
pub struct Sample {
    pub u: f64,
    pub at: Point2,
}

/// Which side of a knot a derivative is read on.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Before,
    After,
}

impl BSpline {
    /// THE SPLINE THROUGH `nodes`, open or closed, as the sketch draws it: through every node with its curvature
    /// unbroken, the spans by the lengths of their chords, natural at the free ends of an open one; a node with a handle
    /// held by it (`node_tangents`). `None` for fewer than two nodes.
    pub fn through(nodes: &[Point2], handles: &[Handle], ends: Ends) -> Option<BSpline> {
        if nodes.len() < 2 {
            return None;
        }
        let tangents = node_tangents(nodes, handles, ends);
        BSpline::of_hermite(&Hermite { nodes: nodes.to_vec(), tangents }, ends)
    }

    /// THE B-SPLINE OF A SPLINE BY ITS NODES AND THEIR TANGENTS: each span a cubic from its node to the next, the knots
    /// giving each span the length of the parameter that keeps the pace of the curve the same on either side of a node -
    /// the derivative arrive / h at the end of one span is leave / h' at the start of the next - so the curve is as smooth
    /// in its parameter as it is in its shape. For a spline through points the lengths are those of the chords.
    pub fn of_hermite(h: &Hermite, ends: Ends) -> Option<BSpline> {
        let n = h.nodes.len();
        let count = if ends == Ends::Closed { n } else { n.saturating_sub(1) };
        if count == 0 {
            return None;
        }
        let mut knots = Vec::with_capacity(count + 1);
        knots.push(0.0);
        let (mut w, mut along) = (1.0, 0.0);
        for k in 0..count {
            if k > 0 {
                let (arrive, leave) = (h.tangents[k].arrive.len(), h.tangents[k].leave.len());
                if arrive > 1e-12 && leave > 1e-12 {
                    w *= leave / arrive;
                }
            }
            along += w;
            knots.push(along);
        }
        let spans: Vec<Span> = (0..count)
            .map(|k| {
                let w = knots[k + 1] - knots[k];
                let (i, j) = (k, (k + 1) % n);
                Span { from: h.nodes[i], leave: h.tangents[i].leave * (1.0 / w), to: h.nodes[j], arrive: h.tangents[j].arrive * (1.0 / w), run: [knots[k], knots[k + 1]] }
            })
            .collect();
        BSpline::of_spans(&spans)
    }

    /// THE CUBIC BEZIER OF FOUR POINTS AS A B-SPLINE: the same curve, its knots 0 four times and 1 four times.
    pub fn bezier(points: [Point2; 4]) -> BSpline {
        BSpline { knots: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0], poles: points.to_vec() }
    }

    /// THE B-SPLINE OF CUBIC SPANS THAT FOLLOW ON, each by its ends and its derivatives there: the spans laid as Bezier
    /// pieces - each inner knot three times - and every inner knot taken away as far as the curve allows, so a joint
    /// smooth to the curvature keeps one knot, a joint smooth to the tangent two, a corner three. `None` for no span.
    pub fn of_spans(spans: &[Span]) -> Option<BSpline> {
        let first = spans.first()?;
        let mut knots = vec![first.run[0]; 4];
        let mut poles = vec![first.from];
        for s in spans {
            let third = (s.run[1] - s.run[0]) / 3.0;
            poles.push(s.from + s.leave * third);
            poles.push(s.to - s.arrive * third);
            poles.push(s.to);
            knots.extend([s.run[1]; 3]);
        }
        knots.push(spans.last()?.run[1]);
        let mut b = BSpline { knots, poles };
        b.normalise();
        for u in b.inner_knots() {
            b.remove_all(u);
        }
        Some(b)
    }

    /// THE SPLINE AS A SKETCH HOLDS IT: a node at every knot, with the tangents of the spans on either side of it - the
    /// derivative there times the length of the parameter of the span - a closed one's last node its first again, left
    /// out. The B-spline of those (`of_hermite`) is this one again.
    pub fn hermite(&self, ends: Ends) -> Hermite {
        let mut at = vec![0.0];
        at.extend(self.inner_knots());
        at.push(1.0);
        let last = at.len() - 1;
        let mut nodes = Vec::with_capacity(at.len());
        let mut tangents = Vec::with_capacity(at.len());
        for (k, u) in at.iter().enumerate() {
            nodes.push(self.at(*u));
            let arrive = (k > 0).then(|| self.velocity_on(*u, Side::Before) * (u - at[k - 1]));
            let leave = (k < last).then(|| self.velocity_on(*u, Side::After) * (at[k + 1] - u));
            let (arrive, leave) = match (arrive, leave) {
                (Some(a), Some(l)) => (a, l),
                (Some(a), None) => (a, a),
                (None, Some(l)) => (l, l),
                (None, None) => (Point2::new(0.0, 0.0), Point2::new(0.0, 0.0)),
            };
            tangents.push(NodeTangents { arrive, leave });
        }
        if ends == Ends::Closed && nodes.len() > 2 {
            nodes.pop();
            let end = tangents.pop().map_or(Point2::new(0.0, 0.0), |t| t.arrive);
            tangents[0].arrive = end;
        }
        Hermite { nodes, tangents }
    }

    /// The knots scaled to run from 0 to 1.
    fn normalise(&mut self) {
        let (lo, hi) = (self.knots[DEGREE], self.knots[self.poles.len()]);
        let w = (hi - lo).max(1e-300);
        for k in &mut self.knots {
            *k = (*k - lo) / w;
        }
    }

    /// THE NUMBER OF SPANS between its knots.
    pub fn spans(&self) -> usize {
        self.inner_knots().len() + 1
    }

    /// THE INDEX OF THE KNOT SPAN holding `t`: the `i` with knots[i] <= t < knots[i + 1], the last span holding 1.
    pub fn span_of(&self, t: f64) -> usize {
        let n = self.poles.len();
        let t = t.clamp(0.0, 1.0);
        if t >= self.knots[n] {
            return n - 1;
        }
        (self.knots.partition_point(|k| *k <= t) - 1).clamp(DEGREE, n - 1)
    }

    /// THE POINT AND ITS DERIVATIVES at `t`, by the basis functions and their derivatives (The NURBS Book, A2.3).
    pub fn derivs(&self, t: f64) -> Derivs {
        let t = t.clamp(0.0, 1.0);
        self.derivs_in(self.span_of(t), t)
    }

    /// The point and its derivatives at `t` by the polynomial of knot span `i`.
    fn derivs_in(&self, i: usize, t: f64) -> Derivs {
        let d = basis_derivs(&self.knots, i, t);
        let mut out = [Point2::new(0.0, 0.0); 3];
        for (k, row) in d.iter().enumerate() {
            for (j, w) in row.iter().enumerate() {
                out[k] = out[k] + self.poles[i - DEGREE + j] * *w;
            }
        }
        Derivs { at: out[0], velocity: out[1], bend: out[2] }
    }

    /// The first derivative at knot `u`, by the span ending there or the span starting there.
    fn velocity_on(&self, u: f64, side: Side) -> Point2 {
        let n = self.poles.len();
        let i = match side {
            Side::Before => self.knots.partition_point(|k| *k < u).saturating_sub(1),
            Side::After => self.knots.partition_point(|k| *k <= u).saturating_sub(1),
        };
        self.derivs_in(i.clamp(DEGREE, n - 1), u).velocity
    }

    /// THE POINT at `t`, by de Boor's rule alone: the curve is laid out by millions of points, and reading the two
    /// derivatives with each tripled the work.
    pub fn at(&self, t: f64) -> Point2 {
        let t = t.clamp(0.0, 1.0);
        let i = self.span_of(t);
        let mut d: [Point2; DEGREE + 1] = std::array::from_fn(|j| self.poles[i - DEGREE + j]);
        for r in 1..=DEGREE {
            for j in (r..=DEGREE).rev() {
                let k = i - DEGREE + j;
                let den = self.knots[k + DEGREE + 1 - r] - self.knots[k];
                let a = if den.abs() < 1e-300 { 0.0 } else { (t - self.knots[k]) / den };
                d[j] = d[j - 1] * (1.0 - a) + d[j] * a;
            }
        }
        d[DEGREE]
    }

    /// The first derivative at `t`.
    pub fn velocity(&self, t: f64) -> Point2 {
        self.derivs(t).velocity
    }

    /// The knots strictly inside the curve, each once.
    pub fn inner_knots(&self) -> Vec<f64> {
        let mut out: Vec<f64> = Vec::new();
        for k in &self.knots[DEGREE + 1..self.poles.len()] {
            if out.last() != Some(k) {
                out.push(*k);
            }
        }
        out
    }

    /// How many times knot value `u` stands in the knots.
    fn multiplicity(&self, u: f64) -> usize {
        self.knots.iter().filter(|k| **k == u).count()
    }

    /// INSERT KNOT `u` until it stands `times` times more, three at the most, the curve unchanged (Boehm; The NURBS
    /// Book, A5.1).
    pub fn insert(&mut self, u: f64, times: usize) {
        for _ in 0..times {
            let k = self.knots.partition_point(|x| *x <= u) - 1;
            let s = self.multiplicity(u);
            if s >= DEGREE {
                return;
            }
            let mut poles = Vec::with_capacity(self.poles.len() + 1);
            poles.extend_from_slice(&self.poles[..=k - DEGREE]);
            for i in k - DEGREE + 1..=k - s {
                let a = (u - self.knots[i]) / (self.knots[i + DEGREE] - self.knots[i]);
                poles.push(self.poles[i - 1] * (1.0 - a) + self.poles[i] * a);
            }
            poles.extend_from_slice(&self.poles[k - s..]);
            self.poles = poles;
            self.knots.insert(k + 1, u);
        }
    }

    /// THE PIECE OF THE CURVE FROM `from` TO `to` (0 <= from < to <= 1), the same curve, clamped: each end inserted
    /// until it stands three times and the curve passes through a pole there, the poles between taken, the knots scaled
    /// to run from 0 to 1 again. Of an unclamped curve - a loop laid periodic - the piece from 0 to 1 is the curve
    /// clamped.
    pub fn piece(&self, [from, to]: [f64; 2]) -> BSpline {
        let mut b = self.clone();
        for u in [from, to] {
            let m = b.multiplicity(u);
            b.insert(u, DEGREE.saturating_sub(m));
        }
        // a knot standing three times from index i on has the curve pass through pole i - 1 there; a clamped end, four
        // times from index 0, through pole 0
        let first = |u: f64| b.knots.iter().position(|k| *k == u).unwrap_or(0);
        let (start, end) = (first(from).saturating_sub(1), first(to) - 1);
        let poles: Vec<Point2> = b.poles[start..=end].to_vec();
        let mut knots = vec![from; DEGREE + 1];
        knots.extend(b.knots.iter().copied().filter(|k| *k > from && *k < to));
        knots.extend([to; DEGREE + 1]);
        let mut out = BSpline { knots, poles };
        out.normalise();
        out
    }

    /// THE CURVE AS CUBIC BEZIER PIECES, one a span, each by its four points: every inner knot inserted until it stands
    /// three times (The NURBS Book, A5.6) - what a format of Bezier curves alone, as SVG, takes.
    pub fn beziers(&self) -> Vec<[Point2; 4]> {
        let mut b = self.clone();
        for u in self.inner_knots() {
            let m = b.multiplicity(u);
            b.insert(u, DEGREE.saturating_sub(m));
        }
        b.poles.windows(DEGREE + 1).step_by(DEGREE).map(|w| [w[0], w[1], w[2], w[3]]).collect()
    }

    /// THE SAME CURVE RUN THE OTHER WAY.
    pub fn reversed(&self) -> BSpline {
        BSpline { knots: self.knots.iter().rev().map(|k| 1.0 - k).collect(), poles: self.poles.iter().rev().copied().collect() }
    }

    /// THE CURVE TAKEN THROUGH AN AFFINE MAP OF THE SHEET, `map`: a B-spline of the poles mapped.
    pub fn mapped(&self, map: &dyn Fn(Point2) -> Point2) -> BSpline {
        BSpline { knots: self.knots.clone(), poles: self.poles.iter().map(|p| map(*p)).collect() }
    }

    /// THIS CURVE AND `next`, which starts where this one ends, AS ONE: this one over the share `share` of the parameter,
    /// `next` over the rest; the knot of the joint taken away as far as the curve allows.
    pub fn joined(&self, next: &BSpline, share: f64) -> BSpline {
        let n = self.poles.len();
        let mut knots = vec![0.0; DEGREE + 1];
        knots.extend(self.knots[DEGREE + 1..n].iter().map(|k| k * share));
        knots.extend([share; DEGREE]);
        knots.extend(next.knots[DEGREE + 1..next.poles.len()].iter().map(|k| share + k * (1.0 - share)));
        knots.extend([1.0; DEGREE + 1]);
        let mut poles = self.poles.clone();
        poles.extend_from_slice(&next.poles[1..]);
        let mut b = BSpline { knots, poles };
        b.remove_all(share);
        b
    }

    /// THE CURVE FITTED TO `samples` by least squares, its inner knots `inner`: an open one through the first sample and
    /// the last, a closed one periodic - its curvature unbroken where it closes too. Each span needs samples of its own,
    /// four at the least. `None` where the system has no single answer.
    pub fn fit(samples: &[Sample], inner: &[f64], ends: Ends) -> Option<BSpline> {
        match ends {
            Ends::Open => fit_open(samples, inner),
            Ends::Closed => fit_closed(samples, inner),
        }
    }

    /// Knot `u` taken away as many times as the curve allows.
    fn remove_all(&mut self, u: f64) {
        while self.multiplicity(u) > 0 && self.remove(u) {}
    }
    /// TAKE KNOT `u` AWAY ONCE where the curve stays within `REMOVAL`; answers whether it was.
    fn remove(&mut self, u: f64) -> bool {
        let p = DEGREE;
        let s = self.multiplicity(u);
        let r = self.knots.iter().rposition(|k| *k == u).expect("the knot");
        // a knot standing more than the degree times is an end of the curve, not a joint to smooth away
        if s > p || r < p + 1 {
            return false;
        }
        let (first, last) = (r - p, r - s);
        let ord = p + 1;
        let off = first - 1;
        let n = last - off + 2;
        let mut temp = vec![Point2::new(0.0, 0.0); n];
        temp[0] = self.poles[off];
        temp[last + 1 - off] = self.poles[last + 1];
        let (mut i, mut j) = (first, last);
        let (mut ii, mut jj) = (1, last - off);
        while j as isize - i as isize > 0 {
            let alfi = (u - self.knots[i]) / (self.knots[i + ord] - self.knots[i]);
            let alfj = (u - self.knots[j]) / (self.knots[j + ord] - self.knots[j]);
            temp[ii] = (self.poles[i] - temp[ii - 1] * (1.0 - alfi)) * (1.0 / alfi);
            temp[jj] = (self.poles[j] - temp[jj + 1] * alfj) * (1.0 / (1.0 - alfj));
            i += 1;
            ii += 1;
            j -= 1;
            jj -= 1;
        }
        let kept = if (j as isize) - (i as isize) < 0 {
            temp[ii - 1].dist(temp[jj + 1]) <= REMOVAL
        } else {
            let alfi = (u - self.knots[i]) / (self.knots[i + ord] - self.knots[i]);
            self.poles[i].dist(temp[ii + 1] * alfi + temp[ii - 1] * (1.0 - alfi)) <= REMOVAL
        };
        if !kept {
            return false;
        }
        let (mut i, mut j) = (first, last);
        while j as isize - i as isize > 0 {
            self.poles[i] = temp[i - off];
            self.poles[j] = temp[j - off];
            i += 1;
            j -= 1;
        }
        // the pole at the middle of an even run stands twice; one goes
        let gone = (2 * r - s - p) / 2;
        self.poles.remove(gone);
        self.knots.remove(r);
        true
    }
}

/// THE TANGENTS OF EVERY NODE OF A SPLINE, the Hermite vectors of the spans on either side of it, each span a unit of
/// its own parameter: those of its handle where a node has one, and the rest solved so the curve runs through the nodes
/// with its curvature unbroken - a cubic spline of continuous curvature (C2) in the lengths of the chords of its spans,
/// as a spline through points is in the sketchers a person comes from; natural (no curvature) at the free ends of an
/// open one. A node with a handle is held by it, the curvature turning there.
///
/// Taken from the neighbours of each node alone (Catmull-Rom), the tangents left the curve smooth to the tangent only
/// (C1): the curvature jumped at every node, and a fillet along the side of a body swept from such a spline, laid as one
/// face, was refused at any radius - the walk along it broke (`WalkingFailure`).
pub(crate) fn node_tangents(pts: &[Point2], handles: &[Handle], ends: Ends) -> Vec<NodeTangents> {
    let n = pts.len();
    let closed = ends == Ends::Closed && n >= 3;
    let vec = |v: [f64; 2]| Point2::new(v[0], v[1]);
    let given: Vec<Option<NodeTangents>> = (0..n)
        .map(|i| {
            let h = handles.get(i).copied().unwrap_or_default();
            h.arrive.map(|a| NodeTangents { arrive: vec(a), leave: h.leave.map(vec).unwrap_or(vec(a)) })
        })
        .collect();
    // the length of span k, from node k to the next: its chord
    let chord = |k: usize| pts[k % n].dist(pts[(k + 1) % n]).max(1e-9);
    // the place of each free node among the unknowns
    let mut place = vec![usize::MAX; n];
    let free: Vec<usize> = (0..n).filter(|i| given[*i].is_none()).collect();
    for (u, &k) in free.iter().enumerate() {
        place[k] = u;
    }
    // THE EQUATION OF NODE k in m, the derivative by the length along the chords, divided by the lengths of its two
    // spans so the matrix is symmetric: m[k-1] / h[k-1] + 2 (1 / h[k-1] + 1 / h[k]) m[k] + m[k+1] / h[k]
    //   = 3 ((p[k] - p[k-1]) / h[k-1]^2 + (p[k+1] - p[k]) / h[k]^2),
    // the curvature the same on either side of it; at a free end of an open spline the half of it on its one span, no
    // curvature there. A neighbour held by its handle goes to the right side.
    let mut lower: Vec<Triplet<usize, usize, f64>> = Vec::new();
    let mut side = [vec![0.0; free.len()], vec![0.0; free.len()]];
    for (u, &k) in free.iter().enumerate() {
        let before = (closed || k > 0).then(|| (k + n - 1) % n);
        let after = (closed || k + 1 < n).then(|| (k + 1) % n);
        let mut diag = 0.0;
        let mut rhs = Point2::new(0.0, 0.0);
        let mut neighbours = Vec::with_capacity(2);
        if let Some(j) = before {
            let h = chord(j);
            diag += 2.0 / h;
            rhs = rhs + (pts[k] - pts[j]) * (3.0 / (h * h));
            neighbours.push(Neighbour { node: j, weight: 1.0 / h, held: given[j].map(|g| g.leave * (1.0 / h)) });
        }
        if let Some(j) = after {
            let h = chord(k);
            diag += 2.0 / h;
            rhs = rhs + (pts[j] - pts[k]) * (3.0 / (h * h));
            neighbours.push(Neighbour { node: j, weight: 1.0 / h, held: given[j].map(|g| g.arrive * (1.0 / h)) });
        }
        lower.push(Triplet::new(u, u, diag.max(1e-12)));
        for Neighbour { node, weight, held } in neighbours {
            match held {
                Some(m) => rhs = rhs - m * weight,
                None if place[node] < u => lower.push(Triplet::new(u, place[node], weight)),
                None => {}
            }
        }
        side[0][u] = rhs.x;
        side[1][u] = rhs.y;
    }
    let solved = if free.is_empty() { None } else { crate::linalg::solve_spd(free.len(), &crate::linalg::summed_by_place(lower), &side) };
    (0..n)
        .map(|i| {
            given[i].unwrap_or_else(|| {
                let m = solved.as_ref().map_or(Point2::new(0.0, 0.0), |x| Point2::new(x[0][place[i]], x[1][place[i]]));
                // each span a unit of its own parameter: the derivative by length times the chord of the span
                let into = if closed || i > 0 { chord((i + n - 1) % n) } else { chord(i) };
                let out = if closed || i + 1 < n { chord(i) } else { into };
                NodeTangents { arrive: m * into, leave: m * out }
            })
        })
        .collect()
}

/// A NEIGHBOUR OF A NODE IN THE EQUATION OF ITS TANGENT: the node, its weight in the equation, and its derivative by
/// length where its handle holds it.
struct Neighbour {
    node: usize,
    weight: f64,
    held: Option<Point2>,
}

/// How many even places a span is fitted by, before the places where the curve turns fast.
const FIT_LOOKS: usize = 8;

/// HOW FAR THE CURVE MAY TURN between two places it is fitted by: 10 deg.
const FIT_TURN: f64 = 10.0 * std::f64::consts::PI / 180.0;

/// THE NARROWEST SPAN OF A FIT THAT IS CUT AGAIN, as a share of the parameter of the curve.
const NARROWEST_CUT: f64 = 1e-6;

/// HOW FAR THE CURVE MAY TURN ALONG ONE SPAN of its first fit: 30 deg.
const SPAN_TURN: f64 = 30.0 * std::f64::consts::PI / 180.0;

/// HOW FAR THE CURVE TURNS along `[a, b]`, read on 16 chords of it.
fn turn_along(at: &dyn Fn(f64) -> Point2, [a, b]: [f64; 2]) -> f64 {
    let pts: Vec<Point2> = (0..=16).map(|k| at(a + (b - a) * k as f64 / 16.0)).collect();
    let chords: Vec<Point2> = pts.windows(2).map(|w| w[1] - w[0]).filter(|c| c.len() > 1e-12).collect();
    chords.windows(2).map(|w| (w[0].x * w[1].y - w[0].y * w[1].x).atan2(w[0].x * w[1].x + w[0].y * w[1].y).abs()).sum()
}

/// A CURVE TO APPROXIMATE (`BSpline::approximate`): the point at each place of its parameter from 0 to 1, the places its
/// bends change at to begin the knots with, its ends, how far a point stands off it at a place - the distance to the
/// point of the curve there, or to the curve it should stand by, as an offset - and the tolerance; an open curve's two
/// ends where the fit must pass exactly, if not the points at 0 and 1.
pub struct Approximated<'a> {
    pub at: &'a dyn Fn(f64) -> Point2,
    pub bends: &'a [f64],
    pub ends: Ends,
    pub off: &'a dyn Fn(Point2, f64) -> f64,
    pub tolerance: f64,
    pub through: Option<[Point2; 2]>,
}

/// THE PLACES A SPAN `[a, b]` IS FITTED AND CHECKED BY, its start and the places within it: `FIT_LOOKS` even places,
/// each stretch between two of them cut in two while the curve turns on it by more than `FIT_TURN`, ten times at most.
/// Round the outside of a bend tighter than the distance an offset turns half a turn between two even places, and a fit
/// to those alone did not see the turn: every cut of a span halved its error, which stood at 7.8 mm on a spline of 39
/// nodes offset 3.
fn fit_places(at: &dyn Fn(f64) -> Point2, [a, b]: [f64; 2]) -> Vec<f64> {
    fn finer(at: &dyn Fn(f64) -> Point2, [a, b]: [f64; 2], depth: u32, out: &mut Vec<f64>) {
        let m = 0.5 * (a + b);
        let (p, q, r) = (at(a), at(m), at(b));
        let (u, v) = (q - p, r - q);
        let (lu, lv) = (u.len(), v.len());
        if depth > 0 && lu > 1e-12 && lv > 1e-12 && u.x * v.x + u.y * v.y < FIT_TURN.cos() * lu * lv {
            finer(at, [a, m], depth - 1, out);
            finer(at, [m, b], depth - 1, out);
        } else {
            out.push(a);
        }
    }
    let mut out = Vec::new();
    for j in 0..FIT_LOOKS {
        let (x, y) = (a + (b - a) * j as f64 / FIT_LOOKS as f64, a + (b - a) * (j + 1) as f64 / FIT_LOOKS as f64);
        finer(at, [x, y], 10, &mut out);
    }
    out
}

impl BSpline {
    /// THE CURVE APPROXIMATED BY A CUBIC B-SPLINE within its tolerance, as the kernel of a pro-CAD approximates an
    /// offset or a curve of another form: the knots begun at its bends - at least four spans of a loop - the poles fitted
    /// by least squares (`fit`) to places laid by its turning (`fit_places`), and a span standing off the curve by more
    /// than the tolerance at a place it is fitted by, or half way to the next, cut in two, the rest kept - until every
    /// span holds or the spans are 16 times the bends and 256 more, the best fit then. `None` where no fit has an answer.
    pub fn approximate(c: &Approximated) -> Option<BSpline> {
        let closed = c.ends == Ends::Closed;
        let margin = 1e-6;
        let mut knots: Vec<f64> = c.bends.iter().copied().filter(|u| *u > margin && *u < 1.0 - margin).collect();
        knots.sort_by(f64::total_cmp);
        knots.dedup_by(|a, b| (*a - *b).abs() < margin);
        let least = if closed { DEGREE + 1 } else { 1 };
        let grid_of = |knots: &[f64]| -> Vec<f64> { std::iter::once(0.0).chain(knots.iter().copied()).chain(std::iter::once(1.0)).collect() };
        while knots.len() + 1 < least {
            let grid = grid_of(&knots);
            let k = (0..grid.len() - 1).max_by(|a, b| (grid[a + 1] - grid[*a]).total_cmp(&(grid[b + 1] - grid[*b]))).unwrap_or(0);
            knots.insert(k, 0.5 * (grid[k] + grid[k + 1]));
        }
        // A SPAN THE CURVE TURNS ALONG BY MORE THAN `SPAN_TURN` IS CUT IN TWO BEFORE THE FIRST FIT, twelve times at most:
        // a cubic does not follow a half turn, and a least squares fit missing it there rings along the whole curve - the
        // offset 0.5 of a closed spline of 82 nodes, whose normal turns half a turn along one span round a bend of 2.35 mm,
        // stood a median 0.1 mm off on its own knots, every span went astray, and the copy was cut up to 415 poles
        for _ in 0..12 {
            let grid = grid_of(&knots);
            let sharp: Vec<usize> = (0..grid.len() - 1).filter(|&k| turn_along(c.at, [grid[k], grid[k + 1]]) > SPAN_TURN).collect();
            if sharp.is_empty() {
                break;
            }
            for &k in sharp.iter().rev() {
                knots.insert(k, 0.5 * (grid[k] + grid[k + 1]));
            }
        }
        let most = 16 * (knots.len() + 1) + 256;
        let mut best: Option<BSpline> = None;
        loop {
            let grid = grid_of(&knots);
            let spans = grid.len() - 1;
            let looks: Vec<Vec<f64>> = (0..spans).map(|k| fit_places(c.at, [grid[k], grid[k + 1]])).collect();
            let mut samples: Vec<Sample> = looks.iter().flatten().map(|u| Sample { u: *u, at: (c.at)(*u) }).collect();
            if !closed {
                let [first, last] = c.through.unwrap_or([(c.at)(0.0), (c.at)(1.0)]);
                samples[0].at = first;
                samples.push(Sample { u: 1.0, at: last });
            }
            let Some(b) = BSpline::fit(&samples, &knots, c.ends) else { break };
            // A SPAN NARROWER THAN `NARROWEST_CUT` IS NOT CUT AGAIN: where the curve jumps - the offset of a spline turning
            // back on itself, its normal turning half a turn at a point - no cubic follows it however short, and cutting on
            // ran up to the most spans, every cut a fit of all of them: 11.5 s for one offset of a spline of three nodes
            let astray: Vec<usize> = (0..spans)
                .filter(|&k| grid[k + 1] - grid[k] > NARROWEST_CUT)
                .filter(|&k| {
                    let us = &looks[k];
                    us.iter().zip(us.iter().skip(1).chain(std::iter::once(&grid[k + 1]))).any(|(x, y)| [*x, 0.5 * (x + y)].iter().any(|u| (c.off)(b.at(*u), *u) > c.tolerance))
                })
                .collect();
            let done = astray.is_empty() || spans >= most;
            best = Some(b);
            if done {
                break;
            }
            for &k in astray.iter().rev() {
                knots.insert(k, 0.5 * (grid[k] + grid[k + 1]));
            }
        }
        best
    }
}

/// WHAT A POLE OF A FIT IS: one of the unknowns, by its place among them, or held where the curve must start or end.
#[derive(Clone, Copy)]
enum Role {
    Free(usize),
    Pinned(Point2),
}

/// A POLE AND THE WEIGHT OF ITS BASIS FUNCTION at a place of the curve.
#[derive(Clone, Copy)]
struct Weighted {
    pole: usize,
    weight: f64,
}

/// THE POLES OF THE LEAST SQUARES FIT of `samples`: the weights of the four poles at each place by `weights`, each pole
/// free or pinned by `role`, `unknowns` of them free - the normal equations, symmetric and banded, solved by the sparse
/// solver of the core.
fn least_squares(samples: &[Sample], unknowns: usize, weights: &dyn Fn(f64) -> [Weighted; DEGREE + 1], role: &dyn Fn(usize) -> Role) -> Option<Vec<Point2>> {
    let mut lower: Vec<Triplet<usize, usize, f64>> = Vec::new();
    let mut side = [vec![0.0; unknowns], vec![0.0; unknowns]];
    for s in samples {
        let ws = weights(s.u);
        let mut rest = s.at;
        for w in &ws {
            if let Role::Pinned(p) = role(w.pole) {
                rest = rest - p * w.weight;
            }
        }
        for a in &ws {
            let Role::Free(ia) = role(a.pole) else { continue };
            side[0][ia] += a.weight * rest.x;
            side[1][ia] += a.weight * rest.y;
            for b in &ws {
                if let Role::Free(ib) = role(b.pole) {
                    if ia >= ib {
                        lower.push(Triplet::new(ia, ib, a.weight * b.weight));
                    }
                }
            }
        }
    }
    let solved = crate::linalg::solve_spd(unknowns, &crate::linalg::summed_by_place(lower), &side)?;
    Some((0..unknowns).map(|k| Point2::new(solved[0][k], solved[1][k])).collect())
}

/// THE OPEN CURVE FITTED TO `samples`, clamped, through the first sample and the last (`BSpline::fit`).
fn fit_open(samples: &[Sample], inner: &[f64]) -> Option<BSpline> {
    let (first, last) = (samples.first()?.at, samples.last()?.at);
    let mut knots = vec![0.0; DEGREE + 1];
    knots.extend_from_slice(inner);
    knots.extend([1.0; DEGREE + 1]);
    let n = inner.len() + DEGREE + 1;
    let shape = BSpline { knots, poles: vec![Point2::new(0.0, 0.0); n] };
    let weights = |u: f64| {
        let i = shape.span_of(u);
        let d = basis_derivs(&shape.knots, i, u.clamp(0.0, 1.0));
        std::array::from_fn(|q| Weighted { pole: i - DEGREE + q, weight: d[0][q] })
    };
    let role = |j: usize| match j {
        0 => Role::Pinned(first),
        j if j == n - 1 => Role::Pinned(last),
        j => Role::Free(j - 1),
    };
    let free = least_squares(samples, n - 2, &weights, &role)?;
    let mut poles = vec![first];
    poles.extend(free);
    poles.push(last);
    Some(BSpline { knots: shape.knots, poles })
}

/// THE CLOSED CURVE FITTED TO `samples`, periodic - pole k and pole k + m one pole, `m` the spans - and then clamped at
/// its start (`BSpline::fit`). At least four spans.
fn fit_closed(samples: &[Sample], inner: &[f64]) -> Option<BSpline> {
    let mut at = vec![0.0];
    at.extend_from_slice(inner);
    at.push(1.0);
    let m = at.len() - 1;
    if m < DEGREE + 1 {
        return None;
    }
    // the knots carried a turn on either side
    let mut knots: Vec<f64> = (0..DEGREE).map(|q| at[m - DEGREE + q] - 1.0).collect();
    knots.extend_from_slice(&at);
    knots.extend((1..=DEGREE).map(|q| at[q] + 1.0));
    let weights = |u: f64| {
        let j = (at.partition_point(|k| *k <= u.clamp(0.0, 1.0)).max(1) - 1).min(m - 1);
        let d = basis_derivs(&knots, j + DEGREE, u.clamp(0.0, 1.0));
        std::array::from_fn(|q| Weighted { pole: (j + q) % m, weight: d[0][q] })
    };
    let free = least_squares(samples, m, &weights, &Role::Free)?;
    let poles: Vec<Point2> = (0..m + DEGREE).map(|k| free[k % m]).collect();
    Some(BSpline { knots, poles }.piece([0.0, 1.0]))
}

/// THE BASIS FUNCTIONS OF SPAN `i` AT `t` AND THEIR FIRST TWO DERIVATIVES: row k is the k-th derivative of the four
/// functions that are not zero there (The NURBS Book, A2.3).
fn basis_derivs(knots: &[f64], i: usize, t: f64) -> [[f64; DEGREE + 1]; 3] {
    let p = DEGREE;
    let mut ndu = [[0.0; DEGREE + 1]; DEGREE + 1];
    let mut left = [0.0; DEGREE + 1];
    let mut right = [0.0; DEGREE + 1];
    ndu[0][0] = 1.0;
    for j in 1..=p {
        left[j] = t - knots[i + 1 - j];
        right[j] = knots[i + j] - t;
        let mut saved = 0.0;
        for r in 0..j {
            ndu[j][r] = right[r + 1] + left[j - r];
            let temp = if ndu[j][r] == 0.0 { 0.0 } else { ndu[r][j - 1] / ndu[j][r] };
            ndu[r][j] = saved + right[r + 1] * temp;
            saved = left[j - r] * temp;
        }
        ndu[j][j] = saved;
    }
    let mut ders = [[0.0; DEGREE + 1]; 3];
    for j in 0..=p {
        ders[0][j] = ndu[j][p];
    }
    let mut a = [[0.0; DEGREE + 1]; 2];
    for r in 0..=p as isize {
        let (mut s1, mut s2) = (0usize, 1usize);
        a[0][0] = 1.0;
        for k in 1..=2isize {
            let mut d = 0.0;
            let rk = r - k;
            let pk = p as isize - k;
            if r >= k {
                let den = ndu[(pk + 1) as usize][rk as usize];
                a[s2][0] = if den == 0.0 { 0.0 } else { a[s1][0] / den };
                d = a[s2][0] * ndu[rk as usize][pk as usize];
            }
            let j1 = if rk >= -1 { 1 } else { -rk };
            let j2 = if r - 1 <= pk { k - 1 } else { p as isize - r };
            for j in j1..=j2 {
                let den = ndu[(pk + 1) as usize][(rk + j) as usize];
                a[s2][j as usize] = if den == 0.0 { 0.0 } else { (a[s1][j as usize] - a[s1][(j - 1) as usize]) / den };
                d += a[s2][j as usize] * ndu[(rk + j) as usize][pk as usize];
            }
            if r <= pk {
                let den = ndu[(pk + 1) as usize][r as usize];
                a[s2][k as usize] = if den == 0.0 { 0.0 } else { -a[s1][(k - 1) as usize] / den };
                d += a[s2][k as usize] * ndu[r as usize][pk as usize];
            }
            ders[k as usize][r as usize] = d;
            std::mem::swap(&mut s1, &mut s2);
        }
    }
    let mut f = p as f64;
    for row in ders.iter_mut().skip(1) {
        for v in row.iter_mut() {
            *v *= f;
        }
        f *= (p - 1) as f64;
    }
    ders
}
