//! Axis ranges, the mapping from a value to a position along an axis, and tick
//! marks, for linear and logarithmic axes. Pure arithmetic, no painting.

/// Tick positions for an axis from `min` to `max`: about `target` of them, at
/// multiples of 1, 2 or 5 times a power of ten, all inside the range.
pub fn nice_ticks(min: f64, max: f64, target: usize) -> (Vec<f64>, f64) {
    let span = max - min;
    if !(span.is_finite() && span > 0.0) || target == 0 {
        return (Vec::new(), 0.0);
    }
    let rough = span / target as f64;
    let power = 10f64.powf(rough.log10().floor());
    let step = [1.0, 2.0, 5.0, 10.0]
        .iter()
        .map(|m| m * power)
        .find(|s| *s >= rough)
        .unwrap_or(10.0 * power);
    // A little slack, so an end that is itself a round number is not lost to
    // rounding (3e-9 / 5e-10 is 6.000000000000001).
    let first = (min / step - 1e-9).ceil() as i64;
    let last = (max / step + 1e-9).floor() as i64;
    let ticks = (first..=last).map(|k| k as f64 * step).collect();
    (ticks, step)
}

/// A tick value with just enough decimals for the spacing, and a clean zero.
pub fn tick_text(value: f64, step: f64) -> String {
    if value.abs() < step * 1e-9 {
        return "0".to_owned();
    }
    // The fewest decimals that show the step exactly (0.25 needs two).
    let decimals = (0..=8)
        .find(|d| {
            let scaled = step * 10f64.powi(*d);
            (scaled - scaled.round()).abs() < 1e-6 * scaled.abs().max(1.0)
        })
        .unwrap_or(8) as usize;
    format!("{value:.decimals$}")
}

/// The range an axis should cover: the data's, padded, and never empty.
pub fn axis_range(lo: f64, hi: f64, pad: f64) -> (f64, f64) {
    if hi > lo {
        let margin = (hi - lo) * pad;
        return (lo - margin, hi + margin);
    }
    let around = if lo == 0.0 { 1.0 } else { lo.abs() * 0.1 };
    (lo - around, lo + around)
}

/// One axis: the values it covers (in the unit it is drawn in) and whether the
/// scale is logarithmic. A log axis always has `0 < lo < hi`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Axis {
    pub lo: f64,
    pub hi: f64,
    pub log: bool,
}

impl Axis {
    /// The axis for data spanning `data_lo..data_hi`, padded by the fraction `pad`
    /// of its length, with either end replaced by a given limit.
    pub fn new(
        data_lo: f64,
        data_hi: f64,
        pad: f64,
        log: bool,
        limit_lo: Option<f64>,
        limit_hi: Option<f64>,
    ) -> Axis {
        let (mut lo, mut hi) = if log {
            let (a, b) = (
                data_lo.max(f64::MIN_POSITIVE),
                data_hi.max(f64::MIN_POSITIVE),
            );
            if b > a {
                // Padding in the log domain, so it looks the same as on a linear axis.
                let margin = (b / a).powf(pad);
                (a / margin, b * margin)
            } else {
                (a / 10.0, b * 10.0)
            }
        } else {
            axis_range(data_lo, data_hi, pad)
        };
        if let Some(l) = limit_lo {
            lo = l;
        }
        if let Some(h) = limit_hi {
            hi = h;
        }
        let ordered = hi > lo && hi.is_finite() && lo.is_finite();
        if !ordered || (log && lo <= 0.0) {
            // Limits that cross, or are absurd: fall back to a range around what is left.
            return if log {
                Axis::new(hi.max(1e-300), hi.max(1e-300), 0.0, true, None, None)
            } else {
                let (lo, hi) = axis_range(lo, lo, 0.0);
                Axis { lo, hi, log }
            };
        }
        Axis { lo, hi, log }
    }

    /// Where `v` falls along the axis: 0 at `lo`, 1 at `hi`.
    pub fn fraction(&self, v: f64) -> f64 {
        if self.log {
            (v.ln() - self.lo.ln()) / (self.hi.ln() - self.lo.ln())
        } else {
            (v - self.lo) / (self.hi - self.lo)
        }
    }

