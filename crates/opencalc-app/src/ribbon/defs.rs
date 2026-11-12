//! The OpenCalc ribbon as data (`docs/RIBBON_MAP.md`).
//!
//! Items name a command only; their label and icon come from the command table.
//! The tabs are organised by task (Home, Insert, Draw, Format, Units, View, Help),
//! with contextual tabs for the selected thing; the grouping is our own.

use crate::commands::{greek_symbols, CommandId};

use super::{Cell, Column, Context, Item, Panel, Ribbon, Split, Tab};

fn it(command: CommandId) -> Item {
    Item::of(command)
}

fn panel(title: &'static str, columns: Vec<Column>) -> Panel {
    Panel {
        title,
        columns,
        extra: Vec::new(),
    }
}

fn stack(commands: &[CommandId]) -> Column {
    Column::Stack(commands.iter().map(|c| it(*c)).collect())
}

pub fn opencalc_ribbon() -> Ribbon {
    Ribbon {
        tabs: vec![
            home(),
            insert(),
            draw(),
            format(),
            units(),
            view(),
            help(),
            shape_format(),
        ],
    }
}

/// Everyday work: the clipboard, new regions, finding, region housekeeping and
/// calculating. Nothing here is rare, so it is the tab the program opens on.
fn home() -> Tab {
    Tab {
        title: "Home",
        context: None,
        panels: vec![
            panel(
                "Clipboard",
                vec![
                    Column::LargeSplit(Split {
                        label: Some("Paste"),
                        items: vec![it(CommandId::ClipboardPaste)],
                    }),
                    stack(&[CommandId::ClipboardCut, CommandId::ClipboardCopy]),
                ],
            ),
            panel(
                "Regions",
                vec![
                    Column::Large(it(CommandId::InsertMath)),
                    Column::Large(it(CommandId::InsertText)),
                ],
            ),
            panel(
                "Edit",
                vec![stack(&[CommandId::EditFind, CommandId::EditReplace])],
            ),
            panel(
                "Region",
                vec![stack(&[
                    CommandId::RegionDelete,
                    CommandId::RegionLock,
                    CommandId::RegionTag,
                ])],
            ),
            panel(
                "Calculate",
                vec![
                    Column::Large(it(CommandId::CalcAll)),
                    stack(&[CommandId::CalcAuto]),
                ],
            ),
            panel(
                "Check",
                vec![
                    Column::Large(it(CommandId::CalcCheck)),
                    stack(&[CommandId::WarnNext, CommandId::WarnPrev]),
                ],
            ),
        ],
    }
}

/// Things that are added to a worksheet besides plain maths and text: data,
/// graphics, programs, symbols and functions.
fn insert() -> Tab {
    let greek: Vec<Item> = greek_symbols()
        .map(|c| it(CommandId::InsertSymbol(c)))
        .collect();
    Tab {
        title: "Insert",
        context: None,
        panels: vec![
            panel(
                "Data",
                vec![
                    Column::Large(it(CommandId::InsertMatrix)),
                    Column::Large(it(CommandId::InsertRange)),
                    Column::Large(it(CommandId::InsertTable)),
                ],
            ),
            panel(
                "Objects",
                vec![
                    Column::Large(it(CommandId::InsertPlot)),
                    Column::Large(it(CommandId::InsertPicture)),
                    Column::Large(it(CommandId::InsertArea)),
                ],
            ),
            panel(
                "Programming",
                vec![
                    Column::Large(it(CommandId::InsertSolveBlock)),
                    Column::Large(it(CommandId::InsertProgram)),
                ],
            ),
            Panel {
                title: "Symbols",
                columns: vec![Column::Large(it(CommandId::InsertGreek))],
                extra: greek,
            },
            panel(
                "Functions",
                vec![Column::Large(it(CommandId::ViewFunctions))],
            ),
        ],
    }
}

