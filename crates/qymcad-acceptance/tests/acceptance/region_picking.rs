//! A REGION OF A SKETCH IS PICKED BY A CLICK INSIDE IT AND SHOWN FILLED, in every tool that takes a closed profile.
//!
//! The owner's request: in the tools that take closed geometry of a sketch, the picked region is filled with a colour
//! of its own rather than only outlined, and a region is picked by a click inside it, not only on its line.
use qymcad::{Key, Picture, Session};
use qymcad_acceptance::{build, probe};

/// A rectangle 40 x 30 on XY with a circle of R 6 inside it, the sketch left: two regions, a ring and a disk.
fn a_ring_and_a_disk() -> Session {
    let mut s = Session::start();
    build::into_the_first_part(&mut s);
    build::rectangle_on_xy(&mut s);
    build::draw(&mut s, "tb-circle-hint", &[(20.0, 15.0), (26.0, 15.0)]);
    let finish = s.word("wb-finish");
    s.press_word(&finish);
    s
}

probe! {
    /// A CLICK INSIDE A REGION PICKS IT AND FILLS IT, a hole of it left open: in the picker of an extrusion over a
    /// rectangle with a circle inside, a click between them fills the ring and leaves the disk as the sheet; a click in
    /// the circle fills the disk as well.
    fn a_click_inside_a_region_picks_it_and_fills_it_its_holes_left_open() {
        let mut s = a_ring_and_a_disk();
        let extrude = s.word("tb-extrude-hint");
        s.press_hint(&extrude);
        // the sheet's own colour, at a corner of the canvas, clear of the sketch
        let corner = s.canvas().min + qymcad::vec2(12.0, 12.0);
        // where a click lands, and where the colour is read - away from the pointer's cross drawn at the click
        let ring = s.on_sheet(5.0, 5.0);
        let disk = s.on_sheet(20.0, 15.0);
        let in_ring = s.on_sheet(35.0, 25.0);
        let in_disk = s.on_sheet(20.0, 18.0);
        let before = s.snapshot();
        let sheet = before.colour_at(corner);
        assert!(Picture::apart(before.colour_at(in_ring), sheet) < 8, "setup: the ring is filled before any click");
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/look");
        let _ = std::fs::create_dir_all(&dir);

        s.click(ring);
        let after_ring = s.snapshot();
        let _ = std::fs::write(dir.join("region_picker_ring.png"), after_ring.png());
        assert!(Picture::apart(after_ring.colour_at(in_ring), sheet) >= 20, "the ring was clicked and is not filled: {:?} against the sheet {sheet:?}", after_ring.colour_at(in_ring));
        assert!(Picture::apart(after_ring.colour_at(in_disk), sheet) < 8, "the ring was picked and its hole is filled too: {:?} against the sheet {sheet:?}", after_ring.colour_at(in_disk));

        // the region under the pointer is lit before the click - which region a click would take
        s.move_to(disk);
        let hovered = s.snapshot();
        let _ = std::fs::write(dir.join("region_picker_hover.png"), hovered.png());
        assert!(Picture::apart(hovered.colour_at(in_disk), sheet) >= 8, "the disk under the pointer is not lit before the click: {:?} against the sheet {sheet:?}", hovered.colour_at(in_disk));

        s.click(disk);
        let both = s.snapshot();
        let _ = std::fs::write(dir.join("region_picker_both.png"), both.png());
        assert!(Picture::apart(both.colour_at(in_disk), sheet) >= 20, "the disk was clicked and is not filled: {:?}", both.colour_at(in_disk));
        // on to the size, in 3D: the picked regions lie filled on the sketch's plane under the preview (for the eyes)
        s.key(Key::Enter);
        let _ = std::fs::write(dir.join("region_picker_3d.png"), s.snapshot().png());
        s.key(Key::Escape);
    }
}

/// The area between the spline the sketcher draws through three nodes and the chord of its ends: a Catmull-Rom curve,
/// the tangent at a node half the step between its neighbours, one-sided at the ends - integrated in fine steps.
fn cap_of_a_spline_through(n: [(f64, f64); 3]) -> f64 {
    let half = |a: (f64, f64), b: (f64, f64)| ((b.0 - a.0) / 2.0, (b.1 - a.1) / 2.0);
    let m = [half(n[0], n[1]), half(n[0], n[2]), half(n[1], n[2])];
    let at = |s: usize, t: f64| {
        let (t2, t3) = (t * t, t * t * t);
        let h = [2.0 * t3 - 3.0 * t2 + 1.0, t3 - 2.0 * t2 + t, -2.0 * t3 + 3.0 * t2, t3 - t2];
        (h[0] * n[s].0 + h[1] * m[s].0 + h[2] * n[s + 1].0 + h[3] * m[s + 1].0, h[0] * n[s].1 + h[1] * m[s].1 + h[2] * n[s + 1].1 + h[3] * m[s + 1].1)
    };
    let steps = 4000;
    let mut line: Vec<(f64, f64)> = (0..2).flat_map(|s| (0..steps).map(move |k| (s, k as f64 / steps as f64))).map(|(s, t)| at(s, t)).collect();
    line.push(n[2]);
    let k = line.len();
    ((0..k).map(|i| line[i].0 * line[(i + 1) % k].1 - line[(i + 1) % k].0 * line[i].1).sum::<f64>() / 2.0).abs()
}

