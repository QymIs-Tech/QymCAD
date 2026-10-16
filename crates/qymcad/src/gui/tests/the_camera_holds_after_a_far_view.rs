//! THE CAMERA HOLDS AFTER A FAR VIEW, in both centres of turning and both projections.
//!
//! Reported behaviour: "problems with the camera far out, say at 4 metres: then, zoomed in to 100 mm and turned, it
//! collapses - everything flies off into space, it turns fast and touchy. Both variants of the list need redoing." The
//! chain is the reporter's: the whole document framed by the home button of the cube, the wheel down to 100 mm over a
//! part, a turn by a drag of 200 px, a pan, a turn again - by the hand, in whole frames. After every step: the point of
//! the model the step holds stays on its spot of the screen, the part stays in the frame, a pixel turns the view by the
//! same angle as it did framed.
use super::super::hand::{Hand, ViewDrag};
use crate::gui::App;

/// A part holding a block of 50 at its own origin, the part standing at `at`.
fn block_part(app: &mut App, name: &str, at: [f64; 3]) {
    let root = app.project.root;
    app.project.set_active_component(Some(root));
    let part = app.project.add_part(name);
    app.enter_component(part);
    let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    app.project.add_rect_entity(si, 0.0, 0.0, 50.0, 50.0, qymcad_core::feature::Purpose::Real);
    app.project.regen_sketch(si);
    app.finish_sketch_edit();
    let sid = app.project.sketches[si].id;
    let profiles = app.project.sketches[si].contour_ids.clone();
    app.project.add_extrude_multi(sid, profiles, 50.0, qymcad_core::feature::Reach::Forward, 0.0, Vec::new());
    if let Some(c) = app.project.components.iter_mut().find(|c| c.id == part) {
        (c.transform[3], c.transform[7], c.transform[11]) = (at[0], at[1], at[2]);
    }
    qymcad_ui_state::regenerate_now(&mut app.rebuild_ctx());
}

fn blocks(far: &[[f64; 3]]) -> Option<App> {
    let mut app = App::default();
    for (k, at) in far.iter().enumerate() {
        block_part(&mut app, &format!("Block {k}"), *at);
    }
    app.enter_component(app.project.root);
    Some(app)
}

/// A document of the owner's, where the samples are.
fn sample() -> Option<App> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/vacuumCleaner.qcad").to_string();
    if !std::path::Path::new(&path).exists() {
        eprintln!("PASSED OVER: the private sample vacuumCleaner.qcad is not in this tree");
        return None;
    }
    let project = qymcad_io::load_project(&path).expect("the document opens");
    let mut app = App::default();
    app.finish_project_load(path, project, Vec::new());
    qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
    Some(app)
}

/// A document the chain runs on.
struct Doc {
    name: &'static str,
    open: fn() -> Option<App>,
}

const DOCS: [Doc; 3] = [
    Doc { name: "two parts 5 m apart", open: || blocks(&[[0.0; 3], [5000.0, 0.0, 0.0]]) },
    Doc { name: "one part of 50 mm", open: || blocks(&[[0.0; 3]]) },
    Doc { name: "the vacuum cleaner", open: sample },
];

/// Where a world point lands, and whether it is in front of the eye.
fn spot(hand: &Hand, p: [f64; 3]) -> Option<egui::Pos2> {
    let cam = &hand.app.viewing.cam;
    let rect = hand.app.viewing.view_rect;
    let basis = cam.basis();
    let inv_d = qymcad_ui_state::persp_inv_d_eye(cam, &hand.app.set, rect.height() * 0.5);
    let ahead = inv_d <= 0.0 || qymcad_ui_state::v_dot(qymcad_ui_state::v_sub(p, cam.target), basis.2) * inv_d > -0.999;
    ahead.then(|| qymcad_ui_state::Screen { cam, set: &hand.app.set, rect, basis: &basis }.at(p).0)
}

