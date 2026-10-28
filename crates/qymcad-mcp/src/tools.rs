//! THE TOOLS: what a model may do to the document, each with the schema of its arguments and the words that tell a
//! model when to reach for it.
//!
//! Every tool that changes the document rebuilds it before answering, and the answer says what the change made and
//! which nodes failed: a model that cannot see the screen learns of a red node from the answer or not at all.
use qymcad_core::feature::{BasePlane, Purpose, Reach, SketchPlane};
use qymcad_core::model::Id;
use serde_json::{json, Value};

use crate::args::{choice, id, number, number_or, picks, point, size, text, text_or};
use crate::workshop::{View, Workshop};
use crate::reading;

/// A tool as the client lists it.
struct Tool {
    name: &'static str,
    description: &'static str,
    schema: fn() -> Value,
}

/// The schema of an object: its properties and which of them a call must carry.
fn object(properties: Value, required: &[&str]) -> Value {
    json!({ "type": "object", "properties": properties, "required": required })
}

/// The key a picture travels under inside an answer, base64 of a PNG; the protocol turns it into an image block.
pub const IMAGE_KEY: &str = "image_png";

const TOOLS: [Tool; 20] = [
    Tool {
        name: "undo",
        description: "Take the last steps back, as Ctrl+Z does in the window - the assistant's or the person's, newest first. Use it when a step came out wrong, rather than building over it. Needs a window.",
        schema: || object(json!({ "count": { "type": "integer", "description": "How many steps; one when left out." } }), &[]),
    },
    Tool {
        name: "screenshot",
        description: "A picture of the QymCAD window as the person sees it: the model in the view, the tree, the parameters. Look at it after the steps that shape the part and compare it with what was asked for - the numbers say what was built, the picture says whether it is the right thing. Needs a window.",
        schema: || object(json!({ "view": { "type": "string", "enum": View::WORDS, "description": "Turn the camera to this view and frame the model first; the view stays as it is when left out." } }), &[]),
    },
    Tool {
        name: "view",
        description: "Turn the camera of the window to a standard view and frame the whole model, for the person and for the next `screenshot`. Needs a window.",
        schema: || object(json!({ "view": { "type": "string", "enum": View::WORDS } }), &["view"]),
    },
    Tool { name: "new_document", description: "Start a new empty document with one part, dropping the one in hand unsaved.", schema: || object(json!({}), &[]) },
    Tool {
        name: "open_document",
        description: "Open a .qcad file and rebuild it. Answers with the document as `describe_document` gives it.",
        schema: || object(json!({ "path": { "type": "string", "description": "Path of the .qcad file." } }), &["path"]),
    },
    Tool {
        name: "save_document",
        description: "Save the document as .qcad, the file the QymCAD window opens. Without `path`, saves where it was opened or last saved.",
        schema: || object(json!({ "path": { "type": "string", "description": "Path of the .qcad file to write." } }), &[]),
    },
    Tool {
        name: "describe_document",
        description: "The whole document: parts, sketches with their contours, the timeline of features with any that failed, the bodies with volume and bounding box, the parameters.",
        schema: || object(json!({}), &[]),
    },
    Tool {
        name: "describe_body",
        description: "The faces and edges of one body by persistent id, with centre and normal of every face and the ends, middle and direction of every edge (radius and axis for a round one). Read it before `fillet`, `chamfer`, `shell` or `hole` to pick what they act on.",
        schema: || object(json!({ "body": { "type": "integer", "description": "Body id from `describe_document`." } }), &["body"]),
    },
    Tool {
        name: "sketch",
        description: "Draw a sketch on a base plane of the active part: rectangles, circles and polylines in the plane's own x and y. Closed shapes become contours an `extrude` or `revolve` takes. A circle inside a rectangle makes a contour with a hole.",
        schema: || {
            object(
                json!({
                    "plane": { "type": "string", "enum": ["XY", "XZ", "YZ"], "description": "Base plane; XY when left out." },
                    "name": { "type": "string" },
                    "shapes": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "kind": { "type": "string", "enum": ["rectangle", "circle", "polyline"] },
                                "from": { "type": "array", "items": { "type": "number" }, "description": "Rectangle: one corner [x, y]." },
                                "to": { "type": "array", "items": { "type": "number" }, "description": "Rectangle: the opposite corner [x, y]." },
                                "centre": { "type": "array", "items": { "type": "number" }, "description": "Circle: centre [x, y]." },
                                "radius": { "type": "number", "description": "Circle: radius." },
                                "points": { "type": "array", "items": { "type": "array", "items": { "type": "number" } }, "description": "Polyline: points [[x, y], ...]." },
                                "closed": { "type": "boolean", "description": "Polyline: join the last point to the first. True when left out." }
                            },
                            "required": ["kind"]
                        }
                    }
                }),
                &["shapes"],
            )
        },
    },
    Tool {
        name: "extrude",
        description: "Extrude every closed contour of a sketch. `add` grows the part's body (the first solid of a part starts it); `cut` and `intersect` act on `body`.",
        schema: || {
            object(
                json!({
                    "sketch": { "type": "integer", "description": "Sketch id." },
                    "distance": { "type": "number", "description": "How far, mm." },
                    "direction": { "type": "string", "enum": ["forward", "backward", "both"], "description": "Along the plane normal, against it, or half each way. Forward when left out." },
                    "operation": { "type": "string", "enum": ["add", "cut", "intersect"], "description": "Add when left out." },
                    "body": { "type": "integer", "description": "Body a cut or an intersection acts on." },
                    "through": { "type": "boolean", "description": "Cut: go through everything, ignoring the distance." }
                }),
                &["sketch", "distance"],
            )
        },
    },
    Tool {
        name: "revolve",
        description: "Revolve every closed contour of a sketch about the sketch's own x or y axis, adding to the part's body.",
        schema: || {
            object(
                json!({
                    "sketch": { "type": "integer" },
                    "axis": { "type": "string", "enum": ["x", "y"], "description": "Axis of the sketch plane; y when left out." },
                    "angle": { "type": "number", "description": "Degrees; 360 when left out." }
                }),
                &["sketch"],
            )
        },
    },
    Tool {
        name: "primitive",
        description: "Add a box, cylinder, sphere, cone or torus at the origin of the active part, adding to the part's body.",
        schema: || {
            object(
                json!({
                    "shape": { "type": "string", "enum": ["box", "cylinder", "sphere", "cone", "torus"] },
                    "x": { "type": "number", "description": "Box: size along X." },
                    "y": { "type": "number", "description": "Box: size along Y." },
                    "z": { "type": "number", "description": "Box: size along Z." },
                    "radius": { "type": "number", "description": "Cylinder, sphere: radius. Cone: bottom radius. Torus: radius of the ring." },
                    "top_radius": { "type": "number", "description": "Cone: top radius, zero for a point." },
                    "tube_radius": { "type": "number", "description": "Torus: radius of the tube." },
                    "height": { "type": "number", "description": "Cylinder, cone: height along Z." }
                }),
                &["shape"],
            )
        },
    },
    Tool {
        name: "fillet",
        description: "Round edges of a body. Edge ids come from `describe_body`; no ids rounds every edge.",
        schema: || object(json!({ "body": { "type": "integer" }, "radius": { "type": "number" }, "edges": { "type": "array", "items": { "type": "integer" } } }), &["body", "radius"]),
    },
    Tool {
        name: "chamfer",
        description: "Bevel edges of a body by a distance. Edge ids come from `describe_body`; no ids bevels every edge.",
        schema: || object(json!({ "body": { "type": "integer" }, "distance": { "type": "number" }, "edges": { "type": "array", "items": { "type": "integer" } } }), &["body", "distance"]),
    },
    Tool {
        name: "shell",
        description: "Hollow a body to a wall thickness, the listed faces removed to open it. Face ids come from `describe_body`.",
        schema: || {
            object(
                json!({
                    "body": { "type": "integer" },
                    "thickness": { "type": "number" },
                    "faces": { "type": "array", "items": { "type": "integer" }, "description": "Faces taken away; none leaves a closed hollow." },
                    "side": { "type": "string", "enum": ["inward", "outward", "centred"], "description": "Where the wall goes from the surface; inward when left out." }
                }),
                &["body", "thickness"],
            )
        },
    },
    Tool {
        name: "hole",
        description: "Drill a plain hole into a planar face, at the face's centre, along its inward normal. Face id from `describe_body`.",
        schema: || object(json!({ "body": { "type": "integer" }, "face": { "type": "integer" }, "diameter": { "type": "number" }, "depth": { "type": "number" } }), &["body", "face", "diameter", "depth"]),
    },
    Tool {
        name: "boolean",
        description: "Combine two bodies: cut the tool out of the target, unite them, or keep what they share. Both are taken up into the result.",
        schema: || {
            object(json!({ "target": { "type": "integer" }, "tool": { "type": "integer" }, "operation": { "type": "string", "enum": ["cut", "union", "intersect"] } }), &["target", "tool", "operation"])
        },
    },
    Tool {
        name: "set_parameter",
        description: "Create or change a document parameter (`50`, `w/2`, `2*pi*r`). Every feature whose dimension refers to it rebuilds.",
        schema: || object(json!({ "name": { "type": "string" }, "expression": { "type": "string" } }), &["name", "expression"]),
    },
    Tool {
        name: "set_dimension",
        description: "Drive one dimension of a feature (`height`, `radius`, `thickness`, `angle`, `dist`, `diameter`, `depth`) by an expression, which may name parameters; an empty expression goes back to the stored number.",
        schema: || {
            object(json!({ "feature": { "type": "integer", "description": "Feature id from `describe_document`." }, "key": { "type": "string" }, "expression": { "type": "string" } }), &["feature", "key", "expression"])
        },
    },
    Tool {
        name: "export",
        description: "Write every visible body into one file: STEP (exact B-rep) or STL (mesh), chosen by `format` or by the extension.",
        schema: || object(json!({ "path": { "type": "string" }, "format": { "type": "string", "enum": ["step", "stl"] } }), &["path"]),
    },
];

