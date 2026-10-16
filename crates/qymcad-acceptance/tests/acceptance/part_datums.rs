//! THE DATUMS: a plane, an axis and a point, each made every way the program offers, and each read back from the
//! document - where it stands and which way it looks.
use qymcad::{Key, Session};
use qymcad_acceptance::build;
use qymcad_acceptance::probe;

/// The datums of the document, newest last.
fn datums(s: &mut Session) -> Vec<qymcad::Datum> {
    s.document().datums
}

/// The last datum of `kind`.
fn last(s: &mut Session, kind: &str) -> qymcad::Datum {
    datums(s).into_iter().rfind(|d| d.kind == kind).unwrap_or_else(|| panic!("the document holds no {kind}: {:?}", datums(s)))
}

/// Take the tool whose hint is `hint`.
fn take(s: &mut Session, hint: &str) {
    let hint = s.word(hint);
    s.press_hint(&hint);
}

/// Press the word `key` names in the bar of options at the top.
fn bar_word(s: &mut Session, key: &str) {
    let word = s.word(key);
    s.press_word_near(&word, qymcad::pos2(640.0, 24.0));
}

/// Type `text` into the field under the caption `caption`.
fn field(s: &mut Session, caption: &str, text: &str) {
    s.fill(caption, text);
}

/// Whether the place `at` is `want`, to a thousandth.
fn stands_at(at: [f64; 3], want: [f64; 3]) -> bool {
    at.iter().zip(want).all(|(a, b)| (a - b).abs() < 1e-3)
}

/// Whether the direction `dir` points along `want` (either way).
fn points_along(dir: [f64; 3], want: [f64; 3]) -> bool {
    let dot: f64 = dir.iter().zip(want).map(|(a, b)| a * b).sum();
    let n: f64 = dir.iter().map(|a| a * a).sum::<f64>().sqrt() * want.iter().map(|a| a * a).sum::<f64>().sqrt();
    n > 0.0 && (dot.abs() / n - 1.0).abs() < 1e-3
}

/// WHERE THE ROUND SIDE OF A SHAFT CAN BE CLICKED: only the facets turned towards the eye can be, so the places round
/// it are tried in turn.
fn the_side_of_a_shaft(s: &mut Session, r: f64, z: f64) -> qymcad::Pos2 {
    let middle = s.document().bodies.iter().rfind(|b| !b.consumed && !b.sheet).map(|b| [(b.min[0] + b.max[0]) / 2.0, (b.min[1] + b.max[1]) / 2.0]).unwrap_or([0.0, 0.0]);
    for k in 0..(72 * 3) {
        let a = (k % 72) as f64 * std::f64::consts::TAU / 72.0;
        let r = r - 0.05 * (k / 72) as f64;
        let p = [middle[0] + r * a.cos(), middle[1] + r * a.sin(), z];
        let mut seen = None;
        let message = qymcad_acceptance::refusal(|| seen = Some(s.face_at(p)));
        if message.is_empty() {
            if let Some(at) = seen {
                return at;
            }
        }
    }
    panic!("no place on the round side of the shaft can be seen from here");
}

/// A datum plane `dist` from what stands under the pointer at `p`, made the way a person makes one.
fn plane_from(s: &mut Session, p: [f64; 3], dist: &str) {
    take(s, "g-datum-plane-hint");
    let at = s.in_space(p);
    s.click(at);
    let offset = s.word("f-offset");
    field(s, &offset, dist);
    s.key(Key::Enter);
}

probe! {
    /// A PLANE FROM A PLANE OF THE WORLD stands the offset away from it, facing the same way.
    fn a_plane_is_made_from_a_plane_of_the_world() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        plane_from(&mut s, [0.0, 0.0, 0.0], "20");
        let d = last(&mut s, "plane");
        assert!(stands_at(d.at, [0.0, 0.0, 20.0]), "the plane 20 above the table stands at {:?}", d.at);
        assert!(points_along(d.dir, [0.0, 0.0, 1.0]), "the plane above the table does not lie flat: it looks {:?}", d.dir);
    }
}

