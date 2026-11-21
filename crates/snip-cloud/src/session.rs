//! Sign in with Cloudflare (Authorization Code with PKCE through a loopback
//! redirect) and the token lifecycle. Every refresh rotates the refresh token
//! and revokes the previous access token, so one session hands out a single
//! current access token and reports each new refresh token for saving.

use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context as _, anyhow};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::Value;
use sha2::{Digest, Sha256};
use ureq::Agent;

use crate::{Config, Error, Result, http::Reply};

/// `offline_access` must be requested explicitly to receive a refresh token.
const SCOPE: &str = "workers-r2.write account-settings.read offline_access";
const SIGN_IN_TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// Refresh this long before expiry so a token can't lapse mid-request.
const EXPIRY_MARGIN: Duration = Duration::from_secs(60);

/// The tokens from a completed sign-in.
pub struct Tokens {
    access: String,
    refresh: String,
    expires_in: Duration,
}

/// Opens the browser at Cloudflare's consent page and waits for its redirect
/// back to the loopback port, until `cancel` is set or ten minutes pass.
pub fn sign_in(
    config: &Config,
    cancel: &AtomicBool,
    open_browser: impl FnOnce(&str) -> std::io::Result<()>,
) -> Result<Tokens> {
    let listener = TcpListener::bind(("127.0.0.1", config.redirect_port)).with_context(|| {
        format!(
            "listening for the sign-in callback on port {}",
            config.redirect_port
        )
    })?;
    listener.set_nonblocking(true)?;
    let verifier = random_token(64);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let state = random_token(16);
    let url = url::Url::parse_with_params(
        &config.authorize_url,
        [
            ("response_type", "code"),
            ("client_id", config.client_id.as_str()),
            ("redirect_uri", config.redirect_uri().as_str()),
            ("scope", SCOPE),
            ("state", state.as_str()),
            ("code_challenge", challenge.as_str()),
            ("code_challenge_method", "S256"),
        ],
    )
    .context("building the sign-in address")?;
    open_browser(url.as_str()).context("opening the browser")?;

    let deadline = Instant::now() + SIGN_IN_TIMEOUT;
    let code = loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Canceled);
        }
        if Instant::now() > deadline {
            return Err(Error::Other(anyhow!("sign-in timed out waiting for the browser")));
        }
        match listener.accept() {
            Ok((stream, _)) => {
                if let Some(code) = callback(stream, &state)? {
                    break code;
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(error) => return Err(error.into()),
        }
    };

    let reply = Reply::read(crate::http::agent().post(&config.token_url).send_form([
        ("grant_type", "authorization_code"),
        ("code", code.as_str()),
        ("redirect_uri", config.redirect_uri().as_str()),
        ("client_id", config.client_id.as_str()),
        ("code_verifier", verifier.as_str()),
    ]))?;
    tokens(&reply.body("exchanging the sign-in code")?)
}

/// Answers one browser request. `None` means keep waiting: another path, or a
/// stale tab whose state doesn't match this sign-in.
fn callback(mut stream: TcpStream, state: &str) -> Result<Option<String>> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut reader = BufReader::new(&stream);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    // Drain the headers; closing with unread input resets the connection
    // before the browser shows the page.
    let mut header = String::new();
    while reader.read_line(&mut header)? > 2 {
        header.clear();
    }
    let target = request_line.split_whitespace().nth(1).unwrap_or("/");
    let url =
        url::Url::parse(&format!("http://127.0.0.1{target}")).context("reading the sign-in callback")?;
    if url.path() != "/oauth/callback" {
        respond(&mut stream, "404 Not Found", "")?;
        return Ok(None);
    }
    let query = |name: &str| {
        url.query_pairs()
            .find(|(key, _)| key == name)
            .map(|(_, v)| v.into_owned())
    };
    if query("state").as_deref() != Some(state) {
        respond(
            &mut stream,
            "400 Bad Request",
            "This sign-in link has expired. Return to snip-snip and try again.",
        )?;
        return Ok(None);
    }
    if let Some(error) = query("error") {
        respond(
            &mut stream,
            "200 OK",
            "Sign-in didn't finish. You can close this tab and return to snip-snip.",
        )?;
        if error == "access_denied" {
            return Err(Error::Canceled);
        }
        let description = query("error_description").unwrap_or_default();
        return Err(Error::Other(anyhow!(
            "Cloudflare sign-in failed: {error} {description}"
        )));
    }
    let Some(code) = query("code") else {
        respond(&mut stream, "400 Bad Request", "The sign-in reply had no code.")?;
        return Err(Error::Other(anyhow!("the sign-in reply had no code")));
    };
    respond(
        &mut stream,
        "200 OK",
        "Signed in to snip-snip. You can close this tab.",
    )?;
    Ok(Some(code))
}

fn respond(stream: &mut TcpStream, status: &str, message: &str) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{message}",
        message.len()
    )
}

