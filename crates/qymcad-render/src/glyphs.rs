//! THE GLYPHS OF JOINTS AND OF FIXED PARTS: their marks, their limits, their axes.

use super::*;

/// A VECTOR icon of a joint's kind inside its 3D badge — like the sketcher's constraint glyphs
/// (`paint_gly`), with no dependence on a font (phosphor inside `painter.text` came out as "tofu"). Drawn
/// over the badge's circle.
pub fn paint_joint_glyph(p: &egui::Painter, c: Pos2, r: f32, k: qymcad_core::feature::JointKind, col: Color32) {
    use egui::vec2 as v;
    use qymcad_core::feature::JointKind as J;
    let st = Stroke::new(1.8, col);
    let s = r * 0.62; // the symbol's half-size, kept large enough to read
    let head = |tip: Pos2, dir: egui::Vec2| {
        let d = dir.normalized();
        let n = v(-d.y, d.x);
        p.line_segment([tip, tip - d * (s * 0.55) + n * (s * 0.4)], st);
        p.line_segment([tip, tip - d * (s * 0.55) - n * (s * 0.4)], st);
    };
    match k {
        J::Rigid => {
            // a diamond for a fixed joint: recognisable and unlike a blank box
            let pts = vec![c + v(0.0, -s), c + v(s, 0.0), c + v(0.0, s), c + v(-s, 0.0)];
            p.add(egui::Shape::convex_polygon(pts, col, Stroke::NONE));
        }
        J::Revolute => {
            // an arc of about 290 deg with an arrowhead: a rotation
            let pts: Vec<Pos2> = (0..=24)
                .map(|i| {
                    let a = std::f64::consts::PI * (0.2 + 1.6 * i as f64 / 24.0);
                    c + v(a.cos() as f32 * s, a.sin() as f32 * s)
                })
                .collect();
            let n = pts.len();
            p.add(egui::Shape::line(pts.clone(), st));
            head(pts[n - 1], pts[n - 1] - pts[n - 2]);
        }
        J::Slider => {
            // a double-headed arrow: one translation
            let (l, rt) = (c + v(-s, 0.0), c + v(s, 0.0));
            p.line_segment([l, rt], st);
            head(l, l - rt);
            head(rt, rt - l);
        }
        J::Cylindrical => {
            // a circle plus an axis: one rotation and one translation
            p.circle_stroke(c, s * 0.7, st);
            p.line_segment([c + v(-s, 0.0), c + v(s, 0.0)], st);
        }
        J::Planar => {
            // a parallelogram for a plane: slanted, so it is not a square
            let pts = vec![c + v(-s, s * 0.55), c + v(s * 0.45, s * 0.55), c + v(s, -s * 0.55), c + v(-s * 0.45, -s * 0.55)];
            p.add(egui::Shape::convex_polygon(pts, Color32::TRANSPARENT, st));
        }
        J::Ball => {
            p.circle_filled(c, s * 0.85, col); // a ball: three rotations
        }
        J::PinSlot => {
            p.rect_stroke(egui::Rect::from_center_size(c, v(s * 2.0, s * 1.05)), s * 0.5, st, egui::StrokeKind::Middle); // the slot
            p.circle_filled(c + v(-s * 0.45, 0.0), s * 0.32, col); // the pin
        }
        J::Parallel => {
            // two parallel strokes: a condition rather than a fit
            p.line_segment([c + v(-s, -s * 0.45), c + v(s, -s * 0.45)], st);
            p.line_segment([c + v(-s, s * 0.45), c + v(s, s * 0.45)], st);
        }
    }
}

