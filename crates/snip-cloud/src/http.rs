//! Cloudflare API replies. Non-2xx statuses are data, not transport errors.

use std::time::Duration;

use anyhow::anyhow;
use serde_json::Value;
use ureq::{Agent, Body, http::Response};

use crate::{Error, Result};

/// Cloudflare's code for an account without an R2 subscription.
const NOT_ENTITLED: i64 = 10042;
/// Larger than any snip; the REST upload limit is 300 MB.
const MAX_BODY: u64 = 300 * 1024 * 1024;

pub(crate) fn agent() -> Agent {
    Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(120)))
        .build()
        .into()
}

pub(crate) struct Reply {
    pub status: u16,
    pub body: Vec<u8>,
}

impl Reply {
    pub fn read(response: std::result::Result<Response<Body>, ureq::Error>) -> Result<Self> {
        let mut response = response?;
        let status = response.status().as_u16();
        let body = response.body_mut().with_config().limit(MAX_BODY).read_to_vec()?;
        Ok(Self { status, body })
    }

    pub fn succeeded(&self) -> bool {
        (200..300).contains(&self.status)
    }

    fn codes(&self) -> Vec<i64> {
        let envelope: Value = serde_json::from_slice(&self.body).unwrap_or(Value::Null);
        envelope["errors"]
            .as_array()
            .map(|errors| errors.iter().filter_map(|e| e["code"].as_i64()).collect())
            .unwrap_or_default()
    }

    /// A rejected bearer token, as opposed to a missing feature or resource.
    pub fn is_auth_failure(&self) -> bool {
        matches!(self.status, 401 | 403) && !self.codes().contains(&NOT_ENTITLED)
    }

    /// The failure as an error, naming Cloudflare's own message.
    pub fn error(&self, doing: &str) -> Error {
        if self.codes().contains(&NOT_ENTITLED) {
            return Error::R2NotEnabled;
        }
        let envelope: Value = serde_json::from_slice(&self.body).unwrap_or(Value::Null);
        let message = envelope["errors"][0]["message"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| String::from_utf8_lossy(&self.body).chars().take(200).collect());
        Error::Other(anyhow!(
            "{doing}: Cloudflare returned {} ({message})",
            self.status
        ))
    }

    /// The `result` of a successful Cloudflare API envelope.
    pub fn result(self, doing: &str) -> Result<Value> {
        if !self.succeeded() {
            return Err(self.error(doing));
        }
        let mut envelope: Value =
            serde_json::from_slice(&self.body).map_err(|e| anyhow!("{doing}: unreadable reply: {e}"))?;
        Ok(envelope["result"].take())
    }
}
