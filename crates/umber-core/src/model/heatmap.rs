//! Heatmaps: a grid of cells on two category axes, one per x and y category, each colored by
//! its value through the color scale of the axes.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use super::color_by::{CmapChoice, check_center, check_mapped_values};
use super::figure::Figure;
use super::mark::Mark;
use super::render::Render;
use crate::color_scale::ColorScaleSpec;
use crate::{Error, NumberStyle, Result, ScaleKind, TickFormat, Transform};

/// The hint of a heatmap whose values do not have one row per y category.
const ROWS_HINT: &str = "pass one row per y category.";

/// Options of a heatmap.
///
/// # Examples
///
/// ```
/// use umber_core::{CmapChoice, HeatmapOptions, Render, Transform};
///
/// let options = HeatmapOptions::default();
/// assert_eq!((&options.cmap, options.scale, options.center), (&CmapChoice::Theme, Transform::Linear, None));
/// assert!(!options.annotate);
/// assert_eq!(options.format, None);
/// assert_eq!(options.render, Render::Auto);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct HeatmapOptions {
    /// The colormap; [`CmapChoice::Theme`] for the theme's diverging colormap when `center` is
    /// set and its sequential colormap otherwise.
    pub cmap: CmapChoice,
    /// How values map to the colormap: linearly, or by their logarithm.
    pub scale: Transform,
    /// The value at the middle of a diverging colormap; `None` for a sequential mapping.
    pub center: Option<f64>,
    /// Whether each cell shows its value, centered in it.
    pub annotate: bool,
    /// The format of the values shown on annotated cells; `None` for the y tick format. Only
    /// annotated heatmaps take one.
    pub format: Option<TickFormat>,
    /// Whether the cells are drawn as vector shapes or as one image (see [`Render`]). The
    /// values of annotated cells are text over the cells either way.
    pub render: Render,
}

impl Default for HeatmapOptions {
    fn default() -> Self {
        HeatmapOptions {
            cmap: CmapChoice::Theme,
            scale: Transform::Linear,
            center: None,
            annotate: false,
            format: None,
            render: Render::Auto,
        }
    }
}

/// A heatmap: its x and y categories, one value per pair of them, and its [`HeatmapOptions`].
///
/// The values are stored row by row, one row per y category, each row holding one value per x
/// category. A NaN value leaves its cell empty. The categories and values are validated together
/// and never change; clones share them.
///
/// # Examples
///
/// ```
/// use umber_core::{Figure, HeatmapOptions, Mark, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = figure.add_axes(ScaleKind::Category, ScaleKind::Category)?;
/// let (x, y) = (["Mon".to_owned(), "Tue".to_owned()], ["Day".to_owned()]);
/// axes.heatmap(&x, &y, &[3.0, 5.0], HeatmapOptions::default())?;
/// let Some(Mark::Heatmap(heatmap)) = axes.marks().first() else { unreachable!() };
/// assert_eq!(heatmap.x_categories(), ["Mon", "Tue"]);
/// assert_eq!(heatmap.y_categories(), ["Day"]);
/// assert_eq!(heatmap.values(), [3.0, 5.0]);
/// # Ok::<(), umber_core::Error>(())
/// ```
///
/// A heatmap cannot be built outside [`Axes`](super::Axes):
///
/// ```compile_fail,E0451
/// use umber_core::{HeatmapMark, HeatmapOptions};
///
/// let mark = HeatmapMark {
///     x: unreachable!(),
///     y: unreachable!(),
///     values: unreachable!(),
///     options: HeatmapOptions::default(),
/// };
/// ```
#[derive(Clone, PartialEq)]
pub struct HeatmapMark {
    x: Arc<[String]>,
    y: Arc<[String]>,
    values: Arc<[f64]>,
    options: HeatmapOptions,
}

