//! The File menu behind the app button (SPEC section 4.1).
//!
//! Two columns. The left lists the file commands, with the application commands
//! (Options, About, Quit) at its foot. The right is a pane: the recent worksheets,
//! or the choices under New, Save As or Export, each with a line saying what it
//! does. The left row whose pane is showing takes the pane's tone and joins it,
//! as the active ribbon tab joins the ribbon.
//!
//! The menu is a panel like any other: it reads state and pushes commands. Whether
//! it is open is application state (`app_menu_open`); which row the cursor is on
//! and which pane shows is view state, kept in egui's memory while it is open.
//!
//! Keys while it is open: Up and Down move, Right enters the pane, Left leaves
//! it, Enter or Space runs, 1 to 9 and 0 open a recent worksheet, Escape closes.

use std::path::Path;

use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CornerRadius, FontId, Id, Key, Modifiers, Order, Rect,
    Response, Sense, Stroke, StrokeKind, Ui,
};

use super::{Commands, PanelCx};
use crate::app::AppState;
use crate::commands::CommandId;
use crate::icons;
use crate::ribbon::CommandSource;
use crate::theme::{focus_ring, metrics, type_size, Theme};

/// One of the choices a left-hand row offers in the pane.
pub(crate) struct Choice {
    pub command: CommandId,
    /// What it does, in a few words, under its label.
    pub detail: &'static str,
}

/// What a left-hand row shows in the pane.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Pane {
    /// Nothing: hovering the row leaves the pane as it was.
    Keep,
    Recent,
    Choices(&'static [Choice]),
}

/// A row of the left column.
pub(crate) struct Entry {
    /// What a click on the row runs. With none, a click opens its pane.
    pub run: Option<CommandId>,
    /// The row's label when it runs nothing (otherwise the command's label).
    pub label: &'static str,
    pub icon: char,
    pub pane: Pane,
    /// The heading over its pane.
    pub heading: &'static str,
}

impl PartialEq for Choice {
    fn eq(&self, other: &Choice) -> bool {
        self.command == other.command
    }
}
impl Eq for Choice {}

impl Entry {
    fn label(&self) -> &'static str {
        self.run.map_or(self.label, CommandId::label)
    }

    fn opens_pane(&self) -> bool {
        self.pane != Pane::Keep
    }
}

const fn runs(command: CommandId, icon: char, pane: Pane, heading: &'static str) -> Entry {
    Entry {
        run: Some(command),
        label: "",
        icon,
        pane,
        heading,
    }
}

const NEW: &[Choice] = &[
    Choice {
        command: CommandId::FileNew,
        detail: "Start with an empty page",
    },
    Choice {
        command: CommandId::FileNewFromTemplate,
        detail: "Start from a saved template",
    },
    Choice {
        command: CommandId::FileExample,
        detail: "A worked example to explore and change",
    },
];

const SAVE_AS: &[Choice] = &[
    Choice {
        command: CommandId::FileSaveAs,
        detail: "Save a copy under a new name",
    },
    Choice {
        command: CommandId::FileSaveAsTemplate,
        detail: "Keep it as a starting point for new worksheets",
    },
];

const EXPORT: &[Choice] = &[
    Choice {
        command: CommandId::FileExportPdf,
        detail: "The pages as they print, with searchable text",
    },
    Choice {
        command: CommandId::FileExportPng,
        detail: "Each page as a picture, at 192 dpi",
    },
];

/// The left column, top to bottom. From [`APP_ROWS`] on, the rows sit at the
/// foot of the column, under a divider.
pub(crate) const ENTRIES: [Entry; 9] = [
    runs(
        CommandId::FileNew,
        icons::FILE_NEW,
        Pane::Choices(NEW),
        "New",
    ),
    runs(
        CommandId::FileOpen,
        icons::FOLDER_OPEN,
        Pane::Recent,
        "Recent worksheets",
    ),
    runs(CommandId::FileSave, icons::SAVE, Pane::Keep, ""),
    runs(
        CommandId::FileSaveAs,
        icons::SAVE_AS,
        Pane::Choices(SAVE_AS),
        "Save a copy",
    ),
    Entry {
        run: None,
        label: "Export",
        icon: icons::FILE_OUTPUT,
        pane: Pane::Choices(EXPORT),
        heading: "Export",
    },
    runs(CommandId::FileClose, icons::FILE_CLOSE, Pane::Keep, ""),
    runs(CommandId::AppOptions, icons::SETTINGS, Pane::Keep, ""),
    runs(CommandId::HelpAbout, icons::INFO, Pane::Keep, ""),
    runs(CommandId::AppQuit, icons::QUIT, Pane::Keep, ""),
];
const APP_ROWS: usize = 6;
/// The row whose pane shows when the menu opens: Open, with the recent files.
const OPEN_ROW: usize = 1;