probe! {
    /// A PLANE FROM A FACE OF A BODY stands the offset away from that face.
    fn a_plane_is_made_from_a_face_of_a_body() {
        let mut s = Session::start();
        build::block(&mut s);
        take(&mut s, "g-datum-plane-hint");
        let top = s.face_at([20.0, 15.0, 10.0]);
        s.click(top);
        let offset = s.word("f-offset");
        field(&mut s, &offset, "5");
        s.key(Key::Enter);
        let d = last(&mut s, "plane");
        assert!((d.at[2] - 15.0).abs() < 1e-3, "the plane 5 above the top of a block 10 thick stands at {:?}", d.at);
        assert!(points_along(d.dir, [0.0, 0.0, 1.0]), "the plane over the top face does not lie flat: it looks {:?}", d.dir);
    }
}

probe! {
    /// A PLANE FROM ANOTHER PLANE adds its offset to the one it was made from.
    fn a_plane_is_made_from_another_plane() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        plane_from(&mut s, [0.0, 0.0, 0.0], "20");
        plane_from(&mut s, [0.0, 0.0, 20.0], "5");
        let all: Vec<qymcad::Datum> = datums(&mut s).into_iter().filter(|d| d.kind == "plane").collect();
        assert!(all.len() == 2, "two planes were made, and the document holds {}", all.len());
        assert!((all[1].at[2] - 25.0).abs() < 1e-3, "the plane 5 above the one at 20 stands at {:?}", all[1].at);
    }
}

probe! {
    /// A POINT BY ITS COORDINATES stands where the three numbers say.
    fn a_point_is_made_by_its_coordinates() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        take(&mut s, "g-datum-point-hint");
        bar_word(&mut s, "cmd-coordinates");
        for (caption, value) in [("X", "5"), ("Y", "6"), ("Z", "7")] {
            field(&mut s, caption, value);
        }
        s.key(Key::Enter);
        let d = last(&mut s, "point");
        assert!(stands_at(d.at, [5.0, 6.0, 7.0]), "the point typed as (5, 6, 7) stands at {:?}", d.at);
    }
}

probe! {
    /// A POINT AT A VERTEX stands on the corner of the body that was clicked.
    fn a_point_is_made_at_a_vertex_of_a_body() {
        let mut s = Session::start();
        build::block(&mut s);
        take(&mut s, "g-datum-point-hint");
        bar_word(&mut s, "cmd-to-vertex");
        let corner = s.vertex_at([40.0, 30.0, 10.0]);
        s.click(corner);
        s.key(Key::Enter);
        let d = last(&mut s, "point");
        assert!(stands_at(d.at, [40.0, 30.0, 10.0]), "the point put on the corner of the block stands at {:?}", d.at);
    }
}

probe! {
    /// AN AXIS ALONG AN EDGE runs the way that edge runs.
    fn an_axis_is_made_along_an_edge() {
        let mut s = Session::start();
        build::block(&mut s);
        take(&mut s, "g-datum-axis-hint");
        let edge = s.edge_at([20.0, 0.0, 10.0]);
        s.click(edge);
        s.key(Key::Enter);
        let d = last(&mut s, "axis");
        assert!(points_along(d.dir, [1.0, 0.0, 0.0]), "the axis along the front top edge of the block runs {:?}", d.dir);
        assert!((d.at[2] - 10.0).abs() < 1e-3 && (d.at[1]).abs() < 1e-3, "the axis does not lie on that edge: it starts at {:?}", d.at);
    }
}

