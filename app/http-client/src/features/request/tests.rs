use gpui::TestAppContext;

use super::*;
use crate::foundation::i18n::init_i18n;

fn initialize(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        init_i18n(cx);
    });
}

#[gpui::test]
fn page_prepare_uses_the_form_snapshot_without_rewriting_the_editor(cx: &mut TestAppContext) {
    initialize(cx);
    let (view, cx) = cx.add_window_view(RequestView::new);
    let form = cx.update(|_, cx| view.read(cx).form.clone());
    let original = " https://example.test/items?q=one#local-fragment ".to_owned();

    cx.update(|_, cx| RequestDraft::URL.set(&form, original.clone(), cx));
    let prepared = cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.prepare_request(cx)
                .expect("valid request must prepare")
        })
    });

    assert_eq!(prepared.url.as_str(), "https://example.test/items?q=one");
    cx.update(|_, cx| {
        assert_eq!(RequestDraft::URL.get(&form, cx), original);
    });
}

#[gpui::test]
fn page_prepare_reports_submit_errors_on_the_precise_url_path(cx: &mut TestAppContext) {
    initialize(cx);
    let (view, cx) = cx.add_window_view(RequestView::new);
    let form = cx.update(|_, cx| view.read(cx).form.clone());

    assert!(
        cx.update(|_, cx| view.update(cx, |view, cx| view.prepare_request(cx)))
            .is_err()
    );
    cx.update(|_, cx| {
        let issues = RequestDraft::URL.errors(&form, cx);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].code(), "required");
    });
}
