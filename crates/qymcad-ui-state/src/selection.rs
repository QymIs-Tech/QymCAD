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

/// THE KIND OF A TEXT in the sketch selection (`SketchSelection::items`), beside a point (0), an entity (1) and an axis
/// (3): a text kept by its id, so the move tools take it, alone or with lines. Reported behaviour (issue #32): a text was
/// picked apart from the selection, and Move and Rotate never saw it.
pub const SEL_TEXT: u8 = 4;

/// DOES THE TEXT TOOL SHOW ITS LABEL AT THE POINTER - where a click would put a new one. Not while a label is being
/// edited: the tool is taken up for the edit, and its ghost followed the pointer over the label being edited. Reported
/// behaviour (found checking issue #32): a double click on a text, and a second copy of it slid about under the cursor.
pub fn text_ghost_shown(armed: &Armed, inline: &InlineEdit) -> bool {
    armed.draw_kind() == 11 && inline.text().is_none()
}

/// A RECTANGLE IS TAKEN WHOLE by the tools that carry geometry - move, copy, rotate, the patterns: a click on one side of a
/// rectangle picked that side alone, and a turn of it was a turn of one line out of a shape held square. Every side of
/// a rectangle with one side picked joins the selection.
pub fn take_whole_rects(project: &Project, si: usize, sel_sk: &mut SketchSelection) {
    let Some(s) = project.sketches.get(si) else { return };
    let picked: Vec<Id> = sel_sk.items.iter().filter(|(k, _)| *k == 1).map(|(_, id)| *id).collect();
    for r in s.rects.iter().filter(|r| r.sides.iter().any(|e| picked.contains(e))) {
        for side in r.sides {
            if !picked.contains(&side) {
                sel_sk.items.push((1, side));
            }
        }
    }
}

/// THE KIND OF A SPLINE in the sketch selection, kept by the id of its first node: a spline has no id of its own, and no
/// entity has the id of a point. Picked by a click on its curve; its handles show while it is picked.
pub const SEL_SPLINE: u8 = 5;

/// THE SPLINES OF SKETCH `si` WHOSE HANDLES SHOW AND ARE TAKEN: those picked by their curve, and those with a node
/// picked. A handle shown on every spline of a sketch covered the drawing, and one under a point took the point's drag.
pub fn splines_with_handles(project: &Project, sel_sk: &SketchSelection, si: usize) -> Vec<usize> {
    let Some(s) = project.sketches.get(si) else { return Vec::new() };
    let picked = |sp: &qymcad_core::model::Spline| sel_sk.items.iter().any(|&(k, id)| (k == SEL_SPLINE && sp.points.first() == Some(&id)) || (k == 0 && sp.points.contains(&id)));
    s.splines.iter().enumerate().filter(|(_, sp)| picked(sp)).map(|(spi, _)| spi).collect()
}

/// The indices of the texts selected in sketch `si`, in the order they were picked.
pub fn sel_text_indices(project: &Project, sel_sk: &SketchSelection, si: usize) -> Vec<usize> {
    let Some(s) = project.sketches.get(si) else { return Vec::new() };
    sel_sk.items.iter().filter(|(k, _)| *k == SEL_TEXT).filter_map(|(_, id)| s.texts.iter().position(|t| t.id == *id)).collect()
}

/// THE SKETCH SELECTION AND THE DEFERRED ACTION ON IT.
///
/// "What is selected" and "what to do once enough has been gathered" are one state: a constraint or an editing
/// tool is pressed and then geometry is clicked. As separate fields this was expressed so that the deferred
/// action could outlive a cleared selection and fire on THE NEXT, unrelated set.
#[derive(Clone, Default, PartialEq)]
pub struct SketchSelection {
    /// what is selected: (the kind - a point, an entity and so on - plus its Id)
    pub items: Vec<(u8, Id)>,
    /// awaiting a set for a constraint (the constraint's code)
    pub constraint: Option<u8>,
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
    SketchFor(u8),
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
    pub fn sketch_for(&self) -> Option<u8> {
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
    sel_sk.items.iter().filter(|(k, _)| *k == 0).map(|(_, id)| *id).collect()
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
    use qymcad_core::model::EntityKind;
    let Some(s) = project.sketches.get(si) else { return Vec::new() };
    sel_sk
        .items
        .iter()
        .filter(|(k, _)| *k == 1)
        .filter_map(|(_, eid)| {
            s.entities.iter().find(|e| e.id == *eid).and_then(|e| match e.kind {
                EntityKind::Line { a, b } => Some((a, b)),
                _ => None,
            })
        })
        .collect()
}

/// The (centre, radius) of the first selected circle entity — for a tangency.
pub fn sel_circle_cr(project: &Project, sel_sk: &SketchSelection, si: usize) -> Option<(Id, f64)> {
    use qymcad_core::model::EntityKind;
    let s = project.sketches.get(si)?;
    let pos = |id: Id| s.points.iter().find(|q| q.id == id).map(|q| (q.x, q.y));
    sel_sk.items.iter().filter(|(k, _)| *k == 1).find_map(|(_, eid)| {
        s.entities.iter().find(|e| e.id == *eid).and_then(|e| match e.kind {
            EntityKind::Circle { center, r } => Some((center, r)),
            // an arc: the radius is the distance from the centre to an end (the tangency uses the arc's LIVE radius variable)
            EntityKind::Arc { center, a, .. } => match (pos(center), pos(a)) {
                (Some((cx, cy)), Some((ax, ay))) => Some((center, ((ax - cx).powi(2) + (ay - cy).powi(2)).sqrt())),
                _ => None,
            },
            _ => None,
        })
    })
}

/// The ends of an existing LINE on which the point `p` lies (somewhere along it, not at an end) — for
/// the automatic point-on-edge. Excludes the line (ea, eb) and any line where `p` is an end.
pub fn line_under_point(project: &Project, view: &View2d, si: usize, p: Point2, ea: Id, eb: Id) -> Option<(Id, Id)> {
    use qymcad_core::model::EntityKind;
    let s = project.sketches.get(si)?;
    let points_by_id: std::collections::HashMap<Id, &qymcad_core::model::SketchPoint> = s.points.iter().map(|p| (p.id, p)).collect(); // a table: these are looked up for every entity or constraint
    let pt = |id: Id| points_by_id.get(&id).copied().map(|q| Point2::new(q.x, q.y));
    for e in &s.entities {
        if let EntityKind::Line { a, b } = e.kind {
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