impl HeatmapMark {
    /// Validates a heatmap for `axes` under `context`, before it is stored, so a failed call
    /// changes nothing, then stores it; the stored values share the allocation of `values`. The
    /// checks run in this order, and the first failure is the error:
    ///
    /// 1. both axes are category axes (`Error::Usage`, naming `x` or `y`);
    /// 2. no x category and no y category appears twice (`Error::Data`);
    /// 3. `values` has `y.len() * x.len()` values (`Error::Data`);
    /// 4. every value is NaN or finite, at most 1e300 in magnitude, and positive and at least
    ///    1e-300 on a log scale (`Error::Data`, naming the value by row and column);
    /// 5. the center is finite, at most 1e300 in magnitude, and not combined with a log scale
    ///    (`Error::Style`);
    /// 6. a format comes with `annotate` (`Error::Usage`) and is a number format
    ///    (`Error::Style`);
    /// 7. the colormap, scale and center are those of the axes' other color-mapped marks
    ///    (`Error::Usage`, naming the first that differs);
    /// 8. the color limits of the axes are positive on a log scale and hold the center
    ///    (`Error::Style`);
    /// 9. the panel stays within one color-sharing group and agrees with its other members
    ///    (`Error::Usage`).
    pub(super) fn new(
        context: &'static str,
        figure: &Figure,
        index: usize,
        (x, y): (Vec<String>, Vec<String>),
        values: Arc<[f64]>,
        options: HeatmapOptions,
    ) -> Result<HeatmapMark> {
        let axes = figure.panel(index);
        check_axis(context, "x", axes.x)?;
        check_axis(context, "y", axes.y)?;
        check_unique(
            context,
            "x_categories",
            &x,
            "give every column its own name.",
        )?;
        check_unique(context, "y_categories", &y, "give every row its own name.")?;
        let (columns, rows) = (x.len(), y.len());
        if columns.checked_mul(rows) != Some(values.len()) {
            let cells = (columns as u128) * (rows as u128);
            return Err(Error::data(
                context,
                format!(
                    "values has {} values; it must have len(y_categories) * len(x_categories) \
                     = {rows} * {columns} = {cells}.",
                    values.len()
                ),
            )
            .with_parameter("values")
            .with_hint(ROWS_HINT));
        }
        check_mapped_values(
            context,
            &values,
            options.scale,
            |i| format!("values[{}][{}]", i / columns, i % columns),
            "use NaN to leave a cell empty.",
        )?;
        check_center(context, options.center, options.scale)?;
        check_format(context, &options)?;
        let mark = HeatmapMark {
            x: Arc::from(x),
            y: Arc::from(y),
            values,
            options,
        };
        let spec = mark.color_scale();
        if let Some(existing) = axes.marks.iter().find_map(Mark::color_scale) {
            spec.check_same_arguments(&existing, context)?;
        }
        if let Some(limits) = axes.color_limits {
            // A log scale is the argument `scale` that the limits refuse; a center outside them
            // is the argument `center`. A centered scale is linear, so only one can fail.
            ColorScaleSpec {
                center: None,
                ..spec.clone()
            }
            .check_range(limits, context, Some("scale"))?;
            spec.check_range(limits, context, Some("center"))?;
        }
        figure.check_shared_color(index, &spec, context, None)?;
        Ok(mark)
    }

    /// The x categories, left to right, each appearing once.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, HeatmapOptions, Mark, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Category, ScaleKind::Category)?;
    /// axes.heatmap(&["a".to_owned()], &["b".to_owned()], &[1.0], HeatmapOptions::default())?;
    /// let Some(Mark::Heatmap(heatmap)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(heatmap.x_categories(), ["a"]);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn x_categories(&self) -> &[String] {
        &self.x
    }

    /// The y categories, top to bottom, each appearing once.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, HeatmapOptions, Mark, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Category, ScaleKind::Category)?;
    /// axes.heatmap(&["a".to_owned()], &["b".to_owned()], &[1.0], HeatmapOptions::default())?;
    /// let Some(Mark::Heatmap(heatmap)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(heatmap.y_categories(), ["b"]);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn y_categories(&self) -> &[String] {
        &self.y
    }

    /// The values row by row: one row per y category, each holding one value per x category.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, HeatmapOptions, Mark, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Category, ScaleKind::Category)?;
    /// let x = ["a".to_owned(), "b".to_owned()];
    /// axes.heatmap(&x, &["r".to_owned()], &[1.0, f64::NAN], HeatmapOptions::default())?;
    /// let Some(Mark::Heatmap(heatmap)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(heatmap.values()[0], 1.0);
    /// assert!(heatmap.values()[1].is_nan());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn values(&self) -> &[f64] {
        &self.values
    }

    /// The colormap, scale, center, annotation, format and how the cells are drawn.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, HeatmapOptions, Mark, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Category, ScaleKind::Category)?;
    /// let options = HeatmapOptions { annotate: true, ..HeatmapOptions::default() };
    /// axes.heatmap(&["a".to_owned()], &["b".to_owned()], &[1.0], options.clone())?;
    /// let Some(Mark::Heatmap(heatmap)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(heatmap.options(), &options);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn options(&self) -> &HeatmapOptions {
        &self.options
    }

    /// The colormap, scale and center the heatmap maps its values with.
    pub(crate) fn color_scale(&self) -> ColorScaleSpec {
        ColorScaleSpec {
            cmap: self.options.cmap.clone(),
            scale: self.options.scale,
            center: self.options.center,
        }
    }
}