/// Every command the menu can raise (for the test that each command is reachable).
#[cfg(test)]
pub(crate) fn commands() -> Vec<CommandId> {
    let mut all = Vec::new();
    for e in &ENTRIES {
        all.extend(e.run);
        if let Pane::Choices(choices) = e.pane {
            all.extend(choices.iter().map(|c| c.command));
        }
    }
    all.push(CommandId::OpenRecent(0));
    all
}

const LEFT_W: f32 = 232.0;
const PANE_W: f32 = 360.0;
const PAD: f32 = 6.0;
const ROW_H: f32 = 34.0;
const DIVIDER_H: f32 = 9.0;
const HEAD_H: f32 = 34.0;
const RECENT_H: f32 = 42.0;
const CHOICE_H: f32 = 50.0;
/// Recent worksheets shown before the pane scrolls.
const RECENT_SHOWN: usize = 8;
/// How long the pointer rests on a row before its pane opens, so that moving
/// across other rows on the way to the pane does not change it.
const DWELL: f64 = 0.15;
const ICON: f32 = 16.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Col {
    Left,
    Pane,
}

/// What the menu remembers between frames while it is open.
#[derive(Clone, Debug)]
struct View {
    /// The left row whose pane is showing.
    pane: usize,
    /// The highlighted row.
    cursor: Option<(Col, usize)>,
    /// The cursor was last moved by a key, so it shows the focus ring.
    keyboard: bool,
    /// A row the pointer is resting on, and since when.
    dwell: Option<(usize, f64)>,
}

impl Default for View {
    fn default() -> View {
        View {
            pane: OPEN_ROW,
            cursor: None,
            keyboard: false,
            dwell: None,
        }
    }
}

fn view_id() -> Id {
    Id::new("app-menu-view")
}

/// The menu under the app button at `anchor`, if it is open.
pub fn show(ctx: &egui::Context, cx: &PanelCx, out: &mut Commands, anchor: Rect) {
    if !cx.state.app_menu_open {
        ctx.data_mut(|d| d.remove::<View>(view_id()));
        return;
    }
    let mut view: View = ctx.data(|d| d.get_temp(view_id())).unwrap_or_default();
    let m = Menu {
        state: cx.state,
        source: cx.registry,
        theme: &cx.state.theme,
    };
    m.keys(ctx, &mut view, out);

    let frame = egui::Frame::new()
        .fill(m.theme.surface)
        .stroke(Stroke::new(1.0_f32, m.theme.border))
        .corner_radius(CornerRadius::same(metrics::WINDOW_RADIUS))
        .shadow(ctx.style().visuals.popup_shadow)
        .inner_margin(1);
    let area = egui::Area::new(Id::new("app-menu"))
        .order(Order::Foreground)
        .fixed_pos(anchor.left_bottom() + vec2(0.0, 3.0))
        .constrain(true)
        .show(ctx, |ui| {
            frame.show(ui, |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                let height = m.body_height();
                ui.horizontal_top(|ui| {
                    m.left(ui, &mut view, out, height);
                    m.pane(ui, &mut view, out, height);
                });
            });
        });

    let menu = area.response.rect;
    let outside = ctx.input(|i| {
        i.pointer.any_pressed()
            && i.pointer
                .interact_pos()
                .is_some_and(|p| !menu.contains(p) && !anchor.contains(p))
    });
    if outside {
        out.push(CommandId::AppMenuClose);
    }
    ctx.data_mut(|d| d.insert_temp(view_id(), view));
}

struct Menu<'a> {
    state: &'a AppState,
    source: &'a dyn CommandSource,
    theme: &'a Theme,
}

