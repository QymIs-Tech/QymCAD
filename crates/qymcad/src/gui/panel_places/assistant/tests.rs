//! AN ASSISTANT WORKS IN THE WINDOW: a caller on another thread, the window running its frames, the steps made in the
//! window's own document - drawn, measured, undoable by name - and a picture of the window handed back.
//!
//! The listening is one per process, so the whole life of it is one check: switched on by the setting, worked, switched
//! off by the setting.
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use qymcad_mcp::link::Window;
use qymcad_mcp::{json, Value};
use qymcad_ui_state::AssistantAccess;

use crate::gui::import_door::tests::{frame, running};

/// What the caller got from each call, in order: the answer, or the refusal.
struct Calls {
    sketch: Result<Value, String>,
    extrude: Result<Value, String>,
    fillet: Result<Value, String>,
    picture: Result<Value, String>,
    undo: Result<Value, String>,
    new_document: Result<Value, String>,
}

/// The caller's side: the calls a model makes, one after another, each built on the answer before.
fn work(mut w: Window) -> Calls {
    let mut call = |tool: &str, args: Value| w.call(tool, &args).unwrap_or_else(|e| Err(format!("the line broke: {e}")));
    let sketch = call("sketch", json!({ "shapes": [{ "kind": "rectangle", "from": [0, 0], "to": [40, 30] }] }));
    let sid = sketch.as_ref().ok().and_then(|s| s["sketch"].as_u64()).unwrap_or_default();
    let extrude = call("extrude", json!({ "sketch": sid, "distance": 10 }));
    let block = extrude.as_ref().ok().and_then(|e| e["bodies"][0]["id"].as_u64()).unwrap_or_default();
    let fillet = call("fillet", json!({ "body": block, "radius": 2 }));
    let picture = call("screenshot", json!({ "view": "iso" }));
    let undo = call("undo", json!({}));
    let new_document = call("new_document", json!({}));
    Calls { sketch, extrude, fillet, picture, undo, new_document }
}

/// A picture the window hands back: what the frame would capture, small and of one colour.
fn handed_back() -> egui::Event {
    let image = egui::ColorImage::new([64, 40], vec![egui::Color32::from_rgb(40, 90, 160); 64 * 40]);
    egui::Event::Screenshot { viewport_id: egui::ViewportId::ROOT, user_data: Default::default(), image: Arc::new(image) }
}

#[test]
fn an_assistant_builds_a_part_in_the_window_step_by_step_and_looks_at_it() {
    // the door is written where this process keeps its files, never in the person's own folder
    let _ = qymcad_paths::keep_under(std::env::temp_dir().join(format!("qymcad-sessions-{}", std::process::id())));
    let (mut app, ctx) = running();
    let mut fails: Vec<String> = Vec::new();

    let _ = frame(&mut app, &ctx, Vec::new());
    assert!(!qymcad_mcp::link::listening(), "the window listens with the setting at its factory value");
    app.set.assistant = AssistantAccess::Welcome;
    let _ = frame(&mut app, &ctx, Vec::new());
    let door = qymcad_mcp::link::own_door().expect("the window listens once the setting lets an assistant in");
    let written = qymcad_mcp::link::read_door().map(|d| d.port);
    if written != Some(door.port) {
        fails.push(format!("the door file says {written:?}, the window listens on {}", door.port));
    }
    // a caller without the token is turned away
    let mut stranger = Window::at(qymcad_mcp::link::Door { port: door.port, token: "guess".into() }).expect("the port answers");
    match stranger.call("describe_document", &json!({})) {
        Ok(Err(_)) => {}
        other => fails.push(format!("a call without the token was taken: {other:?}")),
    }

    let window = Window::at(door).expect("the window answers at its door");
    let done = Arc::new(AtomicBool::new(false));
    let finished = Arc::clone(&done);
    let caller = std::thread::spawn(move || {
        let calls = work(window);
        finished.store(true, Ordering::SeqCst);
        calls
    });
    let undo_before = app.disk.edits.undo.len();
    for _ in 0..3000 {
        if done.load(Ordering::SeqCst) {
            break;
        }
        let _ = frame(&mut app, &ctx, vec![handed_back()]);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let calls = caller.join().expect("the caller ends");

    let volume = |r: &Result<Value, String>| r.as_ref().ok().and_then(|a| a["bodies"][0]["volume_mm3"].as_f64());
    if calls.sketch.as_ref().map(|s| s["contours"].as_array().map_or(0, Vec::len) == 1) != Ok(true) {
        fails.push(format!("the sketch was not drawn in the window: {:?}", calls.sketch));
    }
    match volume(&calls.extrude) {
        Some(v) if (v - 12000.0).abs() < 1.0 => {}
        _ => fails.push(format!("the extrusion was not answered built, 12000 mm^3: {:?}", calls.extrude)),
    }
    match volume(&calls.fillet) {
        Some(v) if v < 12000.0 - 10.0 && v > 11000.0 => {}
        _ => fails.push(format!("the rounded block was not answered rebuilt: {:?}", calls.fillet)),
    }
    match &calls.picture {
        Ok(p) if p["width"] == 64 && p[qymcad_mcp::tools::IMAGE_KEY].as_str().is_some_and(|s| s.starts_with("iVBORw0KGgo")) => {}
        other => fails.push(format!("the picture of the window did not come back as a PNG: {:?}", other.as_ref().map(|p| p["width"].clone()))),
    }
    // the rounding taken back: the block as it was, and the step named
    match (&calls.undo, volume(&calls.undo)) {
        (Ok(u), Some(v)) if (v - 12000.0).abs() < 1.0 && u["undone"][0].as_str().is_some_and(|s| s.contains("fillet")) => {}
        _ => fails.push(format!("undo did not take the rounding back: {:?}", calls.undo)),
    }
    if calls.new_document.is_ok() {
        fails.push("the assistant replaced the person's document in the window".into());
    }
    // the document the person sees holds the part, and every step is one of the person's undo steps, named
    let shown = app.project.bodies.iter().filter(|b| !b.mesh.tris.is_empty() && !app.project.consumed_bodies().contains(&b.id)).count();
    if shown != 1 {
        fails.push(format!("the window shows {shown} bodies, not the one part"));
    }
    let steps: Vec<String> = app.disk.edits.undo[undo_before..].iter().map(|s| s.name.clone()).collect();
    if steps.len() != 2 || !steps[0].contains("sketch") || !steps[1].contains("extrude") {
        fails.push(format!("the sketch and the extrusion are not two undo steps named for them, the rounding taken back: {steps:?}"));
    }

    // switched off, the window stops listening and the door is taken down
    app.set.assistant = AssistantAccess::Off;
    let _ = frame(&mut app, &ctx, Vec::new());
    if qymcad_mcp::link::listening() {
        fails.push("the window still listens with the setting off".into());
    }
    std::thread::sleep(std::time::Duration::from_millis(200));
    if qymcad_mcp::link::read_door().is_some() {
        fails.push("the door file outlived the listening".into());
    }
    assert!(fails.is_empty(), "{}", fails.join("\n"));
}
