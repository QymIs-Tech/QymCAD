//! THE KINDS OF CURVE OF A SKETCH: the one place they are told apart. Everything else reads a curve through the parts
//! of its kind named here, or as the geometry it is (`Curve`).
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EntityKind {
    /// Segment between points `a` and `b`.
    Line { a: Id, b: Id },
    /// Arc around `center` from `a` to `b` (counter-clockwise when `ccw`).
    Arc { center: Id, a: Id, b: Id, ccw: bool },
    /// Circle around `center` with radius `r`.
    Circle { center: Id, r: f64 },
    /// Ellipse: centre `c`, endpoint of the major semi-axis `ma`, endpoint of the minor semi-axis `mi`.
    /// The semi-axes are kept perpendicular by an implicit constraint; the major axis is |c-ma|, the minor
    /// |c-mi|, and the rotation is the direction from `c` to `ma`. A real analytic entity with five degrees
    /// of freedom, not a polygon. With a `span` it is an arc of the ellipse: the stretch of it from one point on it to
    /// another.
    Ellipse {
        c: Id,
        ma: Id,
        mi: Id,
        #[serde(default)]
        span: Option<EllipseSpan>,
    },
    /// A spline: a cubic Hermite curve through its `nodes`, in order, a loop when `ends` is `Closed`. Each node has
    /// a `Handle` - the tangent the curve arrives by and the one it leaves by - one per node.
    Spline { nodes: Vec<Id>, handles: Vec<Handle>, ends: crate::feature::Ends },
}

/// THE STRETCH OF AN ELLIPSE AN ARC OF IT TAKES: from point `from` to point `to`, both held on the ellipse. The arc runs
/// between their eccentric angles - the angle `t` of the point `c + a cos t u + b sin t v` of an ellipse of semi-axes
/// `a`, `b` along `u` (to its major end) and `v` (to its minor end) - the way `winding` goes: `Ccw` the way `t` grows,
/// from the major end towards the minor one. Taken through a mirror the axes turn over, and the arc with them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EllipseSpan {
    pub from: Id,
    pub to: Id,
    pub winding: crate::feature::Winding,
}

/// A LINE BY ITS ENDS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineParts {
    pub a: Id,
    pub b: Id,
}

/// AN ARC BY ITS CENTRE AND ITS ENDS, run from `a` to `b` counter-clockwise when `ccw`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArcParts {
    pub center: Id,
    pub a: Id,
    pub b: Id,
    pub ccw: bool,
}

/// A CIRCLE BY ITS CENTRE AND ITS RADIUS.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CircleParts {
    pub center: Id,
    pub r: f64,
}

/// AN ELLIPSE OR AN ARC OF ONE BY ITS FRAME - its centre, the end of its major axis, the end of its minor axis - and the
/// stretch of it an arc takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EllipseParts {
    pub c: Id,
    pub ma: Id,
    pub mi: Id,
    pub span: Option<EllipseSpan>,
}

/// A SPLINE BY ITS NODES, ITS HANDLES AND ITS ENDS, as it is held.
#[derive(Clone, Copy, Debug)]
pub struct SplineParts<'a> {
    pub nodes: &'a [Id],
    pub handles: &'a [Handle],
    pub ends: crate::feature::Ends,
}

/// A POINT OF A CURVE PUT IN THE PLACE OF ANOTHER: the one it stood on, and the one it stands on now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Repoint {
    pub from: Id,
    pub to: Id,
}

/// THE POINTS OF A CURVE A PIECE OF IT STANDS ON as well: the centre of a round curve or of an ellipse, the ends of the
/// axes of an ellipse.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Standing {
    pub centre: Option<Id>,
    pub axes: Option<[Id; 2]>,
}

/// WHERE THE RADIUS OF A ROUND CURVE COMES FROM, for the solver's variable of it: a circle holds it, an arc's is the
/// distance from its centre to its first end.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RadiusSource {
    Held(CircleParts),
    ToEnd { center: Id, end: Id },
}

