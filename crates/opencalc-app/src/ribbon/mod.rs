//! Data-driven ribbon (spec section 4.8).
//!
//! The ribbon is declared as data. Each item names a command ID; enabled
//! state, active state and shortcut text come from a [`CommandSource`], so
//! nothing is defined twice. The ribbon never runs commands itself: it reports
//! the clicked ID and the caller handles it.

mod cells;
mod flyout;
mod show;
pub mod widgets;

pub mod defs;

use std::collections::HashMap;

use crate::commands::CommandId;

pub struct Item {
    pub command: CommandId,
    /// Copied from the command table when the item is built, so a label and icon
    /// are defined once (rule R5).
    pub label: &'static str,
    pub icon: char,
}

impl Item {
    pub fn of(command: CommandId) -> Item {
        Item {
            command,
            label: command.label(),
            icon: command.icon(),
        }
    }
}

pub struct Split {
    /// A fixed label overrides the label of the last pick.
    pub label: Option<&'static str>,
    pub items: Vec<Item>,
}

pub enum Cell {
    /// Small icon only; the label is the tooltip.
    Icon(Item),
    /// A drop-down whose items come from the command source when it opens; the
    /// face reads what is chosen now. `first` is always listed first.
    Menu { menu: Item, first: Item },
}

pub enum Column {
    /// Big icon over a two-line label.
    Large(Item),
    /// The icon runs the last pick; the label opens the list.
    LargeSplit(Split),
    /// Up to three small icon and label rows.
    Stack(Vec<Item>),
    /// Up to three rows of small cells.
    Rows(Vec<Vec<Cell>>),
}

pub struct Panel {
    pub title: &'static str,
    pub columns: Vec<Column>,
    /// Items shown in the flyout opened by the arrow beside the title.
    pub extra: Vec<Item>,
}

pub struct Tab {
    pub title: &'static str,
    pub panels: Vec<Panel>,
}

pub struct Ribbon {
    pub tabs: Vec<Tab>,
}

/// UI state kept beside the ribbon.
#[derive(Default)]
pub struct RibbonState {
    pub active_tab: usize,
    /// (tab, panel) of the open flyout.
    pub open_panel: Option<(usize, usize)>,
    /// Last item picked from each split, by the split's first command.
    pub split_picks: HashMap<CommandId, CommandId>,
}

/// What the ribbon needs from the command registry.
pub trait CommandSource {
    /// Unknown commands are not enabled.
    fn is_enabled(&self, command: CommandId) -> bool;
    fn shortcut_text(&self, command: CommandId) -> Option<String>;
    fn is_active(&self, _command: CommandId) -> bool {
        false
    }
    /// A menu filled at run time: its items, and the text on its face (else its
    /// own label).
    fn menu(&self, _menu: CommandId) -> (Vec<CommandId>, Option<String>) {
        (Vec::new(), None)
    }
}

#[derive(Default)]
pub struct RibbonOutput {
    pub clicked: Option<CommandId>,
    /// How wide the drawn tab is, to test that it fits the narrowest window.
    #[cfg(test)]
    pub width: f32,
}
