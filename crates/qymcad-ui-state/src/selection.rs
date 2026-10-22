//! WHAT IS PICKED: the selection of the window, of the tree, of a sketch and of the geometry.

use super::*;

#[derive(Clone, Copy, PartialEq)]
pub enum Sel {
    None,
    Mesh(usize),
    /// A part's face: (the mesh index, the face index).
    Face(usize, usize),
    /// A contour object (an index into project.contours).
    Contour(usize),
    /// A sketch node (an index into project.sketches).
    Sketch(usize),
    /// A work plane (an index into project.planes).
    Plane(usize),
    /// A datum point (an index into project.datum_points).
    DatumPoint(usize),
    /// A datum axis (an index into project.datum_axes).
    DatumAxis(usize),
    /// A feature of the build tree (a node index into project.timeline).
    Feature(usize),
    /// A component or part (an index into project.components).
    Component(usize),
    /// A mate (by the joint's Id) - selecting the list row and the 3D glyph together, both ways.
    Joint(Id),
    /// An edge of a body: (the body, the persistent id of the edge). Taken by a click on it with nothing in hand.
    Edge(Id, u32),
    /// A corner of a body, as the end of one of its edges: (the body, the edge's id, its far end).
    Vertex(Id, u32, bool),
}

/// DOES THE TEXT TOOL SHOW ITS LABEL AT THE POINTER - where a click would put a new one. Not while a label is being
/// edited: the tool is taken up for the edit, and its ghost followed the pointer over the label being edited. Reported
/// behaviour (found checking issue #32): a double click on a text, and a second copy of it slid about under the cursor.
pub fn text_ghost_shown(armed: &Armed, inline: &InlineEdit) -> bool {
    armed.drawing() == Some(SketchShape::Text) && inline.text().is_none()
}

/// A FIGURE IS TAKEN WHOLE by the tools that carry geometry - move, copy, rotate, mirror, the patterns: a click on one
/// side of a rectangle picked that side alone, and a turn of it was a turn of one line out of a shape held square. Every
/// curve of a rectangle, a polygon or a slot with one curve picked joins the selection.
pub fn take_whole_figures(project: &Project, si: usize, sel_sk: &mut SketchSelection) {
    let picked: Vec<Id> = sel_sk.curves().collect();
    for curve in with_whole_figures(project, si, &picked) {
        if !sel_sk.items.contains(&crate::SketchItem::curve(curve)) {
            sel_sk.items.push(crate::SketchItem::curve(curve));
        }
    }
}

/// The curves `picked`, and every curve of a rectangle, a polygon or a slot one of them belongs to.
fn with_whole_figures(project: &Project, si: usize, picked: &[Id]) -> Vec<Id> {
    let mut out = picked.to_vec();
    let Some(s) = project.sketches.get(si) else { return out };
    // a rectangle by its sides, a polygon by its sides and its circle, a slot by its sides and its ends
    let wholes = s.figures.iter().map(|f| f.curves.clone());
    for curves in wholes.filter(|c| c.iter().any(|e| picked.contains(e))) {
        out.extend(curves.into_iter().filter(|c| !picked.contains(c)));
    }
    out
}

/// THE SPLINES OF SKETCH `si` WHOSE HANDLES SHOW AND ARE TAKEN, by their ids: those picked by their curve, and those with
/// a node picked. A handle shown on every spline of a sketch covered the drawing, and one under a point took the point's
/// drag.
pub fn splines_with_handles(project: &Project, sel_sk: &SketchSelection, si: usize) -> Vec<Id> {
    let Some(s) = project.sketches.get(si) else { return Vec::new() };
    let picked = |e: &qymcad_core::model::SketchEntity, nodes: &[Id]| {
        sel_sk.items.iter().any(|&crate::SketchItem { kind: k, id }| (k == crate::ItemKind::Curve && id == e.id) || (k == crate::ItemKind::Point && nodes.contains(&id)))
    };
    // a copy an offset holds has no handles to take: it is laid again from the curve offset
    let held: std::collections::HashSet<Id> = s.offsets.iter().flat_map(|o| o.copies.iter().copied()).collect();
    s.entities.iter().filter(|e| !held.contains(&e.id) && e.kind.as_spline().is_some_and(|sp| picked(e, sp.nodes))).map(|e| e.id).collect()
}

