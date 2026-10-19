"""Type stub for the private native extension ``umber._umber``.

Errors that umber reports on purpose are ``DataError``, ``StyleError`` or ``UsageError`` with the
instance attributes ``_umber_parameter`` and ``_umber_hint``. A ``TypeError``, ``ValueError`` or
``RuntimeError`` whose message starts with "internal error:" means an argument or a call the
Python layer should never make; it carries neither attribute.

Float data arguments must be one-dimensional, aligned (start and strides multiples of 8 bytes),
C-contiguous or strided float64 buffers whose format is "d", "@d" or "=d"; an explicit byte order
such as "<d" is refused. ``np.require(values, dtype="=f8", requirements=["A"]).view(np.float64)``
produces such a buffer. Time data arguments are int64 buffers of microseconds since
1970-01-01T00:00 under the same rules, of format "q" or "l" (eight bytes), with the smallest
int64 for a missing time; the core checks their range. Name arguments must be a ``list`` of
``str`` without lone surrogates.
Text arguments (titles, labels) must not hold lone surrogates either; PyO3 refuses them with a
``UnicodeEncodeError`` that carries neither attribute.

Enum arguments are the integer values that ``enum_members()`` lists. The ``Figure`` methods
that take an axes position resolve it through ``axes_at``; a cell without axes raises
``umber.UsageError`` with a parameter and hint.

The native objects are not shareable across threads while a call is in progress. A ``Figure`` is a
mutable PyO3 object: a call that needs it while another call holds a
conflicting borrow raises ``RuntimeError`` (PyO3's "Already borrowed" or "Already mutably
borrowed") instead of waiting. Rendering copies the figure under a brief borrow, then renders
the copy with the interpreter released, so other threads keep running and the figure stays
usable.

Other native methods do not explicitly release the interpreter. The Python façade converts
data before entering those methods and adds no locking of its own. On a regular (GIL) build,
non-rendering calls with the façade's normalized arguments are serialized. Direct native calls
can invoke Python buffer callbacks while borrowing the figure; those callbacks can release the
interpreter or re-enter the figure, causing a borrow conflict even on a GIL build.

Free-threaded CPython (3.14t and later) is supported: the module keeps the GIL disabled, and
different figures build and render in parallel threads. Calls on one ``Figure`` from several
threads do overlap there, so any of them may raise PyO3's borrow error; the figure's state stays
valid.
"""

from typing import ClassVar, final

import numpy
import numpy.typing

__all__ = [
    "MAX_BINS",
    "MAX_LEVELS",
    "Cmap",
    "ColorBy",
    "Figure",
    "FontValue",
    "font_from_files",
    "font_system",
    "check_file_bold_fallback",
    "Theme",
    "__version__",
    "box_stats",
    "bundled_fonts",
    "kde_scott",
    "scott_bandwidth",
    "kde_curve",
    "ecdf_steps",
    "check_alpha",
    "check_log_base",
    "check_palette_index",
    "check_size_points",
    "check_size_times",
    "check_tick_format",
    "color_hex",
    "color_rgb",
    "color_with_alpha",
    "enum_members",
    "grid_auto_size",
    "grid_cells",
    "named_colors",
]

__version__: str
MAX_BINS: int
MAX_LEVELS: int

# A color argument: (kind, payload, alpha). Kind 0 is Color.AUTO (payload unused, 0), 1 is
# Color.palette() (payload: the index), 2 is a fixed color (payload: 0xRRGGBBAA; alpha unused,
# since the opacity is part of the payload).
type _ColorArg = tuple[int, int, float]

def enum_members() -> dict[str, list[tuple[str, int]]]: ...
def grid_auto_size(rows: int, cols: int) -> tuple[float, float]: ...
def grid_cells(rows: int, cols: int, count: int) -> list[tuple[int, int]]: ...

