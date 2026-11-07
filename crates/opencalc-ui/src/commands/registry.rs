//! Which commands are available right now, and which are switched on.

use std::collections::HashSet;

use super::keys::shortcut_text;
use super::CommandId;
use crate::ribbon::CommandSource;

/// What the app is showing, as far as which commands can run.
pub struct CommandContext {
    /// A worksheet (not the Home page) is showing.
    pub has_sheet: bool,
    /// At least one region is selected.
    pub has_selection: bool,
    /// The selection includes a math region (result formats apply to those).
    pub selects_math: bool,
    /// The selection includes a text region (bold and italic apply to those).
    pub selects_text: bool,
    /// Two or more regions are selected (aligning needs a pair).
    pub selects_several: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    /// A math region is open for editing (symbols can be inserted into it).
    pub editing_math: bool,
}

pub struct Registry {
    enabled: HashSet<CommandId>,
    active: HashSet<CommandId>,
    /// Whether a symbol can be inserted: a math region is being edited.
    symbols: bool,
}

impl Registry {
    /// `active`: toggle commands currently switched on.
    pub fn new(ctx: &CommandContext, active: &[CommandId]) -> Registry {
        Registry {
            enabled: CommandId::ALL
                .iter()
                .copied()
                .filter(|c| c.is_implemented() && available(*c, ctx))
                .collect(),
            active: active.iter().copied().collect(),
            symbols: ctx.editing_math,
        }
    }

    /// Whether a command can run. Parameterised commands (tab select and close)
    /// are always available; unimplemented ones never are.
    pub fn allows(&self, command: CommandId) -> bool {
        match command {
            CommandId::SelectSheet(_)
            | CommandId::CloseSheet(_)
            | CommandId::ZoomTo(_)
            | CommandId::PagePaper(_)
            | CommandId::PageLandscape(_)
            | CommandId::PageMarginMm(_)
            | CommandId::OpenRecent(_)
            | CommandId::GoToFinding(_)
            | CommandId::InsertMatrixSize(..) => true,
            CommandId::InsertSymbol(_) => self.symbols,
            other => self.enabled.contains(&other),
        }
    }
}

/// The context rules: which commands need a worksheet, a selection, or history.
fn available(command: CommandId, ctx: &CommandContext) -> bool {
    use CommandId::*;
    let needs_sheet = matches!(
        command,
        FileClose
            | ClipboardPaste
            | ClipboardCut
            | ClipboardCopy
            | ArrangeSeparate
            | FileExportPdf
            | FileExportPng
            | EditFind
            | EditReplace
            | FindNext
            | FindPrev
            | FindReplaceOne
            | FindReplaceAll
            | UnitsDefine
            | UnitsManage
            | FileSave
            | FileSaveAs
            | CalcAll
            | InsertMath
            | InsertText
            | InsertArea
            | InsertPlot
            | InsertTable
            | InsertPicture
            | ViewZoomIn
            | ViewZoomOut
            | ViewZoomFit
            | ViewZoom100
            | DocPageSetup
            | ViewPageView
            | ViewDraftView
            | EditUndo
            | EditRedo
            | RegionDelete
            | CalcCheck
            | InsertMatrix
            | InsertRange
            | WarnNext
            | WarnPrev
    );
    (ctx.has_sheet || !needs_sheet)
        && (ctx.has_selection || !matches!(command, RegionDelete | ClipboardCut | ClipboardCopy))
        && (ctx.selects_math
            || !matches!(
                command,
                FormatDecimalsMore
                    | FormatDecimalsLess
                    | FormatScientific
                    | FormatEngineering
                    | UnitsDisplayM
                    | UnitsDisplayMm
                    | UnitsDisplayIn
                    | UnitsDisplayN
                    | UnitsDisplayKn
                    | UnitsDisplayReset
            ))
        && (ctx.selects_several || !matches!(command, ArrangeAlignH | ArrangeAlignV))
        && (ctx.selects_text || !matches!(command, FormatBold | FormatItalic))
        && (ctx.can_undo || command != EditUndo)
        && (ctx.can_redo || command != EditRedo)
        && (ctx.editing_math || command != InsertGreek)
}

impl CommandSource for Registry {
    fn is_enabled(&self, command: CommandId) -> bool {
        self.allows(command)
    }

    fn shortcut_text(&self, command: CommandId) -> Option<String> {
        shortcut_text(command)
    }

    fn is_active(&self, command: CommandId) -> bool {
        self.active.contains(&command)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(has_sheet: bool, has_selection: bool, can_undo: bool, can_redo: bool) -> CommandContext {
        CommandContext {
            has_sheet,
            has_selection,
            selects_math: has_selection,
            selects_text: has_selection,
            selects_several: has_selection,
            can_undo,
            can_redo,
            editing_math: has_sheet,
        }
    }

    #[test]
    fn unimplemented_commands_are_never_enabled() {
        let r = Registry::new(&ctx(true, true, true, true), &[]);
        for c in CommandId::ALL {
            assert_eq!(r.allows(*c), c.is_implemented(), "{}", c.name());
        }
        assert!(r.allows(CommandId::InsertSymbol('x')));
    }

    #[test]
    fn symbols_can_only_be_inserted_while_a_math_region_is_being_edited() {
        let mut c = ctx(true, false, false, false);
        c.editing_math = false;
        let r = Registry::new(&c, &[]);
        assert!(!r.allows(CommandId::InsertSymbol('x')));
        assert!(!r.allows(CommandId::InsertGreek));
        c.editing_math = true;
        let r = Registry::new(&c, &[]);
        assert!(r.allows(CommandId::InsertSymbol('x')) && r.allows(CommandId::InsertGreek));
    }

    #[test]
    fn sheet_commands_are_disabled_on_the_home_page() {
        let r = Registry::new(&ctx(false, false, false, false), &[]);
        for c in [
            CommandId::InsertMath,
            CommandId::ViewZoomIn,
            CommandId::CalcAll,
            CommandId::FileClose,
        ] {
            assert!(!r.allows(c), "{}", c.name());
        }
        assert!(r.allows(CommandId::FileNew));
        assert!(r.allows(CommandId::HelpAbout));
    }

    #[test]
    fn delete_needs_a_selection_and_undo_redo_need_history() {
        let none = Registry::new(&ctx(true, false, false, false), &[]);
        assert!(!none.allows(CommandId::RegionDelete));
        assert!(!none.allows(CommandId::EditUndo) && !none.allows(CommandId::EditRedo));
        let some = Registry::new(&ctx(true, true, true, false), &[]);
        assert!(some.allows(CommandId::RegionDelete) && some.allows(CommandId::EditUndo));
        assert!(!some.allows(CommandId::EditRedo));
        let home = Registry::new(&ctx(false, true, true, true), &[]);
        assert!(!home.allows(CommandId::EditUndo), "never on the Home page");
    }

    #[test]
    fn tab_commands_are_always_available() {
        let r = Registry::new(&ctx(false, false, false, false), &[]);
        assert!(r.allows(CommandId::SelectSheet(7)) && r.allows(CommandId::CloseSheet(0)));
    }

    #[test]
    fn active_toggles_and_shortcut_text_come_through_the_command_source() {
        let r = Registry::new(&ctx(true, false, false, false), &[CommandId::ViewGrid]);
        assert!(r.is_active(CommandId::ViewGrid) && !r.is_active(CommandId::ViewSnap));
        assert_eq!(
            r.shortcut_text(CommandId::EditUndo).as_deref(),
            Some("Ctrl+Z")
        );
    }
}
