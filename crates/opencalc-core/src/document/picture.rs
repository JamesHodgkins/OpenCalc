//! The Picture region: a PNG image kept inside the worksheet.
//!
//! The file's bytes are stored as they were read (so saving never re-encodes and
//! never loses anything) and written to the worksheet file as base64 text. Core
//! does not decode images; it checks the PNG header, which gives the size, and
//! holds the limits that keep a hostile file from asking for gigabytes. The window
//! program decodes and draws.

use std::sync::Arc;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The largest PNG file kept, in bytes.
pub const MAX_PICTURE_BYTES: usize = 2 * 1024 * 1024;
/// The most pixels an image may have (width times height).
pub const MAX_PICTURE_PIXELS: u64 = 16_000_000;
/// The longest side, in pixels.
pub const MAX_PICTURE_SIDE: u32 = 8192;
/// The most pictures in one worksheet.
pub const MAX_PICTURES: usize = 16;
/// The longest name or description kept, in characters.
pub const MAX_PICTURE_TEXT: usize = 200;

const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

/// A picture region's content. May be empty (no image chosen yet).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PictureSpec {
    /// Where the image came from: a label only, never used to read a file.
    pub name: String,
    /// What the picture shows, for anyone who cannot see it.
    pub alt: String,
    data: Arc<Vec<u8>>,
    /// A fingerprint of `data`, to recognise the same image without comparing it.
    id: u64,
}

fn fingerprint(bytes: &[u8]) -> u64 {
    // FNV-1a: small, and enough to tell images apart in a cache.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h ^ bytes.len() as u64
}

/// The width and height a PNG header declares, if it is one.
fn header_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 33 || bytes[..8] != SIGNATURE || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let w = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    Some((w, h))
}

impl Default for PictureSpec {
    fn default() -> PictureSpec {
        PictureSpec::raw("", "", Vec::new())
    }
}

impl PictureSpec {
    /// Keeps `bytes` as a picture, if they are a PNG within the limits.
    pub fn from_png(name: &str, bytes: Vec<u8>) -> Result<PictureSpec, String> {
        if bytes.len() > MAX_PICTURE_BYTES {
            return Err(format!(
                "the file is larger than {} MB",
                MAX_PICTURE_BYTES >> 20
            ));
        }
        let Some((w, h)) = header_size(&bytes) else {
            return Err("this is not a PNG image (only PNG is supported)".into());
        };
        if w == 0 || h == 0 {
            return Err("the image has no size".into());
        }
        if w > MAX_PICTURE_SIDE
            || h > MAX_PICTURE_SIDE
            || u64::from(w) * u64::from(h) > MAX_PICTURE_PIXELS
        {
            return Err(format!(
                "the image is {w} by {h} pixels; at most {MAX_PICTURE_SIDE} on a side and {} million pixels",
                MAX_PICTURE_PIXELS / 1_000_000
            ));
        }
        Ok(PictureSpec::raw(name, "", bytes))
    }

