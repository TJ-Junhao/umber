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
//! out. Panel headers are fitted in every solve of the grid, once the columns are placed and
//! before the rows are ([`fitted`]): a header box spans the plot and the y tick label column,
//! whose widths do not depend on the header, so each text is measured against its final width
//! once, and the rows, the height checks included, reserve the height of the size it takes.
//! The first solve of a figure has ticks guessed for a plot size, whose widths need not be the
//! final ones; when it refuses the height, it is tried again with the titles at their floors
//! ([`PanelFit::floors`]) for a provisional plot, and only the solves for the final ticks
//! refuse a height for the size the titles are fitted to.
//! A figure with equal aspect keeps the rooms of its reference layout, and with them the header
//! boxes and the sizes its titles take there, which its rows reserve; only where the header
//! starts in its box follows the figure's own y tick labels ([`super::grid::rehead`]). So such
//! a title keeps the reference's size even when the final box would hold a larger one.

use std::borrow::Cow;

use super::inset::CONTAINMENT_TOLERANCE;
use super::panel::{Header, HeaderText, PanelBands};
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

/// The size a panel title or subtitle without an explicit size is set at.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) enum TextFit {
    /// Its own size: it fits.
    #[default]
    Own,
    /// At most this many points.
    AtMost(f32),
}

impl TextFit {
    /// The smaller of this fit and `size`.
    fn at_most(self, size: f32) -> TextFit {
        match self {
            TextFit::Own => TextFit::AtMost(size),
            TextFit::AtMost(most) => TextFit::AtMost(most.min(size)),
        }
    }
}

/// The sizes of the panel titles and of the panel subtitles of a figure, each shared by every
/// panel whose text has no explicit size.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct PanelFit {
    /// The panel titles.
    pub title: TextFit,
    /// The panel subtitles.
    pub subtitle: TextFit,
}

impl PanelFit {
    fn of(self, kind: HeaderText) -> TextFit {
        match kind {
            HeaderText::Title => self.title,
            HeaderText::Subtitle => self.subtitle,
        }
    }
}

impl PanelFit {
    /// Every panel title and subtitle without an explicit size at its floor, the least it
    /// shrinks to under `theme`: for a provisional layout that must not refuse a height that
    /// the titles may yet leave once they are fitted.
    pub(crate) fn floors(theme: &Theme) -> PanelFit {
        PanelFit {
            title: TextFit::AtMost(floor(theme, HeaderText::Title, TextRole::Body)),
            subtitle: TextFit::AtMost(floor(theme, HeaderText::Subtitle, TextRole::Body)),
        }
    }
}

/// Whether the text `kind` of the panel of `input` has no explicit size, and so may shrink.
fn shrinks(input: &LayoutInput<'_>, kind: HeaderText) -> bool {
    let asked = match kind {
        HeaderText::Title => input.axes.title_size(),
        HeaderText::Subtitle => input.axes.subtitle_size(),
    };
    asked == SizeSpec::Auto
}

/// The sizes the titles and subtitles of the panels of `inputs`, measured in `bands`, take in
/// header boxes `rooms` wide (each in the same order): the panel titles without an explicit
/// size all take the smallest size any of them needs, and so do the panel subtitles.
///
/// # Errors
///
/// As [`Header::fits_at`].
fn fit_panel_headers(
    inputs: &[LayoutInput<'_>],
    bands: &[PanelBands],
    rooms: &[f64],
) -> Result<PanelFit> {
    let mut fit = PanelFit::default();
    for kind in [HeaderText::Title, HeaderText::Subtitle] {
        let mut least = TextFit::Own;
        for ((input, bands), &room) in inputs.iter().zip(bands).zip(rooms) {
            if !shrinks(input, kind) {
                continue;
            }
            let floor = floor(input.theme, kind, TextRole::Body);
            if let Some(size) = needed(&bands.heading, kind, input.lib, room, floor)? {
                least = least.at_most(size);
            }
        }
        match kind {
            HeaderText::Title => fit.title = least,
            HeaderText::Subtitle => fit.subtitle = least,
        }
    }
    Ok(fit)
}

/// Sets the titles and subtitles without an explicit size of the panels of `inputs`, measured
/// in `bands` (in the same order), at the sizes of `fit`, so that their headers take the
/// height of those sizes.
///
/// # Errors
///
/// As [`Header::resize`].
pub(crate) fn apply_panel_fit(
    inputs: &[LayoutInput<'_>],
    bands: &mut [PanelBands],
    fit: PanelFit,
) -> Result<()> {
    for kind in [HeaderText::Title, HeaderText::Subtitle] {
        let TextFit::AtMost(most) = fit.of(kind) else {
            continue;
        };
        for (input, bands) in inputs.iter().zip(&mut *bands) {
            if shrinks(input, kind)
                && let Some((_, size)) = bands.heading.width_and_size(kind)
                && most < size
            {
                bands.heading.resize(kind, input.lib, most)?;
            }
        }
    }
    Ok(())
}

/// `bands` with the panel titles and subtitles of `inputs` fitted to header boxes `rooms`
/// wide (see [`fit_panel_headers`]), and the sizes they take; `bands` itself when nothing
/// shrinks.
///
/// # Errors
///
/// As [`fit_panel_headers`] and [`apply_panel_fit`].
pub(super) fn fitted<'b>(
    inputs: &[LayoutInput<'_>],
    bands: &'b [PanelBands],
    rooms: &[f64],
) -> Result<(Cow<'b, [PanelBands]>, PanelFit)> {
    let fit = fit_panel_headers(inputs, bands, rooms)?;
    if fit == PanelFit::default() {
        return Ok((Cow::Borrowed(bands), fit));
    }
    let mut owned = bands.to_vec();
    apply_panel_fit(inputs, &mut owned, fit)?;
    Ok((Cow::Owned(owned), fit))
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
