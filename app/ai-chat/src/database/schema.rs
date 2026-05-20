// @generated automatically by Diesel CLI.

diesel::table! {
    conversation_templates (id) {
        id -> Integer,
        name -> Text,
        icon -> Text,
        description -> Nullable<Text>,
        prompts -> Json,
        created_time -> TimestamptzSqlite,
        updated_time -> TimestamptzSqlite,
    }
}

diesel::table! {
    conversations (id) {
        id -> Integer,
        folder_id -> Nullable<Integer>,
        path -> Text,
        title -> Text,
        icon -> Text,
        created_time -> TimestamptzSqlite,
        updated_time -> TimestamptzSqlite,
        info -> Nullable<Text>,
    }
}

diesel::table! {
    folders (id) {
        id -> Integer,
        name -> Text,
        path -> Text,
        parent_id -> Nullable<Integer>,
        created_time -> TimestamptzSqlite,
        updated_time -> TimestamptzSqlite,
    }
}

diesel::table! {
    global_shortcut_bindings (id) {
        id -> Integer,
        hotkey -> Text,
        enabled -> Bool,
        template_id -> Nullable<Integer>,
        provider_name -> Text,
        model_id -> Text,
        mode -> Text,
        request_template -> Json,
        input_source -> Text,
        created_time -> TimestamptzSqlite,
        updated_time -> TimestamptzSqlite,
    }
}

diesel::table! {
    messages (id) {
        id -> Integer,
        conversation_id -> Integer,
        conversation_path -> Text,
        provider -> Text,
        role -> Text,
        content -> Json,
        send_content -> Json,
        status -> Text,
        created_time -> TimestamptzSqlite,
        updated_time -> TimestamptzSqlite,
        start_time -> TimestamptzSqlite,
        end_time -> TimestamptzSqlite,
        error -> Nullable<Text>,
    }
}

diesel::table! {
    message_run_states (message_id) {
        message_id -> Integer,
        provider -> Text,
        run_id -> Nullable<Text>,
        output_item_ids -> Json,
        continuation_metadata -> Json,
        request_body -> Json,
        usage -> Nullable<Json>,
        model -> Nullable<Text>,
        settings -> Nullable<Json>,
        created_time -> TimestamptzSqlite,
        updated_time -> TimestamptzSqlite,
    }
}

diesel::table! {
    message_output_items (id) {
        id -> Integer,
        message_id -> Integer,
        sequence -> Integer,
        item_kind -> Text,
        provider_item_id -> Nullable<Text>,
        status -> Text,
        payload -> Json,
        created_time -> TimestamptzSqlite,
        updated_time -> TimestamptzSqlite,
    }
}

diesel::table! {
    message_attachments (id) {
        id -> Integer,
        message_id -> Integer,
        attachment_id -> Text,
        kind -> Text,
        mime_type -> Nullable<Text>,
        name -> Nullable<Text>,
        metadata -> Json,
        external_uri -> Nullable<Text>,
        path -> Nullable<Text>,
        sha256 -> Nullable<Text>,
        created_time -> TimestamptzSqlite,
        updated_time -> TimestamptzSqlite,
    }
}

diesel::joinable!(conversations -> folders (folder_id));
diesel::joinable!(global_shortcut_bindings -> conversation_templates (template_id));
diesel::joinable!(message_attachments -> messages (message_id));
diesel::joinable!(message_output_items -> messages (message_id));
diesel::joinable!(message_run_states -> messages (message_id));
diesel::joinable!(messages -> conversations (conversation_id));

diesel::allow_tables_to_appear_in_same_query!(
    conversation_templates,
    conversations,
    folders,
    global_shortcut_bindings,
    message_attachments,
    message_output_items,
    message_run_states,
    messages,
);
