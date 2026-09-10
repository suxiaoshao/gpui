use super::*;
use crate::state::conversation::Session;
use gpui_kit::component::{
    StyledExt,
    combobox::{Combobox, ComboboxEvent, ComboboxState},
    label::Label,
    searchable_list::{SearchableGroup, SearchableListItem, SearchableVec},
    tag::Tag,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Choice {
    Model { provider: String, id: String },
    Thinking(String),
}
#[derive(Clone, PartialEq, Eq)]
pub(super) struct OptionItem {
    value: Choice,
    title: SharedString,
    search: String,
    disabled: bool,
    provider: Option<String>,
    reasoning: bool,
    vision: bool,
}
impl SearchableListItem for OptionItem {
    type Value = Choice;
    fn title(&self) -> SharedString {
        self.title.clone()
    }
    fn display_title(&self) -> Option<AnyElement> {
        self.provider.as_ref().map(|provider| {
            h_flex()
                .min_w_0()
                .gap_2()
                .child(
                    crate::foundation::assets::provider_icon(provider)
                        .size_4()
                        .flex_none(),
                )
                .child(
                    Label::new(self.title())
                        .text_sm()
                        .whitespace_nowrap()
                        .truncate(),
                )
                .into_any_element()
        })
    }
    fn render(&self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        if let Some(provider) = &self.provider {
            let mut details = v_flex()
                .min_w_0()
                .gap_0p5()
                .child(Label::new(self.title()).text_sm().font_medium());
            if self.reasoning || self.vision {
                let mut tags = h_flex().gap_1().flex_wrap();
                if self.reasoning {
                    tags = tags.child(
                        Tag::secondary()
                            .small()
                            .outline()
                            .child(t(cx, "conversation-model-reasoning")),
                    );
                }
                if self.vision {
                    tags = tags.child(
                        Tag::secondary()
                            .small()
                            .outline()
                            .child(t(cx, "conversation-model-vision")),
                    );
                }
                details = details.child(tags);
            }
            h_flex()
                .min_w_0()
                .items_start()
                .gap_2()
                .child(
                    crate::foundation::assets::provider_icon(provider)
                        .size_4()
                        .flex_none()
                        .mt(px(1.))
                        .text_color(cx.theme().muted_foreground),
                )
                .child(details)
                .into_any_element()
        } else {
            Label::new(self.title()).truncate().into_any_element()
        }
    }
    fn value(&self) -> &Choice {
        &self.value
    }
    fn matches(&self, query: &str) -> bool {
        self.search.contains(&query.to_lowercase())
    }
    fn disabled(&self) -> bool {
        self.disabled
    }
}
type Items = SearchableVec<SearchableGroup<OptionItem>>;

#[derive(Default, PartialEq, Eq)]
pub(super) struct Projection {
    groups: Vec<(String, Vec<OptionItem>)>,
    selected: Option<Choice>,
}
pub(super) struct Picker {
    state: Entity<ComboboxState<Items>>,
    projection: Projection,
    _subscription: Subscription,
}
impl Picker {
    pub(super) fn new(
        key: String,
        searchable: bool,
        window: &mut Window,
        cx: &mut Context<HomeView>,
    ) -> Self {
        let owner = cx.weak_entity();
        Self::with_handler(
            searchable,
            move |value, cx| {
                let Ok(state) = owner.read_with(cx, |owner, _| owner.state.clone()) else {
                    return;
                };
                state.update(cx, |state, cx| match value {
                    Choice::Model { provider, id } => {
                        let model = state
                            .sessions
                            .get(&key)
                            .and_then(|s| {
                                s.models
                                    .iter()
                                    .find(|m| m.provider == provider && m.id == id)
                            })
                            .cloned();
                        if let Some(model) = model {
                            state.set_model(&key, model, cx);
                        }
                    }
                    Choice::Thinking(level) => state.set_thinking(&key, level, cx),
                });
            },
            window,
            cx,
        )
    }
    fn with_handler<Owner: 'static>(
        searchable: bool,
        on_change: impl Fn(Choice, &mut App) + 'static,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Self {
        let state = cx.new(|cx| {
            ComboboxState::new(
                SearchableVec::new(Vec::<SearchableGroup<OptionItem>>::new()),
                vec![],
                window,
                cx,
            )
            .searchable(searchable)
        });
        let on_change = Rc::new(on_change);
        let subscription = cx.subscribe_in(&state, window, move |_, state, event, window, cx| {
            // Closing or cancelling emits Confirm too; only an actual Change writes to Pi.
            let ComboboxEvent::Change(values) = event else {
                return;
            };
            let value = state
                .read(cx)
                .selection()
                .iter()
                .find(|(_, item)| !item.disabled && values.first() == Some(item.value()))
                .map(|(_, item)| item.value.clone());
            if let Some(value) = value {
                let on_change = on_change.clone();
                window.defer(cx, move |_, cx| on_change(value, cx));
            }
        });
        Self {
            state,
            projection: Projection::default(),
            _subscription: subscription,
        }
    }
    pub(super) fn sync(&mut self, projection: Projection, window: &mut Window, cx: &mut App) {
        if self.projection == projection {
            return;
        }
        let items = SearchableVec::new(
            projection
                .groups
                .iter()
                .map(|(title, items)| SearchableGroup::new(title.clone()).items(items.clone()))
                .collect::<Vec<_>>(),
        );
        self.state.update(cx, |state, cx| {
            let query = state.query(cx);
            state.set_items(items, window, cx);
            state.set_selected_values(
                &projection.selected.clone().into_iter().collect::<Vec<_>>(),
                window,
                cx,
            );
            state.set_query(query, window, cx);
        });
        self.projection = projection;
    }
    pub(super) fn element(&self) -> Combobox<Items> {
        Combobox::new(&self.state)
    }
}

