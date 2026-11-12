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
    export: VecDeque<Option<PathBuf>>,
    data_open: VecDeque<Option<PathBuf>>,
    image_open: VecDeque<Option<PathBuf>>,
    template_open: VecDeque<Option<PathBuf>>,
    template_save: VecDeque<Option<PathBuf>>,
    data_save: VecDeque<Option<PathBuf>>,
    /// The file names suggested by Save dialogs.
    suggested: Vec<String>,
}

struct Fake(Rc<RefCell<Script>>);

impl Files for Fake {
    fn pick_open(&mut self) -> Option<PathBuf> {
        self.0.borrow_mut().open.pop_front().flatten()
    }

    fn pick_template_open(&mut self) -> Option<PathBuf> {
        self.0.borrow_mut().template_open.pop_front().flatten()
    }

    fn pick_template_save(&mut self, _name: &str, _folder: Option<&Path>) -> Option<PathBuf> {
        self.0.borrow_mut().template_save.pop_front().flatten()
    }

    fn pick_image_open(&mut self, _folder: Option<&Path>) -> Option<PathBuf> {
        self.0.borrow_mut().image_open.pop_front().flatten()
    }

    fn pick_data_open(&mut self, _folder: Option<&Path>) -> Option<PathBuf> {
        self.0.borrow_mut().data_open.pop_front().flatten()
    }

    fn pick_data_save(&mut self, _name: &str, _folder: Option<&Path>) -> Option<PathBuf> {
        self.0.borrow_mut().data_save.pop_front().flatten()
    }

    fn pick_save(&mut self, name: &str, _folder: Option<&Path>) -> Option<PathBuf> {
        let mut s = self.0.borrow_mut();
        s.suggested.push(name.to_owned());
        s.save.pop_front().flatten()
    }

    fn pick_export(
        &mut self,
        _name: &str,
        _kind: crate::export::Kind,
        _folder: Option<&Path>,
    ) -> Option<PathBuf> {
        self.0.borrow_mut().export.pop_front().flatten()
    }
}

