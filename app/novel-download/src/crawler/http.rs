use std::{future::Future, num::NonZeroU8, time::Duration};

use reqwest::{StatusCode, Url, redirect};

use crate::errors::HttpProblem;

const ZGZL_HOST: &str = "m.zgzl.net";
const DEFAULT_ATTEMPTS: NonZeroU8 = NonZeroU8::new(3).expect("three is non-zero");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RetryPolicy {
    max_attempts: NonZeroU8,
    delay: Duration,
}

impl RetryPolicy {
    pub(crate) const fn new(max_attempts: NonZeroU8, delay: Duration) -> Self {
        Self {
            max_attempts,
            delay,
        }
    }

    const fn standard() -> Self {
        Self::new(DEFAULT_ATTEMPTS, Duration::from_secs(1))
    }
}

#[derive(Clone, Debug)]
pub(crate) struct HttpClient {
    client: reqwest::Client,
    retry: RetryPolicy,
}

impl HttpClient {
    pub(crate) fn new() -> Result<Self, HttpProblem> {
        let client = reqwest::Client::builder()
            .redirect(redirect_policy())
            .build()
            .map_err(|source| HttpProblem::new(root_url(), 1, source))?;

        Ok(Self {
            client,
            retry: RetryPolicy::standard(),
        })
    }

    pub(crate) async fn get_text(&self, url: &Url) -> Result<String, HttpProblem> {
        let client = self.client.clone();
        let request_url = url.clone();
        let result = retry_with(
            self.retry,
            is_transient_http_error,
            move || {
                let client = client.clone();
                let url = request_url.clone();
                async move {
                    client
                        .get(url)
                        .send()
                        .await?
                        .error_for_status()?
                        .text()
                        .await
                }
            },
            |delay| async move {
                smol::Timer::after(delay).await;
            },
        )
        .await;

        result.map_err(|failure| HttpProblem::new(url.clone(), failure.attempts, failure.error))
    }
}

fn redirect_policy() -> redirect::Policy {
    redirect_policy_for(is_allowed_redirect_target)
}

fn redirect_policy_for(
    is_allowed_target: impl Fn(&Url) -> bool + Send + Sync + 'static,
) -> redirect::Policy {
    let default_policy = redirect::Policy::default();
    redirect::Policy::custom(move |attempt| {
        if is_allowed_target(attempt.url()) {
            default_policy.redirect(attempt)
        } else {
            attempt.error("redirect target must remain https://m.zgzl.net")
        }
    })
}

fn is_allowed_redirect_target(target: &Url) -> bool {
    target.scheme() == "https" && target.host_str() == Some(ZGZL_HOST)
}

#[derive(Debug)]
struct RetryFailure<E> {
    attempts: u8,
    error: E,
}

async fn retry_with<T, E, Attempt, AttemptFuture, Sleep, SleepFuture>(
    policy: RetryPolicy,
    should_retry: impl Fn(&E) -> bool,
    mut attempt: Attempt,
    mut sleep: Sleep,
) -> Result<T, RetryFailure<E>>
where
    Attempt: FnMut() -> AttemptFuture,
    AttemptFuture: Future<Output = Result<T, E>>,
    Sleep: FnMut(Duration) -> SleepFuture,
    SleepFuture: Future<Output = ()>,
{
    for attempt_number in 1..=policy.max_attempts.get() {
        match attempt().await {
            Ok(value) => return Ok(value),
            Err(error) if attempt_number < policy.max_attempts.get() && should_retry(&error) => {
                sleep(policy.delay).await;
            }
            Err(error) => {
                return Err(RetryFailure {
                    attempts: attempt_number,
                    error,
                });
            }
        }
    }

    unreachable!("a non-zero retry policy always executes at least one attempt")
}

fn is_transient_http_error(error: &reqwest::Error) -> bool {
    error.is_timeout()
        || error.is_connect()
        || error.is_body()
        || error.status().is_some_and(is_transient_status)
}

fn is_transient_status(status: StatusCode) -> bool {
    status == StatusCode::REQUEST_TIMEOUT
        || status == StatusCode::TOO_MANY_REQUESTS
        || status.is_server_error()
}

fn root_url() -> Url {
    Url::parse("https://m.zgzl.net/").expect("fixed HTTPS URL is valid")
}

#[cfg(test)]
mod tests {
    use std::{
        cell::Cell,
        io::{Read, Write},
        net::TcpListener,
        num::NonZeroU8,
        rc::Rc,
        thread,
        time::Duration,
    };

    use async_compat::Compat;
    use reqwest::{StatusCode, Url};

    use super::{
        RetryPolicy, is_allowed_redirect_target, is_transient_status, redirect_policy_for,
        retry_with,
    };

    const DEFAULT_REDIRECT_LIMIT: usize = 10;

