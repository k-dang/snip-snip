//! Cloud snips in the app: the sign-in state, the saved session, and the
//! store that Copy to cloud and the Library use. See docs/adr/0002.

use std::sync::{
    Arc, LazyLock,
    atomic::{AtomicBool, Ordering},
};

use anyhow::{Context as _, Result, anyhow};
use gpui::{App, Global};
use snip_cloud::{Account, Bucket, CloudSnip, Config, Session};
use snip_core::RgbaImage;

/// Bucket operations, substituted in tests. Calls block.
pub trait CloudStore: Send + Sync {
    fn upload(&self, image: &RgbaImage) -> snip_cloud::Result<CloudSnip>;
    fn latest(&self, limit: usize) -> snip_cloud::Result<Vec<CloudSnip>>;
    fn image(&self, snip: &CloudSnip) -> snip_cloud::Result<RgbaImage>;
    fn thumbnail(&self, snip: &CloudSnip) -> snip_cloud::Result<Vec<u8>>;
    fn delete(&self, snip: &CloudSnip) -> snip_cloud::Result<()>;
}

impl CloudStore for Bucket {
    fn upload(&self, image: &RgbaImage) -> snip_cloud::Result<CloudSnip> {
        Bucket::upload(self, image)
    }
    fn latest(&self, limit: usize) -> snip_cloud::Result<Vec<CloudSnip>> {
        Bucket::latest(self, limit)
    }
    fn image(&self, snip: &CloudSnip) -> snip_cloud::Result<RgbaImage> {
        Bucket::image(self, snip)
    }
    fn thumbnail(&self, snip: &CloudSnip) -> snip_cloud::Result<Vec<u8>> {
        Bucket::thumbnail(self, snip)
    }
    fn delete(&self, snip: &CloudSnip) -> snip_cloud::Result<()> {
        Bucket::delete(self, snip)
    }
}

/// Where the session and chosen account are kept, substituted in tests.
pub trait Credentials: Send + Sync {
    /// The saved refresh token and account, if sign-in finished.
    fn load(&self) -> Result<Option<(String, Account)>>;
    fn save_session(&self, refresh_token: &str) -> Result<()>;
    fn save_account(&self, account: &Account) -> Result<()>;
    fn clear(&self) -> Result<()>;
}

pub enum State {
    SignedOut,
    SigningIn(Arc<AtomicBool>),
    ChoosingAccount(Arc<Session>, Vec<Account>),
    /// Opening or creating the bucket.
    Connecting,
    R2NotEnabled(Arc<Session>, Account),
    Ready(Arc<dyn CloudStore>, Account),
}

pub struct Cloud {
    credentials: Arc<dyn Credentials>,
    pub state: State,
    pub error: Option<String>,
}

impl Global for Cloud {}

impl Cloud {
    pub fn signed_out(credentials: Arc<dyn Credentials>) -> Self {
        Self {
            credentials,
            state: State::SignedOut,
            error: None,
        }
    }
}

/// Restores a saved sign-in without touching the network, so cloud features
/// are available offline and fail on use instead.
pub fn install(credentials: Arc<dyn Credentials>, cx: &mut App) {
    cx.set_global(Cloud::signed_out(credentials.clone()));
    let load = cx.background_executor().spawn({
        let credentials = credentials.clone();
        async move { credentials.load() }
    });
    cx.spawn(async move |cx| {
        let loaded = load.await;
        cx.update(|cx| {
            let cloud = cx.global_mut::<Cloud>();
            match loaded {
                Ok(Some((refresh_token, account))) => {
                    let session = Arc::new(Session::restore(
                        Config::cloudflare(),
                        refresh_token,
                        saver(credentials),
                    ));
                    let store = Arc::new(Bucket::existing(session, &account.id));
                    cloud.state = State::Ready(store, account);
                }
                Ok(None) => {}
                Err(error) => {
                    cloud.error = Some(format!("Could not read the saved Cloudflare session: {error:#}"))
                }
            }
        });
    })
    .detach();
}

/// Saves each refresh token a session receives. A failed save only costs a
/// sign-in on the next launch, so it is logged rather than surfaced.
fn saver(credentials: Arc<dyn Credentials>) -> impl Fn(&str) + Send + Sync + 'static {
    move |token| {
        if let Err(error) = credentials.save_session(token) {
            eprintln!("cloud: could not save the session: {error:#}");
        }
    }
}

pub fn store(cx: &App) -> Option<Arc<dyn CloudStore>> {
    match &cx.try_global::<Cloud>()?.state {
        State::Ready(store, _) => Some(store.clone()),
        _ => None,
    }
}