/// WHAT HOLDS THE POINTS OF A CURVE: the sketch, or the body it is projected from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeldBy {
    Sketch,
    Body,
}

/// WHETHER A MAP TURNS THE PLANE OVER: a move, a turn and a mirror about a point keep its face, a mirror about a line
/// flips it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Facing {
    Kept,
    Flipped,
}

/// HOW A COPY OF A CURVE IS TAKEN: its points to the new ones, the tangents of a spline through the linear part of the
/// map, and whether the map turns the plane over.
pub struct KindMap<'a> {
    pub point: &'a dyn Fn(Id) -> Id,
    pub turn: &'a dyn Fn([f64; 2]) -> [f64; 2],
    pub facing: Facing,
}

/// A SPLINE HELD TO BE CHANGED: its nodes, its handles and its ends.
pub struct SplineMut<'a> {
    pub nodes: &'a mut Vec<Id>,
    pub handles: &'a mut Vec<Handle>,
    pub ends: &'a mut crate::feature::Ends,
}

impl From<LineParts> for EntityKind {
    fn from(p: LineParts) -> Self {
        EntityKind::Line { a: p.a, b: p.b }
    }
}

impl From<ArcParts> for EntityKind {
    fn from(p: ArcParts) -> Self {
        EntityKind::Arc { center: p.center, a: p.a, b: p.b, ccw: p.ccw }
    }
}

impl From<CircleParts> for EntityKind {
    fn from(p: CircleParts) -> Self {
        EntityKind::Circle { center: p.center, r: p.r }
    }
}

impl From<EllipseParts> for EntityKind {
    fn from(p: EllipseParts) -> Self {
        EntityKind::Ellipse { c: p.c, ma: p.ma, mi: p.mi, span: p.span }
    }
}

impl From<SplineParts<'_>> for EntityKind {
    fn from(p: SplineParts<'_>) -> Self {
        EntityKind::Spline { nodes: p.nodes.to_vec(), handles: p.handles.to_vec(), ends: p.ends }
    }
}

/// THE CLASS OF A CURVE A CONSTRAINT TAKES IT AS: a straight, a round curve, an ellipse, or a curve of no class of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CurveClass {
    Straight,
    Round,
    Oval,
    Free,
}

impl EntityKind {
    /// THE LINE IT IS, by its ends; `None` for any other kind.
    pub fn as_line(&self) -> Option<LineParts> {
        match *self {
            EntityKind::Line { a, b } => Some(LineParts { a, b }),
            _ => None,
        }
    }

    /// THE ARC OF A CIRCLE IT IS; `None` for any other kind.
    pub fn as_arc(&self) -> Option<ArcParts> {
        match *self {
            EntityKind::Arc { center, a, b, ccw } => Some(ArcParts { center, a, b, ccw }),
            _ => None,
        }
    }

    /// THE CIRCLE IT IS; `None` for any other kind.
    pub fn as_circle(&self) -> Option<CircleParts> {
        match *self {
            EntityKind::Circle { center, r } => Some(CircleParts { center, r }),
            _ => None,
        }
    }

    /// THE ELLIPSE OR THE ARC OF ONE IT IS; `None` for any other kind.
    pub fn as_ellipse(&self) -> Option<EllipseParts> {
        match *self {
            EntityKind::Ellipse { c, ma, mi, span } => Some(EllipseParts { c, ma, mi, span }),
            _ => None,
        }
    }

