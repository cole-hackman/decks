//! The one seam between this crate and the network.
//!
//! Every provider takes an `Http` rather than reaching for a client itself.
//! That is what makes query construction, response parsing, rate limiting and
//! caching testable without a socket — and in this container it is the only
//! thing that makes them testable at all, because the network policy denies
//! `musicbrainz.org` (see `docs/lexicon/GAPS.md` §Environment blockers).
//!
//! It also keeps the promise in `CLAUDE.md` legible: the library leaves the
//! machine through exactly the calls that pass through this trait, and nowhere
//! else. A reviewer can check that claim by grepping for implementors.

use std::future::Future;

/// What a provider gets back from a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
}

impl Response {
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum HttpError {
    #[error("network: {0}")]
    Network(String),
    #[error("request timed out")]
    Timeout,
}

/// An HTTP GET, and nothing else.
///
/// Deliberately GET-only: every provider we integrate is a read, and a trait
/// that cannot POST cannot accidentally send a user's library anywhere.
pub trait Http: Send + Sync {
    fn get(
        &self,
        url: &str,
        headers: &[(&str, &str)],
    ) -> impl Future<Output = Result<Response, HttpError>> + Send;
}

#[cfg(any(test, feature = "test-util"))]
pub mod fake {
    //! An `Http` that answers from a table, for tests.

    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[derive(Default)]
    pub struct FakeHttp {
        routes: Mutex<HashMap<String, Response>>,
        /// Every URL requested, in order — so a test can assert that a cache
        /// hit really did skip the network rather than just returning the same
        /// answer twice.
        pub calls: Mutex<Vec<String>>,
    }

    impl FakeHttp {
        pub fn new() -> Self {
            Self::default()
        }

        /// Answer `url` with `body` and HTTP 200.
        pub fn route(self, url: &str, body: &str) -> Self {
            self.routes.lock().unwrap().insert(
                url.to_string(),
                Response {
                    status: 200,
                    body: body.as_bytes().to_vec(),
                },
            );
            self
        }

        pub fn route_status(self, url: &str, status: u16, body: &[u8]) -> Self {
            self.routes.lock().unwrap().insert(
                url.to_string(),
                Response {
                    status,
                    body: body.to_vec(),
                },
            );
            self
        }

        pub fn call_count(&self) -> usize {
            self.calls.lock().unwrap().len()
        }

        pub fn called(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl Http for FakeHttp {
        async fn get(&self, url: &str, _headers: &[(&str, &str)]) -> Result<Response, HttpError> {
            self.calls.lock().unwrap().push(url.to_string());
            match self.routes.lock().unwrap().get(url) {
                Some(r) => Ok(r.clone()),
                // An unrouted URL is a 404 rather than a panic: "the provider
                // asked for something we did not stub" is a normal outcome to
                // assert on, and several tests do exactly that.
                None => Ok(Response {
                    status: 404,
                    body: Vec::new(),
                }),
            }
        }
    }
}
