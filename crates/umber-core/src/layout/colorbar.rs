//! Measurement and placement of colorbars beside and below a plot.

use super::{
    AXIS_LABEL_GAP, Area, Frame, Label, Styles, Y_LABEL_CENTER, measure, measure_ticks, metrics,
};
use crate::color_scale::ResolvedColorScale;
use crate::model::Axes;
use crate::resolve::NormalizedPos;
use crate::text::SceneFonts;
use crate::{ColorbarPosition, Error, Result, Theme};

/// The gap from a tick's end to its label, in points.
const TICK_GAP: f64 = 3.0;
/// The gap below the horizontal colorbar label, in points.
const LABEL_GAP: f64 = 4.0;

/// The side occupied by a visible colorbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ColorbarSide {
    /// Beside the plot.
    Right,
    /// Below the plot.
    Bottom,
}

impl ColorbarSide {
    /// Resolves the axes override and theme default; hidden bars have no side.
    pub(crate) fn resolve(axes: &Axes, theme: &Theme) -> Option<Self> {
        Self::of(
            axes.colorbar_options()
                .position
                .unwrap_or(theme.colorbar_position()),
        )
    }

    /// A shared bar's side. The resolved default can hide any mode; visible row and column
    /// bars keep their required side, while all-panel bars follow the theme/figure choice.
    pub(crate) fn shared(figure: &crate::Figure) -> Option<Self> {
        let side = Self::of(
            figure
                .colorbar_options()
                .position
                .unwrap_or(figure.theme().colorbar_position()),
        )?;
        match figure.grid().share_color() {
            crate::Share::None => None,
            crate::Share::Row => Some(Self::Right),
            crate::Share::Col => Some(Self::Bottom),
            crate::Share::All => Some(side),
        }
    }

    /// A visible position as a side.
    fn of(position: ColorbarPosition) -> Option<Self> {
        match position {
            ColorbarPosition::Right => Some(Self::Right),
            ColorbarPosition::Bottom => Some(Self::Bottom),
            ColorbarPosition::None => None,
        }
    }
}

/// A colorbar's measured texts and fixed metrics, before its plot area is known.
#[derive(Debug, Clone)]
pub(crate) struct MeasuredColorbar {
    /// Where the bar belongs.
    pub position: ColorbarSide,
    /// Labels and their positions along the bar.
    ticks: Vec<(NormalizedPos, Label)>,
    /// The label above the bar.
    label: Option<Label>,
    /// Bar thickness in points.
    thickness: f64,
    /// Gap before the bar block.
    gap: f64,
    /// Outward tick length.
    tick_length: f64,
    /// Widest tick label.
    tick_width: f64,
    /// The tick font's ascent.
    ascent: f64,
    /// The tick font's descent.
    descent: f64,
    /// The tick font's size.
    size: f64,
    /// Height of the label and its gap.
    label_height: f64,
    /// Descent of the label font.
    label_descent: f64,
}

/// A shared group's measured bar and the grid cells it spans.
#[derive(Debug, Clone)]
pub(crate) struct SharedColorbar {
    /// The first color-mapped panel in row-major order, providing the color scale.
    pub source: usize,
    /// The row, column or whole grid whose plot edges the bar spans.
    pub cells: crate::CellRange,
    /// The measured block.
    pub measured: MeasuredColorbar,
}

/// A colorbar placed on the canvas; its ticks retain normalized positions for drawing.
#[derive(Debug, Clone)]
pub(crate) struct PlacedColorbar {
    /// Where the bar belongs.
    pub position: ColorbarSide,
    /// The gradient rectangle.
    pub bar: Area,
    /// The tick labels, placed.
    pub ticks: Vec<(NormalizedPos, Label)>,
    /// The label above the bar.
    pub label: Option<Label>,
}

impl MeasuredColorbar {
    /// Measures a visible colorbar. No finite color range or a hidden position takes no room.
    pub(crate) fn measure(
        axes: &Axes,
        color: Option<&ResolvedColorScale>,
        theme: &Theme,
        lib: &SceneFonts<'_>,
    ) -> Option<Self> {
        let color = color?;
        let options = axes.colorbar_options();
        let position = ColorbarSide::resolve(axes, theme)?;
        Some(Self::with_options(options, color, position, theme, lib))
    }

