# pi-rpc

A Rust client for one local Pi CLI process and its JSONL RPC connection. It has no GPUI dependency. Each `Client::spawn` creates one child; cloned clients share that process. The host owns installation, authentication, configuration, working directories, UI and any collection of clients.

Validated with Pi 0.85.1. Unknown events and extension fields retain their raw JSON; this is not a guarantee that older Pi releases implement the same semantics.

## Usage

Call `Client::spawn` from an entered Tokio runtime. Spawn returns a client and event stream before the protocol is ready, so the host can handle extension requests during startup. The automatic `get_state` response establishes readiness; `state()` / `subscribe()` describe connection lifecycle, and the Ready payload is the initial snapshot. Use `get_state()` when a current Pi session snapshot is needed.

```rust,no_run
use pi_rpc::{Client, LaunchOptions, protocol::{Event, Prompt}};

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let options = LaunchOptions::new("/absolute/path/to/pi", "/path/to/project");
let (client, mut events) = Client::spawn(options).await?;
let consumer = tokio::spawn(async move {
    while let Some(event) = events.recv().await {
        match event {
            Event::ExtensionUi { request, raw } => {
                // Route to host UI. Use client.reply(id, UiReply) for a user answer.
                // Do not report cancelled merely because UI is unavailable.
                let _ = (request, raw);
            }
            Event::Agent { kind, raw } => {
                // Apply the event to the host's view of this session.
                let _ = (kind, raw);
            }
        }
    }
});
let initial = client.ready().await?;
let commands = client.get_commands().await?;
let accepted = client.prompt(Prompt::new("Explain this project")).await?;
// Acceptance is not completion: consume execution events through agent_settled.
let _ = (initial, commands, accepted);
let report = client.close().await;
consumer.await?;
assert!(report.reason.is_none(), "connection close failed: {report:?}");
# Ok(())
# }
```

The example sends a model request when run with a configured Pi; tests below do not. A real UI consumer must retain access to a client for replies without keeping an accidental ownership cycle alive indefinitely.

`probe::probe(command, deadline)` locates a command and checks `--version`. Its existing 15-second application budget, bounded output and cancellation behavior are independent of a long-running RPC request. `LaunchOptions` accepts an executable, cwd, extra argument vector, environment overrides and startup timeout; use the resolved probe path when available. Arguments are passed through `Command::args`, not concatenated into a shell string. On Windows the Rust standard library handles `.cmd`/`.bat` launching and quoting ([Rust process documentation](https://doc.rust-lang.org/stable/std/process/index.html#windows-argument-splitting)); the host still supplies the actual shim path.

Typed commands cover state, command discovery, prompt, abort, clear_queue, entries, fork messages/fork, session names, available models/model selection, thinking levels and session statistics. `get_entries` returns Pi parent links and the execution leaf; `fork` switches that client to an independent session and returns draft text, so the host must update its session binding after success. `request_raw` is an escape hatch for other Pi commands: it owns the request ID and applies the same limits and response association. `reply` uses the extension UI envelope and ID, without adding a command-response waiter. Dropping a request future removes only the local waiter; it does not abort Pi or replay the request.

## Limits and failure behavior

Defaults are configurable through `LaunchOptions::limits`:

| Resource | Default |
| --- | --- |
| Encoded incoming/outgoing frame | 32 MiB |
| Total queued event bytes | 32 MiB, at most 128 events |
| Total queued/in-flight normal write bytes | 32 MiB, at most 32 queued writes |
| Pending requests | 128 |
| stderr tail | 64 KiB |
| Startup, including spawn and first get_state | 15 seconds |

The byte budgets measure JSON wire bytes, not total allocator usage; parsed JSON and the event currently held by the consumer also occupy memory. A 32 MiB frame leaves room for a roughly 24 MiB binary image encoded as base64, less JSON overhead; it is not an unlimited history-transfer contract. The initial capacities are conservative implementation defaults, not measurements of all Pi workloads. Tests use reduced capacities to exercise failure behavior.

LF is the only separator; CRLF and a valid final JSON fragment without LF are accepted. UTF-8 is buffered across reads, and U+2028/U+2029 inside JSON strings do not split frames. Malformed/oversized input, event overload or I/O failure closes the connection and settles all pending requests. Full outgoing queues or pending-request capacity reject that submission. Unknown or cancelled-request responses are not turned into events. Dropping the event stream closes the connection, so a forgotten consumer cannot silently discard an active session.

A failed command returns `Error::Rejected`; other typed errors distinguish startup, protocol, I/O, capacity and closed-connection failures. A failed startup preserves its cause. `CloseReport` includes an observed exit status when available, the connection/shutdown error and the captured bounded stderr tail; the library does not automatically log message content or environment values.

## Shutdown and ownership

`close()` starts closing immediately and returns an awaitable result. Repeated calls share the outcome; cancelling the wait does not cancel cleanup. The last client drop requests the same cleanup while the Tokio runtime is alive. Keep the runtime alive and explicitly await close before exiting the host.

Normal close rejects new requests and settles pending waiters, then sends abort with a reserved request ID. Pi's successful abort response confirms the current execution has settled. The writer then releases stdin so Pi can execute its EOF/session-shutdown path. The client continues reading final events and observing process exit. This entire graceful phase has one 2-second deadline and finishes immediately when stdout closes and the process exits; there are no fixed sleeps or additional SIGTERM/reap phases.

On communication failure or shutdown timeout, the owner cancels and joins its pipe tasks, releases the Child with `kill_on_drop(true)`, and publishes `ConnectionState::Closed`. Tokio handles termination and background reaping. Dropping a Tokio JoinHandle alone would detach its task, so cancelling the pipe tasks remains necessary. No extra OS-process wait is added on error paths, including failed startup and failed version probes.

`Closed` means the connection and its owned I/O resources have been released. `CloseReport.status` is `Some` only when exit was observed, and can be `None` after a failure or timeout. `reason` preserves the failure or reports `ShutdownTimeout`; it does not claim graceful completion or synchronous OS reaping. The host closes clients sequentially and keeps its runtime alive while awaiting these connection results.

Abort bypasses the normal write queue. A blocked or partially written normal request can prevent a valid abort exchange; the same graceful deadline bounds this case. Forced Child termination does not promise that arbitrary extension-created descendants or Windows shim descendants have exited. Windows runtime validation remains platform-specific.

## Tests

```sh
cargo test -p pi-rpc --locked
cargo test -p pi-rpc --test installed_pi --locked -- --ignored
```

Default integration tests compile a small std-only Rust fixture with the installed Rust toolchain, then exercise real pipes and process termination. They do not need Pi, Node, a provider or network access. The Windows-only shim test runs under the normal Windows test target.

The ignored test requires an existing Pi executable from `PI_RPC_TEST_COMMAND` or PATH. It uses isolated cwd/agent/session directories, offline mode, no provider credentials and a minimal explicit extension. It verifies command association, four standard UI methods, two independent processes and shutdown, without making model calls. Explicitly running it without Pi fails clearly.

Pi 0.85.1 binds `session_start` extensions before registering its stdin reader. A startup extension awaiting UI can therefore prevent Ready even after the host replies. The client reports failed readiness; it does not patch Pi or pretend the user cancelled.
