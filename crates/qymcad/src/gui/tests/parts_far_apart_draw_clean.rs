//! PARTS METRES APART ARE DRAWN CLEAN, framed together and looked at one by one.
//!
//! Reported behaviour: "the display glitches if the parts are very far from each other". Two blocks of 50 mm, 5 m apart:
//! each face of each block must come out in one tone, the same tone a pixel of turning later, and nothing may be drawn
//! where no block stands - whether the other block is far behind the one looked at or behind the eye.
use crate::gui::App;

const SIZE: egui::Vec2 = egui::vec2(900.0, 700.0);

fn rect() -> egui::Rect {
    egui::Rect::from_min_size(egui::pos2(0.0, 0.0), SIZE)
}

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

/// Where the second block stands against the first, at the origin.
struct Layout {
    name: &'static str,
    far: [f64; 3],
}

/// From the usual three-quarter view the eye looks along -(0.63, -0.53, 0.57): a block put along the sight stands on
/// screen exactly over or under the other, behind it or in front of it - or behind the eye once the view is close.
const LAYOUTS: [Layout; 5] = [
    Layout { name: "5 m along X", far: [5000.0, 0.0, 0.0] },
    Layout { name: "5 m behind along the sight", far: [-3155.0, 2655.0, -2825.0] },
    Layout { name: "5 m in front along the sight", far: [3155.0, -2655.0, 2825.0] },
    Layout { name: "20 m behind along the sight", far: [-12620.0, 10620.0, -11300.0] },
    Layout { name: "100 m along X", far: [100000.0, 0.0, 0.0] },
];

fn two_parts(layout: &Layout) -> App {
    let mut app = App::default();
    block_part(&mut app, "Near", [0.0; 3]);
    block_part(&mut app, "Far", layout.far);
    app.enter_component(app.project.root);
    assert_eq!(app.project.bodies.len(), 2, "setup: two blocks");
    app.viewing.mode_3d = true;
    app.viewing.cam.init = true;
    app
}

/// Where the camera is put: framing both, or so many mm across at one block.
struct Aim {
    name: &'static str,
    at_far: Option<bool>,
    across: f32,
}

const AIMS: [Aim; 4] = [
    Aim { name: "both framed", at_far: None, across: 0.0 },
    Aim { name: "100 mm at the near block", at_far: Some(false), across: 100.0 },
    Aim { name: "100 mm at the far block", at_far: Some(true), across: 100.0 },
    Aim { name: "1 m at the near block", at_far: Some(false), across: 1000.0 },
];

fn aim(app: &mut App, at: &Aim, layout: &Layout) {
    app.viewing.cam.yaw = -0.7;
    app.viewing.cam.pitch = 0.6;
    match at.at_far {
        None => crate::gui::fit3d(&mut app.viewing.cam, &app.project, rect()),
        Some(far) => {
            let o = if far { layout.far } else { [0.0; 3] };
            app.viewing.cam.target = [o[0] + 25.0, o[1] + 25.0, o[2] + 25.0];
            app.viewing.cam.scale = SIZE.x / at.across;
        }
    }
}

/// A picture of the view by the card or by the software raster.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Drawer {
    Raster,
    Card,
}

fn draw(app: &App, by: Drawer) -> Option<egui::ColorImage> {
    match by {
        Drawer::Raster => qymcad_render::rasterize_3d(&app.painting(), rect(), &app.viewing.cam.basis(), 1.0, 1.0),
        Drawer::Card => crate::gui::gpu_shot::eyes::shot(&app.painting(), rect()),
    }
}

/// The same view with the blocks hidden: what is left is what stands under them - the floor.
fn without_parts(app: &mut App, by: Drawer) -> Option<egui::ColorImage> {
    for b in &mut app.project.bodies {
        b.visible = false;
    }
    let img = draw(app, by);
    for b in &mut app.project.bodies {
        b.visible = true;
    }
    img
}

