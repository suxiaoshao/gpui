#![allow(dead_code)]

use gpui::{AnyElement, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window};
use gpui_component::{ActiveTheme, label::Label, v_flex};

#[derive(IntoElement)]
pub(super) struct ProviderListPane {
    body: AnyElement,
}

impl ProviderListPane {
    pub(super) fn new(body: impl IntoElement) -> Self {
        Self {
            body: body.into_any_element(),
        }
    }
}

impl RenderOnce for ProviderListPane {
    fn render(self, _: &mut Window, _: &mut gpui::App) -> impl IntoElement {
        self.body
    }
}

#[derive(IntoElement)]
pub(super) struct ProviderDetailPane {
    body: AnyElement,
}

impl ProviderDetailPane {
    pub(super) fn new(body: impl IntoElement) -> Self {
        Self {
            body: body.into_any_element(),
        }
    }
}

impl RenderOnce for ProviderDetailPane {
    fn render(self, _: &mut Window, _: &mut gpui::App) -> impl IntoElement {
        self.body
    }
}

#[derive(IntoElement)]
pub(super) struct ProviderFieldControl {
    body: AnyElement,
}

impl ProviderFieldControl {
    pub(super) fn new(body: impl IntoElement) -> Self {
        Self {
            body: body.into_any_element(),
        }
    }
}

impl RenderOnce for ProviderFieldControl {
    fn render(self, _: &mut Window, _: &mut gpui::App) -> impl IntoElement {
        self.body
    }
}

#[derive(IntoElement)]
pub(super) struct ProviderModelTable {
    body: AnyElement,
}

impl ProviderModelTable {
    pub(super) fn new(body: impl IntoElement) -> Self {
        Self {
            body: body.into_any_element(),
        }
    }
}

impl RenderOnce for ProviderModelTable {
    fn render(self, _: &mut Window, _: &mut gpui::App) -> impl IntoElement {
        self.body
    }
}

#[derive(IntoElement)]
pub(super) struct CapabilityTagRow {
    label: SharedString,
}

impl CapabilityTagRow {
    pub(super) fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
        }
    }
}

impl RenderOnce for CapabilityTagRow {
    fn render(self, _: &mut Window, cx: &mut gpui::App) -> impl IntoElement {
        v_flex()
            .rounded(cx.theme().radius)
            .bg(cx.theme().muted)
            .px_2()
            .py_1()
            .child(Label::new(self.label).text_xs())
    }
}