fn tokens(body: &[u8]) -> Result<Tokens> {
    let json: Value = serde_json::from_slice(body).context("reading Cloudflare's tokens")?;
    let field = |name: &str| json[name].as_str().map(str::to_owned);
    Ok(Tokens {
        access: field("access_token").context("Cloudflare returned no access token")?,
        refresh: field("refresh_token").context(
            "Cloudflare returned no refresh token; the OAuth client needs the refresh_token grant",
        )?,
        expires_in: Duration::from_secs(json["expires_in"].as_u64().unwrap_or(0)),
    })
}

fn random_token(bytes: usize) -> String {
    let mut buffer = vec![0u8; bytes];
    getrandom::fill(&mut buffer).expect("the OS random source is available");
    URL_SAFE_NO_PAD.encode(buffer)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    pub id: String,
    pub name: String,
}

struct Current {
    access: Option<(String, Instant)>,
    refresh: String,
}

/// A signed-in Cloudflare session, shared across threads.
pub struct Session {
    config: Config,
    agent: Agent,
    current: Mutex<Current>,
    on_refresh: Box<dyn Fn(&str) + Send + Sync>,
}

impl Session {
    /// Starts a session from a fresh sign-in. `on_refresh` receives the
    /// refresh token now and after every rotation, to save it.
    pub fn new(config: Config, tokens: Tokens, on_refresh: impl Fn(&str) + Send + Sync + 'static) -> Self {
        on_refresh(&tokens.refresh);
        let access = Some((tokens.access, Instant::now() + tokens.expires_in));
        Self::with(config, access, tokens.refresh, Box::new(on_refresh))
    }

    /// Resumes a saved session; the first call refreshes its access token.
    pub fn restore(
        config: Config,
        refresh_token: String,
        on_refresh: impl Fn(&str) + Send + Sync + 'static,
    ) -> Self {
        Self::with(config, None, refresh_token, Box::new(on_refresh))
    }

    fn with(
        config: Config,
        access: Option<(String, Instant)>,
        refresh: String,
        on_refresh: Box<dyn Fn(&str) + Send + Sync>,
    ) -> Self {
        Self {
            config,
            agent: crate::http::agent(),
            current: Mutex::new(Current { access, refresh }),
            on_refresh,
        }
    }

    pub fn accounts(&self) -> Result<Vec<Account>> {
        let url = format!("{}/accounts?per_page=50", self.config.api_url);
        let result = self
            .send("GET", &url, None)?
            .result("listing Cloudflare accounts")?;
        Ok(result
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|account| {
                Some(Account {
                    id: account["id"].as_str()?.to_owned(),
                    name: account["name"].as_str().unwrap_or_default().to_owned(),
                })
            })
            .collect())
    }

    pub(crate) fn api_url(&self) -> &str {
        &self.config.api_url
    }

    /// Sends an authorized request with an optional `(content type, bytes)`
    /// body. A rejected token is refreshed and the request retried once,
    /// which also covers a token another thread's refresh revoked while this
    /// request was in flight.
    pub(crate) fn send(&self, method: &str, url: &str, body: Option<(&str, &[u8])>) -> Result<Reply> {
        let attempt = |token: &str| {
            let mut request = ureq::http::Request::builder()
                .method(method)
                .uri(url)
                .header("Authorization", format!("Bearer {token}"));
            if let Some((content_type, _)) = body {
                request = request.header("Content-Type", content_type);
            }
            let request = request
                .body(body.map_or(&[][..], |(_, bytes)| bytes))
                .map_err(ureq::Error::from)?;
            Reply::read(self.agent.run(request))
        };
        let token = self.access(None)?;
        let reply = attempt(&token)?;
        if !reply.is_auth_failure() {
            return Ok(reply);
        }
        attempt(&self.access(Some(&token))?)
    }

    /// The current access token, refreshed when near expiry or when it is
    /// the `rejected` one.
    fn access(&self, rejected: Option<&str>) -> Result<String> {
        let mut current = self
            .current
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some((token, expires)) = &current.access
            && Instant::now() + EXPIRY_MARGIN < *expires
            && rejected != Some(token.as_str())
        {
            return Ok(token.clone());
        }
        let reply = Reply::read(self.agent.post(&self.config.token_url).send_form([
            ("grant_type", "refresh_token"),
            ("refresh_token", current.refresh.as_str()),
            ("client_id", self.config.client_id.as_str()),
        ]))?;
        if matches!(reply.status, 400 | 401) {
            return Err(Error::SignedOut);
        }
        let tokens = tokens(&reply.body("refreshing the Cloudflare session")?)?;
        (self.on_refresh)(&tokens.refresh);
        current.refresh = tokens.refresh;
        current.access = Some((tokens.access.clone(), Instant::now() + tokens.expires_in));
        Ok(tokens.access)
    }
}
