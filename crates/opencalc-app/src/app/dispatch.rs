//! Turning a command into a change of application state (rules R3, R4, R5).
//!
//! `dispatch` is the only code that changes [`AppState`] in response to a user
//! action. It never touches egui or the worker: anything that needs them comes
//! back as an [`Effect`] for `App` to carry out.

use opencalc_core::document::{RegionKind, ShapeKind, ShapeSpec};
use opencalc_core::edit::Edit;
use opencalc_core::registry::UnitSystem;

use super::actions::{arm_pen, separate_regions, set_page, set_unit_system};
use super::filing::{dispatch_file, Then};
use super::state::AppState;
use crate::canvas::CanvasCmd;
use crate::commands::CommandId;
use crate::sheet::OpenSheet;
use std::path::PathBuf;

/// Something a command needs done that state alone cannot do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    /// Send the active sheet to the evaluation worker.
    Recalculate,
    /// Calculate now, even in manual mode (the Calculate command).
    CalculateNow,
    /// Re-apply the theme to egui.
    ApplyTheme,
    /// Carry out the settings egui and the auto-save timer hold: the interface
    /// size and the recovery interval.
    ApplySettings,
    Quit,
    /// Size the dock panels from the current window width.
    ResetDockWidths,
    /// Read the system's `.ocalc` file type again (the Options window shows it).
    RefreshFileType,
    /// Register this program for `.ocalc` files, or remove that.
    SetFileType(bool),
    /// Ask which file to open, then open it.
    ChooseOpen,
    /// Ask for a template (or worksheet) and start a new worksheet from it.
    NewFromTemplate,
    /// Ask where to write the active worksheet as a template.
    SaveAsTemplate,
    /// Export the active worksheet (asks first if its results are out of date).
    Export(crate::export::Kind),
    /// Export it without asking that.
    ExportNow(crate::export::Kind),
    /// Fill the selected table from a CSV file, or write it to one.
    ImportCsv,
    /// Choose an image file for the selected picture.
    ChoosePicture,
    ExportCsv,
    /// Clipboard work, which needs the system clipboard.
    Copy,
    Cut,
    Paste,
    Open(PathBuf),
    /// Put back the changes in a recovery file.
    Restore(super::filing::Recovery),
    DeleteFile(PathBuf),
    /// Save worksheet `sheet`. With no path yet, or `ask`, the user is asked where.
    /// Once saved, `then` carries on with what was waiting (closing, quitting).
    Save {
        sheet: usize,
        then: Option<Then>,
        ask: bool,
    },
}

