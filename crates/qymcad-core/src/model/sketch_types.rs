//! THE RECORDS OF A SKETCH: its notes, texts, rectangles, patterns, projections, splines, constraints.

use super::*;

/// A sketch: a named group of contours (a tree node). It may be the result of an import, in which case
/// `source` references the embedded original.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Sketch {
    pub id: Id,
    pub name: String,
    /// Stable ids of the contours in this sketch (order is for display).
    pub contour_ids: Vec<Id>,
    /// Import source (an id in `Project::sources`) when the sketch was imported.
    #[serde(default)]
    pub source: Option<Id>,
    /// Typed sketch points, shared between entities: moving a point moves every entity that references it,
    /// which is the foundation of dimensions and constraints.
    #[serde(default)]
    pub points: Vec<SketchPoint>,
    /// Typed entities (line, arc, circle) expressed through point ids.
    #[serde(default)]
    pub entities: Vec<SketchEntity>,
    /// Whether the entity chain is closed (used when tessellating into a contour).
    #[serde(default)]
    pub closed: bool,
    /// Geometric constraints and dimensions (solved by the sketch solver).
    #[serde(default)]
    pub constraints: Vec<Constraint>,
    /// Splines (smooth curves through control points).
    #[serde(default)]
    pub splines: Vec<Spline>,
    /// Text notes and annotations. Not geometry: they never reach a profile or a toolpath.
    #[serde(default)]
    pub notes: Vec<Note>,
    /// Parametric text geometry (labels turned into glyph contours). Does reach profiles and toolpaths.
    #[serde(default)]
    pub texts: Vec<SketchText>,
    /// Editable patterns: a source, the layout parameters and the ids of the derived copies.
    #[serde(default)]
    pub patterns: Vec<SketchPattern>,
    /// The rectangles kept as one shape: their sides, corners, centre and construction diagonals, see `SketchRect`.
    #[serde(default)]
    pub rects: Vec<SketchRect>,
    /// Projections of body geometry (driven entities that reference their source).
    #[serde(default)]
    pub projections: Vec<SketchProjection>,
    /// Placement of the sketch in 3D (a plane, a face or a datum). The 2D geometry is lifted through this
    /// frame. Defaults to the global XY plane.
    #[serde(default)]
    pub plane: crate::feature::SketchPlane,
    /// Id of the origin point (0,0), fixed. Zero means it has not been materialised yet — it is created
    /// lazily on the first constraint or dimension that references the origin. Never reaches a profile.
    #[serde(default)]
    pub origin: Id,
    /// Guide points of the X and Y axes (fixed, at (1,0) and (0,1)), so the axes can be picked as lines for
    /// constraints (parallel, point-on-axis, symmetry). Zero means they have not been created.
    #[serde(default)]
    pub axis_pts: [Id; 2],
    /// THE ANCHOR OF THE FRAME OF REFERENCE at (0,0) - the second end of both axis lines.
    ///
    /// SEPARATE FROM `origin`, and that separation is the whole point. `origin` is a point a person may
    /// snap geometry to, and `merge_close_points` glues anything landing on it INTO it - so the centre of
    /// a circle drawn at zero becomes the origin itself. Moving that circle then moved the origin, and
    /// with it both axes, since an axis is the infinite line through the origin and its guide.
    ///
    /// Reported behaviour, in that order: first "a dimension to the X or Y axis comes out crooked" (the
    /// axes had become diagonals - measured on a file where the origin stood at (-23.45, 18.70)), and
    /// then, after the origin was pinned at zero, "I cannot move a circle away from the origin" - because
    /// the pinned point WAS the circle's centre.
    ///
    /// The anchor is nobody's geometry: it is never picked, never glued, never moved. Zero means it has
    /// not been materialised yet.
    #[serde(default)]
    pub frame: Id,
    /// Offset of the sketch origin within its plane (u,v in frame axes), produced by snapping to an edge
    /// or a vertex when the plane was chosen. `None` means the default origin (the projection of the source
    /// origin). Invariant to body placement: u*X + v*Y travels with the frame (see `sketch_frame`).
    #[serde(default)]
    pub origin_uv: Option<Point2>,
    /// THE PARTS THE LAST SOLVE DID NOT REACH: its time ran out before them (`solver::Budget`), and they stand as
    /// they stood until the next solve. Not a fact of the drawing - it is not written to a file.
    #[serde(skip)]
    pub left_unsolved: usize,
    /// WHAT A DRAG UNDER WAY KEEPS from one frame to the next (`drag::DragSession`). Not a fact of the drawing.
    #[serde(skip)]
    pub(crate) drag_session: Option<Box<drag::DragSession>>,
    /// WHAT ITS LOOPS WERE MADE FROM (`round::Laid`): a rebuild makes again only the loops round what changed since.
    /// Not a fact of the drawing.
    #[serde(skip)]
    pub(crate) laid: Option<Box<round::Laid>>,
    /// WHAT ITS CHECKS REMEMBER OF EACH PART (`solver::PartMemo`): a change counts again the parts it touched. Not a
    /// fact of the drawing.
    #[serde(skip)]
    pub(crate) checked: CheckMemo,
    /// THE PLACES OF THE POINTS AND OF THE ENTITIES BY ID (`Sketch::point`, `Sketch::entity`). Not a fact of the drawing.
    #[serde(skip)]
    pub(crate) point_at: IdPlaces,
    #[serde(skip)]
    pub(crate) entity_at: IdPlaces,
}

