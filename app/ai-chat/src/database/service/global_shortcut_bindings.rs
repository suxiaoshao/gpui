use super::utils::{deserialize_offset_date_time, serialize_offset_date_time};
use crate::{
    database::{
        Mode, ShortcutInputSource,
        model::{
            SqlGlobalShortcutBinding, SqlNewGlobalShortcutBinding, SqlUpdateGlobalShortcutBinding,
        },
    },
    errors::{AiChatError, AiChatResult},
};
use diesel::SqliteConnection;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct GlobalShortcutBinding {
    pub id: i32,
    pub hotkey: String,
    pub enabled: bool,
    #[serde(rename = "templateId")]
    pub template_id: Option<i32>,
    #[serde(rename = "providerName")]
    pub provider_name: String,
    #[serde(rename = "modelId")]
    pub model_id: String,
    pub mode: Mode,
    #[serde(rename = "requestTemplate")]
    pub request_template: serde_json::Value,
    #[serde(rename = "inputSource")]
    pub input_source: ShortcutInputSource,
    #[serde(
        rename = "createdTime",
        serialize_with = "serialize_offset_date_time",
        deserialize_with = "deserialize_offset_date_time"
    )]
    pub created_time: OffsetDateTime,
    #[serde(
        rename = "updatedTime",
        serialize_with = "serialize_offset_date_time",
        deserialize_with = "deserialize_offset_date_time"
    )]
    pub updated_time: OffsetDateTime,
}

impl TryFrom<SqlGlobalShortcutBinding> for GlobalShortcutBinding {
    type Error = AiChatError;

    fn try_from(value: SqlGlobalShortcutBinding) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value.id,
            hotkey: value.hotkey,
            enabled: value.enabled,
            template_id: value.template_id,
            provider_name: value.provider_name,
            model_id: value.model_id,
            mode: value.mode.parse()?,
            request_template: value.request_template,
            input_source: value.input_source.parse()?,
            created_time: value.created_time,
            updated_time: value.updated_time,
        })
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct NewGlobalShortcutBinding {
    pub hotkey: String,
    pub enabled: bool,
    #[serde(rename = "templateId")]
    pub template_id: Option<i32>,
    #[serde(rename = "providerName")]
    pub provider_name: String,
    #[serde(rename = "modelId")]
    pub model_id: String,
    pub mode: Mode,
    #[serde(rename = "requestTemplate")]
    pub request_template: serde_json::Value,
    #[serde(rename = "inputSource")]
    pub input_source: ShortcutInputSource,
}

impl GlobalShortcutBinding {
    pub fn find(id: i32, conn: &mut SqliteConnection) -> AiChatResult<Self> {
        SqlGlobalShortcutBinding::find(id, conn)?.try_into()
    }

    pub fn all(conn: &mut SqliteConnection) -> AiChatResult<Vec<Self>> {
        SqlGlobalShortcutBinding::all(conn)?
            .into_iter()
            .map(TryInto::try_into)
            .collect()
    }

    pub fn insert(
        new_binding: NewGlobalShortcutBinding,
        conn: &mut SqliteConnection,
    ) -> AiChatResult<Self> {
        let now = OffsetDateTime::now_utc();
        let sql_new = SqlNewGlobalShortcutBinding {
            hotkey: &new_binding.hotkey,
            enabled: new_binding.enabled,
            template_id: new_binding.template_id,
            provider_name: &new_binding.provider_name,
            model_id: &new_binding.model_id,
            mode: &new_binding.mode.to_string(),
            request_template: &new_binding.request_template,
            input_source: &new_binding.input_source.to_string(),
            created_time: now,
            updated_time: now,
        };
        sql_new.insert(conn)?.try_into()
    }

