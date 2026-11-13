//! The application: state, command dispatch, and the egui shell around them.
//!
//! - `state`: plain data (`AppState`), changed only by `dispatch`.
//! - `dispatch`: turns a command into a state change plus `Effect`s.
//! - `frame`: lays out the window and gathers commands from panels and keys.
//! - This module: owns the worker thread and the ribbon, and carries out effects.

mod actions;
mod arranging;
pub mod association;
mod checking;
mod clipboard;
mod data;
pub(crate) mod dispatch;
mod exporting;
mod files;
pub(crate) mod filing;
pub(crate) mod finding;
pub mod formatting;
mod frame;
pub mod inserting;
pub mod labels;
mod options;
mod recovery;
mod state;
mod templates;
pub mod thumbs;
mod view;

use std::collections::HashMap;
use std::path::PathBuf;

use eframe::egui;
use opencalc_core::worker::Worker;

use crate::commands::{CommandId, Registry};
use crate::ribbon::defs::opencalc_ribbon;
use crate::ribbon::{Ribbon, RibbonState};
use crate::theme;
pub use clipboard::{Clipboard, MemoryClipboard, NativeClipboard};
use dispatch::{dispatch, Effect};
pub use files::{Files, NativeFiles};
pub use recovery::default_dir as recovery_dir;
pub use state::AppState;

pub struct App {
    pub state: AppState,
    worker: Worker,
    pub(crate) ribbon: Ribbon,
    pub(crate) ribbon_state: RibbonState,
    /// Commands raised outside a frame (tests, startup), handled in the next one.
    pub queue: Vec<CommandId>,
    /// How the app asks for file paths.
    files: Box<dyn Files>,
    /// Whether `.ocalc` files open in this program (a setting of the system).
    pub(crate) file_type: Box<dyn association::FileType>,
    /// Files that other copies of the program were asked to open.
    incoming: Option<std::sync::mpsc::Receiver<Vec<PathBuf>>>,
    /// The system clipboard.
    pub(crate) clipboard: Box<dyn Clipboard>,
    /// Set once quitting has been decided, so closing the window is let through.
    pub(crate) allow_close: bool,
    /// The window title last sent, to send it only when it changes.
    window_title: String,
    /// Multiplies the export resolution; tests lower it to stay quick.
    pub(crate) export_shrink: f32,
    /// Whether modified worksheets are written to recovery files (off in tests).
    autosave: bool,
    /// Where recovery files of never-saved worksheets go.
    recovery_dir: Option<PathBuf>,
    pub(crate) autosave_every: f64,
    /// When recovery files were last written, in egui time.
    last_autosave: f64,
    /// The recovery file written for each worksheet (by key), to delete it later.
    recovery_files: HashMap<u32, PathBuf>,
}

impl App {
    #[cfg(test)]
    pub fn new(ctx: egui::Context, palette: crate::theme::PaletteKey) -> App {
        App::with_state(ctx, AppState::new(palette))
    }

    pub fn with_state(ctx: egui::Context, state: AppState) -> App {
        let mut app = App::with_files(ctx, state, Box::new(NativeFiles));
        app.clipboard = Box::new(NativeClipboard);
        app.file_type = Box::new(association::NativeFileType::new());
        app.refresh_file_type();
        app
    }

    pub fn with_files(ctx: egui::Context, state: AppState, files: Box<dyn Files>) -> App {
        crate::fonts::install(&ctx);
        theme::apply(&ctx, &state.theme);
        ctx.set_zoom_factor(state.settings.zoom_factor());
        let autosave_every = state.settings.recovery_every().unwrap_or(recovery::EVERY);
        ctx.options_mut(|o| o.zoom_with_keyboard = false);
        let mut app = App {
            state,
            worker: Worker::spawn(move || ctx.request_repaint()),
            ribbon: opencalc_ribbon(),
            ribbon_state: RibbonState::default(),
            queue: Vec::new(),
            files,
            incoming: None,
            clipboard: Box::new(MemoryClipboard::default()),
            file_type: Box::new(association::MemoryFileType::default()),
            allow_close: false,
            window_title: String::new(),
            export_shrink: 1.0,
            autosave: false,
            recovery_dir: None,
            autosave_every,
            last_autosave: 0.0,
            recovery_files: HashMap::new(),
        };
        app.recalculate();
        app
    }

    /// Take new settings as the Options window would hand them over.
    #[cfg(test)]
    pub fn change_settings(&mut self, ctx: &egui::Context, next: crate::settings::Settings) {
        let effects = options::set_settings(&mut self.state, next);
        self.run_effects(ctx, effects);
    }

    /// Open, as tabs, the files that later copies of the program send here.
    pub fn listen_for_files(&mut self, receiver: std::sync::mpsc::Receiver<Vec<PathBuf>>) {
        self.incoming = Some(receiver);
    }

