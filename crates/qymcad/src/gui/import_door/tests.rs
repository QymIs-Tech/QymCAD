use super::{import_answer, import_filters};
use crate::gui::App;
use qymcad_io::Format;
use qymcad_ui_state::Want;

const SCREEN: egui::Vec2 = egui::vec2(1400.0, 900.0);

/// One whole frame of the program with `events` in it; returns the painted labels, icons dropped.
pub(crate) fn frame(app: &mut App, ctx: &egui::Context, events: Vec<egui::Event>) -> Vec<(String, egui::Rect)> {
    fn walk(s: &egui::epaint::Shape, out: &mut Vec<(String, egui::Rect)>) {
        match s {
            egui::epaint::Shape::Text(t) => {
                let text: String = t.galley.text().chars().filter(|c| !('\u{e000}'..='\u{f8ff}').contains(c)).collect();
                out.push((text.trim().to_string(), egui::Rect::from_min_size(t.pos, t.galley.size())));
            }
            egui::epaint::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), SCREEN)), events, ..Default::default() };
    let out = ctx.run_ui(raw, |ui| app.draw_frame(ui));
    let mut texts = Vec::new();
    for cs in &out.shapes {
        walk(&cs.shape, &mut texts);
    }
    texts
}

pub(crate) fn click(spot: egui::Pos2) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(spot),
        egui::Event::PointerButton { pos: spot, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() },
        egui::Event::PointerButton { pos: spot, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() },
    ]
}

pub(crate) fn running() -> (App, egui::Context) {
    let mut app = App::default();
    let ctx = egui::Context::default();
    crate::gui::install_fonts(&ctx);
    app.waiting.splash_until = None;
    // AS THE WINDOW STARTS: in 3D at the isometric camera (`App::new`). `App::default` stands in 2D, and a picture
    // of the window taken from it drew every body as a flat silhouette from above, in one colour - which read as
    // "the capture without a graphics device draws a model under 1 mm or over 10 m wrong"
    app.viewing.mode_3d = true;
    app.viewing.cam = qymcad_ui_state::Cam3::default();
    for _ in 0..3 {
        frame(&mut app, &ctx, Vec::new());
    }
    (app, ctx)
}

/// The chooser answers with `path`, and the frame picks the answer up - the way the live window does.
pub(crate) fn answer(app: &mut App, ctx: &egui::Context, want: Want, path: &str) {
    let (tx, rx) = std::sync::mpsc::channel();
    app.arm_file_ask(rx, import_answer(want));
    tx.send(Some(std::path::PathBuf::from(path))).expect("the chooser's channel is open");
    frame(app, ctx, Vec::new());
    assert!(!app.asking_for_a_file(), "the answer was not picked up by the frame");
}

