//! Edits to what one region holds or how it looks: each sets a field and returns the
//! edit that sets the old value back.

use super::{Edit, EditError};
use crate::document::{
    adjust_runs, clean_label, normalise_runs, FontStyle, PictureSpec, PlotSpec, Point, RegionId,
    RegionKind, ResultFormat, Run, ShapeSpec, Size, TableSpec, Worksheet,
};

impl Worksheet {
    pub(super) fn set_math_source(
        &mut self,
        id: RegionId,
        source: &str,
    ) -> Result<Edit, EditError> {
        self.unlocked(id)?;
        let region = self.region_mut(id)?;
        let RegionKind::Math(m) = &mut region.kind else {
            return Err(EditError::WrongKind(id));
        };
        let old = m.source().to_owned();
        m.set_source(source.to_owned());
        Ok(Edit::SetMathSource { id, source: old })
    }

    pub(super) fn set_label(&mut self, id: RegionId, label: &str) -> Result<Edit, EditError> {
        let region = self.region_mut(id)?;
        let old = std::mem::replace(&mut region.label, clean_label(label));
        Ok(Edit::SetLabel { id, label: old })
    }

    pub(super) fn set_locked(&mut self, id: RegionId, locked: bool) -> Result<Edit, EditError> {
        let region = self.region_mut(id)?;
        let old = std::mem::replace(&mut region.locked, locked);
        Ok(Edit::SetLocked { id, locked: old })
    }

    pub(super) fn set_font_style(
        &mut self,
        id: RegionId,
        style: FontStyle,
    ) -> Result<Edit, EditError> {
        self.unlocked(id)?;
        let region = self.region_mut(id)?;
        let old = std::mem::replace(&mut region.style, style);
        Ok(Edit::SetFontStyle { id, style: old })
    }

    pub(super) fn set_runs(&mut self, id: RegionId, runs: &[Run]) -> Result<Edit, EditError> {
        self.unlocked(id)?;
        let region = self.region_mut(id)?;
        let RegionKind::Text { text } = &region.kind else {
            return Err(EditError::WrongKind(id));
        };
        let fitted = normalise_runs(runs, text.chars().count());
        let old = std::mem::replace(&mut region.runs, fitted);
        Ok(Edit::SetRuns { id, runs: old })
    }

    pub(super) fn set_border(&mut self, id: RegionId, border: bool) -> Result<Edit, EditError> {
        self.unlocked(id)?;
        let region = self.region_mut(id)?;
        let old = std::mem::replace(&mut region.border, border);
        Ok(Edit::SetBorder { id, border: old })
    }

    pub(super) fn set_result_format(
        &mut self,
        id: RegionId,
        format: &ResultFormat,
    ) -> Result<Edit, EditError> {
        self.unlocked(id)?;
        let region = self.region_mut(id)?;
        let RegionKind::Math(m) = &mut region.kind else {
            return Err(EditError::WrongKind(id));
        };
        let old = m.format();
        m.set_format(format.clone());
        Ok(Edit::SetResultFormat { id, format: old })
    }

    pub(super) fn set_text(&mut self, id: RegionId, text: &str) -> Result<Edit, EditError> {
        self.unlocked(id)?;
        let region = self.region_mut(id)?;
        let RegionKind::Text { text: current } = &mut region.kind else {
            return Err(EditError::WrongKind(id));
        };
        let old = std::mem::replace(current, text.to_owned());
        // Styled stretches follow the text they were made on.
        let old_runs = region.runs.clone();
        region.runs = adjust_runs(&old_runs, &old, text);
        if region.runs == old_runs {
            Ok(Edit::SetText { id, text: old })
        } else {
            // Undo must bring the exact old styling back, not a guess.
            Ok(Edit::Batch(vec![
                Edit::SetText { id, text: old },
                Edit::SetRuns { id, runs: old_runs },
            ]))
        }
    }

    pub(super) fn set_plot(&mut self, id: RegionId, spec: &PlotSpec) -> Result<Edit, EditError> {
        self.unlocked(id)?;
        let region = self.region_mut(id)?;
        let RegionKind::Plot(current) = &mut region.kind else {
            return Err(EditError::WrongKind(id));
        };
        let old = std::mem::replace(current, PlotSpec::clone(spec));
        Ok(Edit::SetPlot {
            id,
            spec: Box::new(old),
        })
    }

