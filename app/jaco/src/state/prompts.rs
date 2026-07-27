use std::fmt;

use gpui::{App, AppContext, Task};
use gpui_operation::{Complete, Load, Refresh, Retry, Transition, refresh};
use gpui_store::{Select, Store};
use jaco_core::{PromptContent, PromptId};
use jaco_db::{DbError, NewPrompt, PromptRecord, UpdatePrompt};
use tokio::sync::oneshot;

use crate::{database, database::session::CatalogMutation};

const DEFAULT_SORT_ORDER_STEP: i32 = 10;

pub(crate) type PromptOperation = refresh::Operation<PromptData, PromptProblem, Task<()>>;
pub(crate) type PromptStore = Store<PromptOperation>;

#[derive(Clone, Copy, Default)]
pub(crate) struct SelectPromptRecords;

impl Select<PromptOperation> for SelectPromptRecords {
    type Output = Option<Vec<PromptRecord>>;

    fn select(&self, operation: &PromptOperation) -> Self::Output {
        operation.data().map(|data| data.prompts().to_vec())
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct SelectPromptStatus;

impl Select<PromptOperation> for SelectPromptStatus {
    type Output = (gpui_operation::refresh::Phase, Option<String>);

    fn select(&self, operation: &PromptOperation) -> Self::Output {
        (
            operation.phase(),
            operation.problem().map(ToString::to_string),
        )
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct PromptData {
    prompts: Vec<PromptRecord>,
}

impl PromptData {
    pub(crate) fn prompts(&self) -> &[PromptRecord] {
        &self.prompts
    }

    fn upsert(&mut self, prompt: PromptRecord) {
        match self
            .prompts
            .iter_mut()
            .find(|current| current.id == prompt.id)
        {
            Some(current) => *current = prompt,
            None => self.prompts.push(prompt),
        }
        sort_prompts(&mut self.prompts);
    }

    fn remove(&mut self, prompt_id: &PromptId) {
        self.prompts.retain(|prompt| &prompt.id != prompt_id);
    }
}

#[derive(Debug)]
pub(crate) struct PromptProblem(DbError);

impl fmt::Display for PromptProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for PromptProblem {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

pub(crate) enum PromptMessage {
    Upsert(PromptRecord),
    Remove(PromptId),
    Noop,
}

impl Transition<PromptMessage> for &mut PromptData {
    type Output = ();

    fn transition(self, message: PromptMessage) {
        match message {
            PromptMessage::Upsert(prompt) => self.upsert(prompt),
            PromptMessage::Remove(prompt_id) => self.remove(&prompt_id),
            PromptMessage::Noop => {}
        }
    }
}

fn sort_prompts(prompts: &mut [PromptRecord]) {
    prompts.sort_by(|left, right| {
        left.sort_order
            .cmp(&right.sort_order)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.created_at.cmp(&right.created_at))
            .then_with(|| left.id.cmp(&right.id))
    });
}

pub(crate) fn init(cx: &mut App) {
    PromptStore::install_global(cx, PromptOperation::new());
    let Some(binding) = database::ready_binding(cx) else {
        return;
    };
    let Ok(executor) = database::ready_executor(cx) else {
        return;
    };
    let task = cx.spawn(async move |cx| {
        let result = executor
            .execute(|repository| {
                repository.list_prompts().map(|mut prompts| {
                    sort_prompts(&mut prompts);
                    PromptData { prompts }
                })
            })
            .await
            .map_err(PromptProblem);
        cx.update(|cx| {
            if database::ready_binding(cx).as_ref() != Some(&binding) {
                return;
            }
            catalog(cx).update(cx, |operation| {
                if matches!(operation, PromptOperation::Loading(_)) {
                    operation.transition(Complete(result));
                }
            });
        });
    });
    catalog(cx).update(cx, |operation| operation.transition(Load(task)));
}

pub(crate) fn catalog(cx: &impl AppContext) -> PromptStore {
    PromptStore::global(cx)
}

pub(crate) fn request_refresh(cx: &mut App) {
    let Some(binding) = database::ready_binding(cx) else {
        return;
    };
    let Ok(executor) = database::ready_executor(cx) else {
        return;
    };
    if catalog(cx).read(cx, PromptOperation::is_running) {
        return;
    }
    let task = cx.spawn(async move |cx| {
        let result = executor
            .execute(|repository| {
                repository.list_prompts().map(|mut prompts| {
                    sort_prompts(&mut prompts);
                    PromptData { prompts }
                })
            })
            .await
            .map_err(PromptProblem);
        cx.update(|cx| {
            if database::ready_binding(cx).as_ref() != Some(&binding) {
                return;
            }
            catalog(cx).update(cx, |operation| {
                if operation.is_running() {
                    operation.transition(Complete(result));
                }
            });
        });
    });
    catalog(cx).update(cx, |operation| match operation {
        PromptOperation::Idle(_) => operation.transition(Load(task)),
        PromptOperation::Ready(_) | PromptOperation::Degraded(_) => {
            operation.transition(Refresh(task))
        }
        PromptOperation::Unavailable(_) => operation.transition(Retry(task)),
        PromptOperation::Loading(_)
        | PromptOperation::Refreshing(_)
        | PromptOperation::Retrying(_)
        | PromptOperation::RefreshingDegraded(_) => {}
    });
}

pub(crate) fn list_prompts(cx: &App) -> jaco_db::Result<Vec<PromptRecord>> {
    catalog(cx).read(cx, |operation| {
        operation
            .data()
            .map(|data| data.prompts.clone())
            .ok_or_else(|| DbError::Invariant("prompt resource is not ready".to_string()))
    })
}

pub(crate) fn create_prompt(
    cx: &mut App,
    name: String,
    text: String,
) -> Task<jaco_db::Result<PromptRecord>> {
    if let Err(error) = ensure_ready(cx) {
        return Task::ready(Err(error));
    }
    let sort_order = catalog(cx)
        .read(cx, |operation| {
            operation.data().and_then(|data| {
                data.prompts
                    .last()
                    .map(|prompt| prompt.sort_order + DEFAULT_SORT_ORDER_STEP)
            })
        })
        .unwrap_or(DEFAULT_SORT_ORDER_STEP);
    let command = move |repo: &jaco_db::FreshRepository| {
        repo.insert_prompt(NewPrompt {
            name,
            content: PromptContent { text },
            enabled: true,
            sort_order,
        })
    };
    spawn_prompt_mutation(cx, command, |prompt| PromptMessage::Upsert(prompt.clone()))
}

pub(crate) fn update_prompt(
    cx: &mut App,
    id: PromptId,
    name: String,
    text: String,
) -> Task<jaco_db::Result<PromptRecord>> {
    let command_id = id.clone();
    spawn_prompt_mutation(
        cx,
        move |repo| {
            let current = repo
                .get_prompt(&command_id)?
                .ok_or_else(|| DbError::Invariant(format!("prompt {command_id} is missing")))?;
            repo.update_prompt(
                &command_id,
                UpdatePrompt {
                    name,
                    content: PromptContent { text },
                    enabled: current.enabled,
                    sort_order: current.sort_order,
                },
            )
        },
        |prompt| PromptMessage::Upsert(prompt.clone()),
    )
}

pub(crate) fn delete_prompt(cx: &mut App, id: PromptId) -> Task<jaco_db::Result<usize>> {
    let command_id = id.clone();
    spawn_prompt_mutation(
        cx,
        move |repo| repo.delete_prompt(&command_id),
        move |deleted| {
            if *deleted > 0 {
                PromptMessage::Remove(id)
            } else {
                PromptMessage::Noop
            }
        },
    )
}

fn spawn_prompt_mutation<R>(
    cx: &mut App,
    command: impl FnOnce(&jaco_db::FreshRepository) -> jaco_db::Result<R> + Send + 'static,
    message: impl FnOnce(&R) -> PromptMessage + Send + 'static,
) -> Task<jaco_db::Result<R>>
where
    R: Send + 'static,
{
    if let Err(error) = ensure_ready(cx) {
        return Task::ready(Err(error));
    }
    let Some(binding) = database::ready_binding(cx) else {
        return Task::ready(Err(DbError::Invariant(
            "prompt mutation requires an exact Ready session".to_string(),
        )));
    };
    let executor = match database::ready_executor(cx) {
        Ok(executor) => executor,
        Err(error) => return Task::ready(Err(error)),
    };
    let (sender, receiver) = oneshot::channel();
    let task_binding = binding.clone();
    let driver = cx.spawn(async move |cx| {
        let result = executor.mutate(CatalogMutation::Prompt, command).await;
        if let Ok(value) = &result {
            let message = message(value);
            cx.update(|cx| {
                if database::ready_binding(cx).as_ref() == Some(&binding) {
                    apply(message, cx);
                }
            });
        }
        let _ = sender.send(result);
    });
    crate::app::session::retain_task(task_binding, driver, cx);
    cx.spawn(async move |_| {
        receiver.await.unwrap_or_else(|_| {
            Err(DbError::Invariant(
                "prompt mutation driver ended without a result".to_string(),
            ))
        })
    })
}

fn ensure_ready(cx: &App) -> jaco_db::Result<()> {
    catalog(cx).read(cx, |operation| {
        matches!(operation, PromptOperation::Ready(_))
            .then_some(())
            .ok_or_else(|| DbError::Invariant("prompt resource is not ready".to_string()))
    })
}

fn apply(message: PromptMessage, cx: &mut App) {
    catalog(cx).update(cx, |operation| {
        if let PromptOperation::Ready(ready) = operation {
            ready.transition(message);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{create_prompt, delete_prompt, init, list_prompts, update_prompt};
    use crate::database;
    use gpui::TestAppContext;

    #[gpui::test]
    fn prompt_catalog_tracks_committed_database_rows(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().expect("create temp dir");

        cx.update(|cx| {
            database::install_for_test(cx, dir.path());
            init(cx);
        });
        cx.run_until_parked();
        cx.update(|cx| {
            assert!(list_prompts(cx).expect("list initial prompts").is_empty());
        });

        let task = cx.update(|cx| {
            create_prompt(
                cx,
                "Write release notes".to_string(),
                "Summarize changes".to_string(),
            )
        });
        let prompt = cx
            .foreground_executor()
            .block_test(task)
            .expect("create prompt");
        cx.update(|cx| {
            assert_eq!(
                list_prompts(cx)
                    .expect("list created prompts")
                    .iter()
                    .map(|prompt| prompt.id.as_str())
                    .collect::<Vec<_>>(),
                vec![prompt.id.as_str()]
            );
        });

        let task = cx.update(|cx| {
            update_prompt(
                cx,
                prompt.id.clone(),
                "Write changelog".to_string(),
                "Summarize every change".to_string(),
            )
        });
        let updated = cx
            .foreground_executor()
            .block_test(task)
            .expect("update prompt");
        cx.update(|cx| {
            assert_eq!(
                list_prompts(cx)
                    .expect("list updated prompts")
                    .first()
                    .map(|prompt| (prompt.name.as_str(), prompt.content.text.as_str())),
                Some((updated.name.as_str(), updated.content.text.as_str()))
            );
        });

        let task = cx.update(|cx| delete_prompt(cx, prompt.id.clone()));
        assert_eq!(
            cx.foreground_executor()
                .block_test(task)
                .expect("delete prompt"),
            1
        );
        cx.update(|cx| {
            assert!(list_prompts(cx).expect("list deleted prompts").is_empty());
        });
    }
}
