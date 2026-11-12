//! The command table (SPEC section 4.7, rule R5): every user action, defined once.
//!
//! One macro generates the enum and its name, label and icon together, so a
//! command cannot exist without all three. Menus, the ribbon, shortcuts and
//! tooltips read from here. Which commands the app can actually run is
//! [`CommandId::is_implemented`], an exhaustive `match`: adding a variant without
//! deciding that is a compile error, and the dispatch test checks it agrees with
//! what `dispatch` really handles.

use crate::icons;
use opencalc_core::document::PaperSize;

macro_rules! commands {
    ($($variant:ident: $name:literal, $label:literal, $icon:expr;)*) => {
        /// A user action.
        ///
        /// The parameterised variants are not in the table: a symbol to insert, or
        /// a worksheet tab to select or close.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum CommandId {
            $($variant,)*
            InsertSymbol(char),
            SelectSheet(usize),
            CloseSheet(usize),
            ZoomTo(u16),
            PagePaper(PaperSize),
            PageLandscape(bool),
            PageMarginMm(u8),
            /// Open the n-th recently used file.
            OpenRecent(u8),
            /// Pin the n-th recent file to the top of the list, or unpin it.
            PinRecent(u8),
            /// Take the n-th recent file off the list (the file is not touched).
            ForgetRecent(u8),
            /// Go to the n-th finding of the Check window.
            GoToFinding(u16),
            /// Insert a matrix of this many rows and columns.
            InsertMatrixSize(u8, u8),
            /// Go to the n-th labelled region of the Labels window.
            GoToLabel(u16),
            /// Show the selected results in this unit (source text, such as `N/mm^2`).
            UnitsShowIn(&'static str),
        }

        impl CommandId {
            /// Every command that has a table entry (all but the parameterised ones).
            pub const ALL: &'static [CommandId] = &[$(CommandId::$variant,)*];

            /// The stable name, such as `edit.undo`, for a keymap file (Milestone 3
            /// preferences). Until that exists only the tests use it, so it is
            /// compiled for them alone rather than left as dead code.
            #[cfg(test)]
            pub fn name(self) -> &'static str {
                match self {
                    $(CommandId::$variant => $name,)*
                    CommandId::InsertSymbol(_) => "insert.symbol",
                    CommandId::SelectSheet(_) => "tab.select",
                    CommandId::CloseSheet(_) => "tab.close",
                    CommandId::ZoomTo(_) => "view.zoom_to",
                    CommandId::PagePaper(_) => "page.paper",
                    CommandId::PageLandscape(_) => "page.orientation",
                    CommandId::PageMarginMm(_) => "page.margin",
                    CommandId::OpenRecent(_) => "file.open_recent",
                    CommandId::PinRecent(_) => "file.pin_recent",
                    CommandId::ForgetRecent(_) => "file.forget_recent",
                    CommandId::GoToFinding(_) => "check.go_to",
                    CommandId::InsertMatrixSize(..) => "insert.matrix_size",
                    CommandId::GoToLabel(_) => "labels.go_to",
                    CommandId::UnitsShowIn(_) => "units.show_in",
                }
            }

            /// The text shown on a button or in a tooltip.
            pub fn label(self) -> &'static str {
                match self {
                    $(CommandId::$variant => $label,)*
                    CommandId::InsertSymbol(c) => symbol_name(c),
                    CommandId::SelectSheet(_) => "Select worksheet",
                    CommandId::CloseSheet(_) => "Close worksheet",
                    CommandId::ZoomTo(_) => "Zoom",
                    CommandId::PagePaper(p) => p.label(),
                    CommandId::PageLandscape(true) => "Landscape",
                    CommandId::PageLandscape(false) => "Portrait",
                    CommandId::PageMarginMm(_) => "Margin",
                    CommandId::OpenRecent(_) => "Open recent file",
                    CommandId::PinRecent(_) => "Pin or unpin recent file",
                    CommandId::ForgetRecent(_) => "Remove from recent files",
                    CommandId::GoToFinding(_) => "Go to problem",
                    CommandId::InsertMatrixSize(..) => "Insert matrix",
                    CommandId::GoToLabel(_) => "Go to labelled region",
                    CommandId::UnitsShowIn(u) => crate::format::unit_label(u),
                }
            }

            pub fn icon(self) -> char {
                match self {
                    $(CommandId::$variant => $icon,)*
                    CommandId::InsertSymbol(c) => c,
                    CommandId::SelectSheet(_) => icons::HOME,
                    CommandId::CloseSheet(_) => icons::CLOSE,
                    CommandId::ZoomTo(_) => icons::ZOOM_100,
                    CommandId::PagePaper(_)
                    | CommandId::PageLandscape(_)
                    | CommandId::PageMarginMm(_) => icons::FIT,
                    CommandId::OpenRecent(_) => icons::FOLDER_OPEN,
                    CommandId::PinRecent(_) => icons::PIN,
                    CommandId::ForgetRecent(_) => icons::DELETE,
                    CommandId::GoToFinding(_) => icons::WARNING,
                    CommandId::InsertMatrixSize(..) => icons::TABLE,
                    CommandId::GoToLabel(_) => icons::TAG,
                    CommandId::UnitsShowIn(_) => icons::RULER,
                }
            }
        }
    };
}

