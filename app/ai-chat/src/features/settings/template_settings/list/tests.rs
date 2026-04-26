use super::filter_templates;
use crate::database::{ConversationTemplate, ConversationTemplatePrompt, Role};
use time::OffsetDateTime;

fn template(
    id: i32,
    name: &str,
    description: Option<&str>,
    prompt_count: usize,
) -> ConversationTemplate {
    ConversationTemplate {
        id,
        name: name.to_string(),
        icon: "🤖".to_string(),
        description: description.map(ToString::to_string),
        prompts: (0..prompt_count)
            .map(|_| ConversationTemplatePrompt {
                prompt: "hello".to_string(),
                role: Role::User,
            })
            .collect(),
        created_time: OffsetDateTime::UNIX_EPOCH,
        updated_time: OffsetDateTime::UNIX_EPOCH,
    }
}

#[test]
fn filter_templates_returns_all_for_blank_query() {
    let items = vec![
        template(1, "小说", None, 1),
        template(2, "命名助手", Some("生成更好的名字"), 2),
    ];

    let filtered = filter_templates(&items, "   ");

    assert_eq!(filtered.len(), 2);
    assert_eq!(filtered[0].id, 1);
    assert_eq!(filtered[1].id, 2);
}

#[test]
fn filter_templates_matches_name() {
    let items = vec![
        template(1, "小说", None, 1),
        template(2, "命名助手", Some("生成更好的名字"), 2),
    ];

    let filtered = filter_templates(&items, "命名");

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].id, 2);
}

#[test]
fn filter_templates_matches_description() {
    let items = vec![
        template(1, "小说", Some("写奇幻冒险故事"), 1),
        template(2, "命名助手", Some("生成更好的名字"), 2),
    ];

    let filtered = filter_templates(&items, "奇幻");

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].id, 1);
}

#[test]
fn filter_templates_trims_query_before_matching() {
    let items = vec![
        template(1, "小说", None, 1),
        template(2, "命名助手", Some("生成更好的名字"), 2),
    ];

    let filtered = filter_templates(&items, "  命名  ");

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].id, 2);
}

#[test]
fn filter_templates_matches_name_pinyin() {
    let items = vec![
        template(1, "小说", None, 1),
        template(2, "命名助手", Some("生成更好的名字"), 2),
    ];

    let filtered = filter_templates(&items, "mmzs");

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].id, 2);
}

#[test]
fn filter_templates_matches_description_pinyin() {
    let items = vec![
        template(1, "小说", Some("写奇幻冒险故事"), 1),
        template(2, "命名助手", Some("生成更好的名字"), 2),
    ];

    let filtered = filter_templates(&items, "shengcheng");

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].id, 2);
}