    /// Measures the same block for either panel or figure settings.
    pub(crate) fn with_options(
        options: &crate::ColorbarOptions,
        color: &ResolvedColorScale,
        position: ColorbarSide,
        theme: &Theme,
        lib: &SceneFonts<'_>,
    ) -> Self {
        let styles = Styles::new(theme);
        let ticks = measure_ticks(lib, &color.ticks, styles.tick);
        let label = (!options.label.is_empty()).then(|| measure(lib, &options.label, styles.body));
        let tick_width = ticks
            .iter()
            .map(|(_, label)| label.width())
            .fold(0.0, f64::max);
        let (ascent, descent) = metrics(lib, &styles.tick);
        let (label_ascent, label_descent) = metrics(lib, &styles.body);
        let label_height = if label.is_some() {
            match position {
                ColorbarSide::Right => label_ascent + AXIS_LABEL_GAP,
                ColorbarSide::Bottom => label_ascent + label_descent + LABEL_GAP,
            }
        } else {
            0.0
        };
        Self {
            position,
            ticks,
            label,
            tick_width,
            ascent,
            descent,
            label_height,
            label_descent,
            size: f64::from(styles.tick.size),
            thickness: f64::from(theme.colorbar_thickness()),
            gap: f64::from(theme.colorbar_gap()),
            tick_length: f64::from(theme.colorbar_tick_length()),
        }
    }

    /// Width of a bottom bar's left-aligned label; zero for a right bar.
    pub(super) fn bottom_label_width(&self) -> f64 {
        if self.position == ColorbarSide::Bottom {
            self.label.as_ref().map_or(0.0, Label::width)
        } else {
            0.0
        }
    }

    /// The horizontal room including its gap; zero for a bottom bar.
    pub(crate) fn right_reserve(&self) -> f64 {
        if self.position != ColorbarSide::Right {
            return 0.0;
        }
        self.gap
            + (self.thickness + self.tick_length + TICK_GAP + self.tick_width)
                .max(self.label.as_ref().map_or(0.0, Label::width))
    }

    /// The vertical room including its gap; zero for a right bar.
    pub(crate) fn bottom_reserve(&self) -> f64 {
        if self.position != ColorbarSide::Bottom {
            return 0.0;
        }
        self.gap
            + self.label_height
            + self.thickness
            + self.tick_length
            + TICK_GAP
            + self.ascent
            + self.descent
    }

    /// Room above the plot for a right bar's label.
    pub(super) fn label_band(&self) -> f64 {
        if self.position == ColorbarSide::Right {
            self.label_height
        } else {
            0.0
        }
    }

    /// Room above and below the plot for right-bar end ticks.
    pub(super) fn tick_overhang(&self) -> (f64, f64) {
        if self.position == ColorbarSide::Right {
            (
                (self.ascent - Y_LABEL_CENTER * self.size).max(0.0),
                Y_LABEL_CENTER * self.size + self.descent,
            )
        } else {
            (0.0, 0.0)
        }
    }

    /// Bottom-bar labels impose the same horizontal containment bounds as x tick labels.
    pub(super) fn x_spans(&self) -> impl Iterator<Item = super::inset::TickSpan> + '_ {
        self.ticks
            .iter()
            .filter(|_| self.position == ColorbarSide::Bottom)
            .map(|(pos, label)| super::inset::TickSpan {
                pos: pos.get(),
                half: label.width() / 2.0,
            })
    }

    /// Places the bar outside the axes and any direct labels or bottom legend.
    pub(super) fn place(
        mut self,
        plot: Area,
        right_offset: f64,
        bottom: f64,
        rise: f64,
    ) -> PlacedColorbar {
        let bar = match self.position {
            ColorbarSide::Right => Area {
                x: plot.right() + right_offset + self.gap,
                y: plot.y,
                width: self.thickness,
                height: plot.height,
            },
            ColorbarSide::Bottom => Area {
                x: plot.x,
                y: bottom + self.gap + self.label_height,
                width: plot.width,
                height: self.thickness,
            },
        };
        for (pos, label) in &mut self.ticks {
            match self.position {
                ColorbarSide::Right => {
                    label.x = bar.right() + self.tick_length + TICK_GAP;
                    label.y = bar.y_at(*pos) + Y_LABEL_CENTER * self.size;
                }
                ColorbarSide::Bottom => {
                    label.x = bar.x_at(*pos) - label.width() / 2.0;
                    label.y = bar.bottom() + self.tick_length + TICK_GAP + self.ascent;
                }
            }
        }
        if let Some(label) = &mut self.label {
            label.x = bar.x;
            label.y = match self.position {
                ColorbarSide::Right => bar.y - rise - AXIS_LABEL_GAP,
                ColorbarSide::Bottom => bar.y - LABEL_GAP - self.label_descent,
            };
        }
        PlacedColorbar {
            position: self.position,
            bar,
            ticks: self.ticks,
            label: self.label,
        }
    }
}