pub(super) fn project(session: &Session, cx: &App) -> (Projection, Projection) {
    let disabled = session.busy() || session.operation.is_some() || !session.pending_ui.is_empty();
    let mut groups = BTreeMap::<String, Vec<OptionItem>>::new();
    for item in &session.models {
        groups
            .entry(item.provider.clone())
            .or_default()
            .push(OptionItem {
                value: Choice::Model {
                    provider: item.provider.clone(),
                    id: item.id.clone(),
                },
                title: item.name.clone().into(),
                search: format!("{} {} {}", item.provider, item.id, item.name).to_lowercase(),
                disabled,
                provider: Some(item.provider.clone()),
                reasoning: item.reasoning,
                vision: item
                    .extra
                    .get("input")
                    .and_then(|v| v.as_array())
                    .is_some_and(|values| values.iter().any(|v| v == "image")),
            });
    }
    for items in groups.values_mut() {
        items.sort_by(|a, b| a.title.cmp(&b.title));
    }
    let models = Projection {
        groups: groups.into_iter().collect(),
        selected: session
            .state
            .as_ref()
            .and_then(|s| s.model.as_ref())
            .map(|m| Choice::Model {
                provider: m.provider.clone(),
                id: m.id.clone(),
            }),
    };
    let thinking = Projection {
        groups: vec![(
            String::new(),
            session
                .thinking_levels
                .iter()
                .map(|level| {
                    let title = thinking_label(level, cx);
                    OptionItem {
                        value: Choice::Thinking(level.clone()),
                        search: title.to_lowercase(),
                        title: title.into(),
                        disabled,
                        provider: None,
                        reasoning: false,
                        vision: false,
                    }
                })
                .collect(),
        )],
        selected: session
            .state
            .as_ref()
            .map(|s| Choice::Thinking(s.thinking_level.clone())),
    };
    (models, thinking)
}

pub(super) fn thinking_label(level: &str, cx: &App) -> String {
    let key = match level {
        "off" => "conversation-thinking-off",
        "minimal" => "conversation-thinking-minimal",
        "low" => "conversation-thinking-low",
        "medium" => "conversation-thinking-medium",
        "high" => "conversation-thinking-high",
        "xhigh" => "conversation-thinking-xhigh",
        "max" => "conversation-thinking-max",
        _ => return level.to_owned(),
    };
    t(cx, key)
}

#[cfg(test)]
mod tests {
    use super::{Choice, OptionItem, Picker, Projection};
    use gpui_kit::component::combobox::ComboboxEvent;
    use gpui_kit::{
        AppContext as _, Context, IntoElement, Render, TestAppContext, VisualTestContext, Window,
        div,
    };
    use std::cell::Cell;
    use std::rc::Rc;

    struct Root {
        picker: Picker,
    }
    impl Render for Root {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }
    fn projection(disabled: bool, selected: &str) -> Projection {
        Projection {
            groups: vec![(
                "provider".into(),
                ["alpha", "beta"]
                    .into_iter()
                    .map(|id| OptionItem {
                        value: Choice::Thinking(id.into()),
                        title: id.into(),
                        search: id.into(),
                        disabled,
                        provider: None,
                        reasoning: false,
                        vision: false,
                    })
                    .collect(),
            )],
            selected: Some(Choice::Thinking(selected.into())),
        }
    }
    #[gpui_kit::test]
    fn refresh_preserves_search_and_domain_selection_without_writing_on_cancel(
        cx: &mut TestAppContext,
    ) {
        cx.update(gpui_kit::init);
        let writes = Rc::new(Cell::new(0));
        let counter = writes.clone();
        let window = cx
            .update(|cx| {
                cx.open_window(Default::default(), |window, cx| {
                    cx.new(|cx| Root {
                        picker: Picker::with_handler(
                            true,
                            move |_, _| counter.set(counter.get() + 1),
                            window,
                            cx,
                        ),
                    })
                })
            })
            .unwrap();
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let root = window.root(&mut cx).unwrap();
        cx.update(|window, cx| {
            root.update(cx, |root, cx| {
                root.picker.sync(projection(false, "beta"), window, cx);
                root.picker
                    .state
                    .update(cx, |state, cx| state.set_query("alpha", window, cx));
                root.picker.sync(projection(true, "beta"), window, cx);
                assert_eq!(root.picker.state.read(cx).query(cx).as_ref(), "alpha");
                assert_eq!(
                    root.picker.state.read(cx).selected_value(),
                    Some(Choice::Thinking("beta".into()))
                );
                root.picker.state.update(cx, |_, cx| {
                    cx.emit(ComboboxEvent::Confirm(vec![Choice::Thinking(
                        "beta".into(),
                    )]));
                    cx.emit(ComboboxEvent::Change(vec![Choice::Thinking("beta".into())]));
                });
            })
        });
        cx.run_until_parked();
        assert_eq!(writes.get(), 0);
        cx.update(|window, cx| {
            root.update(cx, |root, cx| {
                root.picker.sync(projection(false, "alpha"), window, cx);
                root.picker.state.update(cx, |_, cx| {
                    cx.emit(ComboboxEvent::Confirm(vec![Choice::Thinking(
                        "alpha".into(),
                    )]));
                });
            })
        });
        cx.run_until_parked();
        assert_eq!(writes.get(), 0);
        cx.update(|_, cx| {
            root.update(cx, |root, cx| {
                root.picker.state.update(cx, |_, cx| {
                    cx.emit(ComboboxEvent::Change(vec![Choice::Thinking(
                        "alpha".into(),
                    )]));
                });
            })
        });
        cx.run_until_parked();
        assert_eq!(writes.get(), 1);
    }
}