probe! {
    /// AN AXIS ALONG A ROUND FACE runs up the middle of it - the middle of that face, not of the world.
    fn an_axis_is_made_along_a_round_face() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        // the cylinder is put on a point of its own, away from the origin, so that the world's own axis cannot pass
        // for the axis of the face
        take(&mut s, "g-datum-point-hint");
        bar_word(&mut s, "cmd-coordinates");
        field(&mut s, "X", "25");
        s.key(Key::Enter);
        take(&mut s, "tb-cylinder-hint");
        let place = s.in_space([25.0, 0.0, 0.0]);
        s.click(place);
        s.key(Key::Enter);
        let cylinder = s.document().bodies.iter().rfind(|b| !b.consumed && !b.sheet).cloned().unwrap_or_else(|| panic!("no cylinder was made"));
        assert!((cylinder.min[0] - 15.0).abs() < 0.1, "the cylinder was not put on the point at 25: it runs from {:?}", cylinder.min);
        take(&mut s, "g-datum-axis-hint");
        let side = the_side_of_a_shaft(&mut s, 10.0, 10.0);
        s.click(side);
        s.key(Key::Enter);
        let d = last(&mut s, "axis");
        assert!(points_along(d.dir, [0.0, 0.0, 1.0]), "the axis of an upright cylinder runs {:?}", d.dir);
        assert!((d.at[0] - 25.0).abs() < 1e-3 && (d.at[1]).abs() < 1e-3, "the axis does not run up the middle of the cylinder at (25, 0): it starts at {:?}", d.at);
    }
}

probe! {
    /// AN AXIS THROUGH TWO POINTS runs from one to the other - not along the world's own axis it would take by
    /// default.
    fn an_axis_is_made_through_two_points() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        for y in [0.0, 20.0] {
            take(&mut s, "g-datum-point-hint");
            bar_word(&mut s, "cmd-coordinates");
            field(&mut s, "X", "10");
            field(&mut s, "Y", &y.to_string());
            s.key(Key::Enter);
        }
        take(&mut s, "g-datum-axis-hint");
        bar_word(&mut s, "cmd-two-points");
        for y in [0.0, 20.0] {
            let at = s.in_space([10.0, y, 0.0]);
            s.click(at);
        }
        s.key(Key::Enter);
        let d = last(&mut s, "axis");
        assert!(points_along(d.dir, [0.0, 1.0, 0.0]), "the axis through (10, 0, 0) and (10, 20, 0) runs {:?}", d.dir);
        assert!((d.at[0] - 10.0).abs() < 1e-3, "the axis does not pass through the points: it starts at {:?}", d.at);
    }
}

probe! {
    /// AN AXIS BY HAND runs where the numbers say.
    fn an_axis_is_made_by_hand() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        take(&mut s, "g-datum-axis-hint");
        bar_word(&mut s, "cmd-manual");
        s.key(Key::Enter);
        let d = last(&mut s, "axis");
        assert!(d.dir.iter().any(|v| v.abs() > 1e-6), "the axis made by hand has no direction at all: {:?}", d.dir);
    }
}

probe! {
    /// THE OFFSET OF A DATUM PLANE IS DRAGGED BY ITS ARROW: a plane taken from the table, offset 20, its arrow from the
    /// origin to (0, 0, 20) pulled 40 px up - the offset grows, and Enter puts the plane where the field says. Reported
    /// behaviour: the datum tools had no arrow, their values were typed only.
    fn the_offset_of_a_datum_plane_is_dragged_by_its_arrow() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        take(&mut s, "g-datum-plane-hint");
        let at = s.in_space([0.0, 0.0, 0.0]);
        s.click(at);
        let offset = s.word("f-offset");
        field(&mut s, &offset, "20");
        let tip = s.projected([0.0, 0.0, 20.0]);
        s.drag(tip, tip + qymcad::vec2(0.0, -40.0), qymcad::PointerButton::Primary, qymcad::Modifiers::NONE);
        let pulled: f64 = s.field(&offset).value.trim().parse().unwrap_or(f64::NAN);
        assert!(pulled > 20.5, "the arrow pulled up 40 px left the offset at {pulled}");
        s.key(Key::Enter);
        let d = last(&mut s, "plane");
        assert!((d.at[2] - pulled).abs() < 1e-3, "the plane stands at {:?}, the field said {pulled}", d.at);
    }
}

