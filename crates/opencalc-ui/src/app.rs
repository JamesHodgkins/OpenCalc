//! Application state, command handling and per-frame orchestration.

use eframe::egui::{self, Frame, Key, Margin, Modifiers, Pos2, Rect};
use opencalc_core::document::RegionKind;
use opencalc_core::eval::Output;
use opencalc_core::worker::Worker;

use crate::canvas::{self, CanvasCmd};
use crate::commands::{CommandContext, Registry};
use crate::ribbon::defs::opencalc_ribbon;
use crate::ribbon::{Ribbon, RibbonState};
use crate::sheet::OpenSheet;
use crate::shell;
use crate::theme::{self, PaletteKey, Theme};

pub struct App {
    pub theme: Theme,
    pub palette: PaletteKey,
    ribbon: Ribbon,
    ribbon_state: RibbonState,
    pub sheets: Vec<OpenSheet>,
    /// `None` shows the Home page.
    pub active: Option<usize>,
    untitled: u32,
    worker: Worker,
    /// How many recalculations have been requested. Tests watch it to prove that
    /// edits which cannot change results do not trigger one.
    pub(crate) revision: u64,
    applied: u64,
    pub output: Output,
    worker_ok: bool,
    pub show_grid: bool,
    pub snap: bool,
    pub live_recalc: bool,
    show_names: bool,
    show_props: bool,
    /// Bumped by "Reset layout" so dock panels forget their saved widths.
    layout_gen: u32,
    /// Window width the default dock widths are a share of: the initial window
    /// width, refreshed by "Reset layout".
    dock_basis: f32,
    /// Screen rectangle of the canvas on the last frame.
    pub(crate) viewport: Rect,
    pub about_open: bool,
    /// Commands raised during a frame, handled after it.
    pub queue: Vec<&'static str>,
}

