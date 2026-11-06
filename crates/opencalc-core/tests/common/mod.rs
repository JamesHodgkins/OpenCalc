//! Shared by the property tests and the benchmark: a deterministic generator of
//! realistic worksheets. Every line is dimensionally valid by construction, so a
//! generated worksheet evaluates without errors and measures real work.

// Each consumer uses a different part of this module.
#![allow(dead_code)]

use opencalc_core::eval::RegionId;
use opencalc_core::expr::Expr;
use opencalc_core::parser::parse;

/// A small deterministic random number generator (an LCG), so failures and
/// benchmark numbers are reproducible without an extra dependency.
pub struct Rng(pub u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407))
    }

    /// A number in `0..n` (`n` must be at least 1).
    pub fn below(&mut self, n: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 33) as usize) % n.max(1)
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

/// Kinds of quantity the generator tracks, so it only combines them validly.
#[derive(Clone, Copy)]
enum Class {
    Length,
    Force,
    Area,
    Pressure,
    Ratio,
}

/// `n` worksheet lines in dependency order: five seed constants (one of each
/// class), then expressions built from earlier names. Names are `L`, `F`, `A`, `P`
/// or `R` followed by the line number, so a line never defines a unit symbol.
pub fn worksheet_lines(n: usize, seed: u64) -> Vec<String> {
    let mut rng = Rng::new(seed);
    let mut names: [Vec<String>; 5] = Default::default();
    let mut lines = Vec::with_capacity(n);
    let seeds = [
        (Class::Length, "L", "3.2 m"),
        (Class::Force, "F", "12.5 kN"),
        (Class::Area, "A", "0.04 m^2"),
        (Class::Pressure, "P", "250 kPa"),
        (Class::Ratio, "R", "0.75"),
    ];
    for i in 0..n {
        let (class, prefix, rhs) = if i < seeds.len() {
            let (c, p, r) = seeds[i];
            (c, p, r.to_owned())
        } else {
            let pick = |rng: &mut Rng, c: Class| rng.pick(&names[c as usize]).clone();
            match rng.below(10) {
                0 => (
                    Class::Length,
                    "L",
                    format!(
                        "{} + {}",
                        pick(&mut rng, Class::Length),
                        pick(&mut rng, Class::Length)
                    ),
                ),
                1 => (
                    Class::Length,
                    "L",
                    format!("{} * 1.5", pick(&mut rng, Class::Length)),
                ),
                2 => (
                    Class::Length,
                    "L",
                    format!("sqrt({})", pick(&mut rng, Class::Area)),
                ),
                3 => (
                    Class::Area,
                    "A",
                    format!(
                        "{} * {}",
                        pick(&mut rng, Class::Length),
                        pick(&mut rng, Class::Length)
                    ),
                ),
                4 => (
                    Class::Area,
                    "A",
                    format!("{} ^ 2", pick(&mut rng, Class::Length)),
                ),
                5 => (
                    Class::Pressure,
                    "P",
                    format!(
                        "{} / {}",
                        pick(&mut rng, Class::Force),
                        pick(&mut rng, Class::Area)
                    ),
                ),
                6 => (
                    Class::Force,
                    "F",
                    format!(
                        "{} * {}",
                        pick(&mut rng, Class::Pressure),
                        pick(&mut rng, Class::Area)
                    ),
                ),
                7 => (
                    Class::Ratio,
                    "R",
                    format!(
                        "{} / {}",
                        pick(&mut rng, Class::Pressure),
                        pick(&mut rng, Class::Pressure)
                    ),
                ),
                8 => (
                    Class::Ratio,
                    "R",
                    format!("sin({} * 0.1) + 1", pick(&mut rng, Class::Ratio)),
                ),
                _ => (
                    Class::Ratio,
                    "R",
                    format!(
                        "{} / {}",
                        pick(&mut rng, Class::Length),
                        pick(&mut rng, Class::Length)
                    ),
                ),
            }
        };
        let name = format!("{prefix}{i}");
        lines.push(format!("{name} := {rhs}"));
        names[class as usize].push(name);
    }
    lines
}

/// Fisher-Yates: a permutation of `0..n`.
pub fn shuffled(n: usize, seed: u64) -> Vec<usize> {
    let mut rng = Rng::new(seed);
    let mut order: Vec<usize> = (0..n).collect();
    for i in (1..n).rev() {
        order.swap(i, rng.below(i + 1));
    }
    order
}

/// Parse lines into regions with ids `1..=n`, laid out in the given order. The id
/// follows the source line, not its position, so results can be compared across
/// layouts.
pub fn regions(lines: &[String], order: &[usize]) -> Vec<(RegionId, Expr)> {
    order
        .iter()
        .map(|&i| (RegionId(i as u64 + 1), parse(&lines[i])))
        .collect()
}

pub fn identity(n: usize) -> Vec<usize> {
    (0..n).collect()
}