    /// THE SPLINE IT IS; `None` for any other kind.
    pub fn as_spline(&self) -> Option<SplineParts<'_>> {
        match self {
            EntityKind::Spline { nodes, handles, ends } => Some(SplineParts { nodes, handles, ends: *ends }),
            _ => None,
        }
    }

    /// A SPLINE through `nodes` with `handles`, open or closed.
    pub fn spline(nodes: Vec<Id>, handles: Vec<Handle>, ends: crate::feature::Ends) -> Self {
        EntityKind::Spline { nodes, handles, ends }
    }

    /// THE SPLINE IT IS, to be changed in place; `None` for any other kind.
    pub fn as_spline_mut(&mut self) -> Option<SplineMut<'_>> {
        match self {
            EntityKind::Spline { nodes, handles, ends } => Some(SplineMut { nodes, handles, ends }),
            _ => None,
        }
    }

    /// HOW A CONSTRAINT TAKES A CURVE OF THIS KIND: as a straight, a round curve, an ellipse or an arc of one, or a curve
    /// of no class of its own - a spline.
    pub fn class(&self) -> CurveClass {
        match self {
            EntityKind::Line { .. } => CurveClass::Straight,
            EntityKind::Circle { .. } | EntityKind::Arc { .. } => CurveClass::Round,
            EntityKind::Ellipse { .. } => CurveClass::Oval,
            EntityKind::Spline { .. } => CurveClass::Free,
        }
    }

    /// THE TWO ENDS OF A LINE OR AN ARC OF A CIRCLE, its first end first; `None` for any other kind.
    pub fn straight_or_round_ends(&self) -> Option<[Id; 2]> {
        match *self {
            EntityKind::Line { a, b } | EntityKind::Arc { a, b, .. } => Some([a, b]),
            _ => None,
        }
    }

    /// THE CIRCLE A ROUND CURVE LIES ON, its points where `at` puts them: a circle's own, an arc's by its centre and the
    /// distance to its first end. `None` for any other kind, or an arc whose points are missing.
    pub fn circle_by(&self, at: &dyn Fn(Id) -> Option<Point2>) -> Option<CircleParts> {
        match *self {
            EntityKind::Circle { center, r } => Some(CircleParts { center, r }),
            EntityKind::Arc { center, a, .. } => Some(CircleParts { center, r: at(center)?.dist(at(a)?) }),
            _ => None,
        }
    }

    /// THE COPY OF THE CURVE taken through `map`: on the new points, a spline's tangents through the linear part of the
    /// map. A mirror about a line turns the way round of an arc of a circle; an arc of an ellipse runs by the eccentric
    /// angle in the frame of its own axes, which the mirror turns over with it, so its winding stays.
    pub fn copied(&self, map: &KindMap) -> EntityKind {
        let m = map.point;
        let mut out = self.clone();
        out.each_point_mut(|id| *id = m(*id));
        match &mut out {
            EntityKind::Arc { ccw, .. } if map.facing == Facing::Flipped => *ccw = !*ccw,
            EntityKind::Spline { handles, .. } => {
                for h in handles.iter_mut() {
                    h.arrive = h.arrive.map(map.turn);
                    h.leave = h.leave.map(map.turn);
                }
            }
            _ => {}
        }
        out
    }

    /// THE POINTS A PIECE OF THE CURVE STANDS ON as well (`Standing`).
    pub fn standing(&self) -> Standing {
        match *self {
            EntityKind::Arc { center, .. } | EntityKind::Circle { center, .. } => Standing { centre: Some(center), axes: None },
            EntityKind::Ellipse { c, ma, mi, .. } => Standing { centre: Some(c), axes: Some([ma, mi]) },
            _ => Standing::default(),
        }
    }

    /// THE KIND OF CURVE `c` LAID IN A SKETCH, on the points `point_at` stands at its ends (and wherever else it needs one)
    /// and on the centre and axes it stands on. An arc of the sketch runs counter-clockwise from its first end; one cut
    /// clockwise is laid the other way.
    pub fn laid(c: &Curve, on: Standing, point_at: &mut dyn FnMut(Point2) -> Id) -> EntityKind {
        match c {
            Curve::Line { a, b } => EntityKind::Line { a: point_at(*a), b: point_at(*b) },
            Curve::Round { centre, r, sweep, .. } => {
                let center = on.centre.unwrap_or_else(|| point_at(*centre));
                if c.closed() {
                    return EntityKind::Circle { center, r: *r };
                }
                let [a, b] = c.ends();
                let (pa, pb) = (point_at(a), point_at(b));
                if *sweep >= 0.0 {
                    EntityKind::Arc { center, a: pa, b: pb, ccw: true }
                } else {
                    EntityKind::Arc { center, a: pb, b: pa, ccw: true }
                }
            }
            Curve::Oval { centre, major, minor, sweep, .. } => {
                let cid = on.centre.unwrap_or_else(|| point_at(*centre));
                let [ma, mi] = on.axes.unwrap_or_else(|| [point_at(*centre + *major), point_at(*centre + *minor)]);
                let span = (!c.closed()).then(|| {
                    let [a, b] = c.ends();
                    let (from, to) = (point_at(a), point_at(b));
                    let winding = if *sweep >= 0.0 { crate::feature::Winding::Ccw } else { crate::feature::Winding::Cw };
                    EllipseSpan { from, to, winding }
                });
                EntityKind::Ellipse { c: cid, ma, mi, span }
            }
            // a node at every knot of its B-spline, held by the tangents there: the entity draws this curve again
            Curve::Spline { b, ends } => {
                let h = b.hermite(*ends);
                let ids: Vec<Id> = h.nodes.iter().map(|q| point_at(*q)).collect();
                let handles = h.tangents.iter().map(|t| Handle { arrive: Some([t.arrive.x, t.arrive.y]), leave: Some([t.leave.x, t.leave.y]) }).collect();
                EntityKind::Spline { nodes: ids, handles, ends: *ends }
            }
        }
    }

    /// THE CONSTRAINTS THE CURVE HOLDS ITSELF BY, neither stored nor deletable: an arc keeps its ends on its circle -
    /// each end tied as `end_on` says, given the centre and the end - and an arc projected from a body, whose centre and
    /// ends the body pins, one end only: the other would say the radius again. An ellipse keeps its axes square, and the
    /// ends of an arc of it on it.
    pub fn intrinsics(&self, end_on: &dyn Fn(Id, Id) -> Constraint, held: HeldBy) -> Vec<Constraint> {
        match *self {
            EntityKind::Arc { center, a, .. } if held == HeldBy::Body => vec![Constraint::PointOnCircle { p: a, c: center }],
            EntityKind::Arc { center, a, b, .. } => vec![end_on(center, a), end_on(center, b)],
            EntityKind::Ellipse { c, ma, mi, span } => {
                std::iter::once(Constraint::Perpendicular { a: c, b: ma, c, d: mi }).chain(span.into_iter().flat_map(|s| [s.from, s.to]).map(|p| Constraint::PointOnEllipse { p, c, ma, mi })).collect()
            }
            _ => Vec::new(),
        }
    }

    /// WHERE ITS RADIUS COMES FROM, for a circle or an arc of one; `None` for any other kind.
    pub fn radius_source(&self) -> Option<RadiusSource> {
        match *self {
            EntityKind::Circle { center, r } => Some(RadiusSource::Held(CircleParts { center, r })),
            EntityKind::Arc { center, a, .. } => Some(RadiusSource::ToEnd { center, end: a }),
            _ => None,
        }
    }

    /// THE END OF A LINE OR AN ARC OF A CIRCLE MOVED to another point: the end standing on `end.from` stands on `end.to`;
    /// the centre of an arc stays. Answers whether an end moved.
    pub fn move_end(&mut self, end: Repoint) -> bool {
        match self {
            EntityKind::Line { a, b } | EntityKind::Arc { a, b, .. } => {
                let held = if *a == end.from {
                    a
                } else if *b == end.from {
                    b
                } else {
                    return false;
                };
                *held = end.to;
                true
            }
            _ => false,
        }
    }

    /// THE ARC RUN THE OTHER WAY ROUND, its ends as they stand; answers whether it is an arc of a circle.
    pub fn reverse_arc(&mut self) -> bool {
        match self {
            EntityKind::Arc { ccw, .. } => {
                *ccw = !*ccw;
                true
            }
            _ => false,
        }
    }

    /// WHAT TELLS ONE CURVE OF THE SAME POINTS FROM ANOTHER: its kind, an arc's way round and a circle's radius - all the
    /// rest a curve holds is its points.
    pub fn signature(&self) -> [u64; 2] {
        match self {
            EntityKind::Line { .. } => [1, 0],
            EntityKind::Arc { ccw, .. } => [2, u64::from(*ccw)],
            EntityKind::Circle { r, .. } => [3, r.to_bits()],
            EntityKind::Ellipse { .. } => [4, 0],
            EntityKind::Spline { .. } => [5, 0],
        }
    }

    /// EVERY POINT IT STANDS ON, to be changed in place: the ends, the centre, the frame, the nodes.
    pub fn each_point_mut(&mut self, mut f: impl FnMut(&mut Id)) {
        match self {
            EntityKind::Line { a, b } => {
                f(a);
                f(b);
            }
            EntityKind::Arc { center, a, b, .. } => {
                f(center);
                f(a);
                f(b);
            }
            EntityKind::Circle { center, .. } => f(center),
            EntityKind::Ellipse { c, ma, mi, span } => {
                f(c);
                f(ma);
                f(mi);
                if let Some(s) = span {
                    f(&mut s.from);
                    f(&mut s.to);
                }
            }
            EntityKind::Spline { nodes, .. } => nodes.iter_mut().for_each(f),
        }
    }

    /// THE CENTRE OF A ROUND CURVE: a circle's or an arc's; `None` for any other kind.
    pub fn round_centre(&self) -> Option<Id> {
        match *self {
            EntityKind::Circle { center, .. } | EntityKind::Arc { center, .. } => Some(center),
            _ => None,
        }
    }

    /// THE KEY OF THE NAME OF ITS KIND in the catalogue of words: a line, an arc, a circle, an ellipse, a spline.
    pub fn name_key(&self) -> &'static str {
        match self {
            EntityKind::Line { .. } => "ent-line",
            EntityKind::Arc { .. } => "ent-arc",
            EntityKind::Circle { .. } => "ent-circle",
            EntityKind::Ellipse { .. } => "ent-ellipse",
            EntityKind::Spline { .. } => "ent-spline",
        }
    }

    /// THE HOW OF THE CURVE, hashed: its kind, the points it stands on, its way round and its ends - not its radius
    /// or the pull of its handles, which a change of the solver moves.
    pub fn hash_shape<H: std::hash::Hasher>(&self, h: &mut H) {
        use std::hash::Hash;
        match self {
            EntityKind::Line { a, b } => [0, *a, *b, 0].hash(h),
            EntityKind::Arc { center, a, b, ccw } => [1, *center, *a, *b, u64::from(*ccw)].hash(h),
            EntityKind::Circle { center, .. } => [2, *center].hash(h),
            EntityKind::Ellipse { c, ma, mi, span } => {
                [3, *c, *ma, *mi].hash(h);
                span.hash(h);
            }
            EntityKind::Spline { nodes, ends, .. } => {
                4.hash(h);
                nodes.hash(h);
                ends.hash(h);
            }
        }
    }

    /// THE RADIUS OF A CIRCLE SET to `r`; answers whether it is a circle.
    pub fn set_radius(&mut self, r: f64) -> bool {
        match self {
            EntityKind::Circle { r: held, .. } => {
                *held = r;
                true
            }
            _ => false,
        }
    }
}