/// The indices of the texts selected in sketch `si`, in the order they were picked.
pub fn sel_text_indices(project: &Project, sel_sk: &SketchSelection, si: usize) -> Vec<usize> {
    let Some(s) = project.sketches.get(si) else { return Vec::new() };
    sel_sk.items.iter().filter(|crate::SketchItem { kind: k, id: _ }| *k == crate::ItemKind::Text).filter_map(|crate::SketchItem { id, .. }| s.texts.iter().position(|t| t.id == *id)).collect()
}

/// WHAT IS PICKED IN A SKETCH: a point, a curve, a contour or an axis of the sketch, by its id.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SketchItem {
    pub kind: ItemKind,
    pub id: Id,
}

/// THE KIND OF WHAT IS PICKED IN A SKETCH.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ItemKind {
    /// A point: an end, a centre, a node, the origin.
    Point,
    /// A curve: a line, an arc, a circle, an ellipse, a spline.
    Curve,
    /// A closed contour, picked whole.
    Contour,
    /// An axis of the sketch.
    Axis,
    /// A text kept by its id, so the move tools take it, alone or with lines. Reported behaviour (issue #32): a text
    /// was picked apart from the selection, and Move and Rotate never saw it.
    Text,
}

impl SketchItem {
    /// A point by its id.
    pub fn point(id: Id) -> Self {
        SketchItem { kind: ItemKind::Point, id }
    }

    /// A curve by its id.
    pub fn curve(id: Id) -> Self {
        SketchItem { kind: ItemKind::Curve, id }
    }

    /// A closed contour by its id.
    pub fn contour(id: Id) -> Self {
        SketchItem { kind: ItemKind::Contour, id }
    }

    /// An axis of the sketch by its id.
    pub fn axis(id: Id) -> Self {
        SketchItem { kind: ItemKind::Axis, id }
    }

    /// A text by its id.
    pub fn text(id: Id) -> Self {
        SketchItem { kind: ItemKind::Text, id }
    }

    /// The id of the curve, when a curve is what is picked.
    pub fn curve_id(self) -> Option<Id> {
        (self.kind == ItemKind::Curve).then_some(self.id)
    }
}

/// THE SKETCH SELECTION AND THE DEFERRED ACTION ON IT.
///
/// "What is selected" and "what to do once enough has been gathered" are one state: a constraint or an editing
/// tool is pressed and then geometry is clicked. As separate fields this was expressed so that the deferred
/// action could outlive a cleared selection and fire on THE NEXT, unrelated set.
#[derive(Clone, Default, PartialEq)]
pub struct SketchSelection {
    /// what is selected
    pub items: Vec<SketchItem>,
    /// awaiting a set for a constraint (the constraint's code)
    pub constraint: Option<crate::ConstraintKind>,
    /// awaiting a set for an editing tool
    pub modify: Option<EditTool>,
    /// WHAT THE MIRROR IS ABOUT TO REFLECT, while the axis is being pointed at.
    ///
    /// Non-empty means the tool is on its SECOND step: it has been told what, and waits to be told about
    /// what. It used to have no second step at all - the axis was whichever line happened to be in the
    /// selection (and got mirrored too, being in the same set), or silently Y.
    pub mirror_of: Vec<Id>,
    /// THE LINES OF `items` CLICKED AT THEIR MIDDLE, where the triangle of the midpoint showed. Picked as lines; a
    /// constraint that takes points reads them as their midpoints (`qymcad_sketch::middles_for`).
    pub at_middle: Vec<Id>,
    /// THE SIDE OF THE OFFSET a click has fixed, and the distance typed beside the copy (`qymcad_sketch::offset_box`).
    pub offset: OffsetAim,
}

