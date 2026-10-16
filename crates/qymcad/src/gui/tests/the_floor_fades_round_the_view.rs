//! THE FLOOR HAS NO EDGE: it fades out round what is looked at, and never covers a part.
//!
//! Reported behaviour: "in the 3D view the table, the grid and the axes are not endless - one sees the plane end as the
//! view moves". The grid was a square of lines round the middle of the view, faded by the number of a line: a line
//! running towards the far side of the square kept its full strength to the end and stopped there, and the row where
//! the lines stopped was the edge one saw. Run to the horizon instead, the floor was reported as a ripple hard on the
//! eyes. So the floor fades out softly round the point looked at (`qymcad_ui_state::FloorLook`): where the grid ends
//! inside the frame, its lines are already faint.
use crate::gui::App;

/// The frame of the view, in points.
const SIZE: [usize; 2] = [900, 700];

/// Where the camera stands: its name, the projection, the tilt over the floor in degrees, the width of the view in mm.
struct View {
    name: &'static str,
    projection: qymcad_ui_state::Projection,
    tilt: f64,
    across: f32,
}

const VIEWS: [View; 4] = [
    View { name: "perspective, 10 deg over the floor, 20 m", projection: qymcad_ui_state::Projection::Perspective, tilt: 10.0, across: 20_000.0 },
    View { name: "perspective, three-quarter, 4 m", projection: qymcad_ui_state::Projection::Perspective, tilt: 34.0, across: 4_000.0 },
    View { name: "orthographic, 10 deg over the floor, 20 m", projection: qymcad_ui_state::Projection::Ortho, tilt: 10.0, across: 20_000.0 },
    View { name: "orthographic, three-quarter, 400 mm", projection: qymcad_ui_state::Projection::Ortho, tilt: 34.0, across: 400.0 },
];

/// A block of 50 standing at the origin, seen as `view` says.
fn looked_at(view: &View) -> App {
    let mut app = App::default();
    let si = app.create_sketch_on(qymcad_core::feature::SketchPlane::default());
    app.project.add_rect_entity(si, 0.0, 0.0, 50.0, 50.0, qymcad_core::feature::Purpose::Real);
    app.project.regen_sketch(si);
    app.finish_sketch_edit();
    app.chosen.sel = qymcad_ui_state::Sel::Sketch(si);
    app.start_feat_cmd(1);
    app.apply_feat_cmd();
    qymcad_ui_state::rebuild_if_dirty(&mut app.rebuild_ctx());
    app.chosen.sel = qymcad_ui_state::Sel::None;
    app.set.projection = view.projection;
    app.viewing.mode_3d = true;
    app.viewing.cam.init = true;
    app.viewing.cam.target = [0.0; 3];
    app.viewing.cam.yaw = -0.7;
    app.viewing.cam.pitch = view.tilt.to_radians();
    app.viewing.cam.scale = SIZE[0] as f32 / view.across;
    app
}

/// How strongly a pixel shows a line of the grid over the background: the grid is grey, so a pixel whose channels part
/// from the background unevenly is an axis, the block or the cube of the view, and counts for nothing.
fn grid_at(img: &egui::ColorImage, bg: egui::Color32, at: egui::Pos2) -> i32 {
    let p = img.pixels[at.y as usize * img.size[0] + at.x as usize];
    let d = [p.r() as i32 - bg.r() as i32, p.g() as i32 - bg.g() as i32, p.b() as i32 - bg.b() as i32];
    let (lo, hi) = (d.iter().copied().min().unwrap_or(0), d.iter().copied().max().unwrap_or(0));
    if hi - lo > 12 {
        0
    } else {
        hi.abs().max(lo.abs())
    }
}

/// THE GRID FADES OUT, IT DOES NOT BREAK OFF: every pixel is traced to the point of the floor under it, and the strength
/// of the grid is averaged over square bands round the point looked at, half the view's half diagonal wide - the square
/// of lines that ended had its edge on one of them. From one band to the next the grid may not fall from over half its
/// strength to under a tenth of it.
fn breaks(app: &App, img: &egui::ColorImage, bg: egui::Color32) -> Vec<String> {
    let [w, h] = img.size;
    let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(w as f32, h as f32));
    let pn = app.painting();
    let proj = qymcad_ui_state::proj_params(&pn, rect, qymcad_ui_state::gpu_scene_key(&pn));
    let rays = qymcad_ui_state::ViewRays::new(&app.viewing.cam, rect, 1.0, proj);
    let half_diagonal = (w as f64).hypot(h as f64) * 0.5 / app.viewing.cam.scale as f64;
    let centre = app.viewing.cam.target;
    let mut sum = vec![0.0f64; 60];
    let mut count = vec![0usize; 60];
    for y in 0..h {
        for x in 0..w {
            if x + 260 > w && y < 260 {
                continue; // the cube of the view stands in the corner over the floor
            }
            let ray = rays.through(((x as f64 + 0.5) / w as f64 * 2.0 - 1.0, 1.0 - (y as f64 + 0.5) / h as f64 * 2.0));
            if ray.along[2].abs() < 1e-12 {
                continue;
            }
            let t = -ray.from[2] / ray.along[2];
            if proj.inv_d_eye > 0.0 && t <= proj.z_near {
                continue; // the sky
            }
            let p = [ray.from[0] + ray.along[0] * t - centre[0], ray.from[1] + ray.along[1] * t - centre[1]];
            let square = (p[0].abs().max(p[1].abs()) / half_diagonal / 0.1) as usize;
            if square < sum.len() {
                sum[square] += grid_at(img, bg, egui::pos2(x as f32, y as f32)) as f64;
                count[square] += 1;
            }
        }
    }
    // half a diagonal at a time: a line parallel to a side of the square lies whole in one square, and the lines seen
    // stand about that far apart, so a square alone swings between a line and none
    let mean: Vec<Option<f64>> = (0..sum.len() - 5)
        .map(|k| {
            let n: usize = count[k..k + 5].iter().sum();
            (n >= 1000).then(|| sum[k..k + 5].iter().sum::<f64>() / n as f64)
        })
        .collect();
    let most = mean.iter().flatten().copied().fold(0.0, f64::max);
    if most < 0.5 {
        return vec![format!("there is no grid to speak of: {most:.2} at its most")];
    }
    let mut wrong = Vec::new();
    for k in 0..mean.len().saturating_sub(5) {
        if let (Some(a), Some(b)) = (mean[k], mean[k + 5]) {
            if a > most * 0.5 && b < most * 0.1 {
                wrong.push(format!("the grid breaks off past {:.1} half diagonals of the view: {a:.2} -> {b:.2} (at its most {most:.2})", (k + 5) as f64 * 0.1));
                break;
            }
        }
    }
    wrong
}