impl Menu<'_> {
    fn recent(&self) -> &[std::path::PathBuf] {
        &self.state.recent
    }

    fn entry_enabled(&self, e: &Entry) -> bool {
        match (e.run, e.pane) {
            (Some(c), _) => self.source.is_enabled(c),
            (None, Pane::Choices(choices)) => {
                choices.iter().any(|c| self.source.is_enabled(c.command))
            }
            (None, _) => true,
        }
    }

    /// Whether each row of a column can take the cursor.
    fn enabled(&self, col: Col, pane: usize) -> Vec<bool> {
        match col {
            Col::Left => ENTRIES.iter().map(|e| self.entry_enabled(e)).collect(),
            Col::Pane => match ENTRIES[pane].pane {
                Pane::Recent => vec![true; self.recent().len()],
                Pane::Choices(choices) => choices
                    .iter()
                    .map(|c| self.source.is_enabled(c.command))
                    .collect(),
                Pane::Keep => Vec::new(),
            },
        }
    }

    /// Tall enough for the left column and for the recent files, so the menu
    /// keeps its size as the pane changes.
    fn body_height(&self) -> f32 {
        let left = PAD * 2.0 + ROW_H * ENTRIES.len() as f32 + DIVIDER_H;
        let recent = HEAD_H + RECENT_H * self.recent().len().clamp(3, RECENT_SHOWN) as f32 + PAD;
        left.max(recent)
    }

    fn keys(&self, ctx: &egui::Context, view: &mut View, out: &mut Commands) {
        let press = |key| ctx.input_mut(|i| i.consume_key(Modifiers::NONE, key));
        if press(Key::Escape) {
            out.push(CommandId::AppMenuClose);
            return;
        }
        let digits = [
            Key::Num1,
            Key::Num2,
            Key::Num3,
            Key::Num4,
            Key::Num5,
            Key::Num6,
            Key::Num7,
            Key::Num8,
            Key::Num9,
            Key::Num0,
        ];
        for (n, key) in digits.into_iter().enumerate() {
            if press(key) && n < self.recent().len() {
                out.push(CommandId::OpenRecent(n as u8));
                return;
            }
        }
        let down = press(Key::ArrowDown);
        let up = press(Key::ArrowUp);
        let right = press(Key::ArrowRight);
        let left = press(Key::ArrowLeft);
        let enter = press(Key::Enter) | press(Key::Space);
        if !(down || up || right || left || enter) {
            return;
        }
        view.keyboard = true;
        let col = view.cursor.map_or(Col::Left, |(c, _)| c);
        let at = view.cursor.map(|(_, i)| i);
        if down || up {
            if let Some(i) = step(&self.enabled(col, view.pane), at, down) {
                view.cursor = Some((col, i));
                if col == Col::Left && ENTRIES[i].opens_pane() {
                    view.pane = i;
                }
            }
        } else if right {
            if let Some((Col::Left, i)) = view.cursor {
                if ENTRIES[i].opens_pane() {
                    view.pane = i;
                }
            }
            if let Some(j) = step(&self.enabled(Col::Pane, view.pane), None, true) {
                view.cursor = Some((Col::Pane, j));
            }
        } else if left {
            view.cursor = Some((Col::Left, view.pane));
        } else if enter {
            match view.cursor {
                Some((Col::Left, i)) => self.run_row(view, i, out),
                Some((Col::Pane, j)) => self.run_choice(view.pane, j, out),
                None => {}
            }
        }
    }

    /// A left row was picked: run it, or with nothing to run, open its pane.
    fn run_row(&self, view: &mut View, i: usize, out: &mut Commands) {
        let entry = &ENTRIES[i];
        match entry.run {
            Some(c) if self.source.is_enabled(c) => out.push(c),
            Some(_) => {}
            None => {
                view.pane = i;
                let first = step(&self.enabled(Col::Pane, i), None, true);
                if view.keyboard {
                    view.cursor = first.map(|j| (Col::Pane, j));
                }
            }
        }
    }

    fn run_choice(&self, pane: usize, j: usize, out: &mut Commands) {
        match ENTRIES[pane].pane {
            Pane::Recent if j < self.recent().len() => out.push(CommandId::OpenRecent(j as u8)),
            Pane::Choices(choices) => {
                if let Some(c) = choices.get(j).filter(|c| self.source.is_enabled(c.command)) {
                    out.push(c.command);
                }
            }
            _ => {}
        }
    }

    fn left(&self, ui: &mut Ui, view: &mut View, out: &mut Commands, height: f32) {
        ui.vertical(|ui| {
            ui.set_width(LEFT_W);
            ui.set_height(height);
            ui.add_space(PAD);
            let now = ui.input(|i| i.time);
            let moved = ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO);
            let mut hovered_any = false;
            for (i, entry) in ENTRIES.iter().enumerate() {
                if i == APP_ROWS {
                    // The application commands sit at the foot of the column.
                    let used = PAD + ROW_H * APP_ROWS as f32;
                    let foot = ROW_H * (ENTRIES.len() - APP_ROWS) as f32 + PAD;
                    let gap = (height - used - foot).max(DIVIDER_H);
                    let (rect, _) = ui.allocate_exact_size(vec2(LEFT_W, gap), Sense::hover());
                    let y = rect.bottom() - DIVIDER_H / 2.0;
                    ui.painter().hline(
                        rect.x_range().shrink(PAD + 6.0),
                        y,
                        Stroke::new(1.0_f32, self.theme.border),
                    );
                }
                let enabled = self.entry_enabled(entry);
                let sense = if enabled {
                    Sense::click()
                } else {
                    Sense::hover()
                };
                let (rect, resp) = ui.allocate_exact_size(vec2(LEFT_W, ROW_H), sense);
                if enabled && resp.hovered() {
                    hovered_any = true;
                    if moved {
                        view.cursor = Some((Col::Left, i));
                        view.keyboard = false;
                    }
                    // Once the keys have moved on, a pointer left resting on a row
                    // must not pull the pane back to it.
                    if !view.keyboard {
                        self.dwell(ui, view, i, now);
                    }
                }
                let row = RowState {
                    enabled,
                    cursor: view.cursor == Some((Col::Left, i)),
                    owner: view.pane == i,
                    pressed: resp.is_pointer_button_down_on(),
                    focus: view.keyboard,
                };
                self.paint_left(ui, rect, entry, row);
                if resp.clicked() {
                    let on_arrow = resp
                        .interact_pointer_pos()
                        .is_some_and(|p| p.x > rect.right() - 28.0);
                    if on_arrow && matches!(entry.pane, Pane::Choices(_)) {
                        view.pane = i;
                    } else {
                        self.run_row(view, i, out);
                    }
                }
            }
            if !hovered_any && moved && !view.keyboard {
                if let Some((Col::Left, _)) = view.cursor {
                    view.cursor = None;
                }
                view.dwell = None;
            }
        });
    }

    /// The pointer rests on row `i`: after a moment its pane opens.
    fn dwell(&self, ui: &Ui, view: &mut View, i: usize, now: f64) {
        if !ENTRIES[i].opens_pane() || view.pane == i {
            view.dwell = None;
            return;
        }
        match view.dwell {
            Some((j, since)) if j == i => {
                if now - since >= DWELL {
                    view.pane = i;
                    view.dwell = None;
                } else {
                    ui.ctx()
                        .request_repaint_after(std::time::Duration::from_secs_f64(DWELL));
                }
            }
            _ => {
                view.dwell = Some((i, now));
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_secs_f64(DWELL));
            }
        }
    }

    fn paint_left(&self, ui: &Ui, rect: Rect, entry: &Entry, row: RowState) {
        let t = self.theme;
        let painter = ui.painter();
        let r = metrics::RADIUS;
        // The pane's row runs to the pane and takes its tone; others are inset.
        let inset = rect.shrink2(vec2(PAD, 1.0));
        if row.owner && row.enabled {
            let joined = inset.with_max_x(rect.right());
            let corners = CornerRadius {
                nw: r,
                sw: r,
                ne: 0,
                se: 0,
            };
            let fill = if row.pressed { t.pressed } else { t.raised };
            painter.rect_filled(joined, corners, fill);
            // A short accent bar marks whose pane is showing.
            let bar =
                Rect::from_center_size(pos2(inset.left() + 1.5, inset.center().y), vec2(3.0, 16.0));
            painter.rect_filled(bar, CornerRadius::same(2), t.accent);
        } else {
            row.paint_bg(painter, inset, t);
        }
        if row.cursor && row.focus && row.enabled {
            focus_ring(painter, inset, t);
        }
        let colour = row.fg(t);
        let cy = rect.center().y;
        painter.text(
            pos2(inset.left() + 14.0 + ICON / 2.0, cy),
            Align2::CENTER_CENTER,
            entry.icon,
            FontId::proportional(ICON),
            colour,
        );
        painter.text(
            pos2(inset.left() + 14.0 + ICON + 10.0, cy),
            Align2::LEFT_CENTER,
            entry.label(),
            FontId::proportional(type_size::BODY),
            colour,
        );
        let mut right = inset.right() - 10.0;
        if matches!(entry.pane, Pane::Choices(_)) {
            painter.text(
                pos2(right - 5.0, cy),
                Align2::CENTER_CENTER,
                icons::CHEVRON_RIGHT,
                FontId::proportional(12.0),
                if row.enabled { t.text_weak } else { colour },
            );
            right -= 16.0;
        }
        if let Some(keys) = entry.run.and_then(|c| self.source.shortcut_text(c)) {
            painter.text(
                pos2(right, cy),
                Align2::RIGHT_CENTER,
                keys,
                FontId::proportional(type_size::SMALL),
                if row.enabled { t.text_weak } else { colour },
            );
        }
    }

    fn pane(&self, ui: &mut Ui, view: &mut View, out: &mut Commands, height: f32) {
        let t = self.theme;
        ui.vertical(|ui| {
            ui.set_width(PANE_W);
            ui.set_height(height);
            let r = metrics::WINDOW_RADIUS - 1;
            let bg = Rect::from_min_size(ui.cursor().min, vec2(PANE_W, height));
            ui.painter().rect_filled(
                bg,
                CornerRadius {
                    nw: 0,
                    sw: 0,
                    ne: r,
                    se: r,
                },
                t.raised,
            );
            let entry = &ENTRIES[view.pane];
            let (head, _) = ui.allocate_exact_size(vec2(PANE_W, HEAD_H), Sense::hover());
            ui.painter().text(
                pos2(head.left() + 16.0, head.center().y + 2.0),
                Align2::LEFT_CENTER,
                entry.heading.to_uppercase(),
                FontId::proportional(type_size::SMALL),
                t.text_weak,
            );
            match entry.pane {
                Pane::Recent if self.recent().is_empty() => self.no_recent(ui, height - HEAD_H),
                Pane::Recent => {
                    egui::ScrollArea::vertical()
                        .id_salt("app-menu-recent")
                        .max_height(height - HEAD_H - PAD)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                            for (j, path) in self.recent().iter().enumerate() {
                                let resp = self.recent_row(ui, view, j, path);
                                if resp.clicked() {
                                    out.push(CommandId::OpenRecent(j as u8));
                                }
                            }
                        });
                }
                Pane::Choices(choices) => {
                    for (j, choice) in choices.iter().enumerate() {
                        let resp = self.choice_row(ui, view, j, choice);
                        if resp.clicked() && self.source.is_enabled(choice.command) {
                            out.push(choice.command);
                        }
                    }
                }
                Pane::Keep => {}
            }
        });
    }

    /// Set the cursor from the pointer, and work out how a pane row looks.
    fn pane_row_state(
        &self,
        ui: &Ui,
        view: &mut View,
        j: usize,
        resp: &Response,
        enabled: bool,
    ) -> RowState {
        let moved = ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO);
        if enabled && resp.hovered() && moved {
            view.cursor = Some((Col::Pane, j));
            view.keyboard = false;
        } else if moved && !view.keyboard && view.cursor == Some((Col::Pane, j)) && !resp.hovered()
        {
            view.cursor = None;
        }
        RowState {
            enabled,
            cursor: view.cursor == Some((Col::Pane, j)),
            owner: false,
            pressed: resp.is_pointer_button_down_on(),
            focus: view.keyboard,
        }
    }

    fn recent_row(&self, ui: &mut Ui, view: &mut View, j: usize, path: &Path) -> Response {
        let t = self.theme;
        let (rect, resp) = ui.allocate_exact_size(vec2(PANE_W, RECENT_H), Sense::click());
        let row = self.pane_row_state(ui, view, j, &resp, true);
        let inset = rect.shrink2(vec2(PAD + 2.0, 1.0));
        let painter = ui.painter();
        row.paint_bg(painter, inset, t);
        if row.cursor && row.focus {
            focus_ring(painter, inset, t);
        }
        let name = path.file_name().map_or_else(
            || path.to_string_lossy().into_owned(),
            |f| f.to_string_lossy().into_owned(),
        );
        let folder = path
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let is_open = self
            .state
            .sheets
            .iter()
            .any(|s| s.path.as_deref() == Some(path));
        // 1 to 9, then 0: the key that opens it.
        if j < 10 {
            painter.text(
                pos2(inset.left() + 12.0, inset.center().y),
                Align2::CENTER_CENTER,
                ((j + 1) % 10).to_string(),
                FontId::proportional(type_size::SMALL),
                t.text_weak,
            );
        }
        let x = inset.left() + 26.0;
        painter.text(
            pos2(x + ICON / 2.0, inset.center().y),
            Align2::CENTER_CENTER,
            icons::FILE_TEXT,
            FontId::proportional(ICON + 2.0),
            t.text_weak,
        );
        let text_x = x + ICON + 10.0;
        let tag_w = if is_open { 40.0 } else { 0.0 };
        let room = inset.right() - 10.0 - tag_w - text_x;
        painter.text(
            pos2(text_x, inset.top() + 13.0),
            Align2::LEFT_CENTER,
            fit(ui, &name, type_size::BODY, room, false),
            FontId::proportional(type_size::BODY),
            t.text,
        );
        painter.text(
            pos2(text_x, inset.top() + 28.0),
            Align2::LEFT_CENTER,
            fit(ui, &folder, type_size::SMALL, room, true),
            FontId::proportional(type_size::SMALL),
            t.text_weak,
        );
        if is_open {
            painter.text(
                pos2(inset.right() - 10.0, inset.center().y),
                Align2::RIGHT_CENTER,
                "Open",
                FontId::proportional(type_size::SMALL),
                t.accent_text,
            );
        }
        let tip = if is_open {
            format!("{}\nAlready open: switches to it", path.display())
        } else {
            path.display().to_string()
        };
        resp.on_hover_text(tip)
    }

    fn choice_row(&self, ui: &mut Ui, view: &mut View, j: usize, choice: &Choice) -> Response {
        let t = self.theme;
        let enabled = self.source.is_enabled(choice.command);
        let sense = if enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let (rect, resp) = ui.allocate_exact_size(vec2(PANE_W, CHOICE_H), sense);
        let row = self.pane_row_state(ui, view, j, &resp, enabled);
        let inset = rect.shrink2(vec2(PAD + 2.0, 1.0));
        let painter = ui.painter();
        row.paint_bg(painter, inset, t);
        if row.cursor && row.focus && enabled {
            focus_ring(painter, inset, t);
        }
        let colour = row.fg(t);
        painter.text(
            pos2(inset.left() + 22.0, inset.center().y),
            Align2::CENTER_CENTER,
            choice.command.icon(),
            FontId::proportional(22.0),
            colour,
        );
        let x = inset.left() + 46.0;
        painter.text(
            pos2(x, inset.top() + 16.0),
            Align2::LEFT_CENTER,
            choice.command.label(),
            FontId::proportional(type_size::BODY),
            colour,
        );
        painter.text(
            pos2(x, inset.top() + 33.0),
            Align2::LEFT_CENTER,
            choice.detail,
            FontId::proportional(type_size::SMALL),
            if enabled { t.text_weak } else { colour },
        );
        if let Some(keys) = self.source.shortcut_text(choice.command) {
            painter.text(
                pos2(inset.right() - 10.0, inset.top() + 16.0),
                Align2::RIGHT_CENTER,
                keys,
                FontId::proportional(type_size::SMALL),
                if enabled { t.text_weak } else { colour },
            );
        }
        if enabled {
            resp
        } else {
            resp.on_hover_text("Needs an open worksheet")
        }
    }

    /// The pane when nothing has been opened or saved yet.
    fn no_recent(&self, ui: &mut Ui, height: f32) {
        let t = self.theme;
        let (rect, _) = ui.allocate_exact_size(vec2(PANE_W, height), Sense::hover());
        let c = pos2(rect.center().x, rect.top() + (height * 0.4).min(110.0));
        let painter = ui.painter();
        painter.text(
            c,
            Align2::CENTER_CENTER,
            icons::HISTORY,
            FontId::proportional(28.0),
            t.text_weak,
        );
        painter.text(
            c + vec2(0.0, 32.0),
            Align2::CENTER_CENTER,
            "No recent worksheets",
            FontId::proportional(type_size::BODY),
            t.text,
        );
        painter.text(
            c + vec2(0.0, 50.0),
            Align2::CENTER_CENTER,
            "Worksheets you open or save appear here.",
            FontId::proportional(type_size::SMALL),
            t.text_weak,
        );
    }
}

