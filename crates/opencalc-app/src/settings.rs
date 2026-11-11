//! Application settings: what the Options window changes, kept between sessions.
//!
//! These belong to this computer, not to a worksheet. Nothing here changes what a
//! worksheet calculates or how it looks on someone else's machine. The defaults
//! for new worksheets (unit system, page) are copied into a worksheet when it is
//! made; from then on they are the worksheet's own, changed in Page setup and on
//! the Units panel.
//!
//! Stored through eframe's storage as `key=value` lines. Unknown keys and values
//! that do not parse are ignored, so a settings file from another version never
//! stops the program starting; a missing value takes its default.

use opencalc_core::document::{Background, PageSetup, PaperSize, MARGIN_RANGE_MM, UNITS_PER_MM};
use opencalc_core::registry::UnitSystem;

use crate::app::AppState;
use crate::theme::PaletteKey;

/// The storage key for the settings text.
pub const STORAGE_KEY: &str = "settings";

/// Interface sizes offered, in percent of normal.
pub const SCALES: [u16; 6] = [80, 90, 100, 110, 125, 150];

/// Recovery intervals offered, in seconds (0 is off).
pub const RECOVERY: [u32; 5] = [15, 30, 60, 300, 0];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// The size of the whole interface, in percent.
    pub scale: u16,
    /// Spacing of the snap grid (and of the grid drawn on the page).
    pub grid: GridSpacing,
    /// How often modified worksheets are written to recovery files, in seconds; 0 is never.
    pub recovery: u32,
    /// The unit system a new worksheet starts with.
    pub units: UnitSystem,
    /// The page a new worksheet starts with.
    pub page: PageSetup,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            scale: 100,
            grid: GridSpacing::default(),
            recovery: 30,
            units: UnitSystem::default(),
            page: PageSetup::default(),
        }
    }
}

impl Settings {
    /// The interface zoom factor egui is given.
    pub fn zoom_factor(&self) -> f32 {
        f32::from(self.scale) / 100.0
    }

    /// Seconds between recovery writes, or `None` when they are off.
    pub fn recovery_every(&self) -> Option<f64> {
        (self.recovery > 0).then_some(f64::from(self.recovery))
    }
}

/// The snap grid. Inch steps put a stronger line every inch; millimetre steps
/// one every centimetre.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GridSpacing {
    Sixteenth,
    #[default]
    Eighth,
    Quarter,
    Mm2,
    Mm5,
}

impl GridSpacing {
    pub const ALL: [GridSpacing; 5] = [
        GridSpacing::Sixteenth,
        GridSpacing::Eighth,
        GridSpacing::Quarter,
        GridSpacing::Mm2,
        GridSpacing::Mm5,
    ];

    /// The step in document units (96 to the inch).
    pub fn step(self) -> f32 {
        match self {
            GridSpacing::Sixteenth => 6.0,
            GridSpacing::Eighth => 12.0,
            GridSpacing::Quarter => 24.0,
            GridSpacing::Mm2 => 2.0 * UNITS_PER_MM,
            GridSpacing::Mm5 => 5.0 * UNITS_PER_MM,
        }
    }

    /// Every how many steps the stronger line is drawn: an inch or a centimetre.
    pub fn major_every(self) -> i64 {
        match self {
            GridSpacing::Sixteenth => 16,
            GridSpacing::Eighth => 8,
            GridSpacing::Quarter => 4,
            GridSpacing::Mm2 => 5,
            GridSpacing::Mm5 => 2,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            GridSpacing::Sixteenth => "1/16 in",
            GridSpacing::Eighth => "1/8 in",
            GridSpacing::Quarter => "1/4 in",
            GridSpacing::Mm2 => "2 mm",
            GridSpacing::Mm5 => "5 mm",
        }
    }

    fn key(self) -> &'static str {
        match self {
            GridSpacing::Sixteenth => "1/16in",
            GridSpacing::Eighth => "1/8in",
            GridSpacing::Quarter => "1/4in",
            GridSpacing::Mm2 => "2mm",
            GridSpacing::Mm5 => "5mm",
        }
    }
}

/// The label of a recovery interval.
pub fn recovery_label(seconds: u32) -> &'static str {
    match seconds {
        0 => "Off",
        15 => "15 s",
        30 => "30 s",
        60 => "1 min",
        300 => "5 min",
        _ => "Custom",
    }
}

/// Rounds a margin to whole millimetres in the accepted range.
fn margin(mm: f32) -> f32 {
    let (lo, hi) = MARGIN_RANGE_MM;
    if mm.is_finite() {
        mm.round().clamp(lo, hi)
    } else {
        PageSetup::default().margin_mm
    }
}