/// Drawing and laying out: shapes, the order they are painted in, and aligning
/// regions. (More drawing tools, such as lines, belong here as they arrive.)
fn draw() -> Tab {
    Tab {
        title: "Draw",
        context: None,
        panels: vec![
            panel(
                "Shapes",
                vec![
                    Column::Large(it(CommandId::InsertRectangle)),
                    Column::Large(it(CommandId::InsertEllipse)),
                    Column::Large(it(CommandId::InsertLine)),
                    Column::Large(it(CommandId::InsertTriangle)),
                    Column::Large(it(CommandId::InsertDiamond)),
                    Column::Large(it(CommandId::InsertPolygon)),
                ],
            ),
            panel(
                "Order",
                vec![
                    stack(&[CommandId::ArrangeToFront, CommandId::ArrangeForward]),
                    stack(&[CommandId::ArrangeBackward, CommandId::ArrangeToBack]),
                ],
            ),
            panel(
                "Align",
                vec![stack(&[
                    CommandId::ArrangeAlignH,
                    CommandId::ArrangeAlignV,
                    CommandId::ArrangeSeparate,
                ])],
            ),
        ],
    }
}

/// How things look: text style, how numbers are shown, and borders.
fn format() -> Tab {
    Tab {
        title: "Format",
        context: None,
        panels: vec![
            panel(
                "Text",
                vec![Column::Rows(vec![vec![
                    Cell::Icon(it(CommandId::FormatBold)),
                    Cell::Icon(it(CommandId::FormatItalic)),
                    Cell::Icon(it(CommandId::FormatUnderline)),
                ]])],
            ),
            panel(
                "Numbers",
                vec![
                    stack(&[CommandId::FormatDecimalsMore, CommandId::FormatDecimalsLess]),
                    stack(&[CommandId::FormatScientific, CommandId::FormatEngineering]),
                ],
            ),
            panel("Region", vec![Column::Large(it(CommandId::FormatBorder))]),
        ],
    }
}

/// Units: the system results are shown in, the unit of one result, and the
/// worksheet's own units.
fn units() -> Tab {
    Tab {
        title: "Units",
        context: None,
        panels: vec![
            panel(
                "Unit system",
                vec![Column::LargeSplit(Split {
                    label: None,
                    items: vec![
                        it(CommandId::UnitsSi),
                        it(CommandId::UnitsUscs),
                        it(CommandId::UnitsCgs),
                    ],
                })],
            ),
            panel(
                "Result unit",
                vec![Column::Rows(vec![vec![Cell::Menu {
                    menu: it(CommandId::UnitsShowInMenu),
                    first: it(CommandId::UnitsDisplayReset),
                }]])],
            ),
            panel(
                "Custom units",
                vec![stack(&[CommandId::UnitsDefine, CommandId::UnitsManage])],
            ),
        ],
    }
}

/// How the worksheet is shown: zoom, page, guides and windows.
fn view() -> Tab {
    Tab {
        title: "View",
        context: None,
        panels: vec![
            panel(
                "Zoom",
                vec![
                    stack(&[
                        CommandId::ViewZoomIn,
                        CommandId::ViewZoomOut,
                        CommandId::ViewZoomFit,
                    ]),
                    stack(&[CommandId::ViewZoom100]),
                ],
            ),
            panel(
                "Page",
                vec![
                    Column::Large(it(CommandId::DocPageSetup)),
                    stack(&[CommandId::ViewPageView, CommandId::ViewDraftView]),
                ],
            ),
            panel(
                "Show",
                vec![stack(&[
                    CommandId::ViewGrid,
                    CommandId::ViewSnap,
                    CommandId::ViewLabels,
                ])],
            ),
            panel(
                "Window",
                vec![Column::Large(it(CommandId::ViewResetLayout))],
            ),
        ],
    }
}

/// Contextual: the settings of the selected shape.
fn shape_format() -> Tab {
    use crate::ribbon::{Control, ShapeControl as S};
    Tab {
        title: "Shape Format",
        context: Some(Context::Shape),
        panels: vec![
            panel(
                "Shape",
                vec![Column::Controls(vec![
                    Control::Shape(S::Kind),
                    Control::Shape(S::Corners),
                ])],
            ),
            panel(
                "Style",
                vec![
                    Column::Controls(vec![Control::Shape(S::Fill), Control::Shape(S::Outline)]),
                    Column::Controls(vec![Control::Shape(S::Width), Control::Shape(S::Line)]),
                ],
            ),
            panel(
                "Arrowheads",
                vec![Column::Controls(vec![
                    Control::Shape(S::StartHead),
                    Control::Shape(S::EndHead),
                ])],
            ),
            panel(
                "Arrange",
                vec![
                    stack(&[CommandId::ArrangeToFront, CommandId::ArrangeForward]),
                    stack(&[CommandId::ArrangeBackward, CommandId::ArrangeToBack]),
                    stack(&[CommandId::RegionLock, CommandId::RegionDelete]),
                ],
            ),
        ],
    }
}

