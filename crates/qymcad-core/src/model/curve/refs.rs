//! A CURVE AS A CONSTRAINT REFERS TO IT (`CurveRef`), and the reference a curve of each kind is taken by.
//!
//! A spline is referred to by its id and read off the curve as it is when the sketch is solved (`Sketch::as_solved`):
//! a constraint that kept a copy of the nodes and the handles held the curve as it was laid, and lost it once the curve
//! was trimmed, a node of it went or a handle was dragged.
use super::*;

/// A CURVE AS A CONSTRAINT SEES IT: by the points it stands on, so the solver moves the curve by moving them - a line by
/// its ends, a circle or an arc by its centre (its radius is the variable of that centre), an ellipse by its centre and
/// the ends of its axes, a spline by its id.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CurveRef {
    /// a point, where a dimension is measured from a point to a curve
    Point {
        p: Id,
    },
    Line {
        a: Id,
        b: Id,
    },
    Round {
        c: Id,
    },
    /// an arc as it is drawn, between its ends, where a dimension measures to the arc itself and not its circle
    Arc {
        c: Id,
        a: Id,
        b: Id,
        winding: crate::feature::Winding,
    },
    /// an ellipse by its frame: whole, or as drawn between the ends of its `span` where it is an arc of one
    Oval {
        c: Id,
        ma: Id,
        mi: Id,
        #[serde(default)]
        span: Option<EllipseSpan>,
    },
    /// a spline of the sketch, by its id
    Spline {
        curve: Id,
    },
    /// a spline as it stands, for the solver: its nodes, handles and ends read off the sketch before a solve. Never saved.
    #[serde(skip)]
    SplineNow {
        nodes: Vec<Id>,
        handles: Vec<Handle>,
        ends: crate::feature::Ends,
    },
}

impl CurveRef {
    /// The points the curve stands on. A spline referred to by its id stands on none of its own: its nodes are the
    /// curve's, and they come with it when it is read for a solve.
    pub fn points(&self) -> Vec<Id> {
        match self {
            CurveRef::Point { p } => vec![*p],
            CurveRef::Line { a, b } => vec![*a, *b],
            CurveRef::Round { c } => vec![*c],
            CurveRef::Arc { c, a, b, .. } => vec![*c, *a, *b],
            CurveRef::Oval { c, ma, mi, span } => [*c, *ma, *mi].into_iter().chain(span.iter().flat_map(|s| [s.from, s.to])).collect(),
            CurveRef::Spline { .. } => Vec::new(),
            CurveRef::SplineNow { nodes, .. } => nodes.clone(),
        }
    }

    /// Every point the curve stands on, handed to `f` to read or change.
    pub fn each_point_mut(&mut self, mut f: impl FnMut(&mut Id)) {
        match self {
            CurveRef::Point { p } => f(p),
            CurveRef::Line { a, b } => {
                f(a);
                f(b);
            }
            CurveRef::Round { c } => f(c),
            CurveRef::Arc { c, a, b, .. } => {
                f(c);
                f(a);
                f(b);
            }
            CurveRef::Oval { c, ma, mi, span } => {
                f(c);
                f(ma);
                f(mi);
                if let Some(s) = span {
                    f(&mut s.from);
                    f(&mut s.to);
                }
            }
            CurveRef::Spline { .. } => {}
            CurveRef::SplineNow { nodes, .. } => nodes.iter_mut().for_each(f),
        }
    }

    /// The point it is, where it is a point.
    pub fn as_point(&self) -> Option<Id> {
        match self {
            CurveRef::Point { p } => Some(*p),
            _ => None,
        }
    }

    /// The curve of the sketch it names by id, where it names one.
    pub fn curve_id(&self) -> Option<Id> {
        match self {
            CurveRef::Spline { curve } => Some(*curve),
            _ => None,
        }
    }

    /// The curve it names by id named anew where `to` gives a new id for it.
    pub fn rename_curve(&mut self, to: &dyn Fn(Id) -> Option<Id>) {
        if let CurveRef::Spline { curve } = self {
            if let Some(id) = to(*curve) {
                *curve = id;
            }
        }
    }

    /// THE TWO ENDS OF AN OPEN CURVE: a line's, an arc's, an open spline's first and last nodes, as read for a solve.
    /// `None` for a point, a circle, an ellipse, a closed spline.
    pub fn open_ends(&self) -> Option<[Id; 2]> {
        match self {
            CurveRef::Line { a, b } | CurveRef::Arc { a, b, .. } => Some([*a, *b]),
            CurveRef::Oval { span: Some(s), .. } => Some([s.from, s.to]),
            CurveRef::SplineNow { nodes, ends: crate::feature::Ends::Open, .. } => Some([*nodes.first()?, *nodes.last()?]),
            _ => None,
        }
    }