pub(super) struct Rig {
    pub h: Harness,
    script: Rc<RefCell<Script>>,
    pub dir: PathBuf,
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

pub(super) fn setup(name: &str, state: AppState) -> Rig {
    let script = Rc::new(RefCell::new(Script::default()));
    let ctx = egui::Context::default();
    let app = App::with_files(ctx.clone(), state, Box::new(Fake(script.clone())));
    let dir = std::env::temp_dir().join(format!("opencalc-app-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    Rig {
        h: Harness {
            ctx,
            app,
            time: 0.0,
            copied: None,
            held: Default::default(),
        },
        script,
        dir,
    }
}

impl Rig {
    pub fn run(&mut self, command: CommandId) {
        self.h.app.queue.push(command);
        // One frame runs the command; a second runs what it queued (close after save).
        self.h.frame();
        self.h.frame();
    }

    fn answer_save(&mut self, path: Option<PathBuf>) {
        self.script.borrow_mut().save.push_back(path);
    }

    pub fn answer_template_open(&mut self, path: Option<PathBuf>) {
        self.script.borrow_mut().template_open.push_back(path);
    }

    pub fn answer_template_save(&mut self, path: Option<PathBuf>) {
        self.script.borrow_mut().template_save.push_back(path);
    }

    pub fn answer_image_open(&mut self, path: Option<PathBuf>) {
        self.script.borrow_mut().image_open.push_back(path);
    }

    pub fn answer_data_open(&mut self, path: Option<PathBuf>) {
        self.script.borrow_mut().data_open.push_back(path);
    }

    pub fn answer_data_save(&mut self, path: Option<PathBuf>) {
        self.script.borrow_mut().data_save.push_back(path);
    }

    pub fn answer_export(&mut self, path: Option<PathBuf>) {
        self.script.borrow_mut().export.push_back(path);
    }

    fn answer_open(&mut self, path: Option<PathBuf>) {
        self.script.borrow_mut().open.push_back(path);
    }

    pub fn state(&self) -> &AppState {
        &self.h.app.state
    }

    pub fn type_into_active(&mut self, source: &str) {
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
    let file = t.dir.join("beam.ocalc");
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
    assert_eq!(t.script.borrow().suggested, ["Untitled.ocalc"]);
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
    let target = t.dir.join("k.ocalc");
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
    let target = t.dir.join("r.ocalc");
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
    let target = t.dir.join("s.ocalc");
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
    let copy = u.dir.join("s.ocalc");
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
    t.answer_save(Some(t.dir.join("one.ocalc")));
    t.run(CommandId::FileSave);
    let first = std::fs::read(t.dir.join("one.ocalc")).unwrap();
    t.type_into_active("b := 2");
    t.answer_save(Some(t.dir.join("two.ocalc")));
    t.run(CommandId::FileSaveAs);
    assert_eq!(std::fs::read(t.dir.join("one.ocalc")).unwrap(), first);
    assert!(t.dir.join("two.ocalc").exists());
    let sheet = &t.state().sheets[0];
    assert_eq!(
        sheet.path.as_deref(),
        Some(t.dir.join("two.ocalc").as_path())
    );
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
    t.answer_save(Some(t.dir.join("no_such_folder").join("x.ocalc")));
    t.run(CommandId::FileSave);
    let sheet = &t.state().sheets[0];
    assert!(sheet.dirty() && sheet.path.is_none());
    assert_eq!(sheet.title(), "Untitled", "a failed save does not rename");
    let message = t.message().expect("an error message");
    assert!(
        message.contains("x.ocalc") && message.contains("still marked as modified"),
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
    t.answer_save(Some(t.dir.join("c.ocalc")));
    t.run(CommandId::DialogSave);
    assert!(t.dir.join("c.ocalc").exists());
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
    t.answer_save(Some(t.dir.join("q.ocalc")));
    t.run(CommandId::DialogSave);
    assert!(t.dir.join("q.ocalc").exists());
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
    let junk = t.dir.join("junk.ocalc");
    std::fs::write(&junk, b"this is not a zip").unwrap();
    t.answer_open(Some(junk));
    t.run(CommandId::FileOpen);
    let message = t.message().expect("a message");
    assert!(
        message.contains("junk.ocalc") && message.contains("not an OpenCalc worksheet"),
        "{message}"
    );
    assert_eq!(t.state().sheets.len(), 1);
    assert!(t.state().recent.is_empty());

    t.run(CommandId::DialogOk);
    t.answer_open(Some(t.dir.join("missing.ocalc")));
    t.run(CommandId::FileOpen);
    assert!(t.message().unwrap().contains("not found"));
}

#[test]
fn the_recent_list_is_newest_first_without_repeats_and_opens_by_index() {
    let mut t = setup("recent", blank());
    for n in 1..=3 {
        t.run(CommandId::FileNew);
        t.type_into_active("x := 1");
        t.answer_save(Some(t.dir.join(format!("f{n}.ocalc"))));
        t.run(CommandId::FileSave);
    }
    let names: Vec<String> = t
        .state()
        .recent
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["f3.ocalc", "f2.ocalc", "f1.ocalc"]);

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
    let target = t.dir.join("d.ocalc");
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

// ---- auto-save and recovery ---------------------------------------------------

use crate::app::filing::Recovery;

impl Rig {
    /// Turn on auto-save into this rig's own folder, ticking every second.
    fn autosave(&mut self) -> PathBuf {
        let dir = self.dir.join("app");
        self.h.app.enable_autosave(Some(dir.clone()));
        self.h.app.autosave_every = 1.0;
        dir
    }

    /// Let the auto-save timer fire.
    fn tick(&mut self) {
        self.h.time += 5.0;
        self.h.frame();
    }

    fn files_in(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(dir)
            .map(|d| {
                d.flatten()
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        v.sort();
        v
    }

    fn recovery_dialog(&self) -> Option<Recovery> {
        match &self.state().dialog {
            Some(Dialog::Recover(r)) => Some(r.clone()),
            _ => None,
        }
    }

    fn first_dirty(&self) -> bool {
        self.state().sheets[0].dirty()
    }
}

/// A second program run over the same folders: a rig whose directory is `dir`.
fn restart(name: &str, dir: &Path, app_dir: Option<PathBuf>) -> Rig {
    let mut next = setup(name, blank());
    let own = std::mem::replace(&mut next.dir, dir.to_owned());
    let _ = std::fs::remove_dir_all(own);
    next.h.app.enable_autosave(app_dir);
    next.h.app.autosave_every = 1.0;
    next
}

#[test]
fn turning_recovery_files_off_writes_none_and_removes_those_written() {
    let mut t = setup("auto-off", blank());
    let dir = t.autosave();
    t.type_into_active("a := 1");
    t.tick();
    assert_eq!(Rig::files_in(&dir).len(), 1, "on: the change is kept");

    let mut off = t.state().settings;
    off.recovery = 0;
    let ctx = t.h.ctx.clone();
    t.h.app.change_settings(&ctx, off);
    t.tick();
    assert!(Rig::files_in(&dir).is_empty(), "off: the old file goes");
    t.type_into_active("b := 2");
    t.tick();
    assert!(
        Rig::files_in(&dir).is_empty(),
        "off: nothing new is written"
    );
}

#[test]
fn a_modified_worksheet_is_written_to_a_recovery_file_and_the_real_file_is_untouched() {
    let mut t = setup("auto1", blank());
    t.autosave();
    t.type_into_active("a := 1");
    let target = t.dir.join("w.ocalc");
    t.answer_save(Some(target.clone()));
    t.run(CommandId::FileSave);
    let saved = std::fs::read(&target).unwrap();
    t.tick();
    assert_eq!(
        Rig::files_in(&t.dir),
        ["w.ocalc"],
        "clean: nothing to recover"
    );

    t.type_into_active("b := 2");
    t.tick();
    let recovery = t.dir.join("w.ocalc.recovery");
    assert!(recovery.exists());
    assert_eq!(
        std::fs::read(&target).unwrap(),
        saved,
        "only a save changes the file"
    );

    // Unchanged since: not written again.
    std::fs::remove_file(&recovery).unwrap();
    t.tick();
    assert!(!recovery.exists());

    // Saving makes the recovery file unnecessary.
    t.type_into_active("c := 3");
    t.tick();
    assert!(recovery.exists());
    t.run(CommandId::FileSave);
    t.h.frame();
    assert!(!recovery.exists());
}

#[test]
fn an_unsaved_worksheet_recovers_to_the_app_folder_and_closing_it_deletes_the_file() {
    let mut t = setup("auto2", blank());
    let dir = t.autosave();
    t.tick();
    assert!(
        Rig::files_in(&dir).is_empty(),
        "a blank clean sheet writes nothing"
    );
    t.type_into_active("a := 1");
    t.tick();
    let files = Rig::files_in(&dir);
    assert_eq!(files.len(), 1, "{files:?}");
    assert!(files[0].starts_with("untitled-") && files[0].ends_with(".ocalc.recovery"));
    t.run(CommandId::FileClose);
    t.run(CommandId::DialogDiscard);
    t.h.frame();
    assert!(Rig::files_in(&dir).is_empty());
}

/// Give `newer` a modification time two seconds after `older`'s. The app offers a
/// recovery file only when it is strictly newer than the saved file, and a file
/// written a moment after another can get the very same timestamp (the system
/// clock ticks about every 15 ms), so a test that writes both back to back would
/// pass or fail by chance. In real use the autosave comes seconds after a save.
fn make_newer(newer: &Path, older: &Path) {
    let base = std::fs::metadata(older).unwrap().modified().unwrap();
    std::fs::OpenOptions::new()
        .write(true)
        .open(newer)
        .unwrap()
        .set_modified(base + std::time::Duration::from_secs(2))
        .unwrap();
}

#[test]
fn after_a_crash_the_next_start_offers_to_restore_and_restoring_leaves_it_unsaved() {
    let mut t = setup("crash1", blank());
    t.autosave();
    t.type_into_active("a := 1");
    let target = t.dir.join("w.ocalc");
    t.answer_save(Some(target.clone()));
    t.run(CommandId::FileSave);
    let saved = std::fs::read(&target).unwrap();
    t.type_into_active("b := 2");
    t.tick();
    let recovery = t.dir.join("w.ocalc.recovery");
    assert!(recovery.exists());
    make_newer(&recovery, &target);

    // The program stops without a clean close. A new one starts and knows the file
    // only from its recent list.
    let mut next = restart("crash1_next", &t.dir, None);
    std::mem::forget(t);
    next.h.app.state.recent = vec![target.clone()];
    next.h.app.scan_recovery();
    next.h.frame();
    let r = next
        .recovery_dialog()
        .expect("a question about the recovery file");
    assert_eq!(r.original.as_deref(), Some(target.as_path()));

    next.run(CommandId::DialogRestore);
    let sheet = &next.state().sheets[0];
    assert_eq!(sheet.doc.regions().len(), 2, "both inputs are back");
    assert!(sheet.dirty(), "restored changes are not saved yet");
    assert_eq!(sheet.path.as_deref(), Some(target.as_path()));
    assert_eq!(std::fs::read(&target).unwrap(), saved);
    assert!(recovery.exists(), "kept until the worksheet is saved");
    next.run(CommandId::FileSave);
    next.h.frame();
    assert!(!next.first_dirty() && !recovery.exists());
}

#[test]
fn discarding_deletes_the_recovery_file_and_cancel_keeps_it() {
    let mut t = setup("crash2", blank());
    let dir = t.autosave();
    t.type_into_active("a := 1");
    t.tick();
    assert_eq!(Rig::files_in(&dir).len(), 1);

    let mut next = restart("crash2_next", &t.dir, Some(dir.clone()));
    std::mem::forget(t);
    next.h.app.scan_recovery();
    next.h.frame();
    let r = next.recovery_dialog().expect("a question");
    assert_eq!(r.original, None, "never saved");
    next.run(CommandId::DialogCancel);
    assert!(next.state().dialog.is_none());
    assert_eq!(Rig::files_in(&dir).len(), 1, "cancel keeps it");

    next.h.app.scan_recovery();
    next.h.frame();
    next.run(CommandId::DialogDropRecovery);
    assert!(Rig::files_in(&dir).is_empty());
}

#[test]
fn an_unsaved_worksheet_restored_from_the_app_folder_keeps_using_the_same_file() {
    let mut t = setup("crash3", blank());
    let dir = t.autosave();
    t.type_into_active("a := 1");
    t.tick();
    let mut next = restart("crash3_next", &t.dir, Some(dir.clone()));
    std::mem::forget(t);
    next.h.app.scan_recovery();
    next.h.frame();
    next.run(CommandId::DialogRestore);
    assert_eq!(next.state().sheets.len(), 1, "the blank sheet made way");
    assert!(next.state().sheets[0].path.is_none() && next.first_dirty());
    // Still one file, even after more changes and ticks.
    next.type_into_active("b := 2");
    next.tick();
    assert_eq!(Rig::files_in(&dir).len(), 1);
    // Saving it somewhere removes the recovery file.
    let kept = next.dir.join("kept.ocalc");
    next.answer_save(Some(kept));
    next.run(CommandId::FileSave);
    next.h.frame();
    assert!(Rig::files_in(&dir).is_empty());
}

#[test]
fn opening_a_file_with_a_newer_recovery_file_asks_and_an_older_one_is_ignored() {
    let mut t = setup("offer", blank());
    t.type_into_active("a := 1");
    let target = t.dir.join("o.ocalc");
    t.answer_save(Some(target.clone()));
    t.run(CommandId::FileSave);
    let recovery = t.dir.join("o.ocalc.recovery");
    let ago = |secs| std::time::SystemTime::now() - std::time::Duration::from_secs(secs);
    let set_age = |path: &Path, secs| {
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(ago(secs))
            .unwrap();
    };
    set_age(&target, 3600);
    std::fs::copy(&target, &recovery).unwrap();
    set_age(&recovery, 60); // newer than the worksheet
    t.run(CommandId::FileClose);
    t.answer_open(Some(target.clone()));
    t.run(CommandId::FileOpen);
    assert!(
        t.recovery_dialog().is_some(),
        "a recovery file that is newer"
    );
    t.run(CommandId::DialogCancel);

    // Now clearly older than the worksheet.
    t.run(CommandId::FileClose);
    set_age(&recovery, 7200);
    t.answer_open(Some(target));
    t.run(CommandId::FileOpen);
    assert!(
        t.state().dialog.is_none(),
        "an out-of-date recovery file is not offered"
    );
}

#[test]
fn a_damaged_recovery_file_says_so_and_is_kept() {
    let mut t = setup("badrec", blank());
    let dir = t.autosave();
    std::fs::create_dir_all(&dir).unwrap();
    let junk = dir.join("untitled-1-1.ocalc.recovery");
    std::fs::write(&junk, b"junk").unwrap();
    t.h.app.scan_recovery();
    t.h.frame();
    t.run(CommandId::DialogRestore);
    assert!(t.message().unwrap().contains("recovery file was kept"));
    assert!(junk.exists());
    assert_eq!(t.state().sheets.len(), 1);
}