    pub fn update(
        id: i32,
        update: UpdateGlobalShortcutBinding,
        conn: &mut SqliteConnection,
    ) -> AiChatResult<()> {
        let sql_update = SqlUpdateGlobalShortcutBinding {
            id,
            hotkey: &update.hotkey,
            enabled: update.enabled,
            template_id: update.template_id,
            provider_name: &update.provider_name,
            model_id: &update.model_id,
            mode: &update.mode.to_string(),
            request_template: &update.request_template,
            input_source: &update.input_source.to_string(),
            updated_time: OffsetDateTime::now_utc(),
        };
        sql_update.update(conn)
    }

    pub fn delete(id: i32, conn: &mut SqliteConnection) -> AiChatResult<()> {
        SqlGlobalShortcutBinding::delete_by_id(id, conn)
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct UpdateGlobalShortcutBinding {
    pub hotkey: String,
    pub enabled: bool,
    #[serde(rename = "templateId")]
    pub template_id: Option<i32>,
    #[serde(rename = "providerName")]
    pub provider_name: String,
    #[serde(rename = "modelId")]
    pub model_id: String,
    pub mode: Mode,
    #[serde(rename = "requestTemplate")]
    pub request_template: serde_json::Value,
    #[serde(rename = "inputSource")]
    pub input_source: ShortcutInputSource,
}

#[cfg(test)]
mod tests {
    use super::{GlobalShortcutBinding, NewGlobalShortcutBinding, UpdateGlobalShortcutBinding};
    use crate::{
        database::{
            CREATE_TABLE_SQL, Mode, ShortcutInputSource,
            model::SqlNewConversationTemplate,
        },
        errors::AiChatError,
    };
    use diesel::{
        Connection, RunQueryDsl, SqliteConnection, connection::SimpleConnection, sql_query,
    };

    fn setup_conn() -> anyhow::Result<SqliteConnection> {
        let mut conn = SqliteConnection::establish(":memory:")?;
        conn.batch_execute(CREATE_TABLE_SQL)?;
        Ok(conn)
    }

    fn insert_template(conn: &mut SqliteConnection) -> anyhow::Result<i32> {
        let template = SqlNewConversationTemplate::default()?.insert(conn)?;
        Ok(template.id)
    }

    #[test]
    fn global_shortcut_binding_roundtrips_supported_input_sources() -> anyhow::Result<()> {
        let mut conn = setup_conn()?;
        let template_id = insert_template(&mut conn)?;

        let first = GlobalShortcutBinding::insert(
            NewGlobalShortcutBinding {
                hotkey: "super+shift+1".to_string(),
                enabled: true,
                template_id: Some(template_id),
                provider_name: "OpenAI".to_string(),
                model_id: "gpt-4o".to_string(),
                mode: Mode::Contextual,
                request_template: serde_json::json!({"model":"gpt-4o"}),
                input_source: ShortcutInputSource::SelectionOrClipboard,
            },
            &mut conn,
        )?;
        let second = GlobalShortcutBinding::insert(
            NewGlobalShortcutBinding {
                hotkey: "super+shift+2".to_string(),
                enabled: false,
                template_id: None,
                provider_name: "OpenAI".to_string(),
                model_id: "gpt-4.1".to_string(),
                mode: Mode::Single,
                request_template: serde_json::json!({"model":"gpt-4.1"}),
                input_source: ShortcutInputSource::Screenshot,
            },
            &mut conn,
        )?;

        let bindings = GlobalShortcutBinding::all(&mut conn)?;
        assert_eq!(bindings.len(), 2);
        assert!(bindings.contains(&first));
        assert!(bindings.contains(&second));
        Ok(())
    }

    #[test]
    fn global_shortcut_binding_hotkey_is_unique() -> anyhow::Result<()> {
        let mut conn = setup_conn()?;

        let _ = GlobalShortcutBinding::insert(
            NewGlobalShortcutBinding {
                hotkey: "super+shift+k".to_string(),
                enabled: true,
                template_id: None,
                provider_name: "OpenAI".to_string(),
                model_id: "gpt-4o".to_string(),
                mode: Mode::Contextual,
                request_template: serde_json::json!({"model":"gpt-4o"}),
                input_source: ShortcutInputSource::SelectionOrClipboard,
            },
            &mut conn,
        )?;

        let err = GlobalShortcutBinding::insert(
            NewGlobalShortcutBinding {
                hotkey: "super+shift+k".to_string(),
                enabled: false,
                template_id: None,
                provider_name: "OpenAI".to_string(),
                model_id: "gpt-4.1".to_string(),
                mode: Mode::Single,
                request_template: serde_json::json!({"model":"gpt-4.1"}),
                input_source: ShortcutInputSource::Screenshot,
            },
            &mut conn,
        )
        .unwrap_err();

        assert!(matches!(err, AiChatError::Sqlite(_)));
        Ok(())
    }

    #[test]
    fn global_shortcut_binding_supports_find_update_and_delete() -> anyhow::Result<()> {
        let mut conn = setup_conn()?;
        let inserted = GlobalShortcutBinding::insert(
            NewGlobalShortcutBinding {
                hotkey: "super+shift+u".to_string(),
                enabled: true,
                template_id: None,
                provider_name: "OpenAI".to_string(),
                model_id: "gpt-4o".to_string(),
                mode: Mode::Contextual,
                request_template: serde_json::json!({"model":"gpt-4o"}),
                input_source: ShortcutInputSource::SelectionOrClipboard,
            },
            &mut conn,
        )?;

        let found = GlobalShortcutBinding::find(inserted.id, &mut conn)?;
        assert_eq!(found, inserted);

        GlobalShortcutBinding::update(
            inserted.id,
            UpdateGlobalShortcutBinding {
                hotkey: "super+shift+y".to_string(),
                enabled: false,
                template_id: None,
                provider_name: "OpenAI".to_string(),
                model_id: "gpt-4.1".to_string(),
                mode: Mode::Single,
                request_template: serde_json::json!({"model":"gpt-4.1"}),
                input_source: ShortcutInputSource::Screenshot,
            },
            &mut conn,
        )?;

        let updated = GlobalShortcutBinding::find(inserted.id, &mut conn)?;
        assert_eq!(updated.hotkey, "super+shift+y");
        assert!(!updated.enabled);
        assert_eq!(updated.model_id, "gpt-4.1");
        assert_eq!(updated.mode, Mode::Single);
        assert_eq!(updated.input_source, ShortcutInputSource::Screenshot);

        GlobalShortcutBinding::delete(inserted.id, &mut conn)?;
        assert!(GlobalShortcutBinding::all(&mut conn)?.is_empty());
        Ok(())
    }

    #[test]
    fn global_shortcut_binding_rejects_invalid_mode_and_input_source() -> anyhow::Result<()> {
        let mut conn = setup_conn()?;

        let invalid_mode = sql_query(
            "insert into global_shortcut_bindings
             (hotkey, enabled, template_id, provider_name, model_id, mode, request_template, input_source, created_time, updated_time)
             values
             ('cmd-1', 1, null, 'OpenAI', 'gpt-4o', 'invalid', '{}', 'selection_or_clipboard', '2026-01-01 00:00:00+00:00', '2026-01-01 00:00:00+00:00')",
        )
        .execute(&mut conn);
        assert!(invalid_mode.is_err());

        let invalid_source = sql_query(
            "insert into global_shortcut_bindings
             (hotkey, enabled, template_id, provider_name, model_id, mode, request_template, input_source, created_time, updated_time)
             values
             ('cmd-2', 1, null, 'OpenAI', 'gpt-4o', 'contextual', '{}', 'invalid', '2026-01-01 00:00:00+00:00', '2026-01-01 00:00:00+00:00')",
        )
        .execute(&mut conn);
        assert!(invalid_source.is_err());
        Ok(())
    }
}
