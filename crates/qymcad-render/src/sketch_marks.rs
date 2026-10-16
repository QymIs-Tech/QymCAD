//! THE POINTS AND THE DIMENSIONS OF A SKETCH as they are drawn.

use super::*;

/// Two plain points nearer on the screen than this, px - the width of a plain point - are one blot: the later one is
/// left out. Counted by points in sight instead (5 000), a grid of 200 lines across 200 others, 800 ends 5 px apart at
/// a whole view, drew its ends as solid bars.
pub const POINT_ROOM: f32 = 7.0;

/// The room a number of a point takes on the screen, px, as written in the monospace face of 10 px.
pub(crate) fn number_room(n: usize) -> egui::Vec2 {
    egui::vec2(6.2 * n.to_string().len() as f32 + 2.0, 11.0)
}

/// THE PLACES TAKEN ON THE SCREEN, on a grid of square cells: whether a place is free of what was put before it.
pub(crate) struct Taken {
    pub(crate) cell: f32,
    pub(crate) cells: std::collections::HashMap<(i32, i32), Vec<Rect>>,
}

impl Taken {
    pub(crate) fn new(cell: f32) -> Self {
        Taken { cell, cells: std::collections::HashMap::new() }
    }

    pub(crate) fn at(&self, p: Pos2) -> (i32, i32) {
        ((p.x / self.cell).floor() as i32, (p.y / self.cell).floor() as i32)
    }

    /// Whether `r` meets nothing put before, the cells it spans and those round them looked at.
    pub(crate) fn free(&self, r: Rect) -> bool {
        let ((x0, y0), (x1, y1)) = (self.at(r.min), self.at(r.max));
        (x0 - 1..=x1 + 1).all(|x| (y0 - 1..=y1 + 1).all(|y| self.cells.get(&(x, y)).is_none_or(|put| put.iter().all(|q| !q.intersects(r)))))
    }

    pub(crate) fn put(&mut self, r: Rect) {
        let ((x0, y0), (x1, y1)) = (self.at(r.min), self.at(r.max));
        for x in x0..=x1 {
            for y in y0..=y1 {
                self.cells.entry((x, y)).or_default().push(r);
            }
        }
    }
}

/// THE POINTS DRAWN ON THE SCREEN, on a dense grid of cells as wide as a point: whether a point stands nearer than
/// `POINT_ROOM` on both axes to one drawn before - the cells round its own looked at. A map of cells to the boxes put in
/// them, made for 70 000 points in sight, was most of 60 ms of a frame.
pub(crate) struct Dots {
    pub(crate) origin: Pos2,
    pub(crate) cols: usize,
    pub(crate) rows: usize,
    pub(crate) cells: Vec<Vec<Pos2>>,
}

impl Dots {
    pub(crate) fn new(area: Rect) -> Self {
        let (cols, rows) = (((area.width() / POINT_ROOM).ceil() as usize).max(1), ((area.height() / POINT_ROOM).ceil() as usize).max(1));
        Dots { origin: area.min, cols, rows, cells: vec![Vec::new(); cols * rows] }
    }

    pub(crate) fn at(&self, p: Pos2) -> (i64, i64) {
        (((p.x - self.origin.x) / POINT_ROOM).floor() as i64, ((p.y - self.origin.y) / POINT_ROOM).floor() as i64)
    }

    pub(crate) fn cell(&self, x: i64, y: i64) -> Option<usize> {
        (x >= 0 && y >= 0 && (x as usize) < self.cols && (y as usize) < self.rows).then(|| y as usize * self.cols + x as usize)
    }

    /// Whether no point drawn stands nearer than `POINT_ROOM` to `p` on both axes.
    pub(crate) fn free(&self, p: Pos2) -> bool {
        let (cx, cy) = self.at(p);
        let room = POINT_ROOM - 0.02;
        (cx - 1..=cx + 1).all(|x| (cy - 1..=cy + 1).all(|y| self.cell(x, y).is_none_or(|k| self.cells[k].iter().all(|q| (q.x - p.x).abs() >= room || (q.y - p.y).abs() >= room))))
    }

    pub(crate) fn put(&mut self, p: Pos2) {
        let (x, y) = self.at(p);
        if let Some(k) = self.cell(x, y) {
            self.cells[k].push(p);
        }
    }
}

