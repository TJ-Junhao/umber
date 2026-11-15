//! Titles and subtitles that shrink to fit their room.
//!
//! A title or subtitle set without `size=` that is wider than the room it is aligned in is set
//! at the largest size, a multiple of [`STEP`] below its own, at which every line fits; never
//! below a floor, which is itself the last size tried. The text keeps its face and weight, and
//! the header takes the height of the size it is set at. A text that does not fit at its floor
//! is set there and fails the containment check, which names its width at that size. Text that
//! fits, and text with an explicit size, keep their size.
//!
//! The floors are the theme's axis label size for panel titles and subtitles and for the
//! figure subtitle, and the panel title size for the figure title. A subtitle is set at the
//! axis label size already, so it is never set smaller: one too wide fails as before.
//!
//! The panel titles of a figure keep one size: when one shrinks, every panel title without an
//! explicit size takes the smallest size any of them needs, so that the titles of a grid stay
//! equal. Panel subtitles do the same among themselves. The figure title and subtitle shrink
//! on their own.
//!
//! The figure header is fitted once, against the inner canvas width, before the grid is laid
//! out. Panel headers are fitted once the layout has settled ([`fit_panel_headers`]): a header
//! box spans the plot and the y tick label column, whose widths do not depend on the header, so
//! the plots are solved again with the shrunk headers and the same ticks; the columns, and so
//! the rooms the titles were fitted against, come out the same, and only the plots grow taller.
//! Each text is measured against one final width, once, and nothing alternates.

use super::inset::CONTAINMENT_TOLERANCE;
use super::panel::{Header, HeaderText, PanelBands, PanelSlot};
use super::{LayoutInput, TextRole};
use crate::text::SceneFonts;
use crate::{Result, SizeSpec, Theme};

/// The step between the sizes a text shrinks through, in points.
pub(crate) const STEP: f32 = 0.5;

/// How far a text may shrink: from the size it is set at down to its floor.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Shrink {
    /// The size the text is set at, in points.
    size: f32,
    /// The smallest size it may take, in points.
    floor: f32,
}

impl Shrink {
    /// The largest size at which `fits` holds, for a text `width` wide at its own size that
    /// does not fit `room`: a multiple of [`STEP`] below its size and at least the floor, or the
    /// floor itself when no such multiple fits. The floor when nothing fits; the text's own size
    /// when that is the floor already.
    ///
    /// Widths grow in proportion to the size, so the search starts at the size that width
    /// suggests and steps from there; `fits` decides.
    ///
    /// # Errors
    ///
    /// As `fits`.
    fn fit(self, width: f64, room: f64, mut fits: impl FnMut(f32) -> Result<bool>) -> Result<f32> {
        if self.size <= self.floor {
            return Ok(self.size);
        }
        // The ratio is below 1, and an f32 holds a font size.
        let estimate = self.size * (room / width) as f32;
        let mut size = self.below(self.size).min(self.on_step(estimate));
        while !fits(size)? {
            if size <= self.floor {
                return Ok(self.floor);
            }
            size = self.below(size);
        }
        loop {
            let up = above(size);
            if up >= self.size || !fits(up)? {
                return Ok(size);
            }
            size = up;
        }
    }

    /// The largest multiple of [`STEP`] below `size`, or the floor when that is larger.
    fn below(self, size: f32) -> f32 {
        (((size / STEP).ceil() - 1.0) * STEP).max(self.floor)
    }

    /// The largest multiple of [`STEP`] at most `size`, or the floor when that is larger.
    fn on_step(self, size: f32) -> f32 {
        ((size / STEP).floor() * STEP).max(self.floor)
    }
}

/// The smallest multiple of [`STEP`] above `size`.
fn above(size: f32) -> f32 {
    ((size / STEP).floor() + 1.0) * STEP
}

/// The floor of a header text: the theme's axis label size for a subtitle, and for a title
/// the size of `title_floor`'s role.
fn floor(theme: &Theme, kind: HeaderText, title_floor: TextRole) -> f32 {
    match kind {
        HeaderText::Title => title_floor.size(theme, SizeSpec::Auto),
        HeaderText::Subtitle => TextRole::Body.size(theme, SizeSpec::Auto),
    }
}

/// The size the text `kind` of `header`, aligned in `room`, needs to fit it: `None` when it
/// fits at its own size or has none, otherwise as [`Shrink::fit`] with `floor`.
///
/// # Errors
///
/// As [`Header::fits_at`].
fn needed(
    header: &Header,
    kind: HeaderText,
    lib: &SceneFonts<'_>,
    room: f64,
    floor: f32,
) -> Result<Option<f32>> {
    let Some((width, size)) = header.width_and_size(kind) else {
        return Ok(None);
    };
    if width <= room + CONTAINMENT_TOLERANCE {
        return Ok(None);
    }
    Shrink { size, floor }
        .fit(width, room, |size| header.fits_at(kind, lib, size, room))
        .map(Some)
}

