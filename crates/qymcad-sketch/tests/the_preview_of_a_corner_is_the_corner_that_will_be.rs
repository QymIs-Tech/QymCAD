//! THE PREVIEW OF A CORNER IS THE CORNER THAT WILL BE MADE.
//!
//! Reported: with the fillet of a corner, while the radius was being typed, the arc was drawn "always in the direction
//! away from the horizontal line", and the fillet that came on Enter landed in the corner all the same. The two ends of
//! the preview were the right ones - they are the points the lines are cut at, and they are cut there - but the arc
//! between them was walked the other way round.
//!
//! WHY. The angles of the arc were taken from the drawing's own coordinates and the points were then plotted on the
//! screen. The sheet turns the drawing over (`Sheet::at` takes the sign of y off), so an angle read before the turn is
//! walked backwards after it: the arc came out mirrored about the horizontal, and mirroring it is not the same arc.
//! The chamfer showed nothing of this, because a straight cut between two points does not care which way round it is
//! walked.
//!
//! So what is checked here is the ARC, and by the property that tells the two apart without arithmetic of its own: a
//! fillet takes the corner off, so its arc bulges towards the corner - nearer the vertex than the straight chord
//! between its ends. Mirrored, the same arc bulges the other way and is further from the vertex than the chord.
use egui::{Pos2, Rect};
use qymcad_core::feature::Purpose;
use qymcad_core::geom::Point2;
use qymcad_core::model::Project;
use qymcad_ui_state::{Sheet, View2d};

/// THE SHEET THE PREVIEW IS DRAWN ON: the drawing at one pixel to the unit, so that a unit of the drawing and a pixel
/// are the same length and the numbers can be read either way round.
fn sheet() -> Sheet {
    Sheet { view: View2d { center: egui::Vec2::new(20.0, 20.0), scale: 1.0, ..Default::default() }, rect: Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0)) }
}

/// TWO LINES OUT OF (20, 20) ALONG THE TWO GIVEN AXES, and the corner they name.
///
/// The four turns of one right angle, so that a mistake which only shows itself in one of them - an arc walked from
/// the wrong end, a sign that only bites when y grows upwards - is caught rather than passed over.
fn an_angle(d1: (i32, i32), d2: (i32, i32)) -> (Project, usize, (u64, u64)) {
    let mut p = Project::default();
    p.new_document();
    let si = p.new_sketch("S");
    let to = |d: (i32, i32)| (20.0 + d.0 as f64 * 20.0, 20.0 + d.1 as f64 * 20.0);
    let (a, b) = (to(d1), to(d2));
    let e1 = p.add_line_entity(si, 20.0, 20.0, a.0, a.1, Purpose::Real);
    let e2 = p.add_line_entity(si, 20.0, 20.0, b.0, b.1, Purpose::Real);
    p.regen_sketch(si);
    (p, si, (e1, e2))
}

/// THE ARC OF THE PREVIEW AS THE SHEET SHOWS IT, with the three points it is about: the corner, and the two ends.
fn drawn(p: &Project, si: usize, e: (u64, u64), r: f64) -> (Vec<Pos2>, Pos2, Pos2, Pos2) {
    let pid = p.shared_vertex(si, e.0, e.1).expect("the two lines share the corner");
    let b = p.corner_blend(si, pid, e, qymcad_core::model::CornerCut::Fillet(qymcad_core::model::FilletSize::radius(r))).expect("a fillet of this corner can be shown");
    let (Some((c, _)), [e0, e1]) = (b.arc, b.ends) else { panic!("a fillet is shown as an arc, not as a straight cut") };
    let sh = sheet();
    let at = |q: [f64; 2]| sh.at(Point2::new(q[0], q[1]));
    (qymcad_sketch::corner_arc_points(at(c), (r * sh.view.scale as f64) as f32, at(e0), at(e1)), at(b.vertex), at(e0), at(e1))
}

/// THE ARC OF THE PREVIEW BENDS INTO THE CORNER, whichever way the corner is turned.
#[test]
fn the_arc_of_the_preview_bulges_towards_the_corner() {
    for (d1, d2) in [((1, 0), (0, 1)), ((0, 1), (-1, 0)), ((-1, 0), (0, -1)), ((0, -1), (1, 0))] {
        let (p, si, e) = an_angle(d1, d2);
        let (pts, corner, e0, e1) = drawn(&p, si, e, 4.0);
        let near = |a: Pos2, b: Pos2| format!("{} and {} stand {:.3} apart", a, b, a.distance(b));
        assert!(pts[0].distance(e0) < 1e-3, "the arc does not begin at the first end: {}", near(pts[0], e0));
        assert!(pts[pts.len() - 1].distance(e1) < 1e-3, "the arc does not end at the second end: {}", near(pts[pts.len() - 1], e1));
        let mid = pts[pts.len() / 2];
        let chord = Pos2::new((e0.x + e1.x) / 2.0, (e0.y + e1.y) / 2.0);
        assert!(
            mid.distance(corner) < chord.distance(corner),
            "the arc of the preview bulges AWAY from the corner, so the fillet will not be where it is shown:\n  the middle of the arc is at {mid}, the corner at {corner}, the straight cut between the ends at {chord}\n  (lines along {:?} and {:?})",
            d1,
            d2
        );
    }
}

/// AND THE ARC IT SHOWS IS THE ARC THE DOCUMENT WILL BUILD: the fillet of the corner stands on the very two points the
/// preview marks as the ends.
#[test]
fn the_ends_of_the_preview_are_the_ends_of_the_fillet() {
    let (mut p, si, e) = an_angle((1, 0), (0, 1));
    let pid = p.shared_vertex(si, e.0, e.1).expect("the two lines share the corner");
    let b = p.corner_blend(si, pid, e, qymcad_core::model::CornerCut::Fillet(qymcad_core::model::FilletSize::radius(4.0))).expect("a fillet of this corner can be shown");
    assert!(p.fillet_at_pair(si, e, 4.0), "the fillet of the corner did not apply");
    let arc = p.sketches[si].entities.iter().find_map(|x| if let Some(qymcad_core::model::ArcParts { a, b, .. }) = x.kind.as_arc() { Some((a, b)) } else { None }).expect("the fillet is an arc");
    let xy = |id| p.sketches[si].points.iter().find(|q| q.id == id).map(|q| (q.x, q.y)).expect("the arc stands on points of the sketch");
    let ends = [xy(arc.0), xy(arc.1)];
    for e in b.ends {
        let near = |q: (f64, f64)| ends.iter().any(|a: &(f64, f64)| (a.0 - q.0).hypot(a.1 - q.1) < 1e-6);
        assert!(near((e[0], e[1])), "the preview ends the fillet at {e:?}, and its arc stands on {ends:?}: two different corners, or two different cuts of one");
    }
}
