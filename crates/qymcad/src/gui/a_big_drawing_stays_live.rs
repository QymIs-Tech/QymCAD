//! A BIG DRAWING STAYS LIVE, through the window: a DXF of thousands of segments, written by the check itself, comes in
//! through "Import", is laid on a plane, and the sketch it makes is worked by hand - the pointer over it, a corner
//! dragged - each frame timed.
//!
//! Reported behaviour (#95): a sketch of a few hundred lines hangs the program.
//!
//! The budgets of time hold at the level of the probes of time alone (`qymcad_core::time_budget`); what the hand did -
//! the line, its Horizontal, the dimension, the points drawn apart - is held at every level.
#[cfg(test)]
pub(crate) mod tests {
    use super::super::hand::Hand;
    use crate::gui::import_door::tests::{answer, running};
    use qymcad_ui_state::Want;
    use std::time::{Duration, Instant};

    /// A DXF of `n` separate rectangles of four LINE entities each, 10 by 6 mm, on a grid 20 mm apart.
    pub(crate) fn rectangles_dxf(n: usize) -> std::path::PathBuf {
        let dir = std::path::PathBuf::from(format!("{}/../../target/a-big-drawing", env!("CARGO_MANIFEST_DIR")));
        std::fs::create_dir_all(&dir).expect("a folder for the check");
        let side = (n as f64).sqrt().ceil() as usize;
        let mut dxf = String::from("0\nSECTION\n2\nENTITIES\n");
        for k in 0..n {
            let (x, y) = ((k % side) as f64 * 20.0, (k / side) as f64 * 20.0);
            let c = [(x, y), (x + 10.0, y), (x + 10.0, y + 6.0), (x, y + 6.0)];
            for i in 0..4 {
                let (a, b) = (c[i], c[(i + 1) % 4]);
                dxf.push_str(&format!("0\nLINE\n8\n0\n10\n{}\n20\n{}\n30\n0.0\n11\n{}\n21\n{}\n31\n0.0\n", a.0, a.1, b.0, b.1));
            }
        }
        dxf.push_str("0\nENDSEC\n0\nEOF\n");
        let p = dir.join(format!("rectangles-{n}.dxf"));
        std::fs::write(&p, dxf).expect("written");
        p
    }

    /// The points the last frame drew, where they stand on the screen: plain (3.5 px), picked (4.5 px), selected or lit
    /// (5 px).
    pub(crate) fn points_drawn(hand: &Hand) -> Vec<egui::Pos2> {
        [3.5, 4.5, 5.0].into_iter().flat_map(|r| hand.dots_drawn(r)).collect()
    }

    /// How many points of the sketch stand in sight, the reference points of the frame left out.
    pub(crate) fn points_in_sight(hand: &Hand, si: usize) -> usize {
        let app = &hand.app;
        let sheet = qymcad_ui_state::Sheet { view: app.viewing.view, rect: app.viewing.view_rect };
        let s = &app.project.sketches[si];
        let unseen = s.unseen_points();
        let sight = app.viewing.view_rect.expand(8.0);
        s.points.iter().filter(|q| !unseen.contains(&q.id) && sight.contains(sheet.at(qymcad_core::geom::Point2::new(q.x, q.y)))).count()
    }

    /// No two of `drawn` nearer than a point is wide, along either axis.
    pub(crate) fn apart(drawn: &[egui::Pos2]) -> bool {
        let room = qymcad_render::POINT_ROOM - 0.1;
        let mut by_cell: std::collections::HashMap<(i32, i32), Vec<egui::Pos2>> = std::collections::HashMap::new();
        for p in drawn {
            by_cell.entry(((p.x / room) as i32, (p.y / room) as i32)).or_default().push(*p);
        }
        drawn.iter().all(|p| {
            let (cx, cy) = ((p.x / room) as i32, (p.y / room) as i32);
            (cx - 1..=cx + 1).all(|x| (cy - 1..=cy + 1).all(|y| by_cell.get(&(x, y)).is_none_or(|q| q.iter().all(|q| q == p || (q.x - p.x).abs() >= room || (q.y - p.y).abs() >= room))))
        })
    }

    /// No two numbers of points written over one another in the last frame.
    pub(crate) fn numbers_apart(hand: &Hand) -> bool {
        let numbers: Vec<egui::Rect> = hand.words_drawn().iter().filter(|(t, _)| !t.is_empty() && t.chars().all(|c| c.is_ascii_digit())).map(|(_, r)| r.shrink(0.5)).collect();
        numbers.iter().enumerate().all(|(i, a)| numbers.iter().skip(i + 1).all(|b| !a.intersects(*b)))
    }

    /// The time of `work`.
    fn timed(work: impl FnOnce()) -> Duration {
        let started = Instant::now();
        work();
        started.elapsed()
    }

