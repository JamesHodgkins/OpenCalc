//! The OpenCalc ribbon as data (spec section 4.8, "OpenCalc ribbon").
//!
//! Items name a command only; their label and icon come from the command table.

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

pub fn opencalc_ribbon() -> Ribbon {
    Ribbon {
        tabs: vec![
            home(),
            insert(),
            format(),
            units(),
            calculate(),
            view(),
            help(),
        ],
    }
}

fn home() -> Tab {
    Tab {
        title: "Home",
        panels: vec![
            panel(
                "Clipboard",
                vec![
                    Column::LargeSplit(Split {
                        label: Some("Paste"),
                        items: vec![it(CommandId::ClipboardPaste)],
                    }),
                    Column::Stack(vec![
                        it(CommandId::ClipboardCut),
                        it(CommandId::ClipboardCopy),
                    ]),
                ],
            ),
            panel(
                "Edit",
                vec![
                    Column::Stack(vec![it(CommandId::EditUndo), it(CommandId::EditRedo)]),
                    Column::Stack(vec![it(CommandId::EditFind), it(CommandId::EditReplace)]),
                ],
            ),
            panel(
                "Region",
                vec![Column::Stack(vec![
                    it(CommandId::RegionDelete),
                    it(CommandId::RegionLock),
                    it(CommandId::RegionTag),
                ])],
            ),
        ],
    }
}

fn insert() -> Tab {
    let greek: Vec<Item> = greek_symbols()
        .map(|c| it(CommandId::InsertSymbol(c)))
        .collect();
    Tab {
        title: "Insert",
        panels: vec![
            panel("Math", vec![Column::Large(it(CommandId::InsertMath))]),
            panel("Text", vec![Column::Large(it(CommandId::InsertText))]),
            panel(
                "Layout",
                vec![
                    Column::Stack(vec![
                        it(CommandId::InsertArea),
                        it(CommandId::InsertPlot),
                        it(CommandId::InsertTable),
                    ]),
                    Column::Stack(vec![it(CommandId::InsertPicture)]),
                ],
            ),
            Panel {
                title: "Symbols",
                columns: vec![Column::Large(it(CommandId::InsertGreek))],
                extra: greek,
            },
        ],
    }
}

fn format() -> Tab {
    Tab {
        title: "Format",
        panels: vec![
            panel(
                "Font",
                vec![Column::Rows(vec![vec![
                    Cell::Icon(it(CommandId::FormatBold)),
                    Cell::Icon(it(CommandId::FormatItalic)),
                    Cell::Slot {
                        id: "font_size",
                        width: 60.0,
                    },
                ]])],
            ),
            panel(
                "Number",
                vec![
                    Column::Stack(vec![
                        it(CommandId::FormatDecimalsMore),
                        it(CommandId::FormatDecimalsLess),
                    ]),
                    Column::Stack(vec![
                        it(CommandId::FormatScientific),
                        it(CommandId::FormatEngineering),
                    ]),
                ],
            ),
            panel(
                "Arrange",
                vec![Column::Stack(vec![
                    it(CommandId::ArrangeAlignH),
                    it(CommandId::ArrangeAlignV),
                    it(CommandId::ArrangeSeparate),
                ])],
            ),
        ],
    }
}

fn units() -> Tab {
    Tab {
        title: "Units",
        panels: vec![
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
                "Convert",
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
                "Custom",
                vec![Column::Stack(vec![
                    it(CommandId::UnitsDefine),
                    it(CommandId::UnitsManage),
                ])],
            ),
        ],
    }
}

fn calculate() -> Tab {
    Tab {
        title: "Calculate",
        panels: vec![
            panel(
                "Recalculate",
                vec![
                    Column::Large(it(CommandId::CalcAll)),
                    Column::Stack(vec![it(CommandId::CalcChanged)]),
                ],
            ),
            panel("Check", vec![Column::Large(it(CommandId::CalcCheck))]),
            panel(
                "Warnings",
                vec![Column::Stack(vec![
                    it(CommandId::WarnList),
                    it(CommandId::WarnNext),
                    it(CommandId::WarnPrev),
                ])],
            ),
        ],
    }
}

fn view() -> Tab {
    Tab {
        title: "View",
        panels: vec![
            panel(
                "Zoom",
                vec![
                    Column::Stack(vec![
                        it(CommandId::ViewZoomIn),
                        it(CommandId::ViewZoomOut),
                        it(CommandId::ViewZoomFit),
                    ]),
                    Column::Stack(vec![it(CommandId::ViewZoom100)]),
                ],
            ),
            panel(
                "Show",
                vec![Column::Stack(vec![
                    it(CommandId::ViewGrid),
                    it(CommandId::ViewSnap),
                    it(CommandId::ViewGuides),
                ])],
            ),
            panel(
                "Panels",
                vec![Column::Stack(vec![
                    it(CommandId::ViewNames),
                    it(CommandId::ViewProperties),
                    it(CommandId::ViewResetLayout),
                ])],
            ),
            panel("Theme", vec![Column::Stack(vec![it(CommandId::ViewTheme)])]),
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
                                Cell::Slot { .. } => {}
                            }
                        }
                    }
                }
            }
        }
        v
    }

    #[test]
    fn has_the_seven_confirmed_tabs() {
        let titles: Vec<_> = opencalc_ribbon().tabs.iter().map(|t| t.title).collect();
        assert_eq!(
            titles,
            [
                "Home",
                "Insert",
                "Format",
                "Units",
                "Calculate",
                "View",
                "Help"
            ]
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
        let symbols = &r.tabs[1].panels[3];
        let ids: HashSet<_> = symbols.extra.iter().map(|i| i.command).collect();
        assert_eq!(ids.len(), 16);
    }
}