pub fn sign_in(cx: &mut App) {
    let cloud = cx.global_mut::<Cloud>();
    if !matches!(cloud.state, State::SignedOut) {
        return;
    }
    let cancel = Arc::new(AtomicBool::new(false));
    cloud.state = State::SigningIn(cancel.clone());
    cloud.error = None;
    let credentials = cloud.credentials.clone();
    let signed_in = cx.background_executor().spawn(async move {
        let config = Config::cloudflare();
        let tokens = snip_cloud::sign_in(&config, &cancel, |url| open::that(url))?;
        let session = Arc::new(Session::new(config, tokens, saver(credentials)));
        let accounts = session.accounts()?;
        Ok::<_, snip_cloud::Error>((session, accounts))
    });
    cx.spawn(async move |cx| {
        let signed_in = signed_in.await;
        cx.update(|cx| match signed_in {
            Ok((session, mut accounts)) if accounts.len() == 1 => connect(session, accounts.remove(0), cx),
            Ok((_, accounts)) if accounts.is_empty() => {
                fail("This Cloudflare sign-in can't reach any accounts.".into(), cx)
            }
            Ok((session, accounts)) => {
                cx.global_mut::<Cloud>().state = State::ChoosingAccount(session, accounts)
            }
            Err(snip_cloud::Error::Canceled) => cx.global_mut::<Cloud>().state = State::SignedOut,
            Err(error) => fail(format!("Could not sign in: {error}"), cx),
        });
    })
    .detach();
}

pub fn cancel_sign_in(cx: &mut App) {
    if let State::SigningIn(cancel) = &cx.global::<Cloud>().state {
        cancel.store(true, Ordering::Relaxed);
    }
}

pub fn choose_account(account: Account, cx: &mut App) {
    let cloud = cx.global_mut::<Cloud>();
    let State::ChoosingAccount(session, _) = &cloud.state else {
        return;
    };
    let session = session.clone();
    connect(session, account, cx);
}

/// Tries the bucket again after the user enables R2.
pub fn retry(cx: &mut App) {
    let State::R2NotEnabled(session, account) = &cx.global::<Cloud>().state else {
        return;
    };
    let (session, account) = (session.clone(), account.clone());
    connect(session, account, cx);
}

/// The account's R2 page, where R2 is enabled.
pub fn r2_dashboard(cx: &App) -> Option<String> {
    match &cx.global::<Cloud>().state {
        State::R2NotEnabled(_, account) => {
            Some(format!("https://dash.cloudflare.com/{}/r2/overview", account.id))
        }
        _ => None,
    }
}

fn connect(session: Arc<Session>, account: Account, cx: &mut App) {
    let cloud = cx.global_mut::<Cloud>();
    cloud.state = State::Connecting;
    cloud.error = None;
    let credentials = cloud.credentials.clone();
    let opened = cx.background_executor().spawn({
        let (session, account) = (session.clone(), account.clone());
        async move {
            let bucket = Bucket::open(session, &account.id)?;
            credentials
                .save_account(&account)
                .context("saving the chosen account")?;
            Ok::<_, snip_cloud::Error>(bucket)
        }
    });
    cx.spawn(async move |cx| {
        let opened = opened.await;
        cx.update(|cx| match opened {
            Ok(bucket) => cx.global_mut::<Cloud>().state = State::Ready(Arc::new(bucket), account),
            Err(snip_cloud::Error::R2NotEnabled) => {
                cx.global_mut::<Cloud>().state = State::R2NotEnabled(session, account)
            }
            Err(error) => fail(format!("Could not set up the snip-snip bucket: {error}"), cx),
        });
    })
    .detach();
}

/// Forgets this machine's session. Cloud snips stay in the bucket.
pub fn sign_out(cx: &mut App) {
    cancel_sign_in(cx);
    let cloud = cx.global_mut::<Cloud>();
    cloud.state = State::SignedOut;
    cloud.error = cloud
        .credentials
        .clear()
        .err()
        .map(|error| format!("Could not forget the saved session: {error:#}"));
}

fn fail(message: String, cx: &mut App) {
    sign_out(cx);
    cx.global_mut::<Cloud>().error = Some(message);
}

/// Describes a failed cloud operation for a status line. An ended session
/// signs out, so cloud features disappear until the user signs in again.
pub fn describe(error: snip_cloud::Error, cx: &mut App) -> String {
    match error {
        snip_cloud::Error::SignedOut => {
            let message = "Your Cloudflare session ended. Sign in again in Settings.".to_owned();
            fail(message.clone(), cx);
            message
        }
        error => error.to_string(),
    }
}

/// The OS credential store: Windows Credential Manager or the macOS Keychain.
pub struct Keychain;

const SERVICE: &str = "snip-snip";

/// Installs the platform store on first use; a failure surfaces on each call.
static STORE: LazyLock<Result<(), String>> = LazyLock::new(|| {
    #[cfg(windows)]
    let store = windows_native_keyring_store::Store::new();
    #[cfg(target_os = "macos")]
    let store = apple_native_keyring_store::keychain::Store::new();
    keyring_core::set_default_store(store.map_err(|error| error.to_string())?);
    Ok(())
});

impl Keychain {
    fn entry(user: &str) -> Result<keyring_core::Entry> {
        STORE
            .as_ref()
            .map_err(|error| anyhow!("the credential store is unavailable: {error}"))?;
        Ok(keyring_core::Entry::new(SERVICE, user)?)
    }

