//! A PART BUILT THROUGH THE PROTOCOL ALONE, as a client drives the server: lines of JSON in, lines of JSON out, the
//! real kernel behind them. The volumes are measured, not trusted: a node that went red and passed its input through
//! still answers, and only the number tells a 12000 mm^3 block from a 12000 mm^3 block that was meant to be hollow.
use qymcad_mcp::protocol::Server;
use serde_json::{json, Value};

/// A client over one server: it numbers its requests and hands back the `result` of each.
struct Client {
    server: Server,
    next: u64,
}

impl Client {
    fn new() -> Client {
        Client { server: Server::default(), next: 1 }
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let line = json!({ "jsonrpc": "2.0", "id": self.next, "method": method, "params": params }).to_string();
        self.next += 1;
        let answer = self.server.answer(&line).expect("a request is answered");
        let answer: Value = serde_json::from_str(&answer).expect("the answer is JSON");
        assert_eq!(answer["jsonrpc"], "2.0", "the answer is JSON-RPC: {answer}");
        answer
    }

    /// A tool called: its structured answer, or the words of its refusal as `Err`.
    fn call(&mut self, tool: &str, arguments: Value) -> Result<Value, String> {
        let answer = self.request("tools/call", json!({ "name": tool, "arguments": arguments }));
        let result = &answer["result"];
        assert!(result.is_object(), "`{tool}` got no result: {answer}");
        if result["isError"] == true {
            return Err(result["content"][0]["text"].as_str().unwrap_or_default().to_string());
        }
        Ok(result["structuredContent"].clone())
    }

    /// A tool that has to succeed and leave no red node.
    fn must(&mut self, tool: &str, arguments: Value) -> Value {
        let answer = self.call(tool, arguments.clone()).unwrap_or_else(|e| panic!("`{tool}` {arguments} refused: {e}"));
        let failed = answer.get("failed").and_then(Value::as_array).map_or(0, Vec::len);
        assert_eq!(failed, 0, "`{tool}` {arguments} left a red node: {answer}");
        answer
    }
}

/// A body as an answer lists it.
struct Body {
    id: u64,
    volume: f64,
}

/// The one body the document shows.
fn the_body(answer: &Value) -> Body {
    let bodies = answer["bodies"].as_array().expect("the answer lists the bodies");
    assert_eq!(bodies.len(), 1, "a part is one body: {bodies:?}");
    Body { id: bodies[0]["id"].as_u64().expect("an id"), volume: bodies[0]["volume_mm3"].as_f64().expect("a volume") }
}

/// A call that has to be refused, and what is wrong with it.
struct Refusal {
    tool: &'static str,
    args: Value,
    what: &'static str,
}

/// The number a measure should give, and how far off it may be.
struct Want {
    value: f64,
    within: f64,
}

fn near(got: f64, want: Want) -> bool {
    (got - want.value).abs() <= want.within
}

fn temp(name: &str) -> String {
    std::env::temp_dir().join(format!("qymcad-mcp-{}-{name}", std::process::id())).to_string_lossy().into_owned()
}

#[test]
fn the_handshake_names_the_server_and_lists_its_tools() {
    let mut c = Client::new();
    let init = c.request("initialize", json!({ "protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": { "name": "check", "version": "0" } }));
    assert_eq!(init["result"]["protocolVersion"], "2025-03-26", "a revision the server speaks comes back as asked: {init}");
    assert_eq!(init["result"]["serverInfo"]["name"], "qymcad-mcp");
    assert!(init["result"]["capabilities"]["tools"].is_object(), "the server offers tools: {init}");
    assert!(c.server.answer(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).is_none(), "a notification gets no answer");

    let list = c.request("tools/list", json!({}));
    let tools = list["result"]["tools"].as_array().expect("a list of tools");
    let mut fails = Vec::new();
    for t in tools {
        let name = t["name"].as_str().unwrap_or_default();
        if t["description"].as_str().is_none_or(str::is_empty) {
            fails.push(format!("`{name}` says nothing of what it does"));
        }
        if t["inputSchema"]["type"] != "object" {
            fails.push(format!("`{name}` has no object schema"));
        }
    }
    for want in ["sketch", "extrude", "fillet", "describe_body", "save_document", "export"] {
        if !tools.iter().any(|t| t["name"] == want) {
            fails.push(format!("no tool `{want}`"));
        }
    }
    let unknown = c.request("no/such", json!({}));
    if unknown["error"]["code"] != -32601 {
        fails.push(format!("an unknown method is not refused as one: {unknown}"));
    }
    let garbage = c.server.answer("{ not json").expect("a broken line is answered");
    if !garbage.contains("-32700") {
        fails.push(format!("a broken line is not a parse error: {garbage}"));
    }
    assert!(fails.is_empty(), "{}", fails.join("\n"));
}