probe! {
    /// A DATUM POINT IS DRAGGED BY ITS ARROWS: a point by its coordinates, X 10, the arrow along X pulled 40 px along
    /// itself - X grows, Y and Z stay; Enter puts the point there.
    fn a_datum_point_is_dragged_by_its_arrows() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        take(&mut s, "g-datum-point-hint");
        bar_word(&mut s, "cmd-coordinates");
        field(&mut s, "X", "10");
        // the arrow along X on the screen: from the point, the way +X runs there
        let (from, along) = (s.projected([10.0, 0.0, 0.0]), s.projected([11.0, 0.0, 0.0]));
        let way = (along - from).normalized();
        // the middle of the arrow: the fields of the point must not stand over it
        let grab = from + way * 30.0;
        s.drag(grab, grab + way * 40.0, qymcad::PointerButton::Primary, qymcad::Modifiers::NONE);
        s.key(Key::Enter);
        let d = last(&mut s, "point");
        assert!(d.at[0] > 10.5 && d.at[1].abs() < 1e-6 && d.at[2].abs() < 1e-6, "the point pulled along X stands at {:?}", d.at);
    }
}

probe! {
    /// THE ORIGIN OF AN AXIS BY HAND IS DRAGGED BY ITS ARROWS: an axis set by hand, its origin X 10, the arrow along X
    /// pulled 40 px along itself - the origin's X grows; the modes that take an edge or a face are as they were.
    fn the_origin_of_an_axis_by_hand_is_dragged_by_its_arrows() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        take(&mut s, "g-datum-axis-hint");
        bar_word(&mut s, "cmd-manual");
        field(&mut s, "O.x", "10");
        let (from, along) = (s.projected([10.0, 0.0, 0.0]), s.projected([11.0, 0.0, 0.0]));
        let way = (along - from).normalized();
        let grab = from + way * 30.0;
        s.drag(grab, grab + way * 40.0, qymcad::PointerButton::Primary, qymcad::Modifiers::NONE);
        s.key(Key::Enter);
        let d = last(&mut s, "axis");
        assert!(d.at[0] > 10.5, "the origin of the axis pulled along X stands at {:?}", d.at);
    }
}

probe! {
    /// A FACE BEHIND A BASE PLANE IS PICKED, NOT THE PLANE: a block from y 20 to 50, 10 high, the plane XZ (y 0) between
    /// it and the eye, the datum plane taken and the front face of the block clicked near the origin - the plane is made
    /// from that face, offset from y 20 outward, not from XZ. Reported behaviour: the square of a base plane at the origin took the
    /// pointer over the faces behind it, a sketch or a datum could not be put on them.
    fn a_face_behind_a_base_plane_is_picked_before_the_plane() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        let xy = s.word("plane-xy-table");
        s.press_word(&xy);
        let rect = s.word("tb-rect-hint");
        s.press_hint(&rect).click_on_sketch(0.0, 20.0).click_on_sketch(40.0, 50.0);
        let finish = s.word("wb-finish");
        s.press_word(&finish);
        let extrude = s.word("tb-extrude-hint");
        s.press_hint(&extrude);
        s.key(Key::Enter);
        s.key(Key::Enter);
        take(&mut s, "g-datum-plane-hint");
        let at = s.projected([8.0, 20.0, 5.0]);
        s.click(at);
        s.key(Key::Enter);
        let d = last(&mut s, "plane");
        assert!(points_along(d.dir, [0.0, 1.0, 0.0]) && d.at[1] < 19.0, "the plane picked on the front face at y 20 stands at {:?} looking {:?} - taken from a base plane, not from the face", d.at, d.dir);
    }
}

/// HOW MANY POINTS OF THE SCREEN A MILLIMETRE IS, across the view at `at`: the projections of the three axes are two rows
/// of the view's turn, scaled - their squares sum to twice the square of the scale.
fn px_per_mm(s: &mut Session, at: [f64; 3]) -> f32 {
    let c = s.projected(at);
    let sq: f32 = (0..3)
        .map(|k| {
            let mut p = at;
            p[k] += 1.0;
            (s.projected(p) - c).length_sq()
        })
        .sum();
    (sq / 2.0).sqrt()
}