/// The tools as `tools/list` lists them.
pub fn catalogue() -> Vec<Value> {
    TOOLS.iter().map(|t| json!({ "name": t.name, "description": t.description, "inputSchema": (t.schema)() })).collect()
}

/// Is there a tool by this name.
pub fn known(name: &str) -> bool {
    TOOLS.iter().any(|t| t.name == name)
}

/// WHAT A TOOL CAME TO.
pub enum Done {
    /// An answer complete as it stands: a reading, a file written.
    Answer(Value),
    /// A change of the document, answered once it is rebuilt: at once with no window, when the window's own rebuild
    /// has finished in one (`settle`).
    Changed(Change),
    /// A picture of the window, answered once the window has drawn the frame it was asked in and handed it back.
    Picture,
}

/// A change made: what it made, and what else its answer says.
pub struct Change {
    pub made: Id,
    /// More of the answer, laid beside `made`: the contours of a sketch, the value of a parameter.
    pub extra: Value,
}

impl Change {
    fn of(made: Id) -> Done {
        Done::Changed(Change { made, extra: json!({}) })
    }
}

/// The answer a tool gives, read off the document as it stands now - for a change, the document rebuilt.
pub fn settle(ws: &dyn Workshop, done: Done) -> Value {
    match done {
        Done::Answer(v) => v,
        // a picture is the window's to hand back; with none to wait for there is only the document to say
        Done::Picture => json!({ "where": ws.place() }),
        Done::Changed(c) => {
            let mut answer = if c.extra.is_object() { c.extra } else { json!({}) };
            answer["made"] = json!(c.made);
            answer["failed"] = failures(ws);
            answer["bodies"] = reading::document(ws)["bodies"].clone();
            answer
        }
    }
}