/// The corners of the box of the first visible body - the part the chain zooms to.
fn part_box(app: &App) -> Vec<[f64; 3]> {
    let (mi, body) = app.project.bodies.iter().enumerate().find(|(_, b)| b.visible && !b.mesh.verts.is_empty()).expect("setup: a body to look at");
    let b = body.mesh.bounds().expect("setup: the body has a box");
    let wt = app.project.mesh_id(mi).map(|id| app.project.body_world_transform(id)).unwrap_or(qymcad_core::feature::PLACE_IDENTITY);
    (0..8).map(|k| qymcad_core::feature::apply12(&wt, [[b.min.x, b.max.x][k & 1], [b.min.y, b.max.y][(k >> 1) & 1], [b.min.z, b.max.z][(k >> 2) & 1]])).collect()
}

/// The middle of that box.
fn the_part(app: &App) -> [f64; 3] {
    let c = part_box(app);
    std::array::from_fn(|a| (c[0][a] + c[7][a]) * 0.5)
}

/// Whether some of the part is in the frame: its box, wholly in front of the eye, overlaps the view.
fn part_in_frame(hand: &Hand) -> bool {
    let on: Option<Vec<egui::Pos2>> = part_box(hand.app).into_iter().map(|c| spot(hand, c)).collect();
    on.is_some_and(|on| egui::Rect::from_points(&on).intersects(hand.app.viewing.view_rect))
}

/// What one run of the chain is about: the document, the centre of turning, the projection.
struct Run {
    doc: &'static Doc,
    about: qymcad_ui_state::OrbitAbout,
    projection: qymcad_ui_state::Projection,
}

/// The view turns about a point and the point must keep its spot: where the turn is about, as a person sees it - the
/// middle of the view, or where the drag begins.
fn turn_about(hand: &Hand, run: &Run, from: egui::Pos2) -> [f64; 3] {
    let rect = hand.app.viewing.view_rect;
    let at = if run.about == qymcad_ui_state::OrbitAbout::Pointer { from } else { rect.center() };
    crate::gui::look_at_point::point_under(&hand.app.painting(), rect, at)
}

/// HOW FAR THE POINTER GOES BEFORE A DRAG BEGINS: egui takes a press for a click until the pointer has moved this far,
/// and the view moves only after it. A person sees the same: the view starts after a few pixels.
const DRAG_STARTS_AFTER: f32 = 8.0;

/// What one turn by the hand is measured against: the angle a drag of 200 px turned the view framed.
struct Turning {
    framed: f64,
}

/// A turn by a drag of the right button, 200 px to the right: the pivot keeps its spot, the part stays in the frame,
/// and the view turns by the angle the same drag turned it framed.
fn turn(hand: &mut Hand, run: &Run, step: &str, turning: &Turning, wrong: &mut Vec<String>) {
    let rect = hand.app.viewing.view_rect;
    let Some(from) = spot(hand, the_part(hand.app)) else {
        wrong.push(format!("{step}: the part is behind the eye before the turn"));
        return;
    };
    let pivot = turn_about(hand, run, from);
    let (before, yaw0) = (spot(hand, pivot), hand.app.viewing.cam.yaw);
    hand.drag_view(ViewDrag { button: egui::PointerButton::Secondary, modifiers: egui::Modifiers::NONE, from, to: from + egui::vec2(200.0, 0.0) });
    let turned = yaw0 - hand.app.viewing.cam.yaw;
    if (turned - turning.framed).abs() > 0.01 * turning.framed {
        wrong.push(format!("{step}: 200 px turned the view by {turned:.3} rad, and framed by {:.3}", turning.framed));
    }
    match (before, spot(hand, pivot)) {
        (Some(a), Some(b)) if a.distance(b) <= 3.0 => {}
        (a, b) => wrong.push(format!("{step}: the point turned about went from {a:?} to {b:?}")),
    }
    if !part_in_frame(hand) {
        wrong.push(format!("{step}: the part left the frame - its middle is at {:?} of {rect:?}", spot(hand, the_part(hand.app))));
    }
}