impl SketchSelection {
    /// THE CURVES PICKED, by their ids, in the order they were picked.
    pub fn curves(&self) -> impl Iterator<Item = Id> + '_ {
        self.items.iter().filter(|it| it.kind == ItemKind::Curve).map(|it| it.id)
    }

    /// THE POINTS PICKED, by their ids, in the order they were picked.
    pub fn points(&self) -> impl Iterator<Item = Id> + '_ {
        self.items.iter().filter(|it| it.kind == ItemKind::Point).map(|it| it.id)
    }
}

/// WHAT THE COMMAND IS POINTING AT BY A CLICK RIGHT NOW. The modes are MUTUALLY EXCLUSIVE — in the
/// code that was held by hand, with comments next to it saying "exclusive: reset the others". An enum
/// makes it a property of the type: a second mode simply cannot be turned on without turning the first
/// one off.
#[derive(Clone, Copy, Default, PartialEq)]
pub enum Picking {
    #[default]
    None,
    /// the plane for a NEW sketch; inside it, the face, if a body was clicked
    SketchPlane(Option<Id>),
    /// replacing the plane of an existing sketch (by its index)
    ReplaceSketch(usize),
    /// all the edges of a body, for a fillet
    FilletAll,
    /// a contour into a slot of the command (profile / path / section)
    Contour(ContourSlot),
    /// A TOOL WAITING FOR THE SKETCH IT NEEDS, carrying the kind of command that was pressed.
    ///
    /// Reported behaviour: a tool that needs geometry from a sketch used to write a line into the status
    /// bar and not start. Nobody reads the status bar at the moment of pressing, so what a person saw was
    /// a button that did nothing. The kind travels with the wait because the answer must open the command
    /// that was ASKED FOR - a wait that opens the wrong one is worse than not opening any.
    SketchFor(crate::PartTool),
}

impl Picking {
    pub fn clear(&mut self) {
        *self = Picking::None;
    }
    pub fn is_sketch_plane(&self) -> bool {
        matches!(self, Picking::SketchPlane(_))
    }
    pub fn plane_face(&self) -> Option<Id> {
        match *self {
            Picking::SketchPlane(f) => f,
            _ => None,
        }
    }
    pub fn set_plane_face(&mut self, f: Option<Id>) {
        if self.is_sketch_plane() {
            *self = Picking::SketchPlane(f);
        }
    }
    pub fn replace_sketch(&self) -> Option<usize> {
        match *self {
            Picking::ReplaceSketch(i) => Some(i),
            _ => None,
        }
    }
    pub fn fillet_all(&self) -> bool {
        *self == Picking::FilletAll
    }
    pub fn contour(&self) -> Option<ContourSlot> {
        match *self {
            Picking::Contour(s) => Some(s),
            _ => None,
        }
    }
    /// Which command is waiting for a sketch, if any.
    pub fn sketch_for(&self) -> Option<crate::PartTool> {
        match *self {
            Picking::SketchFor(k) => Some(k),
            _ => None,
        }
    }

    /// THE WORDS FOR "THIS PICK IS CANCELLED", for the modes that Escape simply puts down.
    ///
    /// The Escape ladder used to carry a rung per mode, three of them in a row, each three lines long and
    /// each saying the same two things: clear the pick, name it. That is a hand-written list of the variants
    /// of this very enum living somewhere else - it falls behind the moment a mode is added, silently,
    /// because a mode nobody put in the ladder is simply a pick Escape does not release.
    ///
    /// `None` for the modes that need MORE than clearing: a contour pick returns the borrowed view, and
    /// "fillet all" belongs to the command that armed it.
    pub fn cancel_key(&self) -> Option<&'static str> {
        match *self {
            Picking::ReplaceSketch(_) => Some("in-sketch-move-cancelled"),
            Picking::SketchPlane(_) => Some("in-sketch-plane-cancelled"),
            Picking::SketchFor(_) => Some("in-sketch-wait-cancelled"),
            Picking::FilletAll => Some("in-fillet-all-cancelled"),
            Picking::None | Picking::Contour(_) => None,
        }
    }
}

