use gpui_form::{FormSchema, ValidationMessage, ValidationRequest, ValidationSink};

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct FetchRequest {
    pub(super) url: String,
    pub(super) start_page: u32,
    pub(super) end_page: u32,
    pub(super) cookie: String,
}

impl std::fmt::Debug for FetchRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FetchRequest")
            .field("url", &self.url)
            .field("start_page", &self.start_page)
            .field("end_page", &self.end_page)
            .field("cookie_set", &!self.cookie.is_empty())
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq, FormSchema)]
pub(super) struct FetchDraft {
    #[form(required, validate(on_change, on_blur, on_submit))]
    pub(super) url: String,
    pub(super) start_page: u32,
    pub(super) end_page: u32,
    pub(super) cookie: String,
}

impl Default for FetchDraft {
    fn default() -> Self {
        Self {
            url: String::new(),
            start_page: 1,
            end_page: 1,
            cookie: String::new(),
        }
    }
}

impl From<FetchDraft> for FetchRequest {
    fn from(value: FetchDraft) -> Self {
        Self {
            url: value.url.trim().to_string(),
            start_page: value.start_page,
            end_page: value.end_page,
            cookie: value.cookie,
        }
    }
}

impl From<&FetchRequest> for FetchDraft {
    fn from(value: &FetchRequest) -> Self {
        Self {
            url: value.url.clone(),
            start_page: value.start_page,
            end_page: value.end_page,
            cookie: value.cookie.clone(),
        }
    }
}

pub(super) struct FetchValidator;

impl FetchValidator {
    pub(crate) fn rules(self) -> Vec<gpui_form::ValidationRule<FetchDraft>> {
        vec![
            gpui_form::ValidationRule::new("FetchValidator", move |request, out| {
                self.validate(request, out)
            })
            .dependency(FetchDraft::START_PAGE)
            .dependency(FetchDraft::END_PAGE),
        ]
    }
    fn validate(
        &self,
        request: ValidationRequest<'_, FetchDraft>,
        out: &mut ValidationSink<'_, FetchDraft>,
    ) {
        let model = request.model();
        if model.start_page > model.end_page {
            out.at(FetchDraft::END_PAGE).error(
                "page_range",
                ValidationMessage::key("fetch-error-invalid-page-range"),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_round_trip_preserves_cookie_and_page_range() {
        let request = FetchRequest {
            url: "https://example.test/list".to_owned(),
            start_page: 3,
            end_page: 8,
            cookie: "secret".to_owned(),
        };

        let restored = FetchDraft::from(&request);
        assert_eq!(FetchRequest::from(restored), request);
        assert!(!format!("{request:?}").contains("secret"));
    }
}

#[cfg(test)]
#[gpui_kit::test]
fn page_range_rechecks_both_inputs_and_ignores_cookie(cx: &mut gpui_kit::TestAppContext) {
    use gpui_kit::AppContext as _;
    cx.update(|cx| {
        let form = cx.new(|_| {
            gpui_form::Form::new(FetchDraft::default())
                .with_rules(FetchValidator.rules())
                .unwrap()
        });
        FetchDraft::START_PAGE.set(&form, 9, cx).unwrap();
        assert!(FetchDraft::END_PAGE.errors(&form, cx).is_empty());
        assert!(form.update(cx, |form, cx| form.prepare(cx)).is_err());
        assert_eq!(FetchDraft::END_PAGE.errors(&form, cx).len(), 1);
        FetchDraft::COOKIE.set(&form, "cookie".into(), cx).unwrap();
        assert_eq!(FetchDraft::END_PAGE.errors(&form, cx).len(), 1);
        FetchDraft::START_PAGE.set(&form, 1, cx).unwrap();
        assert!(FetchDraft::END_PAGE.errors(&form, cx).is_empty());
        FetchDraft::START_PAGE.set(&form, 9, cx).unwrap();
        FetchDraft::END_PAGE.set(&form, 10, cx).unwrap();
        assert!(FetchDraft::END_PAGE.errors(&form, cx).is_empty());
    });
}
