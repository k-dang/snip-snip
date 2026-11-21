//! Cloud snips: Sign in with Cloudflare and a `snip-snip` R2 bucket in the
//! user's own account, reached through Cloudflare's REST API. Calls block, so
//! run them off the UI thread. See docs/adr/0002.

mod bucket;
mod http;
mod session;

pub use bucket::{Bucket, CloudSnip, THUMBNAIL_SIZE, decode, encode};
pub use session::{Account, Session, Tokens, sign_in};

/// Where snip-snip signs in and which OAuth client it signs in as.
#[derive(Clone, Debug)]
pub struct Config {
    pub authorize_url: String,
    pub token_url: String,
    pub api_url: String,
    pub client_id: String,
    /// The loopback port registered in the client's redirect URI.
    pub redirect_port: u16,
}

impl Config {
    /// snip-snip's OAuth client, registered as a public PKCE client. It is
    /// private, so only members of the account that owns it can sign in.
    pub fn cloudflare() -> Self {
        Self {
            authorize_url: "https://dash.cloudflare.com/oauth2/auth".into(),
            token_url: "https://dash.cloudflare.com/oauth2/token".into(),
            api_url: "https://api.cloudflare.com/client/v4".into(),
            client_id: "e7acbd9a90a07f07bc682e5770482f89".into(),
            redirect_port: 8976,
        }
    }

    /// The account's R2 page in the dashboard, where R2 is enabled.
    pub fn r2_dashboard(account_id: &str) -> String {
        format!("https://dash.cloudflare.com/{account_id}/r2/overview")
    }

    fn redirect_uri(&self) -> String {
        format!("http://127.0.0.1:{}/oauth/callback", self.redirect_port)
    }
}

#[derive(Debug)]
pub enum Error {
    /// The Cloudflare account hasn't enabled R2 yet.
    R2NotEnabled,
    /// Cloudflare no longer accepts the session; the user must sign in again.
    SignedOut,
    /// Sign-in was canceled before the browser returned.
    Canceled,
    Other(anyhow::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::R2NotEnabled => f.write_str("R2 isn't enabled on this Cloudflare account"),
            Error::SignedOut => f.write_str("the Cloudflare session has ended; sign in again"),
            Error::Canceled => f.write_str("sign-in was canceled"),
            Error::Other(error) => write!(f, "{error:#}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<anyhow::Error> for Error {
    fn from(error: anyhow::Error) -> Self {
        Error::Other(error)
    }
}

impl From<ureq::Error> for Error {
    fn from(error: ureq::Error) -> Self {
        Error::Other(error.into())
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error::Other(error.into())
    }
}