/// Run one tool: what it came to, or the reason it refused. A refusal changes nothing: every argument is read before
/// the operation opens, so a window is never left with half a step on its undo stack.
pub fn run(ws: &mut dyn Workshop, name: &str, args: &Value) -> Result<Done, String> {
    match name {
        "new_document" => {
            ws.new_document()?;
            Ok(Done::Answer(reading::document(ws)))
        }
        "open_document" => {
            ws.open(text(args, "path")?)?;
            let mut answer = reading::document(ws);
            answer["failed"] = failures(ws);
            Ok(Done::Answer(answer))
        }
        "save_document" => {
            let path = match args.get("path") {
                Some(_) => text(args, "path")?.to_string(),
                None => ws.path().ok_or("the document has never been saved; give `path`")?,
            };
            ws.save(&path)?;
            Ok(Done::Answer(json!({ "saved": path })))
        }
        "describe_document" => Ok(Done::Answer(reading::document(ws))),
        "undo" => {
            let count = number_or(args, "count", 1.0)?;
            if !(1.0..=100.0).contains(&count) {
                return Err(format!("`count` is from 1 to 100, not {count}"));
            }
            let mut undone = Vec::new();
            for _ in 0..count as usize {
                match ws.undo() {
                    Ok(step) => undone.push(step),
                    Err(why) if undone.is_empty() => return Err(why),
                    Err(_) => break,
                }
            }
            Ok(Done::Changed(Change { made: 0, extra: json!({ "undone": undone }) }))
        }
        "view" => {
            let word = choice(args, "view", &View::WORDS)?;
            ws.look(View::of_word(word).ok_or("an unknown view")?)?;
            Ok(Done::Answer(json!({ "view": word })))
        }
        "screenshot" => {
            if ws.place() != "window" {
                return Err(crate::workshop::NO_WINDOW.into());
            }
            if args.get("view").is_some() {
                let view = View::of_word(choice(args, "view", &View::WORDS)?).ok_or("an unknown view")?;
                ws.look(view)?;
            }
            Ok(Done::Picture)
        }
        "describe_body" => Ok(Done::Answer(reading::body(ws, id(args, "body")?)?)),
        "sketch" => sketch(ws, args),
        "extrude" => extrude(ws, args),
        "revolve" => revolve(ws, args),
        "primitive" => primitive(ws, args),
        "fillet" => {
            let body = live_body(ws, args, "body")?;
            let (radius, edges) = (size(args, "radius")?, picks(args, "edges")?);
            ws.begin(name);
            let part = ws.project().body_owner(body);
            enter(ws, part);
            let made = ws.project_mut().add_fillet(body, radius, edges);
            ws.finish();
            Ok(Change::of(made))
        }
        "chamfer" => {
            let body = live_body(ws, args, "body")?;
            let (distance, edges) = (size(args, "distance")?, picks(args, "edges")?);
            ws.begin(name);
            let part = ws.project().body_owner(body);
            enter(ws, part);
            let made = ws.project_mut().add_chamfer(body, distance, edges);
            ws.finish();
            Ok(Change::of(made))
        }
        "shell" => shell(ws, args),
        "hole" => hole(ws, args),
        "boolean" => {
            let target = live_body(ws, args, "target")?;
            let tool = live_body(ws, args, "tool")?;
            let op = match choice(args, "operation", &["cut", "union", "intersect"])? {
                "cut" => 0,
                "union" => 1,
                _ => 2,
            };
            ws.begin(name);
            let part = ws.project().body_owner(target);
            enter(ws, part);
            let made = ws.project_mut().add_body_boolean(target, tool, op);
            ws.finish();
            Ok(Change::of(made))
        }
        "set_parameter" => set_parameter(ws, args),
        "set_dimension" => {
            let feature = id(args, "feature")?;
            if !ws.project().timeline.iter().any(|n| n.id == feature) {
                return Err(format!("there is no feature {feature}; `describe_document` lists the features"));
            }
            let (key, expression) = (text(args, "key")?, text(args, "expression")?);
            if !expression.trim().is_empty() {
                ws.project().eval_expr(expression).map_err(|e| format!("`{expression}` does not evaluate: {e:?}"))?;
            }
            ws.begin(name);
            ws.project_mut().set_feat_dim(feature, key, expression.to_string());
            ws.finish();
            Ok(Change::of(feature))
        }
        "export" => export(ws, args),
        other => Err(format!("no tool `{other}`")),
    }
}