/// How far apart two colours are, in levels of the channel that differs most.
fn apart(pair: [egui::Color32; 2]) -> i32 {
    let (a, b) = (pair[0].to_array(), pair[1].to_array());
    (0..4).map(|k| (a[k] as i32 - b[k] as i32).abs()).max().unwrap_or(0)
}

/// One face of a block as the camera should see it: which it is, and the points inside it to sample - its middle
/// first - that no nearer block covers on screen.
struct FaceSeen {
    label: String,
    points: Vec<egui::Pos2>,
}

/// What the camera should see of the blocks: the rectangles their corners land in, and their faces turned to the eye.
struct Expected {
    boxes: Vec<egui::Rect>,
    faces: Vec<FaceSeen>,
}

/// One block in front of the eye: where it lands on screen, how deep its middle is, its faces turned to the eye.
struct BlockSeen {
    on_screen: egui::Rect,
    depth: f64,
    faces: Vec<FaceSeen>,
}

fn expected(app: &App) -> Expected {
    let basis = app.viewing.cam.basis();
    let scr = qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect: rect(), basis: &basis };
    let inv_d = qymcad_ui_state::persp_inv_d_eye(&app.viewing.cam, &app.set, SIZE.y * 0.5);
    let depth = |p: [f64; 3]| qymcad_ui_state::v_dot(qymcad_ui_state::v_sub(p, app.viewing.cam.target), basis.2);
    // in front of the eye by a millimetre at least, or (orthographic) anywhere
    let ahead = |p: [f64; 3]| inv_d <= 0.0 || depth(p) + 1.0 / inv_d > 1.0;
    let mut blocks = Vec::new();
    for (mi, body) in app.project.bodies.iter().enumerate() {
        let Some(id) = app.project.mesh_id(mi) else { continue };
        let wt = app.project.body_world_transform(id);
        let world = |p: qymcad_core::geom::Point3| qymcad_core::feature::apply12(&wt, [p.x, p.y, p.z]);
        let corners: Vec<[f64; 3]> = body.mesh.verts.iter().map(|v| world(*v)).collect();
        if !corners.iter().all(|p| ahead(*p)) {
            continue; // a block behind the eye, or cut by it, is not looked at
        }
        let on: Vec<egui::Pos2> = corners.iter().map(|p| scr.at(*p).0).collect();
        let mut block = BlockSeen { on_screen: egui::Rect::from_points(&on).expand(3.0), depth: depth(world(body.mesh.verts[0])), faces: Vec::new() };
        for (fi, f) in body.faces.iter().enumerate() {
            let c = world(f.centroid);
            let sight = qymcad_ui_state::view_dir_at(&app.viewing.cam, c, basis.2, inv_d);
            let n = qymcad_core::feature::apply12_dir(&wt, f.normal);
            if qymcad_ui_state::v_dot(n, sight) > -0.2 {
                continue; // turned away or edge-on
            }
            let fc: Vec<[f64; 3]> = f.triangles.iter().flat_map(|&t| body.mesh.triangle(t as usize)).map(world).collect();
            let mid = scr.at(c).0;
            let inside: Vec<egui::Pos2> = std::iter::once(mid).chain(fc.iter().map(|p| mid + (scr.at(*p).0 - mid) * 0.6)).collect();
            if inside.iter().all(|p| rect().shrink(2.0).contains(*p)) && egui::Rect::from_points(&inside).area() > 150.0 {
                block.faces.push(FaceSeen { label: format!("block {mi} face {fi}"), points: inside });
            }
        }
        blocks.push(block);
    }
    let mut out = Expected { boxes: blocks.iter().map(|b| b.on_screen).collect(), faces: Vec::new() };
    for (k, b) in blocks.iter().enumerate() {
        let covered = |p: &egui::Pos2| blocks.iter().enumerate().any(|(j, o)| j != k && o.depth < b.depth && o.on_screen.contains(*p));
        for f in &b.faces {
            if covered(&f.points[0]) {
                continue; // behind a nearer block on screen
            }
            out.faces.push(FaceSeen { label: f.label.clone(), points: f.points.iter().copied().filter(|p| !covered(p)).collect() });
        }
    }
    out
}

