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
    "edit.undo",
    "edit.redo",
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

/// What the app is showing, as far as which commands can run.
pub struct CommandContext {
    /// A worksheet (not the Home page) is showing.
    pub has_sheet: bool,
    /// At least one region is selected.
    pub has_selection: bool,
    pub can_undo: bool,
    pub can_redo: bool,
}

impl Registry {
    /// `active`: toggle commands currently switched on.
    pub fn new(ctx: &CommandContext, active: &[&'static str]) -> Registry {
        let allowed = |c: &str| -> bool {
            let needs_sheet = matches!(c, "file.close" | "calc.all")
                || c.starts_with("insert.")
                || c.starts_with("view.zoom")
                || matches!(c, "edit.undo" | "edit.redo" | "region.delete");
            (ctx.has_sheet || !needs_sheet)
                && (ctx.has_selection || c != "region.delete")
                && (ctx.can_undo || c != "edit.undo")
                && (ctx.can_redo || c != "edit.redo")
        };
        Registry {
            enabled: HANDLED.iter().copied().filter(|c| allowed(c)).collect(),
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

    fn ctx(has_sheet: bool, has_selection: bool, can_undo: bool, can_redo: bool) -> CommandContext {
        CommandContext {
            has_sheet,
            has_selection,
            can_undo,
            can_redo,
        }
    }

    #[test]
    fn unknown_commands_are_disabled() {
        let r = Registry::new(&ctx(true, true, true, true), &[]);
        assert!(!r.is_enabled("no.such.command"));
        assert!(r.is_enabled("insert.math"));
    }

    #[test]
    fn sheet_commands_are_disabled_on_the_home_page() {
        let r = Registry::new(&ctx(false, false, false, false), &[]);
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
        assert!(!Registry::new(&ctx(true, false, false, false), &[]).is_enabled("region.delete"));
        assert!(Registry::new(&ctx(true, true, true, true), &[]).is_enabled("region.delete"));
        assert!(!Registry::new(&ctx(false, true, false, false), &[]).is_enabled("insert.plot"));
    }

    #[test]
    fn undo_and_redo_are_enabled_only_when_there_is_something_to_do() {
        let none = Registry::new(&ctx(true, false, false, false), &[]);
        assert!(!none.is_enabled("edit.undo") && !none.is_enabled("edit.redo"));
        let undo_only = Registry::new(&ctx(true, false, true, false), &[]);
        assert!(undo_only.is_enabled("edit.undo") && !undo_only.is_enabled("edit.redo"));
        let both = Registry::new(&ctx(true, false, true, true), &[]);
        assert!(both.is_enabled("edit.undo") && both.is_enabled("edit.redo"));
        // And never on the Home page.
        let home = Registry::new(&ctx(false, false, true, true), &[]);
        assert!(!home.is_enabled("edit.undo"));
    }

    #[test]
    fn shortcut_text_is_found() {
        let r = Registry::new(&ctx(true, true, true, true), &[]);
        assert_eq!(r.shortcut_text("edit.undo"), Some("Ctrl+Z"));
        assert_eq!(r.shortcut_text("view.grid"), None);
    }
}