/// How a row looks this frame.
#[derive(Clone, Copy)]
struct RowState {
    enabled: bool,
    /// Under the pointer, or reached with the arrow keys.
    cursor: bool,
    /// Its pane is showing.
    owner: bool,
    pressed: bool,
    /// Keys moved the cursor last: show the focus ring.
    focus: bool,
}

impl RowState {
    fn fg(self, t: &Theme) -> Color32 {
        if self.enabled {
            t.text
        } else {
            t.text_disabled
        }
    }

    /// Flat at rest; hover and press as every other control (house style 4).
    fn paint_bg(self, painter: &egui::Painter, rect: Rect, t: &Theme) {
        if !self.enabled {
            return;
        }
        let r = CornerRadius::same(metrics::RADIUS);
        if self.pressed {
            painter.rect_filled(rect, r, t.pressed);
            painter.rect_stroke(
                rect,
                r,
                Stroke::new(1.0_f32, t.border_strong),
                StrokeKind::Inside,
            );
        } else if self.cursor {
            painter.rect_filled(rect, r, t.raised_high);
            painter.rect_stroke(rect, r, Stroke::new(1.0_f32, t.border), StrokeKind::Inside);
        }
    }
}

/// The next row that can take the cursor, wrapping; from `None`, the first (or
/// the last, going up).
fn step(enabled: &[bool], from: Option<usize>, down: bool) -> Option<usize> {
    let n = enabled.len();
    let mut i = match from {
        Some(i) => i.min(n.checked_sub(1)?),
        None if down => n.checked_sub(1)?,
        None => 0,
    };
    for _ in 0..n {
        i = if down { (i + 1) % n } else { (i + n - 1) % n };
        if enabled[i] {
            return Some(i);
        }
    }
    None
}