/// THE PLACE OF EACH ID IN A LIST, a table that checks itself: a place found is taken only where the list still holds
/// that id there; an id not found puts in the tail the list has grown by, and failing that makes the table again. Asked through `Sketch::point` and
/// `Sketch::entity`, it is right however the lists were changed; looked along the lists, a tool laying an element at
/// every corner of 2 000 rectangles looked through them some ten times a corner - 7 s in a release build. Copied, a
/// sketch makes its own anew.
#[derive(Default)]
pub(crate) struct IdPlaces(pub(crate) std::sync::Mutex<std::collections::HashMap<Id, usize>>);

/// THE MEMORY OF THE CHECKS OF A SKETCH, asked through `&Project` (`sketch_checks`, `sketch_conflicts`). Copied, a
/// sketch starts its own empty; printed, it shows nothing - as `IdPlaces`.
#[derive(Default)]
pub(crate) struct CheckMemo(pub(crate) std::sync::Mutex<crate::solver::PartMemo>);

impl Clone for CheckMemo {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl std::fmt::Debug for CheckMemo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CheckMemo")
    }
}

impl CheckMemo {
    pub(crate) fn with<R>(&self, work: impl FnOnce(&mut crate::solver::PartMemo) -> R) -> R {
        let mut memo = self.0.lock().unwrap_or_else(|e| e.into_inner());
        work(&mut memo)
    }
}

/// Printed without its places: a table filled as ids are asked for is no fact of the sketch, and a sketch compared by
/// its print before and after an operation that changed nothing read as changed where only the table had filled.
impl std::fmt::Debug for IdPlaces {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("IdPlaces")
    }
}

impl Clone for IdPlaces {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl IdPlaces {
    pub(crate) fn find<T>(&self, list: &[T], id: Id, id_of: impl Fn(&T) -> Id) -> Option<usize> {
        let mut places = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(&i) = places.get(&id) {
            if list.get(i).is_some_and(|x| id_of(x) == id) {
                return Some(i);
            }
        }
        // most changes add to the end of a list: the new tail is put in first, the whole list only if that is not it
        let known = places.len();
        if known < list.len() {
            for (i, x) in list.iter().enumerate().skip(known) {
                places.insert(id_of(x), i);
            }
            if let Some(&i) = places.get(&id) {
                if list.get(i).is_some_and(|x| id_of(x) == id) {
                    return Some(i);
                }
            }
        }
        *places = list.iter().enumerate().map(|(i, x)| (id_of(x), i)).collect();
        places.get(&id).copied()
    }
}

impl Sketch {
    /// The point of id `id` (`IdPlaces`).
    pub fn point(&self, id: Id) -> Option<&SketchPoint> {
        self.point_at.find(&self.points, id, |p| p.id).map(|i| &self.points[i])
    }

    /// The entity of id `id` (`IdPlaces`).
    pub fn entity(&self, id: Id) -> Option<&SketchEntity> {
        self.entity_at.find(&self.entities, id, |e| e.id).map(|i| &self.entities[i])
    }

