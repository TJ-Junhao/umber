//! Contrast with the background for color-mapped marks whose colors can come close to it.
//!
//! A colormap may pass close to the theme background (a centered diverging colormap's midpoint
//! does in every theme), and a mark drawn in such a color barely shows. In Oklab, let `d` be the
//! Euclidean distance from the mark's color, taken opaque, to the background, and
//! `w = clamp(1 − d / 0.25, 0, 1)` its weight ([`CONTRAST_DISTANCE`]). A mark with `w > 0` moves
//! its color's Oklab lightness away from the background, down when the background's lightness
//! is at least 0.5 and up otherwise, a and b unchanged. Color-mapped scatter points and bubbles
//! take the color moved by the full 0.25 ([`CONTRAST_LIGHTNESS`]), mixed with the background by
//! `w`, for their outline ([`Contrast::full`]); color-mapped contour lines, arrows and wind barbs
//! are drawn in their color moved by `w × 0.25` ([`Contrast::weighted`]).
//!
//! Text is judged by the WCAG 2 contrast ratio instead ([`relative_luminance`]): the values
//! written on cells and bars take black or white by it, and the emphasis color of each preset
//! theme reaches 4.5:1 against its background. Text written in a series color (direct labels,
//! point labels, the value column) takes [`readable_text`] of it: the color itself when it
//! reaches [`TEXT_CONTRAST`] against the background, else its Oklab lightness moved away from
//! the background, a and b unchanged, by the smallest step that does. A contour's level label on
//! filled bands takes the theme's text or background color, whichever has the higher lowest
//! ratio against the bands painted under it ([`super::contour`]). Marks keep their colors.

use crate::cmap::{Oklab, from_oklab, to_oklab};
use crate::{Rgba, Theme};

/// The WCAG 2 relative luminance of `color`'s sRGB channels, from 0 (black) to 1 (white).
pub(super) fn relative_luminance(color: Rgba) -> f64 {
    let linear = |channel: u8| {
        let c = f64::from(channel) / 255.0;
        if c <= 0.040_45 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b)
}

