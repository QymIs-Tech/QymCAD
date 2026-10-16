//! THE CONSTRAINTS of a sketch: laid, refused, offered on the bar.

use qymcad_ui_state::Notice;
use super::*;

/// THE ENTITIES A CONSTRAINT TOUCHES, derived from its points.
///
/// The list of constraints used to name only THE KIND: horizontal, horizontal, vertical, vertical - four
/// identical rows. On a sketch with thirty constraints such a list is useless: it shows that constraints
/// exist but gives no way to find the one wanted. Highlighting on hover already existed, but it answers
/// "where is this one" rather than "which one do I need".
pub fn constraint_parts(dc: &qymcad_ui_state::DrawCtx, si: usize, c: &qymcad_core::model::Constraint) -> Vec<String> {
    use qymcad_core::model::EntityKind as EK;
    let Some(s) = dc.project.sketches.get(si) else { return Vec::new() };
    let owner = |pid: Id| -> Option<Id> {
        s.entities
            .iter()
            .find(|e| match e.kind {
                EK::Line { a, b } => a == pid || b == pid,
                EK::Arc { center, a, b, .. } => center == pid || a == pid || b == pid,
                EK::Circle { center, .. } => center == pid,
                EK::Ellipse { c, ma, mi } => c == pid || ma == pid || mi == pid,
            })
            .map(|e| e.id)
    };
    let mut out: Vec<String> = Vec::new();
    for pid in c.points() {
        if let Some(eid) = owner(pid) {
            let name = sketch_entity_name(dc.project, si, eid);
            if !name.is_empty() && !out.contains(&name) {
                out.push(name);
            }
        }
    }
    out
}

