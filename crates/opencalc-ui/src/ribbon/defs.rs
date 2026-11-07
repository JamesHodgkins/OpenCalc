//! The OpenCalc ribbon as data (`docs/RIBBON_MAP.md`): Mathcad-style tabs.
//!
//! Items name a command only; their label and icon come from the command table.
//! Tab names follow PTC's public guides; which commands sit where is our own
//! grouping (see `docs/UI_ALIGNMENT.md`).

use crate::commands::{greek_symbols, CommandId};

use super::{Cell, Column, Item, Panel, Ribbon, Split, Tab};

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
            math(),
            functions(),
            matrices(),
            plots(),
            math_formatting(),
            calculation(),
            document(),
            help(),
        ],
    }
}

/// Regions, clipboard, symbols and units.
fn math() -> Tab {
    let greek: Vec<Item> = greek_symbols()
        .map(|c| it(CommandId::InsertSymbol(c)))
        .collect();
    Tab {
        title: "Math",
        panels: vec![
            panel(
                "Regions",
                vec![
                    Column::Large(it(CommandId::InsertMath)),
                    Column::Large(it(CommandId::InsertText)),
                ],
            ),
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
            Panel {
                title: "Symbols",
                columns: vec![Column::Large(it(CommandId::InsertGreek))],
                extra: greek,
            },
            panel(
                "Unit System",
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
                "Display In",
                vec![Column::Rows(vec![
                    vec![Cell::Split(Split {
                        label: Some("Length"),
                        items: vec![
                            it(CommandId::UnitsDisplayM),
                            it(CommandId::UnitsDisplayMm),
                            it(CommandId::UnitsDisplayIn),
                        ],
                    })],
                    vec![Cell::IconSplit(Split {
                        label: None,
                        items: vec![it(CommandId::UnitsDisplayN), it(CommandId::UnitsDisplayKn)],
                    })],
                    vec![Cell::Button(it(CommandId::UnitsDisplayReset))],
                ])],
            ),
            panel(
                "Custom Units",
                vec![stack(&[CommandId::UnitsDefine, CommandId::UnitsManage])],
            ),
        ],
    }
}

fn functions() -> Tab {
    Tab {
        title: "Functions",
        panels: vec![panel(
            "Insert",
            vec![Column::Large(it(CommandId::ViewFunctions))],
        )],
    }
}

fn matrices() -> Tab {
    Tab {
        title: "Matrices/Tables",
        panels: vec![
            panel(
                "Matrix",
                vec![
                    Column::Large(it(CommandId::InsertMatrix)),
                    Column::Large(it(CommandId::InsertRange)),
                ],
            ),
            panel("Table", vec![Column::Large(it(CommandId::InsertTable))]),
        ],
    }
}

fn plots() -> Tab {
    Tab {
        title: "Plots",
        panels: vec![panel(
            "Insert",
            vec![Column::Large(it(CommandId::InsertPlot))],
        )],
    }
}

fn math_formatting() -> Tab {
    Tab {
        title: "Math Formatting",
        panels: vec![
            panel(
                "Result",
                vec![
                    stack(&[CommandId::FormatDecimalsMore, CommandId::FormatDecimalsLess]),
                    stack(&[CommandId::FormatScientific, CommandId::FormatEngineering]),
                ],
            ),
            panel(
                "Font",
                vec![Column::Rows(vec![vec![
                    Cell::Icon(it(CommandId::FormatBold)),
                    Cell::Icon(it(CommandId::FormatItalic)),
                ]])],
            ),
        ],
    }
}

fn calculation() -> Tab {
    Tab {
        title: "Calculation",
        panels: vec![
            panel(
                "Calculate",
                vec![
                    Column::Large(it(CommandId::CalcAll)),
                    stack(&[CommandId::CalcAuto]),
                ],
            ),
            panel("Check", vec![Column::Large(it(CommandId::CalcCheck))]),
            panel(
                "Errors",
                vec![stack(&[CommandId::WarnNext, CommandId::WarnPrev])],
            ),
        ],
    }
}