/// The points an entity stands on.
pub fn entity_points(e: &SketchEntity) -> Vec<Id> {
    match &e.kind {
        EntityKind::Line { a, b } => vec![*a, *b],
        EntityKind::Arc { center, a, b, .. } => vec![*center, *a, *b],
        EntityKind::Circle { center, .. } => vec![*center],
        EntityKind::Ellipse { c, ma, mi, span: None } => vec![*c, *ma, *mi],
        EntityKind::Ellipse { c, ma, mi, span: Some(s) } => vec![*c, *ma, *mi, s.from, s.to],
        EntityKind::Spline { nodes, .. } => nodes.clone(),
    }
}

/// THE TWO ENDS OF AN OPEN CURVE, its start first as `Curve` runs it: a line's, an arc's, an arc of an ellipse's, an open
/// spline's first and last nodes. `None` for a curve with no ends - a circle, an ellipse, a closed spline.
pub fn entity_ends(e: &SketchEntity) -> Option<[Id; 2]> {
    match &e.kind {
        EntityKind::Line { a, b } | EntityKind::Arc { a, b, .. } => Some([*a, *b]),
        EntityKind::Ellipse { span: Some(s), .. } => Some([s.from, s.to]),
        EntityKind::Spline { nodes, ends: crate::feature::Ends::Open, .. } => Some([*nodes.first()?, *nodes.last()?]),
        _ => None,
    }
}

