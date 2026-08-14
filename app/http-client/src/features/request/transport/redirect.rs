use std::collections::HashSet;

use http::{HeaderMap, Method, StatusCode, header};
use url::Url;

use crate::features::request::prepared::PreparedRedirect;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RedirectError {
    InvalidLocation,
    Loop,
    HopLimit,
}

pub(super) struct RedirectState {
    policy: PreparedRedirect,
    visited: HashSet<String>,
    followed: u8,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct NextHop {
    pub(super) url: Url,
    pub(super) method: Method,
    pub(super) keep_body: bool,
}

impl RedirectState {
    pub(super) fn new(policy: PreparedRedirect, initial_url: &Url) -> Self {
        let mut visited = HashSet::new();
        visited.insert(loop_key(initial_url));
        Self {
            policy,
            visited,
            followed: 0,
        }
    }

    pub(super) fn next(
        &mut self,
        status: StatusCode,
        location: Option<&http::HeaderValue>,
        current_url: &Url,
        method: &Method,
        headers: &mut HeaderMap,
        body_available: bool,
    ) -> Result<Option<NextHop>, RedirectError> {
        if !self.policy.follow || !is_redirect_status(status) {
            return Ok(None);
        }
        let Some(location) = location else {
            return Ok(None);
        };
        if self.followed >= self.policy.max_hops {
            return Err(RedirectError::HopLimit);
        }

        let location = location
            .to_str()
            .map_err(|_| RedirectError::InvalidLocation)?;
        let mut url = current_url
            .join(location)
            .map_err(|_| RedirectError::InvalidLocation)?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            return Err(RedirectError::InvalidLocation);
        }
        url.set_fragment(None);

        if !self.visited.insert(loop_key(&url)) {
            return Err(RedirectError::Loop);
        }

        if !same_origin(current_url, &url) {
            headers.remove(header::HOST);
            headers.remove(header::COOKIE);
            if !self.policy.forward_authorization_cross_host {
                headers.remove(header::AUTHORIZATION);
            }
        }

        let (method, keep_body) =
            redirected_method(status, method, body_available, self.policy.preserve_method);
        self.followed += 1;
        Ok(Some(NextHop {
            url,
            method,
            keep_body,
        }))
    }
}

fn is_redirect_status(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::MOVED_PERMANENTLY
            | StatusCode::FOUND
            | StatusCode::SEE_OTHER
            | StatusCode::TEMPORARY_REDIRECT
            | StatusCode::PERMANENT_REDIRECT
    )
}

fn redirected_method(
    status: StatusCode,
    method: &Method,
    body_available: bool,
    preserve_method: bool,
) -> (Method, bool) {
    if preserve_method {
        return (method.clone(), body_available);
    }

    if matches!(
        status,
        StatusCode::MOVED_PERMANENTLY | StatusCode::FOUND | StatusCode::SEE_OTHER
    ) {
        let method = if *method == Method::HEAD {
            Method::HEAD
        } else {
            Method::GET
        };
        return (method, false);
    }
    (method.clone(), body_available)
}

fn same_origin(left: &Url, right: &Url) -> bool {
    left.scheme() == right.scheme()
        && left.host_str() == right.host_str()
        && left.port_or_known_default() == right.port_or_known_default()
}

