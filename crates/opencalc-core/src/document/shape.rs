//! The Shape region: a drawn rectangle, rounded rectangle or ellipse with a fill
//! and an outline. A shape has no maths in it, so it is never evaluated.

use serde::{Deserialize, Serialize};

use super::colour::Colour;

/// The widest outline, in document units.
pub const MAX_STROKE: f32 = 40.0;
/// The largest corner radius, in document units.
pub const MAX_RADIUS: f32 = 1000.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShapeKind {
    #[default]
    Rectangle,
    RoundedRectangle,
    Ellipse,
}

impl ShapeKind {
    pub fn label(self) -> &'static str {
        match self {
            ShapeKind::Rectangle => "Rectangle",
            ShapeKind::RoundedRectangle => "Rounded rectangle",
            ShapeKind::Ellipse => "Ellipse",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Dash {
    #[default]
    Solid,
    Dashed,
    Dotted,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShapeSpec {
    pub kind: ShapeKind,
    /// The inside colour; none leaves it empty (see-through).
    pub fill: Option<Colour>,
    /// The outline colour; none draws no outline.
    pub stroke: Option<Colour>,
    /// Outline width in document units.
    pub stroke_width: f32,
    /// Corner radius of a rounded rectangle, in document units.
    pub corner_radius: f32,
    pub dash: Dash,
}

impl Default for ShapeSpec {
    fn default() -> ShapeSpec {
        ShapeSpec {
            kind: ShapeKind::Rectangle,
            // A light blue inside and a darker blue outline.
            fill: Some(Colour::theme(4, 2)),
            stroke: Some(Colour::theme(4, 4)),
            stroke_width: 1.5,
            corner_radius: 12.0,
            dash: Dash::Solid,
        }
    }
}

impl ShapeSpec {
    /// A new shape of `kind` with the usual colours.
    pub fn of(kind: ShapeKind) -> ShapeSpec {
        ShapeSpec {
            kind,
            ..ShapeSpec::default()
        }
    }

    /// Whether every number and colour is usable.
    pub fn is_valid(&self) -> bool {
        let ok = |v: f32, max: f32| v.is_finite() && (0.0..=max).contains(&v);
        ok(self.stroke_width, MAX_STROKE)
            && ok(self.corner_radius, MAX_RADIUS)
            && self.fill.is_none_or(Colour::is_valid)
            && self.stroke.is_none_or(Colour::is_valid)
    }

    /// The same shape with every number brought into range and bad colours made
    /// automatic.
    pub fn repaired(mut self) -> ShapeSpec {
        let fix = |v: f32, max: f32, otherwise: f32| {
            if v.is_finite() {
                v.clamp(0.0, max)
            } else {
                otherwise
            }
        };
        let d = ShapeSpec::default();
        self.stroke_width = fix(self.stroke_width, MAX_STROKE, d.stroke_width);
        self.corner_radius = fix(self.corner_radius, MAX_RADIUS, d.corner_radius);
        self.fill = self.fill.map(Colour::repaired);
        self.stroke = self.stroke.map(Colour::repaired);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_valid_and_a_blank_object_reads_as_the_default() {
        assert!(ShapeSpec::default().is_valid());
        let read: ShapeSpec = serde_json::from_str("{}").unwrap();
        assert_eq!(read, ShapeSpec::default());
    }

    #[test]
    fn no_fill_and_no_outline_survive_a_round_trip() {
        let s = ShapeSpec {
            fill: None,
            stroke: None,
            ..ShapeSpec::of(ShapeKind::Ellipse)
        };
        let text = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<ShapeSpec>(&text).unwrap(), s);
    }

    #[test]
    fn repair_clamps_numbers_and_fixes_colours() {
        let s = ShapeSpec {
            stroke_width: f32::NAN,
            corner_radius: 1.0e9,
            fill: Some(Colour::theme(99, 0)),
            ..ShapeSpec::default()
        };
        assert!(!s.is_valid());
        let r = s.repaired();
        assert!(r.is_valid());
        assert_eq!(r.stroke_width, ShapeSpec::default().stroke_width);
        assert_eq!(r.corner_radius, MAX_RADIUS);
        assert_eq!(r.fill, Some(Colour::Auto));
    }
}
