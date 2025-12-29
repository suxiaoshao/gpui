use std::ops::Deref;

use super::utils::serialize_offset_date_time;
use crate::{
    database::{
        Conversation, Message,
        model::{SqlConversation, SqlFolder, SqlMessage, SqlNewFolder, SqlUpdateFolder},
    },
    errors::{AiChatError, AiChatResult},
    store::{ChatData, ChatDataEvent},
    views::home::{AddConversation, AddFolder},
};
use diesel::SqliteConnection;
use gpui::*;
use gpui_component::{
    IconName, Sizable, WindowExt,
    button::{Button, ButtonVariants},
    form::{field, v_form},
    input::{Input, InputState},
    menu::DropdownMenu,
    sidebar::SidebarMenuItem,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Serialize, Debug)]
pub struct Folder {
    pub id: i32,
    pub name: String,
    pub path: String,
    #[serde(rename = "parentId")]
    pub parent_id: Option<i32>,
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
    pub conversations: Vec<Conversation>,
    pub folders: Vec<Folder>,
}

impl From<&Folder> for SidebarMenuItem {
    fn from(value: &Folder) -> Self {
        let parent_id = Some(value.id);
        let children = value
            .folders
            .iter()
            .map(SidebarMenuItem::from)
            .chain(value.conversations.iter().map(SidebarMenuItem::from));
        SidebarMenuItem::new(&value.name)
            .icon(IconName::Folder)
            .click_to_open(true)
            .children(children)
            .suffix(
                div()
                    .on_action(move |_: &AddFolder, window, cx| {
                        let folder_input = cx.new(|cx| InputState::new(window, cx));
                        window.open_dialog(cx, move |dialog, _window, _cx| {
                            dialog
                                .title("Add Folder")
                                .child(
                                    v_form().child(
                                        field().label("Name").child(Input::new(&folder_input)),
                                    ),
                                )
                                .footer({
                                    let folder_input = folder_input.clone();
                                    move |_this, _state, _window, _cx| {
                                        vec![
                                            Button::new("ok").primary().label("Submit").on_click({
                                                let folder_input = folder_input.clone();
                                                move |_, window, cx| {
                                                    let name =
                                                        folder_input.read(cx).value().to_string();
                                                    if !name.is_empty() {
                                                        let chat_data =
                                                            cx.global::<ChatData>().deref().clone();
                                                        chat_data.update(cx, |_this, cx| {
                                                            cx.emit(ChatDataEvent::AddFolder {
                                                                name,
                                                                parent_id,
                                                            });
                                                        });
                                                    }
                                                    window.close_dialog(cx);
                                                }
                                            }),
                                            Button::new("cancel").label("Cancel").on_click(
                                                |_, window, cx| {
                                                    window.close_dialog(cx);
                                                },
                                            ),
                                        ]
                                    }
                                })
                        });
                    })
                    .on_action(|_: &AddConversation, window, cx| {})
                    .child(
                        Button::new(value.id)
                            .icon(IconName::EllipsisVertical)
                            .ghost()
                            .xsmall()
                            .dropdown_menu(|this, _window, _cx| {
                                this.check_side(gpui_component::Side::Left)
                                    .menu_with_icon(
                                        "Add Conversation",
                                        IconName::Plus,
                                        Box::new(AddConversation),
                                    )
                                    .menu_with_icon(
                                        "Add Folder",
                                        IconName::Plus,
                                        Box::new(AddFolder),
                                    )
                            }),
                    ),
            )
    }
}