/// Shrinks the figure title and subtitle of `header`, each set at the size `sizes` asks for,
/// to fit `room`, the inner canvas width: each without an explicit size, on its own.
///
/// # Errors
///
/// As [`Header::fits_at`].
pub(super) fn fit_figure_header(
    header: &mut Header,
    sizes: [SizeSpec; 2],
    theme: &Theme,
    lib: &SceneFonts<'_>,
    room: f64,
) -> Result<()> {
    for (kind, asked) in [HeaderText::Title, HeaderText::Subtitle]
        .into_iter()
        .zip(sizes)
    {
        if asked != SizeSpec::Auto {
            continue;
        }
        let floor = floor(theme, kind, TextRole::Title);
        if let Some(size) = needed(header, kind, lib, room, floor)? {
            header.resize(kind, lib, size)?;
        }
    }
    Ok(())
}

/// Shrinks the titles and subtitles of the panels of `inputs`, measured in `bands` and placed
/// in `slots` (in the same order), to fit their header boxes: the panel titles without an
/// explicit size all take the smallest size any of them needs, and so do the panel subtitles.
/// Returns whether any text shrank, in which case the plots are to be solved again for the
/// shorter headers.
///
/// # Errors
///
/// As [`Header::fits_at`].
pub(crate) fn fit_panel_headers(
    inputs: &[LayoutInput<'_>],
    bands: &mut [PanelBands],
    slots: &[PanelSlot],
) -> Result<bool> {
    let mut shrank = false;
    for kind in [HeaderText::Title, HeaderText::Subtitle] {
        let shrinks = |input: &LayoutInput<'_>| {
            let asked = match kind {
                HeaderText::Title => input.axes.title_size(),
                HeaderText::Subtitle => input.axes.subtitle_size(),
            };
            asked == SizeSpec::Auto
        };
        let mut least: Option<f32> = None;
        for ((input, bands), slot) in inputs.iter().zip(&*bands).zip(slots) {
            if !shrinks(input) {
                continue;
            }
            let room = slot.header.right - slot.header.left;
            let floor = floor(input.theme, kind, TextRole::Body);
            if let Some(size) = needed(&bands.heading, kind, input.lib, room, floor)? {
                least = Some(least.map_or(size, |least| least.min(size)));
            }
        }
        let Some(least) = least else {
            continue;
        };
        for (input, bands) in inputs.iter().zip(&mut *bands) {
            if !shrinks(input) {
                continue;
            }
            if let Some((_, size)) = bands.heading.width_and_size(kind)
                && least < size
            {
                bands.heading.resize(kind, input.lib, least)?;
                shrank = true;
            }
        }
    }
    Ok(shrank)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sizes tried by a search that accepts exactly the sizes at most `largest`, and the
    /// size it returns.
    fn search(shrink: Shrink, width: f64, room: f64, largest: f32) -> (f32, Vec<f32>) {
        let mut tried = Vec::new();
        let size = shrink
            .fit(width, room, |size| {
                tried.push(size);
                Ok(size <= largest)
            })
            .unwrap();
        (size, tried)
    }

    #[test]
    fn the_largest_step_that_fits_is_found_from_either_side_of_the_estimate() {
        let shrink = Shrink {
            size: 15.0,
            floor: 8.5,
        };
        // The estimate, 7.5 × 15 / 10 = 11.25, rounds down to 11; 11.5 and 12 fit too.
        assert_eq!(search(shrink, 15.0, 11.25, 12.2).0, 12.0);
        // The estimate is too large: the search steps down to the first size that fits.
        assert_eq!(search(shrink, 15.0, 11.25, 10.6).0, 10.5);
        // Never the size itself, which did not fit.
        assert_eq!(search(shrink, 15.0, 14.9, 100.0).0, 14.5);
    }

    #[test]
    fn the_floor_is_the_last_size_tried() {
        let shrink = Shrink {
            size: 16.5,
            floor: 9.35,
        };
        let (size, tried) = search(shrink, 100.0, 1.0, 0.0);
        assert_eq!(size, 9.35);
        assert_eq!(tried, [9.35]);
        assert_eq!(search(shrink, 100.0, 1.0, 9.4).0, 9.35);
        assert_eq!(search(shrink, 100.0, 1.0, 9.6).0, 9.5);
        // A text at its floor already keeps its size, and nothing is measured.
        let at_floor = Shrink {
            size: 8.5,
            floor: 8.5,
        };
        assert_eq!(search(at_floor, 100.0, 1.0, 100.0), (8.5, Vec::new()));
    }

    #[test]
    fn sizes_off_the_step_shrink_onto_it() {
        let shrink = Shrink {
            size: 17.25,
            floor: 9.775,
        };
        assert_eq!(search(shrink, 17.25, 17.0, 100.0).0, 17.0);
        assert_eq!(search(shrink, 17.25, 12.0, 13.1).0, 13.0);
    }
}