pub fn try_constraint_inner(
    project: &mut Project,
    regen: &mut qymcad_ui_state::Rebuilding,
    sel: qymcad_ui_state::Sel,
    sel_sk: &mut qymcad_ui_state::SketchSelection,
    status: &mut qymcad_ui_state::Notice,
    code: u8,
) -> bool {
    use qymcad_core::model::Constraint;
    let qymcad_ui_state::Sel::Sketch(si) = sel else { return false };
    middles_for(project, si, sel_sk, code);
    let pts = qymcad_ui_state::sel_point_ids(sel_sk);
    let mut lines = qymcad_ui_state::sel_line_pts(project, sel_sk, si);
    // the lines drawn, before the axes join them: Vertical and Horizontal go on these, an axis already stands so
    let drawn = lines.clone();
    // coordinate axes picked as lines (kind 3): their straight lines are materialised and used as lines
    let axes: Vec<u64> = sel_sk.items.iter().filter(|(k, _)| *k == 3).map(|(_, id)| *id).collect();
    for w in axes {
        let (o, d) = project.ensure_axis(si, w as usize);
        lines.push((o, d));
    }
    let mut new: Vec<Constraint> = Vec::new();
    match code {
        0 if pts.len() >= 2 => new.push(Constraint::Coincident { a: pts[0], b: pts[1] }),
        // one point coincident with an entity: a circle or an arc gives point-on-circle (which takes
        // priority), otherwise a line or an axis gives point-on-line
        0 if pts.len() == 1 => {
            let cs = qymcad_ui_state::sel_circle_centers(project, sel_sk, si);
            if !cs.is_empty() {
                new.push(Constraint::PointOnCircle { p: pts[0], c: cs[0] });
            } else if !lines.is_empty() {
                new.push(Constraint::PointOnLine { p: pts[0], a: lines[0].0, b: lines[0].1 });
            }
        }
        // EVERY SELECTED LINE GETS IT, not the first alone; with no line selected, every point lines up with the first.
        // Reported behaviour (issue #34): with three lines selected Vertical turned one of them.
        1 if !drawn.is_empty() => new.extend(drawn.iter().map(|&(a, b)| Constraint::Horizontal { a, b })),
        1 if pts.len() >= 2 => new.extend(pts[1..].iter().map(|&b| Constraint::Horizontal { a: pts[0], b })),
        2 if !drawn.is_empty() => new.extend(drawn.iter().map(|&(a, b)| Constraint::Vertical { a, b })),
        2 if pts.len() >= 2 => new.extend(pts[1..].iter().map(|&b| Constraint::Vertical { a: pts[0], b })),
        // between lines: every further line is tied to the first one picked
        3 if lines.len() >= 2 => new.extend(lines[1..].iter().map(|&(c, d)| Constraint::Parallel { a: lines[0].0, b: lines[0].1, c, d })),
        4 if lines.len() >= 2 => new.push(Constraint::Perpendicular { a: lines[0].0, b: lines[0].1, c: lines[1].0, d: lines[1].1 }),
        5 if lines.len() >= 2 => new.extend(lines[1..].iter().map(|&(c, d)| Constraint::Equal { a: lines[0].0, b: lines[0].1, c, d })),
        5 => {
            // equal radii of two circles
            let cs = qymcad_ui_state::sel_circle_centers(project, sel_sk, si);
            if cs.len() >= 2 {
                new.push(Constraint::EqualRadius { c1: cs[0], c2: cs[1] });
            }
        }
        6 if !pts.is_empty() => new.extend(pts.iter().map(|p| Constraint::Fixed { p: *p })),
        7 if lines.len() >= 2 => new.extend(lines[1..].iter().map(|&(c, d)| Constraint::Collinear { a: lines[0].0, b: lines[0].1, c, d })),
        8 => {
            // concentricity is A REAL kind with a glyph of its own, not a fake made of coincident centres
            let centers = qymcad_ui_state::sel_circle_centers(project, sel_sk, si);
            if centers.len() >= 2 {
                new.push(Constraint::Concentric { c1: centers[0], c2: centers[1] });
            }
        }
        9 => {
            // tangency: a line plus a circle, the line touching it at a point of its own (`touch_point`)
            if let (Some((la, lb)), Some((cc, rr))) = (lines.first().copied(), qymcad_ui_state::sel_circle_cr(project, sel_sk, si)) {
                new.push(Constraint::Tangent { a: la, b: lb, c: cc, r: rr });
                if let Some(p) = touch_point(project, si, sel_sk, (la, lb), cc) {
                    new.push(Constraint::PointOnCircle { p, c: cc });
                }
            }
        }
        10 => {
            // symmetry: two points about an axis (the picked line)
            if pts.len() >= 2 {
                if let Some((la, lb)) = lines.first().copied() {
                    new.push(Constraint::Symmetric { a: pts[0], b: pts[1], la, lb });
                }
            }
        }
        11 if pts.len() == 1 && !lines.is_empty() => {
            // midpoint: the point at the middle of the picked line
            new.push(Constraint::Midpoint { p: pts[0], a: lines[0].0, b: lines[0].1 });
        }
        _ => {}
    }
    if new.is_empty() {
        return false;
    }
    // WHAT IS HELD ALREADY IS NOT LAID AGAIN: a second Vertical on a line that has one turned both redundant (found
    // checking issue #34). With nothing left to add, the sketch says so instead of "The constraint is added".
    let new = project.independent_of(si, new);
    if new.is_empty() {
        sel_sk.clear();
        *status = Notice::info(qymcad_i18n::tr("sk-constraint-already"));
        return true;
    }
    // the points of what was selected: the relations among them that the new ones make redundant are lifted below
    let among: std::collections::HashSet<Id> = pts.iter().copied().chain(lines.iter().flat_map(|&(a, b)| [a, b])).collect();
    let (had, old_pts): (usize, Vec<(f64, f64)>) = (project.sketches[si].constraints.len(), project.sketches[si].points.iter().map(|p| (p.x, p.y)).collect());
    // A LINE MADE HORIZONTAL OR VERTICAL IS TURNED TO IT FIRST, about its middle (about an end that is held), keeping its
    // length: a line given a direction is turned to it, not squashed onto it. The solve takes the least movement of the points, and bringing both
    // ends to one height is less than turning a steep line: a line at 80 deg given Horizontal came out 5 mm long of 30,
    // and a vertical one, its Vertical deleted, was refused as pulled into a point. Taken back with the rest if the
    // constraint is not kept.
    for c in &new {
        let (line, level) = match *c {
            Constraint::Horizontal { a, b } if drawn.contains(&(a, b)) => ((a, b), Level::Horizontal),
            Constraint::Vertical { a, b } if drawn.contains(&(a, b)) => ((a, b), Level::Vertical),
            _ => continue,
        };
        turn_to_level(project, si, line, level);
    }
    // EQUAL ON TWO CIRCLES THAT EACH CARRY THEIR RADIUS: the second one's radius becomes a reference and follows the
    // first - one radius, not a contradiction. Reported behaviour: the circle tool gives each circle its dimension, and
    // Equal on two of them left the sketch over-defined (residual 2.89).
    let referenced: Vec<usize> = new
        .iter()
        .filter_map(|c| match c {
            Constraint::EqualRadius { c1, c2 } => {
                let driving = |cc: Id| project.sketches[si].constraints.iter().position(|k| matches!(k, Constraint::Diameter { c, driven: false, .. } if *c == cc));
                driving(*c1).and(driving(*c2))
            }
            _ => None,
        })
        .collect();
    for &k in &referenced {
        if let Constraint::Diameter { driven, .. } = &mut project.sketches[si].constraints[k] {
            *driven = true;
        }
    }
    let sizes_before = shape_sizes(project, si);
    project.sketches[si].constraints.extend(new);
    let resid = project.solve_sketch(si);
    // A CONSTRAINT MET BY SHRINKING A SHAPE TO NOTHING IS NOT MET. Horizontal over Vertical on one line is satisfied
    // exactly by a line of no length - residual 0 - and both stood green while the line was gone. Reported behaviour
    // (found checking issue #34): three lines made Vertical, then Horizontal, shrank to points.
    let shrunk = shape_sizes(project, si).iter().zip(&sizes_before).any(|(after, before)| *before > 1e-6 && *after < 1e-6);
    sel_sk.clear(); // the selection and whatever was waiting for it
    qymcad_ui_state::invalidate(regen);
    // A CONSTRAINT THE SKETCH CANNOT MEET IS NOT KEPT, as a dimension that does not fit is not: parallel on two lines an
    // angle of 45 deg holds apart left both in the sketch, unsolved (residual 0.79), with nothing marked. It is taken
    // back, the points return where they were, and the status line says why. 1e-2 mm is the threshold the dimension
    // editor rolls back at: above the solver's noise, below any real conflict.
    if resid > 1e-2 || shrunk {
        project.sketches[si].constraints.truncate(had);
        for &k in &referenced {
            if let Constraint::Diameter { driven, .. } = &mut project.sketches[si].constraints[k] {
                *driven = false; // taken back with the constraint
            }
        }
        for (p, (x, y)) in project.sketches[si].points.iter_mut().zip(old_pts) {
            p.x = x;
            p.y = y;
        }
        project.solve_sketch(si);
        *status = Notice::refusal(if shrunk { qymcad_i18n::tr("sk-constraint-shrinks") } else { qymcad_i18n::tr1("sk-constraint-conflict", "r", &qymcad_i18n::num(resid, 2)) });
        return true;
    }
    let lifted = project.drop_implied_relations(si, had, &among);
    *status = if resid >= 1e-3 {
        Notice::info(qymcad_i18n::tr1("sk-constraint-added-resid", "r", &qymcad_i18n::num(resid, 2)))
    } else {
        Notice::info(if lifted > 0 { qymcad_i18n::tr1("sk-constraint-added-implied", "n", &lifted.to_string()) } else { qymcad_i18n::tr("sk-constraint-added") })
    };
    true
}