    fn get(user: &str) -> Result<Option<String>> {
        match Self::entry(user)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
}

const SESSION: &str = "cloudflare-session";
const ACCOUNT: &str = "cloudflare-account";

impl Credentials for Keychain {
    fn load(&self) -> Result<Option<(String, Account)>> {
        let (Some(session), Some(account)) = (Self::get(SESSION)?, Self::get(ACCOUNT)?) else {
            return Ok(None);
        };
        let (id, name) = account.split_once('\n').unwrap_or((&account, ""));
        let account = Account {
            id: id.to_owned(),
            name: name.to_owned(),
        };
        Ok(Some((session, account)))
    }

    fn save_session(&self, refresh_token: &str) -> Result<()> {
        Ok(Self::entry(SESSION)?.set_password(refresh_token)?)
    }

    fn save_account(&self, account: &Account) -> Result<()> {
        Ok(Self::entry(ACCOUNT)?.set_password(&format!("{}\n{}", account.id, account.name))?)
    }

    fn clear(&self) -> Result<()> {
        for user in [SESSION, ACCOUNT] {
            match Self::entry(user)?.delete_credential() {
                Ok(()) | Err(keyring_core::Error::NoEntry) => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
pub mod testing {
    //! In-memory stand-ins for the credential store and the bucket.

    use std::{
        sync::{Mutex, atomic::AtomicBool},
        time::SystemTime,
    };

    use super::*;

    /// Cloud state signed in to `store`.
    pub fn signed_in(store: Arc<MemoryStore>) -> Cloud {
        let account = Account {
            id: "account-1".into(),
            name: "Kevin's Account".into(),
        };
        Cloud {
            credentials: Arc::new(MemoryCredentials::signed_in(account.clone())),
            state: State::Ready(store, account),
            error: None,
        }
    }

    /// A bucket held in memory, newest snip first.
    #[derive(Default)]
    pub struct MemoryStore {
        pub snips: Mutex<Vec<(CloudSnip, RgbaImage)>>,
        pub offline: AtomicBool,
    }

    impl MemoryStore {
        fn reachable(&self) -> snip_cloud::Result<()> {
            match self.offline.load(Ordering::Relaxed) {
                true => Err(snip_cloud::Error::Other(anyhow!("the network is unreachable"))),
                false => Ok(()),
            }
        }
    }

    impl CloudStore for MemoryStore {
        fn upload(&self, image: &RgbaImage) -> snip_cloud::Result<CloudSnip> {
            self.reachable()?;
            let snip = CloudSnip::new(SystemTime::now());
            self.snips
                .lock()
                .unwrap()
                .insert(0, (snip.clone(), image.clone()));
            Ok(snip)
        }
        fn latest(&self, limit: usize) -> snip_cloud::Result<Vec<CloudSnip>> {
            self.reachable()?;
            Ok(self
                .snips
                .lock()
                .unwrap()
                .iter()
                .take(limit)
                .map(|(snip, _)| snip.clone())
                .collect())
        }
        fn image(&self, snip: &CloudSnip) -> snip_cloud::Result<RgbaImage> {
            self.reachable()?;
            let snips = self.snips.lock().unwrap();
            let found = snips.iter().find(|(stored, _)| stored == snip);
            Ok(found.context("no such cloud snip")?.1.clone())
        }
        fn thumbnail(&self, snip: &CloudSnip) -> snip_cloud::Result<Vec<u8>> {
            let image = self.image(snip)?;
            let mut bytes = Vec::new();
            image::codecs::webp::WebPEncoder::new_lossless(&mut bytes)
                .encode(
                    image.as_raw(),
                    image.width(),
                    image.height(),
                    image::ExtendedColorType::Rgba8,
                )
                .map_err(anyhow::Error::from)?;
            Ok(bytes)
        }
        fn delete(&self, snip: &CloudSnip) -> snip_cloud::Result<()> {
            self.reachable()?;
            self.snips.lock().unwrap().retain(|(stored, _)| stored != snip);
            Ok(())
        }
    }

    #[derive(Default)]
    pub struct MemoryCredentials {
        pub saved: Mutex<(Option<String>, Option<Account>)>,
    }

    impl MemoryCredentials {
        pub fn signed_in(account: Account) -> Self {
            Self {
                saved: Mutex::new((Some("refresh-0".into()), Some(account))),
            }
        }
    }

    impl Credentials for MemoryCredentials {
        fn load(&self) -> Result<Option<(String, Account)>> {
            let saved = self.saved.lock().unwrap();
            Ok(saved.0.clone().zip(saved.1.clone()))
        }
        fn save_session(&self, refresh_token: &str) -> Result<()> {
            self.saved.lock().unwrap().0 = Some(refresh_token.to_owned());
            Ok(())
        }
        fn save_account(&self, account: &Account) -> Result<()> {
            self.saved.lock().unwrap().1 = Some(account.clone());
            Ok(())
        }
        fn clear(&self) -> Result<()> {
            *self.saved.lock().unwrap() = (None, None);
            Ok(())
        }
    }
}
