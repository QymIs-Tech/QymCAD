//! THE DIMENSIONS OF A SKETCH as they are drawn: their lines, arcs and texts.

use super::*;

/// AN ANGULAR DIMENSION AS IT STANDS ON THE SCREEN - one geometry for drawing it and for taking its label, so the label
/// is grabbed where the eye sees it.
pub struct AngleDim {
    /// Where the sides meet: the vertex, or where the two lines cross.
    pub center: Pos2,
    /// The screen direction of the first side and the signed turn to the second (|sweep| < pi).
    pub a0: f32,
    pub sweep: f32,
    /// The radius of the arc, px.
    pub r: f32,
    /// The arc drawn, as screen angles: the angle between the sides, run on to the label when it stands past one.
    pub arc: (f32, f32),
    /// The label, outside the arc so the arc does not run through the text.
    pub label: Pos2,
    /// Extension lines carrying a side out to the arc (or in to it) where the arc does not reach the side itself.
    pub ext: Vec<[Pos2; 2]>,
}

/// HOW THE TEXT OF A LINEAR DIMENSION IS TURNED. Along its line is the default: the text of a vertical dimension laid
/// level across a tall narrow line reads badly and takes room the sheet does not have.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, serde::Serialize, serde::Deserialize)]
pub enum DimTextTurn {
    /// level, whatever way the line runs
    Horizontal,
    /// along the dimension line, read from left to right or from the bottom up
    #[default]
    AlongLine,
}

/// A TURN THAT KEEPS A TEXT READABLE: the direction `angle` (radians, screen) folded into [-pi/2, pi/2), so a text laid
/// along a line reads from left to right, and along an upright line from the bottom up.
pub fn readable_angle(angle: f32) -> f32 {
    let pi = std::f32::consts::PI;
    let mut a = angle;
    while a >= pi / 2.0 {
        a -= pi;
    }
    while a < -pi / 2.0 {
        a += pi;
    }
    a
}

/// The size of the labels of sketch dimensions, px, and the range the settings keep it in.
pub const DIM_FONT_DEFAULT: f32 = 13.0;
pub const DIM_FONT_RANGE: std::ops::RangeInclusive<f32> = 8.0..=32.0;

/// WHAT THE LABEL OF A SKETCH DIMENSION SAYS: its value as the sheet shows it (`50.0`, `R5.0`, `45°`), led by the
/// name of a driver and the formula it is set by when the settings ask for them - `w = 2*w+10 = 50.0`. A reference
/// dimension is in brackets. `None` for a constraint that is not a dimension.
pub fn dim_caption(project: &Project, si: usize, c: &qymcad_core::model::Constraint, set: &Settings) -> Option<String> {
    use qymcad_core::model::Constraint;
    let value = match *c {
        // the number alone for every length: which way it runs is plain from its line
        Constraint::Distance { d, .. } => format!("{d:.1}"),
        Constraint::EdgeDistance { d, .. } => format!("T {d:.1}"),
        Constraint::DistancePL { d, .. } => format!("{:.1}", d.abs()), // d is signed (it carries the side)
        Constraint::Diameter { d, diam, .. } => format!("{}{d:.1}", if diam { "Ø" } else { "R" }),
        // the number alone: the arc mark over it is drawn with the dimension, a letter said less
        Constraint::ArcLength { len, .. } => format!("{len:.1}"),
        Constraint::Angle { deg, .. } | Constraint::AngleLines { deg, .. } => format!("{deg:.0}°"),
        _ => return None,
    };
    let mut parts: Vec<String> = Vec::new();
    if set.dim_show_name {
        let sketch = project.sketches.get(si)?.id;
        let refs = Project::dim_refs(c).map(|r| Project::dim_key_pub(&r)).unwrap_or_default();
        let name = project.name_of_target(&qymcad_core::model::DimTarget::Sketch { sketch, refs });
        if !name.is_empty() {
            parts.push(name);
        }
    }
    let expr = c.dim_expr().map(str::trim).unwrap_or("");
    // a formula is shown when there is one: a bare number typed into the field says nothing the value does not
    if set.dim_show_formula && !expr.is_empty() && expr.parse::<f64>().is_err() {
        parts.push(expr.to_string());
    }
    parts.push(value);
    let text = parts.join(" = ");
    Some(if c.is_driven() { format!("({text})") } else { text })
}