/// A picture of the view, and the same view with the blocks hidden.
struct Shot {
    img: egui::ColorImage,
    floor: egui::ColorImage,
}

/// What is wrong with one picture of the view: anything drawn off the blocks, a face of more than one tone.
fn faults(shot: &Shot, seen: &Expected) -> Vec<String> {
    let Shot { img, floor } = shot;
    let mut wrong = Vec::new();
    let w = img.size[0];
    let stray = (0..img.pixels.len())
        .filter(|&i| {
            let at = egui::pos2((i % w) as f32 + 0.5, (i / w) as f32 + 0.5);
            !seen.boxes.iter().any(|b| b.contains(at)) && apart([img.pixels[i], floor.pixels[i]]) > 8
        })
        .count();
    if stray > 0 {
        wrong.push(format!("{stray} pixels drawn where no block stands"));
    }
    // the floor lies behind every block, a block under it seen from above included: each face is sampled whole
    for face in &seen.faces {
        let tones: Vec<egui::Color32> = face.points.iter().map(|p| img.pixels[p.y as usize * w + p.x as usize]).collect();
        if tones.iter().any(|c| c.a() < 128) {
            wrong.push(format!("{}: not drawn where it stands", face.label));
        } else if tones.first().is_some_and(|t0| tones.iter().any(|c| apart([*c, *t0]) > 3)) {
            wrong.push(format!("{}: drawn in more than one tone {tones:?}", face.label));
        }
    }
    wrong
}

#[test]
fn parts_5_m_apart_are_drawn_clean() {
    let mut wrong = Vec::new();
    for layout in &LAYOUTS {
        let mut app = two_parts(layout);
        for by in [Drawer::Raster, Drawer::Card] {
            for projection in [qymcad_ui_state::Projection::Ortho, qymcad_ui_state::Projection::Perspective] {
                app.set.projection = projection;
                for at in &AIMS {
                    aim(&mut app, at, layout);
                    let mut tone_before: std::collections::HashMap<String, egui::Color32> = std::collections::HashMap::new();
                    for turn in 0..2 {
                        app.viewing.cam.yaw = -0.7 + 0.01 * turn as f64;
                        let (Some(img), Some(floor)) = (draw(&app, by), without_parts(&mut app, by)) else {
                            eprintln!("PASSED OVER: {by:?} - there is no graphics device here");
                            continue;
                        };
                        let seen = expected(&app);
                        let name = format!("{}: {by:?} {projection:?} {}, turn {turn}", layout.name, at.name);
                        if let Some(png) = crate::gui::color_image_to_png(&img) {
                            let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/look");
                            std::fs::create_dir_all(&dir).ok();
                            std::fs::write(dir.join(format!("far-{}.png", name.replace([' ', ','], "-"))), png).ok();
                        }
                        let shot = Shot { img, floor };
                        for f in faults(&shot, &seen) {
                            wrong.push(format!("{name}: {f}"));
                        }
                        // a pixel of turning later each face keeps its tone
                        let img = &shot.img;
                        let tones: std::collections::HashMap<String, egui::Color32> =
                            seen.faces.iter().map(|f| (f.label.clone(), img.pixels[f.points[0].y as usize * img.size[0] + f.points[0].x as usize])).collect();
                        for (label, b) in &tones {
                            if let Some(a) = tone_before.get(label).filter(|a| apart([**a, *b]) > 4) {
                                wrong.push(format!("{name}: {label} turned from {a:?} to {b:?}"));
                            }
                        }
                        tone_before = tones;
                    }
                }
            }
        }
    }
    assert!(wrong.is_empty(), "parts 5 m apart are not drawn clean ({}):\n{}", wrong.len(), wrong.join("\n"));
}