# Named colors as (name, 0xRRGGBBAA), in the order BLACK, WHITE, GRAY, RED, ORANGE, YELLOW,
# GREEN, TEAL, BLUE, PURPLE, PINK, BROWN.
def named_colors() -> list[tuple[str, int]]: ...

# Font arguments have kind 0 (bundled index), 1 (file value) or 2 (system value).
type _FontArg = tuple[int, int | FontValue]

@final
class FontValue:
    def family_name(self) -> str: ...
    def bold_fallback(self) -> bool: ...
    def file_paths(self) -> tuple[str, str | None] | None: ...
    def file_source(self) -> tuple[bytes, bytes | None, str, str | None] | None: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

def font_from_files(
    regular: bytes,
    bold: bytes | None,
    name: str,
    regular_path: str,
    bold_path: str | None,
    bold_fallback: bool,
) -> FontValue: ...
def font_system(family: str, bold_fallback: bool) -> FontValue: ...
def check_file_bold_fallback(has_bold: bool, bold_fallback: bool) -> None: ...

# Bundled fonts as (constant name, index), in the order SOURCE_SERIF_4, IBM_PLEX_SANS, GEIST,
# INTER, JETBRAINS_MONO.
def bundled_fonts() -> list[tuple[str, int]]: ...

# Concrete colors are returned as 0xRRGGBBAA.
def color_hex(code: str) -> int: ...
def color_rgb(r: int, g: int, b: int) -> int: ...
def color_with_alpha(rgba: int, a: float) -> int: ...
def check_alpha(a: float) -> None: ...
def check_palette_index(index: int) -> None: ...

# A scale argument is (kind, base): kind 0 is linear, 1 is log, 2 is category, 3 is time; the
# base is used by kind 1 only.
def check_log_base(base: float) -> None: ...

# A size argument: (kind, value). Kind 0 is Size.AUTO (value unused, 0.0), 1 is Size.times() (value:
# the factor) and 2 is Size.pt() (value: the points).
type _SizeArg = tuple[int, float]

def check_size_times(factor: float) -> None: ...
def check_size_points(points: float) -> None: ...

# A tick format argument: (kind, decimals, prefix, suffix). Kind 0 is TickFormat.AUTO, 1 is
# TickFormat.fixed(), 2 is TickFormat.percent(), 3 is TickFormat.scientific() and 4 is
# TickFormat.iso(); the decimals are used by kinds 1 and 2 only (0 otherwise). The prefix and the
# suffix apply to every kind.
type _TickFormatArg = tuple[int, int, str, str]

def check_tick_format(format: _TickFormatArg) -> None: ...

# Float data: a one-dimensional float64 buffer as described above.
type _FloatArray = numpy.typing.NDArray[numpy.float64]
# Time data: a one-dimensional int64 buffer of microseconds as described above.
type _TimeArray = numpy.typing.NDArray[numpy.int64]
# x values: float data for a linear or log x axis, a list of names for a category x axis.
type _AxisValues = _FloatArray | list[str]
# Series values along either axis: float data, names, or times on a time axis.
type _SeriesValues = _FloatArray | list[str] | _TimeArray
# A bar series of grouped and stacked bars: (label, values, color).
type _SeriesArg = tuple[str, _FloatArray, _ColorArg]
# A named distribution: (label, one array per category, color).
type _DistributionArg = tuple[str, list[_FloatArray], _ColorArg]

# Test hook: (q1, median, q3, low_whisker, high_whisker, outliers), or None without samples.
def box_stats(
    values: _FloatArray,
) -> tuple[float, float, float, float, float, list[float]] | None: ...

# Test hook: 128 (position, density) pairs, or None below two distinct finite samples.
def kde_scott(values: _FloatArray) -> list[tuple[float, float]] | None: ...

# Test hook: Scott's bandwidth in data units, or None below two distinct finite samples.
def scott_bandwidth(values: _FloatArray) -> float | None: ...

