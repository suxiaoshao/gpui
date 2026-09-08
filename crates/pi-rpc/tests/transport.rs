mod support;
use pi_rpc::{
    Client, ConnectionState, Error,
    protocol::{Event, Prompt, UiReply},
};
use std::time::Duration;
use tokio::time::timeout;

async fn bounded<F: std::future::Future>(future: F) -> F::Output {
    timeout(Duration::from_secs(12), future)
        .await
        .expect("test exceeded its process cleanup budget")
}
#[tokio::test]
async fn request_order_cancellation_extensions_and_isolation() {
    bounded(async {
        let dir = tempfile::tempdir().unwrap();
        let other_dir = tempfile::tempdir().unwrap();
        let (client, mut events) = Client::spawn(support::options(dir.path(), "")).await.unwrap();
        let (other, _other_events) = Client::spawn(support::options(other_dir.path(), "")).await.unwrap();
        let state = client.ready().await.unwrap();
        assert_eq!(state.extra["futureField"], 7);
        assert_ne!(state.session_id, other.ready().await.unwrap().session_id);
        let hold = client.prompt(Prompt::new("hold"));
        let release = client.prompt(Prompt::new("release"));
        let (hold, release) = tokio::join!(hold, release);
        assert!(hold.is_ok()); assert!(release.is_ok());
        assert!(matches!(events.recv().await, Some(Event::Agent { kind, raw }) if kind == "future_event" && raw["extra"] == 42));
        let commands = client.get_commands().await;
        assert!(matches!(commands, Err(Error::Rejected { .. })));
        assert_eq!(client.clear_queue().await.unwrap().steering, ["one"]);
        assert!(timeout(Duration::from_millis(30), client.prompt(Prompt::new("hold"))).await.is_err());
        client.prompt(Prompt::new("release")).await.unwrap();
        let _ = events.recv().await;
        client.prompt(Prompt::new("ui")).await.unwrap();
        let event = events.recv().await.unwrap();
        let Event::ExtensionUi { request, raw } = event else { panic!("expected UI") };
        assert_eq!(raw["future"], 5);
        client.reply(&request.id, UiReply::Value { value: "answer".into() }).unwrap();
        assert!(matches!(events.recv().await, Some(Event::Agent { raw, .. }) if raw["reply"]["value"] == "answer"));
        let log = std::fs::read_to_string(dir.path().join("process.log")).unwrap();
        assert!(!log.lines().any(|line| line.starts_with("abort ")), "cancelling a wait must not abort Pi");
        let report = client.close().await;
        assert!(report.status.is_some()); assert!(report.cleanup_error.is_none(), "{report:?}");
        assert!(matches!(client.get_state().await, Err(Error::Closed)));
        assert!(client.close().await.status.is_some());
        assert!(other.get_state().await.is_ok());
        assert!(other.close().await.status.is_some());
    }).await;
}
#[tokio::test]
async fn startup_events_are_accessible_before_ready() {
    bounded(async {
        let dir = tempfile::tempdir().unwrap();
        let (client, mut events) = Client::spawn(support::options(dir.path(), "startup_ui"))
            .await
            .unwrap();
        assert!(matches!(
            events.recv().await,
            Some(Event::ExtensionUi { .. })
        ));
        assert!(matches!(client.state(), ConnectionState::Starting));
        client
            .reply("ui-start", UiReply::Confirmed { confirmed: true })
            .unwrap();
        client.clear_queue().await.unwrap();
        assert!(client.ready().await.is_ok());
        assert!(client.close().await.status.is_some());
    })
    .await;
}
#[tokio::test]
async fn protocol_startup_and_consumer_failures_settle_waiters() {
    bounded(async {
        for (mode, expected) in [
            ("early", "ready"),
            ("invalid", "protocol"),
            ("oversize", "capacity"),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let mut options = support::options(dir.path(), mode);
            options.limits.frame_bytes = 512;
            let (client, _events) = Client::spawn(options).await.unwrap();
            let error = client.ready().await.unwrap_err();
            match expected {
                "protocol" => assert!(matches!(error, Error::Protocol(_))),
                "capacity" => assert!(matches!(error, Error::Capacity(_))),
                _ => assert!(matches!(error, Error::NotReady | Error::Closed)),
            }
            assert!(client.close().await.status.is_some());
        }
        let dir = tempfile::tempdir().unwrap();
        let mut options = support::options(dir.path(), "");
        options.limits.events = 2;
        let (client, _events) = Client::spawn(options).await.unwrap();
        client.ready().await.unwrap();
        assert!(matches!(
            client.prompt(Prompt::new("flood")).await,
            Err(Error::Capacity(_))
        ));
        assert!(client.close().await.status.is_some());
    })
    .await;
}
#[tokio::test]
async fn shutdown_observes_abort_eof_and_terminates_lingering_process() {
    bounded(async {
        let dir = tempfile::tempdir().unwrap();
        let mut options = support::options(dir.path(), "linger");
        options.limits.stderr_bytes = 128;
        let (client, _events) = Client::spawn(options).await.unwrap();
        client.ready().await.unwrap();
        client.prompt(Prompt::new("stderr")).await.unwrap();
        let started = std::time::Instant::now();
        let report = client.close().await;
        assert!(started.elapsed() >= Duration::from_millis(1000));
        assert!(report.status.is_some());
        assert!(!report.forced, "{report:?}");
        assert!(report.cleanup_error.is_none(), "{report:?}");
        assert!(report.stderr.ends_with("TAIL\n"));
        assert!(report.stderr.len() <= 128);
        let log = std::fs::read_to_string(dir.path().join("process.log")).unwrap();
        let stamp = |name: &str| {
            log.lines()
                .find_map(|line| {
                    line.strip_prefix(name)
                        .map(|v| v.trim().parse::<u128>().unwrap())
                })
                .unwrap()
        };
        assert!(stamp("eof ") >= stamp("abort ") + 450, "{log}");
    })
    .await;
}
#[tokio::test]
async fn blocked_stdin_and_dropped_last_client_cannot_leak_process_ownership() {
    bounded(async {
        let dir = tempfile::tempdir().unwrap();
        let (client, mut events) = Client::spawn(support::options(dir.path(), "no_read"))
            .await
            .unwrap();
        events.recv().await.unwrap();
        let pending = client.prompt(Prompt::new("x".repeat(1024 * 1024)));
        assert!(timeout(Duration::from_millis(30), pending).await.is_err());
        let mut state = client.subscribe();
        drop(client);
        loop {
            if let ConnectionState::Exited(report) = state.borrow_and_update().clone() {
                assert!(report.status.is_some());
                break;
            }
            state.changed().await.unwrap();
        }
    })
    .await;
}

