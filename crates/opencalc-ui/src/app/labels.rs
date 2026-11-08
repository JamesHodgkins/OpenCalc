//! The Labels window: every labelled region of the worksheet, in reading order, to
//! jump to. On a long worksheet a label is a bookmark.

use opencalc_core::document::{RegionId, RegionKind, Worksheet};

use super::dispatch::Effect;
use super::state::AppState;
use crate::commands::CommandId;

/// One labelled region, for the list.
#[derive(Clone, Debug, PartialEq)]
pub struct Labelled {
    pub id: RegionId,
    pub label: String,
    /// What kind of region it is, and the start of what it holds.
    pub about: String,
}

/// How much of a region's content the list shows.
const GLIMPSE: usize = 40;

/// The labelled regions of `doc`: top to bottom, then left to right.
pub fn labelled(doc: &Worksheet) -> Vec<Labelled> {
    let mut found: Vec<_> = doc
        .regions()
        .iter()
        .filter(|r| !r.label.is_empty())
        .collect();
    found.sort_by(|a, b| {
        (a.pos.y, a.pos.x)
            .partial_cmp(&(b.pos.y, b.pos.x))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    found
        .into_iter()
        .map(|r| {
            let content = match &r.kind {
                RegionKind::Math(m) => m.source().to_owned(),
                RegionKind::Text { text } => text.clone(),
                RegionKind::Area { title } => title.clone(),
                _ => String::new(),
            };
            let first_line: String = content
                .lines()
                .next()
                .unwrap_or("")
                .chars()
                .take(GLIMPSE)
                .collect();
            let about = if first_line.is_empty() {
                r.kind.label().to_owned()
            } else {
                format!("{}: {first_line}", r.kind.label())
            };
            Labelled {
                id: r.id,
                label: r.label.clone(),
                about,
            }
        })
        .collect()
}

/// Run a Labels command. `None` if `command` is not one of them.
pub fn dispatch_labels(state: &mut AppState, command: CommandId) -> Option<Vec<Effect>> {
    match command {
        CommandId::ViewLabels => state.labels_open = true,
        CommandId::LabelsClose => state.labels_open = false,
        CommandId::GoToLabel(n) => {
            let viewport = state.viewport.size();
            if let Some(i) = state.active {
                let sheet = &mut state.sheets[i];
                if let Some(target) = labelled(&sheet.doc).get(usize::from(n)) {
                    let id = target.id;
                    sheet.canvas.selection = vec![id];
                    sheet.canvas.editing = None;
                    sheet.canvas.field = None;
                    sheet.reveal(id, viewport);
                }
            }
        }
        _ => return None,
    }
    Some(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencalc_core::document::Point;
    use opencalc_core::edit::Edit;

    #[test]
    fn labelled_regions_are_listed_top_to_bottom_with_a_glimpse_of_each() {
        let mut w = Worksheet::new("t");
        let mut ids = Vec::new();
        for (kind, y) in [
            (RegionKind::math("a := 1"), 300.0),
            (RegionKind::Table, 100.0),
            (
                RegionKind::Text {
                    text: "first line\nsecond".into(),
                },
                200.0,
            ),
            (RegionKind::math("unlabelled := 1"), 50.0),
        ] {
            let r = w.new_region(kind, Point::new(10.0, y));
            ids.push(r.id);
            let at = w.regions().len();
            w.apply(Edit::InsertRegions {
                regions: vec![(at, r)],
            })
            .unwrap();
        }
        for (id, label) in [(ids[0], "Last"), (ids[1], "First"), (ids[2], "Middle")] {
            w.apply(Edit::SetLabel {
                id,
                label: label.into(),
            })
            .unwrap();
        }
        let list = labelled(&w);
        let names: Vec<&str> = list.iter().map(|l| l.label.as_str()).collect();
        assert_eq!(names, ["First", "Middle", "Last"]);
        assert_eq!(list[0].about, "Table");
        assert_eq!(list[1].about, "Text: first line");
        assert_eq!(list[2].about, "Math: a := 1");
    }
}
