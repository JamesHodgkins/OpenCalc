//! One frame of the window: lay out the panels, gather the commands they and the
//! keyboard raise, then run them.

use eframe::egui::{self, Frame, Margin};

use super::App;
use crate::canvas;
use crate::commands::{CommandId, BINDINGS};
use crate::panels::{self, PanelCx};
use crate::theme::metrics;

impl App {
    /// Kept apart from `eframe::App::update` so tests can drive a frame without a
    /// window or an `eframe::Frame`.
    pub fn frame(&mut self, ctx: &egui::Context) {
        self.poll_worker();
        self.queue_shortcuts(ctx);
        self.window_events(ctx);
        let theme = self.state.theme;
        let registry = self.registry();
        let mut cmds: Vec<CommandId> = Vec::new();
        let width = self.state.dock_basis;
        let bar = |fill, x: i8, y: i8| {
            Frame::new()
                .fill(fill)
                .inner_margin(Margin::symmetric(x, y))
        };

        egui::TopBottomPanel::top("title")
            .exact_height(metrics::TITLE_BAR)
            .frame(bar(theme.base, 8, 3))
            .show_separator_line(false)
            .show(ctx, |ui| {
                let cx = PanelCx {
                    state: &self.state,
                    registry: &registry,
                };
                panels::title::show(ui, &cx, &mut cmds);
            });

        egui::TopBottomPanel::top("ribbon-tabs")
            .exact_height(metrics::TAB_STRIP)
            .frame(bar(theme.base, 8, 0))
            .show_separator_line(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| self.ribbon.tab_strip(ui, &theme, &mut self.ribbon_state));
            });

        egui::TopBottomPanel::top("ribbon")
            .exact_height(metrics::RIBBON)
            .frame(bar(theme.raised, 6, 4))
            .show_separator_line(false)
            .show(ctx, |ui| {
                let out = self
                    .ribbon
                    .show(ui, &theme, &mut self.ribbon_state, &registry);
                cmds.extend(out.clicked);
            });

        egui::TopBottomPanel::top("doc-tabs")
            .exact_height(metrics::DOC_TAB)
            .frame(bar(theme.base, 8, 0))
            .show_separator_line(false)
            .show(ctx, |ui| {
                let cx = PanelCx {
                    state: &self.state,
                    registry: &registry,
                };
                ui.horizontal(|ui| panels::doc_tabs::show(ui, &cx, &mut cmds));
            });

        egui::TopBottomPanel::bottom("status")
            .exact_height(metrics::STATUS_BAR)
            .frame(bar(theme.base, 8, 2))
            .show(ctx, |ui| panels::status::show(ui, &self.state, &mut cmds));

        let dock = Frame::new().fill(theme.surface).inner_margin(8);
        // Edits that panels and the plot window ask for (they only read the document).
        let mut panel_edits: Vec<canvas::CanvasCmd> = Vec::new();
        if self.state.show_functions {
            egui::SidePanel::left(egui::Id::new(("functions", self.state.layout_gen)))
                .default_width(width * 0.20)
                .width_range(180.0..=420.0)
                .frame(dock)
                .show(ctx, |ui| {
                    // A panel is saved at its frame's width, and a frame shrinks to its
                    // content, so claim the full width or the panel collapses.
                    ui.set_min_width(ui.available_width());
                    panels::functions::show(ui, &self.state, &mut panel_edits);
                });
        }
        egui::CentralPanel::default()
            .frame(bar(theme.base, 0, 0))
            .show(ctx, |ui| match self.state.active {
                Some(i) => self.show_canvas(ui, i, ctx),
                None => panels::home::show(ui, &self.state, &mut cmds),
            });

        panels::plot_window::show(ctx, &self.state, &mut panel_edits);
        panels::units::show(ctx, &self.state, &mut cmds, &mut panel_edits);
        if self.state.check.open {
            self.state.refresh_findings();
        }
        panels::check::show(ctx, &self.state, &mut cmds);
        self.state.refresh_find();
        let mut typed: Vec<super::finding::FindCmd> = Vec::new();
        panels::find::show(ctx, &self.state, &mut cmds, &mut typed);
        for cmd in typed {
            self.state.apply_find(cmd);
        }
        if let (Some(i), false) = (self.state.active, panel_edits.is_empty()) {
            let effects = super::dispatch::apply_canvas(&mut self.state, i, panel_edits);
            self.run_effects(ctx, effects);
        }

        panels::about::show(ctx, &self.state, &mut cmds);
        panels::options::show(ctx, &self.state, &mut cmds);
        panels::page_setup::show(ctx, &self.state, &mut cmds);
        panels::dialogs::show(ctx, &self.state, &mut cmds);

        // Seen by the canvas next frame: egui reports IME output only while a text
        // box has focus.
        self.state.text_box_focus = ctx.output(|o| o.ime.is_some());

        let mut pending = std::mem::take(&mut self.queue);
        pending.extend(cmds);
        for command in pending {
            self.run_command(ctx, command);
        }
        self.state.next_recovery();
        self.sync_recovery(ctx);
    }

    fn show_canvas(&mut self, ui: &mut egui::Ui, sheet: usize, ctx: &egui::Context) {
        let found = self.state.found();
        let state = &mut self.state;
        let paper = crate::theme::Theme::paper();
        let env = canvas::Env {
            theme: &state.theme,
            paper: &paper,
            page: state.sheets[sheet].doc.page(),
            output: &state.output,
            units: state.sheets[sheet].doc.unit_system(),
            show_grid: state.show_grid,
            snap: state.snap,
            text_box_focus: state.text_box_focus,
            draft: state.draft,
            found: &found,
        };
        let open = &mut state.sheets[sheet];
        let out = canvas::show(
            ui,
            &env,
            open.doc.regions(),
            &mut open.view,
            &mut open.canvas,
        );
        state.viewport = out.viewport;
        state.page = out.page;
        let effects = super::dispatch::apply_canvas(state, sheet, out.cmds);
        self.run_effects(ctx, effects);
    }

    /// Files dropped on the window, a request to close it, and the window title.
    fn window_events(&mut self, ctx: &egui::Context) {
        let (dropped, close) =
            ctx.input(|i| (i.raw.dropped_files.clone(), i.viewport().close_requested()));
        for file in dropped {
            if let Some(path) = file.path {
                self.open_path(&path);
            }
        }
        if close && !self.allow_close && self.state.sheets.iter().any(|s| s.dirty()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.queue.push(CommandId::AppQuit);
        }
        let title = crate::panels::title::window_title(&self.state);
        if title != self.window_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.window_title = title;
        }
    }

    /// Consume the key bindings that apply right now and queue their commands.
    fn queue_shortcuts(&mut self, ctx: &egui::Context) {
        let registry = self.registry();
        let typing = ctx.wants_keyboard_input();
        for b in BINDINGS {
            if (b.when_not_typing && typing) || !registry.allows(b.command) {
                continue;
            }
            if ctx.input_mut(|i| i.consume_key(b.mods, b.key)) {
                self.queue.push(b.command);
            }
        }
        // Windowing systems report Ctrl+C, X and V as these events, not as keys.
        // With a text box or the math editor focused they belong to it.
        if !typing {
            let seen = ctx.input(|i| {
                [
                    i.events.iter().any(|e| matches!(e, egui::Event::Copy)),
                    i.events.iter().any(|e| matches!(e, egui::Event::Cut)),
                    i.events.iter().any(|e| matches!(e, egui::Event::Paste(_))),
                ]
            });
            let commands = [
                CommandId::ClipboardCopy,
                CommandId::ClipboardCut,
                CommandId::ClipboardPaste,
            ];
            for (on, command) in seen.into_iter().zip(commands) {
                if on && registry.allows(command) && !self.queue.contains(&command) {
                    self.queue.push(command);
                }
            }
        }
    }
}