fn loop_key(url: &Url) -> String {
    let mut url = url.clone();
    url.set_fragment(None);
    url.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::header;

    fn policy() -> PreparedRedirect {
        PreparedRedirect {
            follow: true,
            max_hops: 10,
            preserve_method: false,
            forward_authorization_cross_host: false,
        }
    }

    fn state(initial_url: &Url) -> RedirectState {
        RedirectState::new(policy(), initial_url)
    }

    #[test]
    fn relative_redirect_rewrites_post_and_strips_body() {
        let initial = Url::parse("http://example.test/start").unwrap();
        let mut state = state(&initial);
        let mut headers = HeaderMap::new();
        let next = state
            .next(
                StatusCode::FOUND,
                Some(&http::HeaderValue::from_static("../next#fragment")),
                &initial,
                &Method::POST,
                &mut headers,
                true,
            )
            .unwrap()
            .unwrap();

        assert_eq!(next.url.as_str(), "http://example.test/next");
        assert_eq!(next.method, Method::GET);
        assert!(!next.keep_body);
    }

    #[test]
    fn missing_location_is_a_final_response() {
        let initial = Url::parse("http://example.test/start").unwrap();
        let mut state = state(&initial);
        assert!(
            state
                .next(
                    StatusCode::FOUND,
                    None,
                    &initial,
                    &Method::GET,
                    &mut HeaderMap::new(),
                    false,
                )
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn cross_origin_removes_host_cookie_and_authorization_but_keeps_custom_headers() {
        let initial = Url::parse("http://example.test/start").unwrap();
        let mut state = state(&initial);
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, http::HeaderValue::from_static("example.test"));
        headers.insert(
            header::AUTHORIZATION,
            http::HeaderValue::from_static("manual"),
        );
        headers.insert(
            header::COOKIE,
            http::HeaderValue::from_static("session=secret"),
        );
        headers.insert("x-api-key", http::HeaderValue::from_static("ordinary-data"));
        headers.insert(
            "baidu-api-key",
            http::HeaderValue::from_static("also-ordinary-data"),
        );
        let next = state
            .next(
                StatusCode::TEMPORARY_REDIRECT,
                Some(&http::HeaderValue::from_static("http://other.test/next")),
                &initial,
                &Method::GET,
                &mut headers,
                false,
            )
            .unwrap()
            .unwrap();

        assert!(!headers.contains_key(header::HOST));
        assert!(!headers.contains_key(header::AUTHORIZATION));
        assert!(!headers.contains_key(header::COOKIE));
        assert_eq!(headers.get("x-api-key").unwrap(), "ordinary-data");
        assert_eq!(headers.get("baidu-api-key").unwrap(), "also-ordinary-data");
        let _ = state
            .next(
                StatusCode::TEMPORARY_REDIRECT,
                Some(&http::HeaderValue::from_static(
                    "http://example.test/finish",
                )),
                &next.url,
                &next.method,
                &mut headers,
                next.keep_body,
            )
            .unwrap();
        assert!(!headers.contains_key(header::AUTHORIZATION));
        assert!(!headers.contains_key(header::COOKIE));
        assert_eq!(headers.get("x-api-key").unwrap(), "ordinary-data");
    }

    #[test]
    fn same_origin_keeps_standard_and_custom_headers() {
        let initial = Url::parse("http://example.test/start").unwrap();
        let mut state = state(&initial);
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            http::HeaderValue::from_static("Bearer secret"),
        );
        headers.insert(
            header::COOKIE,
            http::HeaderValue::from_static("session=secret"),
        );
        headers.insert(
            "company-credential",
            http::HeaderValue::from_static("secret"),
        );

        state
            .next(
                StatusCode::TEMPORARY_REDIRECT,
                Some(&http::HeaderValue::from_static("/next")),
                &initial,
                &Method::GET,
                &mut headers,
                false,
            )
            .unwrap()
            .unwrap();

        assert_eq!(headers.get(header::AUTHORIZATION).unwrap(), "Bearer secret");
        assert_eq!(headers.get(header::COOKIE).unwrap(), "session=secret");
        assert_eq!(headers.get("company-credential").unwrap(), "secret");
    }

    #[test]
    fn explicit_cross_origin_authorization_forwarding_still_removes_cookie_and_host() {
        let initial = Url::parse("http://example.test/start").unwrap();
        let mut permissive = policy();
        permissive.forward_authorization_cross_host = true;
        let mut state = RedirectState::new(permissive, &initial);
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            http::HeaderValue::from_static("Bearer secret"),
        );
        headers.insert(
            header::COOKIE,
            http::HeaderValue::from_static("session=secret"),
        );
        headers.insert(header::HOST, http::HeaderValue::from_static("example.test"));

        state
            .next(
                StatusCode::TEMPORARY_REDIRECT,
                Some(&http::HeaderValue::from_static("http://other.test/next")),
                &initial,
                &Method::GET,
                &mut headers,
                false,
            )
            .unwrap()
            .unwrap();

        assert_eq!(headers.get(header::AUTHORIZATION).unwrap(), "Bearer secret");
        assert!(!headers.contains_key(header::COOKIE));
        assert!(!headers.contains_key(header::HOST));
    }

    #[test]
    fn default_method_policy_matches_postman() {
        let initial = Url::parse("http://example.test/start").unwrap();
        for status in [
            StatusCode::MOVED_PERMANENTLY,
            StatusCode::FOUND,
            StatusCode::SEE_OTHER,
        ] {
            for method in [Method::POST, Method::PUT, Method::PATCH, Method::DELETE] {
                let mut state = state(&initial);
                let next = state
                    .next(
                        status,
                        Some(&http::HeaderValue::from_static("/next")),
                        &initial,
                        &method,
                        &mut HeaderMap::new(),
                        true,
                    )
                    .unwrap()
                    .unwrap();
                assert_eq!(next.method, Method::GET);
                assert!(!next.keep_body);
            }
        }

        for status in [
            StatusCode::TEMPORARY_REDIRECT,
            StatusCode::PERMANENT_REDIRECT,
        ] {
            let mut state = state(&initial);
            let next = state
                .next(
                    status,
                    Some(&http::HeaderValue::from_static("/next")),
                    &initial,
                    &Method::PATCH,
                    &mut HeaderMap::new(),
                    true,
                )
                .unwrap()
                .unwrap();
            assert_eq!(next.method, Method::PATCH);
            assert!(next.keep_body);
        }
    }

    #[test]
    fn preserve_method_policy_keeps_method_and_body_for_every_redirect_status() {
        let initial = Url::parse("http://example.test/start").unwrap();
        let mut preserving = policy();
        preserving.preserve_method = true;
        for status in [
            StatusCode::MOVED_PERMANENTLY,
            StatusCode::FOUND,
            StatusCode::SEE_OTHER,
            StatusCode::TEMPORARY_REDIRECT,
            StatusCode::PERMANENT_REDIRECT,
        ] {
            let mut state = RedirectState::new(preserving, &initial);
            let next = state
                .next(
                    status,
                    Some(&http::HeaderValue::from_static("/next")),
                    &initial,
                    &Method::PATCH,
                    &mut HeaderMap::new(),
                    true,
                )
                .unwrap()
                .unwrap();
            assert_eq!(next.method, Method::PATCH);
            assert!(next.keep_body);
        }
    }

    #[test]
    fn redirect_loop_and_hop_limit_are_rejected() {
        let initial = Url::parse("http://example.test/start").unwrap();
        let mut state = state(&initial);
        assert_eq!(
            state.next(
                StatusCode::FOUND,
                Some(&http::HeaderValue::from_static("/start")),
                &initial,
                &Method::GET,
                &mut HeaderMap::new(),
                false,
            ),
            Err(RedirectError::Loop)
        );

        let mut limited = policy();
        limited.max_hops = 0;
        let mut state = RedirectState::new(limited, &initial);
        assert_eq!(
            state.next(
                StatusCode::FOUND,
                Some(&http::HeaderValue::from_static("/next")),
                &initial,
                &Method::GET,
                &mut HeaderMap::new(),
                false,
            ),
            Err(RedirectError::HopLimit)
        );
    }

    #[test]
    fn only_the_five_supported_statuses_redirect() {
        let initial = Url::parse("http://example.test/start").unwrap();
        for status in [
            StatusCode::MULTIPLE_CHOICES,
            StatusCode::NOT_MODIFIED,
            StatusCode::USE_PROXY,
        ] {
            let mut state = state(&initial);
            assert!(
                state
                    .next(
                        status,
                        Some(&http::HeaderValue::from_static("/next")),
                        &initial,
                        &Method::GET,
                        &mut HeaderMap::new(),
                        false,
                    )
                    .unwrap()
                    .is_none()
            );
        }
    }
}
