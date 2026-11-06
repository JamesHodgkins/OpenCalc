//! Window furniture from spec section 4.1: title row, document tabs, dock
//! panels and status bar. These only draw; commands go on `app.queue` and are
//! handled after the frame.

use eframe::egui::{
    self, popup::popup_below_widget, Align2, CornerRadius, FontId, Id, PopupCloseBehavior, Rect,
    RichText, Sense, Ui, Vec2,
};
use opencalc_core::expr::{AssignKind, Expr};

use crate::app::App;
use crate::format::format_quantity;
use crate::icons;
use crate::ribbon::widgets::{self as w, Look};
use crate::ribbon::CommandSource;
use crate::theme::{metrics, type_size};
use opencalc_core::document::RegionKind;

/// Square logo button (opens the application menu) and the Quick Access Toolbar.
pub fn title_row(ui: &mut Ui, app: &mut App) {
    let theme = app.theme;
    let title = match app.active {
        Some(i) => format!("{} \u{2013} OpenCalc", app.sheets[i].title()),
        None => "OpenCalc".to_owned(),
    };
    // Centred worksheet title, painted first so controls sit on top.
    let full = ui.max_rect();
    ui.painter().text(
        full.center(),
        Align2::CENTER_CENTER,
        title,
        FontId::proportional(type_size::TAB),
        theme.text_weak,
    );

    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
        let (rect, resp) =
            ui.allocate_exact_size(Vec2::splat(metrics::CONTROL_HEIGHT), Sense::click());
        let fill = if resp.is_pointer_button_down_on() {
            theme.accent_pressed
        } else if resp.hovered() {
            theme.accent_hover
        } else {
            theme.accent
        };
        ui.painter()
            .rect_filled(rect, CornerRadius::same(metrics::RADIUS), fill);
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            "O",
            FontId::proportional(15.0),
            theme.text_on_accent,
        );
        let menu = Id::new("app-menu");
        if resp.on_hover_text("Application menu").clicked() {
            ui.memory_mut(|m| m.toggle_popup(menu));
        }
        let anchor = ui.interact(rect, Id::new("app-menu-anchor"), Sense::hover());
        popup_below_widget(ui, menu, &anchor, PopupCloseBehavior::CloseOnClick, |ui| {
            ui.set_min_width(160.0);
            for (cmd, label) in [
                ("file.new", "New worksheet"),
                ("help.about", "About OpenCalc"),
                ("app.quit", "Quit"),
            ] {
                if ui
                    .add(
                        egui::Button::new(label)
                            .frame(false)
                            .min_size(Vec2::new(150.0, 22.0)),
                    )
                    .clicked()
                {
                    app.queue.push(cmd);
                }
            }
        });

        let qat = [
            ("file.new", icons::FILE_NEW, "New"),
            ("file.open", icons::FOLDER_OPEN, "Open"),
            ("file.save", icons::SAVE, "Save"),
            ("edit.undo", icons::UNDO, "Undo"),
            ("edit.redo", icons::REDO, "Redo"),
        ];
        let reg = app.registry();
        for (cmd, icon, label) in qat {
            let look = Look {
                enabled: reg.is_enabled(cmd),
                active: false,
            };
            let tip = match reg.shortcut_text(cmd) {
                Some(s) => format!("{label} ({s})"),
                None => label.to_owned(),
            };
            if w::small_button(ui, &theme, look, Some(icon), None, w::SMALL_H)
                .on_hover_text(tip)
                .clicked()
                && look.enabled
            {
                app.queue.push(cmd);
            }
        }
    });
}