fn example(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

/// Frames until the background read has landed, with a ceiling.
pub(crate) fn settle(app: &mut App, ctx: &egui::Context) {
    for _ in 0..600 {
        if app.regen.busy.is_none() {
            return;
        }
        frame(app, ctx, Vec::new());
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("still reading after half a minute");
}

/// One line in IGES, in the fixed columns IGES is written in: a drawing, not a single surface in it.
fn iges_line() -> String {
    let g = "1H,,1H;,1Hc,1Hc,1Hc,1Hc,32,38,6,308,15,1Hc,1.,2,2HMM;";
    [
        format!("{:<72}S{:>7}", "a drawing for the check", 1),
        format!("{:<72}G{:>7}", g, 1),
        format!("{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}D{:>7}", 110, 1, 0, 0, 0, 0, 0, 0, "00000000", 1),
        format!("{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}D{:>7}", 110, 0, 0, 1, 0, "", "", "", 0, 2),
        format!("{:<64}{:>8}P{:>7}", "110,0.,0.,0.,10.,0.,0.;", 1, 1),
        format!("S{:>7}G{:>7}D{:>7}P{:>7}{:<40}T{:>7}", 1, 1, 2, 1, "", 1),
    ]
    .join("\n")
        + "\n"
}

/// The folder the scale checks write their files into, under `target`.
fn scale_file(name: &str) -> String {
    let dir = std::path::PathBuf::from(format!("{}/../../target/import-scale", env!("CARGO_MANIFEST_DIR")));
    std::fs::create_dir_all(&dir).expect("a folder for the check");
    dir.join(name).to_string_lossy().into_owned()
}

pub(crate) fn key(k: egui::Key) -> Vec<egui::Event> {
    vec![egui::Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }]
}

pub(crate) fn spot(texts: &[(String, egui::Rect)], label: &str) -> Option<egui::Pos2> {
    texts.iter().find(|(t, _)| t == label).map(|(_, r)| r.center())
}

fn names(texts: &[(String, egui::Rect)]) -> Vec<String> {
    texts.iter().map(|(t, _)| t.clone()).filter(|t| !t.is_empty()).collect()
}

/// The largest side of everything that came into the document after the first `before` bodies.
fn size_after(app: &App, before: usize) -> f64 {
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    for b in app.project.bodies.iter().skip(before) {
        let Some(bb) = b.mesh.bounds() else { continue };
        for (k, (a, z)) in [(bb.min.x, bb.max.x), (bb.min.y, bb.max.y), (bb.min.z, bb.max.z)].into_iter().enumerate() {
            lo[k] = lo[k].min(a);
            hi[k] = hi[k].max(z);
        }
    }
    (0..3).map(|k| hi[k] - lo[k]).fold(0.0, f64::max)
}

/// A cube of `side` as an ASCII STL, the way a program without units writes it.
pub(crate) fn cube_stl(side: f64) -> String {
    let c = |i: usize| [(i & 1) as f64 * side, ((i >> 1) & 1) as f64 * side, ((i >> 2) & 1) as f64 * side];
    let tris = [[0, 2, 1], [1, 2, 3], [4, 5, 6], [5, 7, 6], [0, 1, 4], [1, 5, 4], [2, 6, 3], [3, 6, 7], [0, 4, 2], [2, 4, 6], [1, 3, 5], [3, 7, 5]];
    let mut out = String::from("solid cube\n");
    for t in tris {
        out.push_str("facet normal 0 0 0\nouter loop\n");
        for i in t {
            let p = c(i);
            out.push_str(&format!("vertex {} {} {}\n", p[0], p[1], p[2]));
        }
        out.push_str("endloop\nendfacet\n");
    }
    out + "endsolid cube\n"
}

/// The file is read and the window about its scale is up; returns what the screen shows.
fn read_and_look(app: &mut App, ctx: &egui::Context, path: &str) -> Vec<(String, egui::Rect)> {
    answer(app, ctx, Want::Anything, path);
    settle(app, ctx);
    let _ = frame(app, ctx, Vec::new()); // a window settles on the second pass
    frame(app, ctx, Vec::new())
}

fn wheel(at: egui::Pos2) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(at),
        egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: egui::vec2(0.0, 120.0), phase: egui::TouchPhase::Move, modifiers: Default::default() },
    ]
}

/// A tetrahedron with legs of `side`, as a 3MF in millimetres by its own word.
fn tet_3mf(name: &str, side: f64) -> String {
    let p = scale_file(name);
    let v = |x: f64, y: f64, z: f64| qymcad_core::geom::Point3::new(x * side, y * side, z * side);
    let tet =
        qymcad_core::geom::Mesh { verts: vec![v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(0.0, 1.0, 0.0), v(0.0, 0.0, 1.0)], tris: vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]], ..Default::default() };
    qymcad_io::export_3mf(&[tet], &p).expect("written");
    p
}

