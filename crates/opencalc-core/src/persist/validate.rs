//! Checks on a loaded worksheet beyond "it parses". A file is untrusted input:
//! what can be repaired safely is repaired and reported, anything else refuses the
//! file with a reason. The same checks run before saving, so the app can never
//! write a file it would not load.

use std::collections::HashSet;
use std::fmt;

use super::LoadError;
use crate::document::{
    MathRegion, PlotSpec, Point, Region, RegionId, RegionKind, Size, Worksheet, MAX_SERIES,
};

pub const MAX_REGIONS: usize = 50_000;
pub const MAX_CUSTOM_UNITS: usize = 1_000;
/// Longest text accepted in any one field, in characters.
pub const MAX_TEXT_CHARS: usize = 65_536;
/// Positions are kept within this distance of the origin.
pub const MAX_COORD: f32 = 1.0e6;
/// Sizes are kept at or below this.
pub const MAX_EXTENT: f32 = 1.0e5;

/// Something wrong in a file that was fixed on loading.
#[derive(Clone, Debug, PartialEq)]
pub enum Repair {
    /// The next free region number was not above the highest one in use.
    NextIdRaised { from: u64, to: u64 },
    /// A region's position was not a usable number, or was far off the page.
    PositionFixed(RegionId),
    /// A region's size was not usable.
    SizeFixed(RegionId),
    /// A plot had no data series; a blank one was added to type into.
    SeriesAdded(RegionId),
    /// The page margin was out of range.
    MarginFixed,
}

impl fmt::Display for Repair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Repair::NextIdRaised { from, to } => {
                write!(f, "the next region number was {from}, raised to {to}")
            }
            Repair::PositionFixed(id) => write!(f, "region {} had an unusable position", id.0),
            Repair::SizeFixed(id) => write!(f, "region {} had an unusable size", id.0),
            Repair::SeriesAdded(id) => write!(f, "plot {} had no series; one was added", id.0),
            Repair::MarginFixed => write!(f, "the page margin was out of range"),
        }
    }
}

fn too_long(what: &'static str, text: &str) -> Result<(), LoadError> {
    // Bytes are at least chars, so a short byte length rules the long case out cheaply.
    if text.len() > MAX_TEXT_CHARS && text.chars().count() > MAX_TEXT_CHARS {
        return Err(LoadError::TooLong {
            what,
            limit: MAX_TEXT_CHARS,
        });
    }
    Ok(())
}

fn math(what: &'static str, m: &MathRegion) -> Result<(), LoadError> {
    too_long(what, m.source())
}

fn check_text(r: &Region) -> Result<(), LoadError> {
    too_long("a region label", &r.label)?;
    match &r.kind {
        RegionKind::Math(m) => math("a math source", m),
        RegionKind::Text { text } => too_long("a text region", text),
        RegionKind::Area { title } => too_long("an area title", title),
        RegionKind::Plot(p) => {
            too_long("a plot title", &p.title)?;
            too_long("a plot axis label", &p.x_label)?;
            too_long("a plot axis label", &p.y_label)?;
            math("a plot source", &p.x)?;
            p.series.iter().try_for_each(|s| math("a plot source", s))
        }
        RegionKind::Table | RegionKind::Picture => Ok(()),
    }
}

fn fix_geometry(r: &mut Region) -> Vec<Repair> {
    let mut out = Vec::new();
    let coord = |v: f32| {
        if v.is_finite() {
            v.clamp(-MAX_COORD, MAX_COORD)
        } else {
            0.0
        }
    };
    let pos = Point::new(coord(r.pos.x), coord(r.pos.y));
    if pos != r.pos {
        r.pos = pos;
        out.push(Repair::PositionFixed(r.id));
    }
    let usable = |v: f32| v.is_finite() && v > 0.0;
    let size = if usable(r.size.w) && usable(r.size.h) {
        Size::new(r.size.w.min(MAX_EXTENT), r.size.h.min(MAX_EXTENT))
    } else {
        r.kind.default_size()
    };
    if size != r.size {
        r.size = size;
        out.push(Repair::SizeFixed(r.id));
    }
    out
}

fn check_plot(id: RegionId, p: &mut PlotSpec) -> Result<Option<Repair>, LoadError> {
    if p.series.len() > MAX_SERIES {
        return Err(LoadError::TooMany {
            what: "series in a plot",
            limit: MAX_SERIES,
        });
    }
    if p.series.is_empty() {
        p.series.push(MathRegion::new(""));
        return Ok(Some(Repair::SeriesAdded(id)));
    }
    Ok(None)
}

/// Check `ws`, repairing what can be repaired. Returns the repairs made.
pub fn check(ws: &mut Worksheet) -> Result<Vec<Repair>, LoadError> {
    if ws.regions.len() > MAX_REGIONS {
        return Err(LoadError::TooMany {
            what: "regions",
            limit: MAX_REGIONS,
        });
    }
    if ws.custom_units.len() > MAX_CUSTOM_UNITS {
        return Err(LoadError::TooMany {
            what: "custom units",
            limit: MAX_CUSTOM_UNITS,
        });
    }
    too_long("the title", &ws.title)?;
    let mut ids = HashSet::with_capacity(ws.regions.len());
    let mut highest = 0;
    for r in &ws.regions {
        if !ids.insert(r.id) {
            return Err(LoadError::DuplicateId(r.id));
        }
        highest = highest.max(r.id.0);
        check_text(r)?;
    }
    let mut repairs = Vec::new();
    let needed = highest.checked_add(1).ok_or(LoadError::IdOverflow)?;
    if ws.next_id < needed {
        repairs.push(Repair::NextIdRaised {
            from: ws.next_id,
            to: needed,
        });
        ws.next_id = needed;
    }
    ws.units().map_err(LoadError::Units)?;
    let page = ws.page.sanitised();
    if page != ws.page {
        ws.page = page;
        repairs.push(Repair::MarginFixed);
    }
    for r in &mut ws.regions {
        repairs.extend(fix_geometry(r));
        let id = r.id;
        if let RegionKind::Plot(p) = &mut r.kind {
            repairs.extend(check_plot(id, p)?);
        }
    }
    Ok(repairs)
}
