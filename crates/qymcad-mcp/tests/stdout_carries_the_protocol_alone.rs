//! STDOUT CARRIES THE PROTOCOL AND NOTHING ELSE, whatever the kernel prints on the way.
//!
//! Reported behaviour: a session broke after `export` to STEP. The STEP writer of OCCT prints its transfer statistics
//! to the standard output of the process, eight lines of asterisks, and a client reading one JSON message per line
//! read them as the answer. Only the real process shows it: driven inside a test, the server's lines and the kernel's
//! never meet on one stream.
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

#[test]
fn every_line_the_server_prints_is_an_answer_even_past_a_step_export() {
    let step = std::env::temp_dir().join(format!("qymcad-mcp-{}-stdout.step", std::process::id()));
    let requests = [
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#.to_string(),
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.to_string(),
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"primitive","arguments":{"shape":"box","x":10,"y":10,"z":10}}}"#.to_string(),
        serde_json::json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": "export", "arguments": { "path": step.to_string_lossy() } } }).to_string(),
        r#"{"jsonrpc":"2.0","id":4,"method":"ping"}"#.to_string(),
    ];
    let mut server = Command::new(env!("CARGO_BIN_EXE_qymcad-mcp")).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().expect("the server starts");
    {
        let mut stdin = server.stdin.take().expect("the server's input");
        for r in &requests {
            writeln!(stdin, "{r}").expect("a request is written");
        }
    } // the input closed: the server answers what it has and ends
    let lines: Vec<String> = BufReader::new(server.stdout.take().expect("the server's output")).lines().map_while(Result::ok).collect();
    let _ = server.wait();
    let _ = std::fs::remove_file(&step);

    let foreign: Vec<&String> = lines.iter().filter(|l| serde_json::from_str::<serde_json::Value>(l).is_err()).collect();
    assert!(foreign.is_empty(), "stdout carries lines that are not the protocol's:\n{}", foreign.iter().map(|l| l.as_str()).collect::<Vec<_>>().join("\n"));
    let ids: Vec<u64> = lines.iter().filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok()).filter_map(|v| v["id"].as_u64()).collect();
    assert_eq!(ids, vec![1, 2, 3, 4], "four requests, four answers, in order: {lines:?}");
    let export: serde_json::Value = serde_json::from_str(&lines[2]).expect("the export's answer");
    assert_eq!(export["result"]["isError"], false, "the export was refused: {export}");
}