/// Ctrl+Z as a hand presses it: the key held with Ctrl in the frame's own state, where the program reads it.
fn undo_by_hand(app: &mut App, ctx: &egui::Context) {
    let command = egui::Modifiers::COMMAND;
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), SCREEN)),
        modifiers: command,
        events: vec![egui::Event::Key { key: egui::Key::Z, physical_key: None, pressed: true, repeat: false, modifiers: command }],
        ..Default::default()
    };
    let _ = ctx.run_ui(raw, |ui| app.draw_frame(ui));
}

/// A cube of 10 mm from STL through the door, kept at the file's numbers; returns its body.
fn stl_cube(app: &mut App, ctx: &egui::Context, name: &str) -> qymcad_core::model::Id {
    let dir = std::path::PathBuf::from(format!("{}/../../target/import-door", env!("CARGO_MANIFEST_DIR")));
    std::fs::create_dir_all(&dir).expect("a folder for the check");
    let p = dir.join(name);
    std::fs::write(&p, cube_stl(10.0)).expect("written");
    answer(app, ctx, Want::Anything, &p.to_string_lossy());
    settle(app, ctx);
    let _ = frame(app, ctx, key(egui::Key::Enter)); // kept at the file's numbers
    let _ = frame(app, ctx, Vec::new());
    app.project.bodies.first().map(|b| b.id).expect("the cube came in")
}

/// Into the part `part` as a person goes in, unless its rows already show; returns where the row of its import's
/// node, `row`, stands.
fn node_row(app: &mut App, ctx: &egui::Context, part: &str, row: &str) -> egui::Pos2 {
    use crate::gui::a_component_stepped_into_is_not_lit::tests::double_click_at;
    let texts = frame(app, ctx, Vec::new());
    if !texts.iter().any(|(t, _)| t.ends_with(row)) {
        let at = spot(&texts, part).unwrap_or_else(|| panic!("no row of the part {part:?}: {:?}", names(&texts)));
        double_click_at(app, ctx, at);
    }
    let texts = frame(app, ctx, Vec::new());
    texts.iter().find(|(t, _)| t.ends_with(row)).map(|(_, r)| r.center()).unwrap_or_else(|| panic!("no row of the import's node {row:?}: {:?}", names(&texts)))
}

/// Into the part `part` as a person goes in, unless its rows already show, and a double click on the row of its
/// import's node, `row`.
fn ask_again(app: &mut App, ctx: &egui::Context, part: &str, row: &str) {
    use crate::gui::a_component_stepped_into_is_not_lit::tests::double_click_at;
    let node = node_row(app, ctx, part, row);
    double_click_at(app, ctx, node);
    let _ = frame(app, ctx, Vec::new()); // a window settles on the second pass
    assert!(app.win.import_scale.is_some(), "a double click on the import's node does not ask about its scale again; status: {}", app.status);
}

/// The side of `body` along x, as it stands in its part.
fn side_of(app: &App, body: qymcad_core::model::Id) -> f64 {
    app.project.mesh_index(body).and_then(|i| app.project.bodies[i].mesh.bounds()).map(|b| b.max.x - b.min.x).unwrap_or(0.0)
}