/// Run a command. `None` means it is not implemented, which the registry already
/// reflects by keeping it disabled.
pub fn dispatch(state: &mut AppState, command: CommandId) -> Option<Vec<Effect>> {
    use CommandId::*;
    // Running anything closes the File menu, as picking from it would; the menu
    // button toggles it.
    match command {
        AppMenu => {
            state.app_menu_open = !state.app_menu_open;
            return Some(Vec::new());
        }
        _ => state.app_menu_open = false,
    }
    if command == AppMenuClose {
        return Some(Vec::new());
    }
    if let Some(fx) = dispatch_file(state, command) {
        return Some(fx);
    }
    if let Some(fx) = super::finding::dispatch_find(state, command) {
        return Some(fx);
    }
    if let Some(fx) = super::formatting::dispatch_format(state, command) {
        return Some(fx);
    }
    if let Some(fx) = super::arranging::dispatch_arrange(state, command) {
        return Some(fx);
    }
    if let Some(fx) = super::checking::dispatch_check(state, command) {
        return Some(fx);
    }
    if let Some(fx) = super::options::dispatch_options(state, command) {
        return Some(fx);
    }
    if let Some(fx) = super::inserting::dispatch_insert(state, command) {
        return Some(fx);
    }
    if let Some(fx) = super::view::dispatch_view(state, command) {
        return Some(fx);
    }
    if let Some(fx) = super::labels::dispatch_labels(state, command) {
        return Some(fx);
    }
    let mut fx = Vec::new();
    match command {
        FileNew => {
            state.untitled += 1;
            let title = format!("Untitled {}", state.untitled);
            let sheet = state.blank_sheet(&title);
            add_sheet(state, sheet);
            fx.push(Effect::Recalculate);
        }
        FileNewFromTemplate => fx.push(Effect::NewFromTemplate),
        FileSaveAsTemplate => fx.push(Effect::SaveAsTemplate),
        FileExample => {
            add_sheet(state, OpenSheet::sample());
            fx.push(Effect::Recalculate);
        }
        SelectSheet(i) => {
            if i < state.sheets.len() {
                state.active = Some(i);
                state.output = Default::default();
                fx.push(Effect::Recalculate);
            }
        }
        TabHome => {
            state.active = None;
            fx.push(Effect::Recalculate);
        }

        InsertMath => insert(state, &mut fx, RegionKind::math("")),
        InsertText => insert(
            state,
            &mut fx,
            RegionKind::Text {
                text: String::new(),
            },
        ),
        InsertArea => insert(
            state,
            &mut fx,
            RegionKind::Area {
                title: String::new(),
            },
        ),
        InsertPlot => insert(state, &mut fx, RegionKind::plot()),
        InsertTable => insert(state, &mut fx, RegionKind::table()),
        InsertPicture => insert(state, &mut fx, RegionKind::picture()),
        InsertRectangle => arm_pen(state, ShapeKind::Rectangle),
        InsertEllipse => arm_pen(state, ShapeKind::Ellipse),
        InsertLine => arm_pen(state, ShapeKind::Line),
        InsertTriangle => arm_pen(state, ShapeKind::Triangle),
        InsertDiamond => arm_pen(state, ShapeKind::Diamond),
        InsertPolygon => arm_pen(state, ShapeKind::Polygon),
        RegionDelete => with_eval_check(state, &mut fx, |s| {
            s.delete_selection();
        }),
        EditSelectAll => select_all(state),
        EditUndo => with_eval_check(state, &mut fx, |s| {
            s.undo();
        }),
        EditRedo => with_eval_check(state, &mut fx, |s| {
            s.redo();
        }),
        CalcAll => fx.push(Effect::CalculateNow),
        ClipboardCopy => fx.push(Effect::Copy),
        ClipboardCut => fx.push(Effect::Cut),
        ClipboardPaste => fx.push(Effect::Paste),
        ArrangeSeparate => separate_regions(state),
        FileExportPdf => fx.push(Effect::Export(crate::export::Kind::Pdf)),
        FileExportPng => fx.push(Effect::Export(crate::export::Kind::Png)),
        PictureChoose => fx.push(Effect::ChoosePicture),
        TableImportCsv => fx.push(Effect::ImportCsv),
        TableExportCsv => fx.push(Effect::ExportCsv),
        UnitsSi => set_unit_system(state, UnitSystem::Si),
        UnitsUscs => set_unit_system(state, UnitSystem::Uscs),
        UnitsCgs => set_unit_system(state, UnitSystem::Cgs),
        CalcAuto => {
            // Back to automatic: bring the numbers up to date at once. (Stale is
            // only ever true in manual mode, so ask before switching.)
            let catch_up = !state.live_recalc && state.results_stale();
            state.live_recalc = !state.live_recalc;
            if catch_up {
                fx.push(Effect::CalculateNow);
            }
        }
        // The ribbon opens the Result unit list itself; its face runs nothing.
        UnitsShowInMenu => {}
        PagePaper(paper) => set_page(state, true, |p| p.paper = paper),
        PageLandscape(landscape) => set_page(state, true, |p| p.landscape = landscape),
        // A drag over the margin box is one undo step (the run merges).
        PageMarginMm(mm) => set_page(state, false, |p| p.margin_mm = f32::from(mm)),
        InsertSymbol(symbol) => {
            if let Some(i) = state.active {
                fx.extend(insert_symbol(state, i, symbol));
            }
        }
        // The large button on the Symbols panel opens its flyout (the ribbon does that).
        InsertGreek => {}
        HelpShortcuts => state.shortcuts_open = true,
        HelpShortcutsClose => state.shortcuts_open = false,
        HelpAbout => state.about_open = true,
        HelpAboutClose => state.about_open = false,

        // Everything else is not built yet; `CommandId::is_implemented` says so and
        // the test below keeps the two in agreement.
        _ => return None,
    }
    Some(fx)
}

