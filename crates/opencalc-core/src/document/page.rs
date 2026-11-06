//! The page a worksheet is laid out on: paper size, orientation and margins.
//!
//! A worksheet is a document, so its regions sit on pages. The page setup is part
//! of the saved worksheet and changes by an undoable edit, like the unit system.
//! Lengths are in document units, which are 1/96 inch (one CSS pixel), the same
//! units region positions use.

use serde::{Deserialize, Serialize};

/// Document units per millimetre (96 per inch).
pub const UNITS_PER_MM: f32 = 96.0 / 25.4;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PaperSize {
    #[default]
    A4,
    Letter,
}

impl PaperSize {
    pub const ALL: [PaperSize; 2] = [PaperSize::A4, PaperSize::Letter];

    pub fn label(self) -> &'static str {
        match self {
            PaperSize::A4 => "A4",
            PaperSize::Letter => "Letter",
        }
    }

    /// Portrait width and height in millimetres.
    fn millimetres(self) -> (f32, f32) {
        match self {
            PaperSize::A4 => (210.0, 297.0),
            PaperSize::Letter => (215.9, 279.4),
        }
    }
}

/// Smallest and largest margin accepted, in millimetres.
pub const MARGIN_RANGE_MM: (f32, f32) = (0.0, 60.0);

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PageSetup {
    #[serde(default)]
    pub paper: PaperSize,
    #[serde(default)]
    pub landscape: bool,
    /// The same margin on all four sides, in millimetres.
    #[serde(default = "default_margin")]
    pub margin_mm: f32,
}

fn default_margin() -> f32 {
    20.0
}

impl Default for PageSetup {
    fn default() -> PageSetup {
        PageSetup {
            paper: PaperSize::default(),
            landscape: false,
            margin_mm: default_margin(),
        }
    }
}

impl PageSetup {
    /// A setup with the margin forced into the accepted range (and not NaN).
    pub fn sanitised(mut self) -> PageSetup {
        let (lo, hi) = MARGIN_RANGE_MM;
        self.margin_mm = if self.margin_mm.is_nan() {
            default_margin()
        } else {
            self.margin_mm.clamp(lo, hi)
        };
        self
    }

    /// Page width and height in document units, after orientation.
    pub fn size(&self) -> (f32, f32) {
        let (w, h) = self.paper.millimetres();
        let (w, h) = if self.landscape { (h, w) } else { (w, h) };
        (w * UNITS_PER_MM, h * UNITS_PER_MM)
    }

    /// The margin in document units.
    pub fn margin(&self) -> f32 {
        self.margin_mm * UNITS_PER_MM
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a4_is_794_by_1123_units_and_landscape_swaps_them() {
        let (w, h) = PageSetup::default().size();
        assert!(
            (w - 793.7).abs() < 0.1 && (h - 1122.5).abs() < 0.1,
            "{w} x {h}"
        );
        let landscape = PageSetup {
            landscape: true,
            ..PageSetup::default()
        };
        assert_eq!(landscape.size(), (h, w));
    }

    #[test]
    fn letter_is_8_5_by_11_inches() {
        let setup = PageSetup {
            paper: PaperSize::Letter,
            ..PageSetup::default()
        };
        let (w, h) = setup.size();
        assert!(
            (w - 816.0).abs() < 0.5 && (h - 1056.0).abs() < 0.5,
            "{w} x {h}"
        );
    }

    #[test]
    fn the_default_margin_is_twenty_millimetres() {
        assert!((PageSetup::default().margin() - 75.59).abs() < 0.01);
    }

    #[test]
    fn a_margin_is_kept_in_range() {
        let set = |m| PageSetup {
            margin_mm: m,
            ..PageSetup::default()
        };
        assert_eq!(set(-5.0).sanitised().margin_mm, 0.0);
        assert_eq!(set(500.0).sanitised().margin_mm, 60.0);
        assert_eq!(set(f32::NAN).sanitised().margin_mm, 20.0);
        assert_eq!(set(15.0).sanitised().margin_mm, 15.0);
    }

    #[test]
    fn a_partial_saved_setup_fills_in_defaults() {
        let s: PageSetup = serde_json::from_str(r#"{"landscape":true}"#).unwrap();
        assert!(s.landscape);
        assert_eq!(s.paper, PaperSize::A4);
        assert_eq!(s.margin_mm, 20.0);
    }
}
