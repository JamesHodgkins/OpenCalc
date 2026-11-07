//! The file container: a ZIP with `manifest.json` and `worksheet.json`.
//!
//! Writing is deterministic (fixed entry order, fixed modification time, fixed
//! permissions), so the same bytes come out for the same document. Reading treats
//! the archive as hostile: entry count, names and inflated sizes are bounded, and
//! only the two entries we know are ever decompressed.

use std::io::{Cursor, Read, Write};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipArchive, ZipWriter};

use super::LoadError;

pub const MANIFEST: &str = "manifest.json";
pub const WORKSHEET: &str = "worksheet.json";

/// Most entries an archive may hold. Pictures and data snapshots will live under
/// `assets/`; until then only the two known parts are expected.
pub const MAX_ENTRIES: usize = 8;
pub const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
pub const MAX_WORKSHEET_BYTES: u64 = 64 * 1024 * 1024;
/// Largest file accepted at all.
pub const MAX_FILE_BYTES: usize = 128 * 1024 * 1024;

/// Pack the two parts. Deterministic: no clock is read.
pub fn pack(manifest: &[u8], worksheet: &[u8]) -> Result<Vec<u8>, String> {
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .last_modified_time(DateTime::default())
        .unix_permissions(0o644);
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in [(MANIFEST, manifest), (WORKSHEET, worksheet)] {
        zip.start_file(name, options).map_err(|e| e.to_string())?;
        zip.write_all(bytes).map_err(|e| e.to_string())?;
    }
    let out = zip.finish().map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}

/// Whether an entry name could escape the folder it is unpacked into.
pub fn unsafe_name(name: &str) -> bool {
    name.starts_with('/')
        || name.starts_with('\\')
        || name.contains('\\')
        || name.contains('\0')
        || name.split('/').any(|part| part == "..")
        || name.as_bytes().get(1) == Some(&b':')
}

/// The manifest and worksheet parts of `bytes`.
pub fn unpack(bytes: &[u8]) -> Result<(Vec<u8>, Vec<u8>), LoadError> {
    unpack_with(bytes, MAX_MANIFEST_BYTES, MAX_WORKSHEET_BYTES)
}

pub(super) fn unpack_with(
    bytes: &[u8],
    manifest_limit: u64,
    worksheet_limit: u64,
) -> Result<(Vec<u8>, Vec<u8>), LoadError> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err(LoadError::TooLarge {
            what: "file",
            limit: MAX_FILE_BYTES as u64,
        });
    }
    let mut zip =
        ZipArchive::new(Cursor::new(bytes)).map_err(|e| LoadError::NotAnArchive(e.to_string()))?;
    if zip.len() > MAX_ENTRIES {
        return Err(LoadError::TooMany {
            what: "archive entries",
            limit: MAX_ENTRIES,
        });
    }
    let mut seen: Vec<String> = Vec::with_capacity(zip.len());
    for i in 0..zip.len() {
        let name = zip
            .name_for_index(i)
            .ok_or_else(|| LoadError::Corrupt("missing entry".into()))?
            .map_err(|e| LoadError::Corrupt(e.to_string()))?
            .into_owned();
        if unsafe_name(&name) {
            return Err(LoadError::UnsafePath(name));
        }
        if seen.contains(&name) {
            return Err(LoadError::DuplicateEntry(name));
        }
        seen.push(name);
    }
    let manifest = entry(&mut zip, MANIFEST, manifest_limit)?;
    let worksheet = entry(&mut zip, WORKSHEET, worksheet_limit)?;
    Ok((manifest, worksheet))
}

/// Read one named entry, never inflating more than `limit` bytes whatever the
/// archive claims about its size.
fn entry(
    zip: &mut ZipArchive<Cursor<&[u8]>>,
    name: &'static str,
    limit: u64,
) -> Result<Vec<u8>, LoadError> {
    let file = zip.by_name(name).map_err(|e| match e {
        zip::result::ZipError::FileNotFound => LoadError::Missing(name),
        other => LoadError::Corrupt(format!("{name}: {other}")),
    })?;
    if file.size() > limit {
        return Err(LoadError::TooLarge { what: name, limit });
    }
    let mut out = Vec::new();
    // One byte past the limit tells "exactly at the limit" from "over it".
    file.take(limit + 1)
        .read_to_end(&mut out)
        .map_err(|e| LoadError::Corrupt(format!("{name}: {e}")))?;
    if out.len() as u64 > limit {
        return Err(LoadError::TooLarge { what: name, limit });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packing_is_deterministic_and_unpacks_to_the_same_parts() {
        let a = pack(b"{}", b"[1,2,3]").unwrap();
        assert_eq!(a, pack(b"{}", b"[1,2,3]").unwrap());
        let (m, w) = unpack(&a).unwrap();
        assert_eq!((m.as_slice(), w.as_slice()), (&b"{}"[..], &b"[1,2,3]"[..]));
    }

    #[test]
    fn names_that_could_escape_are_recognised() {
        for bad in ["../x", "a/../b", "/etc/x", "\\x", "a\\b", "C:/x", "a\0b"] {
            assert!(unsafe_name(bad), "{bad}");
        }
        for good in [MANIFEST, WORKSHEET, "assets/pic.png", "a..b"] {
            assert!(!unsafe_name(good), "{good}");
        }
    }

    #[test]
    fn an_entry_over_the_limit_is_refused_not_inflated() {
        let bytes = pack(b"{}", &[b' '; 1000]).unwrap();
        let err = unpack_with(&bytes, 100, 999).unwrap_err();
        assert!(
            matches!(
                err,
                LoadError::TooLarge {
                    what: WORKSHEET,
                    ..
                }
            ),
            "{err}"
        );
        assert!(
            unpack_with(&bytes, 100, 1000).is_ok(),
            "exactly at the limit is fine"
        );
    }
}
