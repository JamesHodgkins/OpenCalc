//! Find and replace over a worksheet's text: math sources, text regions, and the
//! fields of plots. (Area titles are not included: there is no edit to change one
//! yet.) Positions are in characters, like the parser's.
//!
//! Replacing builds one [`Edit::Batch`], so "replace all" is a single undo step.

use crate::document::{MathRegion, PlotSpec, Region, RegionId, RegionKind, Worksheet};
use crate::edit::Edit;

/// Longest text searched for or put in, in characters.
pub const MAX_QUERY_CHARS: usize = 256;
pub const MAX_REPLACEMENT_CHARS: usize = 1024;
/// Most matches reported; a search is bounded however big the worksheet is.
pub const MAX_MATCHES: usize = 10_000;
/// A replacement that would make a field longer than this is skipped.
const MAX_FIELD_CHARS: usize = 65_536;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub match_case: bool,
    /// The characters on either side of a match must not be letters, digits or `_`.
    pub whole_word: bool,
}

/// Which text of a region a match is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Math,
    Text,
    PlotTitle,
    PlotXLabel,
    PlotYLabel,
    PlotX,
    PlotSeries(usize),
    /// The region's label (any kind of region has one).
    Label,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Match {
    pub region: RegionId,
    pub field: Field,
    /// Characters from the start of the field, `start..end`.
    pub start: usize,
    pub end: usize,
}

/// The searchable text of a region, in reading order.
fn fields(region: &Region) -> Vec<(Field, &str)> {
    let mut fields = kind_fields(&region.kind);
    fields.push((Field::Label, region.label.as_str()));
    fields
}

fn kind_fields(kind: &RegionKind) -> Vec<(Field, &str)> {
    match kind {
        RegionKind::Math(m) => vec![(Field::Math, m.source())],
        RegionKind::Text { text } => vec![(Field::Text, text.as_str())],
        RegionKind::Plot(p) => {
            let mut v = vec![
                (Field::PlotTitle, p.title.as_str()),
                (Field::PlotXLabel, p.x_label.as_str()),
                (Field::PlotYLabel, p.y_label.as_str()),
                (Field::PlotX, p.x.source()),
            ];
            v.extend(
                p.series
                    .iter()
                    .enumerate()
                    .map(|(i, s)| (Field::PlotSeries(i), s.source())),
            );
            v
        }
        RegionKind::Area { .. } | RegionKind::Table | RegionKind::Picture => Vec::new(),
    }
}

fn same(a: char, b: char, match_case: bool) -> bool {
    a == b || (!match_case && a.to_lowercase().eq(b.to_lowercase()))
}

fn word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Non-overlapping matches of `query` in `text`, left to right.
fn scan(text: &[char], query: &[char], options: Options) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    if query.is_empty() || query.len() > text.len() {
        return found;
    }
    let mut i = 0;
    while i + query.len() <= text.len() {
        let end = i + query.len();
        let hit = text[i..end]
            .iter()
            .zip(query)
            .all(|(a, b)| same(*a, *b, options.match_case));
        let bounded = !options.whole_word
            || (!(i > 0 && word_char(text[i - 1])) && !(end < text.len() && word_char(text[end])));
        if hit && bounded {
            found.push((i, end));
            i = end;
        } else {
            i += 1;
        }
    }
    found
}

