//! `b = 5 m` with `b` defined nowhere: it only calculates the right side, and does
//! not define `b`. That is almost always `:=` mistyped, so it is flagged.

use super::{RegionId, Warning};
use crate::constants::constant;
use crate::expr::{AssignKind, Expr};
use crate::registry::UnitRegistry;

/// A warning for every `name = value` region whose name nothing defines (not a
/// region, a unit or a constant). `defined` holds the variable and function names.
pub(super) fn undefined_evaluations<'a>(
    regions: impl Iterator<Item = (RegionId, &'a Expr)>,
    defined: &dyn Fn(&str) -> bool,
    units: &UnitRegistry,
) -> Vec<Warning> {
    regions
        .filter_map(|(id, expr)| {
            let Expr::Assign {
                kind: AssignKind::Evaluate,
                target,
                value,
            } = expr
            else {
                return None;
            };
            // `x =` shows x, and `2 + 3 =` has no name: neither is a mistake.
            let echoes = matches!(&**value, Expr::Name(n) if n == target);
            if target.is_empty() || echoes {
                return None;
            }
            let known = defined(target)
                || units.quantity(1.0, target).is_ok()
                || constant(target).is_some();
            (!known).then(|| Warning::UndefinedEvaluation {
                name: target.clone(),
                region: id,
            })
        })
        .collect()
}
