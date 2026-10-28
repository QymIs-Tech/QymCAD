//! THE LINE TO A RUNNING WINDOW: how the server hands a tool to a QymCAD window, so the person watches every step
//! being made and can take the work over at any one of them.
//!
//! The window listens on the loopback alone, on a port the system picks, and says where in a small file in the
//! program's own folder - the door - with a token beside the port. A call that does not carry the token is refused:
//! the door is readable by the person's own processes only, and the port alone lets nobody in.
//!
//! One JSON message per line each way. A request names the tool and its arguments; the answer is `{"ok": ...}` or
//! `{"refused": "why"}`. The window takes requests one at a time, in its own frame, through `next`.
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

/// WHERE A WINDOW SAYS IT LISTENS: the port and the token a caller has to carry.
pub struct Door {
    pub port: u16,
    pub token: String,
}

/// The file the door is written to, in the program's own folder.
pub fn door_file() -> Option<std::path::PathBuf> {
    qymcad_paths::data("assistant.json")
}

/// The door of a window listening now, read from its file; `None` when no window has opened one.
pub fn read_door() -> Option<Door> {
    let text = std::fs::read_to_string(door_file()?).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    Some(Door { port: u16::try_from(v["port"].as_u64()?).ok()?, token: v["token"].as_str()?.to_string() })
}

/// A CALL WAITING FOR THE WINDOW: the tool, its arguments, and where the answer goes.
pub struct Request {
    pub tool: String,
    pub args: Value,
    reply: mpsc::Sender<Result<Value, String>>,
}

impl Request {
    /// Answer the call; a caller that went away meanwhile is not waited for.
    pub fn answer(self, outcome: Result<Value, String>) {
        let _ = self.reply.send(outcome);
    }
}

/// What wakes the window when a call comes in: a window sleeps between input events and would not look otherwise.
pub type Wake = dyn Fn() + Send + Sync;

/// The listening of this process, while it lasts.
struct Listening {
    stop: Arc<AtomicBool>,
    calls: mpsc::Receiver<Request>,
    door: Door,
}

static LISTENING: Mutex<Option<Listening>> = Mutex::new(None);

/// START LISTENING, once: a second call while listening changes nothing. The door is written to `file` when one is
/// given - a window writes it where the server looks, a check keeps it to itself.
pub fn listen(wake: Arc<Wake>, file: Option<std::path::PathBuf>) -> Result<(), String> {
    let mut held = LISTENING.lock().map_err(|_| "the listening is poisoned".to_string())?;
    if held.is_some() {
        return Ok(());
    }
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| format!("cannot listen on the loopback: {e}"))?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let token = fresh_token();
    if let Some(f) = &file {
        if let Some(dir) = f.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        std::fs::write(f, json!({ "port": port, "token": token, "pid": std::process::id() }).to_string()).map_err(|e| format!("cannot write the door {}: {e}", f.display()))?;
    }
    let (calls_in, calls) = mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let stopped = Arc::clone(&stop);
    let needed = token.clone();
    std::thread::spawn(move || accept(listener, Gate { stop: stopped, token: needed, calls: calls_in, wake, file }));
    *held = Some(Listening { stop, calls, door: Door { port, token } });
    Ok(())
}

/// Stop listening: the door is taken down and the calls not yet taken are refused.
pub fn stop() {
    if let Ok(mut held) = LISTENING.lock() {
        if let Some(l) = held.take() {
            l.stop.store(true, Ordering::SeqCst);
        }
    }
}

/// Is this process listening now.
pub fn listening() -> bool {
    LISTENING.lock().map(|h| h.is_some()).unwrap_or_default()
}

/// The door this process listens at, for a caller in the same process.
pub fn own_door() -> Option<Door> {
    LISTENING.lock().ok()?.as_ref().map(|l| Door { port: l.door.port, token: l.door.token.clone() })
}

/// The next call waiting, if any; the window takes one per step.
pub fn next() -> Option<Request> {
    LISTENING.lock().ok()?.as_ref()?.calls.try_recv().ok()
}