#[tokio::test]
async fn request_limit_and_cancelled_close_wait_preserve_cleanup() {
    bounded(async {
        let dir = tempfile::tempdir().unwrap();
        let mut options = support::options(dir.path(), "");
        options.limits.requests = 1;
        let (client, _events) = Client::spawn(options).await.unwrap();
        client.ready().await.unwrap();
        let waiting = client.prompt(Prompt::new("hold"));
        let closing = async {
            loop {
                let log =
                    std::fs::read_to_string(dir.path().join("process.log")).unwrap_or_default();
                if log.lines().any(|line| line.starts_with("prompt ")) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            assert!(matches!(
                client.get_state().await,
                Err(Error::Capacity("pending requests"))
            ));
            // Closing starts synchronously, and dropping the wait cannot cancel it.
            drop(client.close());
        };
        let (result, ()) = tokio::join!(waiting, closing);
        assert!(matches!(result, Err(Error::Closed)));
        assert!(client.close().await.status.is_some());
    })
    .await;
}
#[tokio::test]
async fn mismatched_response_and_process_exit_fail_pending_requests() {
    bounded(async {
        for message in ["mismatch", "exit"] {
            let dir = tempfile::tempdir().unwrap();
            let (client, _events) = Client::spawn(support::options(dir.path(), ""))
                .await
                .unwrap();
            client.ready().await.unwrap();
            let result = client.prompt(Prompt::new(message)).await;
            if message == "mismatch" {
                assert!(matches!(result, Err(Error::Protocol(_))));
            } else {
                assert!(matches!(result, Err(Error::Closed)));
            }
            assert!(client.close().await.status.is_some());
        }
    })
    .await;
}
#[tokio::test]
async fn startup_deadline_closes_a_process_that_never_reads_requests() {
    bounded(async {
        let dir = tempfile::tempdir().unwrap();
        let mut options = support::options(dir.path(), "no_read");
        options.startup_timeout = Duration::from_millis(200);
        let (client, _events) = Client::spawn(options).await.unwrap();
        assert!(matches!(client.ready().await, Err(Error::StartupTimeout)));
        assert!(client.close().await.status.is_some());
    })
    .await;
}
#[cfg(windows)]
#[tokio::test]
async fn windows_cmd_shim_uses_structured_arguments() {
    bounded(async {
        let dir = tempfile::tempdir().unwrap();
        let shim = dir.path().join("pi shim.cmd");
        std::fs::write(
            &shim,
            format!("@echo off\r\n\"{}\" %*\r\n", support::fixture().display()),
        )
        .unwrap();
        let mut options = support::options(dir.path(), "");
        options.executable = shim;
        let (client, _events) = Client::spawn(options).await.unwrap();
        client.ready().await.unwrap();
        assert!(client.close().await.status.is_some());
    })
    .await;
}

#[cfg(unix)]
#[tokio::test]
async fn ignored_sigterm_is_reported_and_the_direct_child_is_still_reaped() {
    bounded(async {
        let dir = tempfile::tempdir().unwrap();
        let (client, _events) = Client::spawn(support::options(dir.path(), "ignore_term"))
            .await
            .unwrap();
        client.ready().await.unwrap();
        let report = client.close().await;
        assert!(report.forced, "{report:?}");
        assert!(
            report.cleanup_error.is_some(),
            "failed graceful termination must stay visible"
        );
        assert!(
            report.status.is_some(),
            "forced cleanup must reap the owned child"
        );
    })
    .await;
}
