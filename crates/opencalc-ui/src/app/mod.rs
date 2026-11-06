//! The application: state, command dispatch, and the egui shell around them.
//!
//! - `state`: plain data (`AppState`), changed only by `dispatch`.
//! - `dispatch`: turns a command into a state change plus `Effect`s.
//! - `frame`: lays out the window and gathers commands from panels and keys.
//! - This module: owns the worker thread and the ribbon, and carries out effects.

pub(crate) mod dispatch;
mod frame;
mod state;

use eframe::egui;
use opencalc_core::worker::Worker;

use crate::commands::{CommandId, Registry};
use crate::ribbon::defs::opencalc_ribbon;
use crate::ribbon::{Ribbon, RibbonState};
use crate::theme::{self, PaletteKey};
use dispatch::{dispatch, Effect};
pub use state::AppState;

pub struct App {
    pub state: AppState,
    worker: Worker,
    ribbon: Ribbon,
    ribbon_state: RibbonState,
    /// Commands raised outside a frame (tests, startup), handled in the next one.
    pub queue: Vec<CommandId>,
}

impl App {
    pub fn new(ctx: egui::Context, palette: PaletteKey) -> App {
        App::with_state(ctx, AppState::new(palette))
    }

    pub fn with_state(ctx: egui::Context, state: AppState) -> App {
        crate::fonts::install(&ctx);
        theme::apply(&ctx, &state.theme);
        ctx.options_mut(|o| o.zoom_with_keyboard = false);
        let mut app = App {
            state,
            worker: Worker::spawn(move || ctx.request_repaint()),
            ribbon: opencalc_ribbon(),
            ribbon_state: RibbonState::default(),
            queue: Vec::new(),
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
                Effect::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
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
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame(ctx);
    }
}