/// THE POINTS OF A SKETCH, each in the colour of how defined it is: green defined, yellow still free, red while the
/// sketch holds a conflict of dimensions (harmless redundancy reddens nothing). The selected, the lit and the picked
/// ones larger, and always drawn. Only the points in sight are drawn; a plain point only where no point drawn before
/// it stands nearer than `POINT_ROOM`, and a number only where no number written before it stands - brought nearer,
/// the points part and are all drawn again. `picked` - the points the dimension tool holds.
pub fn draw_sketch_points(pn: &Painting, painter: &egui::Painter, rect: Rect, si: usize, picked: &[Id]) {
    let Some(s) = pn.project.sketches.get(si) else { return };
    let sh = Sheet { view: pn.view, rect };
    let diag = sketch_diag(pn.cache, pn.project, si);
    let has_conflict = !diag.conflicts.is_empty();
    // the reference points and the virtual sharps of rectangles are not drawn as numbered geometry
    let unseen = s.unseen_points();
    let selected: std::collections::HashSet<Id> = pn.sel_sk.items.iter().filter(|(k, _)| *k == 0).map(|(_, id)| *id).collect();
    let sight = rect.expand(8.0);
    let in_sight: Vec<(usize, Pos2)> =
        s.points.iter().enumerate().filter(|(_, p)| !unseen.contains(&p.id)).map(|(pi, p)| (pi, sh.at(Point2::new(p.x, p.y)))).filter(|(_, sp)| sight.contains(*sp)).collect();
    // the points marked go first, so a plain point gives way to them and not they to it
    let marked = |id: Id| selected.contains(&id) || pn.hover.sketch == Some((0, id)) || picked.contains(&id);
    // stable, in two passes rather than a sort: 70 000 points in sight sorted every frame were part of 60 ms
    let in_sight = in_sight.iter().filter(|&&(pi, _)| marked(s.points[pi].id)).chain(in_sight.iter().filter(|&&(pi, _)| !marked(s.points[pi].id))).copied();
    let (mut dots, mut numbers) = (Dots::new(rect.expand(8.0)), Taken::new(24.0));
    for (pi, sp) in in_sight {
        let id = s.points[pi].id;
        let base_col = if has_conflict {
            pn.scheme.pal.error_mild() // a conflict of dimensions - the points do not satisfy the constraints
        } else if diag.free.get(pi).copied().unwrap_or(true) {
            pn.scheme.pal.underdefined() // still free
        } else {
            pn.scheme.pal.ok() // defined
        };
        let (col, r) = if selected.contains(&id) {
            (pn.scheme.pal.emphasis(), 5.0)
        } else if pn.hover.sketch == Some((0, id)) {
            (pn.scheme.pal.preview(), 5.0) // the pre-select highlight
        } else if picked.contains(&id) {
            (pn.scheme.pal.sketch_point(), 4.5)
        } else {
            (base_col, 3.5)
        };
        if !marked(id) && !dots.free(sp) {
            continue;
        }
        dots.put(sp);
        painter.circle_filled(sp, r, col);
        let label = Rect::from_min_size(sp + egui::vec2(5.0, -5.0 - 11.0), number_room(pi + 1));
        if pn.set.show_point_numbers && numbers.free(label) {
            numbers.put(label);
            painter.text(sp + egui::vec2(5.0, -5.0), egui::Align2::LEFT_BOTTOM, format!("{}", pi + 1), egui::FontId::monospace(10.0), pn.scheme.pal.text_faint());
        }
    }
}