/// `text` cut to fit `width` with an ellipsis: at the start for a folder (the
/// end of a path is the telling part), else at the end.
fn fit(ui: &Ui, text: &str, size: f32, width: f32, keep_end: bool) -> String {
    let w = |s: &str| crate::ribbon::widgets::text_width(ui, s, size);
    if w(text) <= width {
        return text.to_owned();
    }
    let chars: Vec<char> = text.chars().collect();
    let cut = |k: usize| -> String {
        if keep_end {
            std::iter::once('\u{2026}')
                .chain(chars[k..].iter().copied())
                .collect()
        } else {
            chars[..chars.len() - k]
                .iter()
                .copied()
                .chain(std::iter::once('\u{2026}'))
                .collect()
        }
    };
    // The fewest characters to drop: the width falls as more are dropped.
    let (mut lo, mut hi) = (1, chars.len());
    while lo < hi {
        let mid = (lo + hi) / 2;
        if w(&cut(mid)) <= width {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    cut(lo)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_skips_disabled_rows_and_wraps() {
        let rows = [true, false, true, true];
        assert_eq!(step(&rows, None, true), Some(0));
        assert_eq!(step(&rows, None, false), Some(3));
        assert_eq!(step(&rows, Some(0), true), Some(2));
        assert_eq!(step(&rows, Some(2), false), Some(0));
        assert_eq!(step(&rows, Some(3), true), Some(0));
        assert_eq!(step(&[false, false], None, true), None);
        assert_eq!(step(&[], None, true), None);
    }

    #[test]
    fn every_pane_has_a_heading_and_the_menu_opens_on_recent() {
        for e in &ENTRIES {
            assert_eq!(e.opens_pane(), !e.heading.is_empty(), "{}", e.label());
        }
        assert!(ENTRIES[OPEN_ROW].pane == Pane::Recent);
        assert!(ENTRIES[APP_ROWS..].iter().all(|e| !e.opens_pane()));
    }
}