fn keep(img: &egui::ColorImage, name: &str) {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/look");
    std::fs::create_dir_all(&dir).ok();
    if let Some(png) = crate::gui::color_image_to_png(img) {
        std::fs::write(dir.join(format!("floor-{}.png", name.replace([' ', ','], "-"))), png).ok();
    }
}

/// THE FLOOR AS THE WINDOW DRAWS IT, by the software raster: the bodies, the floor and whatever lies over them, taken
/// without a screen.
#[test]
fn the_floor_fades_out_in_the_raster() {
    let mut wrong = Vec::new();
    for view in &VIEWS {
        let mut app = looked_at(view);
        app.set.gpu_viewport = false;
        let bg = app.scheme.pal.viewport_bg();
        let img = crate::gui::help_raster::shot_ui(SIZE, bg, |ui| app.viewport(ui));
        keep(&img, &format!("raster {}", view.name));
        let rect = app.viewing.view_rect;
        let canvas = egui::ColorImage {
            size: [rect.width() as usize, rect.height() as usize],
            pixels: (rect.min.y as usize..rect.max.y as usize).flat_map(|y| (rect.min.x as usize..rect.max.x as usize).map(move |x| (x, y))).map(|(x, y)| img.pixels[y * SIZE[0] + x]).collect(),
            ..img
        };
        for w in breaks(&app, &canvas, bg) {
            wrong.push(format!("{}: {w}", view.name));
        }
    }
    assert!(wrong.is_empty(), "the floor ends at an edge ({}):\n{}", wrong.len(), wrong.join("\n"));
}

/// THE SAME FLOOR ON THE CARD.
#[test]
fn the_floor_fades_out_on_the_card() {
    let mut wrong = Vec::new();
    for view in &VIEWS {
        let app = looked_at(view);
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(SIZE[0] as f32, SIZE[1] as f32));
        let Some(card) = crate::gui::gpu_shot::eyes::shot(&app.painting(), rect) else {
            eprintln!("PASSED OVER: there is no graphics device here - the card cannot be asked");
            return;
        };
        // the card's picture is transparent where nothing is drawn: laid over the background as the window lays it
        let bg = app.scheme.pal.viewport_bg();
        let over = |p: &egui::Color32| {
            let k = 1.0 - p.a() as f32 / 255.0;
            let m = |s: u8, b: u8| (s as f32 + b as f32 * k).round().min(255.0) as u8;
            egui::Color32::from_rgb(m(p.r(), bg.r()), m(p.g(), bg.g()), m(p.b(), bg.b()))
        };
        let img = egui::ColorImage { pixels: card.pixels.iter().map(over).collect(), ..card };
        keep(&img, &format!("card {}", view.name));
        for w in breaks(&app, &img, bg) {
            wrong.push(format!("{}: {w}", view.name));
        }
    }
    assert!(wrong.is_empty(), "the card's floor ends at an edge ({}):\n{}", wrong.len(), wrong.join("\n"));
}

/// THE FLOOR IS KEPT WHILE THE VIEW STANDS: the model changing under a standing camera - a part hidden, as every frame of
/// a preview changes it - does not work the floor out again; the camera turning does, and the floor kept is the floor
/// worked out afresh. Worked out on every frame, the floor made the frames of an extrusion three times slower under a
/// loaded processor, and the autosave of a person waiting came before the extrusion was done.
#[test]
fn the_floor_is_kept_while_the_view_stands() {
    let mut app = looked_at(&VIEWS[1]);
    let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(SIZE[0] as f32, SIZE[1] as f32));
    let raster = |app: &App| qymcad_render::rasterize_3d(&app.painting(), rect, &app.viewing.cam.basis(), 1.0, 1.0).expect("the raster drew nothing");
    let kept = |app: &App| {
        let floor = app.cache.floor.borrow();
        (floor[0].rev, floor[0].value.as_ptr() as usize)
    };
    raster(&app);
    let first = kept(&app);
    app.project.bodies[0].visible = false;
    raster(&app);
    assert_eq!(kept(&app), first, "the floor was worked out again for a change of the model under a standing camera");
    app.viewing.cam.yaw += 0.01;
    raster(&app);
    let turned = kept(&app).0;
    assert_ne!(turned, first.0, "the camera turned and the floor was not worked out again");
    let held = app.cache.floor.borrow()[0].value.clone();
    app.cache.floor.borrow_mut().clear();
    raster(&app);
    assert!(app.cache.floor.borrow()[0].value == held, "the floor kept is not the floor worked out afresh");
}