pub fn paint_gly(p: &egui::Painter, c: Pos2, h: f32, g: Gly, col: Color32) {
    use egui::vec2 as v;
    let st = Stroke::new(1.4, col);
    let thin = Stroke::new(1.0, col);
    match g {
        Gly::Coincident => {
            p.circle_stroke(c - v(2.0, 0.0), h * 0.6, st);
            p.circle_stroke(c + v(2.0, 0.0), h * 0.6, st);
        }
        Gly::Horiz => {
            let (l, r) = (c + v(-h, 0.0), c + v(h, 0.0));
            p.line_segment([l, r], st);
            p.line_segment([l + v(0.0, -3.0), l + v(0.0, 3.0)], thin);
            p.line_segment([r + v(0.0, -3.0), r + v(0.0, 3.0)], thin);
        }
        Gly::Vert => {
            let (t, b) = (c + v(0.0, -h), c + v(0.0, h));
            p.line_segment([t, b], st);
            p.line_segment([t + v(-3.0, 0.0), t + v(3.0, 0.0)], thin);
            p.line_segment([b + v(-3.0, 0.0), b + v(3.0, 0.0)], thin);
        }
        Gly::Parallel => {
            p.line_segment([c + v(-3.0, -h), c + v(-1.0, h)], st);
            p.line_segment([c + v(3.0, -h), c + v(5.0, h)], st);
        }
        Gly::Perp => {
            p.line_segment([c + v(0.0, -h), c + v(0.0, h)], st);
            p.line_segment([c + v(-h, h), c + v(h, h)], st);
        }
        Gly::Equal => {
            p.line_segment([c + v(-h, -2.5), c + v(h, -2.5)], st);
            p.line_segment([c + v(-h, 2.5), c + v(h, 2.5)], st);
        }
        Gly::Collinear => {
            p.line_segment([c + v(-h, 0.0), c + v(h, 0.0)], st);
            p.circle_filled(c, 1.6, col);
        }
        Gly::Concentric => {
            p.circle_stroke(c, h, st);
            p.circle_stroke(c, h * 0.5, st);
        }
        Gly::Midpoint => {
            // a line with a tick at its middle
            p.line_segment([c + v(-h, 0.0), c + v(h, 0.0)], st);
            p.line_segment([c + v(0.0, -h * 0.6), c + v(0.0, h * 0.6)], st);
        }
        Gly::PointOnLine => {
            // a line with a point on it
            p.line_segment([c + v(-h, 0.0), c + v(h, 0.0)], st);
            p.circle_filled(c, 2.0, col);
        }
        Gly::PointOnCircle => {
            // an arc of a circle with a point on it (a glyph of its own, not an alias of PointOnLine)
            p.circle_stroke(c + v(0.0, h * 0.7), h, thin);
            p.circle_filled(c + v(0.0, h * 0.7 - h), 2.0, col);
        }
        Gly::Fix => {
            let r = Rect::from_center_size(c, v(h * 1.5, h * 1.5));
            p.rect_stroke(r, 1.0, st, egui::StrokeKind::Middle);
            p.circle_filled(c, 1.6, col);
        }
        Gly::Construction => {
            p.add(egui::Shape::dashed_line(&[c + v(-h, 0.0), c + v(h, 0.0)], st, 3.0, 2.0));
        }
        Gly::DimLin => {
            let (l, r) = (c + v(-h, 0.0), c + v(h, 0.0));
            p.line_segment([l, r], st);
            p.line_segment([l, l + v(3.0, -2.0)], thin);
            p.line_segment([l, l + v(3.0, 2.0)], thin);
            p.line_segment([r, r + v(-3.0, -2.0)], thin);
            p.line_segment([r, r + v(-3.0, 2.0)], thin);
        }
        Gly::DimAng => {
            let o = c + v(-h * 0.6, h * 0.7);
            p.line_segment([o, o + v(h * 1.6, 0.0)], st);
            p.line_segment([o, o + v(h * 1.1, -h * 1.5)], st);
            p.circle_stroke(o, h * 0.7, thin);
        }
        Gly::DimRad => {
            p.circle_stroke(c, h * 0.85, st);
            let e = c + v(h * 0.85, 0.0);
            p.line_segment([c, e], thin);
            p.line_segment([e, e + v(-3.0, -2.0)], thin);
            p.line_segment([e, e + v(-3.0, 2.0)], thin);
        }
        Gly::Mirror => {
            p.add(egui::Shape::dashed_line(&[c + v(0.0, -h), c + v(0.0, h)], thin, 2.0, 2.0));
            for s in [-1.0_f32, 1.0] {
                let x = s * 2.0;
                let tip = s * (h - 1.0);
                p.line_segment([c + v(x, -3.0), c + v(tip, 0.0)], st);
                p.line_segment([c + v(tip, 0.0), c + v(x, 3.0)], st);
                p.line_segment([c + v(x, -3.0), c + v(x, 3.0)], st);
            }
        }
        Gly::ArrayLin => {
            for k in 0..3 {
                let x = -h + k as f32 * h;
                p.rect_stroke(Rect::from_center_size(c + v(x, 0.0), v(4.0, 4.0)), 0.0, st, egui::StrokeKind::Middle);
            }
        }
        Gly::ArrayCirc => {
            for k in 0..6 {
                let a = std::f32::consts::TAU * k as f32 / 6.0;
                p.circle_filled(c + v(h * 0.85 * a.cos(), h * 0.85 * a.sin()), 1.6, col);
            }
        }
        Gly::Fillet => {
            let cen = c + v(h * 0.5, h * 0.5);
            let pts: Vec<Pos2> = (0..=8)
                .map(|i| {
                    let t = std::f32::consts::PI + std::f32::consts::FRAC_PI_2 * (i as f32 / 8.0);
                    cen + v(h * t.cos(), h * t.sin())
                })
                .collect();
            p.add(egui::Shape::line(pts, st));
            p.line_segment([cen + v(-h, 0.0), cen + v(-h, h * 0.7)], st);
            p.line_segment([cen + v(0.0, -h), cen + v(-h * 0.7, -h)], st);
        }
        Gly::Chamfer => {
            let tl = c + v(-h, -h);
            p.line_segment([tl + v(0.0, h * 0.5), tl + v(0.0, h * 1.7)], st);
            p.line_segment([tl + v(h * 0.5, 0.0), tl + v(h * 1.7, 0.0)], st);
            p.line_segment([tl + v(0.0, h * 0.5), tl + v(h * 0.5, 0.0)], st);
        }
        Gly::Offset => {
            p.line_segment([c + v(-h, -h * 0.5), c + v(h, -h * 0.5)], st);
            p.add(egui::Shape::dashed_line(&[c + v(-h, h * 0.5), c + v(h, h * 0.5)], thin, 2.0, 2.0));
        }
        Gly::Tangent => {
            p.circle_stroke(c + v(0.0, 2.0), h * 0.7, st);
            p.line_segment([c + v(-h, -h * 0.6), c + v(h, -h * 0.6)], st); // the tangent
        }
        Gly::Symmetric => {
            p.add(egui::Shape::dashed_line(&[c + v(0.0, -h), c + v(0.0, h)], thin, 2.0, 2.0));
            p.circle_filled(c + v(-h * 0.7, 0.0), 1.8, col);
            p.circle_filled(c + v(h * 0.7, 0.0), 1.8, col);
            p.line_segment([c + v(-h * 0.7, 0.0), c + v(-2.0, 0.0)], thin);
            p.line_segment([c + v(2.0, 0.0), c + v(h * 0.7, 0.0)], thin);
        }
        Gly::Trim => {
            // scissors: a line with a piece cut out of it (dashed in the middle)
            p.line_segment([c + v(-h, 0.0), c + v(-h * 0.35, 0.0)], st);
            p.line_segment([c + v(h * 0.35, 0.0), c + v(h, 0.0)], st);
            p.add(egui::Shape::dashed_line(&[c + v(-h * 0.35, 0.0), c + v(h * 0.35, 0.0)], thin, 1.5, 1.5));
            p.line_segment([c + v(-3.0, -h), c + v(3.0, h)], st);
        }
        Gly::Extend => {
            // a line plus a dashed continuation with an arrow up to the barrier
            p.line_segment([c + v(-h, 0.0), c + v(2.0, 0.0)], st);
            p.add(egui::Shape::dashed_line(&[c + v(2.0, 0.0), c + v(h, 0.0)], thin, 1.5, 1.5));
            p.line_segment([c + v(h, -h), c + v(h, h)], st); // the barrier
            p.line_segment([c + v(h, 0.0), c + v(h - 3.0, -2.0)], thin);
            p.line_segment([c + v(h, 0.0), c + v(h - 3.0, 2.0)], thin);
        }
        Gly::Break => {
            // two lines with a break between them
            p.line_segment([c + v(-h, 0.0), c + v(-2.0, 0.0)], st);
            p.line_segment([c + v(2.0, 0.0), c + v(h, 0.0)], st);
            p.line_segment([c + v(0.0, -h), c + v(0.0, h)], thin);
        }
        Gly::Ellipse => {
            let n = 24;
            let pts: Vec<Pos2> = (0..=n)
                .map(|i| {
                    let a = std::f32::consts::TAU * i as f32 / n as f32;
                    c + v(h * a.cos(), h * 0.6 * a.sin())
                })
                .collect();
            p.add(egui::Shape::line(pts, st));
        }
        Gly::Spline => {
            let pts: Vec<Pos2> = (0..=16)
                .map(|i| {
                    let t = i as f32 / 16.0;
                    let x = -h + 2.0 * h * t;
                    let y = (t * std::f32::consts::TAU).sin() * h * 0.5;
                    c + v(x, y)
                })
                .collect();
            p.add(egui::Shape::line(pts, st));
        }
        Gly::Circle3 => {
            p.circle_stroke(c, h, st);
            for k in 0..3 {
                let a = std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * k as f32 / 3.0;
                p.circle_filled(c + v(h * a.cos(), h * a.sin()), 1.6, col);
            }
        }
        Gly::Text => {
            p.line_segment([c + v(-h, -h), c + v(h, -h)], st); // the top bar
            p.line_segment([c + v(0.0, -h), c + v(0.0, h)], st); // the stem
        }
    }
}