    #[test]
    fn retry_sleeps_only_between_transient_failures() {
        smol::block_on(async {
            let attempts = Rc::new(Cell::new(0));
            let sleeps = Rc::new(Cell::new(0));
            let attempts_for_closure = attempts.clone();
            let sleeps_for_closure = sleeps.clone();
            let policy = RetryPolicy::new(NonZeroU8::new(3).unwrap(), Duration::from_secs(1));

            let result = retry_with(
                policy,
                |_| true,
                move || {
                    let attempt = attempts_for_closure.get() + 1;
                    attempts_for_closure.set(attempt);
                    async move {
                        if attempt == 3 {
                            Ok::<_, &'static str>("done")
                        } else {
                            Err("transient")
                        }
                    }
                },
                move |_| {
                    sleeps_for_closure.set(sleeps_for_closure.get() + 1);
                    async {}
                },
            )
            .await;

            assert_eq!(result.unwrap(), "done");
            assert_eq!(attempts.get(), 3);
            assert_eq!(sleeps.get(), 2);
        });
    }

    #[test]
    fn retry_does_not_sleep_after_final_or_non_retryable_error() {
        smol::block_on(async {
            let final_sleeps = Rc::new(Cell::new(0));
            let final_sleeps_for_closure = final_sleeps.clone();
            let policy = RetryPolicy::new(NonZeroU8::new(3).unwrap(), Duration::from_secs(1));

            let result = retry_with(
                policy,
                |_| true,
                || async { Err::<(), _>("transient") },
                move |_| {
                    final_sleeps_for_closure.set(final_sleeps_for_closure.get() + 1);
                    async {}
                },
            )
            .await;

            assert_eq!(result.unwrap_err().attempts, 3);
            assert_eq!(final_sleeps.get(), 2);

            let non_retry_attempts = Rc::new(Cell::new(0));
            let non_retry_sleeps = Rc::new(Cell::new(0));
            let attempts_for_closure = non_retry_attempts.clone();
            let sleeps_for_closure = non_retry_sleeps.clone();
            let result = retry_with(
                policy,
                |_| false,
                move || {
                    attempts_for_closure.set(attempts_for_closure.get() + 1);
                    async { Err::<(), _>("permanent") }
                },
                move |_| {
                    sleeps_for_closure.set(sleeps_for_closure.get() + 1);
                    async {}
                },
            )
            .await;

            assert_eq!(result.unwrap_err().attempts, 1);
            assert_eq!(non_retry_attempts.get(), 1);
            assert_eq!(non_retry_sleeps.get(), 0);
        });
    }

    #[test]
    fn only_retryable_statuses_are_classified_as_transient() {
        for status in [
            StatusCode::REQUEST_TIMEOUT,
            StatusCode::TOO_MANY_REQUESTS,
            StatusCode::INTERNAL_SERVER_ERROR,
            StatusCode::BAD_GATEWAY,
        ] {
            assert!(is_transient_status(status), "{status}");
        }
        for status in [
            StatusCode::BAD_REQUEST,
            StatusCode::UNAUTHORIZED,
            StatusCode::NOT_FOUND,
        ] {
            assert!(!is_transient_status(status), "{status}");
        }
    }

    #[test]
    fn redirects_must_remain_https_on_m_zgzl_net() {
        for url in [
            "https://m.zgzl.net/chapter/1",
            "https://m.zgzl.net:443/chapter/1",
        ] {
            assert!(
                is_allowed_redirect_target(&Url::parse(url).unwrap()),
                "{url}"
            );
        }

        for url in [
            "http://m.zgzl.net/chapter/1",
            "https://zgzl.net/chapter/1",
            "https://www.m.zgzl.net/chapter/1",
            "https://m.zgzl.net.evil.example/chapter/1",
        ] {
            assert!(
                !is_allowed_redirect_target(&Url::parse(url).unwrap()),
                "{url}"
            );
        }
    }

    #[test]
    fn allowed_redirect_loop_stops_at_reqwests_default_limit() {
        let (url, server) = redirect_loop_server(DEFAULT_REDIRECT_LIMIT + 1);
        let client = reqwest::Client::builder()
            .redirect(redirect_policy_for(|_| true))
            .build()
            .unwrap();

        let result = smol::block_on(Compat::new(async { client.get(url).send().await }));

        assert!(result.unwrap_err().is_redirect());
        assert_eq!(server.join().unwrap(), DEFAULT_REDIRECT_LIMIT + 1);
    }

    fn redirect_loop_server(requests: usize) -> (Url, thread::JoinHandle<usize>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let url = Url::parse(&format!("http://{}/loop", listener.local_addr().unwrap())).unwrap();
        let server = thread::spawn(move || {
            for _ in 0..requests {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = [0; 1024];
                let request_bytes = stream.read(&mut request).unwrap();
                assert!(request_bytes > 0);
                stream
                    .write_all(
                        b"HTTP/1.1 302 Found\r\nLocation: /loop\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    )
                    .unwrap();
            }
            requests
        });

        (url, server)
    }
}