fn help() -> Tab {
    Tab {
        title: "Help",
        context: None,
        panels: vec![panel(
            "Help",
            vec![
                Column::Large(it(CommandId::HelpShortcuts)),
                Column::Large(it(CommandId::HelpGuide)),
                Column::Large(it(CommandId::HelpAbout)),
            ],
        )],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn all_items(r: &Ribbon) -> Vec<&Item> {
        items_of(r.tabs.iter().collect())
    }

    /// The items of the ordinary tabs: each command sits in exactly one of them. A
    /// contextual tab repeats commands for the thing selected, as other apps do.
    fn ordinary_items(r: &Ribbon) -> Vec<&Item> {
        items_of(r.tabs.iter().filter(|t| t.context.is_none()).collect())
    }

    fn items_of(tabs: Vec<&Tab>) -> Vec<&Item> {
        let mut v = Vec::new();
        for p in tabs.into_iter().flat_map(|t| &t.panels) {
            v.extend(p.extra.iter());
            for c in &p.columns {
                match c {
                    Column::Large(i) => v.push(i),
                    Column::LargeSplit(s) => v.extend(s.items.iter()),
                    Column::Stack(is) => v.extend(is.iter()),
                    Column::Controls(_) => {}
                    Column::Rows(rows) => {
                        for cell in rows.iter().flatten() {
                            match cell {
                                Cell::Icon(i) => v.push(i),
                                Cell::Menu { menu, first } => {
                                    v.push(menu);
                                    v.push(first);
                                }
                            }
                        }
                    }
                }
            }
        }
        v
    }

    fn panel_titles(tab: &str) -> Vec<&'static str> {
        let r = opencalc_ribbon();
        let tab = r.tabs.iter().find(|t| t.title == tab).expect(tab);
        tab.panels.iter().map(|p| p.title).collect()
    }

    #[test]
    fn the_tabs_are_organised_by_task() {
        let titles: Vec<_> = opencalc_ribbon().tabs.iter().map(|t| t.title).collect();
        assert_eq!(
            titles,
            [
                "Home",
                "Insert",
                "Draw",
                "Format",
                "Units",
                "View",
                "Help",
                "Shape Format"
            ]
        );
    }

    #[test]
    fn a_contextual_tab_shows_only_while_its_context_is_selected() {
        let r = opencalc_ribbon();
        let shape = r
            .tabs
            .iter()
            .position(|t| t.context == Some(Context::Shape))
            .unwrap();
        assert!(!r.visible(&[]).contains(&shape));
        assert!(r.visible(&[Context::Shape]).contains(&shape));
        assert_eq!(r.visible(&[]).len() + 1, r.tabs.len());
        // The tab on show falls back to the first when its context goes.
        assert_eq!(r.current(shape, &[]), 0);
        assert_eq!(r.current(shape, &[Context::Shape]), shape);
        assert_eq!(r.current(2, &[]), 2, "an ordinary tab is left alone");
    }

    #[test]
    fn the_panels_are_as_documented_in_the_ribbon_map() {
        assert_eq!(
            panel_titles("Home"),
            [
                "Clipboard",
                "Regions",
                "Edit",
                "Region",
                "Calculate",
                "Check"
            ]
        );
        assert_eq!(
            panel_titles("Insert"),
            ["Data", "Objects", "Programming", "Symbols", "Functions"]
        );
        assert_eq!(panel_titles("Draw"), ["Shapes", "Order", "Align"]);
        assert_eq!(panel_titles("Format"), ["Text", "Numbers", "Region"]);
        assert_eq!(
            panel_titles("Units"),
            ["Unit system", "Result unit", "Custom units"]
        );
        assert_eq!(panel_titles("View"), ["Zoom", "Page", "Show", "Window"]);
        assert_eq!(panel_titles("Help"), ["Help"]);
        assert_eq!(
            panel_titles("Shape Format"),
            ["Shape", "Style", "Arrowheads", "Arrange"]
        );
    }

    /// A tab with one control is a sign the ribbon is badly divided: every ordinary
    /// tab holds at least four commands in at least two groups, except Help.
    #[test]
    fn no_ordinary_tab_is_nearly_empty() {
        let r = opencalc_ribbon();
        for tab in r
            .tabs
            .iter()
            .filter(|t| t.context.is_none() && t.title != "Help")
        {
            let count = items_of(vec![tab]).len();
            assert!(count >= 4, "{} has only {count} commands", tab.title);
            assert!(tab.panels.len() >= 2, "{} has one group", tab.title);
        }
    }

    /// Every tab, drawn for real, must fit the narrowest window the program allows.
    #[test]
    fn every_tab_fits_the_smallest_window() {
        use crate::ribbon::{CommandSource, ControlHost, RibbonState};
        use eframe::egui;
        struct All;
        impl CommandSource for All {
            fn is_enabled(&self, _: CommandId) -> bool {
                true
            }
            fn shortcut_text(&self, _: CommandId) -> Option<String> {
                None
            }
        }
        struct Blank;
        impl ControlHost for Blank {
            fn control(
                &mut self,
                _: &mut egui::Ui,
                _: &crate::theme::Theme,
                _: crate::ribbon::Control,
            ) {
            }
        }
        let ribbon = opencalc_ribbon();
        let theme = crate::theme::Theme::paper();
        let ctx = egui::Context::default();
        let all = [Context::Shape];
        for tab in 0..ribbon.tabs.len() {
            let mut width = 0.0;
            let mut state = RibbonState {
                active_tab: tab,
                ..Default::default()
            };
            // Two frames: the first lays text out, the second has settled.
            for _ in 0..2 {
                let input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(3000.0, 400.0),
                    )),
                    ..Default::default()
                };
                let _ = ctx.run(input, |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        width = ribbon
                            .show(ui, &theme, &mut state, &All, &all, &mut Blank)
                            .width;
                    });
                });
            }
            // The ribbon sits inside 6 px margins on each side.
            let room = crate::MIN_WIDTH - 12.0;
            assert!(
                width <= room,
                "the {} tab needs {width:.0} px; the smallest window leaves {room:.0}",
                ribbon.tabs[tab].title
            );
        }
    }

    /// The widgets of a column of controls start at one x, whatever their labels.
    #[test]
    fn the_controls_of_a_column_line_up() {
        use crate::ribbon::{Control, ControlHost, RibbonState, ShapeControl as S};
        use eframe::egui;
        struct All;
        impl crate::ribbon::CommandSource for All {
            fn is_enabled(&self, _: CommandId) -> bool {
                true
            }
            fn shortcut_text(&self, _: CommandId) -> Option<String> {
                None
            }
        }
        #[derive(Default)]
        struct Record(Vec<(Control, f32)>);
        impl ControlHost for Record {
            fn control(&mut self, ui: &mut egui::Ui, _: &crate::theme::Theme, c: Control) {
                self.0.push((c, ui.cursor().min.x));
                ui.allocate_space(egui::vec2(40.0, 10.0));
            }
        }
        let ribbon = opencalc_ribbon();
        let tab = ribbon
            .tabs
            .iter()
            .position(|t| t.context == Some(Context::Shape))
            .unwrap();
        let theme = crate::theme::Theme::paper();
        let ctx = egui::Context::default();
        let mut host = Record::default();
        let mut state = RibbonState {
            active_tab: tab,
            ..Default::default()
        };
        for _ in 0..2 {
            host.0.clear();
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1200.0, 300.0),
                )),
                ..Default::default()
            };
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ribbon.show(ui, &theme, &mut state, &All, &[Context::Shape], &mut host);
                });
            });
        }
        let x = |c: S| {
            host.0
                .iter()
                .find(|(k, _)| *k == Control::Shape(c))
                .unwrap()
                .1
        };
        assert_eq!(x(S::Fill), x(S::Outline), "Fill and Outline");
        assert_eq!(x(S::Width), x(S::Line), "Width and Line");
        assert_eq!(x(S::Kind), x(S::Corners), "Shape and Corners");
        assert!(
            x(S::Width) - x(S::Fill) >= 116.0,
            "a gap is left between the columns: {} apart",
            x(S::Width) - x(S::Fill)
        );
    }

    #[test]
    fn command_ids_are_unique() {
        let r = opencalc_ribbon();
        let mut seen = HashSet::new();
        for i in ordinary_items(&r) {
            assert!(
                seen.insert(i.command),
                "duplicate command id {:?}",
                i.command
            );
        }
    }

    /// Nothing may exist without a way to reach it: every command is on the
    /// ribbon, the application menu or the quick access toolbar, or is raised by a
    /// window, a tab, the Home page or a key (listed here).
    #[test]
    fn every_command_can_be_reached_from_somewhere() {
        use crate::panels::title::QUICK_ACCESS;
        let ribbon = opencalc_ribbon();
        let mut reachable: HashSet<CommandId> =
            all_items(&ribbon).iter().map(|i| i.command).collect();
        reachable.extend(crate::panels::app_menu::commands());
        reachable.extend(QUICK_ACCESS);
        reachable.extend([
            CommandId::FileClose,          // the worksheet tab's close button, Ctrl+W
            CommandId::FileExample,        // the Home page, the File menu
            CommandId::AppMenu,            // the app button, Alt+F
            CommandId::AppMenuClose,       // the File menu: Escape, a click outside
            CommandId::TabHome,            // the Home tab
            CommandId::HelpAboutClose,     // the About window
            CommandId::AppOptionsClose,    // the Options window
            CommandId::AppOptionsDefaults, // the Options window
            CommandId::FileClearRecent,    // the Options window
            CommandId::DocPageSetupClose,  // the Page setup window
            CommandId::ThemeNeutral,       // the Options window
            CommandId::ThemeSlate,         // the Options window
            CommandId::DialogSave,         // the unsaved-changes dialog
            CommandId::DialogDiscard,      // the unsaved-changes dialog
            CommandId::DialogCancel,       // the unsaved-changes dialog
            CommandId::UnitsClose,         // the Units window
            CommandId::FindNext,           // the Find window, F3
            CommandId::FindPrev,           // the Find window, Shift+F3
            CommandId::FindReplaceOne,     // the Find window
            CommandId::FindReplaceAll,     // the Find window
            CommandId::FindClose,          // the Find window
            CommandId::CheckClose,         // the Check window
            CommandId::MatrixClose,        // the Matrix window
            CommandId::LabelClose,         // the Label window
            CommandId::LabelsClose,        // the Labels window
            CommandId::FileTypeRegister,   // the Options window
            CommandId::FileTypeRemove,     // the Options window
            CommandId::HelpShortcutsClose, // the Shortcuts window
            CommandId::DialogOk,           // the message dialog
            CommandId::DialogRestore,      // the recovery dialog
            CommandId::DialogDropRecovery, // the recovery dialog
            CommandId::PictureChoose,      // the Picture window
            CommandId::TableImportCsv,     // the Table window
            CommandId::TableExportCsv,     // the Table window
        ]);
        for command in CommandId::ALL {
            assert!(reachable.contains(command), "{command:?} cannot be reached");
        }
    }

    #[test]
    fn stacks_and_rows_hold_at_most_three_rows() {
        for p in opencalc_ribbon().tabs.iter().flat_map(|t| &t.panels) {
            for c in &p.columns {
                match c {
                    Column::Stack(v) => assert!(v.len() <= 3, "{}", p.title),
                    Column::Rows(v) => assert!(v.len() <= 3, "{}", p.title),
                    _ => {}
                }
            }
        }
    }

    #[test]
    fn greek_flyout_commands_are_all_distinct() {
        let r = opencalc_ribbon();
        let symbols = r
            .tabs
            .iter()
            .flat_map(|t| &t.panels)
            .find(|p| p.title == "Symbols")
            .expect("a Symbols panel");
        let ids: HashSet<_> = symbols.extra.iter().map(|i| i.command).collect();
        assert_eq!(ids.len(), 16);
    }
}