# Test hook: `points` (position, density) pairs from `start` to `end`, the density in data units,
# or None without a finite sample or below two points.
def kde_curve(
    values: _FloatArray, bandwidth: float, start: float, end: float, points: int
) -> list[tuple[float, float]] | None: ...

# Test hook: the x and y of the ECDF's post step line, NaN left out.
def ecdf_steps(values: _FloatArray, complementary: bool) -> tuple[list[float], list[float]]: ...

@final
class Cmap:
    """An immutable colormap; ``colors()`` lists its stops as opaque ``0xRRGGBBAA`` integers."""

    def __eq__(self, other: object, /) -> bool: ...
    # mypy does not infer that a class defining __eq__ is unhashable.
    __hash__: ClassVar[None]  # type: ignore[assignment]  # pyright: ignore[reportIncompatibleMethodOverride]
    @staticmethod
    def viridis() -> Cmap: ...
    @staticmethod
    def magma() -> Cmap: ...
    @staticmethod
    def cividis() -> Cmap: ...
    @staticmethod
    def blue_red() -> Cmap: ...
    @staticmethod
    def from_colors(colors: list[_ColorArg]) -> Cmap: ...
    def reversed(self) -> Cmap: ...
    def colors(self) -> list[int]: ...

@final
class ColorBy:
    """Values mapped through a colormap (``None`` for the theme's), checked when built.

    The scale argument is linear or log (kinds 0 and 1); ``center`` is ``None`` for a
    sequential mapping.
    """

    def __new__(
        cls,
        values: _FloatArray,
        cmap: Cmap | None,
        scale: int,
        base: float,
        center: float | None,
    ) -> ColorBy: ...

@final
class Theme:
    """An immutable theme. Theme colors must be concrete (color kind 2)."""

    def __eq__(self, other: object, /) -> bool: ...
    # mypy does not infer that a class defining __eq__ is unhashable.
    __hash__: ClassVar[None]  # type: ignore[assignment]  # pyright: ignore[reportIncompatibleMethodOverride]
    @staticmethod
    def pigment() -> Theme: ...
    @staticmethod
    def graphite() -> Theme: ...
    @staticmethod
    def midnight() -> Theme: ...
    def with_palette(self, colors: list[_ColorArg]) -> Theme: ...
    def with_background(self, value: _ColorArg) -> Theme: ...
    def with_text_color(self, value: _ColorArg) -> Theme: ...
    def with_muted_color(self, value: _ColorArg) -> Theme: ...
    def with_grid_color(self, value: _ColorArg) -> Theme: ...
    def with_baseline_color(self, value: _ColorArg) -> Theme: ...
    def with_grid_lines(self, value: int) -> Theme: ...
    def with_grid_dash(self, value: int) -> Theme: ...
    def with_title_font(self, value: _FontArg) -> Theme: ...
    def with_title_align(self, value: int) -> Theme: ...
    def with_body_font(self, value: _FontArg) -> Theme: ...
    def with_legend_style(self, value: int) -> Theme: ...
    def with_legend_position(self, value: int) -> Theme: ...
    def with_fill_style(self, value: int) -> Theme: ...
    def with_font_scale(self, value: float) -> Theme: ...
    def with_line_width(self, value: float) -> Theme: ...
    def with_marker_size(self, value: float) -> Theme: ...
    def with_marker_alpha(self, value: float) -> Theme: ...
    def with_baseline_width(self, value: float) -> Theme: ...
    def with_bar_corner_radius(self, value: float) -> Theme: ...
    def with_hist_gap(self, value: float) -> Theme: ...
    def with_cmap(self, cmap: Cmap) -> Theme: ...
    def with_diverging_cmap(self, cmap: Cmap) -> Theme: ...
    def with_colorbar_position(self, position: int) -> Theme: ...

