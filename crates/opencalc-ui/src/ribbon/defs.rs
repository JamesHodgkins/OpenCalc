//! The OpenCalc ribbon as data (spec section 4.8, "OpenCalc ribbon").
//!
//! Icons are placeholder glyphs until the Lucide icon font is bundled
//! (spec section 4.6).

use crate::icons;

use super::{Cell, Column, Item, Panel, Ribbon, Split, Tab};

const fn it(command: &'static str, label: &'static str, icon: char) -> Item {
    Item {
        command,
        label,
        icon,
    }
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
                        items: vec![it("clipboard.paste", "Paste", icons::PASTE)],
                    }),
                    Column::Stack(vec![
                        it("clipboard.cut", "Cut", icons::CUT),
                        it("clipboard.copy", "Copy", icons::COPY),
                    ]),
                ],
            ),
            panel(
                "Edit",
                vec![
                    Column::Stack(vec![
                        it("edit.undo", "Undo", icons::UNDO),
                        it("edit.redo", "Redo", icons::REDO),
                    ]),
                    Column::Stack(vec![
                        it("edit.find", "Find", icons::FIND),
                        it("edit.replace", "Replace", icons::REPLACE),
                    ]),
                ],
            ),
            panel(
                "Region",
                vec![Column::Stack(vec![
                    it("region.delete", "Delete", icons::DELETE),
                    it("region.lock", "Lock", icons::LOCK),
                    it("region.tag", "Tag", icons::TAG),
                ])],
            ),
        ],
    }
}

fn insert() -> Tab {
    let greek: Vec<Item> = [
        ("alpha", '\u{03b1}'),
        ("beta", '\u{03b2}'),
        ("gamma", '\u{03b3}'),
        ("delta", '\u{03b4}'),
        ("epsilon", '\u{03b5}'),
        ("theta", '\u{03b8}'),
        ("lambda", '\u{03bb}'),
        ("mu", '\u{03bc}'),
        ("pi", '\u{03c0}'),
        ("rho", '\u{03c1}'),
        ("sigma", '\u{03c3}'),
        ("tau", '\u{03c4}'),
        ("phi", '\u{03c6}'),
        ("omega", '\u{03c9}'),
        ("Delta", '\u{0394}'),
        ("Omega", '\u{03a9}'),
    ]
    .into_iter()
    .map(|(name, ch)| it(greek_command(name), greek_label(name), ch))
    .collect();
    Tab {
        title: "Insert",
        panels: vec![
            panel(
                "Math",
                vec![Column::Large(it("insert.math", "Math region", icons::MATH))],
            ),
            panel(
                "Text",
                vec![Column::Large(it("insert.text", "Text region", icons::TEXT))],
            ),
            panel(
                "Layout",
                vec![
                    Column::Stack(vec![
                        it("insert.area", "Area", icons::AREA),
                        it("insert.plot", "Plot", icons::PLOT),
                        it("insert.table", "Table", icons::TABLE),
                    ]),
                    Column::Stack(vec![it("insert.picture", "Picture", icons::PICTURE)]),
                ],
            ),
            Panel {
                title: "Symbols",
                columns: vec![Column::Large(it(
                    "insert.greek",
                    "Greek letters",
                    icons::GREEK,
                ))],
                extra: greek,
            },
        ],
    }
}

/// Command ID and tooltip for a Greek letter, from a static table.
fn greek_command(name: &str) -> &'static str {
    GREEK
        .iter()
        .find(|(n, _, _)| *n == name)
        .map_or("insert.greek", |(_, c, _)| *c)
}

fn greek_label(name: &str) -> &'static str {
    GREEK
        .iter()
        .find(|(n, _, _)| *n == name)
        .map_or("Greek letter", |(_, _, l)| *l)
}

pub static GREEK: [(&str, &str, &str); 16] = [
    ("alpha", "greek.alpha", "alpha"),
    ("beta", "greek.beta", "beta"),
    ("gamma", "greek.gamma", "gamma"),
    ("delta", "greek.delta", "delta"),
    ("epsilon", "greek.epsilon", "epsilon"),
    ("theta", "greek.theta", "theta"),
    ("lambda", "greek.lambda", "lambda"),
    ("mu", "greek.mu", "mu"),
    ("pi", "greek.pi", "pi"),
    ("rho", "greek.rho", "rho"),
    ("sigma", "greek.sigma", "sigma"),
    ("tau", "greek.tau", "tau"),
    ("phi", "greek.phi", "phi"),
    ("omega", "greek.omega", "omega"),
    ("Delta", "greek.Delta", "Delta"),
    ("Omega", "greek.Omega", "Omega"),
];

