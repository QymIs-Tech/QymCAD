//! WHERE THE POINTER SNAPS in a sketch.

use super::*;

/// Snapping the cursor: a vertex or a centre (always), then a midpoint, an intersection, a point on an
/// edge, an axis, a grid node. It sets `snap_hint` (the type for the glyph) and returns a world point.
/// The type codes: 0 vertex, 1 grid, 2 axis, 3 midpoint, 4 centre, 5 intersection, 6 on an edge.
pub fn snap_world(sk: &mut qymcad_ui_state::SketchCtx, rect: Rect, screen: Pos2) -> Point2 {
    let sh = qymcad_ui_state::Sheet { view: *sk.view, rect };
    let w = qymcad_ui_state::to_world(&*sk.view, rect, screen);
    // "No ties" on the bar of a drawing tool: the point goes where the pointer is, and the preview snaps to nothing
    if sk.armed.draw_kind() != 0 && sk.tool.ties == qymcad_ui_state::Ties::Free {
        *sk.snap_hint = None;
        return Point2::new(w.x, w.y);
    }
    let sd = |p: Point2, view: &qymcad_ui_state::View2d| qymcad_ui_state::Sheet { view: *view, rect }.at(p).distance(screen);
    // the centres of the circles and arcs of the active sketch, so a centre can be told from a plain vertex
    let centers: std::collections::HashSet<u64> = qymcad_ui_state::edit_si(&*sk.project, &*sk.sketch_ses)
        .and_then(|si| sk.project.sketches.get(si))
        .map(|s| {
            s.entities
                .iter()
                .filter_map(|e| match e.kind {
                    qymcad_core::model::EntityKind::Circle { center, .. } | qymcad_core::model::EntityKind::Arc { center, .. } => Some(center),
                    _ => None,
                })
                .collect()
        })
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
            Some(std::iter::once(Point2::new(q.x, q.y)).chain(s.rects.iter().filter(|r| r.centre == q.id).flat_map(|r| r.corners.into_iter().filter_map(at))).collect())
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
                    let p = match c.edges[k] {
                        qymcad_core::geom::ProfEdge::Line { b, .. } | qymcad_core::geom::ProfEdge::Arc { b, .. } => b,
                        qymcad_core::geom::ProfEdge::Circle { .. } => continue,
                    };
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
    if let Some(si) = qymcad_ui_state::edit_si(&*sk.project, &*sk.sketch_ses) {
        let qymcad_ui_state::ActiveEdges { lines, circles: circs } = qymcad_ui_state::active_edges(&sk.draw(), si);
        // ONLY WHAT RUNS NEAR THE POINTER: every point offered below - a midpoint, an intersection, a point on an
        // edge - lies on its segments or circles and is taken within the reach of a snap, so a curve that passes
        // further than that from the pointer offers nothing. Every pair of 70 000 segments, intersected each frame,
        // took 15 s.
        let reach = qymcad_ui_state::grab::grab(sk.set, Grab::Snap) + 1.0;
        let near_seg = |a: Point2, b: Point2| qymcad_ui_state::screen_dist_seg(screen, sh.at(a), sh.at(b)) <= reach;
        let lines: Vec<(Point2, Point2)> = lines.into_iter().filter(|(a, b)| !own(*a) && !own(*b) && near_seg(*a, *b)).collect(); // what moves with the drag is no target
        let circs: Vec<_> = circs.into_iter().filter(|rim| (sh.at(rim.centre).distance(screen) - rim.radius as f32 * sk.view.scale).abs() <= reach).collect();
        // the segments of the projected outlines of the reference body, used for INTERSECTIONS with the
        // sketch lines and for points on an edge. That is how the intersection of a construction line with
        // a face or the outline of a part becomes snappable.
        let ref_segs: Vec<(Point2, Point2)> = ref_edges.iter().flat_map(|poly| poly.windows(2).map(|s| (s[0], s[1]))).filter(|(a, b)| near_seg(*a, *b)).collect();
        // the priority: a midpoint (3) over an intersection (5) over a point on an edge (6)
        // 1) the midpoints of segments (SKETCH lines only - the midpoints of a tessellated outline are noise)
        for (a, b) in &lines {
            let m = Point2::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
            let d = sd(m, &*sk.view);
            if d <= qymcad_ui_state::grab::grab(sk.set, Grab::Snap) && cand.is_none_or(|(bd, _, bty)| (3u8, d) < (bty, bd)) {
                cand = Some((d, m, 3));
            }
        }
        // 2) intersections: sketch with sketch, sketch with a circle, AND sketch with THE OUTLINE OF A PART
        // (a construction line against a face)
        for i in 0..lines.len() {
            for j in (i + 1)..lines.len() {
                if let Some(p) = qymcad_ui_state::seg_seg_intersect(lines[i].0, lines[i].1, lines[j].0, lines[j].1) {
                    let d = sd(p, &*sk.view);
                    if d <= qymcad_ui_state::grab::grab(sk.set, Grab::Snap) && cand.is_none_or(|(bd, _, bty)| (5u8, d) < (bty, bd)) {
                        cand = Some((d, p, 5));
                    }
                }
            }
            for rim in &circs {
                for p in qymcad_ui_state::seg_circle_intersect(lines[i].0, lines[i].1, rim.centre, rim.radius).into_iter().filter(|p| rim.holds(*p)) {
                    let d = sd(p, &*sk.view);
                    if d <= qymcad_ui_state::grab::grab(sk.set, Grab::Snap) && cand.is_none_or(|(bd, _, bty)| (5u8, d) < (bty, bd)) {
                        cand = Some((d, p, 5));
                    }
                }
            }
            for (ra, rb) in &ref_segs {
                if let Some(p) = qymcad_ui_state::seg_seg_intersect(lines[i].0, lines[i].1, *ra, *rb) {
                    let d = sd(p, &*sk.view);
                    if d <= qymcad_ui_state::grab::grab(sk.set, Grab::Snap) && cand.is_none_or(|(bd, _, bty)| (5u8, d) < (bty, bd)) {
                        cand = Some((d, p, 5));
                    }
                }
            }
        }
        // 2b) THE AXES X = 0 AND Y = 0 CROSS THINGS TOO.
        //
        // Reported behaviour: "a circle of construction geometry has a snap, but at the same time the
        // cursor is not pulled to the X and Y axes, which makes it awkward to put the centre of a new
        // circle on the construction circle". The axes were looked at only in the SECOND half of the
        // snapping, after this block has already returned an answer - so the moment the cursor came near a
        // circle, the circle answered "a point on my edge" and the axes never got a turn. And they were
        // nowhere among the things that get intersected either, so the one point being aimed at - where
        // the construction circle crosses the axis - was offered by neither half.
        for axis in [0u8, 1] {
            for (a, b) in lines.iter().chain(ref_segs.iter()) {
                if let Some(p) = qymcad_ui_state::seg_axis_intersect(*a, *b, axis) {
                    let d = sd(p, &*sk.view);
                    if d <= qymcad_ui_state::grab::grab(sk.set, Grab::Snap) && cand.is_none_or(|(bd, _, bty)| (5u8, d) < (bty, bd)) {
                        cand = Some((d, p, 5));
                    }
                }
            }
            for rim in &circs {
                for p in qymcad_ui_state::circle_axis_intersect(rim.centre, rim.radius, axis).into_iter().filter(|p| rim.holds(*p)) {
                    let d = sd(p, &*sk.view);
                    if d <= qymcad_ui_state::grab::grab(sk.set, Grab::Snap) && cand.is_none_or(|(bd, _, bty)| (5u8, d) < (bty, bd)) {
                        cand = Some((d, p, 5));
                    }
                }
            }
        }
        // 3) a point on an edge (the cursor projected onto a sketch segment, THE OUTLINE OF A PART or a
        // circle) plus A TIE TO THE GRID ALONG the face (face against grid, type 5, which outranks a plain
        // point on an edge): across the face it sticks to the face, along it to the nearest grid line, so
        // the intersection of a face and the grid can be hit exactly.
        let gsz = sk.set.snap.grid.max(0.1);
        for (a, b) in lines.iter().chain(ref_segs.iter()) {
            if let Some(p) = qymcad_ui_state::project_on_seg(w, *a, *b) {
                if let Some(gpt) = qymcad_ui_state::grid_cross_on_seg(p, *a, *b, gsz) {
                    let d = sd(gpt, &*sk.view);
                    if d <= qymcad_ui_state::grab::grab(sk.set, Grab::Snap) && cand.is_none_or(|(bd, _, bty)| (5u8, d) < (bty, bd)) {
                        cand = Some((d, gpt, 5));
                    }
                }
                let d = sd(p, &*sk.view);
                if d <= qymcad_ui_state::grab::grab(sk.set, Grab::Snap) && cand.is_none_or(|(bd, _, bty)| (6u8, d) < (bty, bd)) {
                    cand = Some((d, p, 6));
                }
            }
        }
        for rim in &circs {
            let (c, r) = (rim.centre, rim.radius);
            let dir = Point2::new(w.x - c.x, w.y - c.y);
            let len = (dir.x * dir.x + dir.y * dir.y).sqrt();
            if len > 1e-9 {
                let p = Point2::new(c.x + dir.x / len * r, c.y + dir.y / len * r);
                if !rim.holds(p) {
                    continue; // the part of an arc's circle the arc does not run over is no edge
                }
                let d = sd(p, &*sk.view);
                if d <= qymcad_ui_state::grab::grab(sk.set, Grab::Snap) && cand.is_none_or(|(bd, _, bty)| (6u8, d) < (bty, bd)) {
                    cand = Some((d, p, 6));
                }
            }
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