impl App {
    pub fn new(ctx: egui::Context, palette: PaletteKey) -> App {
        let theme = Theme::palette(palette);
        crate::fonts::install(&ctx);
        theme::apply(&ctx, &theme);
        ctx.options_mut(|o| o.zoom_with_keyboard = false);
        let mut app = App {
            theme,
            palette,
            ribbon: opencalc_ribbon(),
            ribbon_state: RibbonState::default(),
            sheets: vec![OpenSheet::sample()],
            active: Some(0),
            untitled: 1,
            worker: Worker::spawn(move || ctx.request_repaint()),
            revision: 0,
            applied: 0,
            output: Output::default(),
            worker_ok: true,
            show_grid: true,
            snap: true,
            live_recalc: true,
            show_names: true,
            show_props: true,
            layout_gen: 0,
            dock_basis: crate::INITIAL_WIDTH,
            viewport: Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0)),
            about_open: false,
            queue: Vec::new(),
        };
        app.recalculate();
        app
    }

    pub fn registry(&self) -> Registry {
        let mut on = Vec::new();
        for (flag, cmd) in [
            (self.show_grid, "view.grid"),
            (self.snap, "view.snap"),
            (self.show_names, "view.names"),
            (self.show_props, "view.properties"),
        ] {
            if flag {
                on.push(cmd);
            }
        }
        let sheet = self.active.map(|i| &self.sheets[i]);
        let ctx = CommandContext {
            has_sheet: sheet.is_some(),
            has_selection: sheet.is_some_and(|s| !s.canvas.selection.is_empty()),
            can_undo: sheet.is_some_and(OpenSheet::can_undo),
            can_redo: sheet.is_some_and(OpenSheet::can_redo),
        };
        Registry::new(&ctx, &on)
    }

    pub fn recalculate(&mut self) {
        self.revision += 1;
        let snapshot = self
            .active
            .map(|i| self.sheets[i].doc.math_regions())
            .unwrap_or_default();
        self.worker_ok = self.worker.request(self.revision, snapshot);
    }

    fn poll_worker(&mut self) {
        if let Some(r) = self.worker.poll_for(self.revision) {
            self.applied = r.revision;
            self.output = r.output;
        }
    }

    pub fn status_text(&self) -> String {
        if !self.worker_ok {
            "worker stopped".to_owned()
        } else if self.applied == self.revision {
            format!("OpenCalc {} \u{b7} up to date", opencalc_core::version())
        } else {
            "calculating\u{2026}".to_owned()
        }
    }

    pub fn select_sheet(&mut self, i: usize) {
        self.active = Some(i);
        self.output = Output::default();
        self.recalculate();
    }

    pub fn close_sheet(&mut self, i: usize) {
        self.sheets.remove(i);
        self.active = match self.active {
            _ if self.sheets.is_empty() => None,
            Some(a) if a == i => Some(i.min(self.sheets.len() - 1)),
            Some(a) if a > i => Some(a - 1),
            other => other,
        };
        self.output = Output::default();
        self.recalculate();
    }

    fn add_sheet(&mut self, sheet: OpenSheet) {
        self.sheets.push(sheet);
        self.select_sheet(self.sheets.len() - 1);
    }

    pub fn zoom_percent(&self) -> f32 {
        self.active.map_or(1.0, |i| self.sheets[i].view.zoom) * 100.0
    }

    /// Zoom the active sheet about the middle of the canvas.
    fn zoom_by(&mut self, factor: f32) {
        let (centre, origin) = (self.viewport.center(), self.viewport.min);
        if let Some(i) = self.active {
            self.sheets[i].view.zoom_about(centre, origin, factor);
        }
    }

    /// Recalculate if the active sheet's evaluation inputs changed since `before`
    /// (an `eval_revision`). Moving or resizing a region never triggers one.
    fn recalc_if_changed(&mut self, sheet: usize, before: u64) {
        if self.sheets[sheet].doc.eval_revision() != before {
            self.recalculate();
        }
    }

    /// Add a region at the sheet's insertion point (see `OpenSheet::insert`).
    fn insert_region(&mut self, kind: RegionKind) {
        let Some(i) = self.active else { return };
        let before = self.sheets[i].doc.eval_revision();
        self.sheets[i].insert(kind);
        self.recalc_if_changed(i, before);
    }

    fn delete_selection(&mut self) {
        let Some(i) = self.active else { return };
        let before = self.sheets[i].doc.eval_revision();
        self.sheets[i].delete_selection();
        self.recalc_if_changed(i, before);
    }

    fn undo_redo(&mut self, undo: bool) {
        let Some(i) = self.active else { return };
        let before = self.sheets[i].doc.eval_revision();
        let sheet = &mut self.sheets[i];
        if undo {
            sheet.undo();
        } else {
            sheet.redo();
        }
        self.recalc_if_changed(i, before);
    }

    /// Apply what the canvas asked for. Typing recalculates only in live mode.
    fn apply_canvas_cmds(&mut self, sheet: usize, cmds: Vec<CanvasCmd>) {
        let before = self.sheets[sheet].doc.eval_revision();
        for cmd in cmds {
            match cmd {
                CanvasCmd::Edit(edit) => {
                    self.sheets[sheet].edit(edit);
                }
                CanvasCmd::Seal => self.sheets[sheet].seal(),
            }
        }
        if self.live_recalc {
            self.recalc_if_changed(sheet, before);
        }
    }

    fn run(&mut self, ctx: &egui::Context, command: &str) {
        match command {
            "file.new" => {
                self.untitled += 1;
                let title = format!("Worksheet {}", self.untitled);
                self.add_sheet(OpenSheet::new(&title));
            }
            "file.example" => self.add_sheet(OpenSheet::sample()),
            "file.close" => {
                if let Some(i) = self.active {
                    self.close_sheet(i);
                }
            }
            "tab.home" => {
                self.active = None;
                self.recalculate();
            }
            "app.quit" => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            "insert.math" => self.insert_region(RegionKind::math("")),
            "insert.text" => self.insert_region(RegionKind::Text {
                text: String::new(),
            }),
            "insert.area" => self.insert_region(RegionKind::Area {
                title: String::new(),
            }),
            "insert.plot" => self.insert_region(RegionKind::Plot),
            "insert.table" => self.insert_region(RegionKind::Table),
            "insert.picture" => self.insert_region(RegionKind::Picture),
            "region.delete" => self.delete_selection(),
            "edit.undo" => self.undo_redo(true),
            "edit.redo" => self.undo_redo(false),
            "calc.all" => self.recalculate(),
            "view.zoom_in" => self.zoom_by(1.25),
            "view.zoom_out" => self.zoom_by(1.0 / 1.25),
            "view.zoom_100" => {
                let (centre, origin) = (self.viewport.center(), self.viewport.min);
                if let Some(i) = self.active {
                    self.sheets[i].view.set_zoom_about(centre, origin, 1.0);
                }
            }
            "view.zoom_fit" => {
                if let Some(i) = self.active {
                    let size = self.viewport.size();
                    let sheet = &mut self.sheets[i];
                    if let Some(bounds) = sheet.bounds() {
                        sheet.view.fit(bounds, size, 24.0);
                    }
                }
            }
            "view.grid" => self.show_grid = !self.show_grid,
            "view.snap" => self.snap = !self.snap,
            "view.names" => self.show_names = !self.show_names,
            "view.properties" => self.show_props = !self.show_props,
            "view.reset_layout" => {
                self.show_names = true;
                self.show_props = true;
                self.layout_gen += 1;
                self.dock_basis = ctx.screen_rect().width().max(crate::MIN_WIDTH);
            }
            "view.theme" => {
                self.palette = self.palette.other();
                self.theme = Theme::palette(self.palette);
                theme::apply(ctx, &self.theme);
            }
            "help.about" => self.about_open = true,
            _ => {}
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let ctrl = Modifiers::COMMAND;
        let table = [
            ("file.new", ctrl, Key::N),
            ("file.close", ctrl, Key::W),
            ("insert.math", ctrl, Key::M),
            ("insert.text", ctrl, Key::T),
            ("insert.area", ctrl | Modifiers::SHIFT, Key::A),
            ("view.zoom_in", ctrl, Key::Plus),
            ("view.zoom_in", ctrl, Key::Equals),
            ("view.zoom_out", ctrl, Key::Minus),
            ("view.zoom_100", ctrl, Key::Num0),
            ("calc.all", ctrl, Key::F9),
        ];
        let registry = self.registry();
        for (command, mods, key) in table {
            if registry_enabled(&registry, command) && ctx.input_mut(|i| i.consume_key(mods, key)) {
                self.queue.push(command);
            }
        }
        // Undo and redo only when no text field has the keyboard: a text field has
        // its own undo for the characters being typed.
        if !ctx.wants_keyboard_input() {
            let redo_keys = [(ctrl | Modifiers::SHIFT, Key::Z), (ctrl, Key::Y)];
            for (mods, key) in redo_keys {
                if registry_enabled(&registry, "edit.redo")
                    && ctx.input_mut(|i| i.consume_key(mods, key))
                {
                    self.queue.push("edit.redo");
                }
            }
            if registry_enabled(&registry, "edit.undo")
                && ctx.input_mut(|i| i.consume_key(ctrl, Key::Z))
            {
                self.queue.push("edit.undo");
            }
        }
        // Delete only when no text field has the keyboard, so it never eats a character.
        if registry_enabled(&registry, "region.delete")
            && !ctx.wants_keyboard_input()
            && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Delete))
        {
            self.queue.push("region.delete");
        }
    }
}