/// THE PART A STEP GOES INTO, made the active one for the step. New nodes go into the active context, and in a window
/// that context is where the person stands, said again every frame from the window's own path: a step that left it
/// as it found it put an extrusion into the root assembly, which has no body to build - the window showed nothing and
/// the answer listed no body. So a step names its part itself, as a tool does by stepping into the part it works on.
fn enter(ws: &mut dyn Workshop, part: Option<Id>) {
    if let Some(part) = part.filter(|p| ws.project().component_is_part(*p)) {
        ws.project_mut().set_active_component(Some(part));
    }
}

/// THE PART A NEW SKETCH OR PRIMITIVE GOES INTO: the one the person stands in, else the last part of the document,
/// else a new one - never a new part for every sketch of a person who stands in the assembly.
fn drawing_part(p: &mut qymcad_core::model::Project) {
    let ctx = p.active_ctx();
    if p.component_is_part(ctx) {
        return;
    }
    let last = p.components.iter().rev().map(|c| c.id).find(|id| p.component_is_part(*id));
    match last {
        Some(part) => p.set_active_component(Some(part)),
        None => {
            let _ = p.part_to_draw_in();
        }
    }
}

/// The nodes that do not build now, as the answer lists them.
fn failures(ws: &dyn Workshop) -> Value {
    json!(ws.failed().iter().map(|f| json!({ "feature": f.node, "reason": f.reason })).collect::<Vec<_>>())
}

