//! How wide the Home page is and whether it has a side column. A pure function of
//! the width available, so it is tested without a window.

/// The single-column page's width, and the room kept at each side of it.
const SINGLE_WIDTH: f32 = 720.0;
const SINGLE_MARGIN: f32 = 16.0;
/// From this window width the page has a side column.
const WIDE_FROM: f32 = 900.0;
const WIDE_MARGIN: f32 = 32.0;
/// The most the two columns and the gap between them may take.
const WIDE_MAX: f32 = 1120.0;
pub const COLUMN_GAP: f32 = 24.0;
const SIDE_MIN: f32 = 260.0;
const SIDE_MAX: f32 = 320.0;
const MAIN_MIN: f32 = 520.0;

/// Where the page's content goes in a window `available` points wide.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Layout {
    /// One column `width` wide, `margin` in from the left.
    Single { width: f32, margin: f32 },
    /// A main column, a gap, and a side column.
    Wide { main: f32, side: f32, margin: f32 },
}

impl Layout {
    pub fn margin(self) -> f32 {
        match self {
            Layout::Single { margin, .. } | Layout::Wide { margin, .. } => margin,
        }
    }

    /// The width of the main column.
    pub fn main(self) -> f32 {
        match self {
            Layout::Single { width, .. } => width,
            Layout::Wide { main, .. } => main,
        }
    }
}

pub fn layout(available: f32) -> Layout {
    if available >= WIDE_FROM {
        let content = (available - 2.0 * WIDE_MARGIN).min(WIDE_MAX);
        let side = (content * 0.3).clamp(SIDE_MIN, SIDE_MAX);
        let main = content - side - COLUMN_GAP;
        if main >= MAIN_MIN {
            return Layout::Wide {
                main,
                side,
                margin: (available - content) / 2.0,
            };
        }
    }
    let width = (available - 2.0 * SINGLE_MARGIN).clamp(0.0, SINGLE_WIDTH);
    Layout::Single {
        width,
        margin: (available - width) / 2.0,
    }
}

/// How many items of at least `min` points fit across `width` with `gap` between
/// them, between 1 and `most`.
pub fn columns(width: f32, min: f32, gap: f32, most: usize) -> usize {
    (((width + gap) / (min + gap)).floor() as usize).clamp(1, most)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrow_windows_get_one_column_with_room_at_both_sides() {
        for available in [320.0, 540.0, 800.0, 899.0] {
            let Layout::Single { width, margin } = layout(available) else {
                panic!("one column at {available}");
            };
            assert!(width <= SINGLE_WIDTH);
            assert!(margin >= SINGLE_MARGIN.min(available / 2.0), "{available}");
            assert!((margin * 2.0 + width - available).abs() < 0.01, "centred");
        }
        // 800 x 600 keeps the whole 720 column.
        assert_eq!(
            layout(800.0),
            Layout::Single {
                width: 720.0,
                margin: 40.0
            }
        );
    }

    #[test]
    fn from_900_there_is_a_side_column_that_never_squeezes_the_main_one() {
        for available in [900.0, 1000.0, 1280.0, 1920.0, 3840.0] {
            let Layout::Wide { main, side, margin } = layout(available) else {
                panic!("two columns at {available}");
            };
            assert!(main >= MAIN_MIN, "{available}: {main}");
            assert!((SIDE_MIN..=SIDE_MAX).contains(&side), "{available}: {side}");
            assert!(margin >= WIDE_MARGIN, "{available}");
            assert!(
                main + side + COLUMN_GAP <= WIDE_MAX + 0.01,
                "no endless rows"
            );
            assert!((margin * 2.0 + main + side + COLUMN_GAP - available).abs() < 0.01);
        }
    }

    #[test]
    fn as_many_fit_across_as_the_room_allows_and_never_fewer_than_one() {
        assert_eq!(columns(720.0, 170.0, 12.0, 4), 4);
        assert_eq!(columns(360.0, 170.0, 12.0, 4), 2);
        assert_eq!(columns(100.0, 170.0, 12.0, 4), 1);
        assert_eq!(columns(5000.0, 170.0, 12.0, 4), 4, "capped");
        assert_eq!(columns(0.0, 170.0, 12.0, 4), 1);
    }
}