    #[test]
    fn a_big_drawing_is_worked_by_hand() {
        // A RELEASE BUILD TAKES THE DRAWING OF THE BAR, 70 000 segments, and holds a frame to its time; a test build, ten
        // times slower, takes 10 000 and holds what is drawn
        let release = !cfg!(debug_assertions);
        let n = if release { 17_500 } else { 2_500 };
        let path = rectangles_dxf(n);
        let (mut app, ctx) = running();
        let t_import = timed(|| answer(&mut app, &ctx, Want::Anything, &path.to_string_lossy()));
        let mut hand = Hand::new(&mut app);
        let t_place = timed(|| {
            hand.click([5.0, 5.0, 0.0]);
        });
        let si = hand.app.project.sketches.iter().position(|s| s.entities.len() >= 4 * n).expect("the drawing came in as a sketch");
        // the numbers of the points are behind their box in Settings -> Sketch: ticked here, to see they keep apart
        assert!(crate::gui::the_point_numbers_wait_for_their_setting::tests::point_numbers_ticked(&mut hand), "the box of the numbers of the points was not reached");
        let t_frame = timed(|| {
            hand.frame(Vec::new());
        });
        // the whole drawing in sight: the points stand closer than they are wide, and only those with room are drawn -
        // no two nearer than a point is wide, fewer than are in sight - and no number over another
        let (drawn, seen) = (points_drawn(&hand), points_in_sight(&hand, si));
        assert!(drawn.len() < seen, "all {seen} points in sight are drawn, the most of them one over another");
        assert!(apart(&drawn), "two points are drawn nearer than a point is wide");
        assert!(numbers_apart(&hand), "two numbers of points are written one over another");
        let t_hover = timed(|| {
            hand.hover2d(10.0, 6.0);
        });
        let t_drag = timed(|| {
            hand.drag2d((10.0, 6.0), (13.0, 9.0));
        });
        // two frames with the pointer over the drawing. Measured on 70 000 segments: 0.4 s before the pick and the snap
        // looked up their points from a table, 0.27 s before the keys of the status were mixed quickly, 0.21 s before
        // the panel and the glyphs stopped gathering the points of every entity, 0.12 s after
        if release {
            assert!(qymcad_core::time_budget::within(t_hover, Duration::from_millis(200)), "two frames with the pointer over 70 000 segments took {t_hover:?}, budget 200 ms in a release build");
        }
        // brought near a corner, a few hundred points in sight: drawn and numbered again
        hand.look2d((30.0, 30.0)).frame(Vec::new());
        let (near, seen) = (points_drawn(&hand).len(), points_in_sight(&hand, si));
        assert_eq!(near, seen, "near a corner, the points apart, {near} of the {seen} in sight are drawn");
        assert!(hand.shows("1"), "near a corner the points are not numbered");
        assert!(numbers_apart(&hand), "near a corner two numbers are written one over another");
        // the pointer inside a rectangle, no corner within reach: the snap looks for what runs near it, not for every
        // crossing of every pair of segments (5e7 pairs here, 2.45e9 on 70 000 segments - 15 s a frame)
        let t_away = timed(|| {
            hand.hover2d(35.0, 33.0);
        });
        assert!(qymcad_core::time_budget::within(t_away, Duration::from_secs(1)), "the pointer away from the corners took {t_away:?} for its two frames, budget 1 s in a test build");
        eprintln!("import {t_import:?}, place {t_place:?}, frame {t_frame:?}, hover {t_hover:?}, drag {t_drag:?}, away {t_away:?}; {} entities", hand.app.project.sketches[si].entities.len());
    }

    #[test]
    fn a_sketch_of_three_hundred_shapes_takes_a_line_and_a_dimension() {
        // Reported behaviour (#95): in a sketch of 300 lines a constraint or a dimension took seconds to minutes.
        let path = rectangles_dxf(300);
        let (mut app, ctx) = running();
        answer(&mut app, &ctx, Want::Anything, &path.to_string_lossy());
        let mut hand = Hand::new(&mut app);
        hand.click([5.0, 5.0, 0.0]);
        let si = hand.app.project.sketches.iter().position(|s| s.entities.len() >= 1_200).expect("the drawing came in as a sketch");
        let before = hand.app.project.sketches[si].constraints.len();
        // a line drawn nearly level below the drawing takes a Horizontal of its own
        let t_line = timed(|| {
            hand.sk_tool(1).click2d(0.0, -20.0).click2d(60.0, -19.6);
            hand.key(egui::Key::Escape).key(egui::Key::Escape);
        });
        let s = &hand.app.project.sketches[si];
        assert_eq!(s.entities.len(), 1_201, "the line did not come in");
        assert!(s.constraints[before..].iter().any(|c| matches!(c, qymcad_core::model::Constraint::Horizontal { .. })), "the line drawn nearly level took no Horizontal");
        let laid = s.constraints.len();
        let t_dim = timed(|| {
            hand.sk_tool(0);
            assert!(hand.press_hint(&crate::i18n::tr("tb-dim-hint")), "the dimension tool");
            hand.click2d(30.0, -19.8);
            hand.click2d(30.0, -28.0);
            hand.key(egui::Key::Enter).key(egui::Key::Escape);
        });
        let dims = hand.app.project.sketches[si].constraints[laid..].iter().filter(|c| matches!(c, qymcad_core::model::Constraint::Distance { .. })).count();
        assert_eq!(dims, 1, "the dimension was not laid on the line");
        eprintln!("1 200 segments: a line drawn {t_line:?}, a dimension laid {t_dim:?}");
        // whole gestures of the hand, each several frames, in a test build; the solve and the diagnostics of the whole
        // sketch for every step took 3 s a constraint on 1 000 lines in a release build before the parts
        assert!(qymcad_core::time_budget::within(t_line, Duration::from_secs(3)), "a line drawn among 1 200 segments took {t_line:?}");
        assert!(qymcad_core::time_budget::within(t_dim, Duration::from_secs(3)), "a dimension laid among 1 200 segments took {t_dim:?}");
    }
}