/// What the accepting thread holds.
struct Gate {
    stop: Arc<AtomicBool>,
    token: String,
    calls: mpsc::Sender<Request>,
    wake: Arc<Wake>,
    file: Option<std::path::PathBuf>,
}

/// THE ACCEPTING THREAD: one thread per caller, until the listening stops. The listener does not block, so the stop is
/// heard within one nap (50 ms) instead of at the next caller.
fn accept(listener: TcpListener, gate: Gate) {
    while !gate.stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                let caller = Gate { stop: Arc::clone(&gate.stop), token: gate.token.clone(), calls: gate.calls.clone(), wake: Arc::clone(&gate.wake), file: None };
                std::thread::spawn(move || serve_caller(stream, caller));
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(50)),
            Err(_) => break,
        }
    }
    if let Some(f) = &gate.file {
        let _ = std::fs::remove_file(f);
    }
}

/// One caller: its calls are handed to the window one at a time and answered in order.
fn serve_caller(stream: TcpStream, gate: Gate) {
    if stream.set_nonblocking(false).is_err() {
        return;
    }
    let Ok(mut out) = stream.try_clone() else { return };
    for line in BufReader::new(stream).lines() {
        let Ok(line) = line else { break };
        let answer = match serde_json::from_str::<Value>(&line) {
            Err(e) => Err(format!("the call is not JSON: {e}")),
            Ok(call) if call["token"].as_str() != Some(gate.token.as_str()) => Err("the call does not carry the window's token".to_string()),
            Ok(_) if gate.stop.load(Ordering::SeqCst) => Err("the window has stopped taking an assistant".to_string()),
            Ok(call) => {
                let (reply, answered) = mpsc::channel();
                let request = Request { tool: call["tool"].as_str().unwrap_or_default().to_string(), args: call["arguments"].clone(), reply };
                if gate.calls.send(request).is_err() {
                    Err("the window has stopped taking an assistant".to_string())
                } else {
                    (gate.wake)();
                    answered.recv().unwrap_or_else(|_| Err("the window closed before it answered".to_string()))
                }
            }
        };
        let text = match answer {
            Ok(v) => json!({ "ok": v }),
            Err(why) => json!({ "refused": why }),
        };
        if writeln!(out, "{text}").and_then(|_| out.flush()).is_err() {
            break;
        }
    }
}

/// A token no other process can guess: the keys of the standard hasher are drawn from the system's randomness.
fn fresh_token() -> String {
    use std::hash::BuildHasher;
    let a = std::collections::hash_map::RandomState::new().hash_one(std::process::id());
    let b = std::collections::hash_map::RandomState::new().hash_one(a);
    format!("{a:016x}{b:016x}")
}

/// A WINDOW REACHED FROM THE SERVER: the calls go to it and come back answered.
pub struct Window {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
    token: String,
}

impl Window {
    /// The window whose door is in the program's folder, when one is listening there.
    pub fn find() -> Option<Window> {
        Window::at(read_door()?)
    }

    /// The window at a door.
    pub fn at(door: Door) -> Option<Window> {
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], door.port));
        let stream = TcpStream::connect_timeout(&addr, Duration::from_millis(500)).ok()?;
        let writer = stream.try_clone().ok()?;
        Some(Window { reader: BufReader::new(stream), writer, token: door.token })
    }

    /// Hand a tool to the window: its answer, its refusal, or `Err` of the line when the window is gone.
    pub fn call(&mut self, tool: &str, args: &Value) -> std::io::Result<Result<Value, String>> {
        writeln!(self.writer, "{}", json!({ "token": self.token, "tool": tool, "arguments": args }))?;
        self.writer.flush()?;
        let mut line = String::new();
        if self.reader.read_line(&mut line)? == 0 {
            return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "the window closed the line"));
        }
        let v: Value = serde_json::from_str(&line).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(match v.get("ok") {
            Some(ok) => Ok(ok.clone()),
            None => Err(v["refused"].as_str().unwrap_or("the window refused without a reason").to_string()),
        })
    }
}
