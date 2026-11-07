//! Checking a worksheet without a window: evaluate it and list what is wrong.
//!
//! This is what `opencalc-cli check` reports. It uses the same evaluator and the
//! same error positions the app underlines, so the two cannot disagree.

use crate::document::{RegionId, RegionKind, Worksheet};
use crate::eval::{evaluate_all, Warning};
use crate::expr::Expr;
use crate::parser::parse_with_spans;
use crate::registry::UnitRegistry;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// One problem found in a worksheet.
#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
    pub severity: Severity,
    pub region: RegionId,
    /// The region's top-left corner, in document units.
    pub at: (f32, f32),
    pub message: String,
    /// The region's source text, for a math region.
    pub source: Option<String>,
    /// The failing part of `source`, as a range of characters, when known.
    pub range: Option<(usize, usize)>,
}

/// Everything wrong with `ws`, in layout order. Blank math regions are fine.
pub fn check(ws: &Worksheet) -> Vec<Finding> {
    let units = ws.units().unwrap_or_else(|_| UnitRegistry::builtin());
    let regions = ws.math_regions();
    let out = evaluate_all(&regions, &ws.plot_inputs(), &units);
    let place = |id: RegionId| ws.region(id).map_or((0.0, 0.0), |r| (r.pos.x, r.pos.y));
    let mut findings = Vec::new();
    for region in ws.regions() {
        let id = region.id;
        match &region.kind {
            RegionKind::Math(m) if !m.source().trim().is_empty() => {
                let Some(Err(error)) = out.values.get(&id) else {
                    continue;
                };
                let (message, range) = match m.expr() {
                    Expr::Error(e) => (e.message.clone(), Some((e.start, e.end))),
                    _ => {
                        let range = out.error_paths.get(&id).and_then(|path| {
                            let node = parse_with_spans(m.source()).1;
                            node.at_path(path).map(|n| (n.outer.start, n.outer.end))
                        });
                        (error.to_string(), range)
                    }
                };
                findings.push(Finding {
                    severity: Severity::Error,
                    region: id,
                    at: place(id),
                    message,
                    source: Some(m.source().to_owned()),
                    range,
                });
            }
            RegionKind::Plot(_) => {
                if let Some(Err(error)) = out.plots.get(&id) {
                    findings.push(Finding {
                        severity: Severity::Error,
                        region: id,
                        at: place(id),
                        message: format!("plot: {error}"),
                        source: None,
                        range: None,
                    });
                }
            }
            _ => {}
        }
    }
    for warning in &out.warnings {
        match warning {
            Warning::Redefinition { name, regions } => {
                for id in regions {
                    let source = match ws.region(*id).map(|r| &r.kind) {
                        Some(RegionKind::Math(m)) => Some(m.source().to_owned()),
                        _ => None,
                    };
                    findings.push(Finding {
                        severity: Severity::Warning,
                        region: *id,
                        at: place(*id),
                        message: format!("'{name}' is defined more than once; the last one wins"),
                        source,
                        range: None,
                    });
                }
            }
        }
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Point;
    use crate::edit::Edit;

    fn sheet(sources: &[&str]) -> Worksheet {
        let mut w = Worksheet::new("t");
        for (i, s) in sources.iter().enumerate() {
            let r = w.new_region(RegionKind::math(s), Point::new(10.0, i as f32 * 40.0));
            let at = w.regions().len();
            w.apply(Edit::InsertRegions {
                regions: vec![(at, r)],
            })
            .unwrap();
        }
        w
    }

    #[test]
    fn a_clean_worksheet_has_no_findings_and_blank_regions_are_fine() {
        assert!(check(&sheet(&["a := 1 m", "b := a * 2", "", "  "])).is_empty());
    }

    #[test]
    fn an_evaluation_error_names_the_region_and_the_failing_part() {
        let f = check(&sheet(&["ok := 1", "q := 1 m + 2 s"]));
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].severity, Severity::Error);
        assert_eq!(f[0].region, RegionId(2));
        assert_eq!(f[0].at, (10.0, 40.0));
        assert_eq!(
            f[0].range,
            Some((5, 14)),
            "the sum, as the app underlines it"
        );
        assert_eq!(f[0].source.as_deref(), Some("q := 1 m + 2 s"));
    }

    #[test]
    fn a_parse_error_marks_where_the_parser_stopped() {
        let f = check(&sheet(&["x := (1 + "]));
        assert_eq!(f.len(), 1);
        assert!(f[0].range.is_some(), "{f:?}");
    }

    #[test]
    fn undefined_names_and_cycles_are_errors() {
        let f = check(&sheet(&["a := b + 1", "c := d", "d := c"]));
        assert_eq!(f.len(), 3, "{f:?}");
        assert!(f[0].message.contains("undefined name 'b'"));
        assert!(f[1].message.contains("circular"));
    }

    #[test]
    fn a_name_defined_twice_is_a_warning_on_each_definition() {
        let f = check(&sheet(&["a := 1", "a := 2"]));
        assert_eq!(f.len(), 2);
        assert!(f.iter().all(|x| x.severity == Severity::Warning));
        assert_eq!(f.iter().map(|x| x.region.0).collect::<Vec<_>>(), [1, 2]);
    }
}
