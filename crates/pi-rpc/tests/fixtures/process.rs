// Standalone std-only executable, compiled by the integration-test helper.
use std::{io::{self, BufRead, Write}, time::{Duration, Instant}};
fn field(line: &str, key: &str) -> String {
    let marker = format!("\"{key}\":\"");
    line.split_once(&marker).and_then(|(_, tail)| tail.split_once('"')).map(|(s,_)| s.to_owned()).unwrap_or_default()
}
fn response(id: &str, command: &str, data: &str) {
    println!("{{\"type\":\"response\",\"id\":\"{id}\",\"command\":\"{command}\",\"success\":true,\"data\":{data}}}");
    io::stdout().flush().unwrap();
}
fn main() {
    let mode = std::env::var("FIXTURE_MODE").unwrap_or_default();
    #[cfg(unix)]
    if mode == "ignore_term" {
        unsafe extern "C" { fn signal(number: i32, handler: usize) -> usize; }
        unsafe { signal(15, 1); }
    }
    let start = Instant::now();
    let log = std::env::var_os("FIXTURE_LOG");
    let record = |event: &str| {
        if let Some(path) = &log {
            let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path).unwrap();
            writeln!(file, "{event} {}", start.elapsed().as_millis()).unwrap();
        }
    };
    if mode == "early" { return; }
    if mode == "no_read" {
        println!("{{\"type\":\"fixture_started\"}}");
        io::stdout().flush().unwrap();
        loop { std::thread::sleep(Duration::from_secs(1)); }
    }
    let mut held = None;
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        let id = field(&line, "id");
        let command = field(&line, "type");
        record(&command);
        if command == "extension_ui_response" {
            println!("{{\"type\":\"fixture_reply\",\"reply\":{line}}}");
            io::stdout().flush().unwrap();
            continue;
        }
        if command == "get_state" {
            if mode == "startup_ui" && id == "0" {
                println!("{{\"type\":\"extension_ui_request\",\"id\":\"ui-start\",\"method\":\"confirm\",\"title\":\"Start\",\"message\":\"Reply while starting\"}}");
                io::stdout().flush().unwrap();
                held = Some((id, command));
                continue;
            }
            if mode == "invalid" { println!("{{broken"); }
            else if mode == "oversize" { println!("{}", "x".repeat(4096)); }
            else {
                response(&id, &command, &format!("{{\"sessionId\":\"{}\",\"isStreaming\":false,\"isCompacting\":false,\"futureField\":7}}", std::process::id()));
            }
        } else if command == "prompt" {
            match field(&line, "message").as_str() {
                "hold" => { held = Some((id, command)); }
                "release" => {
                    response(&id, &command, "null");
                    println!("{{\"type\":\"future_event\",\"extra\":42}}");
                    if let Some((id, cmd)) = held.take() { response(&id, &cmd, "null"); }
                }
                "flood" => {
                    for _ in 0..256 { println!("{{\"type\":\"delta\",\"text\":\"data\"}}"); }
                    io::stdout().flush().unwrap();
                }
                "stderr" => { eprintln!("{}TAIL", "x".repeat(8192)); response(&id, &command, "null"); }
                "exit" => return,
                "mismatch" => response(&id, "abort", "null"),
                "ui" => {
                    println!("{{\"type\":\"extension_ui_request\",\"id\":\"ui-1\",\"method\":\"input\",\"title\":\"Question\",\"future\":5}}");
                    response(&id, &command, "null");
                }
                _ => response(&id, &command, "null"),
            }
        } else if command == "get_commands" {
            println!("{{\"type\":\"response\",\"id\":\"{id}\",\"command\":\"get_commands\",\"success\":false,\"error\":\"fixture rejection\"}}");
        } else if command == "clear_queue" {
            if let Some((id, cmd)) = held.take() {
                response(&id, &cmd, "{\"sessionId\":\"fixture\",\"isStreaming\":false,\"isCompacting\":false}");
            }
            response(&id, &command, "{\"steering\":[\"one\"],\"followUp\":[],\"future\":true}");
        } else if command == "abort" {
            // Deliberately no response: shutdown must not await this command.
        }
        io::stdout().flush().unwrap();
    }
    record("eof");
    if mode == "linger" || mode == "ignore_term" { loop { std::thread::sleep(Duration::from_secs(1)); } }
}
