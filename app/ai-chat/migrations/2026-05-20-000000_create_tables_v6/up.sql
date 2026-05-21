create table folders
(
    id           INTEGER primary key autoincrement not null,
    name         TEXT                              not null,
    path         TEXT                              not null,
    parent_id    INTEGER,
    created_time DateTime                          not null,
    updated_time DateTime                          not null,
    unique (name, parent_id),
    unique (path),
    foreign key (parent_id) references folders (id)
);

CREATE TABLE conversation_templates
(
    id                    Integer PRIMARY KEY AUTOINCREMENT not null,
    name                  TEXT                              NOT NULL,
    icon                  TEXT                              not null,
    description           TEXT,
    prompts               JSON                              NOT NULL,
    required_capabilities JSON                              NOT NULL default '[]',
    created_time          DateTime                          not null,
    updated_time          DateTime                          not null
);

create table conversations
(
    id           INTEGER primary key autoincrement not null,
    folder_id    INTEGER,
    path         TEXT                              not null,
    title        TEXT                              not null,
    icon         TEXT                              not null,
    created_time DateTime                          not null,
    updated_time DateTime                          not null,
    info         TEXT,
    foreign key (folder_id) references folders (id),
    unique (path)
);

create table messages
(
    id                INTEGER primary key autoincrement not null,
    conversation_id   INTEGER                           not null,
    conversation_path TEXT                              not null,
    provider          TEXT                              not null,
    role              TEXT                              not null check ( role in ('developer', 'user', 'assistant') ),
    content           JSON                              not null,
    send_content      JSON                              not null,
    status            TEXT                              not null check ( status in ('normal', 'hidden', 'loading', 'thinking', 'paused', 'error') ),
    created_time      DateTime                          not null,
    updated_time      DateTime                          not null,
    start_time        DateTime                          not null,
    end_time          DateTime                          not null,
    error             TEXT,
    foreign key (conversation_id) references conversations (id)
);

create table message_run_states
(
    message_id            INTEGER  primary key not null,
    provider              TEXT                 not null,
    run_id                TEXT,
    output_item_ids       JSON                 not null default '[]',
    continuation_metadata JSON                 not null default 'null',
    request_body          JSON                 not null,
    usage                 JSON,
    model                 TEXT,
    settings              JSON,
    created_time          DateTime             not null,
    updated_time          DateTime             not null,
    foreign key (message_id) references messages (id) on delete cascade
);

create table message_output_items
(
    id               INTEGER primary key autoincrement not null,
    message_id       INTEGER                           not null,
    sequence         INTEGER                           not null,
    item_kind        TEXT                              not null,
    provider_item_id TEXT,
    status           TEXT                              not null,
    payload          JSON                              not null,
    created_time     DateTime                          not null,
    updated_time     DateTime                          not null,
    foreign key (message_id) references messages (id) on delete cascade,
    unique (message_id, sequence)
);

create table message_attachments
(
    id               INTEGER primary key autoincrement not null,
    message_id       INTEGER                           not null,
    attachment_id    TEXT                              not null,
    kind             TEXT                              not null,
    mime_type        TEXT,
    name             TEXT,
    metadata         JSON                              not null default '{}',
    external_uri     TEXT,
    path             TEXT,
    sha256           TEXT,
    created_time     DateTime                          not null,
    updated_time     DateTime                          not null,
    foreign key (message_id) references messages (id) on delete cascade,
    unique (message_id, attachment_id, kind)
);

create table global_shortcut_bindings
(
    id               INTEGER primary key autoincrement not null,
    hotkey           TEXT                              not null,
    enabled          BOOLEAN                           not null default 1 check ( enabled in (0, 1) ),
    template_id      INTEGER,
    provider_name    TEXT                              not null,
    model_id         TEXT                              not null,
    mode             TEXT                              not null check ( mode in ('contextual', 'single', 'assistant-only') ),
    request_template JSON                              not null default '{}',
    input_source     TEXT                              not null check ( input_source in ('selection_or_clipboard', 'screenshot') ),
    created_time     DateTime                          not null,
    updated_time     DateTime                          not null,
    foreign key (template_id) references conversation_templates (id),
    unique (hotkey)
);

CREATE INDEX idx_messages_conversation_id ON messages (conversation_id);

CREATE INDEX idx_messages_start_time ON messages (start_time);

CREATE INDEX idx_messages_status ON messages (status);

CREATE INDEX idx_messages_content ON messages (content);

CREATE INDEX idx_message_output_items_message_sequence ON message_output_items (message_id, sequence);

CREATE INDEX idx_message_output_items_kind ON message_output_items (item_kind);

CREATE INDEX idx_message_attachments_message_id ON message_attachments (message_id);
