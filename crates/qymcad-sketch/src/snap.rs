//! WHERE THE POINTER SNAPS in a sketch.

use super::*;

/// Snapping the cursor: a vertex or a centre (always), then a midpoint, an intersection, a point on an
/// edge, an axis, a grid node. It sets `snap_hint` (the type for the glyph) and returns a world point.
/// The type codes: 0 vertex, 1 grid, 2 axis, 3 midpoint, 4 centre, 5 intersection, 6 on an edge.
pub fn snap_world(sk: &mut qymcad_ui_state::SketchCtx, rect: Rect, screen: Pos2) -> Point2 {
    let sh = qymcad_ui_state::Sheet { view: *sk.view, rect };
    let w = qymcad_ui_state::to_world(&*sk.view, rect, screen);
    // "No ties" on the bar of a drawing tool: the point goes where the pointer is, and the preview snaps to nothing
    if sk.armed.drawing().is_some() && sk.tool.ties == qymcad_ui_state::Ties::Free {
        *sk.snap_hint = None;
        return Point2::new(w.x, w.y);
    }
    let sd = |p: Point2, view: &qymcad_ui_state::View2d| qymcad_ui_state::Sheet { view: *view, rect }.at(p).distance(screen);
    // the centres of the circles and arcs of the active sketch, so a centre can be told from a plain vertex
    let centers: std::collections::HashSet<u64> = qymcad_ui_state::edit_si(&*sk.project, &*sk.sketch_ses)
        .and_then(|si| sk.project.sketches.get(si))
        .map(|s| s.entities.iter().filter_map(|e| e.kind.round_centre()).collect())
        .unwrap_or_default();
    // THE VERTICES of existing geometry snap ALWAYS - that is topology. Type 4 marks the centres.
    // THE ACTIVE SKETCH ONLY: every sketch and every contour of the project used to be walked, so the
    // cursor stuck to the vertices of other people's sketches (on different planes the 2D coordinates
    // coincide) and to the tessellation of every circle in the project - random snaps to the wrong place
    // plus a cost of O(the whole model) per frame. The reference geometry of neighbours and of the host
    // face arrives separately, below, through `qymcad_pick::sketch_ref_edges_2d`, already projected into the sketch.
    let mut best: Option<(f32, Point2, u8)> = None;
    // A POINT BEING DRAGGED DOES NOT SNAP TO ITSELF. Its own place, and whatever stands on it (a coincident end, the
    // vertex of the contour drawn through it), is a vertex within reach until the hand is 9 px away: a point led
    // at 3 px a frame stuck, moved in jerks and was let go short - measured, 1 mm short of a 7 mm drag.
    //
    // NOR TO WHAT GOES WITH IT: the corners of a rectangle dragged by its centre, and every line ending at a point that
    // moves. They stand where the drag was a frame ago, and the middle of the rectangle's own diagonal - its centre
    // a frame back - caught the centre: a rectangle led 12 mm by its centre came 10.7.
    let moving: Vec<Point2> = sk
        .drag
        .pt()
        .and_then(|(dsi, pi)| {
            let s = sk.project.sketches.get(dsi)?;
            let q = s.points.get(pi)?;
            let at = |id: Id| s.points.iter().find(|p| p.id == id).map(|p| Point2::new(p.x, p.y));
            Some(std::iter::once(Point2::new(q.x, q.y)).chain(s.rects().iter().filter(|r| r.centre == q.id).flat_map(|r| r.corners.into_iter().filter_map(at))).collect())
        })
        .unwrap_or_default();
    let own = |p: Point2| moving.iter().any(|q| (p.x - q.x).abs() < 1e-9 && (p.y - q.y).abs() < 1e-9);
    if let Some(asi) = qymcad_ui_state::edit_si(&*sk.project, &*sk.sketch_ses) {
        if let Some(s) = sk.project.sketches.get(asi) {
            for sp in &s.points {
                let p = Point2::new(sp.x, sp.y);
                if own(p) {
                    continue;
                }
                let d = sd(p, &*sk.view);
                let ty = if centers.contains(&sp.id) { 4 } else { 0 };
                if best.is_none_or(|(bd, _, _)| d < bd) {
                    best = Some((d, p, ty));
                }
            }
            for cid in &s.contour_ids {
                let Some(ci) = sk.project.contour_index(*cid) else { continue };
                // NOR TO THE CURVE IT DRAGS ALONG: a spline drawn through a dragged node runs under the cursor, and
                // the points it is cut into caught the node - the middle node of a spline led to (20, 25) stopped at
                // (18.7, 24.8). The true corners of such a contour are points of the sketch, snapped above.
                let c = &sk.project.contours[ci];
                if c.points.iter().any(|p| own(*p)) {
                    continue;
                }
                // WHERE TWO CURVES MEET ON A LOOP, and nowhere else along it: the end of an edge whose neighbour comes
                // from another curve - a crossing, the ends of the curves being points of the sketch, snapped above. The
                // points a circle or a spline is cut into were snapped as vertices, a glyph of a coincidence was shown
                // on a rim and the click was tied to nothing there.
                let n = c.edges.len();
                for k in (0..n).filter(|k| c.closed || k + 1 < n) {
                    if c.edge_src.get(k) == c.edge_src.get((k + 1) % n) {
                        continue;
                    }
                    // the end of the edge; a circle and a whole ellipse have none
                    let Some([_, p]) = c.edges[k].ends() else { continue };
                    let d = sd(p, &*sk.view);
                    if best.is_none_or(|(bd, _, _)| d < bd) {
                        best = Some((d, p, 5));
                    }
                }
            }
        }
    }
    // the edges of the REFERENCE body (a face of its own part, or a neighbour during the creation
    // session), projected into the sketch. THE ENDS of those edges (the corners of the outline) are
    // VERTICES (type 0, taking priority). The nearest point ON an edge and the INTERSECTIONS with it come
    // below, in the inference: a point on an edge used to stand as a vertex and SHORT-CIRCUITED the rest,
    // making it impossible to snap to the intersection of a construction line with the outline of a part.
    let ref_edges =
        qymcad_ui_state::edit_si(&*sk.project, &*sk.sketch_ses).map(|si| qymcad_pick::sketch_ref_edges_2d(&*sk.cache, &*sk.cmd, &*sk.live, &*sk.project, &*sk.regen, si)).unwrap_or_default();
    for poly in &ref_edges {
        for end in [poly.first(), poly.last()].into_iter().flatten() {
            let d = sd(*end, &*sk.view);
            if best.is_none_or(|(bd, _, _)| d < bd) {
                best = Some((d, *end, 0));
            }
        }
    }
    // THE FIRST NODE OF A SPLINE BEING DRAWN is a vertex to snap to, from the third node on: a click there lands on it and
    // closes the spline. It is no point of the sketch yet and was no target, and a click closed the spline only within
    // 0.4 mm of it - 2.4 px at the 6 px to the millimetre of a new sketch. Reported behaviour: "the point on the mouse does
    // not snap to the start of the spline, it is hard to hit".
    if sk.armed.drawing() == Some(qymcad_ui_state::SketchShape::Spline) && sk.tool.pts.len() >= 3 {
        if let Some(&first) = sk.tool.pts.first() {
            let d = sd(first, &*sk.view);
            if best.is_none_or(|(bd, _, _)| d < bd) {
                best = Some((d, first, 0));
            }
        }
    }
    // THE PLACE A LINE BEING DRAWN TOUCHES A CURVE, nearer the pointer than any vertex, before the vertex: on a closed
    // spline the place a line from above touched it stood 1.2 mm off a node, and the node 7 px off the pointer took the
    // end of the line
    let start = (sk.armed.drawing() == Some(qymcad_ui_state::SketchShape::Line)).then(|| sk.tool.pts.last().copied()).flatten();
    if let (Some(from), Some(si)) = (start, qymcad_ui_state::edit_si(&*sk.project, &*sk.sketch_ses)) {
        let reach = f64::from(qymcad_ui_state::grab::grab(sk.set, Grab::Snap)) / f64::from(sk.view.scale);
        let touch = curve_snaps(&sk.project.sketches[si], CurveSnapAsk { at: Point2::new(w.x, w.y), start: Some(from), reach, moving: &[] })
            .into_iter()
            .filter(|f| f.rank == 2)
            .map(|snap| Offered { off: sd(snap.at, &*sk.view), snap })
            .filter(|o| o.off <= qymcad_ui_state::grab::grab(sk.set, Grab::Snap) && best.is_none_or(|(bd, _, _)| o.off < bd))
            .min_by(|a, b| a.off.total_cmp(&b.off));
        if let Some(o) = touch {
            *sk.snap_hint = Some((o.snap.at, o.snap.kind));
            return o.snap.at;
        }
    }
    if let Some((d, p, ty)) = best {
        if d <= qymcad_ui_state::grab::grab(sk.set, Grab::Snap) {
            *sk.snap_hint = Some((p, ty));
            return p;
        }
    }
    if !sk.set.snap.on {
        *sk.snap_hint = None;
        return w;
    }
    // the inference snaps of the active sketch: a midpoint, then an intersection, then a point on an edge.
    // `cand` holds (the screen distance, the point, the type); a smaller (type, distance) wins.
    let mut cand: Option<(f32, Point2, u8)> = None;
    let mut curve_cand: Option<Offered> = None;
    if let Some(si) = qymcad_ui_state::edit_si(&*sk.project, &*sk.sketch_ses) {
        // ONLY WHAT RUNS NEAR THE POINTER: every point offered below - a middle, a crossing, a point on a curve - lies on
        // its curve and is taken within the reach of a snap, so a curve that passes further than that from the pointer
        // offers nothing. Every pair of 70 000 segments, intersected each frame, took 15 s.
        let reach = qymcad_ui_state::grab::grab(sk.set, Grab::Snap) + 1.0;
        let reach_mm = f64::from(reach) / f64::from(sk.view.scale);
        let near_seg = |a: Point2, b: Point2| qymcad_ui_state::screen_dist_seg(screen, sh.at(a), sh.at(b)) <= reach;
        // THE CURVES OF THE SKETCH NEAR THE POINTER, of every kind alike - what moves with a drag is no target
        let s = &sk.project.sketches[si];
        let goes = |id: &Id| s.point(*id).is_some_and(|q| own(Point2::new(q.x, q.y)));
        let curves: Vec<qymcad_core::model::curve::Curve> = s
            .entities
            .iter()
            .filter(|e| !qymcad_core::model::entity_points(e).iter().any(goes))
            .filter_map(|e| qymcad_core::model::curve::Curve::of(s, e))
            .filter(|c| c.nearest(w).off <= reach_mm)
            .collect();
        // the segments of the projected outlines of the reference body, crossed with the curves of the sketch and taken a
        // point on: that is how the crossing of a construction line with a face or the outline of a part is snapped to
        let ref_segs: Vec<(Point2, Point2)> = ref_edges.iter().flat_map(|poly| poly.windows(2).map(|s| (s[0], s[1]))).filter(|(a, b)| near_seg(*a, *b)).collect();
        let grab_snap = qymcad_ui_state::grab::grab(sk.set, Grab::Snap);
        let view = *sk.view;
        let mut offer = |rank: u8, p: Point2| {
            let d = sd(p, &view);
            if d <= grab_snap && cand.is_none_or(|(bd, _, bty)| rank.cmp(&bty).then(d.total_cmp(&bd)).is_lt()) {
                cand = Some((d, p, rank));
            }
        };
        // the priority: a middle (3) over a crossing (5) over a point on a curve (6)
        // 1) THE MIDDLE OF A STRAIGHT; the middle of any other curve comes with the snaps of the curves below
        for c in curves.iter().filter(|c| c.straight()) {
            offer(3, c.at(0.5));
        }
        // 2) THE CROSSINGS OF THE CURVES WITH THE OUTLINE OF THE BODY, with the axes X = 0 and Y = 0, and with the centre
        // lines of the circles - of a curve of any kind. Reported behaviour: "a circle of construction geometry has a
        // snap, but the cursor is not pulled to the X and Y axes, which makes it awkward to put the centre of a new circle
        // on the construction circle"; the axes and the centre lines were crossed with lines and circles alone.
        let axes = [
            [Point2::new(w.x - 4.0 * reach_mm, 0.0), Point2::new(w.x + 4.0 * reach_mm, 0.0)],
            [Point2::new(0.0, w.y - 4.0 * reach_mm), Point2::new(0.0, w.y + 4.0 * reach_mm)],
        ];
        let centre_lines: Vec<[Point2; 2]> = qymcad_ui_state::circle_centre_lines(&*sk.project, si).iter().filter(|cl| near_seg(cl.from, cl.to)).map(|cl| [cl.from, cl.to]).collect();
        for c in &curves {
            for seg in ref_segs.iter().map(|&(a, b)| [a, b]).chain(axes).chain(centre_lines.iter().copied()) {
                for p in c.meets_segment(seg) {
                    offer(5, p);
                }
            }
        }
        for axis in [0u8, 1] {
            for (a, b) in &ref_segs {
                if let Some(p) = qymcad_ui_state::seg_axis_intersect(*a, *b, axis) {
                    offer(5, p);
                }
            }
        }
        // 3) A POINT ON A STRAIGHT OR ON THE OUTLINE OF A PART, plus A TIE TO THE GRID ALONG it (type 5, which outranks a
        // plain point on a curve): across it sticks to the straight, along it to the nearest grid line, so the crossing of
        // a face and the grid can be hit exactly
        let gsz = sk.set.snap.grid.max(0.1);
        let straights: Vec<(Point2, Point2)> = curves.iter().filter(|c| c.straight()).map(|c| (c.at(0.0), c.at(1.0))).collect();
        for (a, b) in straights.iter().chain(ref_segs.iter()) {
            if let Some(p) = qymcad_ui_state::project_on_seg(w, *a, *b) {
                if let Some(gpt) = qymcad_ui_state::grid_cross_on_seg(p, *a, *b, gsz) {
                    offer(5, gpt);
                }
                offer(6, p);
            }
        }
        // 2d) A POINT ALONG A CENTRE LINE, as a point on a curve (6) - only within it: its ends, a quarter of the radius past
        // the rim, are no places to snap to, and a centre of a circle clicked 0.5 mm past one was pulled onto it. Not tied
        // to the grid along it as a straight of the sketch is: lying over an axis, a centre line would then offer a node of
        // the grid beside the crossing of the axis and a circle.
        for [from, to] in &centre_lines {
            let (u, v) = (w - *from, *to - *from);
            let t = (u.x * v.x + u.y * v.y) / (v.x * v.x + v.y * v.y);
            if (0.0..=1.0).contains(&t) {
                offer(6, Point2::new(from.x + v.x * t, from.y + v.y * t));
            }
        }
        // 2c) THE CURVES OF ANY KIND near the pointer (`curve_snaps`): where they cross, their quadrants and middles, the
        // point nearest the pointer, the place a line being drawn touches them - the nearest of the best rank, weighed
        // against the rest once they are all found, by its rank and not by what it says
        let start = (sk.armed.drawing() == Some(qymcad_ui_state::SketchShape::Line)).then(|| sk.tool.pts.last().copied()).flatten();
        for found in curve_snaps(s, CurveSnapAsk { at: w, start, reach: reach_mm, moving: &moving }) {
            let d = sd(found.at, &*sk.view);
            if d <= grab_snap && curve_cand.as_ref().is_none_or(|b| found.rank.cmp(&b.snap.rank).then(d.total_cmp(&b.off)).is_lt()) {
                curve_cand = Some(Offered { off: d, snap: found });
            }
        }
    }
    if let Some(Offered { off: d, snap: found }) = curve_cand {
        if cand.is_none_or(|(bd, _, bty)| found.rank.cmp(&bty).then(d.total_cmp(&bd)).is_lt()) {
            cand = Some((d, found.at, found.kind));
        }
    }
    if let Some((_, p, ty)) = cand {
        *sk.snap_hint = Some((p, ty));
        return p;
    }
    // snapping to the X = 0 and Y = 0 axes and to the origin. The free coordinate ALONG the axis is still
    // pulled towards the grid nodes (otherwise snapping by cells disappeared along an axis).
    let g = sk.set.snap.grid.max(0.1);
    let snap_to_grid = |v: f64| (v / g).round() * g;
    let near_x0 = (sh.at(Point2::new(0.0, w.y)).x - screen.x).abs() <= qymcad_ui_state::grab::grab(sk.set, Grab::Snap);
    let near_y0 = (sh.at(Point2::new(w.x, 0.0)).y - screen.y).abs() <= qymcad_ui_state::grab::grab(sk.set, Grab::Snap);
    if near_x0 || near_y0 {
        // the coordinate ACROSS the axis becomes 0; along the axis it goes to the grid, if a node is near
        // on screen
        let gx = snap_to_grid(w.x);
        let gy = snap_to_grid(w.y);
        let ax = if near_x0 {
            0.0
        } else if (sh.at(Point2::new(gx, w.y)).x - screen.x).abs() <= qymcad_ui_state::grab::grab(sk.set, Grab::Snap) {
            gx
        } else {
            w.x
        };
        let ay = if near_y0 {
            0.0
        } else if (sh.at(Point2::new(w.x, gy)).y - screen.y).abs() <= qymcad_ui_state::grab::grab(sk.set, Grab::Snap) {
            gy
        } else {
            w.y
        };
        let ap = Point2::new(ax, ay);
        *sk.snap_hint = Some((ap, 2)); // an axis
        return ap;
    }
    // a grid node
    let gp = Point2::new(snap_to_grid(w.x), snap_to_grid(w.y));
    if sh.at(gp).distance(screen) <= qymcad_ui_state::grab::grab(sk.set, Grab::Snap) {
        *sk.snap_hint = Some((gp, 1)); // the grid
        return gp;
    }
    *sk.snap_hint = None;
    w
}