    /// System points of the sketch: the origin and the endpoints of the X and Y axes. They cannot be
    /// dragged, deleted or counted as free.
    ///
    /// One source of truth on purpose: spelled out by hand in six places the set drifts apart, some copies
    /// forgetting the origin and others the axes.
    /// THE CONSTRAINTS A CUT CORNER HOLDS ITSELF BY, by their index: the virtual sharp standing on the extension of each
    /// side (`PointOnLine` of a point no geometry ends at), the legs of a symmetric chamfer kept equal on it (`Equal` from
    /// the sharp to the two ends of the cut line), and the arc of a fillet touching the lines it ends on (`Tangent` of a
    /// line to the arc that ends at one of its points). They are the corner's own, as the turn of a rectangle is the
    /// rectangle's: shown, a chamfer of 3 carried two "=" and a point-on-line badge out in the air beside its cut, and a
    /// fillet a tangency badge on each line. A dimension a person measured to the sharp is not among them.
    pub fn corner_holders(&self) -> std::collections::HashSet<usize> {
        // no constraint of a kind a corner holds itself by, no holder - before the points of every entity are gathered:
        // 6 ms on 70 000 segments, and the list of constraints asks every frame
        if !self.constraints.iter().any(|c| matches!(c, Constraint::PointOnLine { .. } | Constraint::Equal { .. } | Constraint::Tangent { .. })) {
            return Default::default();
        }
        let drawn: std::collections::HashSet<Id> = self
            .entities
            .iter()
            .flat_map(|e| match e.kind {
                EntityKind::Line { a, b } => vec![a, b],
                EntityKind::Arc { center, a, b, .. } => vec![center, a, b],
                EntityKind::Circle { center, .. } => vec![center],
                EntityKind::Ellipse { c, ma, mi } => vec![c, ma, mi],
            })
            .collect();
        let cut = |x: Id, y: Id| self.entities.iter().any(|e| matches!(e.kind, EntityKind::Line { a, b } if (a == x && b == y) || (a == y && b == x)));
        let at = |id: Id| self.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
        // whether `p` lies on the line through `a` and `b` beyond its ends, by more than a millionth of its length
        let past_the_ends = |p: Id, a: Id, b: Id| match (at(p), at(a), at(b)) {
            (Some(p), Some(a), Some(b)) => {
                let (dx, dy) = (b.0 - a.0, b.1 - a.1);
                let t = ((p.0 - a.0) * dx + (p.1 - a.1) * dy) / (dx * dx + dy * dy).max(1e-18);
                !(-1e-6..=1.0 + 1e-6).contains(&t)
            }
            _ => false,
        };
        self.constraints
            .iter()
            .enumerate()
            .filter(|(_, c)| match **c {
                // the sharp stands past the end of a side cut back from it; a point put on a line or on an axis - the centre
                // of a rectangle, a point of its own - stands on it, and its badge says it is held there
                Constraint::PointOnLine { p, a, b } => !drawn.contains(&p) && !self.system_ids().contains(&p) && cut(a, b) && past_the_ends(p, a, b),
                Constraint::Equal { a, b, c, d } => a == c && !drawn.contains(&a) && cut(b, d),
                Constraint::Tangent { a, b, c, .. } => {
                    self.entities.iter().any(|e| matches!(e.kind, EntityKind::Arc { center, a: x, b: y, .. } if center == c && [x, y].iter().any(|q| *q == a || *q == b)))
                }
                _ => false,
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// WHETHER THE CIRCLE OR ARC ABOUT `centre` CARRIES A SIZE OF ITS OWN: a radius or a diameter, the length of the arc,
    /// or the chord between its two ends. Where it does, no passive radius is labelled beside it: a fillet sized by its
    /// chord or its arc length showed an R it does not hold by as well.
    pub fn rim_sized(&self, centre: Id) -> bool {
        let chord = |x: Id, y: Id| self.entities.iter().any(|e| matches!(e.kind, EntityKind::Arc { center, a, b, .. } if center == centre && ((a == x && b == y) || (a == y && b == x))));
        self.constraints.iter().any(|c| match *c {
            Constraint::Diameter { c, .. } | Constraint::ArcLength { c, .. } => c == centre,
            Constraint::Distance { a, b, .. } => chord(a, b),
            _ => false,
        })
    }

    /// The points not drawn among the sketch's own: the frame of reference, drawn by the axis marker. The virtual sharp of
    /// a cut corner is drawn, and picked, like any point.
    pub fn unseen_points(&self) -> std::collections::HashSet<Id> {
        self.system_ids().into_iter().collect()
    }

    pub fn system_ids(&self) -> Vec<Id> {
        std::iter::once(self.origin).chain(std::iter::once(self.frame)).chain(self.axis_pts).filter(|id| *id != 0).collect()
    }

    /// PUT THE FRAME OF REFERENCE BACK WHERE IT BELONGS: the origin at (0,0), the axis guides at (1,0)
    /// and (0,1). Returns whether anything had to be moved.
    ///
    /// Reported behaviour: "I cannot put a dimension from the centre of a circle to the X or Y axis - a
    /// crooked one is placed instead." Measured on the reporter's file: the origin stood at
    /// (-23.454944474040893, 18.70469405576201).
    ///
    /// AN AXIS IS THE INFINITE LINE THROUGH THE ORIGIN AND ITS GUIDE. Move the origin and the "X axis"
    /// becomes a diagonal; a distance to it is then measured honestly, to the wrong line, and the number
    /// on the drawing belongs to nothing. The same for the guides.
    ///
    /// Why this is repaired rather than only prevented: `Constraint::Fixed` pins a point to where it IS,
    /// not to zero, so a frame that moved once is locked in the wrong place for good - and documents in
    /// that state already exist. Called on every rebuild of the sketch, so a file opens repaired.
    pub fn pin_frame(&mut self) -> bool {
        // THE ORIGIN IS PINNED HERE TOO, and that is safe only because it is nobody's geometry by the time
        // this runs: `regen_sketch` calls `detach_geometry_from_origin` first, which gives any drawing
        // welded to the origin a point of its own. Pinning a point that geometry sits on is what nailed
        // down a shape drawn at zero. The frame itself stands on `frame`, which is never anybody's.
        // A DOCUMENT WHOSE AXES WERE HUNG ON THE ORIGIN IS REPAIRED HERE.
        //
        // Before the anchor existed, an axis was the line through the ORIGIN and its guide, and every
        // dimension to an axis holds those two ids. Such files exist and open crooked. Any constraint
        // naming the origin together with an axis guide is re-pointed at the anchor: the line it means
        // is the axis, and the axis now stands on the anchor.
        if self.origin != 0 && self.frame != 0 {
            let guides = self.axis_pts;
            let (origin, frame) = (self.origin, self.frame);
            for c in &mut self.constraints {
                let pts = c.points();
                if pts.contains(&origin) && guides.iter().any(|g| *g != 0 && pts.contains(g)) {
                    remap_constraint_point(c, origin, frame);
                }
            }
        }
        let want: [(Id, f64, f64); 4] = [(self.frame, 0.0, 0.0), (self.origin, 0.0, 0.0), (self.axis_pts[0], 1.0, 0.0), (self.axis_pts[1], 0.0, 1.0)];
        let mut moved = false;
        for (id, x, y) in want {
            if id == 0 {
                continue;
            }
            if let Some(p) = self.points.iter_mut().find(|p| p.id == id) {
                if p.x != x || p.y != y {
                    p.x = x;
                    p.y = y;
                    moved = true;
                }
            }
        }
        moved
    }

    /// Driven points (projections of a body). They cannot be dragged or deleted one by one, being derived
    /// from body geometry. Same role as `system_ids`, and asked for in the same places.
    pub fn projected_points(&self) -> std::collections::HashSet<Id> {
        self.projections.iter().flat_map(|p| p.points.iter().copied()).collect()
    }

    /// Driven entities (projections of a body).
    pub fn projected_entities(&self) -> std::collections::HashSet<Id> {
        self.projections.iter().flat_map(|p| p.entities.iter().copied()).collect()
    }

    /// Points that cannot be dragged: the system ones (origin and axes) plus the driven projections.
    pub fn immovable_points(&self) -> std::collections::HashSet<Id> {
        let mut s = self.projected_points();
        s.extend(self.system_ids());
        s
    }

    /// Points a person has pinned with the anchor constraint.
    pub fn pinned_points(&self) -> std::collections::HashSet<Id> {
        self.constraints
            .iter()
            .filter_map(|c| match c {
                Constraint::Fixed { p } => Some(*p),
                _ => None,
            })
            .collect()
    }

    /// WHAT GOES WITH THE POINT `centre` WHEN IT IS DRAGGED, as a circle goes with its centre: the corners of a rectangle
    /// it is the centre of, the two ends of an arc it is the centre of, every point of a slot it is the centre of an end
    /// of (a slot's two arcs are held to one radius, `EqualRadius`), and the axis ends of an ellipse. Left behind, the
    /// drag pulled the centre alone: an arc and a slot were refused the drag, and an ellipse turned and changed shape.
    /// The centre of a fillet - an arc touching its lines (`Tangent`) with no partner of one radius - carries nothing:
    /// the lines it touches hold it. Empty for any other point.
    pub fn carried_with(&self, centre: Id) -> Vec<Id> {
        let rect: Vec<Id> = self.rects.iter().filter(|r| r.centre == centre).flat_map(|r| r.corners).collect();
        if !rect.is_empty() {
            return rect;
        }
        if let Some((ma, mi)) = self.entities.iter().find_map(|e| match e.kind {
            EntityKind::Ellipse { c, ma, mi } if c == centre => Some((ma, mi)),
            _ => None,
        }) {
            return vec![ma, mi];
        }
        let Some(arc) = self.entities.iter().find(|e| matches!(e.kind, EntityKind::Arc { center, .. } if center == centre)) else { return Vec::new() };
        let touches = self.constraints.iter().any(|c| matches!(*c, Constraint::Tangent { c, .. } if c == centre));
        let partner = self.constraints.iter().find_map(|c| match *c {
            Constraint::EqualRadius { c1, c2 } if c1 == centre => Some(c2),
            Constraint::EqualRadius { c1, c2 } if c2 == centre => Some(c1),
            _ => None,
        });
        match (touches, partner) {
            // a slot: every point of the shape the arc is part of, the centres of its arcs with them
            (_, Some(_)) => {
                let mut shape = vec![arc.id];
                let mut k = 0;
                while k < shape.len() {
                    let here = self.entities.iter().find(|e| e.id == shape[k]).map_or(Vec::new(), entity_points);
                    for e in &self.entities {
                        if !shape.contains(&e.id) && entity_points(e).iter().any(|p| here.contains(p)) {
                            shape.push(e.id);
                        }
                    }
                    k += 1;
                }
                let mut out: Vec<Id> = shape.iter().flat_map(|id| self.entities.iter().find(|e| e.id == *id).map_or(Vec::new(), entity_points)).filter(|p| *p != centre).collect();
                out.sort_unstable();
                out.dedup();
                out
            }
            // a fillet: held by the lines it touches
            (true, None) => Vec::new(),
            // an arc of its own: its two ends
            (false, None) => match arc.kind {
                EntityKind::Arc { a, b, .. } => vec![a, b],
                _ => Vec::new(),
            },
        }
    }

    /// EVERYTHING AN EDITING TOOL MUST LEAVE WHERE IT IS: the frame, the driven projections, and whatever
    /// the person pinned.
    ///
    /// Why the pinned ones need naming at all. `Constraint::Fixed` carries no coordinates - it pins the point
    /// to where it IS at the moment of the solve. So a tool that shifts the point first and asks the solver
    /// afterwards gets a constraint satisfied by the NEW place: the pin travels with the drawing instead of
    /// holding it, the glyph stays green and nobody is told. Not the same list as `immovable_points`, which
    /// answers "may this be deleted one by one" - a pinned point is deleted like any other.
    pub fn held_points(&self) -> std::collections::HashSet<Id> {
        let mut s = self.immovable_points();
        s.extend(self.pinned_points());
        s
    }
}

/// A text note in a sketch (an annotation). Not geometry.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Note {
    pub x: f64,
    pub y: f64,
    pub text: String,
}

/// WHERE THE OUTLINES OF A TEXT CAME FROM.
///
/// Kept in the document beside the baked glyphs, because a re-bake without it uses whatever font happens to
/// be loaded: open the file again, change the height of a label, and it is silently redrawn in somebody
/// else's typeface. Measured: a label written in Liberation Sans came back in Cantarell, 1.201 wide for its
/// height against 1.379.
///
/// Both the name and the path, on purpose. The name survives a move to another machine, where the file lies
/// elsewhere; the path finds the file at once on this one. The index is the face inside a collection - a
/// `.ttc` holds several, and a variable font its instances.
///
/// Empty means "not recorded": labels written before this was kept. Their drawing is intact, since the
/// glyphs are in the file; what cannot be done to them is an honest re-bake.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct FontRef {
    #[serde(default)]
    pub family: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub index: u32,
}

impl FontRef {
    /// Is anything recorded at all.
    pub fn is_empty(&self) -> bool {
        self.family.is_empty() && self.path.is_empty()
    }
}

/// Parametric sketch text: a label turned into glyph contours, which are real geometry for profiles and
/// toolpaths.
///
/// The parameters (position, height, angle, string) are the source of truth; `glyphs` are the baked glyph
/// polylines in world coordinates. Baking happens in the application, because the font lives there: after
/// an edit to the string or the height the application re-bakes the glyphs and calls `regen_sketch`. The
/// text stays one selectable, movable, editable object, unlike loose contours produced by a one-way
/// conversion.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SketchText {
    pub id: Id,
    pub x: f64,
    pub y: f64,
    pub height: f64,
    #[serde(default)]
    pub angle: f64,
    pub text: String,
    #[serde(default)]
    pub construction: bool,
    /// Baked closed glyph polylines (world coordinates).
    #[serde(default)]
    pub glyphs: Vec<Vec<Point2>>,
    /// The font these glyphs were baked from - see `FontRef`.
    #[serde(default)]
    pub font: FontRef,
}

/// What a body is for the purpose of export (see [`Project::export_kind`]): one policy for STEP and STL.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportKind {
    /// Live B-rep: exported to STEP as exact geometry and to STL as a tessellation at the chosen accuracy.
    Brep,
    /// A mesh with no recipe (an STL import): no B-rep exists at all. It goes into STL; STEP skips it,
    /// because there is nothing exact to write, and the export has to say so.
    MeshOnly,
    /// A body with a recipe but without a live B-rep: a failed regenerate (a red node). The screen shows
    /// the last good geometry and STL exports that same geometry, while STEP will not contain the body at
    /// all. Both exports have to report this; staying silent about it is the defect.
    Stale,
}