impl Folder {
    pub fn insert(
        NewFolder { name, parent_id }: NewFolder,
        conn: &mut SqliteConnection,
    ) -> AiChatResult<Self> {
        let now = OffsetDateTime::now_utc();
        let parent_folder = parent_id
            .map(|folder_id| SqlFolder::find(folder_id, conn))
            .transpose()?;
        let path = match parent_folder {
            Some(parent_folder) => format!("{}/{}", parent_folder.path, name),
            None => format!("/{name}"),
        };
        if SqlFolder::path_exists(&path, conn)? {
            return Err(AiChatError::FolderPathExists(path));
        }
        if SqlConversation::path_exists(&path, conn)? {
            return Err(AiChatError::ConversationPathExists(path));
        }
        let new_folder = SqlNewFolder {
            name,
            path,
            parent_id,
            created_time: now,
            updated_time: now,
        };
        let new_folder = new_folder.insert(conn)?;
        let folder = Self::from_sql_folder(new_folder, conn)?;
        Ok(folder)
    }
    pub fn update(
        id: i32,
        NewFolder { name, parent_id }: NewFolder,
        conn: &mut SqliteConnection,
    ) -> AiChatResult<()> {
        let now = OffsetDateTime::now_utc();
        let parent_folder = parent_id
            .map(|folder_id| SqlFolder::find(folder_id, conn))
            .transpose()?;
        let old_folder = SqlFolder::find(id, conn)?;
        let path = match parent_folder {
            Some(parent_folder) => format!("{}/{}", parent_folder.path, name),
            None => format!("/{name}"),
        };

        let path_updated = old_folder.path != path;
        if path_updated && SqlFolder::path_exists(&path, conn)? {
            return Err(AiChatError::FolderPathExists(path));
        }
        if SqlConversation::path_exists(&path, conn)? {
            return Err(AiChatError::ConversationPathExists(path));
        }
        let update_folder = SqlUpdateFolder {
            id,
            path: path.clone(),
            name,
            parent_id,
            updated_time: now,
        };
        conn.immediate_transaction::<_, AiChatError, _>(move |conn| {
            update_folder.update(conn)?;
            if path_updated {
                Self::update_path(&old_folder.path, &path, conn)?;
                Conversation::update_path(&old_folder.path, &path, conn)?;
                Message::update_path(&old_folder.path, &path, conn)?;
            }
            Ok(())
        })?;

        Ok(())
    }
    pub fn query(conn: &mut SqliteConnection) -> AiChatResult<Vec<Self>> {
        let sql_folders = SqlFolder::query(conn)?;
        sql_folders
            .into_iter()
            .map(|sql_folder| Self::from_sql_folder(sql_folder, conn))
            .collect()
    }
    fn from_sql_folder(
        SqlFolder {
            id,
            path,
            name,
            parent_id,
            created_time,
            updated_time,
        }: SqlFolder,
        conn: &mut SqliteConnection,
    ) -> AiChatResult<Self> {
        let conversations = Conversation::find_by_folder_id(id, conn)?;
        let folders = Self::find_by_parent_id(id, conn)?;
        Ok(Self {
            id,
            name,
            path,
            parent_id,
            created_time,
            updated_time,
            conversations,
            folders,
        })
    }
    fn find_by_parent_id(parent_id: i32, conn: &mut SqliteConnection) -> AiChatResult<Vec<Self>> {
        let sql_folders = SqlFolder::query_by_parent_id(parent_id, conn)?;
        sql_folders
            .into_iter()
            .map(|sql_folder| Self::from_sql_folder(sql_folder, conn))
            .collect()
    }
    pub fn delete_by_id(id: i32, conn: &mut SqliteConnection) -> AiChatResult<()> {
        conn.immediate_transaction::<_, AiChatError, _>(|conn| {
            let folder = SqlFolder::find(id, conn)?;
            SqlMessage::delete_by_path(&folder.path, conn)?;
            SqlConversation::delete_by_path(&folder.path, conn)?;
            SqlFolder::delete_by_path(&folder.path, conn)?;
            SqlFolder::delete_by_id(id, conn)?;
            Ok(())
        })?;
        Ok(())
    }
    pub fn update_path(
        old_path_pre: &str,
        new_path_pre: &str,
        conn: &mut SqliteConnection,
    ) -> AiChatResult<()> {
        let update_list = SqlFolder::find_by_path_pre(old_path_pre, conn)?;
        let time = OffsetDateTime::now_utc();
        update_list
            .iter()
            .map(|old| SqlUpdateFolder::from_new_path(old, old_path_pre, new_path_pre, time))
            .try_for_each(|update| {
                update.update(conn)?;
                Ok::<(), AiChatError>(())
            })?;
        Ok(())
    }
    pub fn move_folder(
        id: i32,
        new_parent_id: Option<i32>,
        conn: &mut SqliteConnection,
    ) -> AiChatResult<()> {
        let SqlFolder {
            parent_id: old_parent_id,
            mut path,
            ..
        } = SqlFolder::find(id, conn)?;
        let old_path = path.clone();
        let old_path_pre = match old_parent_id {
            Some(parent_id) => {
                let SqlFolder { path, .. } = SqlFolder::find(parent_id, conn)?;
                path
            }
            _ => "/".to_string(),
        };
        let new_path_pre = match new_parent_id {
            Some(parent_id) => {
                let SqlFolder { path, .. } = SqlFolder::find(parent_id, conn)?;
                path
            }
            _ => "/".to_string(),
        };
        path.replace_range(0..old_path_pre.len(), &new_path_pre);
        conn.immediate_transaction::<_, AiChatError, _>(|conn| {
            let time = OffsetDateTime::now_utc();
            SqlUpdateFolder::move_folder(id, new_parent_id, &path, time, conn)?;
            Self::update_path(&old_path, &path, conn)?;
            Conversation::update_path(&old_path, &path, conn)?;
            Message::update_path(&old_path, &path, conn)?;
            Ok(())
        })?;
        Ok(())
    }
}

#[derive(Deserialize)]
pub struct NewFolder<'a> {
    name: &'a str,
    #[serde(rename = "parentId")]
    parent_id: Option<i32>,
}

impl<'a> NewFolder<'a> {
    pub fn new(name: &'a str, parent_id: Option<i32>) -> Self {
        Self { name, parent_id }
    }
}