#[test]
fn a_block_is_rounded_drilled_driven_by_a_parameter_saved_and_exported() {
    let mut c = Client::new();
    let mut fails: Vec<String> = Vec::new();

    // a 40 x 30 rectangle, 10 up: 12000 mm^3
    let sk = c.must("sketch", json!({ "shapes": [{ "kind": "rectangle", "from": [0, 0], "to": [40, 30] }] }));
    let sketch = sk["sketch"].as_u64().expect("the sketch id");
    assert_eq!(sk["contours"].as_array().map_or(0, Vec::len), 1, "one rectangle is one contour: {sk}");
    let ex = c.must("extrude", json!({ "sketch": sketch, "distance": 10 }));
    let extrude = ex["made"].as_u64().expect("the extrusion");
    let Body { id: block, volume: v } = the_body(&ex);
    if !near(v, Want { value: 12000.0, within: 1.0 }) {
        fails.push(format!("the block holds {v} mm^3, not 12000"));
    }

    // its faces and edges, read the way a model reads them
    let b = c.must("describe_body", json!({ "body": block }));
    let faces = b["face_list"].as_array().cloned().unwrap_or_default();
    let edges = b["edge_list"].as_array().cloned().unwrap_or_default();
    if faces.len() != 6 || edges.len() != 12 {
        fails.push(format!("a block has 6 faces and 12 edges, not {} and {}", faces.len(), edges.len()));
    }
    let top = faces.iter().find(|f| f["normal"] == json!([0.0, 0.0, 1.0])).and_then(|f| f["id"].as_u64()).expect("the face looking up");
    let verticals: Vec<u64> = edges.iter().filter(|e| e["direction"][2].as_f64().is_some_and(|z| z.abs() > 0.99)).filter_map(|e| e["id"].as_u64()).collect();
    assert_eq!(verticals.len(), 4, "a block has four upright edges: {edges:?}");

    // the four upright edges rounded, r 5: each corner loses (1 - pi/4) * 25 * 10 mm^3
    let fi = c.must("fillet", json!({ "body": block, "radius": 5, "edges": verticals }));
    let Body { id: rounded, volume: v } = the_body(&fi);
    let want = 12000.0 - 4.0 * (1.0 - std::f64::consts::PI / 4.0) * 25.0 * 10.0;
    if !near(v, Want { value: want, within: 1.0 }) {
        fails.push(format!("the rounded block holds {v} mm^3, not {want:.1}"));
    }

    // a hole dia 8, 6 deep, into the top face: pi * 16 * 6 mm^3 out
    let ho = c.must("hole", json!({ "body": rounded, "face": top, "diameter": 8, "depth": 6 }));
    let v2 = the_body(&ho).volume;
    let hole = std::f64::consts::PI * 16.0 * 6.0;
    if !near(v - v2, Want { value: hole, within: 3.0 }) {
        fails.push(format!("the hole took {} mm^3, not {hole:.1}", v - v2));
    }

    // the height driven by a parameter: everything after the extrusion follows, the hole and the rounds included
    let pa = c.must("set_parameter", json!({ "name": "H", "expression": "20" }));
    if pa["value"] != 20.0 {
        fails.push(format!("the parameter does not hold 20: {pa}"));
    }
    let _ = c.must("set_dimension", json!({ "feature": extrude, "key": "height", "expression": "H" }));
    let doc = c.must("describe_document", json!({}));
    let v3 = the_body(&doc).volume;
    let want = 2.0 * 12000.0 - 4.0 * (1.0 - std::f64::consts::PI / 4.0) * 25.0 * 20.0 - hole;
    if !near(v3, Want { value: want, within: 4.0 }) {
        fails.push(format!("at H = 20 the part holds {v3} mm^3, not {want:.1}"));
    }

    // saved and opened again, the same part
    let file = temp("block.qcad");
    let _ = c.must("save_document", json!({ "path": file }));
    let mut other = Client::new();
    let opened = other.must("open_document", json!({ "path": file }));
    let v4 = the_body(&opened).volume;
    if !near(v4, Want { value: v3, within: 1.0 }) {
        fails.push(format!("opened again the part holds {v4} mm^3, not {v3}"));
    }

    // exported: a STEP the kernel reads back as one solid, and an STL
    let step = temp("block.step");
    let ex = c.must("export", json!({ "path": step }));
    if ex["bodies"] != 1 {
        fails.push(format!("the STEP holds {} bodies, not 1", ex["bodies"]));
    }
    match qymcad_kernel::step_solids(&step) {
        Ok(solids) if solids.len() == 1 => {}
        other => fails.push(format!("the STEP does not read back as one solid: {:?}", other.map(|s| s.len()))),
    }
    let stl = temp("block.stl");
    let _ = c.must("export", json!({ "path": stl }));
    if std::fs::metadata(&stl).map_or(0, |m| m.len()) < 1000 {
        fails.push("the STL is empty".into());
    }
    for f in [file, step, stl] {
        let _ = std::fs::remove_file(f);
    }
    assert!(fails.is_empty(), "{}", fails.join("\n"));
}