/// Pattern kind: linear (step dx, dy) or circular (centre plus sector).
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub enum PatternKind {
    /// Linear pattern: `count` copies along (dx,dy) and `count2` copies along the second direction
    /// (dx2,dy2), forming a grid of rows by columns. `count2 <= 1` makes it one-dimensional; the second
    /// direction is optional.
    Linear {
        dx: f64,
        dy: f64,
        count: u32,
        #[serde(default)]
        dx2: f64,
        #[serde(default)]
        dy2: f64,
        #[serde(default)]
        count2: u32,
    },
    Circular {
        cx: f64,
        cy: f64,
        count: u32,
        total_deg: f64,
    },
}

/// A SKETCH RECTANGLE KEPT AS ONE SHAPE: four sides each held at its turn (`Constraint::Orientation`: the first and the
/// third at the turn of the rectangle, the second and the fourth square to them), and a centre on the middle of its
/// diagonal - drawn as two construction diagonals for a rectangle drawn from its centre. It stays a rectangle until it
/// is broken - a side deleted - and then the record, the centre and the diagonals go and four plain lines are left.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SketchRect {
    pub id: Id,
    /// The corners in order round the rectangle.
    pub corners: [Id; 4],
    /// The sides: corner k to corner k + 1.
    pub sides: [Id; 4],
    pub centre: Id,
    /// The construction diagonals of a rectangle drawn from its centre: corner 0 to 2, corner 1 to 3. A rectangle drawn
    /// by its corners has none - its centre is held on the middle without them - as the diagonals would split every
    /// line crossing it for Trim (a construction line is a boundary).
    pub diagonals: Option<[Id; 2]>,
    /// What a width or a height changed grows the rectangle from.
    pub anchor: RectAnchor,
}