@final
class Figure:
    """A figure with a grid of axes. Every axes method takes the axes' top-left cell first.

    Rendering releases the interpreter. A size of ``None`` is the facade's ``AUTO``; supplied
    sizes are validated before rows and columns, with default sizes resolved in core.
    """

    def __new__(
        cls,
        width: float | None,
        height: float | None,
        theme: Theme,
        rows: int,
        cols: int,
        share_x: int,
        share_y: int,
        legend: int,
        share_color: int,
    ) -> Figure: ...
    def clone(self) -> Figure: ...
    # A grid index is ``(kind, index)``: kind 0 is AUTO, 1 is the index given. Both return the
    # ``(row, col)`` of the axes' top-left cell.
    def add_axes(
        self,
        row_kind: int,
        row: int,
        col_kind: int,
        col: int,
        row_span: int,
        col_span: int,
        x_kind: int,
        x_base: float,
        y_kind: int,
        y_base: float,
    ) -> tuple[int, int]: ...
    def axes_at(
        self,
        row_kind: int,
        row: int,
        col_kind: int,
        col: int,
        x_kind: int,
        x_base: float,
        y_kind: int,
        y_base: float,
    ) -> tuple[int, int]: ...
    def add_pie_axes(
        self, row_kind: int, row: int, col_kind: int, col: int, row_span: int, col_span: int
    ) -> tuple[int, int]: ...
    def pie_axes_at(self, row_kind: int, row: int, col_kind: int, col: int) -> tuple[int, int]: ...
    def pie(
        self,
        row: int,
        col: int,
        categories: list[str],
        values: _FloatArray,
        hole: float,
        annotate: bool,
        format: _TickFormatArg | None,
        colors: list[_ColorArg] | None,
        size: _SizeArg,
    ) -> None: ...
    def pie_title(self, row: int, col: int, text: str, size: _SizeArg) -> None: ...
    def pie_legend(
        self, row: int, col: int, style: int, position: int | None, size: _SizeArg
    ) -> None: ...
    def figure_title(self, text: str, size: _SizeArg) -> None: ...
    def figure_subtitle(self, text: str, size: _SizeArg) -> None: ...
    def title(self, row: int, col: int, text: str, size: _SizeArg) -> None: ...
    def subtitle(self, row: int, col: int, text: str, size: _SizeArg) -> None: ...
    def xlabel(self, row: int, col: int, text: str, size: _SizeArg) -> None: ...
    def ylabel(self, row: int, col: int, text: str, size: _SizeArg) -> None: ...
    def xlim(self, row: int, col: int, low: float, high: float) -> None: ...
    def ylim(self, row: int, col: int, low: float, high: float) -> None: ...
    def xticks(self, row: int, col: int, positions: _FloatArray) -> None: ...
    def yticks(self, row: int, col: int, positions: _FloatArray) -> None: ...
    def xticks_labeled(
        self, row: int, col: int, positions: _FloatArray, labels: list[str]
    ) -> None: ...
    def yticks_labeled(
        self, row: int, col: int, positions: _FloatArray, labels: list[str]
    ) -> None: ...
    def xtick_format(self, row: int, col: int, format: _TickFormatArg) -> None: ...
    def ytick_format(self, row: int, col: int, format: _TickFormatArg) -> None: ...
    def baseline(self, row: int, col: int, shown: bool | None) -> None: ...
    def legend(self, row: int, col: int, style: int, size: _SizeArg) -> None: ...
    def legend_at(self, row: int, col: int, style: int, position: int, size: _SizeArg) -> None: ...
    def line(
        self,
        row: int,
        col: int,
        x: _SeriesValues,
        y: _SeriesValues,
        label: str,
        color: _ColorArg,
        dash: int,
        width: _SizeArg,
    ) -> None: ...
    def area(
        self,
        row: int,
        col: int,
        x: _SeriesValues,
        y: _SeriesValues,
        label: str,
        color: _ColorArg,
        dash: int,
        width: _SizeArg,
    ) -> None: ...
    def scatter(
        self,
        row: int,
        col: int,
        x: _SeriesValues,
        y: _SeriesValues,
        label: str,
        color: _ColorArg | ColorBy,
        marker: int,
        size: _SizeArg,
    ) -> None: ...
    def bubbles(
        self,
        row: int,
        col: int,
        x: _FloatArray | _TimeArray,
        y: _FloatArray | _TimeArray,
        sizes: _FloatArray,
        label: str,
        color: _ColorArg | ColorBy,
        marker: int,
    ) -> None: ...
    def stem(
        self,
        row: int,
        col: int,
        x: _SeriesValues,
        y: _SeriesValues,
        label: str,
        color: _ColorArg,
        marker: int,
        size: _SizeArg,
    ) -> None: ...
    def bar(
        self,
        row: int,
        col: int,
        categories: list[str],
        heights: _FloatArray,
        label: str,
        color: _ColorArg,
        annotate: bool,
        format: _TickFormatArg | None,
        size: _SizeArg,
    ) -> None: ...
    def box_plot(
        self,
        row: int,
        col: int,
        categories: list[str],
        groups: list[_FloatArray],
        label: str,
        color: _ColorArg,
    ) -> None: ...
    def grouped_box_plot(
        self,
        row: int,
        col: int,
        categories: list[str],
        distributions: list[_DistributionArg],
    ) -> None: ...
    def violin(
        self,
        row: int,
        col: int,
        categories: list[str],
        groups: list[_FloatArray],
        label: str,
        color: _ColorArg,
    ) -> None: ...
    def grouped_violin(
        self,
        row: int,
        col: int,
        categories: list[str],
        distributions: list[_DistributionArg],
    ) -> None: ...
    def grouped_bar(
        self,
        row: int,
        col: int,
        categories: list[str],
        series: list[_SeriesArg],
        annotate: bool,
        format: _TickFormatArg | None,
        size: _SizeArg,
    ) -> None: ...
    def stacked_bar(
        self,
        row: int,
        col: int,
        categories: list[str],
        series: list[_SeriesArg],
        annotate: bool,
        format: _TickFormatArg | None,
        size: _SizeArg,
    ) -> None: ...
    def stacked_area(
        self, row: int, col: int, x: _FloatArray, series: list[_SeriesArg]
    ) -> None: ...
    def hist(
        self,
        row: int,
        col: int,
        values: _FloatArray,
        bins: int | _FloatArray,
        style: int,
        cumulative: bool,
        stat: int,
        label: str,
        color: _ColorArg,
    ) -> None: ...
    def mesh(
        self,
        row: int,
        col: int,
        x_edges: _FloatArray | _TimeArray,
        y_edges: _FloatArray | _TimeArray,
        values: _FloatArray,
        cmap: Cmap | None,
        scale: int,
        base: float,
        center: float | None,
        render: int | None,
    ) -> None: ...
    def hist2d(
        self,
        row: int,
        col: int,
        x: _FloatArray,
        y: _FloatArray,
        bins: tuple[int, int],
        cmap: Cmap | None,
        scale: int,
        base: float,
        render: int | None,
    ) -> None: ...
    def hexbin(
        self,
        row: int,
        col: int,
        x: _FloatArray,
        y: _FloatArray,
        gridsize: int,
        cmap: Cmap | None,
        scale: int,
        base: float,
    ) -> None: ...
    def step(
        self,
        row: int,
        col: int,
        x: _SeriesValues,
        y: _SeriesValues,
        position: int,
        label: str,
        color: _ColorArg,
        dash: int,
        width: _SizeArg,
    ) -> None: ...
    def hline(
        self,
        row: int,
        col: int,
        y: float,
        label: str,
        color: _ColorArg,
        dash: int,
        width: _SizeArg,
    ) -> None: ...
    def vline(
        self,
        row: int,
        col: int,
        x: float | str,
        label: str,
        color: _ColorArg,
        dash: int,
        width: _SizeArg,
    ) -> None: ...
    def hspan(
        self, row: int, col: int, low: float, high: float, label: str, color: _ColorArg
    ) -> None: ...
    def vspan(
        self, row: int, col: int, low: float, high: float, label: str, color: _ColorArg
    ) -> None: ...
    def band(
        self,
        row: int,
        col: int,
        x: _FloatArray,
        low: _FloatArray,
        high: _FloatArray,
        label: str,
        color: _ColorArg,
    ) -> None: ...
    def y_error_bars(
        self,
        row: int,
        col: int,
        x: _SeriesValues,
        low: _FloatArray | _TimeArray,
        high: _FloatArray | _TimeArray,
        label: str,
        color: _ColorArg,
        width: _SizeArg,
        caps: bool,
    ) -> None: ...
    def x_error_bars(
        self,
        row: int,
        col: int,
        y: _SeriesValues,
        low: _FloatArray | _TimeArray,
        high: _FloatArray | _TimeArray,
        label: str,
        color: _ColorArg,
        width: _SizeArg,
        caps: bool,
    ) -> None: ...
    def to_png(self, dpi: int) -> bytes: ...
    def to_svg(self, text: int) -> str: ...
    # The file variants report render errors in the context of Figure.save_png() and
    # Figure.save_svg().
    def png_for_file(self, dpi: int) -> bytes: ...
    def svg_for_file(self, text: int) -> str: ...
    # Time axes: times travel as int64 microseconds, scalars as int.
    def xlim_time(self, row: int, col: int, low: int, high: int) -> None: ...
    def ylim_time(self, row: int, col: int, low: int, high: int) -> None: ...
    def xticks_time(self, row: int, col: int, positions: _TimeArray) -> None: ...
    def yticks_time(self, row: int, col: int, positions: _TimeArray) -> None: ...
    def xticks_labeled_time(
        self, row: int, col: int, positions: _TimeArray, labels: list[str]
    ) -> None: ...
    def yticks_labeled_time(
        self, row: int, col: int, positions: _TimeArray, labels: list[str]
    ) -> None: ...
    def vline_time(
        self,
        row: int,
        col: int,
        x: int,
        label: str,
        color: _ColorArg,
        dash: int,
        width: _SizeArg,
    ) -> None: ...
    def hline_time(
        self,
        row: int,
        col: int,
        y: int,
        label: str,
        color: _ColorArg,
        dash: int,
        width: _SizeArg,
    ) -> None: ...
    def hspan_time(
        self, row: int, col: int, low: int, high: int, label: str, color: _ColorArg
    ) -> None: ...
    def vspan_time(
        self, row: int, col: int, low: int, high: int, label: str, color: _ColorArg
    ) -> None: ...
    def band_time(
        self,
        row: int,
        col: int,
        x: _TimeArray,
        low: _FloatArray,
        high: _FloatArray,
        label: str,
        color: _ColorArg,
    ) -> None: ...
    def stacked_area_time(
        self, row: int, col: int, x: _TimeArray, series: list[_SeriesArg]
    ) -> None: ...
    def figure_colorbar(
        self,
        label: str,
        position: int | None,
        format: _TickFormatArg | None,
        size: _SizeArg,
    ) -> None: ...
    def clim(self, row: int, col: int, low: float, high: float) -> None: ...
    def colorbar(
        self,
        row: int,
        col: int,
        label: str,
        position: int | None,
        format: _TickFormatArg | None,
        size: _SizeArg,
    ) -> None: ...
    def heatmap(
        self,
        row: int,
        col: int,
        x_categories: list[str],
        y_categories: list[str],
        values: _FloatArray,
        cmap: Cmap | None,
        scale: int,
        base: float,
        center: float | None,
        annotate: bool,
        format: _TickFormatArg | None,
        render: int | None,
    ) -> None: ...
    def xtick_labels(self, row: int, col: int, size: _SizeArg) -> None: ...
    def ytick_labels(self, row: int, col: int, size: _SizeArg) -> None: ...
    def events(
        self,
        row: int,
        col: int,
        rows: list[str],
        positions: list[_FloatArray | _TimeArray],
        length: float | None,
        width: _SizeArg,
        color: _ColorArg,
    ) -> None: ...
    def strip(
        self,
        row: int,
        col: int,
        categories: list[str],
        groups: list[_FloatArray],
        label: str,
        color: _ColorArg,
        size: _SizeArg,
        jitter: float | None,
    ) -> None: ...
    def swarm(
        self,
        row: int,
        col: int,
        categories: list[str],
        groups: list[_FloatArray],
        label: str,
        color: _ColorArg,
        size: _SizeArg,
    ) -> None: ...
    def grouped_hist(
        self,
        row: int,
        col: int,
        series: list[_SeriesArg],
        bins: int | _FloatArray,
        style: int,
        cumulative: bool,
        stat: int,
    ) -> None: ...
    def ecdf(
        self,
        row: int,
        col: int,
        values: _FloatArray | _TimeArray,
        complementary: bool,
        label: str,
        color: _ColorArg,
        dash: int,
        width: _SizeArg,
    ) -> None: ...
    def kde(
        self,
        row: int,
        col: int,
        values: _FloatArray,
        bandwidth: float | None,
        label: str,
        color: _ColorArg,
        dash: int,
        width: _SizeArg,
    ) -> None: ...
    def grouped_kde(
        self,
        row: int,
        col: int,
        series: list[_SeriesArg],
        bandwidth: float | None,
        dash: int,
        width: _SizeArg,
    ) -> None: ...
    # A coordinate of `at` is a float on a numeric axis, a str on a category axis and an int
    # (microseconds since 1970-01-01T00:00) on a time axis.
    def text(
        self,
        row: int,
        col: int,
        text: str,
        at: tuple[float | str | int, float | str | int],
        align: int,
        size: _SizeArg,
        color: _ColorArg,
    ) -> None: ...
    def annotate(
        self,
        row: int,
        col: int,
        text: str,
        at: tuple[float | str | int, float | str | int],
        offset: tuple[float, float],
        arrow: bool,
        size: _SizeArg,
        color: _ColorArg,
    ) -> None: ...
    def bar_by(
        self,
        row: int,
        col: int,
        categories: list[str],
        series: list[_SeriesArg],
        annotate: bool,
        format: _TickFormatArg | None,
        size: _SizeArg,
    ) -> None: ...
    def grouped_line(
        self,
        row: int,
        col: int,
        groups: list[tuple[str, _SeriesValues, _SeriesValues]],
        dash: int,
        width: _SizeArg,
    ) -> None: ...
    def grouped_area(
        self,
        row: int,
        col: int,
        groups: list[tuple[str, _SeriesValues, _SeriesValues]],
        dash: int,
        width: _SizeArg,
    ) -> None: ...
    def grouped_scatter(
        self,
        row: int,
        col: int,
        groups: list[tuple[str, _SeriesValues, _SeriesValues]],
        marker: int,
        size: _SizeArg,
    ) -> None: ...
    def grouped_step(
        self,
        row: int,
        col: int,
        groups: list[tuple[str, _SeriesValues, _SeriesValues]],
        position: int,
        dash: int,
        width: _SizeArg,
    ) -> None: ...
    def grouped_ecdf(
        self,
        row: int,
        col: int,
        groups: list[tuple[str, _SeriesValues]],
        complementary: bool,
        dash: int,
        width: _SizeArg,
    ) -> None: ...
    def contour(
        self,
        row: int,
        col: int,
        x: _FloatArray,
        y: _FloatArray,
        z: _FloatArray,
        levels: int | _FloatArray | None,
        style: int,
        cmap: Cmap | None,
        scale: int,
        base: float,
        center: float | None,
        color: _ColorArg,
        annotate: bool,
        format: _TickFormatArg | None,
    ) -> None: ...