/// Draw the associative dimensions and constraints of the selected sketch in the viewport.
pub fn draw_sketch_dims(pn: &Painting, painter: &egui::Painter, rect: Rect, si: usize) {
    let sh = qymcad_ui_state::Sheet { view: pn.view, rect };
    use qymcad_core::model::{Constraint, EntityKind};
    let Some(s) = pn.project.sketches.get(si) else { return };
    let dim_col = pn.scheme.pal.dimension();
    let font = egui::FontId::proportional(pn.set.dim_font);
    // what a label says, as the settings ask
    let caption = |c: &Constraint| qymcad_ui_state::dim_caption(pn.project, si, c, pn.set).unwrap_or_default();
    // the text of a linear dimension beside its line, or on a shelf past the arrow when it does not fit between them
    // the text of a linear dimension where it was led along its line, or beside the line's middle, or on a shelf past
    // the arrow when it does not fit between them - the place the mouse takes it by
    let linear_text = |ci: usize, txt: String, col: Color32| {
        let Some((place, _)) = qymcad_ui_state::linear_text_of(pn.project, si, ci, &sh, pn.set) else { return };
        if let Some(shelf) = place.shelf {
            painter.line_segment(shelf, Stroke::new(1.0, col));
        }
        text_turned(painter, place.center, txt, font.clone(), col, place.angle);
    };
    // construction geometry is dashed
    let aux_col = pn.scheme.pal.sketch_construction();
    // A SPLINE PICKED OR UNDER THE POINTER IS LIT along its curve, as a picked or hovered line is
    for (spi, sp) in s.splines.iter().enumerate() {
        let key = sp.points.first().map(|&id| (qymcad_ui_state::SEL_SPLINE, id));
        let (sel, hov) = (key.is_some_and(|k| pn.sel_sk.items.contains(&k)), key.is_some() && pn.hover.sketch == key);
        if !sel && !hov {
            continue;
        }
        let hl = if sel { Stroke::new(2.5, pn.scheme.pal.emphasis()) } else { Stroke::new(2.0, pn.scheme.pal.preview()) };
        let mut line: Vec<Pos2> = pn.project.spline_polyline(si, spi).iter().map(|p| sh.at(*p)).collect();
        if sp.closed {
            line.extend(line.first().copied());
        }
        painter.add(egui::Shape::line(line, hl));
    }
    // THE TANGENT HANDLES of the splines picked (`splines_with_handles`): a line from the knot to the handle and a point
    // at its end to take it by. The one under the pointer is lit in the colour of what a click takes, and the one held by
    // the pressed button in the colour of what is being moved - the handle follows the pointer, so the one under it is
    // the one held.
    let hcol = pn.scheme.pal.dim_helper();
    let grab = qymcad_ui_state::grab::grab(pn.set, qymcad_ui_state::grab::Grab::Point);
    let pointer = painter.ctx().input(|i| i.pointer.hover_pos().map(|p| (p, i.pointer.primary_down())));
    let handles: Vec<(Point2, Point2)> = qymcad_ui_state::splines_with_handles(pn.project, pn.sel_sk, si).into_iter().flat_map(|spi| pn.project.spline_handles(si, spi)).collect();
    let under = pointer.and_then(|(p, _)| handles.iter().map(|(_, h)| sh.at(*h).distance(p)).enumerate().filter(|(_, d)| *d <= grab).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(k, _)| k));
    let held = pointer.is_some_and(|(_, down)| down);
    for (k, (knot, hend)) in handles.into_iter().enumerate() {
        let (sk, sh) = (sh.at(knot), sh.at(hend));
        let (col, r) = match under == Some(k) {
            true if held => (pn.scheme.pal.active(), 5.0),
            true => (pn.scheme.pal.preview(), 5.0),
            false => (hcol, 3.5),
        };
        painter.line_segment([sk, sh], Stroke::new(1.0, col));
        painter.circle_filled(sh, r, col);
        painter.circle_stroke(sh, r, Stroke::new(1.0, pn.scheme.pal.dim_helper_ring()));
    }
    // construction splines are dashed (they do not go into a profile)
    for spi in 0..s.splines.len() {
        if !s.splines[spi].construction {
            continue;
        }
        let poly = pn.project.spline_polyline(si, spi);
        if poly.len() >= 2 {
            let pts: Vec<Pos2> = poly.iter().map(|p| sh.at(*p)).collect();
            painter.add(egui::Shape::dashed_line(&pts, Stroke::new(1.0, aux_col), 6.0, 4.0));
        }
    }
    let points_by_id: std::collections::HashMap<Id, &qymcad_core::model::SketchPoint> = s.points.iter().map(|p| (p.id, p)).collect(); // a table: these are looked up for every entity or constraint
    let pt_of = |id: Id| points_by_id.get(&id).copied().map(|p| Point2::new(p.x, p.y));
    for e in &s.entities {
        if !e.construction {
            continue;
        }
        match e.kind {
            qymcad_core::model::EntityKind::Line { a, b } => {
                if let (Some(pa), Some(pb)) = (pt_of(a), pt_of(b)) {
                    let (sa, sb) = (sh.at(pa), sh.at(pb));
                    painter.add(egui::Shape::dashed_line(&[sa, sb], Stroke::new(1.0, aux_col), 6.0, 4.0));
                }
            }
            qymcad_core::model::EntityKind::Circle { center, r } => {
                if let Some(c) = pt_of(center) {
                    let sc = sh.at(c);
                    let rp = (sh.at(Point2::new(c.x + r, c.y)).x - sc.x).abs();
                    // a dashed circle, as segments
                    let n = 48;
                    let pts: Vec<Pos2> = (0..=n)
                        .map(|k| {
                            let a = std::f64::consts::TAU * k as f64 / n as f64;
                            sc + egui::vec2((rp as f64 * a.cos()) as f32, (rp as f64 * a.sin()) as f32)
                        })
                        .collect();
                    painter.add(egui::Shape::dashed_line(&pts, Stroke::new(1.0, aux_col), 6.0, 4.0));
                }
            }
            qymcad_core::model::EntityKind::Arc { center, a, b, ccw } => {
                if let (Some(c), Some(pa), Some(pb)) = (pt_of(center), pt_of(a), pt_of(b)) {
                    let r = ((pa.x - c.x).powi(2) + (pa.y - c.y).powi(2)).sqrt();
                    let a0 = (pa.y - c.y).atan2(pa.x - c.x);
                    let a1 = (pb.y - c.y).atan2(pb.x - c.x);
                    let arc = qymcad_core::geom::tessellate_arc(c.x, c.y, r, a0, a1, ccw, 0.02);
                    let sp: Vec<Pos2> = arc.iter().map(|p| sh.at(Point2::new(p.x, p.y))).collect();
                    if sp.len() >= 2 {
                        painter.add(egui::Shape::dashed_line(&sp, Stroke::new(1.0, aux_col), 6.0, 4.0));
                    }
                }
            }
            qymcad_core::model::EntityKind::Ellipse { c, ma, mi } => {
                let pts: Vec<Pos2> = qymcad_ui_state::ellipse_outline_world(pn.project, si, c, ma, mi).iter().map(|p| sh.at(*p)).collect();
                if pts.len() >= 2 {
                    painter.add(egui::Shape::dashed_line(&pts, Stroke::new(1.0, aux_col), 6.0, 4.0));
                }
            }
        }
    }
    // CONFLICTING dimensions (the value contradicts the geometry) are red
    let conflicts = qymcad_ui_state::sketch_diag(pn.cache, pn.project, si).conflicts;
    // the linear and angular dimensions (the selected one orange, a conflict red, the one under the cursor white)
    let sc = pn.view.scale; // the label offset `off` is in WORLD units -> pixels through the zoom scale
    for (ci, c) in s.constraints.iter().enumerate() {
        let dim_col = if Some(ci) == pn.gsel.constraint {
            pn.scheme.pal.selected()
        } else if conflicts.contains(&ci) {
            pn.scheme.pal.error()
        } else if Some(ci) == pn.hover.constraint {
            pn.scheme.pal.emphasis()
        } else {
            dim_col
        };
        // a reference (driven) dimension is grey and in brackets (it does not drive the geometry)
        let driven = c.is_driven();
        let dim_col = if driven && Some(ci) != pn.gsel.constraint && Some(ci) != pn.hover.constraint { pn.scheme.pal.dimension_driven() } else { dim_col };
        let label = caption(c); // taken before the match: the arms bind their own `c`
        match *c {
            Constraint::Distance { a, b, d, off: doff, axis, .. } => {
                let (Some(pa), Some(pb)) = (qymcad_ui_state::sketch_pt(pn.project, si, a), qymcad_ui_state::sketch_pt(pn.project, si, b)) else { continue };
                let (sa, sb) = (sh.at(pa), sh.at(pb));
                // the ends of the dimension line (la,lb) and the direction of the extension lines (perp), by orientation
                let (la, lb, dir, perp) = match axis {
                    1 => {
                        // horizontal: the line sits at mid.y+off and measures the difference in x
                        let y = (sa.y + sb.y) / 2.0 + doff as f32 * sc;
                        (Pos2::new(sa.x, y), Pos2::new(sb.x, y), egui::vec2(1.0, 0.0), egui::vec2(0.0, 1.0))
                    }
                    2 => {
                        // vertical: the line sits at mid.x+off and measures the difference in y
                        let x = (sa.x + sb.x) / 2.0 + doff as f32 * sc;
                        (Pos2::new(x, sa.y), Pos2::new(x, sb.y), egui::vec2(0.0, 1.0), egui::vec2(1.0, 0.0))
                    }
                    _ => {
                        let dir = (sb - sa).normalized();
                        let perp = egui::vec2(-dir.y, dir.x);
                        let off = perp * (16.0 + doff as f32 * sc);
                        (sa + off, sb + off, dir, perp)
                    }
                };
                // the extension lines (from the points to the ends of the dimension line) + the dimension line
                painter.line_segment([sa, la], Stroke::new(0.7, dim_col));
                painter.line_segment([sb, lb], Stroke::new(0.7, dim_col));
                painter.line_segment([la, lb], Stroke::new(1.2, dim_col));
                let along = (lb - la).normalized();
                for end in [(la, along), (lb, -along)] {
                    let t = end.1 * 6.0;
                    painter.line_segment([end.0, end.0 - t + perp * 2.5], Stroke::new(1.0, dim_col));
                    painter.line_segment([end.0, end.0 - t - perp * 2.5], Stroke::new(1.0, dim_col));
                }
                let _ = (dir, d);
                linear_text(ci, label.clone(), dim_col);
            }
            Constraint::EdgeDistance { c1, c2, d, m1, m2, off: doff, .. } => {
                // a tangent dimension: the dimension line runs between the RIMS of the circles (centre +/- radius along the line of centres)
                let (Some(p1), Some(p2)) = (qymcad_ui_state::sketch_pt(pn.project, si, c1), qymcad_ui_state::sketch_pt(pn.project, si, c2)) else { continue };
                let r_of = |cid: Id| -> f64 {
                    for e in &s.entities {
                        match e.kind {
                            EntityKind::Circle { center, r } if center == cid => return r,
                            EntityKind::Arc { center, a, .. } if center == cid => {
                                if let (Some(pc), Some(pa)) = (qymcad_ui_state::sketch_pt(pn.project, si, center), qymcad_ui_state::sketch_pt(pn.project, si, a)) {
                                    return ((pa.x - pc.x).powi(2) + (pa.y - pc.y).powi(2)).sqrt();
                                }
                            }
                            _ => {}
                        }
                    }
                    0.0
                };
                let (r1, r2) = (r_of(c1), r_of(c2));
                let len = ((p2.x - p1.x).powi(2) + (p2.y - p1.y).powi(2)).sqrt().max(1e-9);
                let (ux, uy) = ((p2.x - p1.x) / len, (p2.y - p1.y) / len);
                let e1 = Point2::new(p1.x - m1 as f64 * r1 * ux, p1.y - m1 as f64 * r1 * uy);
                let e2 = Point2::new(p2.x + m2 as f64 * r2 * ux, p2.y + m2 as f64 * r2 * uy);
                let (sa, sb) = (sh.at(e1), sh.at(e2));
                let dir = (sb - sa).normalized();
                let perp = egui::vec2(-dir.y, dir.x);
                let off = perp * (16.0 + doff as f32 * sc);
                let (la, lb) = (sa + off, sb + off);
                painter.line_segment([sa, la], Stroke::new(0.7, dim_col));
                painter.line_segment([sb, lb], Stroke::new(0.7, dim_col));
                painter.line_segment([la, lb], Stroke::new(1.2, dim_col));
                let along = (lb - la).normalized();
                for end in [(la, along), (lb, -along)] {
                    let t = end.1 * 6.0;
                    painter.line_segment([end.0, end.0 - t + perp * 2.5], Stroke::new(1.0, dim_col));
                    painter.line_segment([end.0, end.0 - t - perp * 2.5], Stroke::new(1.0, dim_col));
                }
                let _ = d;
                linear_text(ci, label.clone(), dim_col);
            }
            Constraint::DistancePL { p, a, b, d, off: doff, .. } => {
                // the distance from point p to the line a->b: a perpendicular from p to its foot on the line
                let (Some(pp), Some(pa)) = (qymcad_ui_state::sketch_pt(pn.project, si, p), qymcad_ui_state::sketch_pt(pn.project, si, a)) else { continue };
                let (sp, sa) = (sh.at(pp), sh.at(pa));
                let Some(ab) = qymcad_ui_state::line_screen_dir(pn.project, &pn.view, si, a, b, rect) else { continue };
                let t = (sp - sa).dot(ab);
                let foot = sa + ab * t; // the foot of the perpendicular on the line
                                        // the leader is offset ALONG the line (ab), so the dimension line can be raised or lowered
                                        // over the geometry. It used to be offset along perp, and then the label could only travel
                                        // along the axis being measured.
                let off = ab * (doff as f32 * sc);
                let (lp, lf) = (sp + off, foot + off);
                painter.line_segment([sp, lp], Stroke::new(0.7, dim_col));
                painter.line_segment([foot, lf], Stroke::new(0.7, dim_col));
                painter.line_segment([lp, lf], Stroke::new(1.2, dim_col));
                let along = (lf - lp).normalized();
                for end in [(lp, -along), (lf, along)] {
                    let tt = end.1 * 6.0;
                    let pv = egui::vec2(-along.y, along.x);
                    painter.line_segment([end.0, end.0 - tt + pv * 2.5], Stroke::new(1.0, dim_col));
                    painter.line_segment([end.0, end.0 - tt - pv * 2.5], Stroke::new(1.0, dim_col));
                }
                let _ = d;
                linear_text(ci, label.clone(), dim_col);
            }
            Constraint::Angle { .. } | Constraint::AngleLines { .. } => {
                let Some(g) = qymcad_ui_state::angle_dim_geom(pn.project, si, ci, &sh, pn.set) else { continue };
                draw_angle_dim(painter, &g, label.clone(), font.clone(), dim_col);
            }
            Constraint::Diameter { .. } => {
                // the dimension line through the centre (a diameter, rim to rim) or from it (a radius), and the text on
                // a shelf past the knee of the leader, or laid along the line itself
                let Some(g) = qymcad_ui_state::radial_dim_geom(pn.project, si, ci, &sh, pn.set) else { continue };
                painter.line_segment([g.start, g.edge], Stroke::new(1.0, dim_col));
                if let Some(shelf) = g.shelf {
                    painter.line_segment([g.edge, g.knee], Stroke::new(0.7, dim_col));
                    painter.line_segment(shelf, Stroke::new(0.7, dim_col));
                }
                text_turned(painter, g.text, label.clone(), font.clone(), dim_col, g.angle);
            }
            Constraint::ArcLength { .. } => {
                // the arc length as the drawing standards draw it: a dimension arc, extension lines, arrows, the number
                // and the arc mark over it - one geometry with the taking of its text
                let Some(g) = qymcad_ui_state::arc_length_dim_geom(pn.project, si, ci, &sh, pn.set) else { continue };
                for e in g.ext {
                    painter.line_segment(e, Stroke::new(0.7, dim_col));
                }
                painter.add(egui::Shape::line(g.arc.clone(), Stroke::new(1.2, dim_col)));
                for (tip, dir) in g.arrows {
                    let side = egui::vec2(-dir.y, dir.x);
                    let back = tip - dir * 6.0;
                    painter.line_segment([tip, back + side * 2.5], Stroke::new(1.0, dim_col));
                    painter.line_segment([tip, back - side * 2.5], Stroke::new(1.0, dim_col));
                }
                painter.add(egui::Shape::line(g.mark, Stroke::new(1.0, dim_col)));
                painter.text(g.text, egui::Align2::CENTER_CENTER, label.clone(), font.clone(), dim_col);
            }
            _ => {}
        }
    }
    // A CIRCLE OR AN ARC WITH NO SIZE OF ITS OWN SHOWS NO NUMBER. A label of its size in the style of a dimension cannot
    // be told from one: a diameter deleted would look still laid. Reported behaviour: "on circles a radius is always put
    // ... it is nailed down". A size is seen by laying a dimension; while the field of a new circle or arc is open, the
    // field shows it.
}
