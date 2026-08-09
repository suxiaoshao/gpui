use gpui_form::{FormSchema, ValidationMessage, ValidationRequest, ValidationSink, Validator};

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

impl Validator<FetchDraft> for FetchValidator {
    fn validate(
        &self,
        request: ValidationRequest<'_, FetchDraft>,
        out: &mut ValidationSink<'_, FetchDraft>,
    ) {
        let model = request.model();
        if request.includes(&FetchDraft::END_PAGE) && model.start_page > model.end_page {
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
