//! Reading and writing the bytes of a file on disk (std only).
//!
//! Saving never leaves a half-written file: the bytes go to a temporary file in
//! the same folder, are flushed to the disk, and then replace the target in one
//! step. Whatever fails, the old file is untouched and the temporary file is
//! removed.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::MAX_FILE_BYTES;

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A hidden temporary name beside `target`, unique within this process and,
/// with the process ID, across processes.
fn temp_beside(target: &Path) -> PathBuf {
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp = format!(".{name}.{}.{n}.tmp", std::process::id());
    target.with_file_name(temp)
}

/// Replace the file at `path` with `bytes`, all or nothing.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if path.is_dir() {
        return Err(io::Error::other(format!(
            "{} is a folder, not a file",
            path.display()
        )));
    }
    let temp = temp_beside(path);
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp, path)
    })();
    if result.is_err() {
        // The target was not touched; only our own temporary file can be left.
        let _ = fs::remove_file(&temp);
    }
    result
}

/// Read a whole file, refusing one larger than [`MAX_FILE_BYTES`] without
/// reading it.
pub fn read_limited(path: &Path) -> io::Result<Vec<u8>> {
    read_with_limit(path, MAX_FILE_BYTES as u64)
}

pub(super) fn read_with_limit(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    let file = File::open(path).map_err(|e| match e.kind() {
        io::ErrorKind::NotFound => io::Error::new(e.kind(), "the file was not found"),
        _ => e,
    })?;
    let too_big = || {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("the file is larger than {} MB", limit / (1024 * 1024)),
        )
    };
    if file.metadata()?.len() > limit {
        return Err(too_big());
    }
    // The size above can be stale or a lie (a growing file, a pipe), so the read is
    // bounded too.
    let mut out = Vec::new();
    file.take(limit + 1).read_to_end(&mut out)?;
    if out.len() as u64 > limit {
        return Err(too_big());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("opencalc-file-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn entries(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    #[test]
    fn writing_creates_then_replaces_and_leaves_no_temporary_file() {
        let dir = scratch("replace");
        let path = dir.join("a.ocx");
        write_atomic(&path, b"first").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"first");
        write_atomic(&path, b"second, longer").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"second, longer");
        assert_eq!(entries(&dir), ["a.ocx"]);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_failed_write_leaves_the_target_alone_and_no_temporary_file() {
        let dir = scratch("fail");
        // The folder does not exist: nothing can be created.
        assert!(write_atomic(&dir.join("missing").join("a.ocx"), b"x").is_err());
        // The target is a folder: the final step fails after the data was written.
        let target = dir.join("target");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("keep"), b"k").unwrap();
        assert!(write_atomic(&target, b"x").is_err());
        assert_eq!(entries(&dir), ["target"]);
        assert_eq!(entries(&target), ["keep"]);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn reading_refuses_a_file_over_the_limit_and_missing_files_are_errors() {
        let dir = scratch("read");
        let path = dir.join("big");
        fs::write(&path, vec![0u8; 100]).unwrap();
        assert_eq!(read_with_limit(&path, 100).unwrap().len(), 100);
        let err = read_with_limit(&path, 99).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        let missing = read_limited(&dir.join("nope")).unwrap_err();
        assert_eq!(missing.kind(), io::ErrorKind::NotFound);
        assert_eq!(missing.to_string(), "the file was not found");
        fs::remove_dir_all(dir).unwrap();
    }
}
