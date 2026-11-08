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
            /// Go to the n-th finding of the Check window.
            GoToFinding(u16),
            /// Insert a matrix of this many rows and columns.
            InsertMatrixSize(u8, u8),
            /// Go to the n-th labelled region of the Labels window.
            GoToLabel(u16),
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
                    CommandId::GoToFinding(_) => "check.go_to",
                    CommandId::InsertMatrixSize(..) => "insert.matrix_size",
                    CommandId::GoToLabel(_) => "labels.go_to",
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
                    CommandId::GoToFinding(_) => "Go to problem",
                    CommandId::InsertMatrixSize(..) => "Insert matrix",
                    CommandId::GoToLabel(_) => "Go to labelled region",
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
                    CommandId::GoToFinding(_) => icons::WARNING,
                    CommandId::InsertMatrixSize(..) => icons::TABLE,
                    CommandId::GoToLabel(_) => icons::TAG,
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
    FileSaveAs: "file.save_as", "Save As", icons::SAVE;
    FileExportPdf: "file.export_pdf", "Export as PDF", icons::SAVE;
    FileExportPng: "file.export_png", "Export as PNG", icons::PICTURE;
    // Answers to the dialogs that ask about unsaved changes or report a problem.
    DialogSave: "dialog.save", "Save", icons::SAVE;
    DialogDiscard: "dialog.discard", "Don't save", icons::DELETE;
    DialogCancel: "dialog.cancel", "Cancel", icons::CLOSE;
    DialogOk: "dialog.ok", "OK", icons::CLOSE;
    DialogRestore: "dialog.restore", "Restore", icons::REFRESH;
    DialogDropRecovery: "dialog.drop_recovery", "Discard", icons::DELETE;
    FileClose: "file.close", "Close worksheet", icons::CLOSE;
    FileExample: "file.example", "Open example", icons::BOOK;
    AppQuit: "app.quit", "Quit", icons::CLOSE;
    AppOptions: "app.options", "Options", icons::PROPERTIES;
    AppOptionsClose: "app.options_close", "Close Options", icons::CLOSE;
    FileTypeRegister: "app.file_type_register", "Open .ocx files with OpenCalc", icons::FILE_NEW;
    FileTypeRemove: "app.file_type_remove", "Stop opening .ocx files with OpenCalc", icons::CLOSE;
    ThemeNeutral: "theme.neutral", "Neutral", icons::THEME;
    ThemeSlate: "theme.slate", "Slate", icons::THEME;
    TabHome: "tab.home", "Home", icons::HOME;
    // Clipboard and editing.
    ClipboardPaste: "clipboard.paste", "Paste", icons::PASTE;
    ClipboardCut: "clipboard.cut", "Cut", icons::CUT;
    ClipboardCopy: "clipboard.copy", "Copy", icons::COPY;
    EditUndo: "edit.undo", "Undo", icons::UNDO;
    EditRedo: "edit.redo", "Redo", icons::REDO;
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
    InsertPicture: "insert.picture", "Picture", icons::PICTURE;
    InsertGreek: "insert.greek", "Greek letters", icons::GREEK;
    InsertMatrix: "insert.matrix", "Matrix", icons::TABLE;
    InsertRange: "insert.range", "Range", icons::LIST;
    MatrixClose: "insert.matrix_close", "Close Matrix", icons::CLOSE;
    // Format.
    FormatBold: "format.bold", "Bold", icons::BOLD;
    FormatItalic: "format.italic", "Italic", icons::ITALIC;
    FormatDecimalsMore: "format.decimals_more", "More decimals", icons::PLUS;
    FormatDecimalsLess: "format.decimals_less", "Fewer decimals", icons::MINUS;
    FormatScientific: "format.scientific", "Scientific", icons::SCIENTIFIC;
    FormatEngineering: "format.engineering", "Engineering", icons::ENGINEERING;
    ArrangeAlignH: "arrange.align_h", "Align horizontally", icons::ALIGN_H;
    ArrangeAlignV: "arrange.align_v", "Align vertically", icons::ALIGN_V;
    ArrangeSeparate: "arrange.separate", "Separate regions", icons::SEPARATE;
    // Units.
    UnitsSi: "units.si", "SI", icons::RULER;
    UnitsUscs: "units.uscs", "USCS", icons::RULER;
    UnitsCgs: "units.cgs", "CGS", icons::RULER;
    UnitsDisplayM: "units.display_m", "Metres", icons::RULER;
    UnitsDisplayMm: "units.display_mm", "Millimetres", icons::RULER;
    UnitsDisplayIn: "units.display_in", "Inches", icons::RULER;
    UnitsDisplayN: "units.display_n", "Force in newtons", icons::CONVERT;
    UnitsDisplayKn: "units.display_kn", "Force in kilonewtons", icons::CONVERT;
    UnitsDisplayReset: "units.display_reset", "Reset display", icons::RESET;
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