/// THE WIDTH AND THE HEIGHT DIMENSIONS OF A RECTANGLE, as indices of its sketch's constraints: the first along its
/// first side, the second along the side after it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RectDims {
    pub width: usize,
    pub height: usize,
}

/// What stays put when the width or the height of a rectangle changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RectAnchor {
    /// The corner it was drawn from (a point id).
    Corner(Id),
    /// Its centre: a rectangle drawn from the centre grows about it.
    Centre,
}

/// An editable sketch pattern: source entities plus layout parameters produce derived instances.
///
/// The instance ids are stored so that editing the parameters can recreate them. Instances are real
/// entities and reach profiles and toolpaths; this record only ties them back to their source so the
/// pattern stays editable.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SketchPattern {
    pub id: Id,
    pub source: Vec<Id>,
    pub kind: PatternKind,
    #[serde(default)]
    pub instances: Vec<Id>,
}

/// Source of a projection into a sketch: a reference to body geometry by persistent name rather than by
/// number. A projection has to survive edits higher up the timeline, which is the whole point of it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ProjSource {
    /// A single edge of a body.
    Edge(u32),
    /// A whole face: its entire outline is projected. The list of edges is deliberately not stored, since
    /// a fillet or a cut changes which edges there are and a stored list would silently fall behind the
    /// part.
    Face(u32),
}