commands! {
    // Files and application.
    FileNew: "file.new", "New worksheet", icons::FILE_NEW;
    FileOpen: "file.open", "Open", icons::FOLDER_OPEN;
    FileSave: "file.save", "Save", icons::SAVE;
    FileSaveAs: "file.save_as", "Save As", icons::SAVE_AS;
    FileExportPdf: "file.export_pdf", "Export as PDF", icons::FILE_OUTPUT;
    FileExportPng: "file.export_png", "Export as PNG", icons::PICTURE;
    // Answers to the dialogs that ask about unsaved changes or report a problem.
    DialogSave: "dialog.save", "Save", icons::SAVE;
    DialogDiscard: "dialog.discard", "Don't save", icons::DELETE;
    DialogCancel: "dialog.cancel", "Cancel", icons::CLOSE;
    DialogOk: "dialog.ok", "OK", icons::CLOSE;
    DialogRestore: "dialog.restore", "Restore", icons::REFRESH;
    DialogDropRecovery: "dialog.drop_recovery", "Discard", icons::DELETE;
    DialogExportAnyway: "dialog.export_anyway", "Export anyway", icons::FILE_OUTPUT;
    DialogRecalculate: "dialog.recalculate", "Recalculate", icons::REFRESH;
    FileClose: "file.close", "Close worksheet", icons::FILE_CLOSE;
    FileNewFromTemplate: "file.new_from_template", "New from template...", icons::TEMPLATE;
    FileSaveAsTemplate: "file.save_as_template", "Save as template...", icons::TEMPLATE;
    FileExample: "file.example", "Open example", icons::BOOK;
    AppQuit: "app.quit", "Quit", icons::QUIT;
    AppOptions: "app.options", "Options", icons::SETTINGS;
    AppMenu: "app.menu", "File menu", icons::FILE_TEXT;
    AppMenuClose: "app.menu_close", "Close File menu", icons::CLOSE;
    AppOptionsClose: "app.options_close", "Close Options", icons::CLOSE;
    AppOptionsDefaults: "app.options_defaults", "Restore defaults", icons::REFRESH;
    FileClearRecent: "file.clear_recent", "Clear recent list", icons::DELETE;
    FileTypeRegister: "app.file_type_register", "Open .ocalc files with OpenCalc", icons::FILE_NEW;
    FileTypeRemove: "app.file_type_remove", "Stop opening .ocalc files with OpenCalc", icons::CLOSE;
    ThemeNeutral: "theme.neutral", "Neutral", icons::THEME;
    ThemeSlate: "theme.slate", "Slate", icons::THEME;
    TabHome: "tab.home", "Home", icons::HOME;
    // Clipboard and editing.
    ClipboardPaste: "clipboard.paste", "Paste", icons::PASTE;
    ClipboardCut: "clipboard.cut", "Cut", icons::CUT;
    ClipboardCopy: "clipboard.copy", "Copy", icons::COPY;
    EditUndo: "edit.undo", "Undo", icons::UNDO;
    EditRedo: "edit.redo", "Redo", icons::REDO;
    EditSelectAll: "edit.select_all", "Select all", icons::LIST;
    EditDuplicate: "edit.duplicate", "Duplicate", icons::COPY;
    EditFind: "edit.find", "Find", icons::FIND;
    EditReplace: "edit.replace", "Replace", icons::REPLACE;
    FindNext: "find.next", "Find next", icons::FIND;
    FindPrev: "find.prev", "Find previous", icons::FIND;
    FindReplaceOne: "find.replace_one", "Replace", icons::REPLACE;
    FindReplaceAll: "find.replace_all", "Replace all", icons::REPLACE;
    FindClose: "find.close", "Close Find", icons::CLOSE;
    // Regions.
    RegionDelete: "region.delete", "Delete", icons::DELETE;
    RegionLock: "region.lock", "Lock", icons::LOCK;
    RegionTag: "region.tag", "Tag", icons::TAG;
    LabelClose: "region.label_close", "Close Label", icons::CLOSE;
    InsertMath: "insert.math", "Math region", icons::MATH;
    InsertText: "insert.text", "Text region", icons::TEXT;
    InsertArea: "insert.area", "Area", icons::AREA;
    InsertPlot: "insert.plot", "Plot", icons::PLOT;
    InsertTable: "insert.table", "Table", icons::TABLE;
    PictureChoose: "picture.choose", "Choose image", icons::PICTURE;
    TableImportCsv: "table.import_csv", "Import CSV", icons::TABLE;
    TableExportCsv: "table.export_csv", "Export CSV", icons::TABLE;
    InsertPicture: "insert.picture", "Picture", icons::PICTURE;
    InsertRectangle: "insert.rectangle", "Rectangle", icons::SQUARE;
    InsertEllipse: "insert.ellipse", "Ellipse", icons::CIRCLE;
    InsertLine: "insert.line", "Line", icons::LINE;
    InsertTriangle: "insert.triangle", "Triangle", icons::TRIANGLE;
    InsertDiamond: "insert.diamond", "Diamond", icons::DIAMOND;
    InsertPolygon: "insert.polygon", "Polygon", icons::POLYGON;
    InsertGreek: "insert.greek", "Greek letters", icons::GREEK;
    InsertMatrix: "insert.matrix", "Matrix", icons::TABLE;
    InsertRange: "insert.range", "Range", icons::LIST;
    InsertSolveBlock: "insert.solve_block", "Solve block", icons::ENGINEERING;
    InsertProgram: "insert.program", "Program", icons::PROPERTIES;
    MatrixClose: "insert.matrix_close", "Close Matrix", icons::CLOSE;
    // Format.
    FormatBorder: "format.border", "Border", icons::AREA;
    FormatBold: "format.bold", "Bold", icons::BOLD;
    FormatUnderline: "format.underline", "Underline", icons::UNDERLINE;
    FormatItalic: "format.italic", "Italic", icons::ITALIC;
    FormatDecimalsMore: "format.decimals_more", "More decimals", icons::PLUS;
    FormatDecimalsLess: "format.decimals_less", "Fewer decimals", icons::MINUS;
    FormatScientific: "format.scientific", "Scientific", icons::SCIENTIFIC;
    FormatEngineering: "format.engineering", "Engineering", icons::ENGINEERING;
    ArrangeAlignH: "arrange.align_h", "Align horizontally", icons::ALIGN_H;
    ArrangeAlignV: "arrange.align_v", "Align vertically", icons::ALIGN_V;
    ArrangeSeparate: "arrange.separate", "Separate regions", icons::SEPARATE;
    ArrangeToFront: "arrange.to_front", "Bring to front", icons::BRING_TO_FRONT;
    ArrangeForward: "arrange.forward", "Bring forward", icons::BRING_FORWARD;
    ArrangeBackward: "arrange.backward", "Send backward", icons::SEND_BACKWARD;
    ArrangeToBack: "arrange.to_back", "Send to back", icons::SEND_TO_BACK;
    // Units.
    UnitsSi: "units.si", "SI", icons::RULER;
    UnitsUscs: "units.uscs", "USCS", icons::RULER;
    UnitsCgs: "units.cgs", "CGS", icons::RULER;
    UnitsShowInMenu: "units.show_in_menu", "Result unit", icons::CONVERT;
    UnitsDisplayReset: "units.display_reset", "Automatic", icons::RESET;
    UnitsDefine: "units.define", "Define unit", icons::PLUS;
    UnitsManage: "units.manage", "Manage units", icons::LIST;
    UnitsClose: "units.close", "Close Units", icons::CLOSE;
    // Calculate.
    CalcAll: "calc.all", "Recalculate all", icons::REFRESH;
    CalcAuto: "calc.auto", "Automatic calculation", icons::ZAP;
    CalcCheck: "calc.check", "Run checks", icons::CHECK;
    WarnNext: "warn.next", "Next", icons::ARROW_DOWN;
    WarnPrev: "warn.prev", "Previous", icons::ARROW_UP;
    CheckClose: "check.close", "Close Check", icons::CLOSE;
    // View.
    ViewZoomIn: "view.zoom_in", "Zoom in", icons::ZOOM_IN;
    ViewZoomOut: "view.zoom_out", "Zoom out", icons::ZOOM_OUT;
    ViewZoomFit: "view.zoom_fit", "Fit", icons::FIT;
    ViewZoom100: "view.zoom_100", "100%", icons::ZOOM_100;
    ViewGrid: "view.grid", "Grid", icons::GRID;
    ViewSnap: "view.snap", "Snap", icons::SNAP;
    DocPageSetup: "doc.page_setup", "Page setup", icons::FIT;
    DocPageSetupClose: "doc.page_setup_close", "Close Page setup", icons::CLOSE;
    ViewPageView: "view.page_view", "Page View", icons::FIT;
    ViewDraftView: "view.draft_view", "Draft View", icons::LIST;
    ViewFunctions: "view.functions", "Functions", icons::NAMES;
    ViewLabels: "view.labels", "Labels", icons::TAG;
    LabelsClose: "view.labels_close", "Close Labels", icons::CLOSE;
    ViewResetLayout: "view.reset_layout", "Reset layout", icons::RESET;
    // Help.
    HelpShortcuts: "help.shortcuts", "Shortcuts", icons::KEYBOARD;
    HelpShortcutsClose: "help.shortcuts_close", "Close Shortcuts", icons::CLOSE;
    HelpGuide: "help.guide", "User guide", icons::BOOK;
    HelpAbout: "help.about", "About", icons::INFO;
    HelpAboutClose: "help.about_close", "Close About", icons::CLOSE;
}

