//! The right-click menu of the worksheet: what can be done with the selected
//! regions, or, on blank page, what can be put there. Items come from the command
//! registry, so labels, shortcuts and what is enabled match the ribbon.

use eframe::egui::{self, Button, Ui};

use super::Commands;
use crate::commands::{CommandId, Registry};
use crate::ribbon::CommandSource;

/// What the menu offers, in order; `None` is a separator.
pub fn items(has_selection: bool) -> Vec<Option<CommandId>> {
    use CommandId::*;
    if has_selection {
        vec![
            Some(ClipboardCut),
            Some(ClipboardCopy),
            Some(ClipboardPaste),
            Some(EditDuplicate),
            Some(RegionDelete),
            None,
            Some(RegionLock),
            Some(RegionTag),
            None,
            Some(ArrangeToFront),
            Some(ArrangeForward),
            Some(ArrangeBackward),
            Some(ArrangeToBack),
            None,
            Some(EditSelectAll),
        ]
    } else {
        vec![
            Some(ClipboardPaste),
            None,
            Some(InsertMath),
            Some(InsertText),
            None,
            Some(EditSelectAll),
        ]
    }
}

/// The menu on the canvas's response (shown while a right click has opened it).
pub fn show(
    response: &egui::Response,
    registry: &Registry,
    has_selection: bool,
    out: &mut Commands,
) {
    response.context_menu(|ui| {
        ui.set_min_width(190.0);
        for item in items(has_selection) {
            match item {
                None => {
                    ui.separator();
                }
                Some(command) => row(ui, registry, command, out),
            }
        }
    });
}

fn row(ui: &mut Ui, registry: &Registry, command: CommandId, out: &mut Commands) {
    let mut button = Button::new(command.label()).selected(registry.is_active(command));
    if let Some(shortcut) = registry.shortcut_text(command) {
        button = button.shortcut_text(shortcut);
    }
    let response = ui.add_enabled(registry.is_enabled(command), button);
    if response.clicked() {
        out.push(command);
        ui.close_menu();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_selection_gets_edit_arrange_and_a_blank_page_gets_paste_and_insert() {
        let some = items(true);
        for c in [
            CommandId::ClipboardCut,
            CommandId::ClipboardCopy,
            CommandId::RegionDelete,
            CommandId::ArrangeToFront,
        ] {
            assert!(some.contains(&Some(c)), "{c:?}");
        }
        let none = items(false);
        assert!(none.contains(&Some(CommandId::InsertMath)));
        assert!(!none.contains(&Some(CommandId::RegionDelete)));
        // No menu opens or ends on a separator, or has two in a row.
        for list in [some, none] {
            assert!(list.first().is_some_and(Option::is_some));
            assert!(list.last().is_some_and(Option::is_some));
            assert!(list.windows(2).all(|w| w[0].is_some() || w[1].is_some()));
        }
    }
}