/// THE SELECTION IN THE TREE: the multiple selection, the range anchor and the rubber-band box.
#[derive(Clone, Default)]
pub struct TreeSelection {
    pub multi: Vec<Id>,
    pub anchor: Option<Id>,
    pub box_start: Option<Pos2>,
}

/// THE PLACES OF THE MODEL CLICKED WITH NOTHING IN HAND, in order - kept to be handed to the tool taken next, as what
/// was selected before it. A click with Shift adds a place, one without starts them anew, a click on nothing or Esc
/// lets them go. The tool takes them as clicks of its own, each by its own rule - an edge of a fillet, a face of a
/// shell, the parts of a joint; a tool that takes the selection as its object (a pattern of the body picked, the
/// mirrored copy of the part picked, body A of a boolean) spends the first place on it.
#[derive(Clone, Default)]
pub struct PickedBefore {
    pub places: Vec<[f64; 3]>,
}

impl PickedBefore {
    /// A click with nothing in hand: on `place`, or on nothing.
    pub fn note(&mut self, place: Option<[f64; 3]>, adding: Adding) {
        match (place, adding) {
            (Some(p), Adding::Yes) => self.places.push(p),
            (Some(p), Adding::No) => self.places = vec![p],
            (None, _) => self.places.clear(),
        }
    }

    /// The places for the tool now in hand, taken once; nothing while no tool is.
    pub fn hand_over(&mut self, held: InHand) -> Vec<[f64; 3]> {
        match held {
            InHand::Nothing => Vec::new(),
            InHand::Tool => std::mem::take(&mut self.places),
        }
    }

    /// The tool took the selection as its object: the place that selected it is spent.
    pub fn spend_object(&mut self) {
        if !self.places.is_empty() {
            self.places.remove(0);
        }
    }
}

/// SHIFT HOLDS THE BODIES CLICKED WITH NOTHING IN HAND: the body of the selection before the click (`was`) and the
/// body under the pointer (`under`) are both held, as rows picked of the tree, the way a boolean takes its two bodies; a
/// click without Shift lets the bodies held go. The body under the pointer rather than the one of what the click picked:
/// a click near the seam of two pieces picks the edge of the other one. Reported behaviour: Shift with nothing in hand
/// kept only the body clicked last, and a boolean of two bodies could not be taken over a selection.
///
/// A SKETCH SELECTED BEFORE AN EDGE PICKED WITH SHIFT is held as its row: a sweep takes the profile and the edge of its
/// path picked together before it. Only then - a sketch selected and a face or a body picked with Shift holds nothing,
/// so what a group of rows deletes or a boolean of the tree takes stays as it was.
pub fn hold_bodies(project: &Project, was: Sel, under: Option<Id>, chosen: &mut Chosen, adding: Adding) {
    if let (Adding::Yes, Sel::Sketch(si), Some(_), Sel::Edge(..)) = (adding, was, under, chosen.sel) {
        if let Some(id) = project.sketches.get(si).map(|s| s.id).filter(|id| !chosen.tree_sel.multi.contains(id)) {
            chosen.tree_sel.multi.push(id);
        }
        return;
    }
    let body = |s: Sel| match s {
        Sel::Mesh(mi) | Sel::Face(mi, _) => project.mesh_id(mi),
        Sel::Edge(b, _) | Sel::Vertex(b, _, _) => Some(b),
        _ => None,
    };
    let multi = &mut chosen.tree_sel.multi;
    match (adding, body(was), under) {
        (Adding::Yes, Some(a), Some(b)) if a != b => {
            if !multi.contains(&a) {
                *multi = vec![a];
            }
            if !multi.contains(&b) {
                multi.push(b);
            }
        }
        (Adding::Yes, _, _) => {}
        (Adding::No, _, _) => multi.retain(|id| project.mesh_index(*id).is_none()),
    }
}

