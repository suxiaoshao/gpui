use std::time::Instant;

use duckdb::DuckdbConnectionManager;
use gpui::{AsyncApp, WeakEntity};
use gpui_operation::Transition;
use r2d2::PooledConnection;
use reqwest::Client;
use tracing::{Level, event};

use crate::{
    fetch::{self, FetchPageError},
    store::service::Novel,
};

use super::{FetchView, form::FetchRequest, run::FetchMessage};

pub(super) struct Runner<'a> {
    request: FetchRequest,
    owner: WeakEntity<FetchView>,
    conn: PooledConnection<DuckdbConnectionManager>,
    cx: &'a mut AsyncApp,
}

impl Runner<'_> {
    pub(super) fn new(
        request: FetchRequest,
        owner: WeakEntity<FetchView>,
        conn: PooledConnection<DuckdbConnectionManager>,
        cx: &mut AsyncApp,
    ) -> Runner<'_> {
        Runner {
            request,
            owner,
            conn,
            cx,
        }
    }

    pub(super) async fn run(&mut self) {
        event!(
            Level::INFO,
            start_page = self.request.start_page,
            end_page = self.request.end_page,
            has_cookie = !self.request.cookie.is_empty(),
            "fetch runner started"
        );
        let mut total = match Novel::count(&self.conn) {
            Ok(total) => total,
            Err(err) => {
                event!(
                    Level::ERROR,
                    error = %err,
                    "failed to count novels before fetch run"
                );
                self.mark_failed(FetchPageError::new(self.request.start_page, err), None);
                return;
            }
        };
        event!(Level::INFO, total, "initial novel count loaded");

        let client = Client::new();
        for page in self.request.start_page..=self.request.end_page {
            self.update_state(FetchMessage::PageStarted(page));
            let started_at = Instant::now();
            let novels =
                match fetch::fetch_page(&self.request.url, page, &self.request.cookie, &client)
                    .await
                {
                    Ok(novels) => novels,
                    Err(err) => {
                        event!(
                            Level::ERROR,
                            page,
                            error = %err,
                            elapsed_ms = started_at.elapsed().as_millis(),
                            "failed to fetch page data"
                        );
                        self.mark_failed(
                            FetchPageError::new(page, err),
                            Some(started_at.elapsed().as_millis()),
                        );
                        return;
                    }
                };

            let inserted = novels.len();
            for novel in novels {
                if let Err(err) = novel.save(&mut self.conn) {
                    event!(
                        Level::ERROR,
                        page,
                        error = %err,
                        elapsed_ms = started_at.elapsed().as_millis(),
                        "failed to save fetched novel"
                    );
                    self.mark_failed(
                        FetchPageError::new(page, err),
                        Some(started_at.elapsed().as_millis()),
                    );
                    return;
                }
                self.cx.update(crate::store::catalog::invalidate);
            }

            match Novel::count(&self.conn) {
                Ok(next_total) => total = next_total,
                Err(err) => {
                    event!(
                        Level::ERROR,
                        page,
                        error = %err,
                        elapsed_ms = started_at.elapsed().as_millis(),
                        "failed to count novels after page fetch"
                    );
                    self.mark_failed(
                        FetchPageError::new(page, err),
                        Some(started_at.elapsed().as_millis()),
                    );
                    return;
                }
            }
            self.update_state(FetchMessage::PageSucceeded {
                page,
                inserted,
                total,
                elapsed_ms: started_at.elapsed().as_millis(),
            });
        }

        event!(
            Level::INFO,
            start_page = self.request.start_page,
            end_page = self.request.end_page,
            total,
            "fetch runner completed"
        );
        self.update_state(FetchMessage::Succeeded);
    }

    fn mark_failed(&mut self, error: FetchPageError, elapsed_ms: Option<u128>) {
        event!(
            Level::ERROR,
            page = error.page,
            kind = %error.kind,
            message = %error.message,
            "Failed to fetch page"
        );
        self.update_state(FetchMessage::Failed { error, elapsed_ms });
    }

    fn update_state(&mut self, message: FetchMessage) {
        let _ = self.owner.update(self.cx, |view, cx| {
            view.task_state
                .update(cx, |state| state.transition(message));
        });
    }
}