/// A body an operation acts on: it has to stand in the document and not be taken up by another one already - an
/// operation on a consumed body branches the history and its result hides nothing.
fn live_body(ws: &dyn Workshop, args: &Value, key: &str) -> Result<Id, String> {
    let body = id(args, key)?;
    let p = ws.project();
    if !p.bodies.iter().any(|b| b.id == body) {
        return Err(format!("there is no body {body}; `describe_document` lists the bodies"));
    }
    if p.consumed_bodies().contains(&body) {
        return Err(format!("body {body} has been taken up into another body; act on the one `describe_document` lists now"));
    }
    Ok(body)
}

/// A shape of a sketch, read and checked before anything is drawn.
enum Drawn {
    Rectangle {
        from: qymcad_core::geom::Point2,
        to: qymcad_core::geom::Point2,
    },
    Circle {
        centre: qymcad_core::geom::Point2,
        radius: f64,
    },
    /// The segments of a polyline, each from one point to the next.
    Segments(Vec<Segment>),
}

/// One straight piece of a polyline.
struct Segment {
    from: qymcad_core::geom::Point2,
    to: qymcad_core::geom::Point2,
}

fn drawn(shape: &Value) -> Result<Drawn, String> {
    match text(shape, "kind")? {
        "rectangle" => Ok(Drawn::Rectangle { from: point(shape.get("from").unwrap_or(&Value::Null), "from")?, to: point(shape.get("to").unwrap_or(&Value::Null), "to")? }),
        "circle" => Ok(Drawn::Circle { centre: point(shape.get("centre").unwrap_or(&Value::Null), "centre")?, radius: size(shape, "radius")? }),
        "polyline" => {
            let Some(list) = shape.get("points").and_then(Value::as_array) else { return Err("a polyline carries `points`".into()) };
            let pts = list.iter().map(|q| point(q, "points")).collect::<Result<Vec<_>, String>>()?;
            if pts.len() < 2 {
                return Err("a polyline has at least two points".into());
            }
            let closed = shape.get("closed").and_then(Value::as_bool).unwrap_or(true);
            let ends = if closed && pts.len() > 2 { pts.len() } else { pts.len() - 1 };
            Ok(Drawn::Segments((0..ends).map(|k| Segment { from: pts[k], to: pts[(k + 1) % pts.len()] }).collect()))
        }
        other => Err(format!("a shape is a rectangle, a circle or a polyline, not `{other}`")),
    }
}

