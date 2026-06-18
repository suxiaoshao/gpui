diesel::table! {
    schema_migrations (name) {
        name -> Text,
        executed_at -> TimestamptzSqlite,
    }
}

diesel::table! {
    schema_metadata (id) {
        id -> Text,
        schema_version -> Integer,
        created_app_version -> Nullable<Text>,
        last_opened_app_version -> Nullable<Text>,
        payload_json -> Json,
        created_at -> TimestamptzSqlite,
        updated_at -> TimestamptzSqlite,
    }
}

diesel::table! {
    projects (id) {
        id -> Text,
        path -> Text,
        display_name -> Text,
        kind -> Text,
        pinned -> Bool,
        removed -> Bool,
        metadata_json -> Json,
        created_at -> TimestamptzSqlite,
        updated_at -> TimestamptzSqlite,
        last_opened_at -> Nullable<TimestamptzSqlite>,
    }
}

diesel::table! {
    conversations (id) {
        id -> Text,
        project_id -> Text,
        title -> Text,
        status -> Text,
        pinned -> Bool,
        prompt_id -> Nullable<Text>,
        default_provider_id -> Nullable<Text>,
        default_model_id -> Nullable<Text>,
        last_item_seq -> Integer,
        metadata_json -> Json,
        settings_snapshot_json -> Json,
        created_at -> TimestamptzSqlite,
        updated_at -> TimestamptzSqlite,
        archived_at -> Nullable<TimestamptzSqlite>,
        deleted_at -> Nullable<TimestamptzSqlite>,
    }
}

diesel::table! {
    conversation_items (id) {
        id -> Text,
        conversation_id -> Text,
        seq -> Integer,
        kind -> Text,
        status -> Text,
        agent_run_id -> Nullable<Text>,
        provider_step_id -> Nullable<Text>,
        tool_invocation_id -> Nullable<Text>,
        provider_item_id -> Nullable<Text>,
        payload_json -> Json,
        search_text -> Text,
        created_at -> TimestamptzSqlite,
        updated_at -> TimestamptzSqlite,
    }
}

diesel::table! {
    attachments (id) {
        id -> Text,
        conversation_id -> Text,
        kind -> Text,
        storage_kind -> Text,
        mime_type -> Nullable<Text>,
        name -> Nullable<Text>,
        path -> Nullable<Text>,
        external_uri -> Nullable<Text>,
        provider_id -> Nullable<Text>,
        provider_file_id -> Nullable<Text>,
        sha256 -> Nullable<Text>,
        size_bytes -> Nullable<BigInt>,
        metadata_json -> Json,
        created_at -> TimestamptzSqlite,
        updated_at -> TimestamptzSqlite,
    }
}

diesel::table! {
    agent_runs (id) {
        id -> Text,
        conversation_id -> Text,
        trigger_kind -> Text,
        status -> Text,
        input_json -> Json,
        output_json -> Nullable<Json>,
        error_json -> Nullable<Json>,
        created_at -> TimestamptzSqlite,
        started_at -> Nullable<TimestamptzSqlite>,
        completed_at -> Nullable<TimestamptzSqlite>,
        updated_at -> TimestamptzSqlite,
    }
}

diesel::table! {
    provider_steps (id) {
        id -> Text,
        agent_run_id -> Text,
        seq -> Integer,
        provider_id -> Text,
        model_id -> Text,
        status -> Text,
        request_snapshot_json -> Json,
        response_snapshot_json -> Nullable<Json>,
        state_snapshot_json -> Nullable<Json>,
        settings_snapshot_json -> Json,
        error_json -> Nullable<Json>,
        created_at -> TimestamptzSqlite,
        started_at -> Nullable<TimestamptzSqlite>,
        completed_at -> Nullable<TimestamptzSqlite>,
        updated_at -> TimestamptzSqlite,
    }
}

diesel::table! {
    tool_invocations (id) {
        id -> Text,
        agent_run_id -> Text,
        provider_step_id -> Nullable<Text>,
        call_id -> Text,
        source -> Text,
        namespace -> Nullable<Text>,
        server_id -> Nullable<Text>,
        tool_name -> Text,
        runtime_tool_name -> Text,
        status -> Text,
        input_json -> Json,
        output_json -> Nullable<Json>,
        error_json -> Nullable<Json>,
        created_at -> TimestamptzSqlite,
        started_at -> Nullable<TimestamptzSqlite>,
        completed_at -> Nullable<TimestamptzSqlite>,
        updated_at -> TimestamptzSqlite,
    }
}

diesel::table! {
    approval_decisions (id) {
        id -> Text,
        tool_invocation_id -> Text,
        status -> Text,
        request_json -> Json,
        decision_json -> Nullable<Json>,
        requested_at -> TimestamptzSqlite,
        decided_at -> Nullable<TimestamptzSqlite>,
        expires_at -> Nullable<TimestamptzSqlite>,
    }
}

diesel::table! {
    usage_events (id) {
        id -> Text,
        provider_step_id -> Text,
        conversation_id -> Text,
        provider_id -> Text,
        model_id -> Text,
        date_key -> Text,
        input_tokens -> BigInt,
        output_tokens -> BigInt,
        cached_input_tokens -> BigInt,
        cache_write_input_tokens -> BigInt,
        reasoning_tokens -> BigInt,
        total_tokens -> BigInt,
        usage_json -> Json,
        created_at -> TimestamptzSqlite,
    }
}

diesel::table! {
    prompts (id) {
        id -> Text,
        name -> Text,
        content -> Text,
        enabled -> Bool,
        sort_order -> Integer,
        created_at -> TimestamptzSqlite,
        updated_at -> TimestamptzSqlite,
    }
}

diesel::table! {
    shortcuts (id) {
        id -> Text,
        hotkey -> Text,
        enabled -> Bool,
        prompt_id -> Nullable<Text>,
        provider_id -> Nullable<Text>,
        model_id -> Nullable<Text>,
        input_source -> Text,
        action_json -> Json,
        settings_snapshot_json -> Json,
        created_at -> TimestamptzSqlite,
        updated_at -> TimestamptzSqlite,
    }
}

diesel::table! {
    providers (id) {
        id -> Text,
        kind -> Text,
        display_name -> Text,
        enabled -> Bool,
        settings_json -> Json,
        secret_refs_json -> Json,
        created_at -> TimestamptzSqlite,
        updated_at -> TimestamptzSqlite,
    }
}

diesel::table! {
    provider_models (id) {
        id -> Text,
        provider_id -> Text,
        model_id -> Text,
        display_name -> Nullable<Text>,
        enabled -> Bool,
        capabilities_json -> Json,
        metadata_json -> Json,
        fetched_at -> TimestamptzSqlite,
        created_at -> TimestamptzSqlite,
        updated_at -> TimestamptzSqlite,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    agent_runs,
    approval_decisions,
    attachments,
    conversation_items,
    conversations,
    projects,
    prompts,
    provider_models,
    provider_steps,
    providers,
    schema_metadata,
    schema_migrations,
    shortcuts,
    tool_invocations,
    usage_events,
);
