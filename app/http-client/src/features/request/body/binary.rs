use gpui::{
    Context, Entity, IntoElement, ParentElement as _, Render, Styled as _, Window,
    prelude::FluentBuilder as _,
};
use gpui_component::{ActiveTheme as _, label::Label, v_flex};
use gpui_form::{DynamicPath, Form};

use crate::{
    features::request::{
        controls::{FilePathLabels, FormFilePathInput},
        draft::{BinaryBodyDraft, RequestDraft},
    },
    foundation::{I18n, i18n::validation_message},
};

pub(super) struct BinaryBodyView {
    form: Entity<Form<RequestDraft>>,
    file_path: DynamicPath<RequestDraft, Option<std::path::PathBuf>>,
    file: FormFilePathInput,
}

impl BinaryBodyView {
    pub(super) fn new(
        form: Entity<Form<RequestDraft>>,
        binary: DynamicPath<RequestDraft, BinaryBodyDraft>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let file_path = binary.then(BinaryBodyDraft::FILE);
        let file =
            FormFilePathInput::try_new(&form, file_path.clone(), file_labels(cx), window, cx)
                .expect("the Binary case was resolved immediately before its controls were built");
        Self {
            form,
            file_path,
            file,
        }
    }
}

impl Render for BinaryBodyView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let file_label = cx.global::<I18n>().t("field-file");
        let error = self
            .file_path
            .try_errors(&self.form, cx)
            .unwrap_or_default()
            .first()
            .map(|issue| validation_message(issue.message(), cx));
        v_flex()
            .p_2()
            .gap_2()
            .child(Label::new(file_label).text_sm())
            .child((*self.file).clone())
            .when_some(error, |this, error| {
                this.child(Label::new(error).text_xs().text_color(cx.theme().danger))
            })
    }
}

pub(super) fn file_labels(cx: &gpui::App) -> FilePathLabels {
    let i18n = cx.global::<I18n>();
    FilePathLabels {
        select: i18n.t("button-select-file").into(),
        change: i18n.t("button-change-file").into(),
        clear: i18n.t("button-clear-file").into(),
        empty: i18n.t("multipart-file-not-selected").into(),
    }
}