/// WHAT THE SNAPS OF CURVES ARE ASKED: where the pointer is, where a line being drawn starts, and how far around the
/// pointer to look, in millimetres.
struct CurveSnapAsk<'a> {
    at: Point2,
    start: Option<Point2>,
    reach: f64,
    /// the places of what a drag moves: the curves standing on them go with it, and offer nothing - the middle of a
    /// lone spline caught the node it was dragged by, and a side of a rectangle led by its centre the centre
    moving: &'a [Point2],
}

/// A PLACE A CURVE OFFERS to snap to: what the snap says of it (`kind`, as every snap does - 5 a crossing, 6 a point on a
/// curve, the tie a click there lays read from it) and where it stands among the snaps (`rank`, a smaller one first):
/// the place a line being drawn touches a curve 2, before all; a crossing of two curves 4, before a point of a line tied
/// to the grid that stands nearer the pointer on the same line; a middle and a quadrant 5, among the crossings of lines
/// and the nearer of them taken.
struct CurveSnap {
    at: Point2,
    kind: u8,
    rank: u8,
}

/// A SNAP OF A CURVE OFFERED, and how far from the pointer on the screen.
struct Offered {
    off: f32,
    snap: CurveSnap,
}

/// THE PLACES THE CURVES OF SKETCH `s` NEAR THE POINTER OFFER: the middle of each open curve but a line - half way along
/// it - and the quadrants of a circle, an arc or an ellipse, the ends of its axes, where the curve runs; the place a line
/// drawn from `start` touches each; where any two of them meet.
fn curve_snaps(s: &qymcad_core::model::Sketch, ask: CurveSnapAsk) -> Vec<CurveSnap> {
    use qymcad_core::model::curve::Curve;
    let goes = |id: &Id| s.point(*id).is_some_and(|q| ask.moving.iter().any(|m| (m.x - q.x).abs() < 1e-9 && (m.y - q.y).abs() < 1e-9));
    let moves = |e: &qymcad_core::model::SketchEntity| qymcad_core::model::entity_points(e).iter().any(goes);
    let near: Vec<Curve> = s.entities.iter().filter(|e| !moves(e)).filter_map(|e| Curve::of(s, e)).filter(|c| c.nearest(ask.at).off <= ask.reach).collect();
    let mut out = Vec::new();
    for c in &near {
        out.extend(c.quadrants().into_iter().map(|at| CurveSnap { at, kind: 6, rank: 5 }));
        if !c.closed() && !c.straight() {
            out.push(CurveSnap { at: c.half_way(), kind: 6, rank: 5 });
        }
        // the point of the curve nearest the pointer: a point put there stands on the curve and is held on it - an
        // ellipse and a spline offered none, and the grid took the click off them
        out.push(CurveSnap { at: c.nearest(ask.at).point, kind: 6, rank: 6 });
        if let Some(from) = ask.start {
            out.extend(touch_from(c, [from, ask.at]).map(|at| CurveSnap { at, kind: 6, rank: 2 }));
        }
    }
    for (k, a) in near.iter().enumerate() {
        for b in &near[k + 1..] {
            out.extend(a.meet(b).into_iter().map(|m| CurveSnap { at: m.at, kind: 5, rank: 4 }));
        }
    }
    out
}

/// WHERE A LINE FROM `from` TOUCHES CURVE `c` near `near`: the place where the way from `from` runs along the curve, by
/// Newton's steps from the place nearest `near`; `None` where there is none there.
fn touch_from(c: &qymcad_core::model::curve::Curve, [from, near]: [Point2; 2]) -> Option<Point2> {
    let lean = |t: f64| {
        let (q, u) = (c.at(t), c.tangent(t));
        (q.x - from.x) * u.y - (q.y - from.y) * u.x
    };
    let held = |t: f64| if c.closed() { t.rem_euclid(1.0) } else { t.clamp(0.0, 1.0) };
    let (mut t, h) = (c.nearest(near).t, 1e-7);
    for _ in 0..30 {
        let f = lean(t);
        let df = (lean(held(t + h)) - lean(held(t - h))) / (2.0 * h);
        if df.abs() < 1e-15 {
            return None;
        }
        let next = held(t - f / df);
        if (next - t).abs() < 1e-14 {
            t = next;
            break;
        }
        t = next;
    }
    (lean(t).abs() < 1e-9 && c.at(t).dist(from) > 1e-6).then(|| c.at(t))
}
