//! The `snip-snip` bucket. Each cloud snip is a lossless WebP under `snips/`
//! with a small WebP thumbnail under `thumbs/`. Ids sort newest first, so a
//! plain listing returns the latest snips without paging.

use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context as _, anyhow};
use image::{ExtendedColorType, ImageFormat, RgbaImage, codecs::webp::WebPEncoder, imageops::FilterType};
use serde_json::json;

use crate::{Error, Result, Session};

const BUCKET: &str = "snip-snip";
/// The longest side of a thumbnail, in pixels.
pub const THUMBNAIL_SIZE: u32 = 480;
const SNIPS: &str = "snips/";
const THUMBS: &str = "thumbs/";

/// A snip stored in the bucket.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CloudSnip {
    /// Inverted upload milliseconds then a random suffix, both fixed-width hex.
    id: String,
}

impl CloudSnip {
    /// A new snip id for something uploaded at `uploaded`.
    pub fn new(uploaded: SystemTime) -> Self {
        let millis = uploaded
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let mut suffix = [0u8; 4];
        getrandom::fill(&mut suffix).expect("the OS random source is available");
        Self {
            id: format!("{:016x}{:08x}", u64::MAX - millis, u32::from_be_bytes(suffix)),
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        let id = key.strip_prefix(SNIPS)?.strip_suffix(".webp")?;
        (id.len() == 24 && id.bytes().all(|b| b.is_ascii_hexdigit())).then(|| Self { id: id.to_owned() })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn uploaded(&self) -> SystemTime {
        let inverted = u64::from_str_radix(&self.id[..16], 16).unwrap_or(u64::MAX);
        UNIX_EPOCH + Duration::from_millis(u64::MAX - inverted)
    }

    fn image_key(&self) -> String {
        format!("{SNIPS}{}.webp", self.id)
    }

    fn thumbnail_key(&self) -> String {
        format!("{THUMBS}{}.webp", self.id)
    }
}

pub struct Bucket {
    session: Arc<Session>,
    /// `{api}/accounts/{account}/r2/buckets/snip-snip`
    url: String,
}

impl Bucket {
    /// The account's `snip-snip` bucket, set up earlier by [`Bucket::open`].
    pub fn existing(session: Arc<Session>, account_id: &str) -> Self {
        let url = format!("{}/accounts/{account_id}/r2/buckets/{BUCKET}", session.api_url());
        Self { session, url }
    }

    /// Opens the account's `snip-snip` bucket, creating it on first use.
    pub fn open(session: Arc<Session>, account_id: &str) -> Result<Self> {
        let bucket = Self::existing(session, account_id);
        let found = bucket.session.send("GET", &bucket.url, None)?;
        if found.status != 404 {
            found.result("opening the snip-snip bucket")?;
            return Ok(bucket);
        }
        let buckets = format!("{}/accounts/{account_id}/r2/buckets", bucket.session.api_url());
        let name = json!({ "name": BUCKET }).to_string();
        let created = bucket
            .session
            .send("POST", &buckets, Some(("application/json", name.as_bytes())))?;
        // Another machine may have created it first.
        if !created.succeeded() {
            bucket
                .session
                .send("GET", &bucket.url, None)?
                .result("creating the snip-snip bucket")?;
        }
        Ok(bucket)
    }

    /// Stores a snip and its thumbnail. The thumbnail goes first so every
    /// listed snip has one.
    pub fn upload(&self, image: &RgbaImage) -> Result<CloudSnip> {
        let snip = CloudSnip::new(SystemTime::now());
        for (key, image) in [
            (snip.thumbnail_key(), &thumbnail(image)),
            (snip.image_key(), image),
        ] {
            self.session
                .send(
                    "PUT",
                    &self.object_url(&key),
                    Some(("image/webp", &encode(image)?)),
                )?
                .result("uploading a cloud snip")?;
        }
        Ok(snip)
    }

    /// Up to `limit` snips, newest first.
    pub fn latest(&self, limit: usize) -> Result<Vec<CloudSnip>> {
        let url = format!(
            "{}/objects?prefix={SNIPS}&per_page={}",
            self.url,
            limit.clamp(1, 1000)
        );
        let result = self
            .session
            .send("GET", &url, None)?
            .result("listing cloud snips")?;
        Ok(result
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|object| CloudSnip::from_key(object["key"].as_str()?))
            .collect())
    }

    pub fn image(&self, snip: &CloudSnip) -> Result<RgbaImage> {
        decode(&self.get(&snip.image_key())?)
    }

    /// The thumbnail's encoded bytes, ready to cache; see [`decode`].
    pub fn thumbnail(&self, snip: &CloudSnip) -> Result<Vec<u8>> {
        self.get(&snip.thumbnail_key())
    }

    /// Removes a snip, then its thumbnail; a leftover thumbnail is never listed.
    pub fn delete(&self, snip: &CloudSnip) -> Result<()> {
        for key in [snip.image_key(), snip.thumbnail_key()] {
            let reply = self.session.send("DELETE", &self.object_url(&key), None)?;
            if reply.status != 404 {
                reply.result("deleting a cloud snip")?;
            }
        }
        Ok(())
    }

    fn get(&self, key: &str) -> Result<Vec<u8>> {
        let reply = self.session.send("GET", &self.object_url(key), None)?;
        if !reply.succeeded() {
            return Err(reply.error("downloading a cloud snip"));
        }
        Ok(reply.body)
    }

    fn object_url(&self, key: &str) -> String {
        format!("{}/objects/{key}", self.url)
    }
}

fn thumbnail(image: &RgbaImage) -> RgbaImage {
    let (width, height) = image.dimensions();
    let longest = width.max(height);
    if longest <= THUMBNAIL_SIZE {
        return image.clone();
    }
    let scale = THUMBNAIL_SIZE as f64 / longest as f64;
    let size = |side: u32| ((side as f64 * scale).round() as u32).max(1);
    image::imageops::resize(image, size(width), size(height), FilterType::Triangle)
}

/// Encodes a snip or thumbnail the way the bucket stores it.
pub fn encode(image: &RgbaImage) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    WebPEncoder::new_lossless(&mut bytes)
        .encode(
            image.as_raw(),
            image.width(),
            image.height(),
            ExtendedColorType::Rgba8,
        )
        .context("encoding WebP")?;
    Ok(bytes)
}

/// Decodes a stored snip or thumbnail.
pub fn decode(bytes: &[u8]) -> Result<RgbaImage> {
    image::load_from_memory_with_format(bytes, ImageFormat::WebP)
        .map(|image| image.into_rgba8())
        .map_err(|error| Error::Other(anyhow!("decoding a cloud snip: {error}")))
}
