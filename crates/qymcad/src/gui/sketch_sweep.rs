//! A SWEEP OVER THE SKETCH: build it, constrain it, DRAG it — and see what breaks along the way.
//!
//! The task as set: build geometry, drag it about, see what turns red in the constraints wrongly, what
//! breaks the solver, what breaks the geometry during building and moving; apply different tools in
//! different combinations. The reason is a solid one: while the sketch help was being written, the eye
//! found more than all the guards together — and found it on combinations rather than on separate tools.
//!
//! THERE ARE TWO HALVES HERE, AND THEY DIFFER IN NATURE:
//!
//! * **Numbers** work always: the finiteness of the coordinates, dangling references in constraints, the
//!   sign of the degrees of freedom, the residual of the solver, "a defined sketch does not budge", "an
//!   underdefined one follows the cursor". That is what cannot be seen by eye on a single picture.
//! * **Pictures** (`#[ignore]`, `target/sketch-sweep/`) are what cannot be checked by a number: whether
//!   what should have turned red did, whether the badges have drifted apart, whether the result reads.
//!
//! ONE SCENE SERVES BOTH HALVES. Otherwise the picture would show one thing while the numbers were
//! computed from another, and nobody would notice the divergence.
#[cfg(test)]
mod tests {
    use super::super::App;
    use qymcad_core::model::{Constraint, Id};

    /// What a scene checks after the drag.
    struct Case {
        /// The sketch of the scene.
        si: usize,
        /// The point that gets dragged, and where to.
        drag: Option<(Id, f64, f64)>,
        /// The scene IS MEANT to be contradictory — the residual of the solver is large and must be.
        conflicting: bool,
    }

    /// An empty sketch inside one part: the nodes must not scatter across components.
    fn empty() -> (App, usize) {
        let mut app = App::default();
        let part = app.project.add_component("part");
        app.enter_component_for_test(part);
        let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
        (app, si)
    }