/// THE RANGE OF A LIMITED DEGREE: dashes from the minimum to the maximum, with ticks at the stops.
///
/// It is drawn exactly where the part will come to rest: the points are taken from THE SAME direction of
/// the degree the solver computes with (`joint_slot_axis`), and the bounds from the same `limit_min/max`
/// fields the drag is clamped by. Two pictures of one range would drift apart silently.
pub fn joint_limits(dc: &DrawCtx, painter: &egui::Painter, rect: Rect, jid: Id, o: [f64; 3], l: f64, hs: &[qymcad_ui_state::JointHandle]) {
    let Some(j) = dc.project.joints.iter().find(|x| x.id == jid) else { return };
    let basis = dc.cam.basis();
    let scr = qymcad_ui_state::Screen { cam: dc.cam, set: dc.set, rect, basis: &basis };
    let col = dc.scheme.pal.hint();
    for &qymcad_ui_state::JointHandle { slot, ring, dir } in hs {
        let (lo, hi) = (j.limit_min[slot as usize], j.limit_max[slot as usize]);
        if lo.is_none() && hi.is_none() {
            continue; // the degree is unlimited - nothing to draw
        }
        if ring {
            // ROTATION: an arc along the ring from the minimum to the maximum, measured from the joint's own zero.
            let Some(zero) = dc.project.joint_zero_dir(jid, qymcad_ui_state::current_ctx_id(dc.active_path, dc.project)) else { continue };
            let (lo, hi) = (lo.unwrap_or(-180.0).to_radians(), hi.unwrap_or(180.0).to_radians());
            let cross = [dir[1] * zero[2] - dir[2] * zero[1], dir[2] * zero[0] - dir[0] * zero[2], dir[0] * zero[1] - dir[1] * zero[0]];
            let at = |a: f64| {
                let (s, c) = a.sin_cos();
                [o[0] + l * (zero[0] * c + cross[0] * s), o[1] + l * (zero[1] * c + cross[1] * s), o[2] + l * (zero[2] * c + cross[2] * s)]
            };
            let n = 24;
            let pts: Vec<Pos2> = (0..=n).map(|k| scr.at(at(lo + (hi - lo) * k as f64 / n as f64)).0).collect();
            painter.add(egui::Shape::line(pts, Stroke::new(3.0, col)));
            for a in [lo, hi] {
                let p = at(a);
                let s = scr.at(p).0;
                let c = scr.at(o).0;
                let v = (s - c).normalized() * 7.0;
                painter.line_segment([s - v, s + v], Stroke::new(2.5, col));
            }
            continue;
        }
        // TRANSLATION: dashes along the travel axis from the minimum to the maximum, with cross ticks at the ends.
        let (a, b) = (lo.unwrap_or(-l), hi.unwrap_or(l));
        let at = |t: f64| [o[0] + dir[0] * t, o[1] + dir[1] * t, o[2] + dir[2] * t];
        let n = 16;
        for k in 0..n {
            if k % 2 == 1 {
                continue; // dashed: every other one
            }
            let t0 = a + (b - a) * k as f64 / n as f64;
            let t1 = a + (b - a) * (k + 1) as f64 / n as f64;
            painter.line_segment([scr.at(at(t0)).0, scr.at(at(t1)).0], Stroke::new(2.0, col));
        }
        for t in [a, b] {
            let s = scr.at(at(t)).0;
            let ahead = scr.at(at(t + l * 0.05)).0;
            let v = (ahead - s).normalized().rot90() * 7.0;
            painter.line_segment([s - v, s + v], Stroke::new(2.5, col));
        }
    }
}

