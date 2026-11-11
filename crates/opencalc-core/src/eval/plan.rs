//! Working out what a worksheet's regions depend on before anything is evaluated:
//! who defines each name, the dependency edges, which regions are circular and
//! which functions can be called.

use std::collections::HashMap;

use super::{Fns, UserFn, Warning};
use crate::document::RegionId;
use crate::expr::{AssignKind, Expr};

/// For each name, the regions that define it, in layout order. Variables and
/// functions are separate namespaces.
pub(super) struct Definers<'a> {
    pub vars: HashMap<&'a str, Vec<RegionId>>,
    pub fns: HashMap<&'a str, Vec<RegionId>>,
}

impl<'a> Definers<'a> {
    pub fn of(regions: &'a [(RegionId, Expr)]) -> Self {
        let mut vars: HashMap<&str, Vec<RegionId>> = HashMap::new();
        let mut fns: HashMap<&str, Vec<RegionId>> = HashMap::new();
        for (id, expr) in regions {
            match expr {
                Expr::Assign {
                    kind: AssignKind::Define,
                    target,
                    ..
                } => vars.entry(target.as_str()).or_default().push(*id),
                Expr::MultiDefine { targets, .. } => {
                    for t in targets.iter() {
                        vars.entry(t.as_str()).or_default().push(*id);
                    }
                }
                Expr::FunctionDef { name, .. } => fns.entry(&**name).or_default().push(*id),
                _ => {}
            }
        }
        Definers { vars, fns }
    }

    /// A warning for every name defined more than once, names in order.
    pub fn redefinitions(&self) -> Vec<Warning> {
        let mut warnings = Vec::new();
        for definers in [&self.vars, &self.fns] {
            let mut names: Vec<&&str> = definers.keys().collect();
            names.sort();
            for name in names {
                let ids = &definers[*name];
                if ids.len() > 1 {
                    warnings.push(Warning::Redefinition {
                        name: (**name).to_owned(),
                        regions: ids.clone(),
                    });
                }
            }
        }
        warnings
    }

    pub fn defines(&self, name: &str) -> bool {
        self.vars.contains_key(name) || self.fns.contains_key(name)
    }

    /// For each name, the region whose definition wins: the last in layout order.
    pub fn winners(&self) -> (HashMap<String, RegionId>, HashMap<String, RegionId>) {
        let last = |definers: &HashMap<&str, Vec<RegionId>>| -> HashMap<String, RegionId> {
            definers
                .iter()
                .filter_map(|(n, ids)| ids.last().map(|id| ((*n).to_owned(), *id)))
                .collect()
        };
        (last(&self.vars), last(&self.fns))
    }
}

/// Edges: region index -> indices of the regions it depends on: the definers of the
/// variables it reads and of the functions it calls.
pub(super) fn dependency_edges(
    regions: &[(RegionId, Expr)],
    var_winner: &HashMap<String, RegionId>,
    fn_winner: &HashMap<String, RegionId>,
) -> Vec<Vec<usize>> {
    let index_of: HashMap<RegionId, usize> = regions
        .iter()
        .enumerate()
        .map(|(i, (id, _))| (*id, i))
        .collect();
    regions
        .iter()
        .map(|(_, e)| {
            let variables = e.free_names();
            let functions = e.called_names();
            let reads = variables.iter().filter_map(|n| var_winner.get(n));
            let calls = functions.iter().filter_map(|n| fn_winner.get(n));
            reads.chain(calls).map(|id| index_of[id]).collect()
        })
        .collect()
}

/// Which regions are circular. A cycle made only of function definitions is
/// recursion, which is allowed: a definition's body runs when the function is
/// called, not when it is defined, so nothing has to be evaluated before anything
/// else. A cycle that includes a variable (`x := f(1)` with `f` reading `x`) is
/// still circular.
pub(super) fn cycle_flags(
    regions: &[(RegionId, Expr)],
    edges: &[Vec<usize>],
    components: &[Vec<usize>],
) -> Vec<bool> {
    let only_functions = |component: &[usize]| {
        component
            .iter()
            .all(|&i| matches!(regions[i].1, Expr::FunctionDef { .. }))
    };
    let circular = |i: usize, component: &[usize]| {
        (component.len() > 1 || edges[i].contains(&i)) && !only_functions(component)
    };
    let mut flags = vec![false; regions.len()];
    for component in components {
        for &i in component {
            flags[i] = circular(i, component);
        }
    }
    flags
}

/// Winning definitions of functions, ready to be called.
pub(super) fn winning_fns<'a>(
    regions: &'a [(RegionId, Expr)],
    fn_winner: &HashMap<String, RegionId>,
    in_cycle: &[bool],
) -> Fns<'a> {
    regions
        .iter()
        .enumerate()
        .filter_map(|(i, (id, e))| match e {
            Expr::FunctionDef {
                name,
                params,
                defaults,
                body,
            } if fn_winner.get(&**name) == Some(id) => {
                let circular = in_cycle[i];
                Some((
                    &**name,
                    UserFn {
                        params,
                        defaults,
                        body,
                        circular,
                    },
                ))
            }
            _ => None,
        })
        .collect()
}
