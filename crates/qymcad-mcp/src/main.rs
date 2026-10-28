//! The server on stdio: one JSON-RPC message per line in, one answer per line out.
//!
//! Stdout carries the protocol and nothing else - a stray line there breaks the client's reading of every message
//! after it - so the answers go to a copy of stdout taken at start, and whatever else the process prints goes to
//! stderr (`channel`).
use std::io::{BufRead, Write};

fn main() {
    let answers: Box<dyn Write> = match qymcad_mcp::channel::take_stdout() {
        Some(file) => Box::new(file),
        None => Box::new(std::io::stdout()),
    };
    let mut out = std::io::LineWriter::new(answers);
    let mut server = qymcad_mcp::protocol::Server::reaching_windows();
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        if let Some(answer) = server.answer(&line) {
            if writeln!(out, "{answer}").and_then(|_| out.flush()).is_err() {
                break; // the client went away
            }
        }
    }
}