    /// Whether the curve is a line, read along the whole straight it lies on where a distance or a touching is measured.
    pub fn is_straight(&self) -> bool {
        matches!(self, CurveRef::Line { .. })
    }

    /// HOW HARD THE CURVE IS TO READ ALONG: a point or a line 0, a circle or an arc 1, an ellipse 2, a spline 3. Of two
    /// curves, the simpler is the one read along.
    pub fn order(&self) -> u8 {
        match self {
            CurveRef::Point { .. } | CurveRef::Line { .. } => 0,
            CurveRef::Round { .. } | CurveRef::Arc { .. } => 1,
            CurveRef::Oval { .. } => 2,
            CurveRef::Spline { .. } | CurveRef::SplineNow { .. } => 3,
        }
    }

    /// The centre whose radius is a variable of the solve, for a circle taken whole.
    pub fn radius_centre(&self) -> Option<Id> {
        match self {
            CurveRef::Round { c } => Some(*c),
            _ => None,
        }
    }

    /// WHETHER THE SOLVER CAN READ THE CURVE: a circle's or an arc's centre is the centre of one (`is_centre`), a spline
    /// has been read off the sketch and has two nodes; anything else as it is.
    pub fn readable(&self, is_centre: &dyn Fn(Id) -> bool) -> bool {
        match self {
            CurveRef::Round { c } | CurveRef::Arc { c, .. } => is_centre(*c),
            CurveRef::Spline { .. } => false,
            CurveRef::SplineNow { nodes, .. } => nodes.len() >= 2,
            _ => true,
        }
    }

    /// A point, where a dimension is measured from a point to a curve.
    pub fn at_point(p: Id) -> CurveRef {
        CurveRef::Point { p }
    }

    /// A line between points `a` and `b`, as a dimension measures to a line.
    pub fn line(ends: [Id; 2]) -> CurveRef {
        CurveRef::Line { a: ends[0], b: ends[1] }
    }
}

impl Constraint {
    /// Every curve the constraint refers to, handed to `f` to change.
    pub fn each_curve_mut(&mut self, mut f: impl FnMut(&mut CurveRef)) {
        match self {
            Constraint::PointOnCurve { on, .. } => f(on),
            Constraint::TangentCurves { one, other, .. } | Constraint::CurveDistance { from: one, to: other, .. } => {
                f(one);
                f(other);
            }
            _ => {}
        }
    }

    /// The curves of the sketch the constraint names by id.
    pub fn curve_ids(&self) -> Vec<Id> {
        match self {
            Constraint::PointOnCurve { on, .. } => on.curve_id().into_iter().collect(),
            Constraint::TangentCurves { one, other, .. } | Constraint::CurveDistance { from: one, to: other, .. } => one.curve_id().into_iter().chain(other.curve_id()).collect(),
            _ => Vec::new(),
        }
    }
}

impl Sketch {
    /// THE CURVE `on` AS IT STANDS NOW: a spline referred to by its id read off the sketch - its nodes, handles and ends.
    /// A spline the sketch no longer has stays as it was, and the solver cannot read it.
    pub fn resolved(&self, on: &CurveRef) -> CurveRef {
        let now = on.curve_id().and_then(|id| self.entity(id)).and_then(|e| e.kind.as_spline());
        match now {
            Some(sp) => CurveRef::SplineNow { nodes: sp.nodes.to_vec(), handles: sp.handles.to_vec(), ends: sp.ends },
            None => on.clone(),
        }
    }

    /// CONSTRAINT `c` AS THE SOLVER READS IT: every curve it refers to as it stands now (`resolved`).
    pub fn as_solved(&self, c: &Constraint) -> Constraint {
        let mut out = c.clone();
        if !c.curve_ids().is_empty() {
            out.each_curve_mut(|on| *on = self.resolved(on));
        }
        out
    }

    /// DROP THE CONSTRAINTS ON WHAT IS GONE: a point no longer in the sketch, or a curve - and the offsets whose source
    /// or copy went.
    pub(crate) fn drop_constraints_on_what_is_gone(&mut self) {
        self.drop_offsets_of_what_is_gone();
        let points: std::collections::HashSet<Id> = self.points.iter().map(|p| p.id).collect();
        let curves: std::collections::HashSet<Id> = self.entities.iter().map(|e| e.id).collect();
        self.constraints.retain(|c| crate::model::constraint_point_ids(c).iter().all(|id| points.contains(id)) && c.curve_ids().iter().all(|id| curves.contains(id)));
    }

    /// A CURVE NAMED ANEW in every constraint that refers to it.
    pub(crate) fn rename_curve_refs(&mut self, named: CurveNamed) {
        for c in self.constraints.iter_mut() {
            c.each_curve_mut(|on| on.rename_curve(&|id| (id == named.was).then_some(named.now)));
        }
    }
}

