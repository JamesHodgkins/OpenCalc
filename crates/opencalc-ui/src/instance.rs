//! One OpenCalc window, however many times it is started.
//!
//! The first copy to start listens on a port of the local machine and writes the
//! port, with a secret, to a file in the user's own folder. A copy started later
//! reads that file, hands over the files it was asked to open, and exits; the first
//! copy opens them as tabs and comes to the front. If the file is stale (the first
//! copy is gone) the connection fails and the newcomer becomes the first copy.
//! The secret keeps other programs on the machine from sending files to the window.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

/// The most a message may hold, in bytes: a list of paths, nothing more.
const MAX_MESSAGE: u64 = 64 * 1024;
const TIMEOUT: Duration = Duration::from_secs(2);

/// What starting up came to.
pub enum Start {
    /// Another copy has the window and now has the files; this one should exit.
    Forwarded,
    /// This is the first copy. Files sent by later copies arrive here.
    First(Receiver<Vec<PathBuf>>),
}

/// The folder the port file lives in: the user's own, or none if there is none.
pub fn default_dir() -> Option<PathBuf> {
    let var = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let base = if cfg!(windows) {
        var("LOCALAPPDATA")?
    } else {
        var("XDG_RUNTIME_DIR").or_else(|| var("HOME").map(|h| h.join(".cache")))?
    };
    Some(base.join("OpenCalc"))
}

fn file_in(dir: &Path) -> PathBuf {
    dir.join("instance")
}

/// A secret nobody can guess: the system seeds every hasher differently.
fn secret() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut text = String::new();
    for _ in 0..2 {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u128(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos()),
        );
        h.write_u32(std::process::id());
        text.push_str(&format!("{:016x}", h.finish()));
    }
    text
}

/// Try to hand `paths` (possibly none: just "come to the front") to a running copy.
fn forward(dir: &Path, paths: &[PathBuf]) -> bool {
    let Ok(info) = std::fs::read_to_string(file_in(dir)) else {
        return false;
    };
    let mut lines = info.lines();
    let (Some(port), Some(token)) = (
        lines.next().and_then(|p| p.parse::<u16>().ok()),
        lines.next(),
    ) else {
        return false;
    };
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, TIMEOUT) else {
        return false;
    };
    let _ = stream.set_write_timeout(Some(TIMEOUT));
    let mut message = format!("{token}\n");
    for p in paths {
        message.push_str(&p.to_string_lossy());
        message.push('\n');
    }
    stream.write_all(message.as_bytes()).is_ok() && stream.flush().is_ok()
}

/// Read one message: the secret, then paths. `None` if the secret is wrong.
fn read_message(stream: TcpStream, token: &str) -> Option<Vec<PathBuf>> {
    stream.set_read_timeout(Some(TIMEOUT)).ok()?;
    let mut reader = BufReader::new(stream.take(MAX_MESSAGE));
    let mut first = String::new();
    reader.read_line(&mut first).ok()?;
    if first.trim_end() != token {
        return None;
    }
    let mut paths = Vec::new();
    for line in reader.lines() {
        let line = line.ok()?;
        if !line.is_empty() {
            paths.push(PathBuf::from(line));
        }
    }
    Some(paths)
}

/// Listen for later copies. `wake` is called after each message (to redraw).
fn listen(dir: &Path, wake: impl Fn() + Send + 'static) -> Option<Receiver<Vec<PathBuf>>> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).ok()?;
    let port = listener.local_addr().ok()?.port();
    let token = secret();
    std::fs::create_dir_all(dir).ok()?;
    std::fs::write(file_in(dir), format!("{port}\n{token}\n")).ok()?;
    let (send, receive) = channel();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            if let Some(paths) = read_message(stream, &token) {
                if send.send(paths).is_err() {
                    return;
                }
                wake();
            }
        }
    });
    Some(receive)
}

/// Start up: pass the work to a running copy, or become the first. With no usable
/// folder or port, every copy simply opens its own window.
pub fn start(dir: Option<&Path>, paths: &[PathBuf], wake: impl Fn() + Send + 'static) -> Start {
    let Some(dir) = dir else {
        return Start::First(channel().1);
    };
    if forward(dir, paths) {
        return Start::Forwarded;
    }
    Start::First(listen(dir, wake).unwrap_or_else(|| channel().1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("opencalc-instance-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn first(start: Start) -> Receiver<Vec<PathBuf>> {
        match start {
            Start::First(r) => r,
            Start::Forwarded => panic!("expected to be the first copy"),
        }
    }

    #[test]
    fn a_second_copy_hands_its_files_to_the_first_and_is_told_to_exit() {
        let dir = scratch("handover");
        let receiver = first(start(Some(&dir), &[], || {}));
        let files = [PathBuf::from("C:\\work\\a.ocx"), PathBuf::from("b b.ocx")];
        assert!(matches!(start(Some(&dir), &files, || {}), Start::Forwarded));
        let got = receiver.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(got, files);
        // With no files it still arrives: the first window is asked to come forward.
        assert!(matches!(start(Some(&dir), &[], || {}), Start::Forwarded));
        assert!(receiver
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_stale_file_means_nobody_is_there_so_this_copy_becomes_the_first() {
        let dir = scratch("stale");
        std::fs::create_dir_all(&dir).unwrap();
        // A port nothing listens on: bind one and drop it.
        let port = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        std::fs::write(file_in(&dir), format!("{port}\nsecret\n")).unwrap();
        assert!(!forward(&dir, &[]));
        let _receiver = first(start(Some(&dir), &[], || {}));
        // And the file now describes the live one, which a later copy reaches.
        assert!(matches!(start(Some(&dir), &[], || {}), Start::Forwarded));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_message_without_the_secret_is_ignored_and_junk_files_do_no_harm() {
        let dir = scratch("secret");
        let receiver = first(start(Some(&dir), &[], || {}));
        let info = std::fs::read_to_string(file_in(&dir)).unwrap();
        let port: u16 = info.lines().next().unwrap().parse().unwrap();
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        stream.write_all(b"not-the-secret\nC:\\evil.ocx\n").unwrap();
        drop(stream);
        assert!(receiver.recv_timeout(Duration::from_millis(500)).is_err());
        for junk in ["", "x\ny\n", "99999999\ntoken\n", "\n\n"] {
            std::fs::write(file_in(&dir), junk).unwrap();
            assert!(!forward(&dir, &[]), "{junk:?}");
        }
        assert!(
            matches!(start(None, &[], || {}), Start::First(_)),
            "no folder: stand alone"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_secret_is_long_and_differs_each_time() {
        assert_eq!(secret().len(), 32);
        assert_ne!(secret(), secret());
    }
}
