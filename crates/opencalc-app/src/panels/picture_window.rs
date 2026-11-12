//! The picture panel: appears in the Properties side panel
//! while one picture region is selected. It chooses or removes the image and holds
//! its description. Like every panel it reads the document and only asks.

use eframe::egui::{self, RichText, Ui};
use opencalc_core::document::{PictureSpec, RegionId, RegionKind};
use opencalc_core::edit::Edit;

use crate::app::AppState;
use crate::canvas::CanvasCmd;
use crate::commands::CommandId;
use crate::theme::type_size;

/// The single selected picture region, if that is what is selected.
pub fn selected_picture(state: &AppState) -> Option<(RegionId, &PictureSpec)> {
    let sheet = state.active_sheet()?;
    let [id] = sheet.canvas.selection.as_slice() else {
        return None;
    };
    match &sheet.doc.region(*id)?.kind {
        RegionKind::Picture(spec) => Some((*id, spec)),
        _ => None,
    }
}

/// The fields of the selected picture, if a picture is what is selected.
pub fn body(ui: &mut Ui, state: &AppState, cmds: &mut Vec<CommandId>, out: &mut Vec<CanvasCmd>) {
    let Some((id, spec)) = selected_picture(state) else {
        return;
    };
    let weak = |ui: &mut egui::Ui, text: String| {
        ui.label(
            RichText::new(text)
                .size(type_size::SMALL)
                .color(state.theme.text_weak),
        );
    };
    match spec.dimensions() {
        Some((w, h)) if !spec.is_empty() => {
            let name = if spec.name.is_empty() {
                "image"
            } else {
                &spec.name
            };
            weak(ui, format!("{name}: {w} by {h} pixels"));
        }
        _ => weak(ui, "No image yet (PNG, up to 2 MB)".into()),
    }
    ui.horizontal(|ui| {
        if ui.button("Choose image...").clicked() {
            cmds.push(CommandId::PictureChoose);
        }
        if ui
            .add_enabled(!spec.is_empty(), egui::Button::new("Remove"))
            .clicked()
        {
            out.push(CanvasCmd::Edit(Edit::SetPicture {
                id,
                spec: Box::new(spec.cleared()),
            }));
            out.push(CanvasCmd::Seal);
        }
    });
    ui.add_space(4.0);
    weak(ui, "Description (for anyone who cannot see it)".into());
    let mut text = spec.alt.clone();
    let response = ui.add(
        egui::TextEdit::multiline(&mut text)
            .desired_rows(2)
            .desired_width(f32::INFINITY)
            .char_limit(opencalc_core::document::MAX_PICTURE_TEXT),
    );
    if response.changed() {
        out.push(CanvasCmd::Edit(Edit::SetPicture {
            id,
            spec: Box::new(spec.clone().with_alt(&text)),
        }));
    }
    if response.lost_focus() {
        out.push(CanvasCmd::Seal);
    }
}