/// THE CURVE OF AN ENTITY AS A CONSTRAINT SEES IT: a line by its ends, a circle or an arc by its centre, an ellipse or an
/// arc of one by its frame - whole, as a constraint ties to the whole curve - a spline by its id.
pub fn curve_ref_of(e: &SketchEntity) -> CurveRef {
    match &e.kind {
        EntityKind::Line { a, b } => CurveRef::Line { a: *a, b: *b },
        EntityKind::Circle { center, .. } | EntityKind::Arc { center, .. } => CurveRef::Round { c: *center },
        EntityKind::Ellipse { c, ma, mi, .. } => CurveRef::Oval { c: *c, ma: *ma, mi: *mi, span: None },
        EntityKind::Spline { .. } => CurveRef::Spline { curve: e.id },
    }
}

/// THE CURVE OF AN ENTITY AS IT IS DRAWN, for a dimension measuring to it and a touching at its end: an arc or an arc of
/// an ellipse between its ends, any other as a constraint sees it (`curve_ref_of`).
pub fn curve_ref_drawn(e: &SketchEntity) -> CurveRef {
    match &e.kind {
        EntityKind::Arc { center, a, b, ccw } => CurveRef::Arc { c: *center, a: *a, b: *b, winding: if *ccw { crate::feature::Winding::Ccw } else { crate::feature::Winding::Cw } },
        EntityKind::Ellipse { c, ma, mi, span: span @ Some(_) } => CurveRef::Oval { c: *c, ma: *ma, mi: *mi, span: *span },
        _ => curve_ref_of(e),
    }
}

/// A CURVE NAMED ANEW: the id it had, and the id it has.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CurveNamed {
    pub(crate) was: Id,
    pub(crate) now: Id,
}

/// WHERE A CONSTRAINT HOLDS A CURVE: the constraint, by its place in the sketch, and the point of the curve it holds at.
pub(crate) struct HeldAt {
    ci: usize,
    at: Point2,
}

impl Project {
    /// WHERE THE CONSTRAINTS ON CURVE `eid` OF SKETCH `si` HOLD IT, read before the curve is cut: a point put on it at that
    /// point, a touching at its end at the end, a touching along it at the place it touches, a distance at the end of
    /// the distance on it.
    pub(crate) fn where_refs_hold(&self, si: usize, eid: Id) -> Vec<HeldAt> {
        let Some(s) = self.sketches.get(si) else { return Vec::new() };
        let Some(curve) = s.entity(eid).and_then(|e| Curve::of(s, e)) else { return Vec::new() };
        let at = |id: Id| s.point(id).map(|q| Point2::new(q.x, q.y));
        let mine = |on: &CurveRef| on.curve_id() == Some(eid);
        let radii = self.entity_radii(si);
        let mut out = Vec::new();
        for (ci, c) in s.constraints.iter().enumerate() {
            let held = match c {
                Constraint::PointOnCurve { p, on } if mine(on) => at(*p),
                Constraint::TangentCurves { end: Some(p), one, other, .. } if mine(one) || mine(other) => at(*p),
                Constraint::TangentCurves { one, other, near, .. } if mine(one) || mine(other) => near.map(|[t1, t2]| curve.at(if mine(one) { t1 } else { t2 })),
                Constraint::CurveDistance { from, to, .. } if mine(from) || mine(to) => {
                    crate::solver::distance_ends(&s.points, &radii, [&s.resolved(from), &s.resolved(to)]).map(|[a, b]| if mine(from) { a } else { b })
                }
                _ => None,
            };
            out.extend(held.map(|at| HeldAt { ci, at }));
        }
        out
    }

    /// THE CONSTRAINTS ON CURVE `eid`, CUT INTO `pieces`, PUT ON THE PIECE NEAREST WHERE EACH HELD IT (`where_refs_hold`).
    /// A constraint on the curve that held it nowhere to be read goes with it.
    pub(crate) fn refs_go_to_pieces(&mut self, si: usize, eid: Id, held: &[HeldAt], pieces: &[Id]) {
        let Some(s) = self.sketches.get_mut(si) else { return };
        let off = |id: Id, q: Point2| s.entity(id).and_then(|e| Curve::of(s, e)).map_or(f64::MAX, |c| c.nearest(q).off);
        let onto: Vec<Option<Id>> = held.iter().map(|h| pieces.iter().copied().min_by(|a, b| off(*a, h.at).total_cmp(&off(*b, h.at)))).collect();
        for (h, piece) in held.iter().zip(onto) {
            let Some(piece) = piece else { continue };
            if let Some(c) = s.constraints.get_mut(h.ci) {
                c.each_curve_mut(|on| on.rename_curve(&|id| (id == eid).then_some(piece)));
            }
        }
        s.drop_constraints_on_what_is_gone();
    }
}