    /// A sketch point by its coordinates — that is how a mouse finds it too.
    fn point_at(app: &App, si: usize, x: f64, y: f64) -> Id {
        let s = &app.project.sketches[si];
        s.points
            .iter()
            .min_by(|a, b| {
                let d = |p: &qymcad_core::model::SketchPoint| (p.x - x).powi(2) + (p.y - y).powi(2);
                d(a).partial_cmp(&d(b)).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|p| p.id)
            .expect("the sketch has no points at all")
    }

    /// The coordinates of a point.
    fn xy(app: &App, si: usize, id: Id) -> (f64, f64) {
        app.project.sketches[si].points.iter().find(|p| p.id == id).map(|p| (p.x, p.y)).expect("the point has gone from the sketch")
    }

    /// EVERY SCENE OF THE SWEEP. Combinations rather than separate tools: a tool on its own was checked
    /// long ago, and what breaks is the joint between them.
    #[allow(clippy::type_complexity)]
    fn scenes() -> Vec<(&'static str, fn() -> (App, Case))> {
        vec![
            ("01-rect-free", || {
                let (mut app, si) = empty();
                app.project.add_rect_entity(si, -30.0, -20.0, 30.0, 20.0, qymcad_core::feature::Purpose::Real);
                app.project.regen_sketch(si);
                let p = point_at(&app, si, 30.0, 20.0);
                (app, Case { si, drag: Some((p, 45.0, 34.0)), conflicting: false })
            }),
            ("02-rect-fully-dimensioned", || {
                let (mut app, si) = empty();
                app.project.add_rect_entity(si, 0.0, 0.0, 40.0, 25.0, qymcad_core::feature::Purpose::Real);
                let a = point_at(&app, si, 0.0, 0.0);
                let b = point_at(&app, si, 40.0, 0.0);
                let c = point_at(&app, si, 40.0, 25.0);
                // THE CORNER NEEDS NO FIXING: it landed on the origin, and the origin is fixed by
                // itself. The first edition added a `Fixed` of its own here — out came A DUPLICATE, the
                // sketch honestly showed two redundant constraints, and that was nearly written down as a
                // finding. Checking must go BY NUMBERS.
                dim(&mut app, si, a, b, 40.0);
                dim(&mut app, si, b, c, 25.0);
                app.project.solve_sketch(si);
                app.project.regen_sketch(si);
                (app, Case { si, drag: Some((c, 80.0, 60.0)), conflicting: false })
            }),
            ("03-fillet-corner", || {
                let (mut app, si) = empty();
                app.project.add_rect_entity(si, -25.0, -16.0, 25.0, 16.0, qymcad_core::feature::Purpose::Real);
                app.project.regen_sketch(si);
                let corners: Vec<Id> = app.project.sketches[si].points.iter().map(|p| p.id).collect();
                for pid in corners {
                    app.project.fillet_at_vertex(si, pid, 6.0);
                }
                app.project.regen_sketch(si);
                let p = point_at(&app, si, -25.0, -16.0);
                (app, Case { si, drag: Some((p, -34.0, -22.0)), conflicting: false })
            }),
            ("04-circle-with-radius", || {
                let (mut app, si) = empty();
                app.project.add_circle_entity(si, 10.0, 5.0, 12.0, qymcad_core::feature::Purpose::Real);
                app.project.regen_sketch(si);
                let c = point_at(&app, si, 10.0, 5.0);
                (app, Case { si, drag: Some((c, 25.0, 18.0)), conflicting: false })
            }),
            ("05-slot", || {
                let (mut app, si) = empty();
                app.project.add_slot_entity(si, -18.0, 0.0, 18.0, 0.0, 7.0, qymcad_core::feature::Purpose::Real);
                app.project.regen_sketch(si);
                let p = point_at(&app, si, 18.0, 0.0);
                (app, Case { si, drag: Some((p, 30.0, 9.0)), conflicting: false })
            }),
            ("06-polygon", || {
                let (mut app, si) = empty();
                app.project.add_polygon_entity(si, 0.0, 0.0, 20.0, 0.0, 6, qymcad_core::feature::Purpose::Real);
                app.project.regen_sketch(si);
                let p = point_at(&app, si, 20.0, 0.0);
                (app, Case { si, drag: Some((p, 28.0, 8.0)), conflicting: false })
            }),
            ("06b-polygon-rotated", || {
                // ROTATED: a check that the radius leader goes into the gap at ANY rotation and not only
                // when the first vertex points right.
                let (mut app, si) = empty();
                app.project.add_polygon_entity(si, 0.0, 0.0, 14.0, 14.0, 5, qymcad_core::feature::Purpose::Real);
                app.project.regen_sketch(si);
                let p = point_at(&app, si, 14.0, 14.0);
                (app, Case { si, drag: Some((p, 22.0, 16.0)), conflicting: false })
            }),
            ("07-ellipse", || {
                let (mut app, si) = empty();
                // THE CENTRE IS AWAY FROM THE ORIGIN. The first edition put the ellipse at zero, and
                // `point_at(0,0)` found NOT its centre but the fixed origin of the sketch: "the ellipse
                // does not drag" turned out to be dragging the very thing that must not move.
                app.project.add_ellipse_entity(si, 14.0, 8.0, 24.0, 12.0, 0.0, qymcad_core::feature::Purpose::Real);
                app.project.regen_sketch(si);
                let p = point_at(&app, si, 14.0, 8.0);
                (app, Case { si, drag: Some((p, 26.0, 18.0)), conflicting: false })
            }),
            ("08-arc-and-lines", || {
                let (mut app, si) = empty();
                app.project.add_line_entity(si, -30.0, -10.0, 0.0, -10.0, qymcad_core::feature::Purpose::Real);
                app.project.add_arc_entity(si, 0.0, 0.0, 0.0, -10.0, 10.0, 0.0, qymcad_core::feature::Winding::Ccw, qymcad_core::feature::Purpose::Real);
                app.project.add_line_entity(si, 10.0, 0.0, 30.0, 0.0, qymcad_core::feature::Purpose::Real);
                app.project.regen_sketch(si);
                let p = point_at(&app, si, -30.0, -10.0);
                (app, Case { si, drag: Some((p, -40.0, -18.0)), conflicting: false })
            }),
            ("09-perpendicular-and-equal", || {
                let (mut app, si) = empty();
                app.project.add_line_entity(si, 0.0, 0.0, 30.0, 0.0, qymcad_core::feature::Purpose::Real);
                app.project.add_line_entity(si, 30.0, 0.0, 30.0, 20.0, qymcad_core::feature::Purpose::Real);
                app.project.regen_sketch(si);
                let (a, b) = (point_at(&app, si, 0.0, 0.0), point_at(&app, si, 30.0, 0.0));
                let d = point_at(&app, si, 30.0, 20.0);
                // The joining points of the lines ARE SHARED (both were built through
                // `sketch_point_at`), so there is nothing to make coincident. The first edition wrote
                // `Coincident { a: b, b }` — a point with itself, two empty rows in the Jacobian and a
                // "redundancy" that does not exist.
                app.project.sketches[si].constraints.push(Constraint::Perpendicular { a, b, c: b, d });
                app.project.sketches[si].constraints.push(Constraint::Equal { a, b, c: b, d });
                app.project.solve_sketch(si);
                app.project.regen_sketch(si);
                (app, Case { si, drag: Some((d, 44.0, 30.0)), conflicting: false })
            }),
            ("10-two-rects-mirrored", || {
                let (mut app, si) = empty();
                app.project.add_rect_entity(si, 6.0, -10.0, 26.0, 10.0, qymcad_core::feature::Purpose::Real);
                app.project.add_rect_entity(si, -26.0, -10.0, -6.0, 10.0, qymcad_core::feature::Purpose::Real);
                app.project.regen_sketch(si);
                let p = point_at(&app, si, 26.0, 10.0);
                (app, Case { si, drag: Some((p, 36.0, 18.0)), conflicting: false })
            }),
            ("11-redundant-dimension", || {
                // A REDUNDANT BUT NOT CONTRADICTORY dimension: the same value the constraints already
                // hold. It must be MARKED redundant and must NOT turn red as a conflict.
                let (mut app, si) = empty();
                app.project.add_rect_entity(si, 0.0, 0.0, 40.0, 25.0, qymcad_core::feature::Purpose::Real);
                let a = point_at(&app, si, 0.0, 0.0);
                let b = point_at(&app, si, 40.0, 0.0);
                let c = point_at(&app, si, 40.0, 25.0);
                let d = point_at(&app, si, 0.0, 25.0);
                dim(&mut app, si, a, b, 40.0);
                dim(&mut app, si, b, c, 25.0);
                dim(&mut app, si, d, c, 40.0); // the same width along the top — already held by the rectangularity
                app.project.solve_sketch(si);
                app.project.regen_sketch(si);
                (app, Case { si, drag: Some((c, 60.0, 40.0)), conflicting: false })
            }),
            ("12-conflicting-dimensions", || {
                // THE CONTRADICTION IS DELIBERATE: two incompatible lengths for one side.
                let (mut app, si) = empty();
                app.project.add_rect_entity(si, 0.0, 0.0, 40.0, 25.0, qymcad_core::feature::Purpose::Real);
                let a = point_at(&app, si, 0.0, 0.0);
                let b = point_at(&app, si, 40.0, 0.0);
                dim(&mut app, si, a, b, 40.0);
                dim(&mut app, si, a, b, 55.0);
                app.project.solve_sketch(si);
                app.project.regen_sketch(si);
                (app, Case { si, drag: None, conflicting: true })
            }),
        ]
    }

    /// A linear dimension between two points.
    fn dim(app: &mut App, si: usize, a: Id, b: Id, d: f64) {
        app.project.sketches[si].constraints.push(Constraint::Distance { a, b, d, off: 0.0, expr: String::new(), driven: false, axis: 0 });
    }

    /// WHAT MUST BE TRUE ALWAYS — on any scene, before and after the drag.
    fn invariants(app: &App, si: usize, when: &str, name: &str, bad: &mut Vec<String>) {
        let s = &app.project.sketches[si];
        for p in &s.points {
            if !p.x.is_finite() || !p.y.is_finite() {
                bad.push(format!("{name} ({when}): a coordinate of a point is not a number: ({}, {})", p.x, p.y));
                break;
            }
        }
        // A DANGLING REFERENCE is quiet corruption: a constraint on a deleted point is invisible on
        // screen, but the solver reads rubbish through it, and the sketch "goes mad" a dozen edits
        // later.
        let ids: std::collections::HashSet<Id> = s.points.iter().map(|p| p.id).collect();
        for (i, c) in s.constraints.iter().enumerate() {
            for pid in c.points() {
                if !ids.contains(&pid) {
                    bad.push(format!("{name} ({when}): constraint no. {i} refers to a point {pid} that does not exist"));
                }
            }
        }
        let (dof, redun) = app.project.sketch_dof(si);
        if dof < 0 {
            bad.push(format!("{name} ({when}): negative degrees of freedom ({dof})"));
        }
        if redun < 0 {
            bad.push(format!("{name} ({when}): negative redundancy ({redun})"));
        }
    }

    /// A SKETCH SURVIVES BEING BUILT AND BEING DRAGGED.
    ///
    /// Every scene at once, reported as a list: one failed check has no right to hide the rest —
    /// otherwise the sweep turns into mending one finding per run.
    #[test]
    fn building_and_dragging_never_breaks_a_sketch() {
        let mut bad: Vec<String> = Vec::new();
        for (name, build) in scenes() {
            let (mut app, case) = build();
            invariants(&app, case.si, "after building", name, &mut bad);
            let ents = app.project.sketches[case.si].entities.len();

            let Some((pid, tx, ty)) = case.drag else { continue };
            let before = xy(&app, case.si, pid);
            let (dof, redun) = app.project.sketch_dof(case.si);

            // DRAGGING AS IT HAPPENS IN LIFE: several frames with the fast solver, then a full solve on
            // release. A single call would check something other than what a mouse does.
            for k in 1..=6 {
                let t = k as f64 / 6.0;
                let (x, y) = (before.0 + (tx - before.0) * t, before.1 + (ty - before.1) * t);
                app.project.solve_sketch_drag_fast(case.si, Some((pid, x, y)));
            }
            // THE RELEASE IS A FULL SOLVE WITH NO PIN TO THE CURSOR, exactly as the program does it. The
            // first edition measured the residual WITH the point pinned to the cursor and declared "the
            // solver did not converge" where it had converged perfectly: in a defined sketch the point
            // SIMPLY CANNOT reach the cursor, and the remainder is the distance to the mouse rather than
            // an error in the constraints.
            let dragged_to = xy(&app, case.si, pid);
            let resid = app.project.solve_sketch(case.si);
            app.project.regen_sketch(case.si);

            invariants(&app, case.si, "after dragging", name, &mut bad);
            if app.project.sketches[case.si].entities.len() != ents {
                bad.push(format!("{name}: the drag changed the number of entities ({ents} -> {})", app.project.sketches[case.si].entities.len()));
            }
            if !case.conflicting && resid > 1e-3 {
                bad.push(format!("{name}: the solver did not converge after the drag (a residual of {resid:.3})"));
            }

            // THE POINT IS LOOKED AT BEFORE THE RELEASE: a full solve without the cursor has every right
            // to refine the position, and the question here is whether the geometry followed the mouse.
            let after = dragged_to;
            let moved = ((after.0 - before.0).powi(2) + (after.1 - before.1).powi(2)).sqrt();
            if dof == 0 && redun == 0 {
                // THE THRESHOLD IS NOT ZERO: the solver is numerical, and demanding byte for byte would
                // mean guarding arithmetic rather than behaviour. Half a millimetre is noticeably less
                // than the thickness of a line on screen; with the former weight of the mouse there were
                // FIFTY-TWO millimetres here.
                if moved > 0.5 {
                    bad.push(format!("{name}: a fully defined sketch followed the mouse for {moved:.2} mm — on screen that reads as the dimensions not holding"));
                }
            } else {
                let was = ((tx - before.0).powi(2) + (ty - before.1).powi(2)).sqrt();
                let now = ((tx - after.0).powi(2) + (ty - after.1).powi(2)).sqrt();
                if now > was + 1e-6 {
                    bad.push(format!("{name}: the point moved AWAY from the cursor rather than towards it ({was:.1} -> {now:.1} mm at {dof} degrees of freedom)"));
                }
            }
        }
        assert!(bad.is_empty(), "the sweep over the sketch found {} breakages:\n{}", bad.len(), bad.join("\n"));
    }

    /// EXACTLY WHAT DESERVES IT IS MARKED RED.
    ///
    /// Both directions. A false alarm teaches people not to trust the program, a missed one leaves a
    /// sketch that "somehow will not solve". Fillets are deliberately not here: their false rank
    /// redundancy on tangencies is known and dealt with separately (`fillet_tangency`).
    #[test]
    fn only_the_scenes_that_deserve_it_are_flagged() {
        let mut bad: Vec<String> = Vec::new();
        for (name, build) in scenes() {
            let (app, case) = build();
            // THE SAME THING THE SCREEN DRAWS IS ASKED. The raw rank from the kernel marks the tangencies
            // of a fillet and a slot falsely — that is sorted out and filtered in `flagged_redundant`.
            // What must be checked is what a person SEES, otherwise the guard would complain about a false
            // alarm closed long ago.
            let flagged = app.flagged_redundant(case.si);
            let conflicts = app.project.sketch_conflicts(case.si);
            let expect_redundant = name.contains("redundant");
            let expect_conflict = name.contains("conflicting");

            if !flagged.is_empty() && !expect_redundant {
                let what: Vec<String> = flagged.iter().map(|i| format!("{:?}", app.project.sketches[case.si].constraints[*i])).collect();
                bad.push(format!("{name}: something that is not redundant is marked orange: {}", what.join(", ")));
            }
            if expect_redundant && flagged.is_empty() {
                bad.push(format!("{name}: a redundant dimension is NOT marked — one will be left guessing why the sketch does not move"));
            }
            if !conflicts.is_empty() && !expect_conflict {
                bad.push(format!("{name}: a conflict is marked red on a scene where there is no contradiction ({} of them)", conflicts.len()));
            }
            if expect_conflict && conflicts.is_empty() {
                bad.push(format!("{name}: two incompatible dimensions are NOT marked as a conflict — the sketch silently will not solve"));
            }
        }
        assert!(bad.is_empty(), "the marking lies ({}):\n{}", bad.len(), bad.join("\n"));
    }

    /// A CONSTRAINT BADGE STANDS BY THE GEOMETRY IT HOLDS, NOT IN EMPTY SPACE.
    ///
    /// A regular hexagon is held by five equality constraints, and the first side is the same for all of
    /// them. That gave five badges in one place, and the spreading carried them to the right by 17 pixels
    /// each — so on a screenshot they lined up in a row in empty space, attached to nothing. No number
    /// shows that at all: the constraints are intact, the sketch solves, "everything works".
    #[test]
    fn a_constraint_badge_stays_next_to_the_geometry_it_holds() {
        let mut bad: Vec<String> = Vec::new();
        for (name, build) in scenes() {
            let (mut app, case) = build();
            let rect = flat_view(&mut app, case.si);
            let s = &app.project.sketches[case.si];
            for (ci, at, _) in app.constraint_glyphs(rect, case.si) {
                // THE MEASUREMENT GOES TO THE SEGMENT, NOT TO ITS ENDS. The horizontality badge stands
                // ABOVE THE MIDDLE of a side, and from there it is half a length to either end — the first
                // edition of this guard declared every properly placed badge a violation.
                let on: Vec<egui::Pos2> = app
                    .project
                    .sketch_constraint_points(case.si, ci)
                    .iter()
                    .filter_map(|id| s.points.iter().find(|p| p.id == *id))
                    .map(|p| app.to_screen(rect, qymcad_core::geom::Point2::new(p.x, p.y)))
                    .collect();
                if on.is_empty() {
                    continue;
                }
                let mut near = f32::MAX;
                for i in 0..on.len() {
                    for j in i..on.len() {
                        near = near.min(dist_to_segment(at, on[i], on[j]));
                    }
                }
                // 60 pixels is the limit of the snaking spread of the badges plus the base offset. The
                // five copies in a row that started all this gave more than seventy.
                if near > 60.0 {
                    bad.push(format!("{name}: the badge of constraint no. {ci} stands {near:.0} px from the geometry it holds"));
                }
            }
        }
        // AND THERE ARE NO IDENTICAL BADGES IN ONE PLACE. Five equalities on one side are not information
        // but a heap: saying "this side equals the others" once is enough.
        for (name, build) in scenes() {
            let (mut app, case) = build();
            let rect = flat_view(&mut app, case.si);
            let g = app.constraint_glyphs(rect, case.si);
            for i in 0..g.len() {
                for j in i + 1..g.len() {
                    if g[i].2 == g[j].2 && g[i].1.distance(g[j].1) < 6.0 {
                        bad.push(format!("{name}: two identical constraint badges are drawn on top of each other"));
                    }
                }
            }
        }
        // AND THE COUNT ADDS UP: a regular hexagon has exactly SIX equality badges, one per side. Five
        // constraints give ten badges, of which five point at one and the same side; the spreading merely
        // laid that heap out more tidily, and a heap it remained. The check goes by count rather than by
        // distance: badges laid out in a snake formally stand apart.
        let (mut app, case) = scenes().into_iter().find(|(n, _)| n == &"06-polygon").expect("the polygon scene").1();
        let rect = flat_view(&mut app, case.si);
        let equals = app.constraint_glyphs(rect, case.si).iter().filter(|(_, _, g)| *g == super::super::Gly::Equal).count();
        assert_eq!(equals, 6, "there must be one equality badge per side, and there are {equals}");

        assert!(bad.is_empty(), "the badges are drawn badly ({}):\n{}", bad.len(), bad.join("\n"));
    }

    /// A DIMENSION LABEL NEVER LANDS ON A VERTEX OF THE GEOMETRY.
    ///
    /// On a polygon the radius leader ran exactly into the first vertex, and "R20.0" read as garbage with
    /// the vertex sitting in the middle of it. That is no accident of the scene: a leader angle of zero
    /// means "to the right", and that is where the first vertex gets dragged when a figure is drawn from
    /// left to right.
    #[test]
    fn a_dimension_label_never_lands_on_a_vertex() {
        let mut bad: Vec<String> = Vec::new();
        for (name, build) in scenes() {
            let (mut app, case) = build();
            let rect = flat_view(&mut app, case.si);
            let n = app.project.sketches[case.si].constraints.len();
            for ci in 0..n {
                let Some(at) = app.dim_label_pos(rect, case.si, ci) else { continue };
                // A LABEL IS A RECTANGLE OF TEXT, not a point. The first edition measured to THE CENTRE
                // of the label and missed exactly the case it was written for: "R20.0" stands fourteen
                // pixels from the vertex and covers it all the same, because the text is drawn from its
                // centre and stretches a couple of dozen pixels to each side.
                let box_ = egui::Rect::from_center_size(at, egui::vec2(44.0, 16.0));
                let s = &app.project.sketches[case.si];
                for p in &s.points {
                    let q = app.to_screen(rect, qymcad_core::geom::Point2::new(p.x, p.y));
                    if box_.contains(q) {
                        bad.push(format!("{name}: the label of dimension no. {ci} covers a point of the sketch"));
                        break;
                    }
                }
            }
        }
        assert!(bad.is_empty(), "dimension labels have sat down on the geometry ({}):\n{}", bad.len(), bad.join("\n"));
    }

    /// The distance from a point to a segment (a degenerate segment is a point).
    fn dist_to_segment(p: egui::Pos2, a: egui::Pos2, b: egui::Pos2) -> f32 {
        let ab = b - a;
        let len2 = ab.length_sq();
        if len2 < 1e-6 {
            return p.distance(a);
        }
        let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
        p.distance(a + ab * t)
    }

    /// A flat view of the sketch at a known scale — shared by the pictures and by the checks of
    /// placement.
    fn flat_view(app: &mut App, _si: usize) -> egui::Rect {
        app.mode_3d = false;
        app.view.scale = 5.0;
        app.view.center = super::super::Vec2::new(0.0, 0.0);
        app.view.initialized = true;
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(720.0, 460.0))
    }