/// The radius of the rings of a datum on the screen, in points, as they are drawn whatever the zoom.
const RING_PX: f32 = 42.0;

/// The length of the arrows of a position on the screen, in points.
const ARROW_PX: f32 = 60.0;

/// The point at `deg` degrees round the ring about world X centred at `c`: from +Y towards +Z.
fn on_the_ring_about_x(c: [f64; 3], radius: f64, deg: f64) -> [f64; 3] {
    let (sin, cos) = deg.to_radians().sin_cos();
    [c[0], c[1] + radius * cos, c[2] + radius * sin]
}

/// `p` turned about the world X line through `c` by `deg` degrees, counter-clockwise looking down +X.
fn turned_about_x(p: [f64; 3], c: [f64; 3], deg: f64) -> [f64; 3] {
    let (sin, cos) = deg.to_radians().sin_cos();
    let (y, z) = (p[1] - c[1], p[2] - c[2]);
    [p[0], c[1] + y * cos - z * sin, c[2] + y * sin + z * cos]
}

/// A RING TURNED BY THE POINTER: the point of the world grabbed on it, and where the pointer let go.
struct RingTurned {
    grabbed: [f64; 3],
    let_go: qymcad::Pos2,
}

/// THE RING ABOUT X AT `c` GRABBED AS A PERSON GRABS IT - between the arrows, at 45 deg off them, where no arrow (from
/// `arrow_from` to each of `arrow_tips` on the screen) passes within 15 points - and dragged 30 points along itself.
fn turn_the_ring_about_x(s: &mut Session, c: [f64; 3], arrow_from: qymcad::Pos2, arrow_tips: &[qymcad::Pos2]) -> RingTurned {
    let radius = f64::from(RING_PX / px_per_mm(s, c));
    let clear = |s: &mut Session, deg: f64| {
        let at = s.projected(on_the_ring_about_x(c, radius, deg));
        arrow_tips.iter().all(|tip| segment_distance(at, arrow_from, *tip) > 15.0)
    };
    let deg = [45.0, 135.0, 225.0, 315.0].into_iter().find(|d| clear(s, *d)).expect("some place of the ring stands clear of the arrows");
    let grabbed = on_the_ring_about_x(c, radius, deg);
    let from = s.projected(grabbed);
    let way = (s.projected(on_the_ring_about_x(c, radius, deg + 1.0)) - from).normalized();
    let let_go = from + way * 30.0;
    s.drag(from, let_go, qymcad::PointerButton::Primary, qymcad::Modifiers::NONE);
    RingTurned { grabbed, let_go }
}

/// The distance on the screen from `p` to the segment from `a` to `b`.
fn segment_distance(p: qymcad::Pos2, a: qymcad::Pos2, b: qymcad::Pos2) -> f32 {
    let ab = b - a;
    let t = if ab.length_sq() < 1e-6 { 0.0 } else { ((p - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0) };
    p.distance(a + ab * t)
}

/// The tips of the three arrows of a position at `c` on the screen, along X, Y and Z.
fn position_arrow_tips(s: &mut Session, c: [f64; 3]) -> Vec<qymcad::Pos2> {
    let l = f64::from(ARROW_PX / px_per_mm(s, c));
    (0..3)
        .map(|k| {
            let mut tip = c;
            tip[k] += l;
            s.projected(tip)
        })
        .collect()
}

probe! {
    /// THE DIRECTION OF AN AXIS BY HAND IS TURNED BY ITS RINGS: an axis by hand up Z from the origin, its ring about X
    /// grabbed between the arrows and dragged 30 points along itself - the direction turns about X, the way the hand went
    /// (the point grabbed, turned with it, comes nearer where the pointer let go); Enter makes the axis so. Reported
    /// behaviour: the axis by hand had no rings, its direction was typed only.
    fn the_direction_of_an_axis_by_hand_is_turned_by_its_rings() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        take(&mut s, "g-datum-axis-hint");
        bar_word(&mut s, "cmd-manual");
        let c = [0.0, 0.0, 0.0];
        let (from, tips) = (s.projected(c), position_arrow_tips(&mut s, c));
        let turn = turn_the_ring_about_x(&mut s, c, from, &tips);
        s.key(Key::Enter);
        let d = last(&mut s, "axis").dir;
        let deg = (-d[1]).atan2(d[2]).to_degrees();
        let followed = s.projected(turned_about_x(turn.grabbed, c, deg)).distance(turn.let_go) < s.projected(turn.grabbed).distance(turn.let_go);
        assert!(d[0].abs() < 1e-6 && deg.abs() > 5.0 && followed, "the ring about X dragged 30 points turned the axis to {d:?} ({deg:.1} deg about X), the hand {}", if followed { "followed" } else { "not followed" });
    }
}