/// Apply what the canvas asked for. Typing recalculates only in live mode.
pub fn apply_canvas(state: &mut AppState, sheet: usize, cmds: Vec<CanvasCmd>) -> Vec<Effect> {
    let before = state.sheets[sheet].doc.eval_revision();
    for cmd in cmds {
        match cmd {
            CanvasCmd::Edit(edit) => {
                state.sheets[sheet].edit(edit);
            }
            CanvasCmd::Seal => state.sheets[sheet].seal(),
            CanvasCmd::DrawShape {
                kind,
                pos,
                size,
                flip,
            } => {
                let spec = ShapeSpec {
                    flip_x: flip.0,
                    flip_y: flip.1,
                    ..ShapeSpec::of(kind)
                };
                state.sheets[sheet].insert_drawn(RegionKind::Shape(Box::new(spec)), pos, size);
                state.ribbon_focus = Some(crate::ribbon::Context::Shape);
            }
            CanvasCmd::TypeAhead(text) => {
                let s = &mut state.sheets[sheet];
                let id = s.insert(RegionKind::math(""));
                s.edit(Edit::SetMathSource { id, source: text });
            }
        }
    }
    let changed = state.sheets[sheet].doc.eval_revision() != before;
    if changed && state.live_recalc {
        vec![Effect::Recalculate]
    } else {
        Vec::new()
    }
}

pub(super) fn add_sheet(state: &mut AppState, mut sheet: OpenSheet) {
    sheet.set_grid(state.settings.grid);
    state.sheets.push(sheet);
    state.active = Some(state.sheets.len() - 1);
    state.output = Default::default();
}

pub(super) fn close_sheet(state: &mut AppState, i: usize) {
    state.sheets.remove(i);
    state.active = match state.active {
        _ if state.sheets.is_empty() => None,
        Some(a) if a == i => Some(i.min(state.sheets.len() - 1)),
        Some(a) if a > i => Some(a - 1),
        other => other,
    };
    state.output = Default::default();
}

/// Run `f` on the active sheet and ask for a recalculation if that changed what
/// the evaluator would compute. Moving or resizing a region never does.
/// Select every region of the active worksheet.
fn select_all(state: &mut AppState) {
    if let Some(i) = state.active {
        state.sheets[i].select_all();
    }
}

fn with_eval_check(state: &mut AppState, fx: &mut Vec<Effect>, f: impl FnOnce(&mut OpenSheet)) {
    let Some(i) = state.active else { return };
    let before = state.sheets[i].doc.eval_revision();
    f(&mut state.sheets[i]);
    if state.sheets[i].doc.eval_revision() != before {
        fx.push(Effect::Recalculate);
    }
}

/// Put `symbol` into the math region being edited, at the caret (replacing the
/// selection), as an ordinary edit of its source. The editor picks it up.
fn insert_symbol(state: &mut AppState, sheet: usize, symbol: char) -> Vec<Effect> {
    let canvas = &state.sheets[sheet].canvas;
    let (Some(id), Some(field)) = (canvas.editing, canvas.field.as_ref()) else {
        return Vec::new();
    };
    let mut after = field.clone();
    after.insert(&symbol.to_string());
    let edit = Edit::SetMathSource {
        id,
        source: after.text(),
    };
    apply_canvas(state, sheet, vec![CanvasCmd::Edit(edit)])
}