    fn raw(name: &str, alt: &str, bytes: Vec<u8>) -> PictureSpec {
        PictureSpec {
            name: name.chars().take(MAX_PICTURE_TEXT).collect(),
            alt: alt.chars().take(MAX_PICTURE_TEXT).collect(),
            id: fingerprint(&bytes),
            data: Arc::new(bytes),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// The PNG file's bytes.
    pub fn png(&self) -> &[u8] {
        &self.data
    }

    /// A fingerprint of the image, equal for equal images.
    pub fn content_id(&self) -> u64 {
        self.id
    }

    /// Width and height in pixels, from the header.
    pub fn dimensions(&self) -> Option<(u32, u32)> {
        header_size(&self.data)
    }

    pub fn with_alt(mut self, alt: &str) -> PictureSpec {
        self.alt = alt
            .chars()
            .filter(|c| !c.is_control())
            .take(MAX_PICTURE_TEXT)
            .collect();
        self
    }

    /// The same description with no image.
    pub fn cleared(&self) -> PictureSpec {
        PictureSpec::default().with_alt(&self.alt)
    }
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for k in 0..4 {
            if k <= chunk.len() {
                out.push(ALPHABET[(n >> (18 - 6 * k) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0);
    for c in text.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            b'\n' | b'\r' | b' ' => continue,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

#[derive(Serialize, Deserialize)]
struct Saved {
    #[serde(default)]
    name: String,
    #[serde(default)]
    alt: String,
    #[serde(default)]
    data: String,
}

impl Serialize for PictureSpec {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        Saved {
            name: self.name.clone(),
            alt: self.alt.clone(),
            data: encode(&self.data),
        }
        .serialize(s)
    }
}

impl<'de> Deserialize<'de> for PictureSpec {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<PictureSpec, D::Error> {
        let saved = Saved::deserialize(d)?;
        // Text that is not base64 leaves the region without an image, rather than
        // refusing the whole worksheet.
        let bytes = decode(&saved.data).unwrap_or_default();
        Ok(PictureSpec::raw(&saved.name, &saved.alt, bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A valid 2 by 3 PNG header followed by filler (core does not decode).
    fn fake_png(w: u32, h: u32) -> Vec<u8> {
        let mut v = SIGNATURE.to_vec();
        v.extend_from_slice(&13u32.to_be_bytes());
        v.extend_from_slice(b"IHDR");
        v.extend_from_slice(&w.to_be_bytes());
        v.extend_from_slice(&h.to_be_bytes());
        v.extend_from_slice(&[8, 2, 0, 0, 0, 0, 0, 0, 0]);
        v
    }

    #[test]
    fn base64_round_trips_every_length() {
        for n in 0..40usize {
            let bytes: Vec<u8> = (0..n).map(|i| (i * 37 + 11) as u8).collect();
            assert_eq!(decode(&encode(&bytes)).unwrap(), bytes, "{n}");
        }
        assert_eq!(encode(b"Man"), "TWFu");
        assert_eq!(encode(b"Ma"), "TWE=");
        assert_eq!(encode(b"M"), "TQ==");
        assert!(decode("TW!u").is_none());
    }

    #[test]
    fn a_png_within_the_limits_is_kept_with_its_size() {
        let p = PictureSpec::from_png("a.png", fake_png(2, 3)).unwrap();
        assert_eq!(p.dimensions(), Some((2, 3)));
        assert_eq!(p.name, "a.png");
        assert!(!p.is_empty());
    }

    #[test]
    fn other_files_and_oversized_images_are_refused() {
        assert!(PictureSpec::from_png("x", b"GIF89a....".to_vec())
            .unwrap_err()
            .contains("not a PNG"));
        assert!(PictureSpec::from_png("x", fake_png(0, 5)).is_err());
        assert!(PictureSpec::from_png("x", fake_png(9000, 10))
            .unwrap_err()
            .contains("pixels"));
        assert!(PictureSpec::from_png("x", fake_png(5000, 5000)).is_err());
        let mut big = fake_png(2, 2);
        big.resize(MAX_PICTURE_BYTES + 1, 0);
        assert!(PictureSpec::from_png("x", big).unwrap_err().contains("MB"));
    }

    #[test]
    fn saving_and_loading_keep_every_byte() {
        let p = PictureSpec::from_png("a.png", fake_png(4, 4))
            .unwrap()
            .with_alt("a square");
        let json = serde_json::to_string(&p).unwrap();
        let back: PictureSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
        assert_eq!(back.content_id(), p.content_id());
    }

    #[test]
    fn unreadable_data_leaves_an_empty_picture_not_a_failed_load() {
        let back: PictureSpec =
            serde_json::from_str(r#"{"name":"n","alt":"a","data":"!!not base64!!"}"#).unwrap();
        assert!(back.is_empty());
        assert_eq!(back.name, "n");
        let blank: PictureSpec = serde_json::from_str("{}").unwrap();
        assert!(blank.is_empty());
    }

    #[test]
    fn different_images_have_different_ids() {
        let a = PictureSpec::from_png("", fake_png(2, 2)).unwrap();
        let b = PictureSpec::from_png("", fake_png(3, 2)).unwrap();
        assert_ne!(a.content_id(), b.content_id());
        assert_eq!(a.content_id(), a.clone().content_id());
    }
}