/// Names of the symbols the Greek flyout offers.
const SYMBOLS: [(char, &str); 16] = [
    ('\u{03b1}', "alpha"),
    ('\u{03b2}', "beta"),
    ('\u{03b3}', "gamma"),
    ('\u{03b4}', "delta"),
    ('\u{03b5}', "epsilon"),
    ('\u{03b8}', "theta"),
    ('\u{03bb}', "lambda"),
    ('\u{03bc}', "mu"),
    ('\u{03c0}', "pi"),
    ('\u{03c1}', "rho"),
    ('\u{03c3}', "sigma"),
    ('\u{03c4}', "tau"),
    ('\u{03c6}', "phi"),
    ('\u{03c9}', "omega"),
    ('\u{0394}', "Delta"),
    ('\u{03a9}', "Omega"),
];

/// The Greek letter for a name such as `alpha` or `Delta` (case matters).
pub fn greek_by_name(name: &str) -> Option<char> {
    SYMBOLS.iter().find(|(_, n)| *n == name).map(|(c, _)| *c)
}

/// The characters of the Greek flyout, in display order.
pub fn greek_symbols() -> impl Iterator<Item = char> {
    SYMBOLS.iter().map(|(c, _)| *c)
}

fn symbol_name(c: char) -> &'static str {
    SYMBOLS
        .iter()
        .find(|(s, _)| *s == c)
        .map_or("Symbol", |(_, n)| n)
}

impl CommandId {
    /// Whether the app can run this command today. Exhaustive on purpose: a new
    /// command must be classified here, and the ribbon shows the unimplemented
    /// ones dimmed rather than letting them do nothing.
    pub fn is_implemented(self) -> bool {
        use CommandId::*;
        match self {
            ViewLabels | LabelsClose | GoToLabel(_) | FileTypeRegister | FileTypeRemove
            | HelpShortcuts | HelpShortcutsClose | RegionTag | LabelClose | RegionLock
            | ArrangeAlignH | ArrangeAlignV | FormatBold | FormatItalic | UnitsDisplayM
            | UnitsDisplayMm | UnitsDisplayIn | UnitsDisplayN | UnitsDisplayKn
            | UnitsDisplayReset | FormatDecimalsMore | FormatDecimalsLess | FormatScientific
            | FormatEngineering | ClipboardPaste | ClipboardCut | ClipboardCopy
            | ArrangeSeparate | FileExportPdf | FileExportPng | EditFind | EditReplace
            | FindNext | FindPrev | FindReplaceOne | FindReplaceAll | FindClose | UnitsDefine
            | UnitsManage | UnitsClose | FileNew | FileOpen | FileSave | FileSaveAs
            | DialogSave | DialogDiscard | DialogCancel | DialogOk | DialogRestore
            | DialogDropRecovery | OpenRecent(_) | GoToFinding(_) | InsertMatrixSize(..)
            | MatrixClose | InsertMatrix | InsertRange | CheckClose | CalcCheck | WarnNext
            | WarnPrev | FileClose | FileExample | AppQuit | TabHome | EditUndo | EditRedo
            | RegionDelete | InsertMath | InsertText | InsertArea | InsertPlot | InsertTable
            | InsertPicture | CalcAll | ViewZoomIn | ViewZoomOut | ViewZoomFit | ViewZoom100
            | ViewGrid | ViewSnap | CalcAuto | ViewFunctions | ViewResetLayout | AppOptions
            | AppOptionsClose | ThemeNeutral | ThemeSlate | UnitsSi | UnitsUscs | UnitsCgs
            | HelpAbout | HelpAboutClose | SelectSheet(_) | CloseSheet(_) | ZoomTo(_)
            | PagePaper(_) | PageLandscape(_) | PageMarginMm(_) | DocPageSetup
            | DocPageSetupClose | ViewPageView | ViewDraftView | InsertGreek | InsertSymbol(_) => {
                true
            }
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