/// A MESH TURNED INTO A SOLID TAKES A HOLE, the way a person makes one, every step a frame: an STL cube of 10
/// through the door, "Turn into a solid" in its properties, K and a click on the middle of its top face - a sketch
/// on that face; C and two clicks - a circle of radius 3 about the middle, Enter keeping the radius it offers;
/// Ctrl+Enter out of the sketch; its row in the tree, Q, 4 typed into the depth field, Enter, Enter. The part holds
/// 1000 - pi 3^2 4 = 886.90 mm^3: a blind hole 4 deep, not the default 10 (717.26).
fn a_hole_by_hand_in_a_mesh_solid(stem: &str) -> (App, egui::Context) {
    use crate::gui::a_component_stepped_into_is_not_lit::tests::calm;
    use qymcad_core::feature::{FeatureKind, SketchPlane};
    let (mut app, ctx) = running();
    let file = format!("{stem}.stl");
    let src = stl_cube(&mut app, &ctx, &file);
    let texts = frame(&mut app, &ctx, Vec::new());
    let at = spot(&texts, stem).unwrap_or_else(|| panic!("no row of the part: {:?}", names(&texts)));
    crate::gui::a_component_stepped_into_is_not_lit::tests::double_click_at(&mut app, &ctx, at); // into the part
    (0..40).for_each(|_| drop(frame(&mut app, &ctx, Vec::new())));
    app.start_feat_cmd(35); // the recognition tool from the Part's bar
    let texts = frame(&mut app, &ctx, Vec::new());
    // THE MESH AS IT IS - a polyhedron - is the tool's second choice, taken in its bar
    let asis = spot(&texts, &crate::i18n::tr("cmd-recognise-asis")).unwrap_or_else(|| panic!("the bar offers no polyhedron: {:?}", names(&texts)));
    let _ = frame(&mut app, &ctx, click(asis));
    let basis = app.viewing.cam.basis();
    let on = qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect: app.viewing.view_rect, basis: &basis }.at([5.0, 5.0, 10.0]).0;
    let _ = frame(&mut app, &ctx, click(on));
    let _ = frame(&mut app, &ctx, key(egui::Key::Enter));
    calm(&mut app, &ctx);
    let solid = app.project.timeline.iter().find_map(|n| match n.kind {
        FeatureKind::MeshSolid { src: s, body, .. } if s == src => Some(body),
        _ => None,
    });
    let solid = solid.unwrap_or_else(|| panic!("no solid made of the mesh: {}", app.status));

    let _ = frame(&mut app, &ctx, key(egui::Key::K));
    assert!(app.tools.picking.is_sketch_plane(), "K asked for no plane: {}", app.status);
    let _ = frame(&mut app, &ctx, Vec::new());
    let basis = app.viewing.cam.basis();
    let top = qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect: app.viewing.view_rect, basis: &basis }.at([5.0, 5.0, 10.0]).0;
    let _ = frame(&mut app, &ctx, click(top));
    let si = qymcad_ui_state::edit_si(&app.project, &app.sketch_ses).unwrap_or_else(|| panic!("the click on the top face at {top:?} opened no sketch: {}", app.status));
    let plane = app.project.sketches[si].plane;
    assert!(matches!(plane, SketchPlane::Face(b, k) if b == solid && k.normal[2] > 0.9), "the sketch stands on {plane:?}, not on the top face of the solid {solid}");

    let _ = frame(&mut app, &ctx, Vec::new());
    let _ = frame(&mut app, &ctx, key(egui::Key::C));
    let fr = qymcad_ui_state::world_frame_of_plane(&app.draw_ctx(), &plane).expect("the frame of the top face");
    let mid = fr.project(qymcad_core::geom::Point3::new(5.0, 5.0, 10.0));
    let sheet = qymcad_ui_state::Sheet { view: app.viewing.view, rect: app.viewing.view_rect };
    let _ = frame(&mut app, &ctx, click(sheet.at(mid)));
    let _ = frame(&mut app, &ctx, click(sheet.at(qymcad_core::geom::Point2::new(mid.x + 3.0, mid.y))));
    // the drawn circle offers its radius in a field that holds the keyboard; Enter keeps it, and only then does
    // Ctrl+Enter reach the sketch - from inside the field it types nothing and leaves nothing
    let _ = frame(&mut app, &ctx, key(egui::Key::Enter));
    let _ = frame(&mut app, &ctx, Vec::new());
    let drawn = app.project.sketches[si].entities.len();
    assert!(!ctx.egui_wants_keyboard_input(), "Enter left the radius field holding the keyboard: {}", app.status);
    let command = egui::Modifiers::COMMAND;
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), SCREEN)),
        modifiers: command,
        events: vec![egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: command }],
        ..Default::default()
    };
    let _ = ctx.run_ui(raw, |ui| app.draw_frame(ui));
    let _ = frame(&mut app, &ctx, Vec::new());
    assert!(qymcad_ui_state::edit_si(&app.project, &app.sketch_ses).is_none(), "Ctrl+Enter did not leave the sketch of {drawn} entities: {}", app.status);

    let name = crate::i18n::name(&app.project.sketches[si].name);
    let texts = frame(&mut app, &ctx, Vec::new());
    let row = texts.iter().find(|(t, _)| t.ends_with(&name)).map(|(_, r)| r.center()).unwrap_or_else(|| panic!("no row of the sketch {name:?}: {:?}", names(&texts)));
    let _ = frame(&mut app, &ctx, click(row));
    let _ = frame(&mut app, &ctx, key(egui::Key::Q));
    assert!(app.tools.cmd.sketch.is_some(), "Q took no sketch to cut with ({drawn} entities drawn): {}", app.status);
    // the depth is TYPED: the field at the geometry holds its own text and writes it on Enter, so a number put
    // straight into the command's parameters was overwritten by the field's "10" and a check of it saw nothing
    let _ = frame(&mut app, &ctx, Vec::new());
    assert!(ctx.egui_wants_keyboard_input(), "the depth field at the geometry does not take the keyboard: {}", app.status);
    let _ = frame(&mut app, &ctx, vec![egui::Event::Text("4".into())]);
    let _ = frame(&mut app, &ctx, key(egui::Key::Enter)); // keeps the typed depth and lets the keyboard go
    let _ = frame(&mut app, &ctx, key(egui::Key::Enter)); // applies the cut
    calm(&mut app, &ctx);
    let _ = frame(&mut app, &ctx, Vec::new());
    (app, ctx)
}

