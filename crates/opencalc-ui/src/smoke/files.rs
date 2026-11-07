//! Saving, opening, closing and quitting through the real frame loop, with the
//! file dialogs answered by a script instead of the operating system.

#![cfg(test)]

use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use eframe::egui::{self, Key, Modifiers};
use opencalc_core::document::RegionKind;

use super::Harness;
use crate::app::filing::{Dialog, Then};
use crate::app::{App, AppState, Files};
use crate::commands::CommandId;
use crate::sheet::OpenSheet;
use crate::theme::PaletteKey;

#[derive(Default)]
struct Script {
    open: VecDeque<Option<PathBuf>>,
    save: VecDeque<Option<PathBuf>>,
    /// The file names suggested by Save dialogs.
    suggested: Vec<String>,
}

struct Fake(Rc<RefCell<Script>>);

impl Files for Fake {
    fn pick_open(&mut self) -> Option<PathBuf> {
        self.0.borrow_mut().open.pop_front().flatten()
    }

    fn pick_save(&mut self, name: &str, _folder: Option<&Path>) -> Option<PathBuf> {
        let mut s = self.0.borrow_mut();
        s.suggested.push(name.to_owned());
        s.save.pop_front().flatten()
    }
}

struct Rig {
    h: Harness,
    script: Rc<RefCell<Script>>,
    dir: PathBuf,
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn setup(name: &str, state: AppState) -> Rig {
    let script = Rc::new(RefCell::new(Script::default()));
    let ctx = egui::Context::default();
    let app = App::with_files(ctx.clone(), state, Box::new(Fake(script.clone())));
    let dir = std::env::temp_dir().join(format!("opencalc-ui-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    Rig {
        h: Harness {
            ctx,
            app,
            time: 0.0,
            copied: None,
        },
        script,
        dir,
    }
}

impl Rig {
    fn run(&mut self, command: CommandId) {
        self.h.app.queue.push(command);
        // One frame runs the command; a second runs what it queued (close after save).
        self.h.frame();
        self.h.frame();
    }

    fn answer_save(&mut self, path: Option<PathBuf>) {
        self.script.borrow_mut().save.push_back(path);
    }

    fn answer_open(&mut self, path: Option<PathBuf>) {
        self.script.borrow_mut().open.push_back(path);
    }

    fn state(&self) -> &AppState {
        &self.h.app.state
    }

    fn type_into_active(&mut self, source: &str) {
        let i = self.state().active.unwrap();
        self.h.app.state.sheets[i].insert(RegionKind::math(source));
        self.h.frame();
    }

    fn unsaved_dialog(&self) -> Option<(usize, Then)> {
        match &self.state().dialog {
            Some(Dialog::Unsaved { sheet, then }) => Some((*sheet, *then)),
            _ => None,
        }
    }

    fn message(&self) -> Option<String> {
        match &self.state().dialog {
            Some(Dialog::Message { text, .. }) => Some(text.clone()),
            _ => None,
        }
    }
}

fn blank() -> AppState {
    AppState::new(PaletteKey::Neutral)
}

#[test]
fn a_new_worksheet_is_clean_and_typing_makes_it_modified() {
    let mut t = setup("dirty", blank());
    assert!(!t.state().sheets[0].dirty());
    t.type_into_active("a := 1");
    assert!(t.state().sheets[0].dirty());
    let title = crate::panels::title::window_title(t.state());
    assert!(title.starts_with('\u{2022}'), "{title}");
    assert!(crate::panels::doc_tabs::label(&t.state().sheets[0]).starts_with('\u{2022}'));
    // The example worksheet starts unmodified too.
    assert!(!OpenSheet::sample().dirty());
}

#[test]
fn save_asks_for_a_name_writes_the_file_and_clears_the_mark() {
    let mut t = setup("save", blank());
    t.type_into_active("a := 1 m");
    let target = t.dir.join("beam"); // no extension typed
    t.answer_save(Some(target));
    t.run(CommandId::FileSave);
    let file = t.dir.join("beam.ocx");
    assert!(file.exists(), "the extension is added");
    let sheet = &t.state().sheets[0];
    assert!(!sheet.dirty());
    assert_eq!(sheet.path.as_deref(), Some(file.as_path()));
    assert_eq!(
        sheet.title(),
        "beam",
        "a first save names the worksheet after the file"
    );
    assert_eq!(t.state().recent.first(), Some(&file));
    assert_eq!(t.script.borrow().suggested, ["Untitled.ocx"]);
    // Saving again does not ask.
    t.type_into_active("b := 2 m");
    assert!(t.state().sheets[0].dirty());
    t.run(CommandId::FileSave);
    assert!(!t.state().sheets[0].dirty());
    assert_eq!(t.script.borrow().suggested.len(), 1);
    assert!(t.message().is_none());
}

#[test]
fn ctrl_s_saves_through_the_keyboard() {
    let mut t = setup("ctrl_s", blank());
    t.type_into_active("a := 1");
    let target = t.dir.join("k.ocx");
    t.answer_save(Some(target.clone()));
    t.h.key(Key::S, Modifiers::COMMAND);
    t.h.frame();
    assert!(target.exists());
    assert!(!t.state().sheets[0].dirty());
}

#[test]
fn what_was_saved_opens_again_the_same() {
    let mut t = setup("roundtrip", blank());
    t.type_into_active("a := 1 m");
    t.type_into_active("b := a + 2 m");
    let target = t.dir.join("r.ocx");
    t.answer_save(Some(target.clone()));
    t.run(CommandId::FileSave);
    let saved = t.state().sheets[0].doc.clone();
    t.run(CommandId::FileClose);
    assert!(t.state().sheets.is_empty());
    t.answer_open(Some(target));
    t.run(CommandId::FileOpen);
    assert_eq!(t.state().sheets.len(), 1);
    let sheet = &t.state().sheets[0];
    assert_eq!(sheet.doc, saved);
    assert!(!sheet.dirty() && !sheet.can_undo());
    // New regions go below what was opened, not on top of it.
    let bottom = sheet.bounds().unwrap().max.y;
    assert!(sheet.canvas.insert_at.y >= bottom);
    // And it calculates.
    t.h.settle();
    assert!(t.state().output.values.len() >= 2);
}

#[test]
fn opening_an_open_file_switches_to_it_and_a_blank_sheet_is_replaced() {
    let mut t = setup("same", blank());
    t.type_into_active("a := 1");
    let target = t.dir.join("s.ocx");
    t.answer_save(Some(target.clone()));
    t.run(CommandId::FileSave);
    t.run(CommandId::FileNew);
    assert_eq!((t.state().sheets.len(), t.state().active), (2, Some(1)));
    t.answer_open(Some(target.clone()));
    t.run(CommandId::FileOpen);
    t.h.frame();
    assert_eq!((t.state().sheets.len(), t.state().active), (2, Some(0)));

    // A blank untouched worksheet makes way for the file.
    let mut u = setup("replace", blank());
    let copy = u.dir.join("s.ocx");
    std::fs::copy(&target, &copy).unwrap();
    u.answer_open(Some(copy));
    u.run(CommandId::FileOpen);
    assert_eq!(u.state().sheets.len(), 1);
    assert_eq!(u.state().sheets[0].title(), "s");
}

#[test]
fn save_as_writes_a_second_file_and_leaves_the_first_alone() {
    let mut t = setup("saveas", blank());
    t.type_into_active("a := 1");
    t.answer_save(Some(t.dir.join("one.ocx")));
    t.run(CommandId::FileSave);
    let first = std::fs::read(t.dir.join("one.ocx")).unwrap();
    t.type_into_active("b := 2");
    t.answer_save(Some(t.dir.join("two.ocx")));
    t.run(CommandId::FileSaveAs);
    assert_eq!(std::fs::read(t.dir.join("one.ocx")).unwrap(), first);
    assert!(t.dir.join("two.ocx").exists());
    let sheet = &t.state().sheets[0];
    assert_eq!(sheet.path.as_deref(), Some(t.dir.join("two.ocx").as_path()));
    assert_eq!(sheet.title(), "two");
    assert!(!sheet.dirty());
}

#[test]
fn a_cancelled_save_changes_nothing() {
    let mut t = setup("cancel", blank());
    t.type_into_active("a := 1");
    t.answer_save(None);
    t.run(CommandId::FileSave);
    assert!(t.state().sheets[0].dirty() && t.state().sheets[0].path.is_none());
    assert!(t.state().dialog.is_none());
    assert_eq!(std::fs::read_dir(&t.dir).unwrap().count(), 0);
}

#[test]
fn a_failed_save_says_so_and_keeps_the_worksheet_modified() {
    let mut t = setup("fail", blank());
    t.type_into_active("a := 1");
    t.answer_save(Some(t.dir.join("no_such_folder").join("x.ocx")));
    t.run(CommandId::FileSave);
    let sheet = &t.state().sheets[0];
    assert!(sheet.dirty() && sheet.path.is_none());
    assert_eq!(sheet.title(), "Untitled", "a failed save does not rename");
    let message = t.message().expect("an error message");
    assert!(
        message.contains("x.ocx") && message.contains("still marked as modified"),
        "{message}"
    );
    t.run(CommandId::DialogOk);
    assert!(t.state().dialog.is_none());
}

#[test]
fn closing_a_modified_worksheet_asks_and_each_answer_does_what_it_says() {
    let mut t = setup("close", blank());
    t.type_into_active("a := 1");
    t.run(CommandId::CloseSheet(0));
    assert_eq!(t.unsaved_dialog(), Some((0, Then::Close(0))));
    // Cancel keeps everything.
    t.run(CommandId::DialogCancel);
    assert!(t.state().dialog.is_none() && t.state().sheets.len() == 1);
    assert!(t.state().sheets[0].dirty());
    // Save writes, then closes.
    t.run(CommandId::FileClose);
    t.answer_save(Some(t.dir.join("c.ocx")));
    t.run(CommandId::DialogSave);
    assert!(t.dir.join("c.ocx").exists());
    assert!(t.state().sheets.is_empty());
    // Don't save closes without writing.
    t.run(CommandId::FileNew);
    t.type_into_active("b := 2");
    t.run(CommandId::FileClose);
    t.run(CommandId::DialogDiscard);
    assert!(t.state().sheets.is_empty());
    assert_eq!(std::fs::read_dir(&t.dir).unwrap().count(), 1);
}

#[test]
fn cancelling_the_save_dialog_while_closing_keeps_the_worksheet_open() {
    let mut t = setup("close_cancel", blank());
    t.type_into_active("a := 1");
    t.run(CommandId::FileClose);
    t.answer_save(None);
    t.run(CommandId::DialogSave);
    assert_eq!(t.state().sheets.len(), 1);
    assert!(t.state().sheets[0].dirty());
}

#[test]
fn a_clean_worksheet_closes_without_asking() {
    let mut t = setup("clean_close", blank());
    t.run(CommandId::FileClose);
    assert!(t.state().dialog.is_none() && t.state().sheets.is_empty());
}

#[test]
fn quitting_asks_about_each_modified_worksheet_and_cancel_stops_it() {
    let mut t = setup("quit", blank());
    t.type_into_active("a := 1");
    t.run(CommandId::FileNew);
    t.type_into_active("b := 2");
    t.run(CommandId::FileNew); // a third, clean one
    t.run(CommandId::AppQuit);
    assert_eq!(t.unsaved_dialog(), Some((0, Then::Quit)));
    assert!(!t.h.app.allow_close);
    t.run(CommandId::DialogCancel);
    assert!(!t.h.app.allow_close && t.state().sheets.len() == 3);

    t.run(CommandId::AppQuit);
    t.answer_save(Some(t.dir.join("q.ocx")));
    t.run(CommandId::DialogSave);
    assert!(t.dir.join("q.ocx").exists());
    assert_eq!(
        t.unsaved_dialog(),
        Some((1, Then::Quit)),
        "on to the next one"
    );
    t.run(CommandId::DialogDiscard);
    assert!(t.h.app.allow_close, "nothing left to ask about");
}

#[test]
fn quitting_with_nothing_modified_just_quits() {
    let mut t = setup("quit_clean", blank());
    t.run(CommandId::AppQuit);
    assert!(t.state().dialog.is_none());
    assert!(t.h.app.allow_close);
}

#[test]
fn opening_something_that_is_not_a_worksheet_explains_and_changes_nothing() {
    let mut t = setup("bad", blank());
    let junk = t.dir.join("junk.ocx");
    std::fs::write(&junk, b"this is not a zip").unwrap();
    t.answer_open(Some(junk));
    t.run(CommandId::FileOpen);
    let message = t.message().expect("a message");
    assert!(
        message.contains("junk.ocx") && message.contains("not an OpenCalc worksheet"),
        "{message}"
    );
    assert_eq!(t.state().sheets.len(), 1);
    assert!(t.state().recent.is_empty());

    t.run(CommandId::DialogOk);
    t.answer_open(Some(t.dir.join("missing.ocx")));
    t.run(CommandId::FileOpen);
    assert!(t.message().unwrap().contains("not found"));
}

#[test]
fn the_recent_list_is_newest_first_without_repeats_and_opens_by_index() {
    let mut t = setup("recent", blank());
    for n in 1..=3 {
        t.run(CommandId::FileNew);
        t.type_into_active("x := 1");
        t.answer_save(Some(t.dir.join(format!("f{n}.ocx"))));
        t.run(CommandId::FileSave);
    }
    let names: Vec<String> = t
        .state()
        .recent
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["f3.ocx", "f2.ocx", "f1.ocx"]);

    // Close everything, then open the oldest from the list.
    while !t.state().sheets.is_empty() {
        t.run(CommandId::FileClose);
    }
    let wanted = t.state().recent[2].clone();
    t.run(CommandId::OpenRecent(2));
    assert_eq!(t.state().sheets[0].path.as_deref(), Some(wanted.as_path()));
    assert_eq!(t.state().recent[0], wanted, "opening moves it to the front");
    assert_eq!(t.state().recent.len(), 3, "no repeats");
}

#[test]
fn the_save_commands_need_a_worksheet() {
    let mut t = setup("home", blank());
    t.run(CommandId::TabHome);
    let r = t.h.app.registry();
    assert!(!r.allows(CommandId::FileSave) && !r.allows(CommandId::FileSaveAs));
    assert!(r.allows(CommandId::FileOpen));
}

impl Rig {
    /// One frame with `edit` applied to the raw input first.
    fn frame_raw(&mut self, edit: impl FnOnce(&mut egui::RawInput)) {
        self.h.time += 0.05;
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280.0, 780.0),
            )),
            time: Some(self.h.time),
            ..Default::default()
        };
        edit(&mut input);
        let app = &mut self.h.app;
        let _ = self.h.ctx.run(input, |ctx| app.frame(ctx));
    }
}

#[test]
fn closing_the_window_with_unsaved_changes_asks_instead_of_closing() {
    let mut t = setup("window_close", blank());
    t.type_into_active("a := 1");
    t.frame_raw(|i| {
        i.viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .events
            .push(egui::ViewportEvent::Close);
    });
    t.h.frame();
    assert_eq!(t.unsaved_dialog(), Some((0, Then::Quit)));
    assert!(!t.h.app.allow_close);
}

#[test]
fn a_file_dropped_on_the_window_opens() {
    let mut t = setup("drop", blank());
    t.type_into_active("a := 1");
    let target = t.dir.join("d.ocx");
    t.answer_save(Some(target.clone()));
    t.run(CommandId::FileSave);
    t.run(CommandId::FileClose);
    t.frame_raw(|i| {
        i.dropped_files.push(egui::DroppedFile {
            path: Some(target.clone()),
            ..Default::default()
        });
    });
    assert_eq!(t.state().sheets.len(), 1);
    assert_eq!(t.state().sheets[0].path.as_deref(), Some(target.as_path()));
}