    pub(super) fn set_table(&mut self, id: RegionId, spec: &TableSpec) -> Result<Edit, EditError> {
        self.unlocked(id)?;
        let region = self.region_mut(id)?;
        let RegionKind::Table(current) = &mut region.kind else {
            return Err(EditError::WrongKind(id));
        };
        let old = std::mem::replace(current, Box::new(spec.clone().repaired()));
        Ok(Edit::SetTable { id, spec: old })
    }

    pub(super) fn set_picture(
        &mut self,
        id: RegionId,
        spec: &PictureSpec,
    ) -> Result<Edit, EditError> {
        self.unlocked(id)?;
        let region = self.region_mut(id)?;
        let RegionKind::Picture(current) = &mut region.kind else {
            return Err(EditError::WrongKind(id));
        };
        let old = std::mem::replace(current, Box::new(spec.clone()));
        Ok(Edit::SetPicture { id, spec: old })
    }

    pub(super) fn set_shape(&mut self, id: RegionId, spec: &ShapeSpec) -> Result<Edit, EditError> {
        self.unlocked(id)?;
        let region = self.region_mut(id)?;
        let RegionKind::Shape(current) = &mut region.kind else {
            return Err(EditError::WrongKind(id));
        };
        let old = std::mem::replace(current, Box::new(spec.clone().repaired()));
        Ok(Edit::SetShape { id, spec: old })
    }

    /// Put a line's ends at `from` and `to`; its box and flips follow. The inverse
    /// restores the old shape settings and box exactly.
    pub(super) fn set_line_ends(
        &mut self,
        id: RegionId,
        from: Point,
        to: Point,
    ) -> Result<Edit, EditError> {
        self.unlocked(id)?;
        let region = self.region_mut(id)?;
        let RegionKind::Shape(spec) = &mut region.kind else {
            return Err(EditError::WrongKind(id));
        };
        if spec.kind != crate::document::ShapeKind::Line {
            return Err(EditError::WrongKind(id));
        }
        let finite = |p: Point| p.x.is_finite() && p.y.is_finite();
        if !finite(from) || !finite(to) {
            return Ok(Edit::Batch(Vec::new()));
        }
        let before = (spec.clone(), region.pos, region.size);
        let (pos, size, flip_x, flip_y) = ShapeSpec::line_box(from, to);
        spec.flip_x = flip_x;
        spec.flip_y = flip_y;
        region.pos = pos;
        region.size = size;
        Ok(Edit::Batch(vec![
            Edit::SetShape { id, spec: before.0 },
            Edit::SetRegionBox {
                id,
                pos: before.1,
                size: before.2,
            },
        ]))
    }

    pub(super) fn set_region_size(&mut self, id: RegionId, size: Size) -> Result<Edit, EditError> {
        self.unlocked(id)?;
        let region = self.region_mut(id)?;
        let old = std::mem::replace(&mut region.size, size);
        Ok(Edit::SetRegionSize { id, size: old })
    }

    /// Position and size together. Values outside the file format's limits (or not
    /// finite) are brought inside them.
    pub(super) fn set_region_box(
        &mut self,
        id: RegionId,
        pos: Point,
        size: Size,
    ) -> Result<Edit, EditError> {
        self.unlocked(id)?;
        let region = self.region_mut(id)?;
        let usable = |v: f32| {
            if v.is_finite() {
                v.clamp(1.0, crate::persist::MAX_EXTENT)
            } else {
                1.0
            }
        };
        let coord = |v: f32| {
            if v.is_finite() {
                v.clamp(-crate::persist::MAX_COORD, crate::persist::MAX_COORD)
            } else {
                0.0
            }
        };
        let old_pos = std::mem::replace(&mut region.pos, Point::new(coord(pos.x), coord(pos.y)));
        let old_size =
            std::mem::replace(&mut region.size, Size::new(usable(size.w), usable(size.h)));
        Ok(Edit::SetRegionBox {
            id,
            pos: old_pos,
            size: old_size,
        })
    }
}
