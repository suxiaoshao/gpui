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
        assert!(report.status.is_some(), "{report:?}"); assert!(report.cleanup_error.is_none(), "{report:?}");
        assert!(second.get_state().await.is_ok());
        let report = second.close().await;
        assert!(report.status.is_some(), "{report:?}"); assert!(report.cleanup_error.is_none(), "{report:?}");
    }).await.expect("installed Pi integration exceeded its total deadline");
}
