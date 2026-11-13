//! The pages of the Options window, one function per section.

use eframe::egui::{self, RichText, Ui};
use opencalc_core::document::{Background, PaperSize, MARGIN_RANGE_MM};
use opencalc_core::registry::UnitSystem;

use super::{choice, row, THEMES};
use crate::app::AppState;
use crate::commands::CommandId;
use crate::panels::controls::combo;
use crate::panels::Commands;
use crate::settings::{recovery_label, GridSpacing, Settings, RECOVERY, SCALES};
use crate::theme::{type_size, Theme};

pub(super) fn general(ui: &mut Ui, state: &AppState, next: &mut Settings, out: &mut Commands) {
    let t = &state.theme;
    row(
        ui,
        t,
        "Theme",
        "The colour of the application. Worksheet pages stay white.",
        |ui| {
            for (palette, command) in THEMES {
                if ui
                    .selectable_label(state.palette == palette, command.label())
                    .clicked()
                {
                    out.push(command);
                }
            }
        },
    );
    let scales: Vec<(u16, String)> = SCALES.iter().map(|&s| (s, format!("{s}%"))).collect();
    row(
        ui,
        t,
        "Interface size",
        "Makes the menus, ribbon, panels and text larger or smaller. Zoom on the \
         worksheet is separate.",
        |ui| combo(ui, "options-scale", &mut next.scale, &scales),
    );
    row(
        ui,
        t,
        "Keyboard shortcuts",
        "Every command key, editing key and mouse action.",
        |ui| {
            if ui.button("Show shortcuts").clicked() {
                out.push(CommandId::HelpShortcuts);
            }
        },
    );
}

pub(super) fn worksheet(ui: &mut Ui, state: &AppState, next: &mut Settings, out: &mut Commands) {
    let t = &state.theme;
    let grids: Vec<(GridSpacing, String)> = GridSpacing::ALL
        .iter()
        .map(|&g| (g, g.label().to_owned()))
        .collect();
    row(
        ui,
        t,
        "Grid spacing",
        "The step regions snap to and the arrow keys move the crosshair by. A \
         stronger line marks every inch, or every centimetre on a millimetre grid.",
        |ui| combo(ui, "options-grid", &mut next.grid, &grids),
    );
    row(
        ui,
        t,
        "Show grid",
        "Draw the grid on the page. It is never printed or exported.",
        |ui| {
            let mut on = state.show_grid;
            if ui.checkbox(&mut on, "").changed() {
                out.push(CommandId::ViewGrid);
            }
        },
    );
    row(
        ui,
        t,
        "Snap to grid",
        "Moved and new regions line up on the grid. Hold Alt while dragging to \
         place a region freely.",
        |ui| {
            let mut on = state.snap;
            if ui.checkbox(&mut on, "").changed() {
                out.push(CommandId::ViewSnap);
            }
        },
    );
}

pub(super) fn new_worksheets(ui: &mut Ui, t: &Theme, next: &mut Settings) {
    ui.label(
        RichText::new(
            "What a worksheet made with New starts with. An open worksheet keeps its \
             own: change it with Page setup and the Units panel.",
        )
        .size(type_size::SMALL)
        .color(t.text_weak),
    );
    ui.add_space(10.0);
    let systems: Vec<(UnitSystem, &str)> =
        UnitSystem::ALL.iter().map(|&u| (u, u.label())).collect();
    row(
        ui,
        t,
        "Unit system",
        "The units results are shown in. Values calculate the same in any system.",
        |ui| choice(ui, "options-units", &mut next.units, &systems),
    );
    let papers: Vec<(PaperSize, &str)> = PaperSize::ALL.iter().map(|&p| (p, p.label())).collect();
    row(ui, t, "Paper", "", |ui| {
        choice(ui, "options-paper", &mut next.page.paper, &papers)
    });
    row(ui, t, "Orientation", "", |ui| {
        choice(
            ui,
            "options-orientation",
            &mut next.page.landscape,
            &[(false, "Portrait"), (true, "Landscape")],
        );
    });
    row(ui, t, "Margin", "The same on all four sides.", |ui| {
        let (lo, hi) = MARGIN_RANGE_MM;
        ui.add(
            egui::DragValue::new(&mut next.page.margin_mm)
                .range(lo..=hi)
                .suffix(" mm")
                .speed(0.25)
                .fixed_decimals(0),
        );
    });
    let patterns: Vec<(Background, &str)> =
        Background::ALL.iter().map(|&b| (b, b.label())).collect();
    row(
        ui,
        t,
        "Background",
        "A pattern on the page to work on. Left out of exports unless a worksheet's \
         Page setup includes it.",
        |ui| {
            choice(
                ui,
                "options-background",
                &mut next.page.background,
                &patterns,
            )
        },
    );
}

pub(super) fn files(ui: &mut Ui, state: &AppState, next: &mut Settings, out: &mut Commands) {
    let t = &state.theme;
    let intervals: Vec<(u32, String)> = RECOVERY
        .iter()
        .map(|&n| (n, recovery_label(n).to_owned()))
        .collect();
    row(
        ui,
        t,
        "Recovery files",
        if next.recovery == 0 {
            "Off: changes made since the last save are lost if OpenCalc stops \
             unexpectedly."
        } else {
            "How often unsaved changes are written to a recovery file, offered back \
             if OpenCalc stops unexpectedly."
        },
        |ui| combo(ui, "options-recovery", &mut next.recovery, &intervals),
    );
    let count = state.recent.len();
    let listed = match count {
        0 => "No worksheets are listed in the File menu.".to_owned(),
        1 => "1 worksheet is listed in the File menu.".to_owned(),
        n => format!("{n} worksheets are listed in the File menu."),
    };
    row(ui, t, "Recent worksheets", &listed, |ui| {
        if ui
            .add_enabled(
                count > 0,
                egui::Button::new(CommandId::FileClearRecent.label()),
            )
            .clicked()
        {
            out.push(CommandId::FileClearRecent);
        }
    });
    // Only where the system lets a program do this for itself.
    let (supported, registered) = state.file_type;
    if supported {
        let (help, command) = if registered {
            (
                "Double-clicking an .ocalc file opens it in OpenCalc.",
                CommandId::FileTypeRemove,
            )
        } else {
            (
                "Make double-clicking an .ocalc file open it here, with its icon.",
                CommandId::FileTypeRegister,
            )
        };
        row(ui, t, "Worksheet files", help, |ui| {
            let label = if registered {
                "Stop"
            } else {
                "Open with OpenCalc"
            };
            if ui.button(label).on_hover_text(command.label()).clicked() {
                out.push(command);
            }
        });
    }
}