/// THE CENTRE OF A ROUND CURVE OR OF AN ELLIPSE: a circle's, an arc's, an ellipse's or an arc of one's.
pub fn entity_centre(e: &SketchEntity) -> Option<Id> {
    match &e.kind {
        EntityKind::Circle { center, .. } | EntityKind::Arc { center, .. } => Some(*center),
        EntityKind::Ellipse { c, .. } => Some(*c),
        _ => None,
    }
}

/// THE FRAME OF AN ELLIPSE OR OF AN ARC OF ONE: its centre, the end of its major axis, the end of its minor axis.
pub fn ellipse_frame(e: &SketchEntity) -> Option<[Id; 3]> {
    match &e.kind {
        EntityKind::Ellipse { c, ma, mi, .. } => Some([*c, *ma, *mi]),
        _ => None,
    }
}

/// THE CIRCLE A ROUND CURVE LIES ON - a circle's own, an arc's by its centre and the distance to its first end; `None`
/// for any other kind, or an arc whose points are missing.
pub fn circle_of(s: &Sketch, e: &SketchEntity) -> Option<CircleParts> {
    match e.kind {
        EntityKind::Circle { center, r } => Some(CircleParts { center, r }),
        EntityKind::Arc { center, a, .. } => {
            let (c, p) = (s.point(center)?, s.point(a)?);
            Some(CircleParts { center, r: (p.x - c.x).hypot(p.y - c.y) })
        }
        _ => None,
    }
}