impl fmt::Debug for HeatmapMark {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HeatmapMark")
            .field("x", &self.x)
            .field("y", &self.y)
            .field("values", &self.values)
            .field("options", &self.options)
            .finish()
    }
}

/// Checks that the axis `axis` (`"x"` or `"y"`), of scale kind `kind`, is a category axis, as a
/// heatmap needs on both sides: `Error::Usage` under `context` naming the axis.
fn check_axis(context: &'static str, axis: &'static str, kind: ScaleKind) -> Result<()> {
    let scale = match kind {
        ScaleKind::Category => return Ok(()),
        ScaleKind::Linear => "linear",
        ScaleKind::Log { .. } => "log",
        ScaleKind::Time => "time",
    };
    Err(Error::usage(
        context,
        format!("{axis} is a {scale} scale; {context} needs a category {axis} axis."),
    )
    .with_parameter(axis)
    .with_hint("add the axes with x=Scale.category() and y=Scale.category()."))
}

/// Checks that no name of `categories`, the argument `parameter`, appears twice: `Error::Data`
/// under `context` naming the repeat and its first appearance, with `hint`.
fn check_unique(
    context: &'static str,
    parameter: &'static str,
    categories: &[String],
    hint: &'static str,
) -> Result<()> {
    let mut first_index: HashMap<&str, usize> = HashMap::with_capacity(categories.len());
    for (i, name) in categories.iter().enumerate() {
        if let Some(first) = first_index.insert(name, i) {
            return Err(Error::data(
                context,
                format!(
                    "{parameter}[{i}] repeats {} from {parameter}[{first}]; each category must \
                     appear once.",
                    crate::error::quoted(name)
                ),
            )
            .with_parameter(parameter)
            .with_hint(hint));
        }
    }
    Ok(())
}

