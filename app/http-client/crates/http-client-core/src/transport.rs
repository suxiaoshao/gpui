use std::{sync::Arc, time::Instant};

use async_channel::{Receiver, Sender};
use reqwest::{Client, redirect::Policy};

use super::{
    CompletedBody, ResponseHead, ResponseProgress, prepared::PreparedRequest,
    problem::RequestProblem,
};

mod body;
mod redirect;
mod worker;

const WORKER_EVENT_CAPACITY: usize = 8;

pub enum WorkerEvent {
    HeadReceived {
        head: ResponseHead,
        head_after: std::time::Duration,
        progress: ResponseProgress,
    },
    BodyProgress(ResponseProgress),
    Finished {
        result: Result<CompletedBody, RequestProblem>,
        finished_after: std::time::Duration,
    },
}

#[derive(Clone)]
pub struct HttpTransport {
    client: Result<Client, Arc<RequestProblem>>,
}

impl HttpTransport {
    pub fn new() -> Self {
        Self::from_builder(Client::builder())
    }

    fn from_builder(builder: reqwest::ClientBuilder) -> Self {
        let client = builder
            .redirect(Policy::none())
            .referer(false)
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .no_zstd()
            .build()
            .map_err(|error| Arc::new(RequestProblem::transport(error.without_url())));
        Self { client }
    }

    pub fn channel() -> (Sender<WorkerEvent>, Receiver<WorkerEvent>) {
        async_channel::bounded(WORKER_EVENT_CAPACITY)
    }

    /// Runs exactly one frozen request and emits exactly one terminal event
    /// unless its receiver has been dropped as part of cancellation.
    pub async fn run(self, prepared: PreparedRequest, sender: Sender<WorkerEvent>) {
        let started_at = Instant::now();
        let timeout = prepared.timeout;
        let result = match self.client {
            Ok(client) => {
                let attempt = worker::execute(prepared, client, &sender, started_at);
                match timeout {
                    Some(timeout) => match tokio::time::timeout(timeout, attempt).await {
                        Ok(result) => result,
                        Err(_) => Err(RequestProblem::timeout()),
                    },
                    None => attempt.await,
                }
            }
            Err(problem) => Err((*problem).clone()),
        };

        let _ = sender
            .send(WorkerEvent::Finished {
                result,
                finished_after: started_at.elapsed(),
            })
            .await;
    }
}

impl Default for HttpTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::ActiveBodyStorage;

    #[tokio::test]
    async fn full_progress_mailbox_drops_extra_progress_but_not_the_terminal_event() {
        let (sender, receiver) = HttpTransport::channel();
        let progress = ResponseProgress {
            declared_encoded_bytes: None,
            received_encoded_bytes: 1,
            stored_body_bytes: 1,
            storage: ActiveBodyStorage::Memory,
        };
        for _ in 0..WORKER_EVENT_CAPACITY {
            assert!(sender.try_send(WorkerEvent::BodyProgress(progress)).is_ok());
        }
        assert!(matches!(
            sender.try_send(WorkerEvent::BodyProgress(progress)),
            Err(async_channel::TrySendError::Full(_))
        ));

        let terminal = tokio::spawn(async move {
            assert!(
                sender
                    .send(WorkerEvent::Finished {
                        result: Err(RequestProblem::internal()),
                        finished_after: Duration::from_millis(1),
                    })
                    .await
                    .is_ok()
            );
        });
        tokio::task::yield_now().await;
        assert!(!terminal.is_finished());

        assert!(matches!(
            receiver.recv().await.unwrap(),
            WorkerEvent::BodyProgress(_)
        ));
        terminal.await.unwrap();

        let mut terminal_seen = false;
        while !receiver.is_empty() {
            if matches!(receiver.recv().await.unwrap(), WorkerEvent::Finished { .. }) {
                terminal_seen = true;
            }
        }
        assert!(terminal_seen);
    }
}