/// HOW MUCH ROOM A LABEL TAKES, px, at the size `px` - an estimate from the count of letters, the same for drawing the
/// label and for taking it with the mouse, so the two agree whatever the font makes of it.
pub fn dim_text_size(text: &str, px: f32) -> egui::Vec2 {
    egui::vec2(text.chars().count() as f32 * px * 0.56, px * 1.2)
}

/// A VALUE WITH A MINUS AND A PLUS beside it: each press moves it by `step` within `range`, and the value stands between
/// the buttons as a number. Answers the value as it now is.
pub fn step_buttons(ui: &mut egui::Ui, v: f32, step: f32, range: std::ops::RangeInclusive<f32>, less: &str, more: &str) -> f32 {
    let mut v = v.clamp(*range.start(), *range.end());
    if ui.add_enabled(v > *range.start(), egui::Button::new(egui_phosphor::regular::MINUS)).on_hover_text(less).clicked() {
        v = (v - step).max(*range.start());
    }
    ui.label(qymcad_i18n::num(v as f64, 0));
    if ui.add_enabled(v < *range.end(), egui::Button::new(egui_phosphor::regular::PLUS)).on_hover_text(more).clicked() {
        v = (v + step).min(*range.end());
    }
    v
}

/// HOW FAR POINT `pos` IS FROM A LABEL whose text of `size` stands centred at `center`: zero anywhere on the text,
/// otherwise the distance to its middle - so a label is taken by any of its letters, however large the settings make it.
pub fn label_reach(center: Pos2, size: egui::Vec2, pos: Pos2) -> f32 {
    if egui::Rect::from_center_size(center, size + egui::vec2(4.0, 4.0)).contains(pos) {
        0.0
    } else {
        center.distance(pos)
    }
}

/// Where the text of a linear dimension stands, and the shelf it stands on when it is carried out.
pub struct DimTextPlace {
    pub center: Pos2,
    /// the turn of the text, radians: 0 level, or the readable direction of its line
    pub angle: f32,
    /// the shelf past the arrow at `lb`, drawn along the dimension line, when the text does not fit between the arrows
    pub shelf: Option<[Pos2; 2]>,
}

/// THE GAP BETWEEN A LABEL AND THE LINE IT NAMES, px.
pub const DIM_TEXT_GAP: f32 = 3.0;

/// WHERE THE TEXT OF A LINEAR DIMENSION STANDS: beside the dimension line `la`-`lb` on the side `perp`, its box clear of
/// the line by a gap whatever the line's direction - the middle of the text used to stand 8 px off the line, and a
/// vertical dimension's text, 40 px wide, lay across it. Text that does not fit between the arrows (6 px each) is
/// carried past the arrow at `lb` onto a shelf along the line.
pub fn dim_text_place(la: Pos2, lb: Pos2, perp: egui::Vec2, size: egui::Vec2, at: Option<f64>, along_line: bool) -> DimTextPlace {
    let len = (lb - la).length();
    let dir = if len > 1e-3 { (lb - la) / len } else { egui::vec2(1.0, 0.0) };
    let perp = if perp.length() > 1e-6 { perp.normalized() } else { egui::vec2(-dir.y, dir.x) };
    // LAID ALONG THE LINE the text runs its width along it and its height across; level, its box meets the line askew
    let angle = if along_line { readable_angle(dir.y.atan2(dir.x)) } else { 0.0 };
    let (along, across) = if along_line {
        (size.x, size.y / 2.0 + DIM_TEXT_GAP)
    } else {
        (dir.x.abs() * size.x + dir.y.abs() * size.y, (perp.x.abs() * size.x + perp.y.abs() * size.y) / 2.0 + DIM_TEXT_GAP)
    };
    // LED ALONG BY HAND: the text stands where it was put, and past an arrow the line runs on under it as a shelf
    if let Some(t) = at {
        let s = t as f32 * len;
        let center = la + dir * s + perp * across;
        let shelf = if s > len {
            Some([lb, la + dir * (s + along / 2.0 + DIM_TEXT_GAP)])
        } else if s < 0.0 {
            Some([la, la + dir * (s - along / 2.0 - DIM_TEXT_GAP)])
        } else {
            None
        };
        return DimTextPlace { center, angle, shelf };
    }
    if along + 2.0 * 6.0 + 4.0 <= len {
        DimTextPlace { center: ((la.to_vec2() + lb.to_vec2()) / 2.0).to_pos2() + perp * across, angle, shelf: None }
    } else {
        let end = lb + dir * (along + 2.0 * DIM_TEXT_GAP + 4.0);
        DimTextPlace { center: lb + dir * (4.0 + DIM_TEXT_GAP + along / 2.0) + perp * across, angle, shelf: Some([lb, end]) }
    }
}