    /// THE PICTURES OF THE SWEEP — what cannot be checked by a number.
    ///
    /// `cargo test -p qymcad -- --ignored --nocapture sketch_sweep_images`, with the shots in
    /// `target/sketch-sweep/`. Two frames per scene: how it was built and how it survived the drag. What
    /// to look at: the constraint badges (is anything glowing orange that is in order), the dimensions
    /// (have the leaders drifted apart) and the geometry itself (has an arc turned inside out).
    #[test]
    #[ignore]
    fn sketch_sweep_images() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/sketch-sweep");
        std::fs::create_dir_all(&dir).expect("the directory of the sweep");
        for (name, build) in scenes() {
            let (mut app, case) = build();
            let si = case.si;
            save(&dir, &format!("{name}-a"), &shot(&mut app, si));
            if let Some((pid, tx, ty)) = case.drag {
                let b = xy(&app, si, pid);
                for k in 1..=6 {
                    let t = k as f64 / 6.0;
                    app.project.solve_sketch_drag_fast(si, Some((pid, b.0 + (tx - b.0) * t, b.1 + (ty - b.1) * t)));
                }
                save(&dir, &format!("{name}-b"), &shot(&mut app, si));
                app.project.solve_sketch(si);
                app.project.regen_sketch(si);
                save(&dir, &format!("{name}-c"), &shot(&mut app, si));
            }
        }
        eprintln!("the shots of the sketch sweep: {}", dir.display());
    }

    /// A frame of the sketch with its contours, constraints and dimensions — exactly what the canvas
    /// draws.
    fn shot(app: &mut App, si: usize) -> egui::ColorImage {
        app.mode_3d = false;
        app.sel = super::super::Sel::Sketch(si);
        app.sketch_ses.editing = Some(app.project.sketches[si].id);
        app.project.regen_sketch(si);
        app.view.scale = 5.0;
        app.view.center = super::super::Vec2::new(0.0, 0.0);
        app.view.initialized = true;
        let bg = app.scheme.pal.viewport_bg();
        let a = &*app;
        super::super::help_raster::shot_ui([720, 460], bg, |ui| {
            let ctx = &ui.ctx().clone();
            ctx.set_visuals(crate::palette::visuals(&a.scheme.pal));
            egui::CentralPanel::default().show(ui, |ui| {
                let painter = ui.painter().clone();
                let r = ui.available_rect_before_wrap();
                painter.rect_filled(r, 0.0, bg);
                a.draw_contours(&painter, r);
                a.draw_sketch_constraints(&painter, r, si);
                a.draw_sketch_dims(&painter, r, si);
            });
        })
    }

    fn save(dir: &std::path::Path, name: &str, img: &egui::ColorImage) {
        let png = App::color_image_to_png(img).expect("PNG");
        std::fs::write(dir.join(format!("{name}.png")), png).expect("the write");
    }
}