/// The WCAG 2 contrast ratio of the opaque colors `a` and `b`, from 1 (equal luminance) to 21
/// (black on white): the lighter luminance plus 0.05 over the darker plus 0.05.
pub(super) fn contrast_ratio(a: Rgba, b: Rgba) -> f64 {
    let (a, b) = (relative_luminance(a), relative_luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// The WCAG 2 contrast ratio against the background that text needs.
pub(super) const TEXT_CONTRAST: f64 = 4.5;
/// The number of equal steps of one unit of Oklab lightness in the search of [`readable_text`].
const TEXT_STEPS: u32 = 1024;

/// The color for text written in the series color `color` on the theme's background: always
/// opaque, since text in a series color is drawn at full opacity. When `color`'s channels
/// already reach [`TEXT_CONTRAST`] against the background they are returned unchanged, bit for
/// bit. Otherwise its Oklab lightness moves away from the background (down when the
/// background's lightness is at least 0.5, up otherwise), a and b unchanged, by the smallest
/// multiple of `1 / 1024` that reaches [`TEXT_CONTRAST`], staying in the sRGB gamut by clamping
/// each channel as [`from_oklab`] does. When that direction cannot reach it, as on a mid-gray
/// background where black falls short, the other direction is searched the same way; on an
/// opaque background black or white always reaches it.
pub(super) fn readable_text(color: Rgba, theme: &Theme) -> Rgba {
    let background = theme.background();
    let color = Rgba { a: 255, ..color };
    match readable_step(color, background) {
        None => color,
        Some((step, lightness, chroma, darker)) => text_step(lightness, chroma, darker, step),
    }
}

/// How far [`readable_text`] moves `color` on `background`: `None` when it needs no move, else
/// the number of `1 / 1024` steps, the color's Oklab lightness and `[a, b]`, and whether the
/// move is down.
fn readable_step(color: Rgba, background: Rgba) -> Option<(u32, f64, [f64; 2], bool)> {
    if contrast_ratio(color, background) >= TEXT_CONTRAST {
        return None;
    }
    let [lightness, a, b] = to_oklab(color);
    let preferred = to_oklab(background)[0] >= 0.5;
    let first_passing = |darker: bool| {
        let room = if darker { lightness } else { 1.0 - lightness };
        let last = (room * f64::from(TEXT_STEPS)).ceil() as u32;
        (1..=last).find_map(|step| {
            let moved = text_step(lightness, [a, b], darker, step);
            (contrast_ratio(moved, background) >= TEXT_CONTRAST).then_some((
                step,
                lightness,
                [a, b],
                darker,
            ))
        })
    };
    // Black and white end the two searches, and one of them reaches the contrast on any opaque
    // background, so the second search finds a step when the first does not. Should neither
    // find one, the answer is whichever of black and white has the higher contrast.
    first_passing(preferred)
        .or_else(|| first_passing(!preferred))
        .or_else(|| {
            let black = contrast_ratio(Rgba::opaque(0, 0, 0), background);
            let white = contrast_ratio(Rgba::opaque(255, 255, 255), background);
            Some((TEXT_STEPS, 0.5, [0.0, 0.0], black >= white))
        })
}

/// The opaque color at Oklab lightness `lightness` moved by `step / 1024` down (`darker`) or up,
/// within `0..=1`, with chroma `[a, b]`.
fn text_step(lightness: f64, [a, b]: [f64; 2], darker: bool, step: u32) -> Rgba {
    let shift = f64::from(step) / f64::from(TEXT_STEPS);
    let moved = if darker {
        lightness - shift
    } else {
        lightness + shift
    };
    from_oklab([moved.clamp(0.0, 1.0), a, b])
}

/// The Oklab distance from the background at which a color-mapped mark needs no contrast.
const CONTRAST_DISTANCE: f64 = 0.25;
/// The largest Oklab lightness shift of a contrasting color away from the background.
const CONTRAST_LIGHTNESS: f64 = 0.25;

/// The theme background, with its Oklab conversion shared by every color compared with it.
#[derive(Debug, Clone, Copy)]
pub(super) struct Background {
    /// The background color.
    color: Rgba,
    /// Its Oklab coordinates.
    lab: Oklab,
}

impl Background {
    /// The background of `theme`.
    pub(super) fn of(theme: &Theme) -> Self {
        let color = theme.background();
        Self {
            color,
            lab: to_oklab(color),
        }
    }

    /// The background color.
    pub(super) fn color(self) -> Rgba {
        self.color
    }

    /// The background's Oklab coordinates.
    pub(super) fn lab(self) -> Oklab {
        self.lab
    }

    /// `color` as a color-mapped line or arrow draws it: its Oklab lightness moved away from the
    /// background by [`Contrast::weighted`] when it comes close to it, and `color` itself
    /// otherwise, alpha ignored.
    pub(super) fn apart(self, color: Rgba) -> Rgba {
        self.contrast(color)
            .map_or(color, |contrast| from_oklab(contrast.weighted()))
    }

    /// The contrast `color`, taken opaque, needs against the background: `None` when it lies at
    /// least [`CONTRAST_DISTANCE`] from it in Oklab (`w = 0`).
    pub(super) fn contrast(self, color: Rgba) -> Option<Contrast> {
        let lab = to_oklab(color);
        let distance = lab
            .iter()
            .zip(self.lab)
            .map(|(color, background)| (color - background).powi(2))
            .sum::<f64>()
            .sqrt();
        let weight = (1.0 - distance / CONTRAST_DISTANCE).clamp(0.0, 1.0);
        if weight == 0.0 {
            return None;
        }
        let shift = if self.lab[0] >= 0.5 {
            -CONTRAST_LIGHTNESS
        } else {
            CONTRAST_LIGHTNESS
        };
        Some(Contrast {
            weight,
            color: lab,
            shift,
        })
    }
}

/// How a color close to the background moves away from it.
#[derive(Debug, Clone, Copy)]
pub(super) struct Contrast {
    /// `w = clamp(1 − d / 0.25, 0, 1)`, in `(0, 1]`: how close the color is to the background.
    pub(super) weight: f64,
    /// The color in Oklab.
    color: Oklab,
    /// The lightness shift away from the background: [`CONTRAST_LIGHTNESS`], negative on a
    /// light background.
    shift: f64,
}

impl Contrast {
    /// The color with its lightness moved by the full shift, before any clamp to the sRGB gamut.
    pub(super) fn full(self) -> Oklab {
        let [lightness, a, b] = self.color;
        [lightness + self.shift, a, b]
    }

    /// The color with its lightness moved by `w` times the shift and kept within `0..=1`,
    /// before any clamp to the sRGB gamut.
    pub(super) fn weighted(self) -> Oklab {
        let [lightness, a, b] = self.color;
        [(lightness + self.weight * self.shift).clamp(0.0, 1.0), a, b]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Oklab distance between two colors, taken opaque.
    fn distance(a: Rgba, b: Rgba) -> f64 {
        to_oklab(a)
            .iter()
            .zip(to_oklab(b))
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt()
    }

    #[test]
    fn contrast_ratio_spans_one_to_twenty_one() {
        assert!((contrast_ratio(Rgba::BLACK, Rgba::BLACK) - 1.0).abs() < 1e-12);
        let white = Rgba {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        };
        let black = Rgba {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        };
        assert!((contrast_ratio(black, white) - 21.0).abs() < 1e-9);
        assert_eq!(contrast_ratio(black, white), contrast_ratio(white, black));
    }

    #[test]
    fn emphasis_colors_read_as_text_and_stand_apart() {
        // Contrast ratios and Oklab distances of the presets, rounded down: the emphasis color
        // colors text, so it needs 4.5:1 against the background, and it must not be mistaken for
        // a cell of the sequential colormap or for one of the first palette colors.
        for (theme, least_ratio) in [
            (Theme::pigment(), 5.4),
            (Theme::graphite(), 5.3),
            (Theme::midnight(), 6.8),
        ] {
            let emphasis = theme.emphasis_color();
            let ratio = contrast_ratio(emphasis, theme.background());
            assert!(
                ratio >= TEXT_CONTRAST && ratio >= least_ratio,
                "{emphasis:?}: {ratio}"
            );
            let cmap = theme.cmap();
            let nearest_cell = (0..=256)
                .map(|i| distance(emphasis, cmap.at(f64::from(i) / 256.0)))
                .fold(f64::INFINITY, f64::min);
            assert!(nearest_cell > 0.2, "{emphasis:?}: {nearest_cell}");
            let nearest_series = theme.palette()[..4]
                .iter()
                .map(|&color| distance(emphasis, color))
                .fold(f64::INFINITY, f64::min);
            assert!(nearest_series > 0.07, "{emphasis:?}: {nearest_series}");
        }
    }

    /// The preset themes, with a light, a dark and a mid-tone custom background.
    fn backgrounds() -> Vec<Theme> {
        let custom = |r, g, b| {
            Theme::pigment()
                .with_background(Rgba::opaque(r, g, b))
                .unwrap()
        };
        vec![
            Theme::pigment(),
            Theme::graphite(),
            Theme::midnight(),
            custom(0xFF, 0xF4, 0xD6),
            custom(0x10, 0x20, 0x40),
        ]
    }

    #[test]
    fn series_text_reaches_the_contrast_and_keeps_what_already_does() {
        for theme in backgrounds() {
            let background = theme.background();
            for &color in theme.palette() {
                let text = readable_text(color, &theme);
                let ratio = contrast_ratio(text, background);
                assert!(
                    ratio >= TEXT_CONTRAST,
                    "{color:?} on {background:?}: {ratio}"
                );
                if contrast_ratio(color, background) >= TEXT_CONTRAST {
                    assert_eq!(text, color);
                }
            }
        }
    }

    #[test]
    fn series_text_keeps_the_hue_and_moves_away_from_the_background() {
        for theme in backgrounds() {
            let background = theme.background();
            let darker = to_oklab(background)[0] >= 0.5;
            for &color in theme.palette() {
                let (before, after) = (to_oklab(color), to_oklab(readable_text(color, &theme)));
                let text = readable_text(color, &theme);
                let clamped = [text.r, text.g, text.b].iter().any(|&c| c == 0 || c == 255);
                if clamped {
                    // A channel clamped to the gamut turns the hue a little.
                    let angle = |[_, a, b]: Oklab| b.atan2(a);
                    assert!((angle(before) - angle(after)).abs() < 0.25, "{color:?}");
                } else {
                    // Rounding to 8-bit channels moves a and b by up to about 0.004.
                    assert!(
                        (before[1] - after[1]).abs() < 0.01,
                        "{color:?}: {before:?} {after:?}"
                    );
                    assert!(
                        (before[2] - after[2]).abs() < 0.01,
                        "{color:?}: {before:?} {after:?}"
                    );
                }
                if after != before {
                    assert_eq!(after[0] < before[0], darker, "{color:?}");
                }
            }
        }
    }

    #[test]
    fn series_text_takes_the_smallest_step() {
        let mut moved = 0;
        for theme in backgrounds() {
            let background = theme.background();
            for &color in theme.palette() {
                let Some((step, lightness, [a, b], darker)) = readable_step(color, background)
                else {
                    continue;
                };
                moved += 1;
                let at = |step| text_step(lightness, [a, b], darker, step);
                assert!(contrast_ratio(at(step), background) >= TEXT_CONTRAST);
                for fewer in 0..step {
                    assert!(
                        contrast_ratio(at(fewer), background) < TEXT_CONTRAST,
                        "{color:?} on {background:?}: step {fewer} of {step} already reaches it"
                    );
                }
            }
        }
        assert!(moved > 0, "some palette color needs a move");
    }

    #[test]
    fn series_text_is_opaque_and_judged_composited() {
        let theme = Theme::graphite();
        let background = theme.background();
        let line = Rgba {
            a: 0x50,
            ..Rgba::opaque(0xF2, 0xA9, 0x00)
        };
        let text = readable_text(line, &theme);
        assert_eq!(text.a, 255);
        // Drawn at full opacity, the text is what the ratio measures.
        assert!(contrast_ratio(text, background) >= TEXT_CONTRAST);
        // An opaque color that reads comes back unchanged.
        let reads = Rgba::opaque(0x23, 0x3F, 0x86);
        assert_eq!(readable_text(reads, &theme), reads);
        assert_eq!(readable_text(Rgba { a: 10, ..reads }, &theme), reads);
    }

    #[test]
    fn series_text_on_a_mid_gray_background_takes_the_direction_that_reaches_it() {
        for gray in [0x66, 0x70, 0x77, 0x80, 0x8C, 0x99] {
            let theme = Theme::pigment()
                .with_background(Rgba::opaque(gray, gray, gray))
                .unwrap();
            for color in [
                Rgba::opaque(gray, gray, gray),
                Rgba::opaque(0xF2, 0xA9, 0x00),
                Rgba::opaque(0x4F, 0x6F, 0xA3),
            ] {
                let text = readable_text(color, &theme);
                let ratio = contrast_ratio(text, theme.background());
                assert!(ratio >= TEXT_CONTRAST, "{color:?} on {gray:#x}: {ratio}");
            }
        }
        // Black reaches only 3.66:1 on #666666, white 5.74:1.
        let theme = Theme::graphite()
            .with_background(Rgba::opaque(0x66, 0x66, 0x66))
            .unwrap();
        let text = readable_text(Rgba::opaque(0x66, 0x66, 0x66), &theme);
        assert!(to_oklab(text)[0] > 0.5, "{text:?}");
    }
}
