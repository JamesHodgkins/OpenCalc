//! Files sent by a second copy of the program open as tabs in this window.

#![cfg(test)]

use std::sync::mpsc::channel;

use opencalc_core::document::Worksheet;
use opencalc_core::persist::save_bytes;

use super::Harness;

#[test]
fn files_from_another_copy_open_as_tabs_and_a_bad_path_is_reported_not_fatal() {
    let dir = std::env::temp_dir().join(format!("opencalc-incoming-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("from_elsewhere.ocalc");
    std::fs::write(&path, save_bytes(&Worksheet::new("Sent over")).unwrap()).unwrap();

    let mut h = Harness::new();
    h.settle();
    let before = h.app.state.sheets.len();
    let (send, receive) = channel();
    h.app.listen_for_files(receive);

    send.send(vec![path.clone()]).unwrap();
    h.frame();
    assert_eq!(h.app.state.sheets.len(), before + 1, "a new tab");
    assert_eq!(h.sheet().title(), "Sent over");

    // The same file again goes to its tab rather than opening twice.
    send.send(vec![path.clone()]).unwrap();
    h.frame();
    h.frame();
    assert_eq!(h.app.state.sheets.len(), before + 1);

    // No files at all is "come to the front": nothing opens, nothing breaks.
    send.send(Vec::new()).unwrap();
    h.frame();
    assert_eq!(h.app.state.sheets.len(), before + 1);

    // A file that is not there says so and leaves the others alone.
    send.send(vec![dir.join("missing.ocalc")]).unwrap();
    h.frame();
    assert_eq!(h.app.state.sheets.len(), before + 1);
    assert!(h.app.state.dialog.is_some(), "the missing file is reported");
    let _ = std::fs::remove_dir_all(&dir);
}