/// Projection of body geometry into a sketch. Driven geometry: it cannot be dragged by hand and is
/// recomputed from the body on every rebuild, but constraints and dimensions may reference it.
///
/// Ids of the derived points and entities stay stable as long as the structure of the source has not
/// changed; otherwise a constraint attached to a corner of the projection would fall off on every rebuild
/// of the body.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SketchProjection {
    pub id: Id,
    /// Source body.
    pub body: Id,
    pub src: ProjSource,
    /// Derived points (driven).
    #[serde(default)]
    pub points: Vec<Id>,
    /// Derived entities (driven).
    #[serde(default)]
    pub entities: Vec<Id>,
    /// THE EDGE OF THE BODY EACH OF `entities` CAME FROM, by its persistent id, in the same order: a rebuild matches the
    /// curves by their edge, so the ids - and the dimensions and the ties on them - stay with the edge whatever order the
    /// kernel gives the edges in.
    #[serde(default)]
    pub edges: Vec<u32>,
}

/// A sketch spline: a smooth Catmull-Rom curve through control points.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Spline {
    pub points: Vec<Id>,
    /// Explicit tangent vector at each node; `None` means the automatic Catmull-Rom tangent, which follows
    /// the nodes. Dragging a handle makes the tangent explicit, pinning the shape at that node.
    #[serde(default)]
    pub tangents: Vec<Option<[f64; 2]>>,
    pub closed: bool,
    /// A construction spline: drawn dashed and never reaching a profile or a toolpath.
    #[serde(default)]
    pub construction: bool,
}

