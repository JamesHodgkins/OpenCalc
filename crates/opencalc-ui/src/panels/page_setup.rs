//! The Page setup window: paper size, orientation and margin of the worksheet.
//! Every control pushes a command; the document changes through `SetPageSetup`.

use eframe::egui::{self, Context, RichText};
use opencalc_core::document::{PaperSize, MARGIN_RANGE_MM};

use super::Commands;
use crate::app::AppState;
use crate::commands::CommandId;
use crate::theme::type_size;

pub fn show(ctx: &Context, state: &AppState, out: &mut Commands) {
    if !state.page_setup_open {
        return;
    }
    let Some(sheet) = state.active_sheet() else {
        out.push(CommandId::DocPageSetupClose);
        return;
    };
    let page = sheet.doc.page();
    let mut open = true;
    egui::Window::new("Page setup")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            let heading = |ui: &mut egui::Ui, text: &str| {
                ui.label(
                    RichText::new(text)
                        .size(type_size::SMALL)
                        .color(state.theme.text_weak),
                );
            };
            heading(ui, "PAPER");
            ui.horizontal(|ui| {
                for paper in PaperSize::ALL {
                    if ui
                        .selectable_label(page.paper == paper, paper.label())
                        .clicked()
                    {
                        out.push(CommandId::PagePaper(paper));
                    }
                }
            });
            heading(ui, "ORIENTATION");
            ui.horizontal(|ui| {
                for landscape in [false, true] {
                    let command = CommandId::PageLandscape(landscape);
                    if ui
                        .selectable_label(page.landscape == landscape, command.label())
                        .clicked()
                    {
                        out.push(command);
                    }
                }
            });
            heading(ui, "MARGIN (ALL SIDES)");
            let mut mm = page.margin_mm.round();
            let (lo, hi) = MARGIN_RANGE_MM;
            let drag = egui::DragValue::new(&mut mm)
                .range(lo..=hi)
                .suffix(" mm")
                .speed(0.25);
            if ui.add(drag).changed() {
                out.push(CommandId::PageMarginMm(mm.round() as u8));
            }
        });
    if !open {
        out.push(CommandId::DocPageSetupClose);
    }
}