/// Apply a geometric constraint to the sketch selection. Returns true when it was applied.
/// `code`: 0 coincident, 1 horizontal, 2 vertical, 3 parallel, 4 perpendicular, 5 equal, 6 fixed,
/// 7 collinear, 8 concentric.
pub fn try_constraint(sk: &mut qymcad_ui_state::SketchCtx, code: u8) -> bool {
    // THE BOUNDARY OF AN OPERATION: placing a constraint is a deliberate act and makes one undo step.
    qymcad_ui_state::begin_edit(sk.edits, sk.project, qymcad_i18n::tr("sk-constraint"));
    let ok = try_constraint_inner(sk.project, sk.regen, *sk.sel, sk.sel_sk, sk.status, code);
    if ok {
        qymcad_ui_state::commit_edit(&mut sk.rebuild());
    } else {
        qymcad_ui_state::abort_edit(&mut sk.rebuild()); // the constraint did not take, so it leaves no trace
    }
    ok
}

/// The constraint button: if the selection is enough, apply it; otherwise go into waiting mode (the
/// button lights up, the elements are picked, Esc cancels).
pub fn constraint_button(sk: &mut qymcad_ui_state::SketchCtx, code: u8) {
    qymcad_ui_state::exit_draw_tools(&mut qymcad_ui_state::tools_in!(sk));
    *sk.armed = qymcad_ui_state::Armed::None;
    sk.sel_sk.constraint = None;
    sk.sel_sk.modify = None;
    let picked = !sk.sel_sk.items.is_empty();
    if try_constraint(sk, code) {
        sk.sel_sk.constraint = None;
    } else {
        // it could not be applied to the current selection, so the picking starts AFRESH, without any
        // elements left stuck: otherwise they would go into the constraint as `pts[0]` or `lines[0]` and
        // tie the wrong things together.
        sk.sel_sk.clear(); // the selection and whatever was waiting for it
        sk.sel_sk.constraint = Some(code);
        // WHAT WAS PICKED DOES NOT TAKE IT, and that is said: two points are not parallel, two lines have no centre.
        // Asking for the geometry again as if nothing had been picked left the person guessing what was wrong.
        *sk.status = if picked {
            Notice::refusal(qymcad_i18n::tr1("sk-constraint-not-for-pick", "takes", &qymcad_i18n::tr(constraint_hint_key(code))))
        } else {
            Notice::info(qymcad_i18n::tr("sk-pick-for-constraint"))
        };
    }
}

/// The words of a constraint's button - its name and what it ties - by its code.
pub fn constraint_hint_key(code: u8) -> &'static str {
    match code {
        0 => "con-coincident-hint",
        1 => "con-horizontal-hint",
        2 => "con-vertical-hint",
        3 => "con-parallel-hint",
        4 => "con-perpendicular-hint",
        5 => "con-equal",
        6 => "con-fix",
        7 => "con-collinear-hint",
        8 => "con-concentric-hint",
        9 => "con-tangent-hint",
        10 => "con-symmetric-hint",
        _ => "con-midpoint-hint",
    }
}
