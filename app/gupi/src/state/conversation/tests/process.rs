// Standalone std-only Pi protocol fixture for application loading regressions.
use std::{collections::HashMap, io::{self, BufRead, Write}, path::Path};
fn field(line: &str, key: &str) -> String {
    let marker = format!("\"{key}\":\"");
    line.split_once(&marker).and_then(|(_, tail)| tail.split_once('"')).map(|(s, _)| s.to_owned()).unwrap_or_default()
}
fn reply(id: &str, command: &str, data: &str, success: bool) {
    if success {
        println!("{{\"type\":\"response\",\"id\":\"{id}\",\"command\":\"{command}\",\"success\":true,\"data\":{data}}}");
    } else {
        println!("{{\"type\":\"response\",\"id\":\"{id}\",\"command\":\"{command}\",\"success\":false,\"error\":\"fixture read failed\"}}");
    }
    io::stdout().flush().unwrap();
}
fn model(id: &str) -> String {
    format!("{{\"id\":\"{id}\",\"name\":\"{id}\",\"provider\":\"fixture\",\"reasoning\":true}}")
}
fn main() {
    let mut selected = "alpha".to_owned();
    let mut thinking = "high".to_owned();
    let mut newer = false;
    let mut compacted = false;
    let mut held = Vec::<(String, String, String, bool)>::new();
    let mut waiters = Vec::<(String, String, usize)>::new();
    let mut counts = HashMap::<String, usize>::new();
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        let id = field(&line, "id");
        let command = field(&line, "type");
        *counts.entry(command.clone()).or_default() += 1;
        let mut log = std::fs::OpenOptions::new().create(true).append(true).open("commands.log").unwrap();
        writeln!(log, "{command}").unwrap();
        match command.as_str() {
            "disconnect" => break,
            "compact_start" => {
                println!("{{\"type\":\"compaction_start\",\"reason\":\"manual\"}}");
                reply(&id, &command, "null", true);
            }
            "compact_end" => {
                compacted = true;
                println!("{{\"type\":\"compaction_end\",\"reason\":\"manual\",\"aborted\":false,\"result\":{{\"summary\":\"compressed context\"}}}}");
                reply(&id, &command, "null", true);
            }
            "wait_for" => waiters.push((id, field(&line, "command"), field(&line, "count").parse().unwrap())),
            "release" => {
                let target = field(&line, "command");
                let _ = std::fs::remove_file(format!("hold-{target}"));
                let mut remaining = vec![];
                for (held_id, held_command, data, success) in held.drain(..) {
                    if held_command == target { reply(&held_id, &held_command, &data, success); }
                    else { remaining.push((held_id, held_command, data, success)); }
                }
                held = remaining;
                reply(&id, &command, "null", true);
            }
            "emit_editor" => {
                println!("{{\"type\":\"extension_ui_request\",\"id\":\"submission-editor\",\"method\":\"set_editor_text\",\"text\":\"next extension draft\"}}");
                reply(&id, &command, "null", true);
            }
            "emit_confirm" => {
                println!("{{\"type\":\"extension_ui_request\",\"id\":\"submission-confirm\",\"method\":\"confirm\",\"title\":\"Continue?\",\"message\":\"Fixture confirmation\"}}");
                reply(&id, &command, "null", true);
            }
            "emit_newer" => {
                newer = true;
                println!("{{\"type\":\"agent_start\"}}");
                println!("{{\"type\":\"message_update\",\"message\":{{\"role\":\"assistant\",\"timestamp\":9,\"content\":[{{\"type\":\"text\",\"text\":\"new live text\"}}]}}}}");
                reply(&id, &command, "null", true);
            }
            "settle" => {
                println!("{{\"type\":\"agent_settled\"}}");
                reply(&id, &command, "null", true);
            }
            _ => {
                let data = match command.as_str() {
                    "get_state" => format!("{{\"sessionId\":\"fixture\",\"isStreaming\":false,\"isCompacting\":false,\"model\":{},\"thinkingLevel\":\"{thinking}\"}}", model(&selected)),
                    "get_entries" => if Path::new("empty-entries").exists() {
                        "{\"entries\":[],\"leafId\":null}".into()
                    } else if compacted {
                        "{\"entries\":[{\"id\":\"old\",\"timestamp\":\"2026-09-11T00:00:00Z\",\"type\":\"message\",\"message\":{\"role\":\"user\",\"content\":\"hello\"}},{\"id\":\"compact\",\"parentId\":\"old\",\"timestamp\":\"2026-09-11T01:00:00Z\",\"type\":\"compaction\",\"summary\":\"compressed context\",\"firstKeptEntryId\":\"old\"}],\"leafId\":\"compact\"}".into()
                    } else if newer {
                        "{\"entries\":[{\"id\":\"new\",\"timestamp\":\"2026-09-11T01:00:00Z\",\"type\":\"message\",\"message\":{\"role\":\"assistant\",\"timestamp\":9,\"content\":[{\"type\":\"text\",\"text\":\"new live text\"}]}}],\"leafId\":\"new\"}".into()
                    } else {
                        "{\"entries\":[{\"id\":\"old\",\"timestamp\":\"2026-09-11T00:00:00Z\",\"type\":\"message\",\"message\":{\"role\":\"user\",\"content\":\"hello\"}}],\"leafId\":\"old\"}".into()
                    },
                    "get_available_models" => format!("{{\"models\":[{},{}]}}", model("alpha"), model("beta")),
                    "get_available_thinking_levels" => "{\"levels\":[\"off\",\"high\"]}".into(),
                    "get_session_stats" => "{\"tokens\":{\"input\":5,\"output\":3,\"cacheRead\":0,\"cacheWrite\":0,\"total\":8},\"cost\":0}".into(),
                    "get_fork_messages" => "{\"messages\":[{\"entryId\":\"old\",\"text\":\"hello\"}]}".into(),
                    "set_model" => { selected = field(&line, "modelId"); "null".into() }
                    "set_thinking_level" => { thinking = field(&line, "level"); "null".into() }
                    "fork" => {
                        println!("{{\"type\":\"extension_ui_request\",\"id\":\"fork-editor\",\"method\":\"set_editor_text\",\"text\":\"extension fork draft\"}}");
                        "{\"cancelled\":false,\"text\":\"fallback fork draft\"}".into()
                    }
                    _ => "null".into(),
                };
                let success = !Path::new(&format!("fail-{command}")).exists();
                if Path::new(&format!("hold-{command}")).exists() { held.push((id, command.clone(), data, success)); }
                else { reply(&id, &command, &data, success); }
            }
        }
        let mut remaining = vec![];
        for (id, target, count) in waiters.drain(..) {
            if counts.get(&target).copied().unwrap_or(0) >= count { reply(&id, "wait_for", "null", true); }
            else { remaining.push((id, target, count)); }
        }
        waiters = remaining;
    }
}