#[test]
fn a_ring_is_revolved_and_a_slot_is_cut_through_it() {
    let mut c = Client::new();
    let mut fails: Vec<String> = Vec::new();

    // a 10 x 20 rectangle standing 20 from the sketch's y axis, turned about it: a ring of radii 20 and 30, 20 high
    let sk = c.must("sketch", json!({ "plane": "XZ", "shapes": [{ "kind": "rectangle", "from": [20, 0], "to": [30, 20] }] }));
    let rv = c.must("revolve", json!({ "sketch": sk["sketch"] }));
    let Body { id: ring, volume: v } = the_body(&rv);
    let want = std::f64::consts::PI * (900.0 - 400.0) * 20.0;
    if !near(v, Want { value: want, within: want * 0.002 }) {
        fails.push(format!("the ring holds {v} mm^3, not {want:.1}"));
    }

    // a 4 mm slot across the whole ring, cut through from a sketch on XY
    let slot = c.must("sketch", json!({ "shapes": [{ "kind": "rectangle", "from": [-40, -2], "to": [40, 2] }] }));
    let cut = c.must("extrude", json!({ "sketch": slot["sketch"], "distance": 30, "operation": "cut", "body": ring, "through": true }));
    // a cut right through leaves the pieces as bodies of one part: two halves here
    let halves = cut["bodies"].as_array().cloned().unwrap_or_default();
    if halves.len() != 2 {
        fails.push(format!("a slot across the ring leaves two halves, not {}", halves.len()));
    }
    let v2: f64 = halves.iter().filter_map(|b| b["volume_mm3"].as_f64()).sum();
    // the slot takes two blocks 10 x 4 x 20 out of the wall, near enough on a ring this wide
    if !(v - v2 > 1500.0 && v - v2 < 1700.0) {
        fails.push(format!("the slot took {} mm^3, not about 1600", v - v2));
    }
    if let Some(name) = halves.first().and_then(|b| b["name"].as_str()) {
        if !name.is_ascii() {
            fails.push(format!("the names of the tree are not in English: {name}"));
        }
    }

    // a closed polyline with a circle inside: a plate with a hole in it
    let mut c2 = Client::new();
    let plate = c2.must("sketch", json!({ "shapes": [{ "kind": "polyline", "points": [[0, 0], [50, 0], [50, 20], [0, 20]] }, { "kind": "circle", "centre": [25, 10], "radius": 5 }] }));
    let ex = c2.must("extrude", json!({ "sketch": plate["sketch"], "distance": 2 }));
    let v3 = the_body(&ex).volume;
    let want = (1000.0 - std::f64::consts::PI * 25.0) * 2.0;
    if !near(v3, Want { value: want, within: 1.0 }) {
        fails.push(format!("the plate with a hole holds {v3} mm^3, not {want:.1}"));
    }
    assert!(fails.is_empty(), "{}", fails.join("\n"));
}

#[test]
fn a_call_that_cannot_be_done_says_why_and_changes_nothing() {
    let mut c = Client::new();
    let mut fails: Vec<String> = Vec::new();
    let refusals = [
        Refusal { tool: "extrude", args: json!({ "sketch": 999999, "distance": 10 }), what: "no sketch" },
        Refusal { tool: "fillet", args: json!({ "body": 999999, "radius": 1 }), what: "no body" },
        Refusal { tool: "sketch", args: json!({ "shapes": [{ "kind": "hexagon" }] }), what: "unknown shape" },
        Refusal { tool: "primitive", args: json!({ "shape": "box", "x": 10, "y": -1, "z": 10 }), what: "negative size" },
        Refusal { tool: "set_parameter", args: json!({ "name": "W", "expression": "2*" }), what: "broken expression" },
        Refusal { tool: "save_document", args: json!({}), what: "nowhere to save" },
    ];
    for Refusal { tool, args, what } in refusals {
        match c.call(tool, args) {
            Ok(answer) => fails.push(format!("{what}: `{tool}` was done: {answer}")),
            Err(words) if words.trim().is_empty() => fails.push(format!("{what}: `{tool}` refused without a reason")),
            Err(_) => {}
        }
    }
    let doc = c.must("describe_document", json!({}));
    if doc["parameters"].as_array().is_some_and(|p| !p.is_empty()) {
        fails.push(format!("a parameter that does not evaluate was kept: {}", doc["parameters"]));
    }
    // a body taken up by a fillet is no longer one to act on
    let made = c.must("primitive", json!({ "shape": "box", "x": 10, "y": 10, "z": 10 }));
    let cube = the_body(&made).id;
    let _ = c.must("fillet", json!({ "body": cube, "radius": 1 }));
    if c.call("chamfer", json!({ "body": cube, "distance": 1 })).is_ok() {
        fails.push("a chamfer was laid on a body a fillet already took up".into());
    }
    assert!(fails.is_empty(), "{}", fails.join("\n"));
}
