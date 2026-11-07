//! The application: state, command dispatch, and the egui shell around them.
//!
//! - `state`: plain data (`AppState`), changed only by `dispatch`.
//! - `dispatch`: turns a command into a state change plus `Effect`s.
//! - `frame`: lays out the window and gathers commands from panels and keys.
//! - This module: owns the worker thread and the ribbon, and carries out effects.

pub(crate) mod dispatch;
mod files;
pub(crate) mod filing;
pub(crate) mod finding;
mod frame;
mod recovery;
mod state;

use std::collections::HashMap;
use std::path::PathBuf;

use eframe::egui;
use opencalc_core::worker::Worker;

use crate::commands::{CommandId, Registry};
use crate::ribbon::defs::opencalc_ribbon;
use crate::ribbon::{Ribbon, RibbonState};
use crate::theme::{self, PaletteKey};
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
    /// Set once quitting has been decided, so closing the window is let through.
    pub(crate) allow_close: bool,
    /// The window title last sent, to send it only when it changes.
    window_title: String,
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
    pub fn new(ctx: egui::Context, palette: PaletteKey) -> App {
        App::with_state(ctx, AppState::new(palette))
    }

    pub fn with_state(ctx: egui::Context, state: AppState) -> App {
        App::with_files(ctx, state, Box::new(NativeFiles))
    }

    pub fn with_files(ctx: egui::Context, state: AppState, files: Box<dyn Files>) -> App {
        crate::fonts::install(&ctx);
        theme::apply(&ctx, &state.theme);
        ctx.options_mut(|o| o.zoom_with_keyboard = false);
        let mut app = App {
            state,
            worker: Worker::spawn(move || ctx.request_repaint()),
            ribbon: opencalc_ribbon(),
            ribbon_state: RibbonState::default(),
            queue: Vec::new(),
            files,
            allow_close: false,
            window_title: String::new(),
            autosave: false,
            recovery_dir: None,
            autosave_every: recovery::EVERY,
            last_autosave: 0.0,
            recovery_files: HashMap::new(),
        };
        app.recalculate();
        app
    }

    pub fn registry(&self) -> Registry {
        Registry::new(&self.state.command_context(), &self.state.active_toggles())
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
                Effect::Recalculate => self.recalculate(),
                Effect::ApplyTheme => theme::apply(ctx, &self.state.theme),
                Effect::Quit => {
                    self.allow_close = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                Effect::ChooseOpen => self.choose_open(),
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
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string("palette", self.state.palette.key().to_owned());
        let recent: Vec<String> = self
            .state
            .recent
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        storage.set_string("recent", recent.join("\n"));
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame(ctx);
    }
}