fn chain(run: &Run) -> Vec<String> {
    let mut wrong = Vec::new();
    let Some(mut app) = (run.doc.open)() else { return wrong };
    app.set.gpu_viewport = false;
    app.set.mouse_nav = qymcad_ui_state::MouseNav::QymCad;
    app.set.zoom_at = qymcad_ui_state::ZoomAt::Cursor;
    app.set.orbit_about = run.about;
    app.set.projection = run.projection;
    let mut hand = Hand::new(&mut app);
    hand.frame(Vec::new());
    let rect = hand.app.viewing.view_rect;

    // FRAMED: the home button of the cube, and the view refits once the turn to it is over
    let home = crate::gui::viewcube::home_rect(&hand.app.cube_ctx(), rect).center();
    hand.click_screen(home);
    std::thread::sleep(std::time::Duration::from_millis(300));
    hand.frame(Vec::new()).frame(Vec::new());
    let across = rect.width() / hand.app.viewing.cam.scale;
    // the angle a drag of 200 px turns the view framed, and the view framed again
    let yaw = hand.app.viewing.cam.yaw;
    let middle = rect.center();
    hand.drag_view(ViewDrag { button: egui::PointerButton::Secondary, modifiers: egui::Modifiers::NONE, from: middle, to: middle + egui::vec2(200.0, 0.0) });
    let turning = Turning { framed: yaw - hand.app.viewing.cam.yaw };
    hand.click_screen(home);
    std::thread::sleep(std::time::Duration::from_millis(300));
    hand.frame(Vec::new()).frame(Vec::new());

    // THE WHEEL DOWN TO 100 MM over the part: the point of the model under the pointer stays under it, notch by notch
    let Some(over) = spot(&hand, the_part(hand.app)) else {
        wrong.push("framed: the part is not on screen".into());
        return wrong;
    };
    let held = crate::gui::look_at_point::point_under(&hand.app.painting(), rect, over);
    let mut notches = 0;
    while rect.width() / hand.app.viewing.cam.scale > 100.0 && notches < 60 {
        hand.wheel3d(over, 1);
        notches += 1;
        match spot(&hand, held) {
            Some(p) if p.distance(over) <= 3.0 => {}
            p => {
                wrong.push(format!("wheel, notch {notches} (framed {across:.0} mm across): the point under the pointer went from {over:?} to {p:?}"));
                break;
            }
        }
    }
    turn(&mut hand, run, "the first turn", &turning, &mut wrong);

    // A PAN, a quarter of the way to the middle of the view: the point under the pointer goes with the pointer
    if let Some(from) = spot(&hand, the_part(hand.app)) {
        let grabbed = crate::gui::look_at_point::point_under(&hand.app.painting(), rect, from);
        let to = from + (rect.center() - from) * 0.25; // towards the middle, so the part stays on the canvas
        hand.drag_view(ViewDrag { button: egui::PointerButton::Secondary, modifiers: egui::Modifiers::SHIFT, from, to });
        match spot(&hand, grabbed) {
            Some(p) if p.distance(to) <= DRAG_STARTS_AFTER => {}
            p => wrong.push(format!("pan: the point grabbed went to {p:?}, the pointer to {to:?}")),
        }
    }
    turn(&mut hand, run, "the second turn", &turning, &mut wrong);
    wrong
}

/// THE REPORTED CHAIN, every document, both centres of turning, both projections.
#[test]
fn the_camera_holds_after_a_far_view() {
    let mut wrong = Vec::new();
    for doc in &DOCS {
        for about in qymcad_ui_state::OrbitAbout::ALL {
            for projection in [qymcad_ui_state::Projection::Ortho, qymcad_ui_state::Projection::Perspective] {
                let run = Run { doc, about, projection };
                for w in chain(&run) {
                    wrong.push(format!("{} / {about:?} / {projection:?}: {w}", doc.name));
                }
            }
        }
    }
    assert!(wrong.is_empty(), "the camera does not hold after a far view ({}):\n{}", wrong.len(), wrong.join("\n"));
}
