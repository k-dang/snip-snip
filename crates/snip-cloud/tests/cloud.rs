//! Drives the real client against a local stand-in for Cloudflare's OAuth
//! and R2 REST endpoints.

use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use image::{Rgba, RgbaImage};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use snip_cloud::{Account, Bucket, Config, Error, Session, THUMBNAIL_SIZE, decode, sign_in};

const ACCOUNT: &str = "account-1";

#[derive(Default)]
struct State {
    entitled: bool,
    bucket: bool,
    objects: BTreeMap<String, Vec<u8>>,
    challenge: Option<String>,
    issued: u32,
    access: Option<String>,
    refresh: Option<String>,
    expires_in: u64,
}

impl State {
    fn issue(&mut self) -> Value {
        self.issued += 1;
        let access = format!("access-{}", self.issued);
        let refresh = format!("refresh-{}", self.issued);
        self.access = Some(access.clone());
        self.refresh = Some(refresh.clone());
        json!({ "access_token": access, "refresh_token": refresh, "expires_in": self.expires_in, "token_type": "bearer" })
    }
}

struct Fake {
    url: String,
    state: Arc<Mutex<State>>,
}

struct Request {
    method: String,
    target: String,
    authorization: Option<String>,
    body: Vec<u8>,
}

impl Fake {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let state = Arc::new(Mutex::new(State {
            entitled: true,
            expires_in: 3600,
            ..State::default()
        }));
        let shared = state.clone();
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { return };
                serve(stream, &shared);
            }
        });
        Self { url, state }
    }

    fn config(&self, redirect_port: u16) -> Config {
        Config {
            authorize_url: "https://dash.example/oauth2/auth".into(),
            token_url: format!("{}/token", self.url),
            api_url: self.url.clone(),
            client_id: "client-1".into(),
            redirect_port,
        }
    }

    /// A saved session whose refresh token the fake currently accepts.
    fn restored(&self, on_refresh: impl Fn(&str) + Send + Sync + 'static) -> Arc<Session> {
        self.state.lock().unwrap().refresh = Some("refresh-0".into());
        Arc::new(Session::restore(self.config(0), "refresh-0".into(), on_refresh))
    }

    fn keys(&self) -> Vec<String> {
        self.state.lock().unwrap().objects.keys().cloned().collect()
    }
}

fn serve(stream: TcpStream, state: &Mutex<State>) {
    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let mut parts = line.split_whitespace();
    let (method, target) = (
        parts.next().unwrap_or("").to_owned(),
        parts.next().unwrap_or("").to_owned(),
    );
    let (mut length, mut authorization) = (0, None);
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).unwrap();
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        let (name, value) = header.split_once(':').unwrap();
        match name.to_ascii_lowercase().as_str() {
            "content-length" => length = value.trim().parse().unwrap(),
            "authorization" => authorization = Some(value.trim().to_owned()),
            _ => {}
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).unwrap();
    let request = Request {
        method,
        target,
        authorization,
        body,
    };
    let (status, reply) = route(&request, &mut state.lock().unwrap());
    let mut stream = stream;
    write!(
        stream,
        "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        reply.len()
    )
    .unwrap();
    stream.write_all(&reply).unwrap();
}

fn api_error(status: u16, code: u32) -> (u16, Vec<u8>) {
    let body = json!({ "success": false, "errors": [{ "code": code, "message": "nope" }], "result": null });
    (status, body.to_string().into_bytes())
}

fn api_ok(result: Value) -> (u16, Vec<u8>) {
    (
        200,
        json!({ "success": true, "errors": [], "result": result })
            .to_string()
            .into_bytes(),
    )
}

