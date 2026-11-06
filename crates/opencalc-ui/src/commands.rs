//! Command registry (spec section 4.7).
//!
//! Every user action is a named command with a stable ID. The ribbon, menus
//! and shortcuts all go through this table. A command is enabled only if the
//! app has a handler for it, so unfinished features show dimmed rather than
//! silently doing nothing.

use std::collections::HashSet;

use crate::ribbon::CommandSource;

/// Default shortcuts. Commands without one are omitted.
const SHORTCUTS: &[(&str, &str)] = &[
    ("file.new", "Ctrl+N"),
    ("file.open", "Ctrl+O"),
    ("file.save", "Ctrl+S"),
    ("file.close", "Ctrl+W"),
    ("edit.undo", "Ctrl+Z"),
    ("edit.redo", "Ctrl+Y"),
    ("edit.find", "Ctrl+F"),
    ("clipboard.cut", "Ctrl+X"),
    ("clipboard.copy", "Ctrl+C"),
    ("clipboard.paste", "Ctrl+V"),
    ("region.delete", "Delete"),
    ("insert.math", "Ctrl+M"),
    ("insert.text", "Ctrl+T"),
    ("insert.area", "Ctrl+Shift+A"),
    ("calc.all", "Ctrl+F9"),
    ("view.zoom_in", "Ctrl++"),
    ("view.zoom_out", "Ctrl+-"),
    ("view.zoom_100", "Ctrl+0"),
];

/// Commands the app currently handles.
pub const HANDLED: &[&str] = &[
    "file.new",
    "file.close",
    "insert.math",
    "insert.text",
    "insert.area",
    "insert.plot",
    "insert.table",
    "insert.picture",
    "region.delete",
    "calc.all",
    "units.si",
    "units.uscs",
    "units.cgs",
    "view.zoom_in",
    "view.zoom_out",
    "view.zoom_100",
    "view.zoom_fit",
    "view.grid",
    "view.snap",
    "view.names",
    "view.properties",
    "view.reset_layout",
    "view.theme",
    "help.about",
];

pub struct Registry {
    enabled: HashSet<&'static str>,
    active: HashSet<&'static str>,
}

impl Registry {
    /// `has_sheet`: a worksheet (not the Home page) is showing.
    /// `has_selection`: at least one region is selected.
    /// `active`: toggle commands currently switched on.
    pub fn new(has_sheet: bool, has_selection: bool, active: &[&'static str]) -> Registry {
        let needs_sheet = |c: &str| {
            matches!(c, "file.close" | "calc.all")
                || c.starts_with("insert.")
                || c.starts_with("view.zoom")
        };
        Registry {
            enabled: HANDLED
                .iter()
                .copied()
                .filter(|c| has_sheet || !needs_sheet(c))
                .filter(|c| has_selection || *c != "region.delete")
                .collect(),
            active: active.iter().copied().collect(),
        }
    }
}

impl CommandSource for Registry {
    fn is_enabled(&self, command: &str) -> bool {
        self.enabled.contains(command)
    }

    fn shortcut_text(&self, command: &str) -> Option<&str> {
        SHORTCUTS
            .iter()
            .find(|(c, _)| *c == command)
            .map(|(_, s)| *s)
    }

    fn is_active(&self, command: &str) -> bool {
        self.active.contains(command)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ribbon::defs::opencalc_ribbon;

    #[test]
    fn unknown_commands_are_disabled() {
        let r = Registry::new(true, true, &[]);
        assert!(!r.is_enabled("no.such.command"));
        assert!(r.is_enabled("insert.math"));
    }

    #[test]
    fn sheet_commands_are_disabled_on_the_home_page() {
        let r = Registry::new(false, false, &[]);
        assert!(!r.is_enabled("insert.math"));
        assert!(!r.is_enabled("view.zoom_in"));
        assert!(r.is_enabled("file.new"));
    }

    #[test]
    fn every_handled_command_exists_in_the_ribbon_or_is_app_level() {
        // Guards against typos: a handled ID the ribbon never offers would be dead.
        const APP_LEVEL: &[&str] = &["file.new", "file.close"];
        let ribbon = opencalc_ribbon();
        let mut ids = HashSet::new();
        for p in ribbon.tabs.iter().flat_map(|t| &t.panels) {
            for c in &p.columns {
                match c {
                    crate::ribbon::Column::Large(i) => {
                        ids.insert(i.command);
                    }
                    crate::ribbon::Column::LargeSplit(s) => {
                        ids.extend(s.items.iter().map(|i| i.command))
                    }
                    crate::ribbon::Column::Stack(v) => ids.extend(v.iter().map(|i| i.command)),
                    crate::ribbon::Column::Rows(_) => {}
                }
            }
        }
        for h in HANDLED {
            assert!(
                ids.contains(h) || APP_LEVEL.contains(h),
                "{h} is handled but not offered"
            );
        }
    }

    #[test]
    fn delete_needs_a_selection() {
        assert!(!Registry::new(true, false, &[]).is_enabled("region.delete"));
        assert!(Registry::new(true, true, &[]).is_enabled("region.delete"));
        assert!(!Registry::new(false, true, &[]).is_enabled("insert.plot"));
    }

    #[test]
    fn shortcut_text_is_found() {
        let r = Registry::new(true, true, &[]);
        assert_eq!(r.shortcut_text("edit.undo"), Some("Ctrl+Z"));
        assert_eq!(r.shortcut_text("view.grid"), None);
    }
}