/// A sketch constraint or dimension, expressed through point ids. Solved by the sketch solver.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Constraint {
    /// Pin a point at its current position (an anchor).
    Fixed { p: Id },
    /// Horizontal: points `a` and `b` share the same height (equal y).
    Horizontal { a: Id, b: Id },
    /// Vertical: points `a` and `b` share the same vertical (equal x).
    Vertical { a: Id, b: Id },
    /// THE TURN OF A SKETCH RECTANGLE: the side from `a` to `b` stands at `deg` degrees to the X axis. The rectangle
    /// holds it itself and it is never drawn nor typed: a corner dragged changes the size and leaves the turn, the
    /// Rotate tool changes `deg`, and an angle dimension a person puts on a side takes its place. Without it a
    /// rectangle held by parallel and perpendicular sides turned under a dragged corner.
    Orientation { a: Id, b: Id, deg: f64 },
    /// Points `a` and `b` coincide.
    Coincident { a: Id, b: Id },
    /// Dimension: the distance between `a` and `b` equals `d`. `off` is the offset of the dimension line
    /// (a perpendicular shift of the label in pixels, for manual placement). `expr` is an optional
    /// expression (`w/2`, `len+5`); when it is not empty, `d` is recomputed from the project parameters,
    /// making the dimension parametric.
    Distance {
        a: Id,
        b: Id,
        d: f64,
        #[serde(default)]
        off: f64,
        #[serde(default)]
        expr: String,
        /// A reference (driven) dimension: it does not constrain the geometry and only displays the
        /// measured value. Applied automatically where an ordinary dimension would be redundant.
        #[serde(default)]
        driven: bool,
        /// Orientation of a linear dimension: 0 aligned (euclidean |AB|), 1 horizontal (|dx|), 2 vertical
        /// (|dy|). Chosen by cursor position while placing it.
        #[serde(default)]
        axis: u8,
        /// Where the text stands along the dimension line, as a share of it from its first end: 0.5 in the middle,
        /// below 0 or above 1 past an arrow, the line then running on to the text as a shelf. Absent: the middle, or a
        /// shelf past the second arrow when the text does not fit between them.
        #[serde(default)]
        at: Option<f64>,
    },
    /// Segments `a-b` and `c-d` are parallel.
    Parallel { a: Id, b: Id, c: Id, d: Id },
    /// Segments `a-b` and `c-d` are perpendicular.
    Perpendicular { a: Id, b: Id, c: Id, d: Id },
    /// Segments `a-b` and `c-d` have equal length.
    Equal { a: Id, b: Id, c: Id, d: Id },
    /// Angle at vertex `b` between the rays `b-a` and `b-c`, in degrees. `expr` is a parametric
    /// expression; when it is not empty, `deg` is recomputed from the project parameters.
    Angle {
        a: Id,
        b: Id,
        c: Id,
        deg: f64,
        #[serde(default)]
        expr: String,
        /// A reference (driven) angle; see `Distance::driven`.
        #[serde(default)]
        driven: bool,
        /// Where the arc of the dimension stands: its radius from the vertex, in sketch units; 0 is the default
        /// distance on screen.
        #[serde(default)]
        off: f64,
        /// Where the label stands along the arc, as a share of the angle from the side `b-a` towards `b-c`: 0.5 on the
        /// bisector (also when absent), below 0 or above 1 past a side, the arc then running on to it. A share rather
        /// than a direction, so the label keeps its place when the sides turn.
        #[serde(default)]
        at: Option<f64>,
    },
    /// Segments `a-b` and `c-d` are collinear (they lie on one straight line).
    Collinear { a: Id, b: Id, c: Id, d: Id },
    /// Point `p` is the midpoint of segment `a-b`.
    Midpoint { p: Id, a: Id, b: Id },
    /// Tangency: the line `a-b` touches the circle with centre `c` and radius `r`.
    Tangent { a: Id, b: Id, c: Id, r: f64 },
    /// Tangency of two circles or arcs (by centre ids): the centre distance equals r1 + r2 when
    /// `external`, or |r1 - r2| when internal. The radii are solver variables keyed by the centres.
    CircleTangent { c1: Id, c2: Id, external: bool },
    /// Points `a` and `b` are symmetric about the axis `la-lb`.
    Symmetric { a: Id, b: Id, la: Id, lb: Id },
    /// Point `p` lies on the infinite line through `a` and `b`: a point coincident with an edge, an axis
    /// or a construction line.
    PointOnLine { p: Id, a: Id, b: Id },
    /// Two circles (by centre id) have equal radii: their radius variables are equal.
    EqualRadius { c1: Id, c2: Id },
    /// Point `p` lies on the circle or arc centred at point `c` (distance equals the radius variable of the
    /// centre). It also implicitly keeps the endpoints of an arc on its circle, which is what makes an arc
    /// a real entity.
    PointOnCircle { p: Id, c: Id },
    /// Concentric: centres `c1` and `c2` coincide. A distinct kind with its own glyph, not to be confused
    /// with coincidence of ordinary points.
    Concentric { c1: Id, c2: Id },
    /// Angle between two segments `a-b` and `c-d` that share no vertex, in degrees.
    AngleLines {
        a: Id,
        b: Id,
        c: Id,
        d: Id,
        deg: f64,
        #[serde(default)]
        expr: String,
        #[serde(default)]
        driven: bool,
        /// Where the arc of the dimension stands: its radius from the intersection, in sketch units; 0 is the
        /// default distance on screen. Set while the dimension follows the cursor, as `off` of `Distance`.
        #[serde(default)]
        off: f64,
        /// Where the label stands along the arc; see `Angle::at`.
        #[serde(default)]
        at: Option<f64>,
    },
    /// Radius or diameter dimension of the circle centred at `c`. `d` is the value — a radius, or a
    /// diameter when `diam` is set. `off` is the label offset; `expr` and `driven` behave as in `Distance`.
    Diameter {
        c: Id,
        d: f64,
        #[serde(default)]
        off: f64,
        #[serde(default)]
        expr: String,
        #[serde(default)]
        driven: bool,
        #[serde(default)]
        diam: bool,
        /// Where the text stands ON the dimension line, as a share of it from its start (the far rim of a diameter,
        /// the centre of a radius), laid along the line; absent, the text stands on the shelf past the knee.
        #[serde(default)]
        at: Option<f64>,
    },
    /// Dimension: the perpendicular distance from point `p` to the line `a-b` equals `d`. Covers
    /// point-to-line, line-to-parallel-line and distance-to-axis dimensions. `off`, `expr` and `driven`
    /// behave as in `Distance`.
    DistancePL {
        p: Id,
        a: Id,
        b: Id,
        d: f64,
        #[serde(default)]
        off: f64,
        #[serde(default)]
        expr: String,
        #[serde(default)]
        driven: bool,
        /// Where the text stands along the dimension line, as a share of it from its first end: 0.5 in the middle,
        /// below 0 or above 1 past an arrow, the line then running on to the text as a shelf. Absent: the middle, or a
        /// shelf past the second arrow when the text does not fit between them.
        #[serde(default)]
        at: Option<f64>,
    },
    /// Tangent (edge-to-edge) dimension: the distance between `c1` and `c2` corrected by the radii of their
    /// circles. `m1` and `m2` are in {-1, 0, +1}: 0 is an ordinary point, -1 the near edge of the circle,
    /// +1 the far edge. The residual is `dist(c1,c2) + m1*r1 + m2*r2 - d`.
    EdgeDistance {
        c1: Id,
        c2: Id,
        d: f64,
        m1: i8,
        m2: i8,
        #[serde(default)]
        off: f64,
        #[serde(default)]
        expr: String,
        #[serde(default)]
        driven: bool,
        /// Where the text stands along the dimension line; see `Distance::at`.
        #[serde(default)]
        at: Option<f64>,
    },
    /// Arc length dimension: the arc (centre `c`, endpoints `a` and `b`, direction `ccw`) has length `len`,
    /// equal to R * theta for radius R and swept angle theta. A driving one holds the length, a reference
    /// one measures it.
    ArcLength {
        c: Id,
        a: Id,
        b: Id,
        ccw: bool,
        len: f64,
        #[serde(default)]
        off: f64,
        #[serde(default)]
        expr: String,
        #[serde(default)]
        driven: bool,
    },
}

