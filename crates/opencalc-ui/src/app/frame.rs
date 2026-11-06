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
            .frame(bar(theme.raised, 6, 4))
            .show_separator_line(false)
            .show(ctx, |ui| {
                let out = self
                    .ribbon
                    .show(ui, &theme, &mut self.ribbon_state, &registry);
                cmds.extend(out.clicked);
                for (_, rect) in &out.slots {
                    panels::slots::font_size(ui, &theme, *rect);
                }
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
        if self.state.show_names {
            egui::SidePanel::left(egui::Id::new(("names", self.state.layout_gen)))
                .default_width(width * 0.13)
                .width_range(120.0..=400.0)
                .frame(dock)
                .show(ctx, |ui| {
                    // A panel is saved at its frame's width, and a frame shrinks to its
                    // content, so claim the full width or the panel collapses.
                    ui.set_min_width(ui.available_width());
                    panels::names::show(ui, &self.state);
                });
        }
        if self.state.show_props {
            egui::SidePanel::right(egui::Id::new(("props", self.state.layout_gen)))
                .default_width(width * 0.30)
                .width_range(160.0..=600.0)
                .frame(dock)
                .show(ctx, |ui| {
                    ui.set_min_width(ui.available_width());
                    panels::properties::show(ui, &self.state);
                });
        }

        egui::CentralPanel::default()
            .frame(bar(theme.base, 0, 0))
            .show(ctx, |ui| match self.state.active {
                Some(i) => self.show_canvas(ui, i, ctx),
                None => panels::home::show(ui, &self.state, &mut cmds),
            });

        panels::about::show(ctx, &self.state, &mut cmds);

        let mut pending = std::mem::take(&mut self.queue);
        pending.extend(cmds);
        for command in pending {
            self.run_command(ctx, command);
        }
    }

    fn show_canvas(&mut self, ui: &mut egui::Ui, sheet: usize, ctx: &egui::Context) {
        let state = &mut self.state;
        let env = canvas::Env {
            theme: &state.theme,
            output: &state.output,
            units: state.sheets[sheet].doc.unit_system(),
            show_grid: state.show_grid,
            snap: state.snap,
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
        let effects = super::dispatch::apply_canvas(state, sheet, out.cmds);
        self.run_effects(ctx, effects);
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
    }
}