/// THE GEOMETRY A PERSON POINTED AT AS THE ANCHOR AXIS, highlighted.
///
/// A straight edge is drawn whole: what is seen is exactly what was pointed at. Everything else (a face,
/// a circular edge, a datum plane) has no direction in the form of a segment - there a line is drawn
/// THROUGH the anchor origin along the chosen direction, longer than the gizmo handles so that it is not
/// mistaken for a degree of freedom.
pub fn joint_axis_refs(dc: &DrawCtx, painter: &egui::Painter, rect: Rect, jid: Id, l: f64) {
    use qymcad_core::feature::AnchorRef;
    let Some(j) = dc.project.joints.iter().find(|x| x.id == jid) else { return };
    let ctx = qymcad_ui_state::current_ctx_id(dc.active_path, dc.project);
    let basis = dc.cam.basis();
    let scr = qymcad_ui_state::Screen { cam: dc.cam, set: dc.set, rect, basis: &basis };
    let col = dc.scheme.pal.active();
    for cid in [j.a, j.b] {
        let Some(c) = dc.project.connector(cid) else { continue };
        let Some(r) = c.axis_ref.as_ref() else { continue };
        if let AnchorRef::EdgeMid(body, eid) = r {
            if let Some(e) = dc.project.regen_edges.get(body).and_then(|es| es.iter().find(|e| e.id == *eid)) {
                if !e.is_circular() {
                    let wt = dc.project.body_display_transform(*body, ctx);
                    let tp = |v: [f64; 3]| scr.at(qymcad_core::feature::apply12(&wt, v)).0;
                    painter.line_segment([tp(e.a), tp(e.b)], Stroke::new(3.5, col));
                    continue;
                }
            }
        }
        // not a segment - draw the direction through the anchor origin
        let (Some(m), Some(dir)) = (dc.project.connector_matrix(cid), dc.project.anchor_direction(r)) else { continue };
        let owner = dc.project.connector(cid).map(|c| c.owner).unwrap_or(ctx);
        let wt = qymcad_core::feature::mat_mul12(&dc.project.relative_transform(owner, ctx), &m);
        let o = [wt[3], wt[7], wt[11]];
        let d = qymcad_core::feature::apply12(&dc.project.relative_transform(owner, ctx), dir);
        let z = qymcad_core::feature::apply12(&dc.project.relative_transform(owner, ctx), [0.0; 3]);
        let d = [d[0] - z[0], d[1] - z[1], d[2] - z[2]];
        let at = |t: f64| scr.at([o[0] + d[0] * t, o[1] + d[1] * t, o[2] + d[2] * t]).0;
        painter.line_segment([at(-1.5 * l), at(1.5 * l)], Stroke::new(3.5, col));
    }
}