/// Home tab, one tab per worksheet with a close button, and a new-worksheet button.
pub fn doc_tabs(ui: &mut Ui, app: &mut App) {
    let theme = app.theme;
    ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);
    let tab = |ui: &mut Ui, label: &str, width: f32, active: bool| -> (Rect, egui::Response) {
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 26.0), Sense::click());
        let fill = if active {
            theme.base
        } else if resp.hovered() {
            theme.raised
        } else {
            theme.surface
        };
        let radius = CornerRadius {
            nw: 4,
            ne: 4,
            sw: 0,
            se: 0,
        };
        ui.painter().rect_filled(rect, radius, fill);
        let colour = if active { theme.text } else { theme.text_weak };
        ui.painter().text(
            egui::pos2(rect.min.x + 10.0, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(type_size::TAB),
            colour,
        );
        (rect, resp)
    };

    let (_, home) = tab(
        ui,
        &format!("{} Home", icons::HOME),
        74.0,
        app.active.is_none(),
    );
    if home.clicked() {
        app.queue.push("tab.home");
    }
    let mut close = None;
    let mut select = None;
    for i in 0..app.sheets.len() {
        let label = app.sheets[i].title().to_owned();
        let width = w::text_width(ui, &label, type_size::TAB) + 48.0;
        let (rect, resp) = tab(ui, &label, width, app.active == Some(i));
        let x = Rect::from_center_size(
            egui::pos2(rect.max.x - 14.0, rect.center().y),
            Vec2::splat(16.0),
        );
        let x_resp = ui.interact(x, Id::new(("close-tab", i)), Sense::click());
        if x_resp.hovered() {
            ui.painter()
                .rect_filled(x, CornerRadius::same(metrics::RADIUS), theme.raised_high);
        }
        ui.painter().text(
            x.center(),
            Align2::CENTER_CENTER,
            icons::CLOSE,
            FontId::proportional(10.0),
            theme.text_weak,
        );
        if x_resp.on_hover_text("Close worksheet (Ctrl+W)").clicked() {
            close = Some(i);
        } else if resp.clicked() {
            select = Some(i);
        }
    }
    let (_, plus) = tab(ui, "+", 28.0, false);
    if plus.on_hover_text("New worksheet (Ctrl+N)").clicked() {
        app.queue.push("file.new");
    }
    if let Some(i) = close {
        app.close_sheet(i);
    } else if let Some(i) = select {
        app.select_sheet(i);
    }
}

/// Names and Units browser.
pub fn names_panel(ui: &mut Ui, app: &App) {
    let theme = app.theme;
    ui.label(
        RichText::new("NAMES")
            .size(type_size::SMALL)
            .color(theme.text_weak),
    );
    let Some(i) = app.active else {
        ui.label(RichText::new("No worksheet open").color(theme.text_weak));
        return;
    };
    let mut any = false;
    for r in app.sheets[i].doc.regions() {
        let RegionKind::Math(m) = &r.kind else {
            continue;
        };
        let Expr::Assign {
            kind: AssignKind::Define,
            target,
            ..
        } = m.expr()
        else {
            continue;
        };
        any = true;
        let value = match app.output.values.get(&r.id) {
            Some(Ok(q)) => format_quantity(*q),
            Some(Err(_)) => "error".to_owned(),
            None => "\u{2026}".to_owned(),
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new(target).color(theme.text));
            ui.label(
                RichText::new(value)
                    .size(type_size::SMALL)
                    .color(theme.text_weak),
            );
        });
    }
    if !any {
        ui.label(RichText::new("No definitions yet").color(theme.text_weak));
    }
    ui.add_space(8.0);
    ui.label(
        RichText::new("UNITS")
            .size(type_size::SMALL)
            .color(theme.text_weak),
    );
    ui.label(
        RichText::new("mm cm m km in ft\ng kg lb  s\nN kN  Pa kPa MPa")
            .size(type_size::SMALL)
            .color(theme.text_weak),
    );
}

/// Region properties: counts, warnings and errors.
pub fn properties_panel(ui: &mut Ui, app: &App) {
    let theme = app.theme;
    ui.label(
        RichText::new("REGION PROPERTIES")
            .size(type_size::SMALL)
            .color(theme.text_weak),
    );
    let Some(i) = app.active else {
        ui.label(RichText::new("No worksheet open").color(theme.text_weak));
        return;
    };
    let sheet = &app.sheets[i];
    ui.label(format!("{} regions", sheet.doc.regions().len()));
    ui.label("Unit system: SI");
    ui.add_space(8.0);
    for w in &app.output.warnings {
        let opencalc_core::eval::Warning::Redefinition { name, regions } = w;
        ui.colored_label(
            theme.warning,
            format!(
                "{} '{name}' defined {} times",
                icons::WARNING,
                regions.len()
            ),
        );
    }
    for r in sheet.doc.regions() {
        if let (Some(Err(e)), RegionKind::Math(m)) = (app.output.values.get(&r.id), &r.kind) {
            ui.colored_label(
                theme.error,
                format!("{} {}: {e}", icons::WARNING, m.source()),
            );
        }
    }
    match sheet.canvas.selection.as_slice() {
        [] => {}
        [id] => {
            if let Some(r) = sheet.doc.region(*id) {
                ui.add_space(8.0);
                ui.label(
                    RichText::new("SELECTED")
                        .size(type_size::SMALL)
                        .color(theme.text_weak),
                );
                ui.label(format!("{} region", r.kind.label()));
                ui.label(format!("x {:.0}  y {:.0}", r.pos.x, r.pos.y));
                ui.label(format!("{:.0} \u{d7} {:.0}", r.size.w, r.size.h));
            }
        }
        many => {
            ui.add_space(8.0);
            ui.label(format!("{} regions selected", many.len()));
        }
    }
}