/// A JOINT OF THE LIST PICKED WITH NOTHING IN HAND: with Shift or Ctrl it is held with the joint selected before - a
/// relation is made of two - and a second pick of one held lets it go; without, it is the selection alone. Reported
/// behaviour: the list held one joint at a time, and a relation could not be taken over a selection.
pub fn hold_joint(project: &Project, sel: &mut Sel, held: &mut Vec<Id>, joint: Id, adding: Adding) {
    let is_joint = |id: &Id| project.joints.iter().any(|j| j.id == *id);
    held.retain(is_joint);
    match (adding, *sel) {
        (Adding::Yes, Sel::Joint(was)) if was != joint => {
            if held.is_empty() {
                held.push(was);
            }
            match held.iter().position(|id| *id == joint) {
                Some(at) => {
                    held.remove(at);
                }
                None => held.push(joint),
            }
        }
        _ => held.clear(),
    }
    *sel = Sel::Joint(joint);
}

/// WHETHER A CLICK ADDS TO WHAT IS PICKED - Shift held - or starts it anew.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Adding {
    Yes,
    No,
}

/// THE GEOMETRY SELECTED FOR COMMANDS: profiles, faces, edges. "Faces of which body" used to live apart
/// from the faces themselves, and the set could outlive a change of body.
#[derive(Clone, Default)]
pub struct GeomSelection {
    pub profiles: std::collections::HashSet<Id>,
    pub faces: std::collections::HashSet<u32>,
    pub faces_body: Option<Id>,
    pub edges: std::collections::HashSet<u32>,
    /// A DESCRIPTION OF THE SELECTION instead of a snapshot.
    ///
    /// A reference remembers NOT ONLY WHAT was selected but also HOW. Click an edge and it is that one
    /// edge, a list. Click a face, or take the "expand selection" menu item, and the intent is a
    /// different one: "all the edges of this face", "everything parallel to this one" — and it must
    /// outlive an edit after which there are more elements than before. Then a query sits here, and the
    /// feature records the query rather than today's numbers.
    ///
    /// Touch even one element on its own and the description collapses and this becomes empty: "all the
    /// edges of the face except this one" is not something we can express yet, and a description must
    /// not lie.
    pub described: Option<qymcad_core::refs::Query>,
    /// What was picked on the model before the tool in hand was taken.
    pub before: PickedBefore,
    /// THE LAST FACE CLICKED, together with its body.
    ///
    /// The "expand selection" menu needs it. In a fillet, clicking a face puts its EDGES into the set
    /// and the face itself is kept nowhere — so the menu, which asked `faces`, saw emptiness and did
    /// not open at all. Reported behaviour: pressing the right button did nothing.
    pub last_face: Option<(u32, Id)>,
    /// THE LAST EDGE UNDER THE CURSOR, together with its body. The same as `last_face`, but for the
    /// edge items of the menu: "the whole tangent chain" is asked for while hovering an edge, not a face.
    ///
    /// They are mutually exclusive: whatever was under the cursor last is what the menu offers.
    /// Otherwise we would have to guess what is being asked about — and it has already been pointed at.
    pub last_edge: Option<(u32, Id)>,
    /// WAITING FOR THE SECOND PICK of a "between": the FIRST face of it sits here.
    ///
    /// `Between` is the only query for which one element pointed at is not enough: a seam belongs to TWO
    /// sets. So the menu item does not create the reference straight away but puts the command into
    /// "now click the other side" mode.
    pub between_first: Option<u32>,
    /// the constraint selected in the sketch list
    pub constraint: Option<usize>,
}

