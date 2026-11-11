//! The Shape region: a drawn rectangle (with optionally rounded corners), ellipse,
//! triangle, diamond, polygon or line, with a fill and an outline. A shape has no maths in it, so it is never evaluated.

use serde::{Deserialize, Serialize};

use super::colour::Colour;
use super::geometry::{Point, Size};

/// The widest outline, in document units.
pub const MAX_STROKE: f32 = 40.0;
/// The largest corner radius, in document units.
pub const MAX_RADIUS: f32 = 1000.0;
/// The fewest and most sides a polygon may have.
pub const MIN_SIDES: u8 = 3;
pub const MAX_SIDES: u8 = 24;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShapeKind {
    #[default]
    Rectangle,
    Ellipse,
    /// Apex at the middle of the top edge, base along the bottom.
    Triangle,
    /// The four middles of the box's edges.
    Diamond,
    /// `sides` points evenly round the ellipse that fits the box, the first at the top.
    Polygon,
    /// A straight line between two corners of the shape's box (see
    /// [`ShapeSpec::line_ends`]). It has no inside, so `fill` is unused.
    Line,
}

impl ShapeKind {
    pub fn label(self) -> &'static str {
        match self {
            ShapeKind::Rectangle => "Rectangle",
            ShapeKind::Ellipse => "Ellipse",
            ShapeKind::Triangle => "Triangle",
            ShapeKind::Diamond => "Diamond",
            ShapeKind::Polygon => "Polygon",
            ShapeKind::Line => "Line",
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

/// What an end of a line carries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Arrowhead {
    #[default]
    None,
    /// A filled triangle.
    Triangle,
    /// Two strokes, like an open V.
    Open,
    /// A filled dot.
    Circle,
    /// A filled diamond.
    Diamond,
}

impl Arrowhead {
    pub const ALL: [Arrowhead; 5] = [
        Arrowhead::None,
        Arrowhead::Triangle,
        Arrowhead::Open,
        Arrowhead::Circle,
        Arrowhead::Diamond,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Arrowhead::None => "None",
            Arrowhead::Triangle => "Triangle",
            Arrowhead::Open => "Open",
            Arrowhead::Circle => "Circle",
            Arrowhead::Diamond => "Diamond",
        }
    }
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
    /// Corner radius of a rectangle, in document units; 0 is square corners.
    pub corner_radius: f32,
    pub dash: Dash,
    /// A line only: it starts at the right edge of its box rather than the left.
    #[serde(skip_serializing_if = "is_false")]
    pub flip_x: bool,
    /// A line only: it starts at the bottom edge of its box rather than the top.
    #[serde(skip_serializing_if = "is_false")]
    pub flip_y: bool,
    /// A line only: what the start and the end carry.
    pub arrow_start: Arrowhead,
    pub arrow_end: Arrowhead,
    /// A polygon only: how many sides.
    pub sides: u8,
}

fn is_false(b: &bool) -> bool {
    !*b
}

