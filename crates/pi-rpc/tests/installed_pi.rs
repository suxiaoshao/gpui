use pi_rpc::{
    Client, Error, LaunchOptions,
    protocol::{Event, Prompt, UiMethod, UiReply},
};
use std::{path::Path, time::Duration};

fn options(executable: &Path, dir: &Path) -> LaunchOptions {
    let mut options = LaunchOptions::new(executable, dir);
    options.clear_env = true;
    for key in [
        "PATH",
        "TMPDIR",
        "TMP",
        "TEMP",
        "LANG",
        "LC_ALL",
        "SystemRoot",
    ] {
        if let Some(value) = std::env::var_os(key) {
            options.env.push((key.into(), value));
        }
    }
    options
        .env
        .push(("PI_CODING_AGENT_DIR".into(), dir.join("agent").into()));
    options.env.push((
        "PI_CODING_AGENT_SESSION_DIR".into(),
        dir.join("sessions").into(),
    ));
    options.args = [
        "--offline",
        "--no-session",
        "--no-extensions",
        "--no-skills",
        "--no-prompt-templates",
        "--no-themes",
        "--no-context-files",
        "--no-approve",
        "--extension",
    ]
    .map(Into::into)
    .to_vec();
    options.args.push(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/extension.mjs")
            .into(),
    );
    options
}
#[tokio::test]
#[ignore = "requires a user-installed Pi; no provider credentials or model requests"]
async fn installed_pi_protocol_extensions_and_two_processes() {
    tokio::time::timeout(Duration::from_secs(35), async {
        let command = std::env::var("PI_RPC_TEST_COMMAND").unwrap_or_else(|_| "pi".into());
        let probe = pi_rpc::probe::probe(command, tokio::time::Instant::now() + Duration::from_secs(5))
            .await.expect("install Pi externally or set PI_RPC_TEST_COMMAND to an existing executable");
        eprintln!("installed Pi {} on {}", probe.version, std::env::consts::OS);
        let first_dir = tempfile::tempdir().unwrap();
        let second_dir = tempfile::tempdir().unwrap();
        let (first, mut events) = Client::spawn(options(&probe.command, first_dir.path())).await.unwrap();
        let (second, _second_events) = Client::spawn(options(&probe.command, second_dir.path())).await.unwrap();
        let first_state = first.ready().await.unwrap();
        let second_state = second.ready().await.unwrap();
        assert_ne!(first_state.session_id, second_state.session_id);
        assert!(first.get_entries().await.unwrap().entries.iter().all(|e| e.kind != "message"));
        assert!(first.get_fork_messages().await.unwrap().messages.is_empty());
        let _ = first.get_available_models().await.unwrap();
        assert!(!first.get_available_thinking_levels().await.unwrap().levels.is_empty());
        let stats = first.get_session_stats().await.unwrap();
        assert_eq!(stats.tokens.input, 0);
        assert_eq!(stats.cost, 0.0);

        assert!(first.get_commands().await.unwrap().commands.iter().any(|c| c.name == "gupi-rpc-test"));
        assert!(matches!(first.request_raw(serde_json::json!({"type":"__gupi_unknown_command"})).await, Err(Error::Rejected { .. })));
        let prompt = first.prompt(Prompt::new("/gupi-rpc-test"));
        let interaction = async {
            let mut methods = Vec::new();
            while let Some(event) = events.recv().await {
                let Event::ExtensionUi { request, .. } = event else { continue; };
                let reply = match request.method {
                    UiMethod::Select { .. } => { methods.push("select"); Some(UiReply::Value { value: "second".into() }) },
                    UiMethod::Confirm { .. } => { methods.push("confirm"); Some(UiReply::Confirmed { confirmed: true }) },
                    UiMethod::Input { .. } => { methods.push("input"); Some(UiReply::Value { value: "answer".into() }) },
                    UiMethod::Editor { .. } => { methods.push("editor"); Some(UiReply::Value { value: "edited".into() }) },
                    UiMethod::Notify { message, .. } => {
                        let result: serde_json::Value = serde_json::from_str(&message).unwrap();
                        assert_eq!(result, serde_json::json!({"selected":"second","confirmed":true,"input":"answer","edited":"edited"}));
                        assert_eq!(methods, ["select", "confirm", "input", "editor"]);
                        return;
                    },
                    _ => None,
                };
                if let Some(reply) = reply { first.reply(&request.id, reply).unwrap(); }
            }
            panic!("Pi exited before extension interaction completed");
        };
        let (response, ()) = tokio::join!(prompt, interaction);
        response.unwrap();
        let report = first.close().await;
        assert!(report.status.is_some(), "{report:?}"); assert!(report.reason.is_none(), "{report:?}");
        assert!(second.get_state().await.is_ok());
        let report = second.close().await;
        assert!(report.status.is_some(), "{report:?}"); assert!(report.reason.is_none(), "{report:?}");
    }).await.expect("installed Pi integration exceeded its total deadline");
}

#[tokio::test]
#[ignore = "requires a user-installed Pi; temporary history only, no model requests"]
async fn installed_pi_restores_renames_and_forks_without_rewriting_the_source() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let command = std::env::var("PI_RPC_TEST_COMMAND").unwrap_or_else(|_| "pi".into());
        let probe = pi_rpc::probe::probe(command, tokio::time::Instant::now() + Duration::from_secs(5)).await.unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("source.jsonl");
        let entries = [
            serde_json::json!({"type":"session","version":3,"id":"gupi-fork-fixture","timestamp":"2026-09-09T10:00:00.000Z","cwd":dir.path()}),
            serde_json::json!({"type":"message","id":"user-1","parentId":null,"timestamp":"2026-09-09T10:00:01.000Z","message":{"role":"user","content":[{"type":"text","text":"preserved question"}],"timestamp":1788948001000u64}}),
            serde_json::json!({"type":"message","id":"answer-1","parentId":"user-1","timestamp":"2026-09-09T10:00:02.000Z","message":{"role":"assistant","content":[{"type":"text","text":"preserved answer"}],"api":"openai-responses","provider":"openai","model":"gpt-5","usage":{"input":1,"output":1,"cacheRead":0,"cacheWrite":0,"totalTokens":2,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},"stopReason":"stop","timestamp":1788948002000u64}}),
        ];
        std::fs::write(&path, entries.iter().map(|e|format!("{e}\n")).collect::<String>()).unwrap();
        let mut launch = options(&probe.command, dir.path());
        launch.args.retain(|a|a!="--no-session");
        launch.args.extend(["--session".into(),path.clone().into()]);
        let (client, _events) = Client::spawn(launch).await.unwrap();
        assert_eq!(client.ready().await.unwrap().session_id, "gupi-fork-fixture");
        assert!(client.get_entries().await.unwrap().entries.iter().any(|e| e.id=="answer-1"));
        client.set_session_name("renamed fixture".into()).await.unwrap();
        assert_eq!(client.get_state().await.unwrap().session_name.as_deref(),Some("renamed fixture"));
        let source = std::fs::read(&path).unwrap();
        assert_eq!(client.get_fork_messages().await.unwrap().messages[0].entry_id,"user-1");
        let fork = client.fork("user-1".into()).await.unwrap();
        assert!(!fork.cancelled);
        assert_eq!(fork.text,"preserved question");
        let new_state=client.get_state().await.unwrap();
        assert_ne!(new_state.session_id,"gupi-fork-fixture");
        assert_ne!(new_state.session_file.as_deref(),path.to_str());
        assert_eq!(std::fs::read(&path).unwrap(),source);
        client.close().await;
    }).await.unwrap();
}
