//! THE PROTOCOL: JSON-RPC 2.0 messages of the Model Context Protocol, one per line.
//!
//! Four methods are answered - `initialize`, `ping`, `tools/list`, `tools/call` - and every notification is taken in
//! silence, as the protocol asks. A tool that refuses is not a protocol error: it answers with `isError` and the
//! reason in words, so the model reads why and tries again, rather than the client dropping the call.
use serde_json::{json, Value};

use crate::bench::Bench;

/// The revisions of the protocol this server speaks, newest first. A client asking for one of them gets it back;
/// any other request is answered with the newest, and the client decides whether it can go on.
pub const PROTOCOL_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

/// JSON-RPC: the line was not JSON.
const PARSE_ERROR: i64 = -32700;
/// JSON-RPC: the line was JSON but not a request.
const INVALID_REQUEST: i64 = -32600;
/// JSON-RPC: no such method.
const METHOD_NOT_FOUND: i64 = -32601;
/// JSON-RPC: the parameters do not fit the method.
const INVALID_PARAMS: i64 = -32602;

/// What the server tells a client about itself at the handshake, for the model to read before the first call.
const INSTRUCTIONS: &str = "Builds and edits one QymCAD document. Lengths are millimetres, angles degrees. \
A part is built from sketches (`sketch`, on XY, XZ or YZ) turned into solids (`extrude`, `revolve`) or from \
primitives (`primitive`), then finished with `fillet`, `chamfer`, `shell`, `hole`, `boolean`. Faces and edges are \
picked by the ids `describe_body` lists, never guessed. Every change rebuilds the model and answers with what \
failed; `describe_document` shows the whole tree. When a QymCAD window takes an assistant, every step is made in \
that window, where the person watches it and can undo it; `screenshot` shows what the window shows - look at it \
after the steps that shape the part, and compare it with what was asked for. `view` turns the camera first. \
`save_document` writes a .qcad; `export` writes STEP or STL.";

/// A protocol error: the code JSON-RPC defines and the words for it.
struct Fault {
    code: i64,
    message: String,
}

/// WHERE THE TOOLS GO: to the document this process holds, or to a running window as well when one listens.
#[derive(Default, PartialEq)]
pub enum Reach {
    /// The document of this process alone, whatever windows are open: what a check needs, so a window someone has
    /// open on the machine never takes its calls.
    #[default]
    ThisProcess,
    /// A QymCAD window that takes an assistant, when one is listening; this process's own document otherwise.
    Windows,
}

/// The server: the protocol in front of the document it works on.
#[derive(Default)]
pub struct Server {
    bench: Bench,
    reach: Reach,
    /// The window the calls go to, once one was found.
    window: Option<crate::link::Window>,
}

impl Server {
    /// A server that works in a running window whenever one takes an assistant, and in a document of its own otherwise.
    pub fn reaching_windows() -> Server {
        Server { reach: Reach::Windows, ..Server::default() }
    }

    /// Run a tool where it belongs: in the window when one is reached, here otherwise.
    fn work(&mut self, name: &str, args: &Value) -> Result<Value, String> {
        if self.window.is_none() && self.reach == Reach::Windows {
            self.window = crate::link::Window::find();
        }
        if let Some(w) = self.window.as_mut() {
            return match w.call(name, args) {
                Ok(outcome) => outcome,
                Err(_) => {
                    self.window = None;
                    Err("the QymCAD window went away in the middle of the call; call again - in the window once it listens again, or without one".into())
                }
            };
        }
        let done = crate::tools::run(&mut self.bench, name, args)?;
        Ok(crate::tools::settle(&self.bench, done))
    }

    /// The answer to one line, as one line; `None` for a notification or a response, which get none.
    pub fn answer(&mut self, line: &str) -> Option<String> {
        // THE NAMES OF THE TREE IN ONE LANGUAGE, whatever the machine is set to: the language is kept per thread and
        // falls to the system's when none was chosen, so a server on a Russian desktop named its bodies in Russian where
        // the same call elsewhere answered "Body 1", and a model told the tools in English met a tree in another language.
        qymcad_i18n::set_language("en");
        let message: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => return Some(reply_fault(Value::Null, Fault { code: PARSE_ERROR, message: e.to_string() })),
        };
        // a message without an id is a notification; one without a method is a response to a request of ours,
        // and this server sends none
        let id = message.get("id").cloned()?;
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            return Some(reply_fault(id, Fault { code: INVALID_REQUEST, message: "a request names its method".into() }));
        };
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        let outcome = match method {
            "initialize" => Ok(initialize(&params)),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": crate::tools::catalogue() })),
            "tools/call" => self.call(&params),
            other => Err(Fault { code: METHOD_NOT_FOUND, message: format!("no method `{other}`") }),
        };
        Some(match outcome {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string(),
            Err(fault) => reply_fault(id, fault),
        })
    }

    /// One tool called: its answer as text for the model to read and as the same object for a client to parse.
    fn call(&mut self, params: &Value) -> Result<Value, Fault> {
        let Some(name) = params.get("name").and_then(Value::as_str) else {
            return Err(Fault { code: INVALID_PARAMS, message: "`tools/call` names the tool in `name`".into() });
        };
        if !crate::tools::known(name) {
            return Err(Fault { code: INVALID_PARAMS, message: format!("no tool `{name}`") });
        }
        let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
        Ok(match self.work(name, &args) {
            Ok(answer) => shown(answer),
            Err(refusal) => json!({ "content": [{ "type": "text", "text": refusal }], "isError": true }),
        })
    }
}

/// AN ANSWER AS THE CLIENT SHOWS IT: the words, and a picture as a picture. A picture comes inside the answer as
/// `image_png` in base64 and goes out as an `image` block the model looks at; it is taken out of the words, where its
/// characters would only cost a model what it reads.
fn shown(mut answer: Value) -> Value {
    let picture = answer.as_object_mut().and_then(|o| o.remove(crate::tools::IMAGE_KEY));
    let mut content = vec![json!({ "type": "text", "text": serde_json::to_string_pretty(&answer).unwrap_or_default() })];
    if let Some(png) = picture.as_ref().and_then(Value::as_str) {
        content.insert(0, json!({ "type": "image", "data": png, "mimeType": "image/png" }));
    }
    json!({ "content": content, "structuredContent": answer, "isError": false })
}

/// The handshake: the revision of the protocol both sides speak, what the server offers, and who it is.
fn initialize(params: &Value) -> Value {
    let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or_default();
    let version = PROTOCOL_VERSIONS.iter().copied().find(|v| *v == asked).unwrap_or(PROTOCOL_VERSIONS[0]);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": "qymcad-mcp", "version": env!("CARGO_PKG_VERSION") },
        "instructions": INSTRUCTIONS,
    })
}

/// A JSON-RPC error answer, as one line.
fn reply_fault(id: Value, fault: Fault) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": fault.code, "message": fault.message } }).to_string()
}