/// THE DIMENSION LINE OF A LINEAR DIMENSION on the screen - its two ends and the side its text stands on - for a length,
/// a point-to-line distance and a gap between round edges: one geometry for drawing the dimension, placing its text and
/// taking it with the mouse. `None` for any other constraint.
pub fn linear_dim_line(project: &Project, si: usize, c: &qymcad_core::model::Constraint, sh: &Sheet) -> Option<(Pos2, Pos2, egui::Vec2)> {
    use qymcad_core::model::Constraint;
    let sc = sh.view.scale;
    let p = |id| sketch_pt(project, si, id);
    match *c {
        Constraint::Distance { a, b, off, axis, .. } => {
            let (sa, sb) = (sh.at(p(a)?), sh.at(p(b)?));
            // THE TEXT STANDS ON THE FAR SIDE OF ITS LINE FROM THE GEOMETRY: a dimension above the geometry has its
            // text above its line, one below below it. A line lying on the geometry puts it above, or to the right.
            let away = |shift: f32, dflt: f32| if shift.abs() > 1e-3 { shift.signum() } else { dflt };
            Some(match axis {
                1 => {
                    let y = (sa.y + sb.y) / 2.0 + off as f32 * sc;
                    (Pos2::new(sa.x, y), Pos2::new(sb.x, y), egui::vec2(0.0, away(off as f32, -1.0)))
                }
                2 => {
                    let x = (sa.x + sb.x) / 2.0 + off as f32 * sc;
                    (Pos2::new(x, sa.y), Pos2::new(x, sb.y), egui::vec2(away(off as f32, 1.0), 0.0))
                }
                _ => {
                    let dir = (sb - sa).normalized();
                    let perp = egui::vec2(-dir.y, dir.x);
                    let shift = 16.0 + off as f32 * sc;
                    (sa + perp * shift, sb + perp * shift, perp * away(shift, 1.0))
                }
            })
        }
        Constraint::DistancePL { p: pt, a, b, off, .. } => {
            let (sp, sa) = (sh.at(p(pt)?), sh.at(p(a)?));
            let ab = line_screen_dir(project, &sh.view, si, a, b, sh.rect)?;
            let foot = sa + ab * (sp - sa).dot(ab);
            let o = ab * (off as f32 * sc); // the leader runs along the line measured from
                                            // its text beside it, on the side it was led to along that line - not along the dimension line itself
            Some((sp + o, foot + o, ab * if off < 0.0 { -1.0 } else { 1.0 }))
        }
        Constraint::EdgeDistance { c1, c2, m1, m2, off, .. } => {
            let (p1, p2) = (p(c1)?, p(c2)?);
            let (r1, r2) = (radius_of(project, si, c1).unwrap_or(0.0), radius_of(project, si, c2).unwrap_or(0.0));
            let len = ((p2.x - p1.x).powi(2) + (p2.y - p1.y).powi(2)).sqrt().max(1e-9);
            let (ux, uy) = ((p2.x - p1.x) / len, (p2.y - p1.y) / len);
            let e1 = Point2::new(p1.x - m1 as f64 * r1 * ux, p1.y - m1 as f64 * r1 * uy);
            let e2 = Point2::new(p2.x + m2 as f64 * r2 * ux, p2.y + m2 as f64 * r2 * uy);
            let (sa, sb) = (sh.at(e1), sh.at(e2));
            let dir = (sb - sa).normalized();
            let perp = egui::vec2(-dir.y, dir.x);
            let shift = 16.0 + off as f32 * sc;
            Some((sa + perp * shift, sb + perp * shift, if shift < 0.0 { -perp } else { perp }))
        }
        _ => None,
    }
}