impl Default for ShapeSpec {
    fn default() -> ShapeSpec {
        ShapeSpec {
            kind: ShapeKind::Rectangle,
            // A light blue inside and a darker blue outline.
            fill: Some(Colour::theme(4, 2)),
            stroke: Some(Colour::theme(4, 4)),
            stroke_width: 1.5,
            corner_radius: 0.0,
            dash: Dash::Solid,
            flip_x: false,
            flip_y: false,
            arrow_start: Arrowhead::None,
            arrow_end: Arrowhead::None,
            sides: 6,
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

    /// The start and end of a line whose region has this `pos` and `size`. A line
    /// runs corner to corner of its box (the start is on the right if `flip_x`, at the
    /// bottom if `flip_y`); a box no more than one unit thick is a level or upright
    /// line, through the middle.
    pub fn line_ends(&self, pos: Point, size: Size) -> (Point, Point) {
        let axis = |lo: f32, len: f32, flip: bool| {
            if len <= 1.0 {
                let mid = lo + len / 2.0;
                (mid, mid)
            } else if flip {
                (lo + len, lo)
            } else {
                (lo, lo + len)
            }
        };
        let (x0, x1) = axis(pos.x, size.w, self.flip_x);
        let (y0, y1) = axis(pos.y, size.h, self.flip_y);
        (Point::new(x0, y0), Point::new(x1, y1))
    }

    /// The box and flips for a line from `from` to `to`: the inverse of
    /// [`ShapeSpec::line_ends`]. A line level or upright to within a unit gets a
    /// box one unit thick, centred on it.
    pub fn line_box(from: Point, to: Point) -> (Point, Size, bool, bool) {
        let axis = |a: f32, b: f32| {
            let d = (a - b).abs();
            if d <= 1.0 {
                ((a + b) / 2.0 - 0.5, 1.0, false)
            } else {
                (a.min(b), d, a > b)
            }
        };
        let (x, w, flip_x) = axis(from.x, to.x);
        let (y, h, flip_y) = axis(from.y, to.y);
        (Point::new(x, y), Size::new(w, h), flip_x, flip_y)
    }

    /// Whether every number and colour is usable.
    pub fn is_valid(&self) -> bool {
        let ok = |v: f32, max: f32| v.is_finite() && (0.0..=max).contains(&v);
        ok(self.stroke_width, MAX_STROKE)
            && ok(self.corner_radius, MAX_RADIUS)
            && (MIN_SIDES..=MAX_SIDES).contains(&self.sides)
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
        self.sides = self.sides.clamp(MIN_SIDES, MAX_SIDES);
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
    fn a_line_runs_corner_to_corner_and_the_box_and_flips_give_it_back() {
        let spec = ShapeSpec::of(ShapeKind::Line);
        let (pos, size) = (Point::new(10.0, 20.0), Size::new(100.0, 40.0));
        assert_eq!(
            spec.line_ends(pos, size),
            (Point::new(10.0, 20.0), Point::new(110.0, 60.0))
        );
        let both = ShapeSpec {
            flip_x: true,
            flip_y: true,
            ..spec.clone()
        };
        assert_eq!(
            both.line_ends(pos, size),
            (Point::new(110.0, 60.0), Point::new(10.0, 20.0))
        );
        for (from, to) in [
            (Point::new(5.0, 5.0), Point::new(80.0, 50.0)),
            (Point::new(80.0, 50.0), Point::new(5.0, 5.0)),
            (Point::new(5.0, 50.0), Point::new(80.0, 5.0)),
            (Point::new(80.0, 5.0), Point::new(5.0, 50.0)),
        ] {
            let (pos, size, flip_x, flip_y) = ShapeSpec::line_box(from, to);
            let s = ShapeSpec {
                flip_x,
                flip_y,
                ..spec.clone()
            };
            assert_eq!(s.line_ends(pos, size), (from, to));
        }
    }

    #[test]
    fn a_level_line_has_a_one_unit_box_centred_on_it() {
        let (pos, size, _, flip_y) =
            ShapeSpec::line_box(Point::new(0.0, 30.0), Point::new(90.0, 30.2));
        assert!((pos.y - 29.6).abs() < 1e-4 && size.h == 1.0 && !flip_y);
        let (a, b) = ShapeSpec::of(ShapeKind::Line).line_ends(pos, size);
        assert_eq!(a.y, b.y, "both ends on the middle of the box");
    }

    #[test]
    fn repair_clamps_numbers_and_fixes_colours() {
        let s = ShapeSpec {
            stroke_width: f32::NAN,
            corner_radius: 1.0e9,
            sides: 200,
            fill: Some(Colour::theme(99, 0)),
            ..ShapeSpec::default()
        };
        assert!(!s.is_valid());
        let r = s.repaired();
        assert!(r.is_valid());
        assert_eq!(r.stroke_width, ShapeSpec::default().stroke_width);
        assert_eq!(r.corner_radius, MAX_RADIUS);
        assert_eq!(r.sides, MAX_SIDES);
        assert_eq!(r.fill, Some(Colour::Auto));
    }
}