    /// Tick values (about `target` of them) and, for a linear axis, their spacing.
    /// On a log axis the ticks are powers of ten (every few decades if there are
    /// many); an axis covering less than a decade falls back to round values.
    pub fn ticks(&self, target: usize) -> Ticks {
        if !self.log {
            let (values, step) = nice_ticks(self.lo, self.hi, target);
            return Ticks {
                values,
                step,
                log: false,
            };
        }
        let first = self.lo.log10().ceil() as i32;
        let last = self.hi.log10().floor() as i32;
        if last - first < 1 {
            // Less than a decade: round values, spaced on the log scale.
            let (values, step) = nice_ticks(self.lo, self.hi, target);
            return Ticks {
                values,
                step,
                log: true,
            };
        }
        let decades = (last - first + 1) as usize;
        let every = decades.div_ceil(target.max(2)).max(1) as i32;
        let values = (first..=last)
            .filter(|k| (k - first) % every == 0)
            .map(|k| 10f64.powi(k))
            .collect();
        Ticks {
            values,
            step: 0.0,
            log: true,
        }
    }
}

/// Tick values and how to write them.
#[derive(Clone, Debug, PartialEq)]
pub struct Ticks {
    pub values: Vec<f64>,
    /// Spacing of linear ticks; 0 for power-of-ten ticks.
    pub step: f64,
    pub log: bool,
}

impl Ticks {
    pub fn label(&self, v: f64) -> String {
        if self.log && self.step == 0.0 {
            power_text(v)
        } else {
            tick_text(v, self.step)
        }
    }
}