/// Where the text of linear dimension `ci` stands, as drawn and as taken.
pub fn linear_text_of(project: &Project, si: usize, ci: usize, sh: &Sheet, set: &Settings) -> Option<(DimTextPlace, egui::Vec2)> {
    use qymcad_core::model::Constraint;
    let c = project.sketches.get(si)?.constraints.get(ci)?;
    let (la, lb, perp) = linear_dim_line(project, si, c, sh)?;
    let size = dim_text_size(&dim_caption(project, si, c, set)?, set.dim_font);
    let at = match *c {
        Constraint::Distance { at, .. } | Constraint::DistancePL { at, .. } | Constraint::EdgeDistance { at, .. } => at,
        _ => None,
    };
    Some((dim_text_place(la, lb, perp, size, at, set.dim_text == DimTextTurn::AlongLine), size))
}

/// THE TEXT OF A LINEAR DIMENSION LED ALONG ITS LINE by the pointer moving `delta` px: the share of the line it stands at
/// after the move, counted from where it stands now (the middle, or its shelf, when it was never placed).
pub fn linear_text_led(project: &Project, si: usize, ci: usize, sh: &Sheet, set: &Settings, delta: egui::Vec2) -> Option<f64> {
    let c = project.sketches.get(si)?.constraints.get(ci)?;
    let (la, lb, _) = linear_dim_line(project, si, c, sh)?;
    let len = (lb - la).length();
    if len < 1.0 {
        return None;
    }
    let dir = (lb - la) / len;
    let (place, _) = linear_text_of(project, si, ci, sh, set)?;
    let now = (place.center - la).dot(dir) / len;
    Some((now + delta.dot(dir) / len) as f64)
}

/// A RADIUS OR A DIAMETER ON THE SCREEN: its dimension line from `start` to the rim at `edge`, and its text - on a shelf
/// past the knee of the leader, or laid along the dimension line itself, turned by `angle`. One geometry for drawing the
/// dimension and for taking its text.
pub struct RadialDim {
    pub start: Pos2,
    pub edge: Pos2,
    pub knee: Pos2,
    pub text: Pos2,
    pub size: egui::Vec2,
    /// the turn of the text, radians: 0 on the shelf, the line's own direction (kept readable) on the line
    pub angle: f32,
    pub shelf: Option<[Pos2; 2]>,
}

/// AN ARC LENGTH ON THE SCREEN, drawn as the drawing standards draw it: a dimension arc about the same centre, out
/// past the measured arc by `ARC_DIM_GAP` px plus the dimension's own offset (`off`, mm along the radius), extension
/// lines from the ends of the arc out to it, arrows at both ends along it, and the number over its middle with a small
/// arc drawn over the number - the mark of an arc length. It was a caption beside the middle of the arc with a letter
/// L, no line and nothing to say what it measured. One geometry for drawing the dimension and for taking its text.
pub struct ArcLengthDim {
    /// the dimension arc, as screen points from its first end to its second
    pub arc: Vec<Pos2>,
    /// the two extension lines, from an end of the arc out past the dimension arc
    pub ext: [[Pos2; 2]; 2],
    /// the two arrows: the tip and the direction it points along the dimension arc
    pub arrows: [(Pos2, egui::Vec2); 2],
    /// the centre of the number
    pub text: Pos2,
    pub size: egui::Vec2,
    /// the arc mark over the number, as screen points
    pub mark: Vec<Pos2>,
}

/// How far the dimension arc of an arc length stands out past the arc it measures, in pixels, before its own offset.
pub(crate) const ARC_DIM_GAP: f32 = 14.0;

