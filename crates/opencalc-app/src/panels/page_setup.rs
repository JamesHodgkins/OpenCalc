//! The Page setup window: paper size, orientation and margin of the worksheet.
//! Every control pushes a command; the document changes through `SetPageSetup`.

use eframe::egui::{self, Context, RichText};
use opencalc_core::document::{
    Background, HeaderFooter, PageSetup, PageText, PaperSize, Tint, MARGIN_RANGE_MM,
    MAX_HEADER_CHARS,
};
use opencalc_core::edit::Edit;

use super::controls::combo;
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
            egui::Grid::new("page-setup-grid")
                .num_columns(2)
                .spacing([16.0, 8.0])
                .show(ui, |ui| {
                    ui.label("Paper size");
                    let papers: Vec<(PaperSize, String)> = PaperSize::ALL
                        .iter()
                        .map(|p| (*p, p.label().to_owned()))
                        .collect();
                    let mut paper = page.paper;
                    combo(ui, "page-paper", &mut paper, &papers);
                    if paper != page.paper {
                        out.push(CommandId::PagePaper(paper));
                    }
                    ui.end_row();

                    ui.label("Orientation");
                    let sides: Vec<(bool, String)> = [false, true]
                        .iter()
                        .map(|l| (*l, CommandId::PageLandscape(*l).label().to_owned()))
                        .collect();
                    let mut landscape = page.landscape;
                    combo(ui, "page-orientation", &mut landscape, &sides);
                    if landscape != page.landscape {
                        out.push(CommandId::PageLandscape(landscape));
                    }
                    ui.end_row();

                    ui.label("Margins (all sides)");
                    let mut mm = page.margin_mm.round();
                    let (lo, hi) = MARGIN_RANGE_MM;
                    let drag = egui::DragValue::new(&mut mm)
                        .range(lo..=hi)
                        .suffix(" mm")
                        .speed(0.25);
                    if ui.add(drag).changed() {
                        out.push(CommandId::PageMarginMm(mm.round() as u8));
                    }
                    ui.end_row();

                    background(ui, page, edits);
                });
            ui.add_space(4.0);
            print_background(ui, page, edits);
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

/// The background rows of the settings grid: a pattern and a paper colour.
fn background(ui: &mut egui::Ui, page: PageSetup, edits: &mut Vec<CanvasCmd>) {
    let mut set = |next: PageSetup| {
        edits.push(CanvasCmd::Edit(Edit::SetPageSetup(next)));
        edits.push(CanvasCmd::Seal);
    };
    ui.label("Background");
    let patterns: Vec<(Background, String)> = Background::ALL
        .iter()
        .map(|b| (*b, b.label().to_owned()))
        .collect();
    let mut pattern = page.background;
    combo(ui, "page-background", &mut pattern, &patterns);
    if pattern != page.background {
        set(PageSetup {
            background: pattern,
            ..page
        });
    }
    ui.end_row();

    ui.label("Paper colour");
    let tints: Vec<(Tint, String)> = Tint::ALL
        .iter()
        .map(|t| (*t, t.label().to_owned()))
        .collect();
    let mut tint = page.tint;
    combo(ui, "page-tint", &mut tint, &tints);
    if tint != page.tint {
        set(PageSetup { tint, ..page });
    }
    ui.end_row();
}

/// Whether exports keep the background.
fn print_background(ui: &mut egui::Ui, page: PageSetup, edits: &mut Vec<CanvasCmd>) {
    let mut print = page.print_background;
    if ui
        .checkbox(&mut print, "Include the background when exporting")
        .changed()
    {
        edits.push(CanvasCmd::Edit(Edit::SetPageSetup(PageSetup {
            print_background: print,
            ..page
        })));
        edits.push(CanvasCmd::Seal);
    }
}