/// THE CONSTRAINT HOLDING POINT `p` ON CURVE `e`, of whatever kind: on the line of a line, on the circle of a circle or
/// an arc, on the ellipse of an ellipse or an arc of one, on a spline.
pub fn point_on(e: &SketchEntity, p: Id) -> Constraint {
    match &e.kind {
        EntityKind::Line { a, b } => Constraint::PointOnLine { p, a: *a, b: *b },
        EntityKind::Circle { center, .. } | EntityKind::Arc { center, .. } => Constraint::PointOnCircle { p, c: *center },
        EntityKind::Ellipse { c, ma, mi, .. } => Constraint::PointOnEllipse { p, c: *c, ma: *ma, mi: *mi },
        EntityKind::Spline { .. } => Constraint::PointOnCurve { p, on: curve_ref_of(e) },
    }
}

/// THE SIZE OF A CURVE A DIMENSION TAKES FROM ONE CLICK, as the dimension it lays: a circle's diameter, an arc's radius,
/// an ellipse's major half axis (the distance from its centre to the end of that axis). `None` for a curve with no such
/// size - a line, a spline - or one whose points are missing.
pub fn size_dimension(s: &Sketch, e: &SketchEntity) -> Option<Constraint> {
    let at = |id: Id| s.point(id).map(|q| Point2::new(q.x, q.y));
    Some(match e.kind {
        EntityKind::Circle { center, r } => Constraint::Diameter { c: center, d: 2.0 * r, off: 0.0, expr: String::new(), driven: false, diam: true, at: None },
        EntityKind::Arc { center, a, .. } => Constraint::Diameter { c: center, d: at(center)?.dist(at(a)?), off: 0.0, expr: String::new(), driven: false, diam: false, at: None },
        EntityKind::Ellipse { c, ma, .. } => Constraint::Distance { a: c, b: ma, d: at(c)?.dist(at(ma)?), off: 0.0, expr: String::new(), driven: false, axis: 0, at: None },
        _ => return None,
    })
}
