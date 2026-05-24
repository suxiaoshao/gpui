use crate::{
    Result,
    error::DbError,
    models::{SqlNewSchemaMetadataRow, SqlNewSchemaMigrationRow},
    schema::{schema_metadata, schema_migrations},
};
use diesel::{
    connection::SimpleConnection, prelude::*, sql_query, sql_types::Text, upsert::excluded,
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

pub(crate) const SCHEMA_VERSION: i32 = 1;

pub(crate) struct Migration {
    pub(crate) name: &'static str,
    pub(crate) sql: &'static str,
}

pub(crate) const MIGRATIONS: &[Migration] = &[Migration {
    name: "0001_create_fresh_schema",
    sql: CREATE_FRESH_SCHEMA_SQL,
}];

const CREATE_FRESH_SCHEMA_SQL: &str = r#"
CREATE TABLE schema_migrations (
    name TEXT PRIMARY KEY,
    executed_at TEXT NOT NULL
);

CREATE TABLE schema_metadata (
    id TEXT PRIMARY KEY DEFAULT 'default',
    schema_version INTEGER NOT NULL,
    created_app_version TEXT,
    last_opened_app_version TEXT,
    payload_json JSON NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE projects (
    id TEXT PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('normal', 'scratch')),
    metadata_json JSON NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    last_opened_at TEXT
);

CREATE TABLE providers (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    display_name TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    settings_json JSON NOT NULL DEFAULT '{}',
    secret_refs_json JSON NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE prompts (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    content_json JSON NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE provider_models (
    id TEXT PRIMARY KEY,
    provider_id TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    model_id TEXT NOT NULL,
    display_name TEXT,
    capabilities_json JSON NOT NULL,
    metadata_json JSON NOT NULL DEFAULT '{}',
    fetched_at TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(provider_id, model_id)
);

CREATE TABLE conversations (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('active', 'archived', 'deleted')),
    prompt_id TEXT REFERENCES prompts(id) ON DELETE SET NULL,
    default_provider_id TEXT REFERENCES providers(id) ON DELETE SET NULL,
    default_model_id TEXT,
    last_item_seq INTEGER NOT NULL DEFAULT 0,
    metadata_json JSON NOT NULL DEFAULT '{}',
    settings_snapshot_json JSON NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    archived_at TEXT,
    deleted_at TEXT
);

CREATE TABLE attachments (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('image', 'file', 'audio', 'attachment')),
    storage_kind TEXT NOT NULL CHECK (storage_kind IN ('local_file', 'external_uri', 'provider_file', 'generated_file')),
    mime_type TEXT,
    name TEXT,
    path TEXT,
    external_uri TEXT,
    provider_id TEXT REFERENCES providers(id) ON DELETE SET NULL,
    provider_file_id TEXT,
    sha256 TEXT,
    size_bytes INTEGER,
    metadata_json JSON NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE agent_runs (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    trigger_kind TEXT NOT NULL CHECK (trigger_kind IN ('user', 'shortcut', 'resume', 'retry')),
    status TEXT NOT NULL,
    input_json JSON NOT NULL,
    output_json JSON,
    error_json JSON,
    created_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT,
    updated_at TEXT NOT NULL
);

CREATE TABLE provider_steps (
    id TEXT PRIMARY KEY,
    agent_run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
    seq INTEGER NOT NULL,
    provider_id TEXT NOT NULL REFERENCES providers(id),
    model_id TEXT NOT NULL,
    status TEXT NOT NULL,
    request_snapshot_json JSON NOT NULL,
    response_snapshot_json JSON,
    state_snapshot_json JSON,
    settings_snapshot_json JSON NOT NULL,
    error_json JSON,
    created_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT,
    updated_at TEXT NOT NULL,
    UNIQUE(agent_run_id, seq)
);

CREATE TABLE tool_invocations (
    id TEXT PRIMARY KEY,
    agent_run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
    provider_step_id TEXT REFERENCES provider_steps(id) ON DELETE SET NULL,
    call_id TEXT NOT NULL,
    source TEXT NOT NULL CHECK (source IN ('local', 'mcp', 'provider_hosted')),
    namespace TEXT,
    server_id TEXT,
    tool_name TEXT NOT NULL,
    runtime_tool_name TEXT NOT NULL,
    status TEXT NOT NULL,
    input_json JSON NOT NULL,
    output_json JSON,
    error_json JSON,
    created_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT,
    updated_at TEXT NOT NULL
);

CREATE TABLE conversation_items (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    seq INTEGER NOT NULL,
    kind TEXT NOT NULL,
    status TEXT NOT NULL,
    agent_run_id TEXT REFERENCES agent_runs(id) ON DELETE SET NULL,
    provider_step_id TEXT REFERENCES provider_steps(id) ON DELETE SET NULL,
    tool_invocation_id TEXT REFERENCES tool_invocations(id) ON DELETE SET NULL,
    provider_item_id TEXT,
    payload_json JSON NOT NULL,
    search_text TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(conversation_id, seq)
);

CREATE TABLE approval_decisions (
    id TEXT PRIMARY KEY,
    tool_invocation_id TEXT NOT NULL UNIQUE REFERENCES tool_invocations(id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK (status IN ('pending', 'approved', 'denied', 'expired', 'canceled')),
    request_json JSON NOT NULL,
    decision_json JSON,
    requested_at TEXT NOT NULL,
    decided_at TEXT,
    expires_at TEXT
);

CREATE TABLE usage_events (
    id TEXT PRIMARY KEY,
    provider_step_id TEXT NOT NULL REFERENCES provider_steps(id) ON DELETE CASCADE,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    provider_id TEXT NOT NULL REFERENCES providers(id),
    model_id TEXT NOT NULL,
    date_key TEXT NOT NULL,
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    cached_input_tokens INTEGER NOT NULL DEFAULT 0,
    cache_write_input_tokens INTEGER NOT NULL DEFAULT 0,
    reasoning_tokens INTEGER NOT NULL DEFAULT 0,
    total_tokens INTEGER NOT NULL DEFAULT 0,
    usage_json JSON NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE shortcuts (
    id TEXT PRIMARY KEY,
    hotkey TEXT NOT NULL UNIQUE,
    enabled INTEGER NOT NULL DEFAULT 1,
    prompt_id TEXT REFERENCES prompts(id) ON DELETE SET NULL,
    provider_id TEXT REFERENCES providers(id) ON DELETE SET NULL,
    model_id TEXT,
    input_source TEXT NOT NULL CHECK (input_source IN ('selection_or_clipboard', 'screenshot')),
    action_json JSON NOT NULL,
    settings_snapshot_json JSON NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE app_settings (
    id TEXT PRIMARY KEY DEFAULT 'default',
    settings_json JSON NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_conversations_project_id ON conversations(project_id);
CREATE INDEX idx_conversation_items_conversation_seq ON conversation_items(conversation_id, seq);
CREATE INDEX idx_agent_runs_conversation_id ON agent_runs(conversation_id);
CREATE INDEX idx_provider_steps_agent_seq ON provider_steps(agent_run_id, seq);
CREATE INDEX idx_tool_invocations_agent_run_id ON tool_invocations(agent_run_id);
CREATE INDEX idx_usage_events_conversation_date ON usage_events(conversation_id, date_key);
"#;

#[derive(diesel::QueryableByName)]
struct TextRow {
    #[diesel(sql_type = Text)]
    value: String,
}

pub(crate) fn bootstrap(conn: &mut SqliteConnection) -> Result<()> {
    bootstrap_with_migrations(conn, MIGRATIONS)
}

pub(crate) fn bootstrap_with_migrations(
    conn: &mut SqliteConnection,
    migrations: &[Migration],
) -> Result<()> {
    conn.batch_execute("PRAGMA foreign_keys = ON;")?;
    let applied = applied_migration_names(conn)?;
    for migration in migrations {
        if applied.iter().any(|name| name == migration.name) {
            continue;
        }
        conn.immediate_transaction(|conn| {
            conn.batch_execute(migration.sql)?;
            let executed_at = now_string()?;
            diesel::insert_into(schema_migrations::table)
                .values(&SqlNewSchemaMigrationRow {
                    name: migration.name.to_string(),
                    executed_at,
                })
                .execute(conn)?;
            Ok::<_, DbError>(())
        })?;
    }
    update_metadata(conn)
}

fn applied_migration_names(conn: &mut SqliteConnection) -> Result<Vec<String>> {
    if !table_exists(conn, "schema_migrations")? {
        return Ok(Vec::new());
    }
    Ok(schema_migrations::table
        .order(schema_migrations::name.asc())
        .select(schema_migrations::name)
        .load::<String>(conn)?)
}

fn table_exists(conn: &mut SqliteConnection, table_name: &str) -> Result<bool> {
    let rows = sql_query(
        "SELECT name AS value FROM sqlite_master WHERE type IN ('table', 'view') AND name = ? LIMIT 1",
    )
    .bind::<Text, _>(table_name)
    .load::<TextRow>(conn)?;
    Ok(rows.into_iter().any(|row| row.value == table_name))
}

fn update_metadata(conn: &mut SqliteConnection) -> Result<()> {
    if !table_exists(conn, "schema_metadata")? {
        return Err(DbError::Invariant(
            "schema_metadata was not created by migrations".to_string(),
        ));
    }
    let now = now_string()?;
    let payload = serde_json::json!({
        "storeKind": "fresh",
        "legacyPolicy": "ignore",
        "featureFlags": []
    });
    let row = SqlNewSchemaMetadataRow {
        id: "default".to_string(),
        schema_version: SCHEMA_VERSION,
        created_app_version: None,
        last_opened_app_version: None,
        payload_json: payload,
        created_at: now.clone(),
        updated_at: now,
    };
    diesel::insert_into(schema_metadata::table)
        .values(&row)
        .on_conflict(schema_metadata::id)
        .do_update()
        .set((
            schema_metadata::schema_version.eq(excluded(schema_metadata::schema_version)),
            schema_metadata::last_opened_app_version
                .eq(excluded(schema_metadata::last_opened_app_version)),
            schema_metadata::payload_json.eq(excluded(schema_metadata::payload_json)),
            schema_metadata::updated_at.eq(excluded(schema_metadata::updated_at)),
        ))
        .execute(conn)?;
    Ok(())
}

fn now_string() -> Result<String> {
    Ok(OffsetDateTime::now_utc().format(&Rfc3339)?)
}

#[cfg(test)]
pub(crate) fn broken_migration_for_test() -> Migration {
    Migration {
        name: "0001_broken",
        sql: "CREATE TABLE broken_rollback_probe (id TEXT PRIMARY KEY); INSERT INTO missing_table VALUES (1);",
    }
}