/// The screen geometry of arc length dimension `ci`; `None` for any other constraint.
pub fn arc_length_dim_geom(project: &Project, si: usize, ci: usize, sh: &Sheet, set: &Settings) -> Option<ArcLengthDim> {
    let s = project.sketches.get(si)?;
    let c = s.constraints.get(ci)?;
    let qymcad_core::model::Constraint::ArcLength { c: centre, a, b, ccw, off, .. } = *c else { return None };
    let (cp, pa, pb) = (sketch_pt(project, si, centre)?, sketch_pt(project, si, a)?, sketch_pt(project, si, b)?);
    let r = (pa.x - cp.x).hypot(pa.y - cp.y);
    let scale = sh.view.scale as f64;
    let rd = (r + ARC_DIM_GAP as f64 / scale + off).max(r * 0.2); // never through the centre
    let (a0, a1) = ((pa.y - cp.y).atan2(pa.x - cp.x), (pb.y - cp.y).atan2(pb.x - cp.x));
    let tau = std::f64::consts::TAU;
    let sweep = if ccw { (a1 - a0).rem_euclid(tau) } else { -(a0 - a1).rem_euclid(tau) };
    let at = |ang: f64, rad: f64| sh.at(Point2::new(cp.x + rad * ang.cos(), cp.y + rad * ang.sin()));
    let steps = ((sweep.abs() * rd * scale / 4.0).ceil() as usize).clamp(4, 96);
    let arc: Vec<Pos2> = (0..=steps).map(|k| at(a0 + sweep * k as f64 / steps as f64, rd)).collect();
    let past = rd + 3.0 / scale; // the extension lines run a little past the dimension arc
    let ext = [[at(a0, r), at(a0, past)], [at(a0 + sweep, r), at(a0 + sweep, past)]];
    let along = |i: usize, j: usize| (arc[j] - arc[i]).normalized();
    let n = arc.len() - 1;
    let arrows = [(arc[0], along(1, 0)), (arc[n], along(n - 1, n))];
    let mid = a0 + sweep / 2.0;
    let size = dim_text_size(&dim_caption(project, si, c, set)?, set.dim_font);
    let out = egui::vec2(mid.cos() as f32, -(mid.sin() as f32)); // the screen's y runs down
    let text = at(mid, rd) + out * (size.y * 0.5 + 9.0);
    // the arc mark: a small arc over the number, bulging away from the dimension arc
    let half = size.x.min(14.0) * 0.5;
    let side = egui::vec2(out.y, -out.x);
    let crown = text + out * (size.y * 0.5 + 4.0);
    let mark: Vec<Pos2> = (0..=8)
        .map(|k| {
            let t = k as f32 / 8.0 * 2.0 - 1.0;
            crown + side * (t * half) + out * ((1.0 - t * t) * 3.0)
        })
        .collect();
    Some(ArcLengthDim { arc, ext, arrows, text, size, mark })
}

/// The screen geometry of radius or diameter dimension `ci`; `None` for any other constraint.
pub fn radial_dim_geom(project: &Project, si: usize, ci: usize, sh: &Sheet, set: &Settings) -> Option<RadialDim> {
    use qymcad_core::model::Constraint;
    let c = project.sketches.get(si)?.constraints.get(ci)?;
    let Constraint::Diameter { c: centre, off, diam, at, .. } = *c else { return None };
    let cp = sketch_pt(project, si, centre)?;
    let r = radius_of(project, si, centre)?;
    let sc = sh.at(cp);
    let r_px = (sh.at(Point2::new(cp.x + r, cp.y)) - sc).length();
    let dir = egui::vec2((off as f32).cos(), (off as f32).sin());
    let (start, edge, knee) = (if diam { sc - dir * r_px } else { sc }, sc + dir * r_px, sc + dir * (r_px + 14.0));
    let size = dim_text_size(&dim_caption(project, si, c, set)?, set.dim_font);
    Some(match at {
        None => {
            let (text, shelf) = radial_text_place(knee, dir, size);
            RadialDim { start, edge, knee, text, size, angle: 0.0, shelf: Some(shelf) }
        }
        Some(t) => {
            // ON THE LINE: turned along it, kept reading left to right, and standing above it by a gap
            let angle = readable_angle(dir.y.atan2(dir.x));
            let up = egui::vec2(angle.sin(), -angle.cos());
            let on = start + (edge - start) * t as f32;
            RadialDim { start, edge, knee, text: on + up * (size.y / 2.0 + DIM_TEXT_GAP), size, angle, shelf: None }
        }
    })
}