impl GeomSelection {
    /// ADD A FACE TO THE DESCRIPTION "all the edges of these faces".
    ///
    /// It accumulates as a union: two faces can be clicked in a row, and both go into one description.
    /// If a description of a DIFFERENT kind was there before (say "everything parallel"), it is
    /// replaced — different intents must not be mixed silently.
    pub fn describe_edges_of_face(&mut self, fid: u32) {
        self.described = Some(match self.described.take() {
            Some(Query::Adjacent(inner)) => Query::Adjacent(Box::new(Query::Union(inner, Box::new(Query::Id(fid))))),
            _ => Query::Adjacent(Box::new(Query::Id(fid))),
        });
    }

    /// THE DESCRIPTION AS A FEATURE RECORDS IT, when one was given: it survives an edit that adds elements.
    pub fn described_ref(&self) -> Option<qymcad_core::refs::Ref> {
        self.described.clone().map(qymcad_core::refs::Ref::many)
    }

    /// THE SELECTION AS A FEATURE RECORDS IT: the description when one was given, otherwise the list `picks` of what
    /// was clicked. One rule for every command that records a selection, on creation and on an edit alike.
    pub fn recorded(&self, picks: &[u32]) -> qymcad_core::refs::Ref {
        self.described_ref().unwrap_or_else(|| qymcad_core::refs::Ref::picks(picks))
    }
}

pub fn sel_point_ids(sel_sk: &SketchSelection) -> Vec<Id> {
    sel_sk.points().collect()
}

/// THE SHAPES THE CHOICE HOLDS for the tools that carry shapes whole - Move, Copy, Rotate, Mirror and the patterns: its
/// curves with the figures they belong to, and its points that stand alone. A point that ends a curve is a place on that curve, and the origin, the axes
/// and a projection are not the sketch's to carry.
pub fn shapes_chosen(project: &Project, si: usize, sel_sk: &SketchSelection) -> Vec<Id> {
    let not_alone = points_not_alone(project, si);
    // a figure with one curve chosen is carried whole, as `take_whole_figures` shows it
    let curves: Vec<Id> = sel_sk.curves().collect();
    let points = sel_sk.items.iter().filter(|&&crate::SketchItem { kind: k, id }| k == crate::ItemKind::Point && !not_alone.contains(&id)).map(|crate::SketchItem { id, .. }| *id);
    with_whole_figures(project, si, &curves).into_iter().chain(points).collect()
}

/// Whether the point stands alone in the sketch: no curve ends or turns about it, and it is not the origin, an axis or
/// a projection. See `shapes_chosen`.
pub fn point_alone(project: &Project, si: usize, pid: Id) -> bool {
    project.sketches.get(si).is_some_and(|s| s.points.iter().any(|p| p.id == pid)) && !points_not_alone(project, si).contains(&pid)
}

fn points_not_alone(project: &Project, si: usize) -> std::collections::HashSet<Id> {
    let Some(s) = project.sketches.get(si) else { return Default::default() };
    // the centre of a rectangle and the middle of a line are places on a shape, held there, not shapes
    let middles = s.constraints.iter().filter_map(|c| if let qymcad_core::model::Constraint::Midpoint { p, .. } = c { Some(*p) } else { None });
    s.entities.iter().flat_map(qymcad_core::model::entity_points).chain(s.immovable_points()).chain(s.rects().iter().map(|r| r.centre)).chain(middles).collect()
}

/// The centre of the selected object (for the gizmo), in the part's XY coordinates.
pub fn selected_centroid(project: &Project, sel: &Sel) -> Option<Point2> {
    match *sel {
        Sel::Contour(i) => project.contours.get(i).map(|c| c.centroid()),
        Sel::Mesh(i) => project.bodies.get(i).map(|b| &b.mesh).and_then(|m| m.bounds()).map(|b| Point2::new((b.min.x + b.max.x) / 2.0, (b.min.y + b.max.y) / 2.0)),
        _ => None,
    }
}

pub fn sel_line_pts(project: &Project, sel_sk: &SketchSelection, si: usize) -> Vec<(Id, Id)> {
    let Some(s) = project.sketches.get(si) else { return Vec::new() };
    sel_sk.items.iter().filter(|it| it.kind == crate::ItemKind::Curve).filter_map(|crate::SketchItem { id: eid, .. }| s.entity(*eid)?.kind.as_line().map(|l| (l.a, l.b))).collect()
}