/// A power of ten as plain digits when readable (`0.01`, `100`), else `1e7`.
pub fn power_text(v: f64) -> String {
    if (1e-3..1e6).contains(&v) {
        let decimals = if v >= 1.0 {
            0
        } else {
            (-v.log10().round()) as usize
        };
        format!("{v:.decimals$}")
    } else {
        format!("1e{}", v.log10().round() as i32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks_are_round_numbers_inside_the_range() {
        let (ticks, step) = nice_ticks(0.0, 10.0, 5);
        assert_eq!(step, 2.0);
        assert_eq!(ticks, [0.0, 2.0, 4.0, 6.0, 8.0, 10.0]);
        let (ticks, step) = nice_ticks(-0.3, 0.7, 5);
        assert!((step - 0.2).abs() < 1e-12, "{step}");
        assert!(ticks.iter().all(|t| *t >= -0.3 - 1e-9 && *t <= 0.7 + 1e-9));
        assert!(ticks.contains(&0.0));
    }

    #[test]
    fn ticks_work_at_any_scale_and_never_run_away() {
        for (lo, hi) in [
            (1e-9, 3e-9),
            (1e9, 5e9),
            (-1e300, 1e300),
            (123.4, 123.9),
            (0.0, 1e-300),
        ] {
            let (ticks, step) = nice_ticks(lo, hi, 6);
            assert!(step > 0.0, "{lo}..{hi}");
            assert!(ticks.len() <= 20, "{lo}..{hi}: {} ticks", ticks.len());
            assert!(ticks.windows(2).all(|w| w[1] > w[0]));
            let slack = (hi - lo) * 1e-8;
            assert!(ticks.iter().all(|t| *t >= lo - slack && *t <= hi + slack));
        }
    }

    #[test]
    fn an_empty_or_invalid_range_has_no_ticks() {
        assert!(nice_ticks(1.0, 1.0, 5).0.is_empty());
        assert!(nice_ticks(2.0, 1.0, 5).0.is_empty());
        assert!(nice_ticks(0.0, f64::NAN, 5).0.is_empty());
        assert!(nice_ticks(0.0, 1.0, 0).0.is_empty());
    }

    #[test]
    fn tick_labels_have_just_enough_decimals() {
        assert_eq!(tick_text(4.0, 2.0), "4");
        assert_eq!(tick_text(0.25, 0.25), "0.25");
        assert_eq!(tick_text(0.5, 0.1), "0.5");
        assert_eq!(tick_text(1200.0, 200.0), "1200");
        // Rounding noise around zero reads as zero.
        assert_eq!(tick_text(1e-17, 0.5), "0");
    }

    #[test]
    fn a_flat_series_still_gets_a_range_to_draw_in() {
        let (lo, hi) = axis_range(5.0, 5.0, 0.05);
        assert!(lo < 5.0 && hi > 5.0);
        let (lo, hi) = axis_range(0.0, 0.0, 0.05);
        assert!(lo < 0.0 && hi > 0.0);
        let (lo, hi) = axis_range(0.0, 10.0, 0.1);
        assert_eq!((lo, hi), (-1.0, 11.0));
    }

    #[test]
    fn a_linear_axis_pads_and_maps_ends_to_zero_and_one() {
        let a = Axis::new(0.0, 10.0, 0.1, false, None, None);
        assert_eq!((a.lo, a.hi), (-1.0, 11.0));
        assert!((a.fraction(-1.0)).abs() < 1e-12 && (a.fraction(11.0) - 1.0).abs() < 1e-12);
        assert!((a.fraction(5.0) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn limits_replace_the_ends_exactly_without_padding() {
        let a = Axis::new(0.0, 10.0, 0.1, false, Some(2.0), Some(8.0));
        assert_eq!((a.lo, a.hi), (2.0, 8.0));
        let a = Axis::new(0.0, 10.0, 0.0, false, Some(4.0), None);
        assert_eq!((a.lo, a.hi), (4.0, 10.0));
    }

    #[test]
    fn crossed_limits_still_give_a_drawable_axis() {
        let a = Axis::new(0.0, 10.0, 0.0, false, Some(5.0), Some(5.0));
        assert!(a.hi > a.lo);
        let a = Axis::new(1.0, 10.0, 0.0, true, Some(50.0), Some(20.0));
        assert!(a.hi > a.lo && a.lo > 0.0);
    }

    #[test]
    fn a_log_axis_maps_equal_ratios_to_equal_distances() {
        let a = Axis::new(1.0, 1000.0, 0.0, true, None, None);
        assert_eq!((a.lo, a.hi), (1.0, 1000.0));
        assert!((a.fraction(10.0) - 1.0 / 3.0).abs() < 1e-12);
        assert!((a.fraction(100.0) - 2.0 / 3.0).abs() < 1e-12);
        // One value only: a decade either side.
        let a = Axis::new(5.0, 5.0, 0.0, true, None, None);
        assert!(a.lo < 5.0 && a.hi > 5.0 && a.lo > 0.0);
    }

    #[test]
    fn log_ticks_are_powers_of_ten() {
        let a = Axis::new(1.0, 1e4, 0.0, true, None, None);
        let t = a.ticks(6);
        assert_eq!(t.values, [1.0, 10.0, 100.0, 1000.0, 1e4]);
        assert_eq!(t.label(100.0), "100");
        assert_eq!(t.label(1e4), "10000");
        let a = Axis::new(0.001, 1.0, 0.0, true, None, None);
        assert_eq!(a.ticks(6).label(0.01), "0.01");
    }

    #[test]
    fn many_decades_are_thinned_and_few_fall_back_to_round_values() {
        let a = Axis::new(1e-9, 1e9, 0.0, true, None, None);
        let t = a.ticks(6);
        assert!(t.values.len() <= 7 && t.values.len() >= 2, "{:?}", t.values);
        assert_eq!(t.label(1e9), "1e9");
        assert_eq!(t.label(1e-9), "1e-9");
        // Under a decade: ordinary round ticks (still placed on the log scale).
        let a = Axis::new(2.0, 8.0, 0.0, true, None, None);
        let t = a.ticks(6);
        assert!(!t.values.is_empty());
        assert!(t
            .values
            .iter()
            .all(|v| *v >= 2.0 - 1e-9 && *v <= 8.0 + 1e-9));
    }

    #[test]
    fn power_labels_read_naturally() {
        assert_eq!(power_text(1.0), "1");
        assert_eq!(power_text(1000.0), "1000");
        assert_eq!(power_text(0.1), "0.1");
        assert_eq!(power_text(0.001), "0.001");
        assert_eq!(power_text(1e6), "1e6");
        assert_eq!(power_text(1e-4), "1e-4");
    }
}