/// THE TEXT OF A RADIUS OR A DIAMETER FOLLOWS THE POINTER: the dimension line turns to it, and the text lies on the line
/// while the pointer is within the circle (a little past its rim), on the shelf once it is led farther out. Answers
/// `(off, at)`.
pub fn radial_text_follow(project: &Project, si: usize, ci: usize, sh: &Sheet, pp: Pos2) -> Option<(f64, Option<f64>)> {
    use qymcad_core::model::Constraint;
    let c = project.sketches.get(si)?.constraints.get(ci)?;
    let Constraint::Diameter { c: centre, off, diam, .. } = *c else { return None };
    let cp = sketch_pt(project, si, centre)?;
    let r = radius_of(project, si, centre)?;
    let sc = sh.at(cp);
    let r_px = (sh.at(Point2::new(cp.x + r, cp.y)) - sc).length().max(1.0);
    let w = pp - sc;
    let rho = w.length();
    let ang = if rho > 4.0 { w.y.atan2(w.x) as f64 } else { off };
    if rho > r_px + 8.0 {
        return Some((ang, None));
    }
    let t = if diam { (rho + r_px) / (2.0 * r_px) } else { rho / r_px };
    Some((ang, Some(t.clamp(0.05, 0.95) as f64)))
}

/// How far `pos` is from a text of `size` centred at `center` and turned by `angle`: zero anywhere on the text.
pub fn label_reach_turned(center: Pos2, size: egui::Vec2, angle: f32, pos: Pos2) -> f32 {
    let d = pos - center;
    let (c, s) = (angle.cos(), angle.sin());
    let local = egui::vec2(d.x * c + d.y * s, -d.x * s + d.y * c); // the pointer in the frame of the text
    label_reach(Pos2::ZERO, size, local.to_pos2())
}

/// WHERE THE TEXT OF A RADIUS OR A DIAMETER STANDS: the leader breaks at `knee` into a shelf running the way the leader
/// points, and the text stands on the shelf rather than on the leader. Answers the middle of the text and the shelf.
pub fn radial_text_place(knee: Pos2, toward: egui::Vec2, size: egui::Vec2) -> (Pos2, [Pos2; 2]) {
    let sign = if toward.x < 0.0 { -1.0 } else { 1.0 };
    let end = knee + egui::vec2(sign * (size.x + 2.0 * DIM_TEXT_GAP), 0.0);
    (knee + egui::vec2(sign * (size.x / 2.0 + DIM_TEXT_GAP), -(size.y / 2.0 + DIM_TEXT_GAP)), [knee, end])
}

/// The default radius of the arc of an angle, px, before one is placed.
pub const ANGLE_ARC_PX: f32 = 24.0;

/// Where the sides of an angular dimension meet and run, on the screen, and where its arc is placed.
pub(crate) struct AngleSides {
    /// where the sides meet
    pub(crate) center: Pos2,
    /// each side by its two ends, the far end first, which gives the side its direction
    pub(crate) s1: [Pos2; 2],
    pub(crate) s2: [Pos2; 2],
    /// the radius of the arc, in sketch units; 0 is the default distance on screen
    pub(crate) off: f64,
    /// where the label stands along the arc, as a share of the angle from the first side; none on the bisector
    pub(crate) at: Option<f64>,
}

/// Where the sides of angular dimension `ci` of sketch `si` meet and run, on the screen.
pub(crate) fn angle_sides(project: &Project, si: usize, c: &qymcad_core::model::Constraint, sh: &Sheet) -> Option<AngleSides> {
    use qymcad_core::model::Constraint;
    let p = |id| sketch_pt(project, si, id).map(|w| sh.at(w));
    match *c {
        Constraint::Angle { a, b, c, off, at, .. } => {
            let (sa, sb, sc) = (p(a)?, p(b)?, p(c)?);
            Some(AngleSides { center: sb, s1: [sa, sb], s2: [sc, sb], off, at })
        }
        Constraint::AngleLines { a, b, c, d, off, at, .. } => {
            let (sa, sb, sc, sd) = (p(a)?, p(b)?, p(c)?, p(d)?);
            let ix = lines_intersect(sa, sb, sc, sd)?;
            // A SIDE THAT ENDS WHERE THE SIDES MEET runs on past its end the way its line runs, first end to second:
            // the angle of a chamfer is measured at the end of the cut, and the side taken from the meeting to that
            // same end had no length and so no direction - the arc of the angle turned with the rounding noise.
            let far = |first: Pos2, second: Pos2| if (second - ix).length() < 1.0 { ix + (second - first).normalized() } else { second };
            Some(AngleSides { center: ix, s1: [far(sa, sb), sa], s2: [far(sc, sd), sc], off, at })
        }
        _ => None,
    }
}