    /// Open what other copies have sent, and bring the window forward for each.
    pub(crate) fn poll_incoming(&mut self, ctx: &egui::Context) {
        let mut batches = Vec::new();
        if let Some(receiver) = &self.incoming {
            while let Ok(paths) = receiver.try_recv() {
                batches.push(paths);
            }
        }
        for paths in batches {
            for path in paths {
                self.open_path(&path);
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
    }

    /// Read from the system whether `.ocalc` files open here, for the Options window.
    pub(crate) fn refresh_file_type(&mut self) {
        self.state.file_type = (self.file_type.supported(), self.file_type.registered());
    }

    /// Make `.ocalc` files open in this program (or stop doing so), and say if it fails.
    fn set_file_type(&mut self, on: bool) {
        if let Err(reason) = self.file_type.set(on) {
            self.state.message(
                "File type",
                format!("The change could not be made: {reason}"),
            );
        }
        self.refresh_file_type();
    }

    pub fn registry(&self) -> Registry {
        let (choices, face) = formatting::unit_menu(&self.state);
        Registry::new(&self.state.command_context(), &self.state.active_toggles())
            .with_unit_menu(choices, face)
    }

    /// Send the active sheet to the worker under a new revision.
    pub(crate) fn recalculate(&mut self) {
        let calc = &mut self.state.calc;
        calc.revision += 1;
        let (snapshot, plots, units) = self
            .state
            .active_sheet()
            .map(|s| {
                (
                    s.doc.math_regions(),
                    s.doc.plot_inputs(),
                    s.doc.units().unwrap_or_default(),
                )
            })
            .unwrap_or_default();
        let shown = self
            .state
            .active_sheet()
            .map(|s| (s.key, s.doc.eval_revision()));
        self.state.calc.shown_for = shown.map(|(key, _)| key);
        if let (Some(i), Some((_, rev))) = (self.state.active, shown) {
            self.state.sheets[i].calculated = Some(rev);
        }
        self.state.calc.worker_ok =
            self.worker
                .request_full(self.state.calc.revision, snapshot, plots, units);
    }

    /// Take the worker's answer if it is for the revision now on screen.
    fn poll_worker(&mut self) {
        if let Some(r) = self.worker.poll_for(self.state.calc.revision) {
            self.state.calc.applied = r.revision;
            self.state.output = r.output;
        }
    }

    fn run_command(&mut self, ctx: &egui::Context, command: CommandId) {
        if let Some(effects) = dispatch(&mut self.state, command) {
            self.run_effects(ctx, effects);
        }
    }

    fn run_effects(&mut self, ctx: &egui::Context, effects: Vec<Effect>) {
        for effect in effects {
            match effect {
                Effect::Recalculate if self.state.holds_back_recalc() => {}
                Effect::Recalculate | Effect::CalculateNow => self.recalculate(),
                Effect::ApplyTheme => theme::apply(ctx, &self.state.theme),
                Effect::ApplySettings => {
                    ctx.set_zoom_factor(self.state.settings.zoom_factor());
                    if let Some(every) = self.state.settings.recovery_every() {
                        self.autosave_every = every;
                    }
                }
                Effect::Quit => {
                    self.allow_close = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                Effect::ChooseOpen => self.choose_open(),
                Effect::NewFromTemplate => self.new_from_template(),
                Effect::SaveAsTemplate => self.save_as_template(),
                Effect::RefreshFileType => self.refresh_file_type(),
                Effect::SetFileType(on) => self.set_file_type(on),
                Effect::Copy => self.copy(),
                Effect::Cut => self.cut(),
                Effect::Paste => self.paste(),
                Effect::Export(kind) => self.export_active(kind),
                Effect::ExportNow(kind) => self.export_now(kind),
                Effect::ImportCsv => self.import_csv(),
                Effect::ChoosePicture => self.choose_picture(),
                Effect::ExportCsv => self.export_csv(),
                Effect::Restore(r) => self.restore(r),
                Effect::DeleteFile(path) => {
                    let _ = std::fs::remove_file(path);
                }
                Effect::Open(path) => self.open_path(&path),
                Effect::Save { sheet, then, ask } => self.save_sheet(sheet, then, ask),
                Effect::ResetDockWidths => {
                    self.state.dock_basis = ctx.screen_rect().width().max(crate::MIN_WIDTH);
                }
            }
        }
    }
}

impl eframe::App for App {
    /// Behind the bars: a hairline of the window's own colour can show between two
    /// of them (a fractional height, or a scaled display), so it is the base tone
    /// rather than black.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.state.theme.base.to_normalized_gamma_f32()
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string(
            crate::settings::STORAGE_KEY,
            crate::settings::to_text(&self.state),
        );
        // Kept for an older version run after this one.
        storage.set_string("palette", self.state.palette.key().to_owned());
        let recent: Vec<String> = self
            .state
            .recent
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        storage.set_string("recent", recent.join("\n"));
        let pinned: Vec<String> = self
            .state
            .pinned
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        storage.set_string("pinned", pinned.join("\n"));
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        self.claim_tab(ctx, raw_input);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        restore_usable_size(ctx);
        self.frame(ctx);
    }
}

/// A window size remembered from last time can be tiny (a minimised window is
/// saved that way) and the system does not always enforce the minimum on restore:
/// open at the usual size instead, once.
fn restore_usable_size(ctx: &egui::Context) {
    let Some(inner) = ctx.input(|i| i.viewport().inner_rect) else {
        return;
    };
    let minimised = ctx.input(|i| i.viewport().minimized).unwrap_or(false);
    if !minimised && (inner.width() < crate::MIN_WIDTH || inner.height() < 520.0) {
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
            crate::INITIAL_WIDTH,
            780.0,
        )));
    }
}