fn insert(state: &mut AppState, fx: &mut Vec<Effect>, kind: RegionKind) {
    with_eval_check(state, fx, |s| {
        s.insert(kind);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{PaletteKey, Theme};
    use opencalc_core::document::Point;
    use opencalc_core::edit::Edit;

    fn state() -> AppState {
        AppState::with_sample(PaletteKey::Neutral)
    }

    fn run(s: &mut AppState, c: CommandId) -> Vec<Effect> {
        dispatch(s, c).expect("implemented")
    }

    /// The table says which commands are implemented; dispatch must agree.
    #[test]
    fn dispatch_handles_exactly_the_commands_marked_implemented() {
        for command in CommandId::ALL {
            let mut s = state();
            assert_eq!(
                dispatch(&mut s, *command).is_some(),
                command.is_implemented(),
                "{}: is_implemented and dispatch disagree",
                command.name()
            );
        }
        // And the parameterised ones.
        let mut s = state();
        assert!(dispatch(&mut s, CommandId::SelectSheet(0)).is_some());
        assert!(dispatch(&mut s, CommandId::CloseSheet(5)).is_some());
    }

    #[test]
    fn toggles_flip_and_flip_back() {
        let mut s = state();
        // (command, how to read it, whether it starts on). The grid starts on.
        for (command, get, starts_on) in [
            (
                CommandId::ViewGrid,
                (|s: &AppState| s.show_grid) as fn(&AppState) -> bool,
                true,
            ),
            (CommandId::ViewSnap, |s| s.snap, true),
            (CommandId::CalcAuto, |s| s.live_recalc, true),
            (CommandId::ViewFunctions, |s| s.show_functions, false),
        ] {
            assert_eq!(get(&s), starts_on, "{}", command.name());
            run(&mut s, command);
            assert_eq!(get(&s), !starts_on, "{}", command.name());
            run(&mut s, command);
            assert_eq!(get(&s), starts_on);
        }
    }

    #[test]
    fn reset_layout_restores_the_panels_and_asks_for_new_widths() {
        let mut s = state();
        s.show_functions = true;
        let gen = s.layout_gen;
        assert_eq!(
            run(&mut s, CommandId::ViewResetLayout),
            vec![Effect::ResetDockWidths]
        );
        assert!(!s.show_functions, "the panels are closed by default");
        assert_eq!(s.layout_gen, gen + 1);
    }

    #[test]
    fn the_theme_commands_switch_the_palette_and_ask_egui_to_apply_it() {
        let mut s = state();
        assert_eq!(s.palette, PaletteKey::Neutral);
        assert_eq!(run(&mut s, CommandId::ThemeSlate), vec![Effect::ApplyTheme]);
        assert_eq!(s.palette, PaletteKey::Slate);
        assert_eq!(s.theme.base, Theme::palette(PaletteKey::Slate).base);
        // Choosing the theme already in use changes nothing.
        assert_eq!(run(&mut s, CommandId::ThemeSlate), vec![]);
        assert_eq!(
            run(&mut s, CommandId::ThemeNeutral),
            vec![Effect::ApplyTheme]
        );
        assert_eq!(s.palette, PaletteKey::Neutral);
    }

    #[test]
    fn zoom_to_sets_the_percentage_and_stays_within_the_limits() {
        let mut s = state();
        run(&mut s, CommandId::ZoomTo(150));
        assert_eq!(s.zoom_percent(), 150.0);
        run(&mut s, CommandId::ZoomTo(5));
        assert_eq!(s.zoom_percent(), 25.0);
        run(&mut s, CommandId::ZoomTo(900));
        assert_eq!(s.zoom_percent(), 400.0);
    }

    #[test]
    fn page_setup_changes_the_page_and_undoes() {
        use opencalc_core::document::PaperSize;
        let mut s = state();
        run(&mut s, CommandId::PagePaper(PaperSize::Letter));
        run(&mut s, CommandId::PageLandscape(true));
        let page = s.sheets[0].doc.page();
        assert_eq!(page.paper, PaperSize::Letter);
        assert!(page.landscape);
        run(&mut s, CommandId::EditUndo);
        assert!(!s.sheets[0].doc.page().landscape);
        run(&mut s, CommandId::EditUndo);
        assert_eq!(s.sheets[0].doc.page().paper, PaperSize::A4);
    }

    #[test]
    fn dragging_the_margin_is_one_undo_step() {
        let mut s = state();
        for mm in [21, 22, 23, 30, 35] {
            run(&mut s, CommandId::PageMarginMm(mm));
        }
        assert_eq!(s.sheets[0].doc.page().margin_mm, 35.0);
        run(&mut s, CommandId::EditUndo);
        assert_eq!(s.sheets[0].doc.page().margin_mm, 20.0);
    }

    #[test]
    fn page_setup_opens_and_draft_view_toggles() {
        let mut s = state();
        run(&mut s, CommandId::DocPageSetup);
        assert!(s.page_setup_open);
        run(&mut s, CommandId::DocPageSetupClose);
        assert!(!s.page_setup_open);
        run(&mut s, CommandId::ViewDraftView);
        assert!(s.draft);
        run(&mut s, CommandId::ViewPageView);
        assert!(!s.draft);
    }

    #[test]
    fn options_opens_and_closes() {
        let mut s = state();
        assert!(!s.options_open);
        run(&mut s, CommandId::AppOptions);
        assert!(s.options_open);
        run(&mut s, CommandId::AppOptionsClose);
        assert!(!s.options_open);
    }

    #[test]
    fn about_opens_and_closes() {
        let mut s = state();
        run(&mut s, CommandId::HelpAbout);
        assert!(s.about_open);
        run(&mut s, CommandId::HelpAboutClose);
        assert!(!s.about_open);
    }

    #[test]
    fn quitting_is_an_effect_not_a_state_change() {
        let mut s = state();
        assert_eq!(run(&mut s, CommandId::AppQuit), vec![Effect::Quit]);
    }

    #[test]
    fn new_and_example_sheets_become_active_and_recalculate() {
        let mut s = state();
        assert_eq!(run(&mut s, CommandId::FileNew), vec![Effect::Recalculate]);
        assert_eq!((s.sheets.len(), s.active), (2, Some(1)));
        assert_eq!(s.sheets[1].title(), "Untitled 2");
        run(&mut s, CommandId::FileNew);
        assert_eq!(s.sheets[2].title(), "Untitled 3");
        run(&mut s, CommandId::FileExample);
        assert_eq!((s.sheets.len(), s.active), (4, Some(3)));
    }

    #[test]
    fn closing_sheets_keeps_the_active_one_sensible() {
        let mut s = state();
        for _ in 0..3 {
            run(&mut s, CommandId::FileNew); // sheets 0..=3, active 3
        }
        run(&mut s, CommandId::SelectSheet(2));
        assert_eq!(s.active, Some(2));
        run(&mut s, CommandId::CloseSheet(0)); // an earlier tab closes: index shifts
        assert_eq!((s.sheets.len(), s.active), (3, Some(1)));
        run(&mut s, CommandId::CloseSheet(1)); // the active tab closes: its neighbour
        assert_eq!((s.sheets.len(), s.active), (2, Some(1)));
        run(&mut s, CommandId::CloseSheet(1)); // the last tab closes: step back
        assert_eq!((s.sheets.len(), s.active), (1, Some(0)));
        run(&mut s, CommandId::FileClose); // the only sheet: back to Home
        assert_eq!((s.sheets.len(), s.active), (0, None));
    }

    #[test]
    fn out_of_range_tab_commands_do_nothing_and_do_not_panic() {
        let mut s = state();
        assert!(run(&mut s, CommandId::SelectSheet(99)).is_empty());
        assert!(run(&mut s, CommandId::CloseSheet(99)).is_empty());
        assert_eq!(s.sheets.len(), 1);
        s.active = None;
        for c in [
            CommandId::FileClose,
            CommandId::EditUndo,
            CommandId::RegionDelete,
            CommandId::InsertMath,
            CommandId::ViewZoomIn,
        ] {
            assert!(run(&mut s, c).is_empty(), "{} on the Home page", c.name());
        }
    }

    #[test]
    fn only_inserting_a_math_or_plot_region_asks_for_a_recalculation() {
        let mut s = state();
        // A plot has results of its own to calculate, even when it is empty.
        for c in [CommandId::InsertMath, CommandId::InsertPlot] {
            assert_eq!(run(&mut s, c), vec![Effect::Recalculate], "{}", c.name());
        }
        for c in [
            CommandId::InsertText,
            CommandId::InsertArea,
            CommandId::InsertTable,
            CommandId::InsertPicture,
            CommandId::InsertRectangle,
            CommandId::InsertEllipse,
            CommandId::InsertLine,
            CommandId::InsertTriangle,
            CommandId::InsertDiamond,
            CommandId::InsertPolygon,
        ] {
            assert!(
                run(&mut s, c).is_empty(),
                "{} cannot change a result",
                c.name()
            );
        }
    }

    #[test]
    fn deleting_and_undoing_recalculate_only_when_math_is_involved() {
        let mut s = state();
        let ids: Vec<_> = s.sheets[0].doc.regions().iter().map(|r| r.id).collect();
        // The title text (first region): deleting it changes no result.
        s.sheets[0].canvas.selection = vec![ids[0]];
        assert!(run(&mut s, CommandId::RegionDelete).is_empty());
        assert!(run(&mut s, CommandId::EditUndo).is_empty());
        // A math region: deleting it and undoing it both do.
        s.sheets[0].canvas.selection = vec![ids[2]];
        assert_eq!(
            run(&mut s, CommandId::RegionDelete),
            vec![Effect::Recalculate]
        );
        assert_eq!(run(&mut s, CommandId::EditUndo), vec![Effect::Recalculate]);
        assert_eq!(run(&mut s, CommandId::EditRedo), vec![Effect::Recalculate]);
    }

    #[test]
    fn zoom_commands_change_the_active_sheets_view_only() {
        let mut s = state();
        s.viewport = eframe::egui::Rect::from_min_size(
            eframe::egui::Pos2::ZERO,
            eframe::egui::vec2(800.0, 600.0),
        );
        run(&mut s, CommandId::FileNew);
        run(&mut s, CommandId::ViewZoomIn);
        assert!((s.sheets[1].view.zoom - 1.25).abs() < 1e-6);
        assert_eq!(s.sheets[0].view.zoom, 1.0, "the other sheet is untouched");
        run(&mut s, CommandId::ViewZoomOut);
        assert!((s.sheets[1].view.zoom - 1.0).abs() < 1e-6);
        run(&mut s, CommandId::ViewZoomIn);
        run(&mut s, CommandId::ViewZoom100);
        assert!((s.sheets[1].view.zoom - 1.0).abs() < 1e-6);
    }

    #[test]
    fn fit_zooms_to_the_regions() {
        let mut s = state();
        s.viewport = eframe::egui::Rect::from_min_size(
            eframe::egui::Pos2::ZERO,
            eframe::egui::vec2(400.0, 300.0),
        );
        run(&mut s, CommandId::ViewZoomFit);
        assert!(
            s.sheets[0].view.zoom < 1.0,
            "the sample is wider than 400 px"
        );
    }

    #[test]
    fn typing_recalculates_only_in_live_mode_and_moving_never_does() {
        let mut s = state();
        let id = s.sheets[0].doc.regions()[2].id;
        let typed = || {
            vec![CanvasCmd::Edit(Edit::SetMathSource {
                id,
                source: "F := 20 kN".into(),
            })]
        };
        let moved = || {
            vec![CanvasCmd::Edit(Edit::MoveRegions {
                moves: vec![(id, Point::new(300.0, 300.0))],
            })]
        };
        assert_eq!(apply_canvas(&mut s, 0, moved()), vec![]);
        assert_eq!(apply_canvas(&mut s, 0, typed()), vec![Effect::Recalculate]);
        s.live_recalc = false;
        let again = vec![CanvasCmd::Edit(Edit::SetMathSource {
            id,
            source: "F := 30 kN".into(),
        })];
        assert_eq!(
            apply_canvas(&mut s, 0, again),
            vec![],
            "live recalculation is off"
        );
        // The document still changed; only the recalculation was held back.
        assert!(s.sheets[0].can_undo());
    }

    #[test]
    fn seal_splits_undo_steps() {
        let mut s = state();
        let id = s.sheets[0].doc.regions()[2].id;
        let step = |text: &str| {
            CanvasCmd::Edit(Edit::SetMathSource {
                id,
                source: text.into(),
            })
        };
        apply_canvas(
            &mut s,
            0,
            vec![
                step("F := 1 kN"),
                step("F := 2 kN"),
                CanvasCmd::Seal,
                step("F := 3 kN"),
            ],
        );
        s.sheets[0].undo();
        s.sheets[0].undo();
        assert!(
            !s.sheets[0].can_undo(),
            "two steps: the sealed burst and the last edit"
        );
    }

    #[test]
    fn choosing_a_unit_system_is_an_undoable_edit_that_needs_no_recalculation() {
        let mut s = state();
        s.active = Some(0);
        assert_eq!(run(&mut s, CommandId::UnitsUscs), vec![]);
        assert_eq!(s.sheets[0].doc.unit_system(), UnitSystem::Uscs);
        assert_eq!(s.unit_system_command(), Some(CommandId::UnitsUscs));
        assert!(s.active_toggles().contains(&CommandId::UnitsUscs));
        run(&mut s, CommandId::UnitsCgs);
        assert_eq!(s.sheets[0].doc.unit_system(), UnitSystem::Cgs);
        // Each choice is its own undo step.
        assert_eq!(run(&mut s, CommandId::EditUndo), vec![]);
        assert_eq!(s.sheets[0].doc.unit_system(), UnitSystem::Uscs);
        run(&mut s, CommandId::EditUndo);
        assert_eq!(s.sheets[0].doc.unit_system(), UnitSystem::Si);
    }

    #[test]
    fn each_worksheet_keeps_its_own_unit_system() {
        let mut s = state();
        s.active = Some(0);
        run(&mut s, CommandId::UnitsUscs);
        run(&mut s, CommandId::FileNew);
        assert_eq!(s.unit_system_command(), Some(CommandId::UnitsSi));
        s.active = Some(0);
        assert_eq!(s.unit_system_command(), Some(CommandId::UnitsUscs));
        s.active = None;
        assert_eq!(s.unit_system_command(), None);
        assert_eq!(dispatch(&mut s, CommandId::UnitsSi), Some(vec![]));
    }
}