/// The (centre, radius) of the first selected circle entity — for a tangency.
pub fn sel_circle_cr(project: &Project, sel_sk: &SketchSelection, si: usize) -> Option<(Id, f64)> {
    let s = project.sketches.get(si)?;
    // an arc: the radius is the distance from the centre to an end (the tangency uses the arc's LIVE radius variable)
    sel_sk
        .items
        .iter()
        .filter(|it| it.kind == crate::ItemKind::Curve)
        .find_map(|crate::SketchItem { id: eid, .. }| s.entity(*eid).and_then(|e| qymcad_core::model::circle_of(s, e)).map(|c| (c.center, c.r)))
}

/// The ends of an existing LINE on which the point `p` lies (somewhere along it, not at an end) — for
/// the automatic point-on-edge. Excludes the line (ea, eb) and any line where `p` is an end.
pub fn line_under_point(project: &Project, view: &View2d, si: usize, p: Point2, ea: Id, eb: Id) -> Option<(Id, Id)> {
    let s = project.sketches.get(si)?;
    let points_by_id: std::collections::HashMap<Id, &qymcad_core::model::SketchPoint> = s.points.iter().map(|p| (p.id, p)).collect(); // a table: these are looked up for every entity or constraint
    let pt = |id: Id| points_by_id.get(&id).copied().map(|q| Point2::new(q.x, q.y));
    for e in &s.entities {
        if let Some(qymcad_core::model::LineParts { a, b }) = e.kind.as_line() {
            if (a == ea && b == eb) || (a == eb && b == ea) {
                continue; // our own line
            }
            let (Some(pa), Some(pb)) = (pt(a), pt(b)) else { continue };
            // p must not coincide with an end of the edge
            let near_end = ((p.x - pa.x).powi(2) + (p.y - pa.y).powi(2)).sqrt() < 1e-3 || ((p.x - pb.x).powi(2) + (p.y - pb.y).powi(2)).sqrt() < 1e-3;
            if near_end {
                continue;
            }
            if let Some(proj) = project_on_seg(p, pa, pb) {
                let dist_world = ((p.x - proj.x).powi(2) + (p.y - proj.y).powi(2)).sqrt();
                // it is on the edge when it is within about 2 px on screen
                if dist_world * view.scale as f64 <= 2.0 {
                    return Some((a, b));
                }
            }
        }
    }
    None
}

/// Whether the segments a1 -> b1 and a2 -> b2 are parallel (their normalised cross product is under 5%).
pub fn lines_parallel(dc: &DrawCtx, si: usize, a1: Id, b1: Id, a2: Id, b2: Id) -> bool {
    match (sketch_pt(dc.project, si, a1), sketch_pt(dc.project, si, b1), sketch_pt(dc.project, si, a2), sketch_pt(dc.project, si, b2)) {
        (Some(p1), Some(p2), Some(p3), Some(p4)) => {
            let (u, v) = ((p2.x - p1.x, p2.y - p1.y), (p4.x - p3.x, p4.y - p3.y));
            let cross = u.0 * v.1 - u.1 * v.0;
            let (lu, lv) = ((u.0 * u.0 + u.1 * u.1).sqrt(), (v.0 * v.0 + v.1 * v.1).sqrt());
            cross.abs() / (lu * lv).max(1e-9) < 0.05
        }
        _ => false,
    }
}

/// The kind a shape is chosen as: a point (0) where the id names a point of the sketch, a curve (1) otherwise.
pub fn kind_of_shape(project: &Project, si: usize, id: Id) -> ItemKind {
    if project.sketches.get(si).is_some_and(|s| s.points.iter().any(|p| p.id == id)) {
        ItemKind::Point
    } else {
        ItemKind::Curve
    }
}