/// Every match of `query` in the worksheet, top to bottom then left to right by
/// region position, and in reading order within a region.
pub fn find_all(ws: &Worksheet, query: &str, options: Options) -> Vec<Match> {
    let query: Vec<char> = query.chars().take(MAX_QUERY_CHARS).collect();
    if query.is_empty() {
        return Vec::new();
    }
    let mut regions: Vec<_> = ws.regions().iter().collect();
    regions.sort_by(|a, b| {
        (a.pos.y, a.pos.x, a.id)
            .partial_cmp(&(b.pos.y, b.pos.x, b.id))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut out = Vec::new();
    for region in regions {
        for (field, text) in fields(region) {
            let chars: Vec<char> = text.chars().collect();
            for (start, end) in scan(&chars, &query, options) {
                out.push(Match {
                    region: region.id,
                    field,
                    start,
                    end,
                });
                if out.len() >= MAX_MATCHES {
                    return out;
                }
            }
        }
    }
    out
}

/// `text` with the given character ranges (ascending, not overlapping) replaced.
fn splice(text: &str, ranges: &[(usize, usize)], replacement: &str) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for &(start, end) in ranges {
        if start < at || end < start || end > chars.len() {
            return None; // stale: the text changed since the search
        }
        out.extend(&chars[at..start]);
        out.push_str(replacement);
        at = end;
    }
    out.extend(&chars[at..]);
    (out.chars().count() <= MAX_FIELD_CHARS).then_some(out)
}

/// The edit that replaces `matches` with `replacement`: `None` if there is nothing
/// to do. A field whose text no longer fits its match, or would grow too long, is
/// left out.
pub fn replace_edit(ws: &Worksheet, matches: &[Match], replacement: &str) -> Option<Edit> {
    let replacement: String = replacement.chars().take(MAX_REPLACEMENT_CHARS).collect();
    let mut ids: Vec<RegionId> = Vec::new();
    for m in matches {
        if !ids.contains(&m.region) {
            ids.push(m.region);
        }
    }
    let mut edits = Vec::new();
    for id in ids {
        let region = ws.region(id)?;
        if region.locked {
            continue;
        }
        let new_text = |field: Field, current: &str| -> Option<String> {
            let mut ranges: Vec<(usize, usize)> = matches
                .iter()
                .filter(|m| m.region == id && m.field == field)
                .map(|m| (m.start, m.end))
                .collect();
            if ranges.is_empty() {
                return None;
            }
            ranges.sort();
            splice(current, &ranges, &replacement)
        };
        match &region.kind {
            RegionKind::Math(m) => {
                if let Some(source) = new_text(Field::Math, m.source()) {
                    edits.push(Edit::SetMathSource { id, source });
                }
            }
            RegionKind::Text { text } => {
                if let Some(text) = new_text(Field::Text, text) {
                    edits.push(Edit::SetText { id, text });
                }
            }
            RegionKind::Plot(p) => {
                if let Some(spec) = replace_in_plot(p, &new_text) {
                    edits.push(Edit::SetPlot { id, spec });
                }
            }
            RegionKind::Area { .. } | RegionKind::Table | RegionKind::Picture => {}
        }
        if let Some(label) = new_text(Field::Label, &region.label) {
            edits.push(Edit::SetLabel { id, label });
        }
    }
    match edits.len() {
        0 => None,
        1 => edits.pop(),
        _ => Some(Edit::Batch(edits)),
    }
}

fn replace_in_plot(
    p: &PlotSpec,
    new_text: &dyn Fn(Field, &str) -> Option<String>,
) -> Option<PlotSpec> {
    let mut spec = p.clone();
    let mut changed = false;
    if let Some(t) = new_text(Field::PlotTitle, &p.title) {
        spec.title = t;
        changed = true;
    }
    if let Some(t) = new_text(Field::PlotXLabel, &p.x_label) {
        spec.x_label = t;
        changed = true;
    }
    if let Some(t) = new_text(Field::PlotYLabel, &p.y_label) {
        spec.y_label = t;
        changed = true;
    }
    if let Some(t) = new_text(Field::PlotX, p.x.source()) {
        spec.x = MathRegion::new(&t);
        changed = true;
    }
    for (i, s) in p.series.iter().enumerate() {
        if let Some(t) = new_text(Field::PlotSeries(i), s.source()) {
            spec.series[i] = MathRegion::new(&t);
            changed = true;
        }
    }
    changed.then_some(spec)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Point;

    fn sheet(items: &[(RegionKind, f32, f32)]) -> Worksheet {
        let mut w = Worksheet::new("t");
        for (kind, x, y) in items {
            let r = w.new_region(kind.clone(), Point::new(*x, *y));
            let at = w.regions().len();
            w.apply(Edit::InsertRegions {
                regions: vec![(at, r)],
            })
            .unwrap();
        }
        w
    }

    fn math(s: &str) -> RegionKind {
        RegionKind::math(s)
    }

    fn all(w: &Worksheet, q: &str) -> Vec<Match> {
        find_all(w, q, Options::default())
    }

    #[test]
    fn labels_are_searched_and_replaced_like_other_text() {
        let mut w = sheet(&[
            (math("a := 1"), 0.0, 0.0),
            (RegionKind::Table, 0.0, 50.0),
            (math("b := 2"), 0.0, 100.0),
        ]);
        let ids: Vec<RegionId> = w.regions().iter().map(|r| r.id).collect();
        for (id, label) in [(ids[0], "Beam check"), (ids[1], "Beam table")] {
            w.apply(Edit::SetLabel {
                id,
                label: label.into(),
            })
            .unwrap();
        }
        // A label on any kind of region is found, in the region's own place.
        let found = all(&w, "beam");
        assert_eq!(
            found
                .iter()
                .map(|m| (m.region, m.field))
                .collect::<Vec<_>>(),
            [(ids[0], Field::Label), (ids[1], Field::Label)]
        );
        let edit = replace_edit(&w, &found, "Column").unwrap();
        w.apply(edit).unwrap();
        assert_eq!(w.region(ids[0]).unwrap().label, "Column check");
        assert_eq!(w.region(ids[1]).unwrap().label, "Column table");
        assert!(all(&w, "beam").is_empty());
    }

    #[test]
    fn finds_every_occurrence_in_reading_order_by_position() {
        // Created bottom first, but found top first.
        let w = sheet(&[
            (math("b := a + a"), 0.0, 100.0),
            (
                RegionKind::Text {
                    text: "the a value".into(),
                },
                0.0,
                0.0,
            ),
        ]);
        let found: Vec<_> = all(&w, "a")
            .iter()
            .map(|m| (m.region.0, m.field, m.start, m.end))
            .collect();
        assert_eq!(
            found,
            [
                (2, Field::Text, 4, 5),
                (2, Field::Text, 7, 8),
                (1, Field::Math, 5, 6),
                (1, Field::Math, 9, 10),
            ]
        );
    }

    #[test]
    fn case_and_whole_word_options() {
        let w = sheet(&[(math("Area := area + area_2 + AREA"), 0.0, 0.0)]);
        assert_eq!(all(&w, "area").len(), 4, "case is ignored by default");
        let case = Options {
            match_case: true,
            ..Options::default()
        };
        assert_eq!(find_all(&w, "area", case).len(), 2);
        let word = Options {
            whole_word: true,
            ..Options::default()
        };
        let m = find_all(&w, "area", word);
        assert_eq!(
            m.iter().map(|m| m.start).collect::<Vec<_>>(),
            [0, 8, 24],
            "not inside area_2"
        );
    }

    #[test]
    fn matches_do_not_overlap_and_empty_or_missing_queries_find_nothing() {
        let w = sheet(&[(math("aaaa"), 0.0, 0.0)]);
        assert_eq!(all(&w, "aa").len(), 2);
        assert!(all(&w, "").is_empty());
        assert!(all(&w, "zzz").is_empty());
        assert!(all(&Worksheet::new("empty"), "a").is_empty());
    }

    #[test]
    fn positions_are_in_characters_not_bytes() {
        let w = sheet(&[(math("\u{3c3} := \u{3c3} + 1"), 0.0, 0.0)]);
        let m = all(&w, "\u{3c3}");
        assert_eq!((m[0].start, m[1].start), (0, 5));
        // Upper and lower case Greek match when case is ignored.
        assert_eq!(all(&w, "\u{3a3}").len(), 2);
    }

    #[test]
    fn plot_fields_are_searched_and_areas_are_not() {
        let plot = PlotSpec::default()
            .with_title("Stress w")
            .with_x("w")
            .with_series(0, "F / w ^ 2")
            .with_labels("width w", "");
        let w = sheet(&[
            (RegionKind::Plot(plot), 0.0, 0.0),
            (
                RegionKind::Area {
                    title: "w area".into(),
                },
                0.0,
                300.0,
            ),
        ]);
        let fields: Vec<Field> = all(&w, "w").iter().map(|m| m.field).collect();
        assert_eq!(
            fields,
            [
                Field::PlotTitle,
                Field::PlotXLabel, // "width": its first letter
                Field::PlotXLabel, // " w" at the end
                Field::PlotX,
                Field::PlotSeries(0),
            ]
        );
        assert!(all(&w, "area").is_empty());
    }

    #[test]
    fn replacing_all_is_one_batch_that_undoes_exactly() {
        let mut w = sheet(&[
            (math("b := a + a"), 0.0, 100.0),
            (
                RegionKind::Text {
                    text: "the a value".into(),
                },
                0.0,
                0.0,
            ),
        ]);
        let before = w.clone();
        let matches = all(&w, "a");
        let edit = replace_edit(&w, &matches, "x").unwrap();
        assert!(matches!(edit, Edit::Batch(_)));
        let applied = w.apply(edit).unwrap();
        assert!(applied.changed);
        let sources: Vec<String> = w
            .regions()
            .iter()
            .map(|r| match &r.kind {
                RegionKind::Math(m) => m.source().to_owned(),
                RegionKind::Text { text } => text.clone(),
                _ => String::new(),
            })
            .collect();
        assert_eq!(sources, ["b := x + x", "the x vxlue"]);
        assert!(all(&w, "a").is_empty());
        w.apply(applied.inverse).unwrap();
        assert_eq!(w, before);
    }

    #[test]
    fn replacing_one_match_leaves_the_others() {
        let mut w = sheet(&[(math("a + a + a"), 0.0, 0.0)]);
        let matches = all(&w, "a");
        let edit = replace_edit(&w, &matches[1..2], "bb").unwrap();
        w.apply(edit).unwrap();
        match &w.regions()[0].kind {
            RegionKind::Math(m) => assert_eq!(m.source(), "a + bb + a"),
            _ => unreachable!(),
        }
    }

    #[test]
    fn replacing_inside_a_plot_changes_only_the_matching_fields() {
        let plot = PlotSpec::default()
            .with_title("w")
            .with_x("w")
            .with_series(0, "w^2");
        let mut w = sheet(&[(RegionKind::Plot(plot), 0.0, 0.0)]);
        let edit = replace_edit(&w, &all(&w, "w"), "width").unwrap();
        w.apply(edit).unwrap();
        match &w.regions()[0].kind {
            RegionKind::Plot(p) => {
                assert_eq!(p.title, "width");
                assert_eq!(p.x.source(), "width");
                assert_eq!(p.series[0].source(), "width^2");
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn stale_matches_and_oversized_results_are_skipped_not_applied() {
        let mut w = sheet(&[(math("a + a"), 0.0, 0.0)]);
        let matches = all(&w, "a");
        // The text changes after the search.
        w.apply(Edit::SetMathSource {
            id: RegionId(1),
            source: "a".into(),
        })
        .unwrap();
        assert!(replace_edit(&w, &matches, "x").is_none());
        // A replacement that would push the field past its limit is left out.
        let w = sheet(&[(math(&"a".repeat(MAX_FIELD_CHARS)), 0.0, 0.0)]);
        assert!(replace_edit(&w, &all(&w, "a"), "bb").is_none());
        // Nothing to replace is no edit.
        assert!(replace_edit(&w, &[], "x").is_none());
    }

    #[test]
    fn a_replacement_that_changes_nothing_is_not_a_change() {
        let mut w = sheet(&[(math("a + a"), 0.0, 0.0), (math("a"), 0.0, 50.0)]);
        let edit = replace_edit(&w, &all(&w, "a"), "a").unwrap();
        let revision = w.revision();
        assert!(!w.apply(edit).unwrap().changed);
        assert_eq!(w.revision(), revision);
    }

    #[test]
    fn a_failed_batch_changes_nothing() {
        let mut w = sheet(&[(math("a"), 0.0, 0.0)]);
        let before = w.clone();
        let batch = Edit::Batch(vec![
            Edit::SetMathSource {
                id: RegionId(1),
                source: "changed".into(),
            },
            Edit::SetMathSource {
                id: RegionId(99),
                source: "nobody".into(),
            },
        ]);
        assert!(w.apply(batch).is_err());
        assert_eq!(w, before);
    }
}