/// Icon toggles in fixed order: grid, snap, live recalculation, unit system, warnings, zoom.
pub fn status_bar(ui: &mut Ui, app: &mut App) {
    let theme = app.theme;
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
        let toggle = |ui: &mut Ui, icon: char, label: &str, on: bool, tip: &str| -> bool {
            let width = w::small_width(ui, true, Some(label));
            let look = Look {
                enabled: true,
                active: on,
            };
            w::small_button(ui, &theme, look, Some(icon), Some(label), width)
                .on_hover_text(tip)
                .clicked()
        };
        if toggle(ui, icons::GRID, "Grid", app.show_grid, "Show grid") {
            app.queue.push("view.grid");
        }
        if toggle(ui, icons::SNAP, "Snap", app.snap, "Snap to grid") {
            app.queue.push("view.snap");
        }
        if toggle(
            ui,
            icons::ZAP,
            "Live",
            app.live_recalc,
            "Recalculate as you type",
        ) {
            app.live_recalc = !app.live_recalc;
        }
        // Shown but disabled: per-worksheet unit systems are not built yet (TD-09).
        // A control that looks live but does nothing is worse than an honest stub.
        let units = Look {
            enabled: false,
            active: false,
        };
        let width = w::small_width(ui, true, Some("SI"));
        w::small_button(ui, &theme, units, Some(icons::RULER), Some("SI"), width)
            .on_hover_text("Unit systems are not available yet. All values are SI internally.");
        let n = app.output.warnings.len();
        let colour = if n > 0 {
            theme.warning
        } else {
            theme.text_weak
        };
        ui.label(
            RichText::new(format!("{} {n}", icons::WARNING))
                .color(colour)
                .size(type_size::BODY),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(egui::Button::new(format!("{:.0}%", app.zoom_percent())).frame(false))
                .on_hover_text("Reset zoom")
                .clicked()
            {
                app.queue.push("view.zoom_100");
            }
            ui.label(
                RichText::new(app.status_text())
                    .size(type_size::SMALL)
                    .color(theme.text_weak),
            );
        });
    });
}

/// The Home page shown when no worksheet is open.
pub fn home_page(ui: &mut Ui, app: &mut App) {
    let theme = app.theme;
    ui.add_space(24.0);
    ui.label(RichText::new("OpenCalc").size(26.0).color(theme.text));
    ui.label(
        RichText::new("Engineering worksheets with units and live recalculation")
            .color(theme.text_weak),
    );
    ui.add_space(16.0);
    ui.horizontal(|ui| {
        if ui.button("New worksheet").clicked() {
            app.queue.push("file.new");
        }
        if ui.button("Open example").clicked() {
            app.queue.push("file.example");
        }
    });
}

pub fn about_window(ctx: &egui::Context, app: &mut App) {
    let mut open = app.about_open;
    egui::Window::new("About OpenCalc")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label(format!("OpenCalc {}", opencalc_core::version()));
            ui.label("An original engineering worksheet written in Rust.");
            ui.label("GPL-3.0-or-later. Milestone 1 (skeleton).");
        });
    app.about_open = open;
}

/// Sunken input well drawn in a ribbon slot, as a stand-in for the font-size control.
pub fn paint_slot(ui: &Ui, app: &App, rect: Rect) {
    let theme = app.theme;
    let r = CornerRadius::same(metrics::RADIUS);
    ui.painter().rect_filled(rect, r, theme.base);
    ui.painter().rect_stroke(
        rect,
        r,
        egui::Stroke::new(1.0_f32, theme.border),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        egui::pos2(rect.min.x + 6.0, rect.center().y),
        Align2::LEFT_CENTER,
        "12 pt",
        FontId::proportional(type_size::RIBBON_LABEL),
        theme.text_disabled,
    );
}