fn format() -> Tab {
    Tab {
        title: "Format",
        panels: vec![
            panel(
                "Font",
                vec![Column::Rows(vec![vec![
                    Cell::Icon(it("format.bold", "Bold", icons::BOLD)),
                    Cell::Icon(it("format.italic", "Italic", icons::ITALIC)),
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
                        it("format.decimals_more", "More decimals", icons::PLUS),
                        it("format.decimals_less", "Fewer decimals", icons::MINUS),
                    ]),
                    Column::Stack(vec![
                        it("format.scientific", "Scientific", icons::SCIENTIFIC),
                        it("format.engineering", "Engineering", icons::ENGINEERING),
                    ]),
                ],
            ),
            panel(
                "Arrange",
                vec![Column::Stack(vec![
                    it("arrange.align_h", "Align horizontally", icons::ALIGN_H),
                    it("arrange.align_v", "Align vertically", icons::ALIGN_V),
                    it("arrange.separate", "Separate regions", icons::SEPARATE),
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
                        it("units.si", "SI", icons::RULER),
                        it("units.uscs", "USCS", icons::RULER),
                        it("units.cgs", "CGS", icons::RULER),
                    ],
                })],
            ),
            panel(
                "Convert",
                vec![Column::Rows(vec![
                    vec![Cell::Split(Split {
                        label: Some("Length"),
                        items: vec![
                            it("units.display_m", "Metres", icons::RULER),
                            it("units.display_mm", "Millimetres", icons::RULER),
                            it("units.display_in", "Inches", icons::RULER),
                        ],
                    })],
                    vec![Cell::IconSplit(Split {
                        label: None,
                        items: vec![
                            it("units.display_n", "Force in newtons", icons::CONVERT),
                            it("units.display_kn", "Force in kilonewtons", icons::CONVERT),
                        ],
                    })],
                    vec![Cell::Button(it(
                        "units.display_reset",
                        "Reset display",
                        icons::RESET,
                    ))],
                ])],
            ),
            panel(
                "Custom",
                vec![Column::Stack(vec![
                    it("units.define", "Define unit", icons::PLUS),
                    it("units.manage", "Manage units", icons::LIST),
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
                    Column::Large(it("calc.all", "Recalculate all", icons::REFRESH)),
                    Column::Stack(vec![it("calc.changed", "Changed only", icons::PLAY)]),
                ],
            ),
            panel(
                "Check",
                vec![Column::Large(it("calc.check", "Run checks", icons::CHECK))],
            ),
            panel(
                "Warnings",
                vec![Column::Stack(vec![
                    it("warn.list", "List", icons::LIST),
                    it("warn.next", "Next", icons::ARROW_DOWN),
                    it("warn.prev", "Previous", icons::ARROW_UP),
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
                        it("view.zoom_in", "Zoom in", icons::ZOOM_IN),
                        it("view.zoom_out", "Zoom out", icons::ZOOM_OUT),
                        it("view.zoom_fit", "Fit", icons::FIT),
                    ]),
                    Column::Stack(vec![it("view.zoom_100", "100%", icons::ZOOM_100)]),
                ],
            ),
            panel(
                "Show",
                vec![Column::Stack(vec![
                    it("view.grid", "Grid", icons::GRID),
                    it("view.snap", "Snap", icons::SNAP),
                    it("view.guides", "Guides", icons::GUIDES),
                ])],
            ),
            panel(
                "Panels",
                vec![Column::Stack(vec![
                    it("view.names", "Names", icons::NAMES),
                    it("view.properties", "Properties", icons::PROPERTIES),
                    it("view.reset_layout", "Reset layout", icons::RESET),
                ])],
            ),
            panel(
                "Theme",
                vec![Column::Stack(vec![it(
                    "view.theme",
                    "Switch palette",
                    icons::THEME,
                )])],
            ),
        ],
    }
}

fn help() -> Tab {
    Tab {
        title: "Help",
        panels: vec![panel(
            "Help",
            vec![
                Column::Large(it("help.shortcuts", "Shortcuts", icons::KEYBOARD)),
                Column::Large(it("help.guide", "User guide", icons::BOOK)),
                Column::Large(it("help.about", "About", icons::INFO)),
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
            assert!(seen.insert(i.command), "duplicate command id {}", i.command);
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