fn document() -> Tab {
    Tab {
        title: "Document",
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
                vec![stack(&[CommandId::ViewGrid, CommandId::ViewSnap])],
            ),
            panel(
                "Arrange",
                vec![stack(&[
                    CommandId::ArrangeAlignH,
                    CommandId::ArrangeAlignV,
                    CommandId::ArrangeSeparate,
                ])],
            ),
            panel(
                "Insert",
                vec![stack(&[CommandId::InsertArea, CommandId::InsertPicture])],
            ),
            panel("Window", vec![stack(&[CommandId::ViewResetLayout])]),
        ],
    }
}

fn help() -> Tab {
    Tab {
        title: "Help",
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
        let mut v = Vec::new();
        for p in r.tabs.iter().flat_map(|t| &t.panels) {
            v.extend(p.extra.iter());
            for c in &p.columns {
                match c {
                    Column::Large(i) => v.push(i),
                    Column::LargeSplit(s) => v.extend(s.items.iter()),
                    Column::Stack(is) => v.extend(is.iter()),
                    Column::Rows(rows) => {
                        for cell in rows.iter().flatten() {
                            match cell {
                                Cell::Button(i) | Cell::Icon(i) => v.push(i),
                                Cell::Split(s) | Cell::IconSplit(s) => v.extend(s.items.iter()),
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
    fn the_tabs_are_mathcads() {
        let titles: Vec<_> = opencalc_ribbon().tabs.iter().map(|t| t.title).collect();
        assert_eq!(
            titles,
            [
                "Math",
                "Functions",
                "Matrices/Tables",
                "Plots",
                "Math Formatting",
                "Calculation",
                "Document",
                "Help"
            ]
        );
    }

    #[test]
    fn the_panels_are_as_documented_in_the_ribbon_map() {
        assert_eq!(
            panel_titles("Math"),
            [
                "Regions",
                "Clipboard",
                "Edit",
                "Region",
                "Symbols",
                "Unit System",
                "Display In",
                "Custom Units"
            ]
        );
        assert_eq!(panel_titles("Functions"), ["Insert"]);
        assert_eq!(panel_titles("Matrices/Tables"), ["Matrix", "Table"]);
        assert_eq!(panel_titles("Plots"), ["Insert"]);
        assert_eq!(panel_titles("Math Formatting"), ["Result", "Font"]);
        assert_eq!(
            panel_titles("Calculation"),
            ["Calculate", "Check", "Errors"]
        );
        assert_eq!(
            panel_titles("Document"),
            ["Zoom", "Page", "Show", "Arrange", "Insert", "Window"]
        );
    }

    #[test]
    fn command_ids_are_unique() {
        let r = opencalc_ribbon();
        let mut seen = HashSet::new();
        for i in all_items(&r) {
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
        use crate::panels::title::{MENU, QUICK_ACCESS};
        let ribbon = opencalc_ribbon();
        let mut reachable: HashSet<CommandId> =
            all_items(&ribbon).iter().map(|i| i.command).collect();
        reachable.extend(MENU);
        reachable.extend(QUICK_ACCESS);
        reachable.extend([
            CommandId::FileClose,          // the worksheet tab's close button, Ctrl+W
            CommandId::FileExample,        // the Home page
            CommandId::TabHome,            // the Home tab
            CommandId::HelpAboutClose,     // the About window
            CommandId::AppOptionsClose,    // the Options window
            CommandId::DocPageSetupClose,  // the Page setup window
            CommandId::ThemeNeutral,       // the Options window
            CommandId::ThemeSlate,         // the Options window
            CommandId::DialogSave,         // the unsaved-changes dialog
            CommandId::DialogDiscard,      // the unsaved-changes dialog
            CommandId::DialogCancel,       // the unsaved-changes dialog
            CommandId::DialogOk,           // the message dialog
            CommandId::DialogRestore,      // the recovery dialog
            CommandId::DialogDropRecovery, // the recovery dialog
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

    #[test]
    fn the_new_controls_that_are_not_built_yet_are_dimmed_not_live() {
        for command in [CommandId::InsertMatrix, CommandId::InsertRange] {
            assert!(!command.is_implemented(), "{command:?}");
        }
    }
}
