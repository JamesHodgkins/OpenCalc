//! The Page setup window: paper size, orientation and margin of the worksheet.
//! Every control pushes a command; the document changes through `SetPageSetup`.

use eframe::egui::{self, Context, RichText};
use opencalc_core::document::{
    HeaderFooter, PageText, PaperSize, MARGIN_RANGE_MM, MAX_HEADER_CHARS,
};
use opencalc_core::edit::Edit;

use super::Commands;
use crate::app::AppState;
use crate::canvas::CanvasCmd;
use crate::commands::CommandId;
use crate::theme::type_size;

pub fn show(ctx: &Context, state: &AppState, out: &mut Commands, edits: &mut Vec<CanvasCmd>) {
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
            headers(ui, state, sheet.doc.headers(), edits);
        });
    if !open {
        out.push(CommandId::DocPageSetupClose);
    }
}

/// One band's three boxes (left, centre, right). `pick` finds the band in the
/// whole setup, so a change rebuilds the setup with only that band replaced.
fn band(
    ui: &mut egui::Ui,
    salt: &str,
    all: &HeaderFooter,
    pick: fn(&mut HeaderFooter) -> &mut PageText,
    edits: &mut Vec<CanvasCmd>,
) {
    let mut current = all.clone();
    let text = pick(&mut current).clone();
    ui.horizontal(|ui| {
        for (n, (hint, value)) in [
            ("left", &text.left),
            ("centre", &text.centre),
            ("right", &text.right),
        ]
        .into_iter()
        .enumerate()
        {
            let mut buffer = value.clone();
            let response = ui.add(
                egui::TextEdit::singleline(&mut buffer)
                    .id_salt((salt, n))
                    .hint_text(hint)
                    .desired_width(110.0)
                    .char_limit(MAX_HEADER_CHARS),
            );
            if response.changed() {
                let mut next = all.clone();
                let piece = pick(&mut next);
                match n {
                    0 => piece.left = buffer,
                    1 => piece.centre = buffer,
                    _ => piece.right = buffer,
                }
                edits.push(CanvasCmd::Edit(Edit::SetHeaders(Box::new(next))));
            }
            if response.lost_focus() {
                edits.push(CanvasCmd::Seal);
            }
        }
    });
}

/// The header and footer section of the window.
fn headers(ui: &mut egui::Ui, state: &AppState, all: &HeaderFooter, edits: &mut Vec<CanvasCmd>) {
    let heading = |ui: &mut egui::Ui, text: &str| {
        ui.label(
            RichText::new(text)
                .size(type_size::SMALL)
                .color(state.theme.text_weak),
        );
    };
    ui.add_space(6.0);
    heading(ui, "HEADER AND FOOTER  ({page}  {pages}  {title})");
    heading(
        ui,
        if all.different_first {
            "Header, later pages"
        } else {
            "Header"
        },
    );
    band(ui, "hf-header", all, |h| &mut h.header, edits);
    heading(
        ui,
        if all.different_first {
            "Footer, later pages"
        } else {
            "Footer"
        },
    );
    band(ui, "hf-footer", all, |h| &mut h.footer, edits);
    let mut first = all.different_first;
    if ui.checkbox(&mut first, "Different first page").changed() {
        let mut next = all.clone();
        next.different_first = first;
        edits.push(CanvasCmd::Edit(Edit::SetHeaders(Box::new(next))));
        edits.push(CanvasCmd::Seal);
    }
    if all.different_first {
        heading(ui, "Header, first page");
        band(ui, "hf-first-header", all, |h| &mut h.first_header, edits);
        heading(ui, "Footer, first page");
        band(ui, "hf-first-footer", all, |h| &mut h.first_footer, edits);
    }
}