use super::symbols::symbol_name;

impl CommandId {
    /// Whether the app can run this command today. Exhaustive on purpose: a new
    /// command must be classified here, and the ribbon shows the unimplemented
    /// ones dimmed rather than letting them do nothing.
    pub fn is_implemented(self) -> bool {
        use CommandId::*;
        match self {
            ViewLabels | LabelsClose | GoToLabel(_) | FileTypeRegister | FileTypeRemove
            | HelpShortcuts | HelpShortcutsClose | RegionTag | LabelClose | RegionLock
            | ArrangeAlignH | ArrangeAlignV | FormatBold | FormatItalic | FormatUnderline
            | FormatBorder | UnitsShowInMenu | UnitsDisplayReset | FormatDecimalsMore
            | FormatDecimalsLess | FormatScientific | FormatEngineering | ClipboardPaste
            | ClipboardCut | ClipboardCopy | ArrangeSeparate | FileExportPdf | FileExportPng
            | EditFind | EditReplace | FindNext | FindPrev | FindReplaceOne | FindReplaceAll
            | FindClose | UnitsDefine | UnitsManage | UnitsClose | FileNew
            | FileNewFromTemplate | FileSaveAsTemplate | FileOpen | FileSave | FileSaveAs
            | DialogSave | DialogDiscard | DialogCancel | DialogOk | DialogRestore
            | DialogDropRecovery | DialogExportAnyway | DialogRecalculate | OpenRecent(_)
            | PinRecent(_) | ForgetRecent(_) | UnitsShowIn(_) | GoToFinding(_)
            | InsertMatrixSize(..) | MatrixClose | InsertMatrix | InsertRange
            | InsertSolveBlock | InsertProgram | CheckClose | CalcCheck | WarnNext | WarnPrev
            | FileClose | FileExample | AppQuit | AppMenu | AppMenuClose | TabHome | EditUndo
            | EditRedo | EditSelectAll | EditDuplicate | RegionDelete | InsertMath | InsertText
            | InsertArea | InsertPlot | InsertTable | PictureChoose | TableImportCsv
            | TableExportCsv | InsertPicture | CalcAll | ViewZoomIn | ViewZoomOut | ViewZoomFit
            | ViewZoom100 | ViewGrid | ViewSnap | CalcAuto | ViewFunctions | ViewResetLayout
            | AppOptions | AppOptionsClose | AppOptionsDefaults | FileClearRecent
            | ThemeNeutral | ThemeSlate | UnitsSi | UnitsUscs | UnitsCgs | HelpAbout
            | HelpAboutClose | SelectSheet(_) | CloseSheet(_) | ZoomTo(_) | PagePaper(_)
            | PageLandscape(_) | PageMarginMm(_) | DocPageSetup | DocPageSetupClose
            | ViewPageView | ViewDraftView | InsertGreek | InsertSymbol(_) | InsertRectangle
            | InsertEllipse | InsertLine | InsertTriangle | InsertDiamond | InsertPolygon
            | ArrangeToFront | ArrangeForward | ArrangeBackward | ArrangeToBack => true,
            HelpGuide => false,
        }
    }