/// An anchor glyph at the grounded parts of the active assembly (like the mate glyphs - it shows what is
/// fixed). It is drawn at the component's ORIGIN (its local zero in the context's frame), projected to the screen.
pub fn draw_grounded_glyphs(pn: &Painting, painter: &egui::Painter, rect: Rect) {
    if !pn.mode_3d || !pn.set.show_joints || !matches!(pn.workbench, Workbench::Assembly) {
        return;
    }
    let ctx = qymcad_ui_state::current_ctx_id(pn.active_path, pn.project);
    let basis = pn.cam.basis();
    for &c in &pn.project.component_children(ctx) {
        if !pn.project.is_grounded(c) {
            continue;
        }
        let o = qymcad_core::feature::apply12(&pn.project.relative_transform(c, ctx), [0.0, 0.0, 0.0]);
        let at = qymcad_ui_state::Screen { cam: &pn.cam, set: pn.set, rect, basis: &basis }.at(o).0;
        let col = pn.scheme.pal.grounded();
        painter.circle_filled(at, 8.0, qymcad_scheme::a(pn.scheme.pal.glyph_backing(), 225));
        painter.circle_stroke(at, 8.0, Stroke::new(1.4, col));
        painter.text(at, egui::Align2::CENTER_CENTER, ph::ANCHOR, egui::FontId::proportional(11.0), col);
    }
}
