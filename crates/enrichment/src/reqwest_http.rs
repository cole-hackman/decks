//! The one implementor of [`crate::http::Http`] that opens a socket.
//!
//! Behind the `reqwest` feature so that the crate's own tests, and any caller
//! that supplies its own transport, never link an HTTP stack at all. Keeping it
//! inside this crate rather than in each consumer is what makes the network
//! surface enumerable: the entire outbound reach of enrichment is this file.

use std::time::Duration;

use crate::http::{Http, HttpError, Response};

pub struct ReqwestHttp {
    client: reqwest::Client,
}

impl ReqwestHttp {
    pub fn new() -> Result<Self, String> {
        let client = reqwest::Client::builder()
            // A hung provider must not hang the UI. Both halves matter: the
            // connect timeout covers a black-holed host, the overall timeout a
            // server that accepts and then stalls.
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self { client })
    }
}

impl Http for ReqwestHttp {
    async fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<Response, HttpError> {
        let mut req = self.client.get(url);
        for (k, v) in headers {
            req = req.header(*k, *v);
        }
        let res = req.send().await.map_err(|e| {
            if e.is_timeout() {
                HttpError::Timeout
            } else {
                HttpError::Network(e.to_string())
            }
        })?;
        let status = res.status().as_u16();
        let body = res
            .bytes()
            .await
            .map_err(|e| HttpError::Network(e.to_string()))?
            .to_vec();
        Ok(Response { status, body })
    }
}