/// An insufficient colorbar band, with the same actionable size hints as a legend.
pub(super) fn room_error(
    context: &'static str,
    needed: f64,
    available: f64,
    vertical: bool,
) -> Error {
    let (needed, available) = super::widths(needed, available.max(0.0));
    Error::style(
        context,
        format!("the colorbar needs {needed} pt; the panel has {available} pt."),
    )
    .with_hint(if vertical {
        "increase the figure height."
    } else {
        "shorten the label or widen the figure."
    })
}

impl PlacedColorbar {
    /// Checks that all texts fit their panel, including on a single-panel figure.
    pub(super) fn check(
        &self,
        context: &'static str,
        frame: Frame,
        outer_edges: [bool; 2],
        lib: &SceneFonts<'_>,
    ) -> Result<()> {
        let left = frame.x
            + if outer_edges[0] {
                super::X_LABEL_EDGE_GAP
            } else {
                0.0
            };
        let right = frame.right()
            - if outer_edges[1] {
                super::X_LABEL_EDGE_GAP
            } else {
                0.0
            };
        for label in self
            .ticks
            .iter()
            .map(|(_, label)| label)
            .chain(self.label.iter())
        {
            if label.x < left - 1e-7 || label.x + label.width() > right + 1e-7 {
                return Err(room_error(
                    context,
                    label.width(),
                    (right - label.x).min(label.x + label.width() - left),
                    false,
                ));
            }
            let (ascent, descent) = metrics(lib, &label.style);
            if label.y - ascent < frame.y - 1e-7 || label.y + descent > frame.bottom() + 1e-7 {
                return Err(room_error(
                    context,
                    ascent + descent,
                    frame.bottom() - (label.y - ascent),
                    true,
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::pipeline::plan_figure;
    use crate::{
        AxisData, Cmap, ColorBy, ColorbarOptions, ColorbarPosition, Figure, FontLibrary,
        LegendPosition, LegendStyle, LineOptions, PointColor, ScaleKind, ScatterOptions, SvgText,
        Theme,
    };

    fn figure(position: ColorbarPosition, center: Option<f64>) -> Figure {
        figure_with(position, center, 6.0, 4.0, Theme::pigment())
    }

    fn figure_with(
        position: ColorbarPosition,
        center: Option<f64>,
        width: f64,
        height: f64,
        theme: Theme,
    ) -> Figure {
        let mut figure = Figure::new(width, height, theme).unwrap();
        let mut axes = figure
            .add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap();
        axes.scatter(
            AxisData::Numbers(vec![1.0, 2.0, 3.0]),
            &[1.0, 2.0, 3.0],
            ScatterOptions {
                color: PointColor::By(ColorBy {
                    center,
                    ..ColorBy::new(vec![-2.0, 0.0, 8.0])
                }),
                ..ScatterOptions::default()
            },
        )
        .unwrap();
        axes.colorbar(ColorbarOptions {
            label: "Intensity".into(),
            position: Some(position),
            ..ColorbarOptions::default()
        })
        .unwrap();
        figure
    }

    #[test]
    fn colorbar_right_spans_plot_height() {
        let f = figure(ColorbarPosition::Right, None);
        let (_, layout) = plan_figure(&f, (0, 0), FontLibrary::global());
        let colorbar = layout.colorbar.unwrap();
        assert_eq!(colorbar.bar.y, layout.plot.y);
        assert_eq!(colorbar.bar.height, layout.plot.height);
        assert_eq!(colorbar.bar.width, 10.0);
        assert_eq!(colorbar.bar.x, layout.plot.right() + 10.0);
        assert_eq!(colorbar.label.as_ref().unwrap().x, colorbar.bar.x);
        assert!(colorbar.label.as_ref().unwrap().y < colorbar.bar.y);
    }

    #[test]
    fn colorbar_right_label_shares_y_title_baseline() {
        for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
            let mut f = figure_with(ColorbarPosition::Right, None, 6.0, 4.0, theme);
            f.single_axes_mut().ylabel("Height");
            let (_, layout) = plan_figure(&f, (0, 0), FontLibrary::global());
            let label = layout.colorbar.as_ref().unwrap().label.as_ref().unwrap();
            assert_eq!(label.y, layout.ylabel.as_ref().unwrap().y);
            f.single_axes_mut()
                .colorbar(ColorbarOptions {
                    position: Some(ColorbarPosition::None),
                    ..ColorbarOptions::default()
                })
                .unwrap();
            let (_, hidden) = plan_figure(&f, (0, 0), FontLibrary::global());
            assert_eq!(layout.plot.y, hidden.plot.y);
        }
    }

    #[test]
    fn colorbar_bottom_spans_plot_width() {
        let mut f = figure(ColorbarPosition::Bottom, None);
        f.single_axes_mut().xlabel("Distance");
        let (_, layout) = plan_figure(&f, (0, 0), FontLibrary::global());
        let colorbar = layout.colorbar.unwrap();
        assert_eq!(colorbar.bar.x, layout.plot.x);
        assert_eq!(colorbar.bar.width, layout.plot.width);
        assert_eq!(colorbar.bar.height, 10.0);
        assert!(colorbar.label.as_ref().unwrap().y > layout.xlabel.unwrap().y);
        assert!(colorbar.label.as_ref().unwrap().y < colorbar.bar.y);
    }

    #[test]
    fn colorbar_and_right_legend_side_by_side() {
        let mut f = figure(ColorbarPosition::Right, None);
        let mut axes = f.single_axes_mut();
        axes.line(
            AxisData::Numbers(vec![1.0, 3.0]),
            &[1.0, 3.0],
            LineOptions {
                label: "Reference".into(),
                ..LineOptions::default()
            },
        )
        .unwrap();
        axes.legend_at(LegendStyle::Header, LegendPosition::Right)
            .unwrap();
        let (_, layout) = plan_figure(&f, (0, 0), FontLibrary::global());
        let bar = layout.colorbar.unwrap();
        let right = bar
            .ticks
            .iter()
            .map(|(_, label)| label.x + label.width())
            .fold(bar.bar.right(), f64::max)
            .max(
                bar.label
                    .as_ref()
                    .map_or(0.0, |label| label.x + label.width()),
            );
        assert!(layout.legend.entries()[0].swatch.x >= right + 12.0 - 1e-9);
    }

    #[test]
    fn colorbar_ticks_include_center() {
        let f = figure(ColorbarPosition::Right, Some(0.3));
        let (resolved, layout) = plan_figure(&f, (0, 0), FontLibrary::global());
        assert!(
            resolved
                .color
                .unwrap()
                .ticks
                .iter()
                .any(|tick| (tick.pos.get() - 0.5).abs() < 1e-12 && tick.label == "0.3")
        );
        assert!(
            layout
                .colorbar
                .unwrap()
                .ticks
                .iter()
                .any(|(pos, label)| (pos.get() - 0.5).abs() < 1e-12 && label.text == "0.3")
        );
    }

    #[test]
    fn colorbar_center_ticks_do_not_overlap() {
        for position in [ColorbarPosition::Right, ColorbarPosition::Bottom] {
            let f = figure(position, Some(0.3));
            let (resolved, layout) = plan_figure(&f, (0, 0), FontLibrary::global());
            let fonts = crate::test_support::figure_fonts(f.theme());
            let (x, y) = crate::layout::tick_rooms(
                crate::layout::PlotSize {
                    width: layout.plot.width,
                    height: layout.plot.height,
                },
                f.theme(),
                &fonts,
                crate::layout::TickSpacing::LabelsApart,
            );
            let room = if position == ColorbarPosition::Bottom {
                x
            } else {
                y
            };
            let color = resolved.color.unwrap();
            assert!(
                room.slack(&color.ticks) >= 0.0,
                "{position:?}: {:?}",
                color.ticks
            );
            assert!(color.ticks.iter().any(|tick| tick.label == "0.3"));
        }
    }

    #[test]
    fn colorbar_below_bottom_legend() {
        let mut f = figure(ColorbarPosition::Bottom, None);
        let mut ax = f.single_axes_mut();
        ax.xlabel("Distance");
        ax.line(
            AxisData::Numbers(vec![1.0, 3.0]),
            &[1.0, 3.0],
            LineOptions {
                label: "Reference".into(),
                ..LineOptions::default()
            },
        )
        .unwrap();
        ax.legend_at(LegendStyle::Header, LegendPosition::Bottom)
            .unwrap();
        let (_, layout) = plan_figure(&f, (0, 0), FontLibrary::global());
        let bar = layout.colorbar.unwrap();
        let entry = &layout.legend.entries()[0];
        assert!(entry.swatch.center_y > layout.xlabel.unwrap().y);
        assert!(bar.label.unwrap().y > entry.swatch.center_y);
    }

    #[test]
    fn colorbar_leaves_room_for_direct_labels() {
        let mut f = figure(ColorbarPosition::Right, None);
        let mut ax = f.single_axes_mut();
        ax.line(
            AxisData::Numbers(vec![1.0, 3.0]),
            &[1.0, 3.0],
            LineOptions {
                label: "A longer reference series label".into(),
                ..LineOptions::default()
            },
        )
        .unwrap();
        ax.legend(LegendStyle::Direct);
        let (_, layout) = plan_figure(&f, (0, 0), FontLibrary::global());
        let bar = layout.colorbar.unwrap();
        for direct in layout.direct {
            if let Some(label) = direct.label {
                assert!(label.x + label.width() + 10.0 <= bar.bar.x + 1e-7);
            }
        }
    }

    #[test]
    fn colorbar_room_errors_name_the_bar() {
        for (position, width, height) in [
            (ColorbarPosition::Right, 1.0, 4.0),
            (ColorbarPosition::Bottom, 6.0, 1.0),
        ] {
            let f = figure_with(position, None, width, height, Theme::pigment());
            let error = f.to_scene(FontLibrary::global()).unwrap_err();
            assert!(matches!(error, crate::Error::Style { .. }));
            assert!(error.to_string().contains("the colorbar needs"), "{error}");
        }
        for vertical in [false, true] {
            let error = super::room_error("Figure.to_scene()", 46.0, 30.0, vertical);
            assert_eq!(
                error.to_string(),
                "Figure.to_scene(): the colorbar needs 46 pt; the panel has 30 pt."
            );
            assert_eq!(
                error.hint(),
                Some(if vertical {
                    "increase the figure height."
                } else {
                    "shorten the label or widen the figure."
                })
            );
        }
    }

    #[test]
    fn colorbar_tick_width_error_names_the_bar() {
        let mut f = figure(ColorbarPosition::Bottom, None);
        f.single_axes_mut()
            .colorbar(ColorbarOptions {
                position: Some(ColorbarPosition::Bottom),
                format: Some(
                    crate::TickFormat::fixed(0)
                        .unwrap()
                        .affix("", &"X".repeat(200))
                        .unwrap(),
                ),
                ..ColorbarOptions::default()
            })
            .unwrap();
        let error = f.to_scene(FontLibrary::global()).unwrap_err();
        assert!(error.to_string().contains("the colorbar needs"), "{error}");
    }

    #[test]
    fn colorbars_keep_grid_plots_aligned() {
        for position in [ColorbarPosition::Right, ColorbarPosition::Bottom] {
            let mut f =
                Figure::with_grid(9.0, 7.0, Theme::pigment(), crate::Grid::new(2, 2).unwrap())
                    .unwrap();
            for i in 0..4 {
                let mut ax = f
                    .add_axes_at(
                        crate::Placement {
                            row: crate::GridIndex::At(i / 2),
                            col: crate::GridIndex::At(i % 2),
                            ..crate::Placement::default()
                        },
                        ScaleKind::Linear,
                        ScaleKind::Linear,
                    )
                    .unwrap();
                ax.scatter(
                    AxisData::Numbers(vec![1.0, 2.0]),
                    &[1.0, 2.0],
                    ScatterOptions {
                        color: PointColor::By(ColorBy::new(vec![0.0, (i + 1) as f64 * 100.0])),
                        ..ScatterOptions::default()
                    },
                )
                .unwrap();
                ax.xlabel("Distance");
                ax.colorbar(ColorbarOptions {
                    label: if i == 0 {
                        "Intensity".into()
                    } else {
                        String::new()
                    },
                    position: Some(if i == 3 {
                        ColorbarPosition::None
                    } else {
                        position
                    }),
                    ..ColorbarOptions::default()
                })
                .unwrap();
            }
            let planned = crate::pipeline::try_plan_panels(&f, FontLibrary::global()).unwrap();
            assert_eq!(planned[0].1.plot.y, planned[1].1.plot.y);
            assert_eq!(planned[0].1.plot.bottom(), planned[1].1.plot.bottom());
            assert_eq!(planned[0].1.plot.x, planned[2].1.plot.x);
            assert_eq!(planned[0].1.plot.right(), planned[2].1.plot.right());
            assert_eq!(
                planned[0].1.xlabel.as_ref().unwrap().y,
                planned[1].1.xlabel.as_ref().unwrap().y
            );
        }
    }

    #[test]
    fn svg_gradient_has_64_stops() {
        let f = figure(ColorbarPosition::Right, None);
        let svg = f.to_svg(SvgText::Editable).unwrap();
        let doc = roxmltree::Document::parse(&svg).unwrap();
        let gradients: Vec<_> = doc
            .descendants()
            .filter(|n| n.has_tag_name("linearGradient"))
            .collect();
        assert_eq!(gradients.len(), 1);
        let stops = gradients[0]
            .children()
            .filter(|n| n.has_tag_name("stop"))
            .count();
        assert!(stops >= 64);
        assert_eq!(stops, 257);
    }

    #[test]
    fn colorbar_paint_retains_original_colormap() {
        for position in [ColorbarPosition::Right, ColorbarPosition::Bottom] {
            let f = figure(position, None);
            let (resolved, layout) = plan_figure(&f, (0, 0), FontLibrary::global());
            let color = resolved.color.unwrap();
            let placed = layout.colorbar.unwrap();
            let crate::Item::Group { items, .. } =
                crate::draw::colorbar::draw(f.theme(), &color, &placed)
            else {
                panic!("the colorbar is a group");
            };
            assert!(matches!(&items[0], crate::Item::Fill {
                paint: crate::Paint::Sampled { cmap, .. }, ..
            } if *cmap == color.cmap));
        }
    }

    #[test]
    fn colorbar_canvas_edges_use_grid_topology() {
        let f = figure(ColorbarPosition::Bottom, None);
        let (_, layout) = plan_figure(&f, (0, 0), FontLibrary::global());
        let fonts = crate::test_support::figure_fonts(f.theme());
        let mut bar = layout.colorbar.unwrap();
        let label = bar.label.as_mut().unwrap();
        label.x = 1.0;
        bar.ticks.clear();
        let frame = super::Frame {
            x: 0.0,
            y: 0.0,
            width: 432.0,
            height: 288.0,
        };
        assert!(
            bar.check("Figure.to_scene()", frame, [false, false], &fonts)
                .is_ok()
        );
        assert!(
            bar.check("Figure.to_scene()", frame, [true, false], &fonts)
                .is_err()
        );
        let label = bar.label.as_mut().unwrap();
        label.x = frame.right() - label.width() - 1.0;
        let frame = super::Frame {
            width: frame.width - 1e-8,
            ..frame
        };
        assert!(
            bar.check("Figure.to_scene()", frame, [false, false], &fonts)
                .is_ok()
        );
        assert!(
            bar.check("Figure.to_scene()", frame, [false, true], &fonts)
                .is_err()
        );
    }

    #[test]
    fn colorbar_pixels_sample_original_cmap() {
        let cmap =
            Cmap::from_colors(&[crate::Rgba::BLACK, crate::Rgba::WHITE, crate::Rgba::RED]).unwrap();
        let f = figure_with(
            ColorbarPosition::Bottom,
            None,
            6.0,
            4.0,
            Theme::pigment().with_cmap(cmap.clone()),
        );
        let (_, layout) = plan_figure(&f, (0, 0), FontLibrary::global());
        let bar = layout.colorbar.unwrap().bar;
        let png = f.to_png(72).unwrap();
        let image = crate::test_support::image::decode_rgba8(&png).unwrap();
        let y = (bar.y + bar.height / 2.0).floor() as u32;
        let start = bar.x as f32;
        let length = f64::from(bar.right() as f32 - start);
        for x in bar.x.ceil() as u32 + 2..bar.right().floor() as u32 - 2 {
            let t = (f64::from(x) + 0.5 - f64::from(start)) / length;
            let color = cmap.at(t);
            assert_eq!(
                image.at(x, y),
                [color.r, color.g, color.b, color.a],
                "at {t}"
            );
        }
    }

    #[test]
    fn png_gradient_matches_cmap() {
        let cmaps = [
            Cmap::viridis(),
            Cmap::magma(),
            Cmap::cividis(),
            Cmap::blue_red(),
            Cmap::from_colors(&[
                crate::Rgba {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 255,
                },
                crate::Rgba {
                    r: 255,
                    g: 255,
                    b: 255,
                    a: 255,
                },
                crate::Rgba {
                    r: 255,
                    g: 0,
                    b: 0,
                    a: 255,
                },
            ])
            .unwrap(),
        ];
        let cases = cmaps.into_iter().flat_map(|cmap| {
            [72, 144, 300].into_iter().flat_map(move |dpi| {
                [ColorbarPosition::Right, ColorbarPosition::Bottom]
                    .into_iter()
                    .map({
                        let cmap = cmap.clone();
                        move |position| (cmap.clone(), dpi, position)
                    })
            })
        });
        for (cmap, dpi, position) in cases {
            let scale = dpi as f64 / 72.0;
            let f = figure_with(position, None, 6.0, 4.0, Theme::pigment().with_cmap(cmap));
            let (_, layout) = plan_figure(&f, (0, 0), FontLibrary::global());
            let bar = layout.colorbar.unwrap().bar;
            let png = f.to_png(dpi).unwrap();
            let mut reader = png::Decoder::new(std::io::Cursor::new(png))
                .read_info()
                .unwrap();
            let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
            let info = reader.next_frame(&mut pixels).unwrap();
            assert_eq!(info.color_type, png::ColorType::Rgba);
            for fraction in (1..100).map(|i| f64::from(i) / 100.0) {
                let (x, y) = match position {
                    ColorbarPosition::Right => (
                        bar.x + bar.width / 2.0,
                        bar.bottom() - fraction * bar.height,
                    ),
                    ColorbarPosition::Bottom => {
                        (bar.x + fraction * bar.width, bar.y + bar.height / 2.0)
                    }
                    ColorbarPosition::None => unreachable!(),
                };
                let (x, y) = ((x * scale).floor() as usize, (y * scale).floor() as usize);
                let t = match position {
                    ColorbarPosition::Right => {
                        (bar.bottom() - (y as f64 + 0.5) / scale) / bar.height
                    }
                    _ => ((x as f64 + 0.5) / scale - bar.x) / bar.width,
                };
                let expected = f.theme().cmap().at(t);
                let offset = (y * info.width as usize + x) * 4;
                for (actual, expected) in pixels[offset..offset + 4]
                    .iter()
                    .zip([expected.r, expected.g, expected.b, expected.a])
                {
                    assert!(
                        actual.abs_diff(expected) <= 1,
                        "{position:?} {t}: {actual} vs {expected}"
                    );
                }
            }
        }
    }

    #[test]
    fn colorbar_none_hides_it() {
        let f = figure(ColorbarPosition::None, None);
        let (_, layout) = plan_figure(&f, (0, 0), FontLibrary::global());
        assert!(layout.colorbar.is_none());
        assert!(
            !f.to_svg(SvgText::Editable)
                .unwrap()
                .contains("linearGradient")
        );
    }

    #[test]
    fn colorbar_without_mark_rejected() {
        let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        let before = axes.clone();
        let error = axes.colorbar(ColorbarOptions::default()).unwrap_err();
        assert_eq!(
            error.to_string(),
            "Axes.colorbar(): this axes has no color-mapped mark."
        );
        assert_eq!(
            error.hint(),
            Some(
                "add a heatmap, mesh, hist2d, hexbin, or a scatter with color=ColorBy(...) first."
            )
        );
        assert_eq!(*axes, before);
    }
}