/// Everything kept between sessions, as text: the settings and the session
/// choices that are remembered with them (theme, grid shown, snap).
pub fn to_text(state: &AppState) -> String {
    let s = &state.settings;
    let page = &s.page;
    [
        ("theme", state.palette.key().to_owned()),
        ("scale", s.scale.to_string()),
        ("grid", s.grid.key().to_owned()),
        ("show_grid", state.show_grid.to_string()),
        ("snap", state.snap.to_string()),
        ("recovery", s.recovery.to_string()),
        ("units", s.units.label().to_lowercase()),
        ("paper", page.paper.label().to_lowercase()),
        ("landscape", page.landscape.to_string()),
        ("margin_mm", margin(page.margin_mm).to_string()),
        ("background", page.background.label().to_lowercase()),
    ]
    .into_iter()
    .map(|(k, v)| format!("{k}={v}\n"))
    .collect()
}

/// Read settings stored by [`to_text`] into `state`. Anything missing or not
/// understood keeps the value `state` already has.
pub fn read_text(text: &str, state: &mut AppState) {
    let s = &mut state.settings;
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        let flag = value.parse::<bool>().ok();
        match key {
            "theme" => state.palette = PaletteKey::from_key(value),
            "scale" => {
                if let Some(n) = value.parse().ok().filter(|n| SCALES.contains(n)) {
                    s.scale = n;
                }
            }
            "grid" => {
                if let Some(g) = GridSpacing::ALL.into_iter().find(|g| g.key() == value) {
                    s.grid = g;
                }
            }
            "show_grid" => state.show_grid = flag.unwrap_or(state.show_grid),
            "snap" => state.snap = flag.unwrap_or(state.snap),
            "recovery" => {
                if let Some(n) = value.parse().ok().filter(|n| RECOVERY.contains(n)) {
                    s.recovery = n;
                }
            }
            "units" => {
                if let Some(u) = UnitSystem::ALL
                    .into_iter()
                    .find(|u| u.label().eq_ignore_ascii_case(value))
                {
                    s.units = u;
                }
            }
            "paper" => {
                if let Some(p) = PaperSize::ALL
                    .into_iter()
                    .find(|p| p.label().eq_ignore_ascii_case(value))
                {
                    s.page.paper = p;
                }
            }
            "landscape" => s.page.landscape = flag.unwrap_or(s.page.landscape),
            "margin_mm" => {
                if let Ok(mm) = value.parse::<f32>() {
                    s.page.margin_mm = margin(mm);
                }
            }
            "background" => {
                if let Some(b) = Background::ALL
                    .into_iter()
                    .find(|b| b.label().eq_ignore_ascii_case(value))
                {
                    s.page.background = b;
                }
            }
            _ => {}
        }
    }
    state.theme = crate::theme::Theme::palette(state.palette);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> AppState {
        AppState::new(PaletteKey::Neutral)
    }

    #[test]
    fn settings_survive_a_round_trip_through_text() {
        let mut a = state();
        a.palette = PaletteKey::Slate;
        a.show_grid = false;
        a.snap = false;
        a.settings = Settings {
            scale: 125,
            grid: GridSpacing::Mm5,
            recovery: 0,
            units: UnitSystem::Uscs,
            page: PageSetup {
                paper: PaperSize::Letter,
                landscape: true,
                margin_mm: 12.0,
                background: Background::Dots,
                ..PageSetup::default()
            },
        };
        let mut b = state();
        read_text(&to_text(&a), &mut b);
        assert_eq!(b.settings, a.settings);
        assert_eq!(b.palette, PaletteKey::Slate);
        assert!(!b.show_grid && !b.snap);
    }

    #[test]
    fn nonsense_and_unknown_keys_keep_the_defaults() {
        let mut s = state();
        read_text(
            "scale=7\ngrid=huge\nrecovery=-1\nmargin_mm=NaN\npaper=A3\nfuture=1\nno equals sign\nsnap=maybe",
            &mut s,
        );
        assert_eq!(s.settings, Settings::default());
        assert!(s.snap);
    }

    #[test]
    fn a_wild_margin_is_brought_into_range() {
        let mut s = state();
        read_text("margin_mm=900", &mut s);
        assert_eq!(s.settings.page.margin_mm, MARGIN_RANGE_MM.1);
    }

    #[test]
    fn every_grid_has_a_distinct_key_and_a_major_line_on_a_round_length() {
        for g in GridSpacing::ALL {
            assert_eq!(
                GridSpacing::ALL
                    .iter()
                    .filter(|o| o.key() == g.key())
                    .count(),
                1
            );
            let major = g.step() * g.major_every() as f32;
            let inch = 96.0;
            let cm = 10.0 * UNITS_PER_MM;
            assert!(
                (major - inch).abs() < 1e-3 || (major - cm).abs() < 1e-3,
                "{g:?}"
            );
        }
    }

    #[test]
    fn every_offered_interval_has_a_label() {
        for n in RECOVERY {
            assert_ne!(recovery_label(n), "Custom");
        }
    }
}
