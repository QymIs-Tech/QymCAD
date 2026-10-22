//! THE CORNERS OF A SKETCH: rounded and chamfered, one by one or all at once.

use super::blends::EdgeAt;
use super::*;

impl Project {
    /// Set the radius of an arc entity, moving its endpoints to radius `rr` while preserving their angles.
    /// PUT A RADIUS DIMENSION ON A PLAIN ARC: the radius set, then held by a driving dimension, as the radius of a
    /// circle is - one constraint more and one freedom less. Reported behaviour: the radius tool wrote 15 into the arc
    /// and left no dimension, the arc free to be dragged to any radius.
    pub fn put_arc_radius_dim(&mut self, si: usize, eid: Id, rr: f64) {
        let Some((center, a, b, ccw)) = self.sketches.get(si).and_then(|s| s.entities.iter().find(|e| e.id == eid)).and_then(|e| match e.kind.as_arc() {
            Some(crate::model::ArcParts { center, a, b, ccw }) => Some((center, a, b, ccw)),
            _ => None,
        }) else {
            return;
        };
        let rr = rr.max(0.01);
        self.set_arc_radius(si, eid, rr);
        if let Some(s) = self.sketches.get_mut(si) {
            if !s.constraints.iter().any(|c| matches!(c, Constraint::Diameter { c: cc, .. } if *cc == center)) {
                let off = arc_label_angle(&s.points, ArcEnds { center, a, b, ccw });
                s.constraints.push(Constraint::Diameter { c: center, d: rr, off, expr: String::new(), driven: false, diam: false, at: None });
            }
        }
        self.solve_sketch(si);
    }
    pub fn set_arc_radius(&mut self, si: usize, eid: Id, rr: f64) {
        let (center, a, b) = {
            let Some(s) = self.sketches.get(si) else { return };
            let Some(e) = s.entities.iter().find(|e| e.id == eid) else { return };
            match e.kind.as_arc() {
                Some(crate::model::ArcParts { center, a, b, .. }) => (center, a, b),
                _ => return,
            }
        };
        let Some((cx, cy)) = self.point_xy(si, center) else { return };
        let rr = rr.max(0.01);
        for pid in [a, b] {
            if let Some((px, py)) = self.point_xy(si, pid) {
                let ang = (py - cy).atan2(px - cx);
                if let Some(p) = self.sketches[si].points.iter_mut().find(|q| q.id == pid) {
                    p.x = cx + rr * ang.cos();
                    p.y = cy + rr * ang.sin();
                }
            }
        }
        self.solve_sketch(si); // the edit ends at the solver, not at a rebuild: `regen_sketch` recomputes the contours and checks no constraint
    }
    /// The other endpoint of a segment (other than `exclude`) that uses point `pid`.
    pub(crate) fn line_other_end(&self, si: usize, pid: Id, exclude: Id) -> Option<Id> {
        let s = self.sketches.get(si)?;
        s.entities.iter().find_map(|e| {
            if e.id == exclude {
                return None;
            }
            if let Some(crate::model::LineParts { a, b }) = e.kind.as_line() {
                if a == pid {
                    return Some(b);
                }
                if b == pid {
                    return Some(a);
                }
            }
            None
        })
    }
    /// Index of the radius dimension of an arc (a `Diameter` constraint in radius mode on the arc centre).
    pub fn fillet_radius_constraint(&self, si: usize, arc_eid: Id) -> Option<usize> {
        let s = self.sketches.get(si)?;
        let center = s.entity(arc_eid)?.kind.as_arc()?.center;
        s.constraints.iter().position(|c| matches!(*c, Constraint::Diameter { c: cc, .. } if cc == center))
    }
    /// Change the radius of a fillet through its dimension, letting the solver move the geometry while keeping
    /// the tangency. Returns whether the arc has a radius dimension.
    pub fn set_fillet_radius_dim(&mut self, si: usize, arc_eid: Id, new_r: f64) -> bool {
        let Some(ci) = self.fillet_radius_constraint(si, arc_eid) else { return false };
        if let Some(Constraint::Diameter { d, diam, .. }) = self.sketches[si].constraints.get_mut(ci) {
            *d = if *diam { 2.0 * new_r.max(0.01) } else { new_r.max(0.01) };
        }
        // The tangency points and the centre are repositioned geometrically for the new radius, giving the
        // solver a consistent starting point so the geometry does not spread.
        self.set_fillet_radius(si, arc_eid, new_r);
        self.solve_sketch(si);
        true
    }
    /// Change the radius of a fillet arc: the corner is reconstructed from the two adjacent lines and the
    /// fillet is rebuilt, moving the tangency points and the centre. Returns `false` when it is not a
    /// fillet.
    pub fn set_fillet_radius(&mut self, si: usize, arc_eid: Id, new_r: f64) -> bool {
        let (center, t1, t2) = {
            let Some(s) = self.sketches.get(si) else { return false };
            let Some(e) = s.entities.iter().find(|e| e.id == arc_eid) else { return false };
            match e.kind.as_arc() {
                Some(crate::model::ArcParts { center, a, b, .. }) => (center, a, b),
                _ => return false,
            }
        };
        let (Some(o1), Some(o2)) = (self.line_other_end(si, t1, arc_eid), self.line_other_end(si, t2, arc_eid)) else { return false };
        let (Some((o1x, o1y)), Some((t1x, t1y)), Some((o2x, o2y)), Some((t2x, t2y))) = (self.point_xy(si, o1), self.point_xy(si, t1), self.point_xy(si, o2), self.point_xy(si, t2)) else {
            return false;
        };
        // The corner is the intersection of the lines o1 to t1 and o2 to t2.
        let Some((px, py)) = line_intersect_inf([o1x, o1y], [t1x, t1y], [o2x, o2y], [t2x, t2y]) else { return false };
        let l1 = ((o1x - px).powi(2) + (o1y - py).powi(2)).sqrt();
        let l2 = ((o2x - px).powi(2) + (o2y - py).powi(2)).sqrt();
        if l1 < 1e-9 || l2 < 1e-9 {
            return false;
        }
        let (ux, uy) = ((o1x - px) / l1, (o1y - py) / l1);
        let (vx, vy) = ((o2x - px) / l2, (o2y - py) / l2);
        let cosang = (ux * vx + uy * vy).clamp(-1.0, 1.0);
        let theta = cosang.acos();
        if theta < 1e-3 || theta > std::f64::consts::PI - 1e-3 {
            return false;
        }
        let half = theta / 2.0;
        let t = (new_r / half.tan()).min(l1 * 0.98).min(l2 * 0.98);
        let r = t * half.tan();
        let (nt1x, nt1y) = (px + ux * t, py + uy * t);
        let (nt2x, nt2y) = (px + vx * t, py + vy * t);
        let bl = ((ux + vx).powi(2) + (uy + vy).powi(2)).sqrt();
        if bl < 1e-9 {
            return false;
        }
        let (bisx, bisy) = ((ux + vx) / bl, (uy + vy) / bl);
        let (ncx, ncy) = (px + bisx * (r / half.sin()), py + bisy * (r / half.sin()));
        let cross = (nt1x - ncx) * (nt2y - ncy) - (nt1y - ncy) * (nt2x - ncx);
        let ccw = cross > 0.0;
        // Move the existing points; the lines follow t1 and t2.
        for (pid, x, y) in [(t1, nt1x, nt1y), (t2, nt2x, nt2y), (center, ncx, ncy)] {
            if let Some(p) = self.sketches[si].points.iter_mut().find(|q| q.id == pid) {
                p.x = x;
                p.y = y;
            }
        }
        if let Some(e) = self.sketches[si].entities.iter_mut().find(|e| e.id == arc_eid) {
            if e.kind.as_arc().is_some_and(|arc| arc.ccw != ccw) {
                e.kind.reverse_arc();
            }
        }
        // Keep the radius dimension in step with the geometry, when the fillet has a parametric one.
        if let Some(ci) = self.fillet_radius_constraint(si, arc_eid) {
            if let Some(Constraint::Diameter { d, diam, .. }) = self.sketches[si].constraints.get_mut(ci) {
                *d = if *diam { 2.0 * r.max(0.01) } else { r.max(0.01) };
            }
        }
        self.regen_sketch(si);
        true
    }
    /// WHAT HOLDS A LINE GOES WITH THE LINE when a fillet or a chamfer shortens it: a horizontal, vertical, parallel,
    /// perpendicular, collinear or equal of an edge's ends (o-pc) is the edge's own, and after the fillet the edge runs
    /// o-t. Left on the vanished vertex, the automatic constraints a person's defaults put on every corner kept that
    /// vertex alive as a point of its own - six points where the filleted corner has five - and a typed radius seemed
    /// to do nothing. A side that is not a line (`None`) keeps what it had.
    pub(crate) fn carry_edge_constraints(&mut self, si: usize, pc: Id, side1: Option<(Id, Id)>, side2: Option<(Id, Id)>) {
        let pair = |x: Id, y: Id| -> (Id, Id) {
            for (o, t) in [side1, side2].into_iter().flatten() {
                // a side the corner ate whole (radius equal to it) has no second point to carry onto
                if o != t && ((x == o && y == pc) || (x == pc && y == o)) {
                    return (o, t);
                }
            }
            (x, y)
        };
        // only a constraint naming the corner has a pair to carry: `pair` changes a pair one of whose points is `pc`. The
        // whole match for every constraint of the sketch was 2.3 s of filleting every corner of 2 000 rectangles.
        let names_corner = |c: &Constraint| match *c {
            Constraint::Horizontal { a, b } | Constraint::Vertical { a, b } | Constraint::Orientation { a, b, .. } | Constraint::Tangent { a, b, .. } | Constraint::PointOnLine { a, b, .. } => {
                a == pc || b == pc
            }
            Constraint::Equal { a, b, c, d } | Constraint::Parallel { a, b, c, d } | Constraint::Perpendicular { a, b, c, d } | Constraint::Collinear { a, b, c, d } => [a, b, c, d].contains(&pc),
            _ => false,
        };
        if let Some(s) = self.sketches.get_mut(si) {
            for c in s.constraints.iter_mut().filter(|c| names_corner(c)) {
                match c {
                    // a tangency or a point held on a line names the line by two points too: rounding the next corner of
                    // a rectangle shortened a side whose tangency still ran to the old vertex, and that vertex was kept
                    // as a point nothing drew (14 or 15 points on a rectangle rounded all round, not 12)
                    Constraint::Horizontal { a, b }
                    | Constraint::Vertical { a, b }
                    | Constraint::Orientation { a, b, .. }
                    | Constraint::Tangent { a, b, .. }
                    | Constraint::PointOnLine { a, b, .. } => (*a, *b) = pair(*a, *b),
                    // AN EQUALITY IS A LENGTH: it is carried onto the pieces the cut leaves only where they are still
                    // equal - both sides of the corner shortened alike. Carried regardless, it said 55 = 58 for a chamfer
                    // of 3 on one of two equal sides of 58, and the solver bent everything tied to the drawing to satisfy
                    // it. Otherwise it stays on the corner, which then stays as the virtual sharp.
                    Constraint::Equal { a, b, c: cc, d } => {
                        let (ab, cd) = (pair(*a, *b), pair(*cc, *d));
                        let at = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
                        let len = |(u, v): (Id, Id)| Some((at(u)?.0 - at(v)?.0).hypot(at(u)?.1 - at(v)?.1));
                        if let (Some(l1), Some(l2)) = (len(ab), len(cd)) {
                            if (l1 - l2).abs() <= 1e-9 * l1.max(l2).max(1.0) {
                                ((*a, *b), (*cc, *d)) = (ab, cd);
                            }
                        }
                    }
                    // a direction is the same along the whole line, so it is carried as it is
                    Constraint::Parallel { a, b, c: cc, d } | Constraint::Perpendicular { a, b, c: cc, d } | Constraint::Collinear { a, b, c: cc, d } => {
                        (*a, *b) = pair(*a, *b);
                        (*cc, *d) = pair(*cc, *d);
                    }
                    _ => {}
                }
            }
        }
    }
    /// Whether any entity of the sketch stands on the point `pid` - an end of a line or an arc, a centre of one.
    pub fn point_used_by_geometry(&self, si: usize, pid: Id) -> bool {
        self.sketches.get(si).is_some_and(|s| s.entities.iter().any(|e| entity_points(e).contains(&pid)))
    }
    /// HOW MANY CONSTRAINTS STATE SOMETHING ABOUT THE POINT `pid` - a dimension measured to a corner above all.
    pub fn constraints_on_point(&self, si: usize, pid: Id) -> usize {
        self.sketches.get(si).map(|s| s.constraints.iter().filter(|c| constraint_point_ids(c).contains(&pid)).count()).unwrap_or(0)
    }
    /// The two ends of an open curve of any kind, used to find the shared vertex when filleting.
    pub(crate) fn edge_end_ids(&self, si: usize, eid: Id) -> Option<(Id, Id)> {
        let s = self.sketches.get(si)?;
        s.entity(eid).and_then(entity_ends).map(|[a, b]| (a, b))
    }
    /// Current value of a tangent (edge-to-edge) dimension: the centre distance plus m1*r1 + m2*r2. The radii
    /// come from the circle or arc with the matching centre, and are zero when the centre is an ordinary
    /// point.
    pub fn measure_edge_distance(&self, si: usize, c1: Id, m1: i8, c2: Id, m2: i8) -> f64 {
        let Some(s) = self.sketches.get(si) else { return 0.0 };
        let p = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
        let r_of = |cid: Id| -> f64 { s.entities.iter().find_map(|e| e.kind.circle_by(&|id| p(id).map(|(x, y)| Point2::new(x, y))).filter(|c| c.center == cid).map(|c| c.r)).unwrap_or(0.0) };
        let (Some((x1, y1)), Some((x2, y2))) = (p(c1), p(c2)) else { return 0.0 };
        let dist = ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt();
        dist + m1 as f64 * r_of(c1) + m2 as f64 * r_of(c2)
    }
    /// Edges (open curves of any kind) for which point `pid` is an endpoint. Used by click-on-corner fillets and
    /// chamfers.
    pub fn vertex_edges(&self, si: usize, pid: Id) -> Vec<Id> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        s.entities
            .iter()
            // construction lines are not edges of a corner: the diagonals of a rectangle end at its corners
            .filter(|e| !e.construction)
            .filter(|e| entity_ends(e).is_some_and(|[a, b]| a == pid || b == pid))
            .map(|e| e.id)
            .collect()
    }
    /// WHETHER AN EDGE STANDS AT A POINT, which is `vertex_edges` asked the other way round.
    ///
    /// Which of the lines of a point were named by a hand and which were let in by the point itself is decided by
    /// this: a line named by the hand takes its place among the lines of the point it arrives at.
    /// THE TWO POINTS AN EDGE STANDS ON, which is what the pairing of the chosen lines walks along.
    pub fn edge_ends(&self, si: usize, eid: Id) -> Option<(Id, Id)> {
        self.edge_end_ids(si, eid)
    }

    pub fn edge_stands_at(&self, si: usize, eid: Id, pid: Id) -> bool {
        self.edge_end_ids(si, eid).is_some_and(|(x, y)| x == pid || y == pid)
    }
    /// THE CORNER TWO EDGES SHARE: the point that is an end of both of them, when there is one.
    ///
    /// Two lines crossing without a point in common share no corner. That is the whole question the fillet and the
    /// chamfer ask of a chosen pair, and it is asked here once rather than at each place a pair is chosen.
    pub fn shared_vertex(&self, si: usize, e1: Id, e2: Id) -> Option<Id> {
        let (Some((a1, b1)), Some((a2, b2))) = (self.edge_end_ids(si, e1), self.edge_end_ids(si, e2)) else { return None };
        if a1 == a2 || a1 == b2 {
            Some(a1)
        } else if b1 == a2 || b1 == b2 {
            Some(b1)
        } else {
            None
        }
    }
    /// THE CORNER A NAMED PAIR OF EDGES MAKES: the point they meet at, where they make one.
    ///
    /// A shared point is not enough: two lines lying along ONE straight line through it share a vertex and no angle
    /// at all - there is nothing to round between them and nothing to cut. That pair names no corner, and a pair that
    /// names none is not an error: the search for a corner goes on with the second line as the first of the next pair.
    ///
    /// AN ARC IS TAKEN BY ITS TANGENT at the point, not by the chord to its far end: a line going on from a fillet along
    /// its tangent makes no corner either. Taken by the chord, it did: a click on the top of a rounded square by the end
    /// of its fillet named the joint a corner, and the fillet put on it stood its centre 8 above the square. The two
    /// are one straight within 1e-6 of a turn - what a solve leaves at a tangency.
    ///
    /// TWO CURVES SHARING BOTH THEIR ENDS - the halves of two circles cut where they cross, an arc closed by its chord -
    /// make a corner at each end, and this answers the first; `corner_at` asks about one point.
    pub fn corner_of_pair(&self, si: usize, e1: Id, e2: Id) -> Option<Id> {
        let (a, b) = self.edge_end_ids(si, e1)?;
        [a, b].into_iter().find(|&point| self.corner_at(si, CornerAt::of(point, [e1, e2])))
    }
    /// WHETHER THE TWO EDGES OF `corner` MAKE A CORNER AT ITS POINT: both end there and do not go on along one straight
    /// line through it.
    pub fn corner_at(&self, si: usize, corner: CornerAt) -> bool {
        let CornerAt { point, pair } = corner;
        let (e1, e2) = pair;
        if e1 == e2 {
            return false;
        }
        let (Some(u), Some(v)) = (self.leaving(si, EdgeAt { edge: e1, corner: point }), self.leaving(si, EdgeAt { edge: e2, corner: point })) else { return false };
        // parallel means one straight line: the angle between them is 180 degrees, and there is no corner in it
        (u.x * v.y - u.y * v.x).abs() > 1e-6
    }
    /// THE CORNERS A NAMED SET OF LINES MAKES: every pair of them that meets, two at a time, in the order they
    /// were named.
    ///
    /// **WHERE TWO OF THEM MEET THERE IS ONE CORNER**, and one pair is that corner — three lines of a triangle make
    /// three points and so three corners, which is the whole of a closed contour. A line that meets nothing of the
    /// set makes none: a line named on its own is half of the next corner, waiting for whatever answers it.
    ///
    /// **WHERE MORE THAN TWO OF THEM STAND ON ONE POINT** — a cross of four lines, a T-joint — every pair of them is
    /// a corner of that point, and taking all of them would round one place six times. So they are taken **as they
    /// were named, two at a time**: the first with the second, the third with the fourth, and a line left over alone
    /// stands there until another one is named beside it. That is what makes a third line joining a corner two lines
    /// have already made add nothing, and a fourth beside it make a second corner.
    ///
    /// The order is the rule, so it is the order the lines were named in and not the order a map hands them back:
    /// a set read off a hash would pair them by accident of the hashing, and the same drawing would round different
    /// corners on a second run.
    pub fn corners_of_lines(&self, si: usize, lines: &[Id]) -> Vec<(Id, Id)> {
        // THE LINES AS THEY WERE NAMED, one line named twice being one line: a line standing twice at the same
        // point would be paired with itself.
        let mut set: Vec<Id> = Vec::new();
        // a construction line is no side of a corner - the diagonal of a rectangle ends at its corners - so a picked one
        // takes no part in the pairing
        let construction = |me: &Self, l: Id| me.sketches.get(si).is_some_and(|s| s.entities.iter().any(|e| e.id == l && e.construction));
        for &l in lines {
            if !set.contains(&l) && !construction(self, l) {
                set.push(l);
            }
        }
        let here = |me: &Self, p: Id| -> Vec<usize> { (0..set.len()).filter(|&m| me.edge_end_ids(si, set[m]).is_some_and(|(x, y)| x == p || y == p)).collect() };
        let mut out: Vec<(Id, Id)> = Vec::new();
        let mut seen: Vec<Id> = Vec::new(); // the points already asked about, so a line asks about each of its ends once
        for &l in &set {
            let Some((x, y)) = self.edge_end_ids(si, l) else { continue };
            for p in [x, y] {
                if seen.contains(&p) {
                    continue;
                }
                seen.push(p);
                for two in here(self, p).chunks(2) {
                    // A PAIR LYING ALONG ONE STRAIGHT LINE SHARES ITS POINT AND MAKES NO ANGLE: it is not a corner,
                    // and naming it as one would offer a cut where there is nothing to cut.
                    if let [m, n] = two {
                        if self.corner_of_pair(si, set[*m], set[*n]).is_some() {
                            out.push((set[*m], set[*n]));
                        }
                    }
                }
            }
        }
        out
    }
    /// WHAT A CORNER WOULD LOOK LIKE IF THE VALUE WERE APPLIED: the point of the corner, where the two named
    /// edges will be cut, and the arc that will stand between them — a chamfer is a straight cut and has none.
    ///
    /// Nothing is changed here. The edges keep their ends and the point of the corner stays where it is: this is
    /// only what a person is shown before answering for it, drawn exactly as the operation will leave it — or not
    /// drawn at all when the value does not fit the corner.
    ///
    /// `None` when the pair is not a corner, when the value is not one, or when it is too big for the corner.
    pub fn corner_blend(&self, si: usize, pid: Id, (e1, e2): (Id, Id), cut: CornerCut) -> Option<CornerBlend> {
        // THE PAIR MUST BE THE CORNER: two lines that meet nowhere, or that lie along one straight line, are not a
        // corner and there is nothing to show where they are not.
        let corner = CornerAt::of(pid, [e1, e2]);
        if !self.corner_at(si, corner) {
            return None;
        }
        // READ OFF THE CURVES THEMSELVES, by the path the cut takes (`blend_any`), whatever their kinds: taken along the
        // chords to the far ends, the preview of a fillet between two arcs of crossing circles stood 1.9 mm off the arc
        // the fillet laid, and a radius taking a side of two lines whole was laid but not shown
        self.blend_any(si, corner, cut)
    }
    /// THE TWO EDGES OF THE CORNER AT `pid`, out of every edge that meets there.
    ///
    /// Where exactly two edges meet, they are the corner and nothing is asked. Where more of them meet - two squares
    /// sharing a single point, four lines through it - there is a corner for every PAIR, and the pair is the two
    /// edges that bracket the point `near` (the cursor, in the drawing's own coordinates): the angle the cursor is
    /// standing in is the corner being pointed at. Without such a point (`None`) a vertex of more than two edges
    /// names no corner: which of them is meant is not knowable from the geometry alone, and the caller that only
    /// wants a bound asks `corner_limit`, which takes the tightest of them all.
    pub fn vertex_pair(&self, si: usize, pid: Id, near: Option<(f64, f64)>) -> Option<(Id, Id)> {
        let (pcx, pcy) = self.point_xy(si, pid)?;
        // each edge that meets at the vertex, with its direction AWAY from it - the way it leaves the vertex
        let dirs: Vec<(Id, f64, f64)> = self.vertex_edges(si, pid).into_iter().filter_map(|eid| self.leaving(si, EdgeAt { edge: eid, corner: pid }).map(|u| (eid, u.x, u.y))).collect();
        if dirs.len() < 2 {
            return None;
        }
        if dirs.len() == 2 {
            let pair = (dirs[0].0, dirs[1].0);
            return self.corner_at(si, CornerAt { point: pid, pair }).then_some(pair);
        }
        let (nx, ny) = near?;
        let (tx, ty) = (nx - pcx, ny - pcy);
        if (tx * tx + ty * ty).sqrt() < 1e-9 {
            return None; // the cursor stands ON the vertex: no side of it to read
        }
        // sorted by the angle of the direction; the pair that brackets the cursor is the one before it and the
        // one after it, round the circle
        let mut v: Vec<(Id, f64)> = dirs.iter().map(|(id, ux, uy)| (*id, uy.atan2(*ux))).collect();
        v.sort_by(|a, b| a.1.total_cmp(&b.1));
        let at = v.partition_point(|(_, a)| *a <= ty.atan2(tx)) % v.len();
        let pair = (v[(at + v.len() - 1) % v.len()].0, v[at].0);
        // the two sides bracketing the cursor are a corner unless they lie along one straight line
        self.corner_at(si, CornerAt { point: pid, pair }).then_some(pair)
    }
    /// THE CORNER OF ONE NAMED EDGE AT `pid`, on the side the point `near` stands.
    ///
    /// Where several edges meet at one point every pair of them is a corner, and `vertex_pair` lets the cursor say
    /// which. Where the corner has been named by an edge AND the point together there are fewer corners to say it
    /// about - only the ones this edge takes part in - and the cursor says which of them by the sector it stands in:
    /// the two corners of an edge are the two sectors either side of it.
    ///
    /// Without a point to read (`near` is `None` - the pointer is not over the sheet, which happens while the value
    /// is being typed) the neighbour after it stands. The drawing has to begin somewhere, and a corner is as good a
    /// beginning as any; refusing outright would leave a field open with nothing behind it.
    pub fn vertex_pair_through(&self, si: usize, pid: Id, eid: Id, near: Option<(f64, f64)>) -> Option<(Id, Id)> {
        let (pcx, pcy) = self.point_xy(si, pid)?;
        // every edge at the point with the angle of the way it leaves it, in the turn of the circle
        let mut v: Vec<(Id, f64)> = self.vertex_edges(si, pid).into_iter().filter_map(|e| self.leaving(si, EdgeAt { edge: e, corner: pid }).map(|u| (e, u.y.atan2(u.x)))).collect();
        if v.len() < 2 {
            return None; // one edge at the point is not a corner of anything
        }
        v.sort_by(|a, b| a.1.total_cmp(&b.1));
        let at = v.iter().position(|(id, _)| *id == eid)?;
        let len = v.len();
        // THE TWO SECTORS THIS EDGE BOUNDS, one on either side of it in the turn of the circle, each measured as the
        // turn from the edge named so that neither breaks where the angles run out
        let a0 = v[at].1;
        let up = {
            let x = v[(at + 1) % len].1;
            if x > a0 {
                x - a0
            } else {
                x + std::f64::consts::TAU - a0
            }
        };
        let down = {
            let x = v[(at + len - 1) % len].1;
            if x < a0 {
                a0 - x
            } else {
                a0 + std::f64::consts::TAU - x
            }
        };
        // THE SECTOR THE CURSOR STANDS IN, and where it stands in neither, the one it is nearer the middle of - the
        // pointer may be in a sector that belongs to two other edges altogether. The two are equally far apart when
        // the cursor is opposite the edge named, and the one after it in the turn of the circle stands then.
        let forward = near.is_none_or(|(nx, ny)| {
            let mut d = (ny - pcy).atan2(nx - pcx) - a0;
            if d < 0.0 {
                d += std::f64::consts::TAU;
            }
            (d - up / 2.0).abs() <= (d - (std::f64::consts::TAU - down / 2.0)).abs()
        });
        let other = if forward { v[(at + 1) % len].0 } else { v[(at + len - 1) % len].0 };
        let pair = (eid, other);
        self.corner_at(si, CornerAt { point: pid, pair }).then_some(pair)
    }
    /// WHETHER ANY TWO EDGES MAKE A CORNER AT `pid`: what the fillet and the chamfer take a point for.
    pub fn is_corner_point(&self, si: usize, pid: Id) -> bool {
        self.vertex_pairs(si, pid).into_iter().any(|pair| self.corner_at(si, CornerAt { point: pid, pair }))
    }
    /// EVERY PAIR OF EDGES THAT COULD BE THE CORNER at `pid`, for the bounds that do not name a pair.
    pub fn vertex_pairs(&self, si: usize, pid: Id) -> Vec<(Id, Id)> {
        let edges = self.vertex_edges(si, pid);
        let mut out = Vec::new();
        for i in 0..edges.len() {
            for j in (i + 1)..edges.len() {
                out.push((edges[i], edges[j]));
            }
        }
        out
    }
    /// Chamfer the corner at vertex `pid`, where exactly two lines meet. Returns whether it succeeded.
    ///
    /// THE FIRST LINE is the one `toward` stands nearer to - the side of the corner that was clicked: the first leg is
    /// laid along it, and for a leg and an angle the angle is measured from it. With no point given, the line drawn
    /// first.
    pub fn chamfer_at_vertex(&mut self, si: usize, pid: Id, legs: ChamferLegs, toward: Option<Point2>) -> bool {
        let Some(pair) = self.vertex_pair(si, pid, None) else { return false };
        self.chamfer_corner(si, CornerAt { point: pid, pair }, legs, toward)
    }
    /// Chamfer the corner at `pid` that the point `(x, y)` stands in, where more than two edges meet there.
    pub fn chamfer_at_vertex_near(&mut self, si: usize, pid: Id, legs: ChamferLegs, x: f64, y: f64, toward: Option<Point2>) -> bool {
        let Some(pair) = self.vertex_pair(si, pid, Some((x, y))) else { return false };
        self.chamfer_corner(si, CornerAt { point: pid, pair }, legs, toward)
    }
    /// The cut of `legs` off the corner the two named edges make, at the point they share (`chamfer_corner`).
    pub fn chamfer_lines_of_pair(&mut self, si: usize, (e1, e2): (Id, Id), legs: ChamferLegs, toward: Option<Point2>) -> bool {
        let Some(point) = self.corner_of_pair(si, e1, e2) else {
            return false; // two lines in one straight line: nothing is cut, and the cut would be along them
        };
        self.chamfer_corner(si, CornerAt::of(point, [e1, e2]), legs, toward)
    }
    /// The cut of `legs` off `corner`, at its point.
    pub fn chamfer_corner(&mut self, si: usize, corner: CornerAt, legs: ChamferLegs, toward: Option<Point2>) -> bool {
        let CornerAt { point: shared, pair } = corner;
        let (e1, e2) = pair;
        if !self.corner_at(si, corner) {
            return false;
        }
        // the side whose way from the corner leans nearer to the click is the first
        let lean = |me: &Self, eid: Id| -> f64 {
            let Some(toward) = toward else { return 0.0 };
            let (Some(u), Some((px, py))) = (me.leaving(si, EdgeAt { edge: eid, corner: shared }), me.point_xy(si, shared)) else { return f64::MAX };
            let (vx, vy) = (toward.x - px, toward.y - py);
            -(u.x * vx + u.y * vy) / vx.hypot(vy).max(1e-12)
        };
        let (first, second) = if lean(self, e2) < lean(self, e1) { (e2, e1) } else { (e1, e2) };
        self.chamfer_any(si, CornerAt::of(shared, [first, second]), legs)
    }
    /// Connected shape: every entity reachable from `eid` through shared endpoints — a rectangle from one of
    /// its sides, a chain of lines and arcs. A circle or an ellipse stands alone. Used by "fillet all" followed
    /// by a click on a shape.
    pub fn connected_entities(&self, si: usize, eid: Id) -> std::collections::HashSet<Id> {
        let mut out: std::collections::HashSet<Id> = std::collections::HashSet::new();
        let Some(s) = self.sketches.get(si) else { return out };
        let ends = |e: &SketchEntity| -> Vec<Id> { e.kind.straight_or_round_ends().map(|ends| ends.to_vec()).unwrap_or_default() };
        let mut queue = vec![eid];
        while let Some(cur) = queue.pop() {
            if !out.insert(cur) {
                continue;
            }
            let Some(ce) = s.entities.iter().find(|e| e.id == cur) else { continue };
            let cends = ends(ce);
            if cends.is_empty() {
                continue; // A circle or an ellipse stands alone.
            }
            for e in &s.entities {
                if !out.contains(&e.id) && ends(e).iter().any(|id| cends.contains(id)) {
                    queue.push(e.id);
                }
            }
        }
        out
    }
    /// Fillet the corners of the selected geometry: a corner is taken only when both of its edges are in
    /// `only` (`None` means the whole sketch). "Fillet all" with a shape selected fillets that shape alone.
    pub fn fillet_all_corners_of(&mut self, si: usize, r: f64, only: Option<&std::collections::HashSet<Id>>) -> usize {
        self.fillet_all_corners_by(si, FilletSize::radius(r), only)
    }
    /// The same with a size given by a radius, a chord or an arc length: every corner gets the radius the size makes on
    /// it, and keeps the size as it was given (see `fillet_at_vertex_by`).
    pub fn fillet_all_corners_by(&mut self, si: usize, size: FilletSize, only: Option<&std::collections::HashSet<Id>>) -> usize {
        let corners: Vec<Id> = {
            let Some(s) = self.sketches.get(si) else { return 0 };
            // BY ID: a hash map is walked in an order chosen at random for every map, and each corner filleted takes
            // its ids (two tangency points, a centre, an arc) in the order it is reached - the same edit gave the arcs
            // other ids in another run, and so another file
            let mut count: std::collections::BTreeMap<Id, usize> = std::collections::BTreeMap::new();
            for e in &s.entities {
                if e.construction || only.is_some_and(|f| !f.contains(&e.id)) {
                    continue;
                }
                if let Some([a, b]) = e.kind.straight_or_round_ends() {
                    *count.entry(a).or_default() += 1;
                    *count.entry(b).or_default() += 1;
                }
            }
            count.into_iter().filter(|&(_, c)| c == 2).map(|(id, _)| id).collect()
        };
        // every corner laid, the sketch rebuilt once (`batch`)
        self.batch(|p| {
            let mut done = 0;
            for pid in corners {
                // Is the vertex still intact, with two edges still meeting there?
                if p.sketches.get(si).is_some_and(|s| s.points.iter().any(|q| q.id == pid)) && p.vertex_edges(si, pid).len() == 2 && p.fillet_at_vertex_by(si, pid, size) {
                    done += 1;
                }
            }
            done
        })
    }
}