impl Constraint {
    /// Points a constraint references.
    ///
    /// A fact about the constraint itself, so it lives in the core rather than in the interface. The
    /// constraint list uses it to name the participants, and so does anything else that asks what a
    /// constraint holds: deletion, highlighting, diagnostics.
    pub fn points(&self) -> Vec<Id> {
        use Constraint::*;
        match *self {
            Fixed { p } => vec![p],
            Horizontal { a, b } | Vertical { a, b } | Orientation { a, b, .. } | Coincident { a, b } | Distance { a, b, .. } => vec![a, b],
            Parallel { a, b, c, d } | Perpendicular { a, b, c, d } | Equal { a, b, c, d } | Collinear { a, b, c, d } | AngleLines { a, b, c, d, .. } => vec![a, b, c, d],
            Angle { a, b, c, .. } | Midpoint { p: a, a: b, b: c } | PointOnLine { p: a, a: b, b: c } | DistancePL { p: a, a: b, b: c, .. } | ArcLength { c: a, a: b, b: c, .. } => vec![a, b, c],
            Tangent { a, b, c, .. } => vec![a, b, c],
            CircleTangent { c1, c2, .. } | EqualRadius { c1, c2 } | Concentric { c1, c2 } | EdgeDistance { c1, c2, .. } => vec![c1, c2],
            Symmetric { a, b, la, lb } => vec![a, b, la, lb],
            PointOnCircle { p, c } => vec![p, c],
            Diameter { c, .. } => vec![c],
        }
    }

    /// WHERE THE LABEL OF A DIMENSION STANDS - its offset, its place along its line or arc (NaN where there is none) and
    /// the axis of a length - for the fingerprint of the document; `None` for a constraint with no label.
    pub fn label_place(&self) -> Option<[f64; 3]> {
        match *self {
            Constraint::Distance { off, axis, at, .. } => Some([off, at.unwrap_or(f64::NAN), axis as f64]),
            Constraint::DistancePL { off, at, .. } | Constraint::Diameter { off, at, .. } | Constraint::EdgeDistance { off, at, .. } => Some([off, at.unwrap_or(f64::NAN), 0.0]),
            Constraint::ArcLength { off, .. } => Some([off, f64::NAN, 0.0]),
            Constraint::Angle { off, at, .. } | Constraint::AngleLines { off, at, .. } => Some([off, at.unwrap_or(f64::NAN), 0.0]),
            _ => None,
        }
    }

    /// Numeric value of a dimensional constraint (for fingerprints and comparisons); `None` for the rest.
    pub fn dim_value(&self) -> Option<f64> {
        match *self {
            Constraint::Distance { d, .. } | Constraint::DistancePL { d, .. } | Constraint::Diameter { d, .. } | Constraint::EdgeDistance { d, .. } => Some(d),
            Constraint::Angle { deg, .. } | Constraint::AngleLines { deg, .. } => Some(deg),
            Constraint::ArcLength { len, .. } => Some(len),
            _ => None,
        }
    }

    /// The formula a dimension is set by, as typed (empty when a number was typed); `None` for a constraint that is
    /// not a dimension.
    pub fn dim_expr(&self) -> Option<&str> {
        match self {
            Constraint::Distance { expr, .. }
            | Constraint::Angle { expr, .. }
            | Constraint::AngleLines { expr, .. }
            | Constraint::Diameter { expr, .. }
            | Constraint::DistancePL { expr, .. }
            | Constraint::EdgeDistance { expr, .. }
            | Constraint::ArcLength { expr, .. } => Some(expr),
            _ => None,
        }
    }

    /// A reference (driven) dimension or angle: it does not constrain the geometry, only displays a value.
    pub fn is_driven(&self) -> bool {
        matches!(
            self,
            Constraint::Distance { driven: true, .. }
                | Constraint::Angle { driven: true, .. }
                | Constraint::DistancePL { driven: true, .. }
                | Constraint::Diameter { driven: true, .. }
                | Constraint::AngleLines { driven: true, .. }
                | Constraint::ArcLength { driven: true, .. }
                | Constraint::EdgeDistance { driven: true, .. }
        )
    }
}