/// Draw a sketch the way the window starts one: a sketch in the part being worked in, its node on the timeline, the
/// shapes laid in, the contours found.
fn sketch(ws: &mut dyn Workshop, args: &Value) -> Result<Done, String> {
    let plane = match choice(args, "plane", &["XY", "XZ", "YZ"])? {
        "XY" => BasePlane::XY,
        "XZ" => BasePlane::XZ,
        _ => BasePlane::YZ,
    };
    let Some(list) = args.get("shapes").and_then(Value::as_array) else { return Err("`shapes` is a list of rectangles, circles and polylines".into()) };
    let shapes = list.iter().map(drawn).collect::<Result<Vec<_>, String>>()?;
    let name = text_or(args, "name", "")?.to_string();
    ws.begin("sketch");
    let p = ws.project_mut();
    drawing_part(p);
    let name = if name.is_empty() { format!("name-sketch-n#{}", p.sketches.len() + 1) } else { name };
    let si = p.new_sketch(name.clone());
    p.sketches[si].plane = SketchPlane::World(plane);
    let sid = p.sketches[si].id;
    p.add_sketch_node(sid, name);
    for shape in shapes {
        match shape {
            Drawn::Rectangle { from, to } => {
                let _ = p.add_rect_entity(si, from.x, from.y, to.x, to.y, Purpose::Real);
            }
            Drawn::Circle { centre, radius } => {
                let _ = p.add_circle_entity(si, centre.x, centre.y, radius, Purpose::Real);
            }
            Drawn::Segments(segments) => {
                for s in segments {
                    let _ = p.add_line_entity(si, s.from.x, s.from.y, s.to.x, s.to.y, Purpose::Real);
                }
            }
        }
    }
    p.regen_sketch(si);
    let contours = p.sketches[si].contour_ids.clone();
    ws.finish();
    Ok(Done::Changed(Change { made: sid, extra: json!({ "sketch": sid, "contours": contours }) }))
}

/// THE OUTER CONTOURS OF A SKETCH, the ones no larger contour holds, refused when there is none: an extrude of nothing
/// is a red node with no reason a model could act on.
///
/// Only the outer ones are handed over, as a click inside a plate and outside its circle hands over the plate: a
/// contour named as a profile is filled even inside another one, so naming them all made a 50 x 20 plate with a
/// circle in it a solid 2000 mm^3 instead of a plate with a hole, 1843 mm^3. Left unnamed, a nested contour is a
/// hole, which is what a circle drawn inside a rectangle means.
fn contours_of(ws: &dyn Workshop, sketch: Id) -> Result<Vec<Id>, String> {
    let p = ws.project();
    let si = p.sketch_index(sketch).ok_or_else(|| format!("there is no sketch {sketch}; `describe_document` lists the sketches"))?;
    let closed: Vec<&qymcad_core::geom::Contour> =
        p.sketches[si].contour_ids.iter().filter_map(|cid| p.contour_index(*cid).and_then(|ci| p.contours.get(ci))).filter(|c| c.closed && c.points.len() >= 3).collect();
    let held = |c: &qymcad_core::geom::Contour| closed.iter().any(|d| d.area() > c.area() && d.contains(c.points[0]));
    let outer: Vec<Id> =
        p.sketches[si].contour_ids.iter().copied().filter(|cid| p.contour_index(*cid).and_then(|ci| p.contours.get(ci)).is_some_and(|c| c.closed && c.points.len() >= 3 && !held(c))).collect();
    if outer.is_empty() {
        return Err(format!("sketch {sketch} has no closed contour to extrude"));
    }
    Ok(outer)
}