    /// Look a command up by its stable name. Parameterised commands have none.
    #[cfg(test)]
    pub fn from_name(name: &str) -> Option<CommandId> {
        CommandId::ALL.iter().copied().find(|c| c.name() == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::greek_symbols;
    use std::collections::HashSet;

    #[test]
    fn names_are_unique_and_look_up_both_ways() {
        let mut seen = HashSet::new();
        for c in CommandId::ALL {
            assert!(seen.insert(c.name()), "duplicate name {}", c.name());
            assert_eq!(CommandId::from_name(c.name()), Some(*c));
        }
        assert_eq!(CommandId::from_name("no.such.command"), None);
        assert!(CommandId::ALL.len() > 60);
    }

    #[test]
    fn names_have_the_area_dot_action_shape() {
        for c in CommandId::ALL {
            let n = c.name();
            let (area, action) = n.split_once('.').expect(n);
            assert!(!area.is_empty() && !action.is_empty(), "{n}");
            assert!(
                n.chars().all(|ch| ch.is_ascii_lowercase()
                    || ch.is_ascii_digit()
                    || ch == '.'
                    || ch == '_'),
                "{n}"
            );
        }
    }

    #[test]
    fn every_command_has_a_label_and_an_icon() {
        for c in CommandId::ALL {
            assert!(!c.label().is_empty(), "{}", c.name());
            assert!(
                ('\u{e000}'..='\u{f8ff}').contains(&c.icon()),
                "{} icon",
                c.name()
            );
        }
    }

    #[test]
    fn the_greek_flyout_symbols_have_names_and_are_their_own_icons() {
        let all: Vec<char> = greek_symbols().collect();
        assert_eq!(all.len(), 16);
        for c in all {
            let id = CommandId::InsertSymbol(c);
            assert_eq!(id.icon(), c);
            assert_ne!(id.label(), "Symbol", "{c} has no name");
        }
        assert_eq!(CommandId::InsertSymbol('\u{3c3}').label(), "sigma");
        assert_eq!(CommandId::InsertSymbol('x').label(), "Symbol");
    }

    #[test]
    fn some_commands_are_implemented_and_some_deliberately_are_not() {
        assert!(CommandId::EditUndo.is_implemented());
        assert!(CommandId::SelectSheet(3).is_implemented());
        assert!(!CommandId::HelpGuide.is_implemented());
        let implemented = CommandId::ALL.iter().filter(|c| c.is_implemented()).count();
        assert!(implemented > 20 && implemented < CommandId::ALL.len());
    }
}
