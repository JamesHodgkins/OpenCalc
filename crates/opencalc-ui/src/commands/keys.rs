//! Key bindings: the one place a shortcut is defined. Both the behaviour (the
//! frame loop consumes these keys) and the tooltip text are derived from it, so
//! a shortcut cannot be shown that does not work, or work without being shown.

use eframe::egui::{Key, Modifiers};

use super::CommandId;

pub struct Binding {
    pub command: CommandId,
    pub mods: Modifiers,
    pub key: Key,
    /// Ignore the key while a text field has the keyboard: the field has its own
    /// undo and must receive Delete and Ctrl+Z itself.
    pub when_not_typing: bool,
}

const CTRL: Modifiers = Modifiers::COMMAND;
const CTRL_SHIFT: Modifiers = Modifiers {
    shift: true,
    ..Modifiers::COMMAND
};

const fn bind(command: CommandId, mods: Modifiers, key: Key) -> Binding {
    Binding {
        command,
        mods,
        key,
        when_not_typing: false,
    }
}

const fn bind_free(command: CommandId, mods: Modifiers, key: Key) -> Binding {
    Binding {
        command,
        mods,
        key,
        when_not_typing: true,
    }
}

/// In priority order. A shifted binding must come before the plain one on the
/// same key, so Ctrl+Shift+Z is redo and not undo.
pub const BINDINGS: &[Binding] = &[
    bind(CommandId::FileNew, CTRL, Key::N),
    bind(CommandId::FileOpen, CTRL, Key::O),
    bind(CommandId::FileSave, CTRL, Key::S),
    bind(CommandId::FileClose, CTRL, Key::W),
    bind_free(CommandId::EditRedo, CTRL_SHIFT, Key::Z),
    bind_free(CommandId::EditRedo, CTRL, Key::Y),
    bind_free(CommandId::EditUndo, CTRL, Key::Z),
    bind(CommandId::EditFind, CTRL, Key::F),
    bind_free(CommandId::ClipboardCut, CTRL, Key::X),
    bind_free(CommandId::ClipboardCopy, CTRL, Key::C),
    bind_free(CommandId::ClipboardPaste, CTRL, Key::V),
    bind(CommandId::InsertMath, CTRL, Key::M),
    bind(CommandId::InsertText, CTRL, Key::T),
    bind(CommandId::InsertArea, CTRL_SHIFT, Key::A),
    bind(CommandId::CalcAll, CTRL, Key::F9),
    bind(CommandId::ViewZoomIn, CTRL, Key::Plus),
    bind(CommandId::ViewZoomIn, CTRL, Key::Equals),
    bind(CommandId::ViewZoomOut, CTRL, Key::Minus),
    bind(CommandId::ViewZoom100, CTRL, Key::Num0),
    bind_free(CommandId::RegionDelete, Modifiers::NONE, Key::Delete),
];

/// The first binding for a command, as tooltip text such as `Ctrl+Shift+A`.
pub fn shortcut_text(command: CommandId) -> Option<String> {
    let b = BINDINGS.iter().find(|b| b.command == command)?;
    let mut text = String::new();
    if b.mods.command || b.mods.ctrl {
        text.push_str("Ctrl+");
    }
    if b.mods.shift {
        text.push_str("Shift+");
    }
    if b.mods.alt {
        text.push_str("Alt+");
    }
    text.push_str(&key_label(b.key));
    Some(text)
}

fn key_label(key: Key) -> String {
    match key {
        Key::Plus => "+".to_owned(),
        Key::Minus => "-".to_owned(),
        Key::Equals => "=".to_owned(),
        Key::Num0 => "0".to_owned(),
        other => format!("{other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_text_is_derived_from_the_binding() {
        assert_eq!(
            shortcut_text(CommandId::EditUndo).as_deref(),
            Some("Ctrl+Z")
        );
        assert_eq!(
            shortcut_text(CommandId::InsertArea).as_deref(),
            Some("Ctrl+Shift+A")
        );
        assert_eq!(
            shortcut_text(CommandId::ViewZoomIn).as_deref(),
            Some("Ctrl++")
        );
        assert_eq!(
            shortcut_text(CommandId::ViewZoom100).as_deref(),
            Some("Ctrl+0")
        );
        assert_eq!(
            shortcut_text(CommandId::RegionDelete).as_deref(),
            Some("Delete")
        );
        assert_eq!(
            shortcut_text(CommandId::CalcAll).as_deref(),
            Some("Ctrl+F9")
        );
        assert_eq!(shortcut_text(CommandId::ViewGrid), None);
    }

    #[test]
    fn no_two_bindings_claim_the_same_keys() {
        for (i, a) in BINDINGS.iter().enumerate() {
            for b in &BINDINGS[i + 1..] {
                let same = a.key == b.key && a.mods == b.mods;
                assert!(!same, "{:?} and {:?} share a key", a.command, b.command);
            }
        }
    }

    #[test]
    fn a_shifted_binding_is_checked_before_the_plain_one_on_the_same_key() {
        let pos = |c: CommandId, shift: bool| {
            BINDINGS
                .iter()
                .position(|b| b.command == c && b.mods.shift == shift && b.key == Key::Z)
        };
        let redo = pos(CommandId::EditRedo, true).expect("Ctrl+Shift+Z redo");
        let undo = pos(CommandId::EditUndo, false).expect("Ctrl+Z undo");
        assert!(redo < undo, "Ctrl+Shift+Z must win over Ctrl+Z");
    }

    #[test]
    fn keys_that_belong_to_text_editing_wait_for_a_free_keyboard() {
        for command in [
            CommandId::EditUndo,
            CommandId::EditRedo,
            CommandId::RegionDelete,
            CommandId::ClipboardCopy,
        ] {
            let b = BINDINGS
                .iter()
                .find(|b| b.command == command)
                .expect("bound");
            assert!(b.when_not_typing, "{command:?}");
        }
    }
}
