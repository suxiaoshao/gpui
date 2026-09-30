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
pub(crate) struct ConnectionChanged(pub InstanceId);
impl EventEmitter<ConnectionChanged> for PiState {}
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
        mut options: LaunchOptions,
        cx: &mut Context<Self>,
    ) -> Result<InstanceId, Error> {
        if self.draining {
            return Err(Error::Closed);
        }
        self.next_id += 1;
        let id = InstanceId(self.next_id);
        let environment = super::environment::current(cx);
        let launch = Tokio::spawn(cx, async move {
            let snapshot = environment.load(false).await;
            if !options.clear_env {
                let mut variables = snapshot.variables();
                variables.append(&mut options.env);
                options.env = variables;
            }
            Client::spawn(options).await.map_err(|error| match error {
                Error::Io(error) => Error::Io(snapshot.explain(error)),
                error => error,
            })
        });
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
                cx.emit(ConnectionChanged(id));
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
            let mut events_open = true;
            loop {
                // Tokio channels can be awaited on GPUI's executor; process I/O stays
                // on the Tokio owner. Only one delivered event is held on this bridge.
                tokio::select! {
                    event = events.recv(), if events_open => {
                        let Some(event) = event else {
                            // The event sender can close before the final status
                            // notification. Keep observing until owners learn the
                            // connection has exited and can reconnect it.
                            events_open = false;
                            continue;
                        };
                        if owner.update(cx, |_, cx| {
                            cx.emit(PiEvent { instance: id, event });
                        }).is_err() { break; }
                    }
                    changed = status.changed() => {
                        if changed.is_err() { break; }
                        let exited = matches!(*status.borrow_and_update(), ConnectionState::Closed(_));
                        if owner.update(cx, |_, cx| { cx.emit(ConnectionChanged(id)); cx.notify(); }).is_err() { break; }
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
        cx.emit(ConnectionChanged(id));
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
                // close() signals the independent process owner synchronously.
                // Start every shutdown before waiting for any per-client deadline.
                let closing = connection.client.close();
                connections.push((connection, closing));
            }
        }
        cx.notify();
        cx.spawn(async move |_, _| {
            let mut reports = Vec::new();
            for (connection, closing) in connections {
                let report = closing.await;
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