/// Checks the format of `options`: only an annotated heatmap shows values, so a format without
/// `annotate` would be dropped (`Error::Usage`), and the values are numbers, which
/// [`TickFormat::iso`] does not write (`Error::Style`). Both about the argument `format`.
fn check_format(context: &'static str, options: &HeatmapOptions) -> Result<()> {
    let Some(format) = &options.format else {
        return Ok(());
    };
    if !options.annotate {
        return Err(Error::usage(
            context,
            "format is given but annotate is false; only annotated cells show their values.",
        )
        .with_parameter("format")
        .with_hint("pass annotate=True, or leave format out."));
    }
    if format.style() == NumberStyle::Iso {
        return Err(Error::style(
            context,
            "format is TickFormat.iso(), which writes times; heatmap values are numbers.",
        )
        .with_parameter("format")
        .with_hint("use TickFormat.AUTO or a number format such as TickFormat.fixed(2)."));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Figure;
    use crate::{Cmap, LogBase, Theme};

    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    fn two_categories() -> Figure {
        let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        f.add_axes(ScaleKind::Category, ScaleKind::Category)
            .unwrap();
        f
    }

    /// Adds a heatmap to the one axes of `f` and asserts that a failure leaves `f` unchanged.
    fn add(
        f: &mut Figure,
        x: &[&str],
        y: &[&str],
        values: &[f64],
        options: HeatmapOptions,
    ) -> Result<()> {
        let before = f.clone();
        let result = f
            .axes_at_mut(crate::GridIndex::Auto, crate::GridIndex::Auto)
            .unwrap()
            .heatmap(&names(x), &names(y), values, options)
            .map(drop);
        if result.is_err() {
            assert_eq!(*f, before, "a failed call changed the figure");
        }
        result
    }

    #[test]
    fn shape_error_exact() {
        let mut f = two_categories();
        let err = add(
            &mut f,
            &["a", "b", "c"],
            &["p", "q", "r", "s"],
            &[0.0; 11],
            HeatmapOptions::default(),
        )
        .unwrap_err();
        assert!(matches!(err, Error::Data { .. }));
        assert_eq!(
            err.to_string(),
            "Axes.heatmap(): values has 11 values; it must have len(y_categories) * \
             len(x_categories) = 4 * 3 = 12."
        );
        assert_eq!(err.parameter(), Some("values"));
        assert_eq!(err.hint(), Some(ROWS_HINT));
        assert!(
            add(
                &mut f,
                &["a", "b", "c"],
                &["p", "q", "r", "s"],
                &[0.0; 12],
                HeatmapOptions::default()
            )
            .is_ok()
        );
    }

    #[test]
    fn heatmap_needs_two_category_axes() {
        for (x, y, axis, scale) in [
            (ScaleKind::Linear, ScaleKind::Category, "x", "linear"),
            (ScaleKind::Category, ScaleKind::Linear, "y", "linear"),
            (ScaleKind::Time, ScaleKind::Category, "x", "time"),
            (
                ScaleKind::Category,
                ScaleKind::Log { base: LogBase::TEN },
                "y",
                "log",
            ),
        ] {
            let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
            f.add_axes(x, y).unwrap();
            // The axes before the data: these values have the wrong length too.
            let err = add(&mut f, &["a"], &["b"], &[], HeatmapOptions::default()).unwrap_err();
            assert!(matches!(err, Error::Usage { .. }), "{err:?}");
            assert_eq!(
                err.to_string(),
                format!(
                    "Axes.heatmap(): {axis} is a {scale} scale; Axes.heatmap() needs a category \
                     {axis} axis."
                )
            );
            assert_eq!(err.parameter(), Some(axis));
        }
    }

    #[test]
    fn repeated_categories_are_named_by_index() {
        let mut f = two_categories();
        let err = add(
            &mut f,
            &["a", "b", "a"],
            &["r"],
            &[0.0; 3],
            HeatmapOptions::default(),
        )
        .unwrap_err();
        assert!(matches!(err, Error::Data { .. }));
        assert_eq!(
            err.to_string(),
            "Axes.heatmap(): x_categories[2] repeats \"a\" from x_categories[0]; each category \
             must appear once."
        );
        assert_eq!(err.parameter(), Some("x_categories"));
        let err = add(
            &mut f,
            &["a"],
            &["r", "r"],
            &[0.0; 2],
            HeatmapOptions::default(),
        )
        .unwrap_err();
        assert_eq!(err.parameter(), Some("y_categories"));
        assert!(
            err.to_string()
                .contains("y_categories[1] repeats \"r\" from y_categories[0]")
        );
    }

    #[test]
    fn values_are_named_by_row_and_column() {
        let mut f = two_categories();
        let err = add(
            &mut f,
            &["a", "b", "c"],
            &["r", "s"],
            &[0.0, 1.0, f64::NAN, 2.0, f64::INFINITY, 3.0],
            HeatmapOptions::default(),
        )
        .unwrap_err();
        assert!(matches!(err, Error::Data { .. }));
        assert_eq!(
            err.to_string(),
            "Axes.heatmap(): values[1][1] is inf; values must be finite."
        );
        assert_eq!(err.hint(), Some("use NaN to leave a cell empty."));
        let log = HeatmapOptions {
            scale: Transform::Log(LogBase::TEN),
            ..HeatmapOptions::default()
        };
        let err = add(&mut f, &["a", "b"], &["r"], &[1.0, 0.0], log).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Axes.heatmap(): values[0][1] is 0; values on a log scale must be positive."
        );
        assert_eq!(err.parameter(), Some("values"));
    }

    #[test]
    fn center_and_format_are_checked() {
        let mut f = two_categories();
        let err = add(
            &mut f,
            &["a"],
            &["r"],
            &[1.0],
            HeatmapOptions {
                center: Some(f64::NAN),
                ..HeatmapOptions::default()
            },
        )
        .unwrap_err();
        assert!(matches!(err, Error::Style { .. }));
        assert_eq!(err.parameter(), Some("center"));
        let err = add(
            &mut f,
            &["a"],
            &["r"],
            &[1.0],
            HeatmapOptions {
                format: Some(TickFormat::fixed(1).unwrap()),
                ..HeatmapOptions::default()
            },
        )
        .unwrap_err();
        assert!(matches!(err, Error::Usage { .. }));
        assert_eq!(
            err.to_string(),
            "Axes.heatmap(): format is given but annotate is false; only annotated cells show \
             their values."
        );
        assert_eq!(err.parameter(), Some("format"));
        let err = add(
            &mut f,
            &["a"],
            &["r"],
            &[1.0],
            HeatmapOptions {
                annotate: true,
                format: Some(TickFormat::iso()),
                ..HeatmapOptions::default()
            },
        )
        .unwrap_err();
        assert!(matches!(err, Error::Style { .. }));
        assert_eq!(err.parameter(), Some("format"));
    }

    #[test]
    fn a_second_heatmap_keeps_the_color_scale() {
        let mut f = two_categories();
        let viridis = HeatmapOptions {
            cmap: CmapChoice::Fixed(Cmap::viridis()),
            ..HeatmapOptions::default()
        };
        add(&mut f, &["a"], &["r"], &[1.0], viridis.clone()).unwrap();
        let err = add(
            &mut f,
            &["b"],
            &["r"],
            &[2.0],
            HeatmapOptions {
                cmap: CmapChoice::Fixed(Cmap::magma()),
                ..viridis.clone()
            },
        )
        .unwrap_err();
        assert!(matches!(err, Error::Usage { .. }));
        assert_eq!(
            err.to_string(),
            "Axes.heatmap(): cmap is Cmap.MAGMA; this axes already maps colors with Cmap.VIRIDIS."
        );
        assert_eq!(err.parameter(), Some("cmap"));
        let err = add(
            &mut f,
            &["b"],
            &["r"],
            &[2.0],
            HeatmapOptions {
                center: Some(0.0),
                ..viridis.clone()
            },
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "Axes.heatmap(): center is 0; this axes already maps colors with no center."
        );
        assert_eq!(err.parameter(), Some("center"));
        assert!(add(&mut f, &["b"], &["r"], &[2.0], viridis).is_ok());
    }

    #[test]
    fn color_limits_are_checked_against_the_arguments() {
        let mut f = two_categories();
        f.axes_at_mut(crate::GridIndex::Auto, crate::GridIndex::Auto)
            .unwrap()
            .clim(0.0, 10.0)
            .unwrap();
        let err = add(
            &mut f,
            &["a"],
            &["r"],
            &[1.0],
            HeatmapOptions {
                scale: Transform::Log(LogBase::TEN),
                ..HeatmapOptions::default()
            },
        )
        .unwrap_err();
        assert!(matches!(err, Error::Style { .. }));
        assert_eq!(err.parameter(), Some("scale"));
        let err = add(
            &mut f,
            &["a"],
            &["r"],
            &[1.0],
            HeatmapOptions {
                center: Some(12.0),
                ..HeatmapOptions::default()
            },
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "Axes.heatmap(): center is 12; it must lie inside the color range 0 to 10."
        );
        assert_eq!(err.parameter(), Some("center"));
    }

    #[test]
    fn other_marks_still_refuse_two_category_axes() {
        let mut f = two_categories();
        let mut axes = f
            .axes_at_mut(crate::GridIndex::Auto, crate::GridIndex::Auto)
            .unwrap();
        let err = axes
            .scatter(
                crate::AxisData::Categories(names(&["a"])),
                &[1.0],
                crate::ScatterOptions::default(),
            )
            .unwrap_err();
        assert!(matches!(err, Error::Usage { .. }));
    }
}