probe! {
    /// A DATUM PLANE IS TILTED BY ITS RING: a plane from the table offset 20, its ring about X at (0, 0, 20) grabbed
    /// between the arrow and dragged 30 points along itself - Enter makes the plane at (0, 0, 20) tilted about X, the way
    /// the hand went. Reported behaviour: a datum plane had no rings.
    fn a_datum_plane_is_tilted_by_its_ring() {
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        plane_offset_20_in_hand(&mut s);
        let c = [0.0, 0.0, 20.0];
        let (from, tip) = (s.projected([0.0, 0.0, 0.0]), s.projected(c));
        let turn = turn_the_ring_about_x(&mut s, c, from, &[tip]);
        s.key(Key::Enter);
        let d = last(&mut s, "plane");
        let deg = (-d.dir[1]).atan2(d.dir[2]).to_degrees();
        let followed = s.projected(turned_about_x(turn.grabbed, c, deg)).distance(turn.let_go) < s.projected(turn.grabbed).distance(turn.let_go);
        assert!(
            stands_at(d.at, c) && d.dir[0].abs() < 1e-6 && deg.abs() > 5.0 && followed,
            "the ring about X dragged 30 points made the plane at {:?} looking {:?} ({deg:.1} deg about X), the hand {}",
            d.at,
            d.dir,
            if followed { "followed" } else { "not followed" }
        );
    }
}

/// A datum plane from the table under the origin, offset 20, in hand.
fn plane_offset_20_in_hand(s: &mut Session) {
    take(s, "g-datum-plane-hint");
    let at = s.in_space([0.0, 0.0, 0.0]);
    s.click(at);
    let offset = s.word("f-offset");
    field(s, &offset, "20");
}

/// The points of the screen the handles of a datum at `c` are drawn through: its arrows from `arrow_from` to each of
/// `arrow_tips`, and, `with_rings`, its rings about X, Y and Z.
fn handle_points(s: &mut Session, c: [f64; 3], arrow_from: qymcad::Pos2, arrow_tips: &[qymcad::Pos2], rings: Rings) -> Vec<qymcad::Pos2> {
    let mut pts: Vec<qymcad::Pos2> = arrow_tips.iter().flat_map(|tip| (0..=20).map(move |k| arrow_from + (*tip - arrow_from) * (k as f32 / 20.0))).collect();
    if rings == Rings::Drawn {
        let r = f64::from(RING_PX / px_per_mm(s, c));
        for k in 0..72 {
            let (sin, cos) = (f64::from(k) * 5.0_f64.to_radians()).sin_cos();
            for p in [[c[0], c[1] + r * cos, c[2] + r * sin], [c[0] + r * cos, c[1], c[2] + r * sin], [c[0] + r * cos, c[1] + r * sin, c[2]]] {
                pts.push(s.projected(p));
            }
        }
    }
    pts
}

/// Whether a datum has rings to look clear of.
#[derive(PartialEq)]
enum Rings {
    Drawn,
    None,
}