/// A box with its top left open - the two triangles of the top, whose corners all stand at z = 10, left out -
/// through the door, into its part, recognised by the tool.
fn an_open_box_recognised(stem: &str) -> (App, egui::Context) {
    use crate::gui::a_component_stepped_into_is_not_lit::tests::{calm, double_click_at};
    let (mut app, ctx) = running();
    let dir = std::path::PathBuf::from(format!("{}/../../target/import-door", env!("CARGO_MANIFEST_DIR")));
    std::fs::create_dir_all(&dir).expect("a folder for the check");
    let p = dir.join(format!("{stem}.stl"));
    let closed = cube_stl(10.0);
    // the two triangles of the top (corners 4 to 7 all stand at z = 10) are left out
    let chunks: Vec<&str> = closed.split("facet normal").collect();
    let open: String = chunks.iter().enumerate().filter(|(i, _)| *i != 3 && *i != 4).map(|(i, c)| if i == 0 { c.to_string() } else { format!("facet normal{c}") }).collect();
    std::fs::write(&p, open).expect("written");
    answer(&mut app, &ctx, Want::Anything, &p.to_string_lossy());
    settle(&mut app, &ctx);
    let _ = frame(&mut app, &ctx, key(egui::Key::Enter));
    let texts = frame(&mut app, &ctx, Vec::new());
    let at = spot(&texts, stem).unwrap_or_else(|| panic!("no row of the part: {:?}", names(&texts)));
    double_click_at(&mut app, &ctx, at);
    (0..40).for_each(|_| drop(frame(&mut app, &ctx, Vec::new())));
    app.start_feat_cmd(35);
    let _ = frame(&mut app, &ctx, Vec::new());
    let basis = app.viewing.cam.basis();
    let on = qymcad_ui_state::Screen { cam: &app.viewing.cam, set: &app.set, rect: app.viewing.view_rect, basis: &basis }.at([5.0, 0.0, 5.0]).0;
    let _ = frame(&mut app, &ctx, click(on));
    assert!(app.params.recognise.src.is_some(), "the click took no mesh: {}", app.status);
    let _ = frame(&mut app, &ctx, key(egui::Key::Enter));
    calm(&mut app, &ctx);
    (app, ctx)
}

mod chooser;
mod meshes;
mod asked_again;
mod solids;