fn extrude(ws: &mut dyn Workshop, args: &Value) -> Result<Done, String> {
    let sketch = id(args, "sketch")?;
    let distance = size(args, "distance")?;
    let reach = match choice(args, "direction", &["forward", "backward", "both"])? {
        "forward" => Reach::Forward,
        "backward" => Reach::Backward,
        _ => Reach::BothWays,
    };
    let contours = contours_of(ws, sketch)?;
    let op = choice(args, "operation", &["add", "cut", "intersect"])?;
    let target = if op == "add" { None } else { Some(live_body(ws, args, "body")?) };
    let through = args.get("through").and_then(Value::as_bool).unwrap_or_default();
    ws.begin("extrude");
    let part = match target {
        Some(body) => ws.project().body_owner(body),
        None => ws.project().sketch_owner(sketch),
    };
    enter(ws, part);
    let p = ws.project_mut();
    let made = match target {
        None => {
            let e = p.add_extrude_multi(sketch, contours, distance, reach, 0.0, Vec::new());
            // a part is one body: the extrusion is united into the body the part already has
            p.finish_base_body(e, 1)
        }
        Some(body) => {
            let span = qymcad_core::model::CombineSpan { height: distance, down: 0.0, extent: qymcad_core::feature::Extent { through, reach }, fill: &[] };
            p.add_combine_multi_op(body, sketch, contours, span, if op == "cut" { 0 } else { 2 })
        }
    };
    ws.finish();
    Ok(Change::of(made))
}

fn revolve(ws: &mut dyn Workshop, args: &Value) -> Result<Done, String> {
    let sketch = id(args, "sketch")?;
    let axis = if choice(args, "axis", &["y", "x"])? == "x" { 0 } else { 1 };
    let angle = number_or(args, "angle", 360.0)?;
    if angle <= 0.0 || angle > 360.0 {
        return Err(format!("`angle` is above 0 and at most 360, not {angle}"));
    }
    let contours = contours_of(ws, sketch)?;
    ws.begin("revolve");
    let part = ws.project().sketch_owner(sketch);
    enter(ws, part);
    let p = ws.project_mut();
    let r = p.add_revolve_axis(sketch, contours, axis, angle, 0, 0);
    let made = p.finish_base_body(r, 1);
    ws.finish();
    Ok(Change::of(made))
}

/// The size of a primitive, read before it is made.
enum Solid {
    Box { x: f64, y: f64, z: f64 },
    Cylinder { radius: f64, height: f64 },
    Sphere { radius: f64 },
    Cone { radius: f64, top: f64, height: f64 },
    Torus { ring: f64, tube: f64 },
}

fn primitive(ws: &mut dyn Workshop, args: &Value) -> Result<Done, String> {
    let solid = match choice(args, "shape", &["box", "cylinder", "sphere", "cone", "torus"])? {
        "box" => Solid::Box { x: size(args, "x")?, y: size(args, "y")?, z: size(args, "z")? },
        "cylinder" => Solid::Cylinder { radius: size(args, "radius")?, height: size(args, "height")? },
        "sphere" => Solid::Sphere { radius: size(args, "radius")? },
        "cone" => Solid::Cone { radius: size(args, "radius")?, top: number_or(args, "top_radius", 0.0)?.max(0.0), height: size(args, "height")? },
        _ => {
            let (ring, tube) = (size(args, "radius")?, size(args, "tube_radius")?);
            if tube >= ring {
                return Err(format!("`tube_radius` {tube} is below `radius` {ring}, or the torus crosses itself"));
            }
            Solid::Torus { ring, tube }
        }
    };
    ws.begin("primitive");
    let p = ws.project_mut();
    drawing_part(p);
    let made = match solid {
        Solid::Box { x, y, z } => p.add_box(x, y, z),
        Solid::Cylinder { radius, height } => p.add_cylinder(radius, height),
        Solid::Sphere { radius } => p.add_sphere(radius),
        Solid::Cone { radius, top, height } => p.add_cone(radius, top, height),
        Solid::Torus { ring, tube } => p.add_torus(ring, tube),
    };
    // a part is one body, as for an extrusion
    let made = p.finish_base_body(made, 1);
    ws.finish();
    Ok(Change::of(made))
}

