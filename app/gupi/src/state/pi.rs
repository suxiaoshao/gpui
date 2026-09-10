//! Application ownership of independent Pi connections. No conversation UI policy.
use gpui_kit::*;
use gpui_tokio::Tokio;
use pi_rpc::{
    Client, CloseReport, ConnectionState, Error, EventStream, LaunchOptions, protocol::Event,
};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct InstanceId(u64);
struct Connection {
    client: Client,
    _consumer: Task<()>,
}
enum Instance {
    Launching { _task: Task<()> },
    Connected(Connection),
    Failed(Error),
}

/// Emitted without storing a second transcript or an unbounded event history.
pub(crate) struct PiEvent {
    pub instance: InstanceId,
    pub event: Event,
}
pub(crate) struct PiState {
    instances: BTreeMap<InstanceId, Instance>,
    next_id: u64,
    draining: bool,
}
impl EventEmitter<PiEvent> for PiState {}
struct GlobalPi(Entity<PiState>);
impl Global for GlobalPi {}

pub(crate) fn init(cx: &mut App) {
    let state = cx.new(|_| PiState {
        instances: BTreeMap::new(),
        next_id: 0,
        draining: false,
    });
    cx.set_global(GlobalPi(state));
}
pub(crate) fn global(cx: &App) -> Entity<PiState> {
    cx.global::<GlobalPi>().0.clone()
}