fn route(request: &Request, state: &mut State) -> (u16, Vec<u8>) {
    let url = url::Url::parse(&format!("http://fake{}", request.target)).unwrap();
    if url.path() == "/token" {
        let form: BTreeMap<String, String> =
            url::form_urlencoded::parse(&request.body).into_owned().collect();
        let accepted = match form["grant_type"].as_str() {
            "authorization_code" => {
                let verifier = URL_SAFE_NO_PAD.encode(Sha256::digest(form["code_verifier"].as_bytes()));
                form["code"] == "the-code" && state.challenge.as_deref() == Some(verifier.as_str())
            }
            "refresh_token" => state.refresh.as_deref() == Some(form["refresh_token"].as_str()),
            _ => false,
        };
        return if accepted {
            (200, state.issue().to_string().into_bytes())
        } else {
            (400, br#"{"error":"invalid_grant"}"#.to_vec())
        };
    }
    let bearer = state.access.as_ref().map(|token| format!("Bearer {token}"));
    if request.authorization.is_none() || request.authorization != bearer {
        return api_error(401, 10000);
    }
    if url.path() == "/accounts" {
        return api_ok(json!([{ "id": ACCOUNT, "name": "Kevin's Account" }]));
    }
    if !state.entitled {
        return api_error(403, 10042);
    }
    let buckets = format!("/accounts/{ACCOUNT}/r2/buckets");
    let bucket = format!("{buckets}/snip-snip");
    let path = url.path();
    match (request.method.as_str(), path) {
        ("POST", p) if p == buckets => {
            state.bucket = true;
            api_ok(json!({ "name": "snip-snip" }))
        }
        ("GET", p) if p == bucket => match state.bucket {
            true => api_ok(json!({ "name": "snip-snip" })),
            false => api_error(404, 10006),
        },
        ("GET", p) if p == format!("{bucket}/objects") => {
            let query: BTreeMap<String, String> = url.query_pairs().into_owned().collect();
            let limit: usize = query["per_page"].parse().unwrap();
            let listed: Vec<Value> = state
                .objects
                .keys()
                .filter(|key| key.starts_with(&query["prefix"]))
                .take(limit)
                .map(|key| json!({ "key": key }))
                .collect();
            api_ok(Value::Array(listed))
        }
        (method, p) => {
            let Some(key) = p.strip_prefix(&format!("{bucket}/objects/")) else {
                return api_error(404, 10007);
            };
            match method {
                "PUT" => {
                    state.objects.insert(key.to_owned(), request.body.clone());
                    api_ok(json!({ "key": key }))
                }
                "GET" => match state.objects.get(key) {
                    Some(bytes) => (200, bytes.clone()),
                    None => api_error(404, 10007),
                },
                "DELETE" => match state.objects.remove(key) {
                    Some(_) => api_ok(json!({ "key": key })),
                    None => api_error(404, 10007),
                },
                _ => api_error(405, 0),
            }
        }
    }
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn gradient(width: u32, height: u32) -> RgbaImage {
    RgbaImage::from_fn(width, height, |x, y| {
        Rgba([(x % 256) as u8, (y % 256) as u8, 128, 255])
    })
}

#[test]
fn sign_in_exchanges_the_code_with_pkce() {
    let fake = Fake::start();
    let config = fake.config(free_port());
    let state = fake.state.clone();
    let tokens = sign_in(&config, &AtomicBool::new(false), |address| {
        // Play the browser: Cloudflare redirects back with a code.
        let url = url::Url::parse(address).unwrap();
        let query: BTreeMap<String, String> = url.query_pairs().into_owned().collect();
        assert_eq!(query["client_id"], "client-1");
        assert_eq!(query["code_challenge_method"], "S256");
        assert!(query["scope"].split(' ').any(|scope| scope == "offline_access"));
        state.lock().unwrap().challenge = Some(query["code_challenge"].clone());
        let callback = format!("{}?code=the-code&state={}", query["redirect_uri"], query["state"]);
        thread::spawn(move || ureq::get(&callback).call().unwrap());
        Ok(())
    })
    .unwrap();

    let saved = Arc::new(Mutex::new(Vec::new()));
    let session = {
        let saved = saved.clone();
        Session::new(config, tokens, move |token| {
            saved.lock().unwrap().push(token.to_owned())
        })
    };
    assert_eq!(
        session.accounts().unwrap(),
        [Account {
            id: ACCOUNT.into(),
            name: "Kevin's Account".into()
        }]
    );
    assert_eq!(*saved.lock().unwrap(), ["refresh-1"]);
}

#[test]
fn canceled_sign_in_stops_waiting() {
    let fake = Fake::start();
    let cancel = AtomicBool::new(false);
    let result = sign_in(&fake.config(free_port()), &cancel, |_| {
        cancel.store(true, Ordering::Relaxed);
        Ok(())
    });
    assert!(matches!(result, Err(Error::Canceled)));
}

#[test]
fn snips_round_trip_newest_first() {
    let fake = Fake::start();
    let bucket = Bucket::open(fake.restored(|_| {}), ACCOUNT).unwrap();
    let older = bucket.upload(&gradient(1000, 500)).unwrap();
    thread::sleep(Duration::from_millis(5));
    let newer_image = gradient(40, 30);
    let newer = bucket.upload(&newer_image).unwrap();

    assert_eq!(bucket.latest(100).unwrap(), [newer.clone(), older.clone()]);
    assert_eq!(bucket.latest(1).unwrap(), std::slice::from_ref(&newer));
    assert!(newer.uploaded() > older.uploaded());
    assert_eq!(bucket.image(&newer).unwrap(), newer_image);
    assert_eq!(
        decode(&bucket.thumbnail(&older).unwrap()).unwrap().dimensions(),
        (THUMBNAIL_SIZE, THUMBNAIL_SIZE / 2)
    );

    bucket.delete(&older).unwrap();
    assert_eq!(bucket.latest(100).unwrap(), std::slice::from_ref(&newer));
    assert_eq!(
        fake.keys(),
        [
            format!("snips/{}.webp", newer.id()),
            format!("thumbs/{}.webp", newer.id())
        ]
    );
}

#[test]
fn every_rotated_refresh_token_is_reported() {
    let fake = Fake::start();
    // Tokens that are already inside the expiry margin refresh on every call.
    fake.state.lock().unwrap().expires_in = 0;
    let saved = Arc::new(Mutex::new(Vec::new()));
    let session = {
        let saved = saved.clone();
        fake.restored(move |token| saved.lock().unwrap().push(token.to_owned()))
    };
    session.accounts().unwrap();
    session.accounts().unwrap();
    assert_eq!(*saved.lock().unwrap(), ["refresh-1", "refresh-2"]);
}

#[test]
fn a_revoked_access_token_is_refreshed_and_retried() {
    let fake = Fake::start();
    let session = fake.restored(|_| {});
    session.accounts().unwrap();
    // A refresh by another request revokes the token this one carries.
    fake.state.lock().unwrap().access = Some("revoked-elsewhere".into());
    fake.state.lock().unwrap().refresh = Some("refresh-1".into());
    assert_eq!(session.accounts().unwrap().len(), 1);
}

#[test]
fn a_rejected_refresh_token_signs_out() {
    let fake = Fake::start();
    let session = fake.restored(|_| {});
    fake.state.lock().unwrap().refresh = Some("issued-later".into());
    assert!(matches!(session.accounts(), Err(Error::SignedOut)));
}

#[test]
fn an_account_without_r2_is_reported() {
    let fake = Fake::start();
    fake.state.lock().unwrap().entitled = false;
    let result = Bucket::open(fake.restored(|_| {}), ACCOUNT);
    assert!(matches!(result, Err(Error::R2NotEnabled)));
}
