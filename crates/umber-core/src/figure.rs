//! The figure model and the calls that render it.
//!
//! A [`Figure`] holds one [`Axes`], which holds [`Mark`]s. Every input is validated at the call
//! that receives it, so a model that exists is always drawable. Marks own copies of their data.
//! Colors stay as [`ColorSpec`](crate::ColorSpec)s until [`Figure::resolved_colors`] resolves them against the
//! theme's palette. [`Figure::to_scene`], [`Figure::to_png`] and [`Figure::to_svg`] run the
//! render pipeline: resolve scales and ticks, lay out the axes, then draw.
//!
//! # Examples
//!
//! ```
//! use umber_core::figure::{AxisData, Figure, LineOptions};
//! use umber_core::{ScaleKind, Theme};
//!
//! let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
//! let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
//! axes.title("Growth")
//!     .line(AxisData::Numbers(vec![0.0, 1.0, 2.0]), &[1.0, 3.0, 2.0], LineOptions::default())?;
//! assert_eq!(figure.axes().map(|a| a.marks().len()), Some(1));
//! # Ok::<(), umber_core::Error>(())
//! ```

pub use crate::model::{Axes, AxisData, Figure, LineOptions, Mark, ScatterOptions};

use crate::render::{SvgText, render_png, render_svg};
use crate::scene::Scene;
use crate::{FontLibrary, Result, Rgba, pipeline, resolve};

impl Figure {
    /// Each mark's color, resolved against the theme's palette, in mark order. `Auto` colors
    /// take palette slots in series order; `Palette` and `Fixed` colors do not consume a slot.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::{AxisData, LineOptions};
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.line(AxisData::Numbers(vec![0.0]), &[1.0], LineOptions::default())?;
    /// assert_eq!(figure.resolved_colors(), vec![figure.theme().palette()[0]]);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn resolved_colors(&self) -> Vec<Rgba> {
        let Some(axes) = self.axes() else {
            return Vec::new();
        };
        resolve::mark_colors(axes.marks(), self.theme().palette())
    }

    /// Builds the figure's [`Scene`]: resolve scales and ticks, lay out the axes, then draw.
    /// The scene is `width × 72` by `height × 72` pt on the theme background. It holds the
    /// axes chrome as one unclipped group, followed by the visible marks, in insertion order,
    /// as one group clipped to the plot area. Without axes it holds the background only. The
    /// same figure always gives the same scene.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, FontLibrary, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// assert!(figure.to_scene(FontLibrary::global()).items.is_empty());
    /// figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?.title("Growth");
    /// let scene = figure.to_scene(FontLibrary::global());
    /// assert_eq!((scene.width, scene.height), (432.0, 288.0));
    /// assert!(scene.leaf_count() > 0);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn to_scene(&self, library: &FontLibrary) -> Scene {
        pipeline::scene(self, library)
    }

    /// Renders the figure as a PNG at `dpi`, with the global [`FontLibrary`]. Errors as
    /// [`render_png`] does: `Error::Style` when `dpi` is outside `10..=2400` or the image would
    /// be too large.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(2.0, 1.0, Theme::graphite())?;
    /// figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// let png = figure.to_png(72)?;
    /// assert_eq!(&png[1..4], b"PNG");
    /// assert!(figure.to_png(1).is_err());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn to_png(&self, dpi: u32) -> Result<Vec<u8>> {
        render_png(&self.to_scene(FontLibrary::global()), dpi)
    }

    /// Renders the figure as an SVG document, with the global [`FontLibrary`].
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, SvgText, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::midnight())?;
    /// figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?.title("Signal");
    /// let svg = figure.to_svg(SvgText::Editable);
    /// assert!(svg.contains(r#"width="432pt" height="288pt""#));
    /// assert!(svg.contains(">Signal</text>"));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn to_svg(&self, text: SvgText) -> String {
        render_svg(&self.to_scene(FontLibrary::global()), text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Alpha, ColorSpec, ScaleKind, Theme};

    fn figure() -> Figure {
        Figure::new(6.0, 4.0, Theme::pigment()).unwrap()
    }

    fn nums(v: &[f64]) -> AxisData {
        AxisData::Numbers(v.to_vec())
    }

    #[test]
    fn auto_colors_follow_series_order() {
        let mut f = figure();
        let palette = f.theme().palette().to_vec();
        let fixed = Rgba::opaque(1, 2, 3);
        let a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        let specs = [
            ColorSpec::AUTO,
            ColorSpec::Fixed(fixed),
            ColorSpec::AUTO,
            ColorSpec::Palette {
                index: 5,
                alpha: Alpha::OPAQUE,
            },
            ColorSpec::AUTO,
        ];
        for color in specs {
            a.line(
                nums(&[0.0]),
                &[1.0],
                LineOptions {
                    color,
                    ..LineOptions::default()
                },
            )
            .unwrap();
        }
        assert_eq!(
            f.resolved_colors(),
            vec![palette[0], fixed, palette[1], palette[5], palette[2]]
        );
    }

    #[test]
    fn auto_colors_count_lines_and_scatters_together() {
        let mut f = figure();
        let palette = f.theme().palette().to_vec();
        let a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        a.scatter(nums(&[0.0]), &[1.0], ScatterOptions::default())
            .unwrap()
            .line(nums(&[0.0]), &[1.0], LineOptions::default())
            .unwrap();
        assert_eq!(f.resolved_colors(), vec![palette[0], palette[1]]);
        assert!(figure().resolved_colors().is_empty());
    }

    #[test]
    fn alpha_on_auto_is_applied() {
        let mut f = figure();
        let first = f.theme().palette()[0];
        let alpha = Alpha::new(0.5).unwrap();
        let a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        a.line(
            nums(&[0.0]),
            &[1.0],
            LineOptions {
                color: ColorSpec::AUTO.with_alpha(alpha),
                ..LineOptions::default()
            },
        )
        .unwrap();
        assert_eq!(f.resolved_colors(), vec![first.with_alpha(alpha)]);
        assert_eq!(f.resolved_colors()[0].a, alpha.to_u8());
    }
}