/// The fields on the screen that stand over a point of `pts`, with the frame of their box (8 points) round them.
fn fields_over(s: &mut Session, pts: &[qymcad::Pos2]) -> Vec<String> {
    s.widgets().into_iter().filter(|w| w.kind == qymcad::Kind::TextField).filter(|w| pts.iter().any(|p| w.rect.expand(8.0).contains(*p))).map(|w| format!("{:?} \"{}\"", w.rect, w.value)).collect()
}

/// A POINT OF THE WORLD SEEN 90 POINTS FROM THE LEFT EDGE OF THE CANVAS, on the axis of the world that runs most nearly
/// across the view: the box of a datum laid on its left is pushed back by the edge.
fn near_the_left_edge(s: &mut Session) -> [f64; 3] {
    let o = s.projected([0.0; 3]);
    let across: Vec<f32> = (0..3)
        .map(|k| {
            let mut e = [0.0; 3];
            e[k] = 1.0;
            s.projected(e).x - o.x
        })
        .collect();
    let k = (0..3).max_by(|a, b| across[*a].abs().total_cmp(&across[*b].abs())).unwrap_or(0);
    let mut at = [0.0; 3];
    at[k] = f64::from((s.canvas().left() + 90.0 - o.x) / across[k]);
    at
}

/// Type the place `at` into the fields under `captions`, X, Y and Z.
fn type_the_place(s: &mut Session, captions: [&str; 3], at: [f64; 3]) {
    for (caption, v) in captions.iter().zip(at) {
        field(s, caption, &format!("{v:.3}"));
    }
}

probe! {
    /// THE FIELDS OF A DATUM STAND CLEAR OF ITS ARROWS AND RINGS: a point by its coordinates and an axis by hand, in the
    /// middle of the view and near its left edge, the view turned three ways - no field, nor the frame of its box, stands
    /// over a point an arrow or a ring is drawn through. Reported behaviour: the fields of an axis by hand and of a point
    /// stood over their arrows - laid left of the point, the box was pushed back over it by the edge of the view.
    fn the_fields_of_a_datum_stand_clear_of_its_handles() {
        let mut failed = Vec::new();
        let mut s = Session::start();
        build::into_the_first_part(&mut s);
        for turn in 0..3 {
            if turn > 0 {
                // the view turned as a person turns it: dragged on an empty place
                let from = s.projected([0.0, 0.0, 0.0]) + qymcad::vec2(250.0, 180.0);
                s.drag(from, from + qymcad::vec2(260.0, 0.0), qymcad::PointerButton::Primary, qymcad::Modifiers::NONE);
            }
            let edge = near_the_left_edge(&mut s);
            for (place, c) in [("in the middle", [10.0, 0.0, 0.0]), ("near the left edge", edge)] {
                take(&mut s, "g-datum-point-hint");
                bar_word(&mut s, "cmd-coordinates");
                type_the_place(&mut s, ["X", "Y", "Z"], c);
                let (from, tips) = (s.projected(c), position_arrow_tips(&mut s, c));
                let pts = handle_points(&mut s, c, from, &tips, Rings::None);
                let over = fields_over(&mut s, &pts);
                if !over.is_empty() {
                    failed.push(format!("a point {place}, the view turned {turn} times: {over:?}"));
                }
                s.key(Key::Escape);

                take(&mut s, "g-datum-axis-hint");
                bar_word(&mut s, "cmd-manual");
                type_the_place(&mut s, ["O.x", "O.y", "O.z"], c);
                let (from, tips) = (s.projected(c), position_arrow_tips(&mut s, c));
                let pts = handle_points(&mut s, c, from, &tips, Rings::Drawn);
                let over = fields_over(&mut s, &pts);
                if !over.is_empty() {
                    failed.push(format!("an axis by hand {place}, the view turned {turn} times: {over:?}"));
                }
                s.key(Key::Escape);
            }
        }
        assert!(failed.is_empty(), "fields standing over the handles of a datum:\n{}", failed.join("\n"));
    }
}