impl PiState {
    pub fn start(
        &mut self,
        options: LaunchOptions,
        cx: &mut Context<Self>,
    ) -> Result<InstanceId, Error> {
        if self.draining {
            return Err(Error::Closed);
        }
        self.next_id += 1;
        let id = InstanceId(self.next_id);
        let launch = Tokio::spawn(cx, Client::spawn(options));
        let task = cx.spawn(async move |owner, cx| {
            let result = launch
                .await
                .unwrap_or_else(|error| Err(Error::Io(error.to_string())));
            let _ = owner.update(cx, |owner, cx| {
                // Removing a launch cancels this completion route. Do not resurrect it.
                if !owner.instances.contains_key(&id) || owner.draining {
                    return;
                }
                let instance = match result {
                    Ok((client, events)) => Instance::Connected(Connection {
                        _consumer: Self::consume(id, client.clone(), events, cx),
                        client,
                    }),
                    Err(error) => Instance::Failed(error),
                };
                owner.instances.insert(id, instance);
                cx.notify();
            });
        });
        self.instances
            .insert(id, Instance::Launching { _task: task });
        cx.notify();
        Ok(id)
    }
    fn consume(
        id: InstanceId,
        client: Client,
        mut events: EventStream,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        let mut status = client.subscribe();
        cx.spawn(async move |owner, cx| {
            loop {
                // Tokio channels can be awaited on GPUI's executor; process I/O stays
                // on the Tokio owner. Only one delivered event is held on this bridge.
                tokio::select! {
                    event = events.recv() => {
                        let Some(event) = event else { break; };
                        if owner.update(cx, |_, cx| {
                            cx.emit(PiEvent { instance: id, event });
                            cx.notify();
                        }).is_err() { break; }
                    }
                    changed = status.changed() => {
                        if changed.is_err() { break; }
                        let exited = matches!(*status.borrow_and_update(), ConnectionState::Closed(_));
                        if owner.update(cx, |_, cx| cx.notify()).is_err() { break; }
                        if exited {
                            // Drain any events queued before the exit report.
                            while let Some(event) = events.recv().await {
                                if owner.update(cx, |_, cx| cx.emit(PiEvent { instance: id, event })).is_err() { break; }
                            }
                            break;
                        }
                    }
                }
            }
        })
    }
    pub fn client(&self, id: InstanceId) -> Result<Option<Client>, Error> {
        match self.instances.get(&id) {
            Some(Instance::Connected(connection)) => Ok(Some(connection.client.clone())),
            Some(Instance::Launching { .. }) => Ok(None),
            Some(Instance::Failed(error)) => Err(error.clone()),
            None => Err(Error::Closed),
        }
    }
    pub fn close(&mut self, id: InstanceId, cx: &mut Context<Self>) -> Task<Option<CloseReport>> {
        let instance = self.instances.remove(&id);
        let closing = match &instance {
            Some(Instance::Connected(connection)) => Some(connection.client.close()),
            _ => None,
        };
        cx.notify();
        cx.spawn(async move |_, _| {
            match (instance, closing) {
                (Some(Instance::Connected(connection)), Some(closing)) => {
                    // Keep the event consumer alive until the process owner is done.
                    let report = closing.await;
                    drop(connection);
                    Some(report)
                }
                _ => None,
            }
        })
    }
    pub fn close_all(&mut self, cx: &mut Context<Self>) -> Task<Vec<CloseReport>> {
        self.draining = true;
        let mut connections = Vec::new();
        for (_, instance) in std::mem::take(&mut self.instances) {
            // Dropping a Launching entry immediately cancels startup. A late Child
            // remains covered by the crate's launch ownership guard.
            if let Instance::Connected(connection) = instance {
                connections.push(connection);
            }
        }
        cx.notify();
        cx.spawn(async move |_, _| {
            let mut reports = Vec::new();
            for connection in connections {
                let report = connection.client.close().await;
                if let Some(error) = &report.reason {
                    tracing::warn!(%error, "Pi connection closed with an error");
                }
                reports.push(report);
                drop(connection);
            }
            reports
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{global, init};
    use gpui_kit as gpui;
    use gpui_kit::TestAppContext;
    use pi_rpc::ConnectionState;
    mod support {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../crates/pi-rpc/tests/support/mod.rs"
        ));
    }
    #[gpui::test]
    async fn two_instances_close_independently_then_drain_before_quit(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        cx.update(|cx| {
            gpui_tokio::init(cx);
            init(cx);
        });
        let owner = cx.update(|cx| global(cx));
        let first_dir = tempfile::tempdir().unwrap();
        let second_dir = tempfile::tempdir().unwrap();
        let first = owner.update(cx, |state, cx| {
            state
                .start(support::options(first_dir.path(), ""), cx)
                .unwrap()
        });
        let second = owner.update(cx, |state, cx| {
            state
                .start(support::options(second_dir.path(), ""), cx)
                .unwrap()
        });
        cx.condition(&owner, |state, _| {
            matches!(state.client(first), Ok(Some(_)))
                && matches!(state.client(second), Ok(Some(_)))
        })
        .await;
        let (first_client, second_client) = owner.read_with(cx, |state, _| {
            (
                state.client(first).unwrap().unwrap(),
                state.client(second).unwrap().unwrap(),
            )
        });
        assert_ne!(
            first_client.ready().await.unwrap().session_id,
            second_client.ready().await.unwrap().session_id
        );
        let report = owner
            .update(cx, |state, cx| state.close(first, cx))
            .await
            .unwrap();
        assert!(report.status.is_some());
        assert!(owner.read_with(cx, |state, _| state.client(first)).is_err());
        assert!(second_client.get_state().await.is_ok());
        // Add another connection so unified shutdown exercises sequential ownership.
        let third_dir = tempfile::tempdir().unwrap();
        let third = owner.update(cx, |state, cx| {
            state
                .start(support::options(third_dir.path(), ""), cx)
                .unwrap()
        });
        cx.condition(&owner, |state, _| {
            matches!(state.client(third), Ok(Some(_)))
        })
        .await;
        let third_client = owner.read_with(cx, |state, _| state.client(third).unwrap().unwrap());
        third_client.ready().await.unwrap();
        let draining = owner.update(cx, |state, cx| state.close_all(cx));
        assert!(
            owner
                .update(cx, |state, cx| state
                    .start(support::options(third_dir.path(), ""), cx))
                .is_err()
        );
        let reports = draining.await;
        assert_eq!(reports.len(), 2);
        assert!(
            reports
                .iter()
                .all(|report| report.status.is_some() && report.reason.is_none())
        );
        assert!(matches!(second_client.state(), ConnectionState::Closed(_)));
        assert!(matches!(third_client.state(), ConnectionState::Closed(_)));
    }
}