probe! {
    /// A REGION CLOSED BY A SPLINE AND TWO LINES IS EXTRUDED BY A CLICK INSIDE IT: the spline drawn through three nodes,
    /// two lines from its ends to a point below, a circle beside them, the extrusion taken and the region clicked - the body holds the area the
    /// curve and the lines bound, times the length.
    fn a_region_closed_by_a_spline_is_picked_by_a_click_and_extruded() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        let xy = s.word("plane-xy-table");
        s.press_word(&xy);
        let nodes = [(0.0, 0.0), (20.0, 24.0), (40.0, 0.0)];
        let spline = s.word("tb-spline-hint");
        s.press_hint(&spline);
        s.click_on_sketch(nodes[0].0, nodes[0].1).click_on_sketch(nodes[1].0, nodes[1].1);
        let end = s.on_sketch(nodes[2].0, nodes[2].1);
        s.double_click(end);
        s.key(Key::Escape);
        build::line(&mut s, (40.0, 0.0), (20.0, -20.0));
        build::line(&mut s, (20.0, -20.0), (0.0, 0.0));
        // a circle beside it, so the sketch has more than one region and the picker waits for the click
        build::circle(&mut s, (60.0, 0.0), (65.0, 0.0));
        let finish = s.word("wb-finish");
        s.press_word(&finish);
        let extrude = s.word("tb-extrude-hint");
        s.press_hint(&extrude);
        let inside = s.on_sheet(20.0, 5.0);
        s.click(inside);
        s.key(Key::Enter);
        let length = s.word("f-length");
        s.fill(&length, "10").key(Key::Enter);
        // the cap of the curve over its chord, and the triangle under the chord: 40 x 20 / 2
        let want = (cap_of_a_spline_through(nodes) + 400.0) * 10.0;
        let bodies: Vec<f64> = s.document().bodies.into_iter().filter(|b| !b.consumed && !b.sheet).map(|b| b.volume).collect();
        assert!(bodies.len() == 1 && (bodies[0] - want).abs() < 0.01 * want, "the region of the spline and the two lines, 10 long, holds {want}; the bodies hold {bodies:?}; the program says {:?}", s.status());
    }
}

probe! {
    /// THE FIELDS OF AN EXTRUSION STAND BESIDE ITS ARROW, clear of it: a rectangle 40 x 30 on XY taken by the extrusion,
    /// its arrow from the middle of the profile, (20, 15, 0), to (20, 15, 10) - every point of the arrow on the screen is
    /// 20 px or more from the words of the box of its length. Reported behaviour: the box stood against the tip of the
    /// arrow, and the arrow could hardly be seen or taken.
    fn the_fields_of_an_extrusion_stand_beside_its_arrow() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        build::rectangle_on_xy(&mut s);
        let finish = s.word("wb-finish");
        s.press_word(&finish);
        let extrude = s.word("tb-extrude-hint");
        s.press_hint(&extrude);
        let (base, tip) = (s.projected([20.0, 15.0, 0.0]), s.projected([20.0, 15.0, 10.0]));
        let length = s.word("f-length");
        // the box of the length: its words within 300 px of the tip, the bars of the window far above it left out
        let canvas = s.canvas();
        let boxes: Vec<qymcad::Rect> = s.words_at().into_iter().filter(|(_, r)| r.min.y > canvas.min.y && r.center().distance(tip) < 300.0).map(|(_, r)| r.expand(2.0)).collect();
        assert!(s.words_at().iter().any(|(w, r)| *w == length && r.center().distance(tip) < 300.0), "setup: no box of the length beside the arrow; on screen {:?}", s.words());
        // how near the arrow comes to the box: the nearest of its points to the nearest word of the box
        let gap = (0..=20).map(|k| base + (tip - base) * (k as f32 / 20.0)).flat_map(|p| boxes.iter().map(move |r| r.distance_to_pos(p))).fold(f32::INFINITY, f32::min);
        let near: Vec<(String, qymcad::Rect)> = s.words_at().into_iter().filter(|(_, r)| r.center().distance(tip) < 120.0).collect();
        assert!(gap >= 20.0, "the arrow from {base:?} to {tip:?} comes {gap:.1} px from the box of its length - it stands on the arrow; near: {near:?}");
    }
}