/// The screen geometry of angular dimension `ci` of sketch `si`; `None` for any other constraint.
pub fn angle_dim_geom(project: &Project, si: usize, ci: usize, sh: &Sheet, set: &Settings) -> Option<AngleDim> {
    let c = project.sketches.get(si)?.constraints.get(ci)?;
    let size = dim_text_size(&dim_caption(project, si, c, set)?, set.dim_font);
    let AngleSides { center, s1, s2, off, at } = angle_sides(project, si, c, sh)?;
    let (u, v) = ((s1[0] - center).normalized(), (s2[0] - center).normalized());
    let pi = std::f32::consts::PI;
    let a0 = u.y.atan2(u.x);
    let mut sweep = v.y.atan2(v.x) - a0;
    while sweep > pi {
        sweep -= 2.0 * pi;
    }
    while sweep < -pi {
        sweep += 2.0 * pi;
    }
    let r = if off > 0.0 { (sh.at(Point2::new(off, 0.0)) - sh.at(Point2::new(0.0, 0.0))).length() } else { ANGLE_ARC_PX };
    let t = at.unwrap_or(0.5) as f32;
    let (lo, hi) = (t.min(0.0), t.max(1.0));
    let la = a0 + sweep * t;
    let ld = egui::vec2(la.cos(), la.sin());
    // the whole box of the text stands outside the arc: its half extent along the direction plus a gap
    let label = center + ld * (r + angle_label_clear(ld, size));
    // A SIDE SHORTER THAN THE ARC, or one that starts past it, is carried to the arc by a thin extension line with a
    // 4 px overshoot, as on a drawing
    let mut ext = Vec::new();
    for (side, dir) in [(s1, u), (s2, v)] {
        let along = |q: Pos2| (q - center).dot(dir);
        let (near, far) = (along(side[0]).min(along(side[1])).max(0.0), along(side[0]).max(along(side[1])));
        if r > far + 1.0 {
            ext.push([center + dir * far, center + dir * (r + 4.0)]);
        } else if r < near - 1.0 {
            ext.push([center + dir * (r - 4.0), center + dir * near]);
        }
    }
    Some(AngleDim { center, a0, sweep, r, arc: (a0 + sweep * lo, a0 + sweep * hi), label, ext })
}

/// How far past the arc the middle of an angle's label stands in direction `ld`: its half extent that way and a gap.
pub(crate) fn angle_label_clear(ld: egui::Vec2, size: egui::Vec2) -> f32 {
    (ld.x.abs() * size.x + ld.y.abs() * size.y) / 2.0 + DIM_TEXT_GAP + 2.0
}

/// THE LABEL OF AN ANGLE FOLLOWS THE POINTER: the radius of the arc is the pointer's distance from where the sides
/// meet (sketch units), and the place along it the share of the angle the pointer stands at - past a side the arc
/// runs on to it. Answers `(off, at)`, or `None` for any other constraint.
pub fn angle_dim_follow(project: &Project, si: usize, ci: usize, sh: &Sheet, set: &Settings, cur: Pos2) -> Option<(f64, Option<f64>)> {
    let g = angle_dim_geom(project, si, ci, sh, set)?;
    let size = dim_text_size(&dim_caption(project, si, project.sketches.get(si)?.constraints.get(ci)?, set)?, set.dim_font);
    let w = cur - g.center;
    let r_px = w.length();
    if r_px < 1.0 || g.sweep.abs() < 1e-4 {
        return None;
    }
    let u = egui::vec2(g.a0.cos(), g.a0.sin());
    let phi = (u.x * w.y - u.y * w.x).atan2(u.dot(w)); // signed turn from the first side to the pointer
    let t = (phi / g.sweep).clamp(-1.0, 2.0);
    let per_px = (sh.at(Point2::new(1.0, 0.0)) - sh.at(Point2::new(0.0, 0.0))).length().max(1e-9);
    Some((((r_px - angle_label_clear(w / r_px, size)).max(4.0) / per_px) as f64, Some(t as f64)))
}
