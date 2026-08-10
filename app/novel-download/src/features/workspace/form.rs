use gpui_form::{
    FormSchema, Prepared, ValidationMessage, ValidationRequest, ValidationSink, Validator,
};

use crate::{
    crawler::source::{PreparedDownloadRequest, parse_download_source},
    errors::DownloadInputError,
};

/// The one editable submission draft owned by the workspace form.
#[derive(Clone, Debug, Default, PartialEq, Eq, FormSchema)]
pub(super) struct DownloadRequest {
    #[form(required)]
    pub(super) source: String,
}

pub(super) struct DownloadRequestValidator;

impl Validator<DownloadRequest> for DownloadRequestValidator {
    fn validate(
        &self,
        request: ValidationRequest<'_, DownloadRequest>,
        out: &mut ValidationSink<'_, DownloadRequest>,
    ) {
        if !request.includes(&DownloadRequest::SOURCE) || request.model().source.trim().is_empty() {
            return;
        }

        if parse_download_source(&request.model().source).is_err() {
            out.at(DownloadRequest::SOURCE).error(
                "download-validation-source-invalid",
                ValidationMessage::key("download-validation-source-invalid"),
            );
        }
    }
}

impl TryFrom<Prepared<DownloadRequest>> for PreparedDownloadRequest {
    type Error = DownloadInputError;

    fn try_from(prepared: Prepared<DownloadRequest>) -> Result<Self, Self::Error> {
        let (_, request) = prepared.into_parts();
        let submitted_source = request.source.trim().to_string();
        let source = parse_download_source(&submitted_source)?;
        Ok(Self::new(submitted_source, source))
    }
}

#[cfg(test)]
mod tests {
    use gpui::{AppContext as _, TestAppContext};
    use gpui_form::{Form, PrepareError, ValidationTrigger};

    use super::*;

    fn form(source: impl Into<String>, cx: &mut gpui::App) -> gpui::Entity<Form<DownloadRequest>> {
        let source = source.into();
        cx.new(|_| Form::new(DownloadRequest { source }).with_validator(DownloadRequestValidator))
    }

    #[gpui::test]
    fn source_validation_is_submit_only(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let form = form(String::new(), cx);

            assert!(DownloadRequest::SOURCE.errors(&form, cx).is_empty());

            DownloadRequest::SOURCE.set(&form, "unsupported-source".into(), cx);
            assert!(DownloadRequest::SOURCE.errors(&form, cx).is_empty());

            DownloadRequest::SOURCE.validate(&form, ValidationTrigger::Blur, cx);
            assert!(DownloadRequest::SOURCE.errors(&form, cx).is_empty());
        });
    }

    #[gpui::test]
    fn prepare_reports_only_required_for_blank_source(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let form = form("   ", cx);

            assert!(matches!(
                form.update(cx, |form, cx| form.prepare(cx)),
                Err(PrepareError::Validation(_))
            ));

            let errors = DownloadRequest::SOURCE.errors(&form, cx);
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code(), "required");
        });
    }

    #[gpui::test]
    fn prepare_reports_unsupported_source_at_the_source_field(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let form = form("https://example.com/info_otew/", cx);

            assert!(matches!(
                form.update(cx, |form, cx| form.prepare(cx)),
                Err(PrepareError::Validation(_))
            ));

            let errors = DownloadRequest::SOURCE.errors(&form, cx);
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code(), "download-validation-source-invalid");
        });
    }

    #[gpui::test]
    fn prepare_converts_each_supported_source_form(cx: &mut TestAppContext) {
        cx.update(|cx| {
            for source in [
                "otew",
                "https://m.zgzl.net/info_otew/#",
                "https://www.zgzl.net/info_qg6k",
                "https://m.zgzl.net/read_otew/68hq7.html",
                "https://m.zgzl.net/read_otew/68hq7_3.html",
            ] {
                let form = form(source, cx);
                let prepared = form
                    .update(cx, |form, cx| form.prepare(cx))
                    .expect("supported source must prepare");
                let request = PreparedDownloadRequest::try_from(prepared)
                    .expect("prepared source must convert with the same parser");

                assert_eq!(request.submitted_source(), source);
                assert_eq!(request.source(), &parse_download_source(source).unwrap());
            }

            let form = form("  otew  ", cx);
            let prepared = form.update(cx, |form, cx| form.prepare(cx)).unwrap();
            let request = PreparedDownloadRequest::try_from(prepared).unwrap();
            assert_eq!(request.submitted_source(), "otew");
        });
    }
}