fn shell(ws: &mut dyn Workshop, args: &Value) -> Result<Done, String> {
    use qymcad_core::feature::ShellSide;
    let body = live_body(ws, args, "body")?;
    let side = match choice(args, "side", &["inward", "outward", "centred"])? {
        "inward" => ShellSide::Inward,
        "outward" => ShellSide::Outward,
        _ => ShellSide::Centred,
    };
    let (thickness, faces) = (size(args, "thickness")?, picks(args, "faces")?);
    ws.begin("shell");
    let part = ws.project().body_owner(body);
    enter(ws, part);
    let made = ws.project_mut().add_shell_mode(body, thickness, faces, side);
    ws.finish();
    Ok(Change::of(made))
}

/// A hole on a face: the face named by its persistent id and witnessed by where it stood, so the reference survives
/// a rebuild above it as one made by a click does.
fn hole(ws: &mut dyn Workshop, args: &Value) -> Result<Done, String> {
    let body = live_body(ws, args, "body")?;
    let face = number(args, "face")? as u32;
    let faces = ws.project().regen_faces.get(&body).cloned().unwrap_or_default();
    let Some((index, f)) = faces.iter().enumerate().find(|(_, f)| f.id == face) else { return Err(format!("body {body} has no face {face}; `describe_body` lists its faces")) };
    let key = qymcad_core::feature::FaceKey { index: index as u32, centroid: [f.centroid.x, f.centroid.y, f.centroid.z], normal: f.normal, id: f.id };
    let (diameter, depth) = (size(args, "diameter")?, size(args, "depth")?);
    ws.begin("hole");
    let part = ws.project().body_owner(body);
    enter(ws, part);
    let made = ws.project_mut().add_hole(body, key, diameter, depth);
    ws.finish();
    Ok(Change::of(made))
}

fn set_parameter(ws: &mut dyn Workshop, args: &Value) -> Result<Done, String> {
    let name = text(args, "name")?.trim().to_string();
    let expression = text(args, "expression")?.to_string();
    if name.is_empty() {
        return Err("a parameter has a name".into());
    }
    // the formula is tried against the parameters as they stand before any step opens
    ws.project().eval_expr(&expression).map_err(|e| format!("`{expression}` does not evaluate: {e:?}"))?;
    ws.begin("set_parameter");
    let p = ws.project_mut();
    let before = p.parameters.iter().position(|q| q.name == name);
    let old = before.map(|i| p.parameters[i].expr.clone());
    match before {
        Some(i) => p.parameters[i].expr = expression.clone(),
        None => p.parameters.push(qymcad_core::model::Param { name: name.clone(), expr: expression.clone(), value: 0.0 }),
    }
    let refused = p.eval_parameters().into_iter().find(|r| r.0 == name);
    if let Some(r) = refused {
        // a formula that evaluates alone and still fails among the rest closes a circle: the parameter is put back
        match (before, old) {
            (Some(i), Some(expr)) => p.parameters[i].expr = expr,
            _ => p.parameters.retain(|q| q.name != name),
        }
        let _ = p.eval_parameters();
        ws.finish();
        return Err(format!("`{expression}` does not evaluate among the parameters: {:?}", r.1));
    }
    p.mark_param_dependents_dirty_for(&name);
    let value = p.parameters.iter().find(|q| q.name == name).map(|q| q.value);
    ws.finish();
    Ok(Done::Changed(Change { made: 0, extra: json!({ "parameter": name, "value": value }) }))
}

fn export(ws: &mut dyn Workshop, args: &Value) -> Result<Done, String> {
    let path = text(args, "path")?;
    let by_extension = if path.to_ascii_lowercase().ends_with(".stl") { "stl" } else { "step" };
    let format = match args.get("format") {
        Some(_) => choice(args, "format", &["step", "stl"])?,
        None => by_extension,
    };
    let bodies = if format == "stl" { ws.export_stl(path)? } else { ws.export_step(path)? };
    Ok(Done::Answer(json!({ "written": path, "format": format, "bodies": bodies })))
}