fn registry_enabled(r: &Registry, command: &str) -> bool {
    crate::ribbon::CommandSource::is_enabled(r, command)
}

impl eframe::App for App {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string("palette", self.palette.key().to_owned());
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame(ctx);
    }
}

impl App {
    /// One frame of the whole UI. Kept apart from `eframe::App::update` so tests
    /// can drive it without a window or an `eframe::Frame`.
    pub fn frame(&mut self, ctx: &egui::Context) {
        self.poll_worker();
        self.shortcuts(ctx);
        let theme = self.theme;
        let width = self.dock_basis;
        let bar = |fill, x: i8, y: i8| {
            Frame::new()
                .fill(fill)
                .inner_margin(Margin::symmetric(x, y))
        };

        egui::TopBottomPanel::top("title")
            .exact_height(32.0)
            .frame(bar(theme.base, 8, 3))
            .show_separator_line(false)
            .show(ctx, |ui| shell::title_row(ui, self));

        egui::TopBottomPanel::top("ribbon-tabs")
            .exact_height(25.0)
            .frame(bar(theme.base, 8, 0))
            .show_separator_line(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| self.ribbon.tab_strip(ui, &theme, &mut self.ribbon_state));
            });

        let registry = self.registry();
        egui::TopBottomPanel::top("ribbon")
            .frame(bar(theme.raised, 6, 4))
            .show_separator_line(false)
            .show(ctx, |ui| {
                let out = self
                    .ribbon
                    .show(ui, &theme, &mut self.ribbon_state, &registry);
                if let Some(c) = out.clicked {
                    self.queue.push(c);
                }
                for (_, rect) in &out.slots {
                    shell::paint_slot(ui, self, *rect);
                }
            });

        egui::TopBottomPanel::top("doc-tabs")
            .exact_height(26.0)
            .frame(bar(theme.base, 8, 0))
            .show_separator_line(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| shell::doc_tabs(ui, self));
            });

        egui::TopBottomPanel::bottom("status")
            .exact_height(24.0)
            .frame(bar(theme.base, 8, 2))
            .show(ctx, |ui| shell::status_bar(ui, self));

        let dock = Frame::new().fill(theme.surface).inner_margin(8);
        if self.show_names {
            egui::SidePanel::left(egui::Id::new(("names", self.layout_gen)))
                .default_width(width * 0.13)
                .width_range(120.0..=400.0)
                .frame(dock)
                .show(ctx, |ui| {
                    // A panel is saved at its frame's width, and a frame shrinks to its
                    // content, so claim the full width or the panel collapses.
                    ui.set_min_width(ui.available_width());
                    shell::names_panel(ui, self)
                });
        }
        if self.show_props {
            egui::SidePanel::right(egui::Id::new(("props", self.layout_gen)))
                .default_width(width * 0.30)
                .width_range(160.0..=600.0)
                .frame(dock)
                .show(ctx, |ui| {
                    ui.set_min_width(ui.available_width());
                    shell::properties_panel(ui, self)
                });
        }

        egui::CentralPanel::default()
            .frame(bar(theme.base, 0, 0))
            .show(ctx, |ui| match self.active {
                None => shell::home_page(ui, self),
                Some(i) => {
                    let env = canvas::Env {
                        theme: &theme,
                        output: &self.output,
                        show_grid: self.show_grid,
                        snap: self.snap,
                    };
                    let sheet = &mut self.sheets[i];
                    let out = canvas::show(
                        ui,
                        &env,
                        sheet.doc.regions(),
                        &mut sheet.view,
                        &mut sheet.canvas,
                    );
                    self.viewport = out.viewport;
                    self.apply_canvas_cmds(i, out.cmds);
                }
            });

        shell::about_window(ctx, self);

        for command in std::mem::take(&mut self.queue) {
            self.run(ctx, command);
        }
    }
}
