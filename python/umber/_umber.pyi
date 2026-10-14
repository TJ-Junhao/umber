"""Type stub for the private native extension ``umber._umber``.

Errors that umber reports on purpose are ``DataError``, ``StyleError`` or ``UsageError`` with the
instance attributes ``_umber_parameter`` and ``_umber_hint``. A ``TypeError``, ``ValueError`` or
``RuntimeError`` whose message starts with "internal error:" means an argument or a call the
Python layer should never make; it carries neither attribute.

Float data arguments must be one-dimensional, aligned (start and strides multiples of 8 bytes),
C-contiguous or strided float64 buffers whose format is "d", "@d" or "=d"; an explicit byte order
such as "<d" is refused. ``np.require(values, dtype="=f8", requirements=["A"]).view(np.float64)``
produces such a buffer. Name arguments must be a ``list`` of ``str`` without lone surrogates.
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
Free-threaded builds are not verified: use one figure per thread there.
"""

from typing import ClassVar, final

import numpy
import numpy.typing

__all__ = [
    "MAX_BINS",
    "Figure",
    "Theme",
    "__version__",
    "box_stats",
    "kde_scott",
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

# Concrete colors are returned as 0xRRGGBBAA.
def color_hex(code: str) -> int: ...
def color_rgb(r: int, g: int, b: int) -> int: ...
def color_with_alpha(rgba: int, a: float) -> int: ...
def check_alpha(a: float) -> None: ...
def check_palette_index(index: int) -> None: ...

# A scale argument is (kind, base): kind 0 is linear, 1 is log, 2 is category; the base is
# used by kind 1 only.
def check_log_base(base: float) -> None: ...

# A size argument: (kind, value). Kind 0 is Size.AUTO (value unused, 0.0), 1 is Size.times() (value:
# the factor) and 2 is Size.pt() (value: the points).
type _SizeArg = tuple[int, float]

def check_size_times(factor: float) -> None: ...
def check_size_points(points: float) -> None: ...

# A tick format argument: (kind, decimals, prefix, suffix). Kind 0 is TickFormat.AUTO, 1 is
# TickFormat.fixed(), 2 is TickFormat.percent() and 3 is TickFormat.scientific(); the decimals are
# used by kinds 1 and 2 only (0 otherwise). The prefix and the suffix apply to every kind.
type _TickFormatArg = tuple[int, int, str, str]

def check_tick_format(format: _TickFormatArg) -> None: ...

# Float data: a one-dimensional float64 buffer as described above.
type _FloatArray = numpy.typing.NDArray[numpy.float64]
# x values: float data for a linear or log x axis, a list of names for a category x axis.
type _AxisValues = _FloatArray | list[str]
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

@final
class Theme:
    """An immutable theme. Theme colors must be concrete (color kind 2)."""

    def __eq__(self, other: object, /) -> bool: ...
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
    def with_title_font(self, value: int) -> Theme: ...
    def with_title_align(self, value: int) -> Theme: ...
    def with_body_font(self, value: int) -> Theme: ...
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
    def figure_title(self, text: str) -> None: ...
    def figure_subtitle(self, text: str) -> None: ...
    def title(self, row: int, col: int, text: str) -> None: ...
    def subtitle(self, row: int, col: int, text: str) -> None: ...
    def xlabel(self, row: int, col: int, text: str) -> None: ...
    def ylabel(self, row: int, col: int, text: str) -> None: ...
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
    def legend(self, row: int, col: int, style: int) -> None: ...
    def legend_at(self, row: int, col: int, style: int, position: int) -> None: ...
    def line(
        self,
        row: int,
        col: int,
        x: _AxisValues,
        y: _FloatArray,
        label: str,
        color: _ColorArg,
        dash: int,
        width: _SizeArg,
    ) -> None: ...
    def area(
        self,
        row: int,
        col: int,
        x: _AxisValues,
        y: _FloatArray,
        label: str,
        color: _ColorArg,
        dash: int,
        width: _SizeArg,
    ) -> None: ...
    def scatter(
        self,
        row: int,
        col: int,
        x: _AxisValues,
        y: _FloatArray,
        label: str,
        color: _ColorArg,
        marker: int,
        size: _SizeArg,
    ) -> None: ...
    def stem(
        self,
        row: int,
        col: int,
        x: _AxisValues,
        y: _FloatArray,
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
        self, row: int, col: int, categories: list[str], series: list[_SeriesArg]
    ) -> None: ...
    def stacked_bar(
        self, row: int, col: int, categories: list[str], series: list[_SeriesArg]
    ) -> None: ...
    def stacked_area(
        self, row: int, col: int, x: _FloatArray, series: list[_SeriesArg]
    ) -> None: ...
    def hist(
        self, row: int, col: int, values: _FloatArray, bins: int, label: str, color: _ColorArg
    ) -> None: ...
    def step(
        self,
        row: int,
        col: int,
        x: _AxisValues,
        y: _FloatArray,
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
        x: _AxisValues,
        low: _FloatArray,
        high: _FloatArray,
        label: str,
        color: _ColorArg,
        width: _SizeArg,
        caps: bool,
    ) -> None: ...
    def x_error_bars(
        self,
        row: int,
        col: int,
        y: _FloatArray,
        low: _FloatArray,
        high: _FloatArray,
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
