"""``Theme``: an immutable set of colors, fonts and sizes that styles a figure."""

from collections.abc import Sequence
from enum import Enum
from operator import attrgetter
from typing import ClassVar, Final, Never, Self, cast, final

from umber import _cmap, _color, _font, _umber
from umber._args import instance, real
from umber._cmap import CMAP_HINT, Cmap
from umber._color import COLOR_HINT, Color
from umber._enums import (
    Align,
    ColorbarPosition,
    Dash,
    FillStyle,
    GridLines,
    LegendPosition,
    LegendStyle,
)
from umber._errors import new_error
from umber._font import FONT_HINT, Font
from umber._report import api_call


def _color_argument(context: str, name: str, value: object) -> tuple[int, int, float]:
    """Check a color argument and return its native form."""
    return _color.native(instance(context, name, value, Color, COLOR_HINT))


def _member(context: str, value: object, cls: type[Enum]) -> int:
    """Check an enum argument and return the integer the native module takes."""
    names = [f"{cls.__name__}.{member.name}" for member in cls]
    hint = f"use {', '.join(names[:-1])} or {names[-1]}."
    return int(instance(context, "value", value, cls, hint).value)


@final
class Theme:
    """An immutable theme: the colors, fonts and sizes that style a figure.

    Start from ``Theme.PIGMENT``, ``Theme.GRAPHITE`` or ``Theme.MIDNIGHT`` and change it with
    the ``with_*`` methods. Each one returns a new theme and leaves this one unchanged. Theme
    colors must be concrete: ``Color.AUTO`` and ``Color.palette()`` refer to a theme palette
    and are refused. Themes compare equal when every setting is equal; they are not hashable.
    Unlike a ``Color``, a theme cannot be pickled, because the native theme has no serialization.

    Example:
        >>> import umber as ub
        >>> dark = ub.Theme.MIDNIGHT.with_line_width(3.0)
        >>> dark == ub.Theme.MIDNIGHT
        False
    """

    __slots__ = ("_native",)
    _native: _umber.Theme

    PIGMENT: ClassVar["Theme"]
    """The default theme: warm paper, serif titles and a dotted horizontal grid.

    Example:
        >>> import umber as ub
        >>> ub.Theme.PIGMENT == ub.Theme.GRAPHITE
        False
    """

    GRAPHITE: ClassVar["Theme"]
    """A clean white theme with a navy-led palette.

    Example:
        >>> import umber as ub
        >>> ub.Theme.GRAPHITE == ub.Theme.GRAPHITE
        True
    """

    MIDNIGHT: ClassVar["Theme"]
    """A dark theme with a monospaced body font.

    Example:
        >>> import umber as ub
        >>> ub.Theme.MIDNIGHT == ub.Theme.PIGMENT
        False
    """

    def __init__(self) -> None:
        """Refuse construction; start from a preset theme."""
        with api_call():
            raise new_error(
                TypeError,
                "Theme()",
                "Theme has no public constructor; start from a preset theme.",
                parameter=None,
                hint="start from Theme.PIGMENT, Theme.GRAPHITE or Theme.MIDNIGHT.",
            )

    def with_palette(self, colors: Sequence[Color]) -> "Theme":
        """Return this theme with the series palette ``colors``.

        Series without a color take the palette colors in order and wrap around.

        Args:
            colors: One to 16 concrete colors, in a list or tuple.

        Returns:
            A new theme; this one is unchanged.

        Raises:
            TypeError: If ``colors`` is not a sequence of ``Color``.
            StyleError: If a color is ``Color.AUTO`` or ``Color.palette()``, or the number of
                colors is not between 1 and 16.

        Example:
            >>> import umber as ub
            >>> theme = ub.Theme.PIGMENT.with_palette([ub.Color.RED, ub.Color.BLUE])
            >>> theme == ub.Theme.PIGMENT
            False
        """
        with api_call():
            arguments = _color.native_sequence("Theme.with_palette()", colors)
            return _wrap(self._native.with_palette(arguments))

    def with_background(self, value: Color) -> "Theme":
        """Return this theme with the background color set to ``value``.

        Args:
            value: A concrete color; ``Color.AUTO`` and ``Color.palette()`` are refused.

        Returns:
            A new theme; this one is unchanged.

        Raises:
            TypeError: If ``value`` is not a ``Color``.
            StyleError: If ``value`` is ``Color.AUTO`` or ``Color.palette()``.

        Example:
            >>> import umber as ub
            >>> theme = ub.Theme.PIGMENT.with_background(ub.Color.hex("#123456"))
            >>> theme == ub.Theme.PIGMENT
            False
        """
        with api_call():
            color = _color_argument("Theme.with_background()", "value", value)
            return _wrap(self._native.with_background(color))

    def with_text_color(self, value: Color) -> "Theme":
        """Return this theme with the text color set to ``value``.

        Args:
            value: A concrete color; ``Color.AUTO`` and ``Color.palette()`` are refused.

        Returns:
            A new theme; this one is unchanged.

        Raises:
            TypeError: If ``value`` is not a ``Color``.
            StyleError: If ``value`` is ``Color.AUTO`` or ``Color.palette()``.

        Example:
            >>> import umber as ub
            >>> theme = ub.Theme.PIGMENT.with_text_color(ub.Color.hex("#123456"))
            >>> theme == ub.Theme.PIGMENT
            False
        """
        with api_call():
            color = _color_argument("Theme.with_text_color()", "value", value)
            return _wrap(self._native.with_text_color(color))

    def with_muted_color(self, value: Color) -> "Theme":
        """Return this theme with the muted text color set to ``value``.

        Args:
            value: A concrete color; ``Color.AUTO`` and ``Color.palette()`` are refused.

        Returns:
            A new theme; this one is unchanged.

        Raises:
            TypeError: If ``value`` is not a ``Color``.
            StyleError: If ``value`` is ``Color.AUTO`` or ``Color.palette()``.

        Example:
            >>> import umber as ub
            >>> theme = ub.Theme.PIGMENT.with_muted_color(ub.Color.hex("#123456"))
            >>> theme == ub.Theme.PIGMENT
            False
        """
        with api_call():
            color = _color_argument("Theme.with_muted_color()", "value", value)
            return _wrap(self._native.with_muted_color(color))

    def with_grid_color(self, value: Color) -> "Theme":
        """Return this theme with the gridline color set to ``value``.

        Args:
            value: A concrete color; ``Color.AUTO`` and ``Color.palette()`` are refused.

        Returns:
            A new theme; this one is unchanged.

        Raises:
            TypeError: If ``value`` is not a ``Color``.
            StyleError: If ``value`` is ``Color.AUTO`` or ``Color.palette()``.

        Example:
            >>> import umber as ub
            >>> theme = ub.Theme.PIGMENT.with_grid_color(ub.Color.hex("#123456"))
            >>> theme == ub.Theme.PIGMENT
            False
        """
        with api_call():
            color = _color_argument("Theme.with_grid_color()", "value", value)
            return _wrap(self._native.with_grid_color(color))

    def with_baseline_color(self, value: Color) -> "Theme":
        """Return this theme with the baseline color set to ``value``.

        Args:
            value: A concrete color; ``Color.AUTO`` and ``Color.palette()`` are refused.

        Returns:
            A new theme; this one is unchanged.

        Raises:
            TypeError: If ``value`` is not a ``Color``.
            StyleError: If ``value`` is ``Color.AUTO`` or ``Color.palette()``.

        Example:
            >>> import umber as ub
            >>> theme = ub.Theme.PIGMENT.with_baseline_color(ub.Color.hex("#123456"))
            >>> theme == ub.Theme.PIGMENT
            False
        """
        with api_call():
            color = _color_argument("Theme.with_baseline_color()", "value", value)
            return _wrap(self._native.with_baseline_color(color))

    def with_grid_lines(self, value: GridLines) -> "Theme":
        """Return this theme with the gridlines set to ``value``.

        Args:
            value: The GridLines to use.

        Returns:
            A new theme; this one is unchanged.

        Raises:
            TypeError: If ``value`` is not a ``GridLines``.

        Example:
            >>> import umber as ub
            >>> theme = ub.Theme.PIGMENT.with_grid_lines(ub.GridLines.BOTH)
            >>> theme == ub.Theme.PIGMENT
            False
        """
        with api_call():
            code = _member("Theme.with_grid_lines()", value, GridLines)
            return _wrap(self._native.with_grid_lines(code))

    def with_grid_dash(self, value: Dash) -> "Theme":
        """Return this theme with the gridline dash set to ``value``.

        Args:
            value: The Dash to use.

        Returns:
            A new theme; this one is unchanged.

        Raises:
            TypeError: If ``value`` is not a ``Dash``.

        Example:
            >>> import umber as ub
            >>> theme = ub.Theme.PIGMENT.with_grid_dash(ub.Dash.DASHED)
            >>> theme == ub.Theme.PIGMENT
            False
        """
        with api_call():
            code = _member("Theme.with_grid_dash()", value, Dash)
            return _wrap(self._native.with_grid_dash(code))

    def with_title_font(self, value: Font) -> "Theme":
        """Return this theme with the title font set to ``value``.

        Args:
            value: The Font to use.

        Returns:
            A new theme; this one is unchanged.

        Raises:
            TypeError: If ``value`` is not a ``Font``.

        Example:
            >>> import umber as ub
            >>> theme = ub.Theme.PIGMENT.with_title_font(ub.Font.INTER)
            >>> theme == ub.Theme.PIGMENT
            False
        """
        with api_call():
            font = instance("Theme.with_title_font()", "value", value, Font, FONT_HINT)
            return _wrap(self._native.with_title_font(_font.native(font)))

    def with_title_align(self, value: Align) -> "Theme":
        """Return this theme with figure and panel title alignment set to ``value``.

        Subtitles follow their title; the merged grid legend follows the figure title.
        Panel legend positions are independent of title alignment. The default is ``LEFT``.

        Args:
            value: The Align to use.

        Returns:
            A new theme; this one is unchanged.

        Raises:
            TypeError: If ``value`` is not an ``Align``.

        Example:
            >>> import umber as ub
            >>> theme = ub.Theme.PIGMENT.with_title_align(ub.Align.CENTER)
            >>> theme == ub.Theme.PIGMENT
            False
        """
        with api_call():
            code = _member("Theme.with_title_align()", value, Align)
            return _wrap(self._native.with_title_align(code))

    def with_body_font(self, value: Font) -> "Theme":
        """Return this theme with the body font set to ``value``.

        Args:
            value: The Font to use.

        Returns:
            A new theme; this one is unchanged.

        Raises:
            TypeError: If ``value`` is not a ``Font``.

        Example:
            >>> import umber as ub
            >>> theme = ub.Theme.PIGMENT.with_body_font(ub.Font.JETBRAINS_MONO)
            >>> theme == ub.Theme.PIGMENT
            False
        """
        with api_call():
            font = instance("Theme.with_body_font()", "value", value, Font, FONT_HINT)
            return _wrap(self._native.with_body_font(_font.native(font)))

    def with_legend_style(self, value: LegendStyle) -> "Theme":
        """Return this theme with the legend style set to ``value``.

        Args:
            value: The LegendStyle to use.

        Returns:
            A new theme; this one is unchanged.

        Raises:
            TypeError: If ``value`` is not a ``LegendStyle``.

        Example:
            >>> import umber as ub
            >>> theme = ub.Theme.PIGMENT.with_legend_style(ub.LegendStyle.CHIPS)
            >>> theme == ub.Theme.PIGMENT
            False
        """
        with api_call():
            code = _member("Theme.with_legend_style()", value, LegendStyle)
            return _wrap(self._native.with_legend_style(code))

    def with_legend_position(self, value: LegendPosition) -> "Theme":
        """Return this theme with the panel legend position set to ``value``.

        Applies to single-panel figures and ``Legend.PER_PANEL``; ``Axes.legend()`` can
        override it. Direct labels stay at line ends, and a merged grid legend stays under
        the figure title. The default is ``TOP_LEFT``.

        Args:
            value: The LegendPosition to use. ``BEST`` places the legend inside the plot,
                covering the least data.

        Returns:
            A new theme; this one is unchanged.

        Raises:
            TypeError: If ``value`` is not a ``LegendPosition``.

        Example:
            >>> import umber as ub
            >>> theme = ub.Theme.PIGMENT.with_legend_position(ub.LegendPosition.BOTTOM)
            >>> theme == ub.Theme.PIGMENT
            False
        """
        with api_call():
            code = _member("Theme.with_legend_position()", value, LegendPosition)
            return _wrap(self._native.with_legend_position(code))

    def with_fill_style(self, value: FillStyle) -> "Theme":
        """Return this theme with the fill style of areas and histogram bins set to ``value``.

        ``FillStyle.SOLID`` paints a flat translucent fill. ``FillStyle.GRADIENT`` paints a fill
        that fades out toward the baseline. The style applies to areas and histograms; lines are
        never filled, so it does not change them.

        Args:
            value: The FillStyle to use.

        Returns:
            A new theme; this one is unchanged.

        Raises:
            TypeError: If ``value`` is not a ``FillStyle``.

        Example:
            >>> import umber as ub
            >>> theme = ub.Theme.PIGMENT.with_fill_style(ub.FillStyle.GRADIENT)
            >>> theme == ub.Theme.PIGMENT
            False
        """
        with api_call():
            code = _member("Theme.with_fill_style()", value, FillStyle)
            return _wrap(self._native.with_fill_style(code))

    def with_font_scale(self, value: float) -> "Theme":
        """Return this theme with the font scale set to ``value``.

        Args:
            value: The number, greater than 0 and at most 10. 1.0 leaves font sizes unscaled.

        Returns:
            A new theme; this one is unchanged.

        Raises:
            TypeError: If ``value`` is not a number.
            StyleError: If ``value`` is not greater than 0 and at most 10.

        Example:
            >>> import umber as ub
            >>> theme = ub.Theme.PIGMENT.with_font_scale(1.25)
            >>> theme == ub.Theme.PIGMENT
            False
        """
        with api_call():
            number = real("Theme.with_font_scale()", "value", value)
            return _wrap(self._native.with_font_scale(number))

    def with_line_width(self, value: float) -> "Theme":
        """Return this theme with the line width set to ``value``.

        Args:
            value: The number, finite and greater than 0. In points.

        Returns:
            A new theme; this one is unchanged.

        Raises:
            TypeError: If ``value`` is not a number.
            StyleError: If ``value`` is not finite and greater than 0.

        Example:
            >>> import umber as ub
            >>> theme = ub.Theme.PIGMENT.with_line_width(3.0)
            >>> theme == ub.Theme.PIGMENT
            False
        """
        with api_call():
            number = real("Theme.with_line_width()", "value", value)
            return _wrap(self._native.with_line_width(number))

    def with_marker_size(self, value: float) -> "Theme":
        """Return this theme with the marker size set to ``value``.

        Args:
            value: The number, finite and greater than 0. In points.

        Returns:
            A new theme; this one is unchanged.

        Raises:
            TypeError: If ``value`` is not a number.
            StyleError: If ``value`` is not finite and greater than 0.

        Example:
            >>> import umber as ub
            >>> theme = ub.Theme.PIGMENT.with_marker_size(9.0)
            >>> theme == ub.Theme.PIGMENT
            False
        """
        with api_call():
            number = real("Theme.with_marker_size()", "value", value)
            return _wrap(self._native.with_marker_size(number))

    def with_marker_alpha(self, value: float) -> "Theme":
        """Return this theme with the marker opacity set to ``value``.

        Args:
            value: The number, from 0.0 to 1.0. 0.0 is transparent and 1.0 opaque.

        Returns:
            A new theme; this one is unchanged.

        Raises:
            TypeError: If ``value`` is not a number.
            StyleError: If ``value`` is not from 0.0 to 1.0.

        Example:
            >>> import umber as ub
            >>> theme = ub.Theme.PIGMENT.with_marker_alpha(0.5)
            >>> theme == ub.Theme.PIGMENT
            False
        """
        with api_call():
            number = real("Theme.with_marker_alpha()", "value", value)
            return _wrap(self._native.with_marker_alpha(number))

    def with_baseline_width(self, value: float) -> "Theme":
        """Return this theme with the baseline width set to ``value``.

        Args:
            value: The number, finite and at least 0. In points.

        Returns:
            A new theme; this one is unchanged.

        Raises:
            TypeError: If ``value`` is not a number.
            StyleError: If ``value`` is not finite and at least 0.

        Example:
            >>> import umber as ub
            >>> theme = ub.Theme.PIGMENT.with_baseline_width(2.0)
            >>> theme == ub.Theme.PIGMENT
            False
        """
        with api_call():
            number = real("Theme.with_baseline_width()", "value", value)
            return _wrap(self._native.with_baseline_width(number))

    def with_bar_corner_radius(self, value: float) -> "Theme":
        """Return this theme with the bar corner radius set to ``value``.

        Args:
            value: The number, finite and at least 0. In points.

        Returns:
            A new theme; this one is unchanged.

        Raises:
            TypeError: If ``value`` is not a number.
            StyleError: If ``value`` is not finite and at least 0.

        Example:
            >>> import umber as ub
            >>> theme = ub.Theme.PIGMENT.with_bar_corner_radius(3.0)
            >>> theme == ub.Theme.PIGMENT
            False
        """
        with api_call():
            number = real("Theme.with_bar_corner_radius()", "value", value)
            return _wrap(self._native.with_bar_corner_radius(number))

    def with_hist_gap(self, value: float) -> "Theme":
        """Return this theme with the histogram bar gap set to ``value``.

        Args:
            value: The number, finite and at least 0. In points.

        Returns:
            A new theme; this one is unchanged.

        Raises:
            TypeError: If ``value`` is not a number.
            StyleError: If ``value`` is not finite and at least 0.

        Example:
            >>> import umber as ub
            >>> theme = ub.Theme.PIGMENT.with_hist_gap(1.0)
            >>> theme == ub.Theme.PIGMENT
            False
        """
        with api_call():
            number = real("Theme.with_hist_gap()", "value", value)
            return _wrap(self._native.with_hist_gap(number))

    def with_cmap(self, cmap: Cmap) -> "Theme":
        """Return this theme with the default sequential colormap set to ``cmap``.

        Args:
            cmap: The colormap.

        Returns:
            A new theme; this one is unchanged.

        Raises:
            TypeError: If ``cmap`` is not a ``Cmap``.

        Example:
            >>> import umber as ub
            >>> theme = ub.Theme.PIGMENT.with_cmap(ub.Cmap.MAGMA)
            >>> theme == ub.Theme.PIGMENT
            False
        """
        with api_call():
            checked = instance("Theme.with_cmap()", "cmap", cmap, Cmap, CMAP_HINT)
            return _wrap(self._native.with_cmap(_cmap.native(checked)))

    def with_diverging_cmap(self, cmap: Cmap) -> "Theme":
        """Return this theme with the default diverging colormap set to ``cmap``.

        Args:
            cmap: The colormap.

        Returns:
            A new theme; this one is unchanged.

        Raises:
            TypeError: If ``cmap`` is not a ``Cmap``.

        Example:
            >>> import umber as ub
            >>> theme = ub.Theme.PIGMENT.with_diverging_cmap(ub.Cmap.BLUE_RED.reversed())
            >>> theme == ub.Theme.PIGMENT
            False
        """
        with api_call():
            checked = instance("Theme.with_diverging_cmap()", "cmap", cmap, Cmap, CMAP_HINT)
            return _wrap(self._native.with_diverging_cmap(_cmap.native(checked)))

    def with_colorbar_position(self, position: ColorbarPosition) -> "Theme":
        """Return this theme with the default colorbar position changed.

        Args:
            position: ``RIGHT``, ``BOTTOM`` or ``NONE``. ``NONE`` hides colorbars by
                default in every sharing mode. ``RIGHT`` and ``BOTTOM`` choose the side
                for unshared panels and ``Share.ALL``; ``Share.ROW`` stays right and
                ``Share.COL`` stays below. ``Axes.colorbar()`` or ``Figure.colorbar()``
                may override the default with a position allowed by the sharing mode.

        Returns:
            A new theme; this theme is unchanged.

        Raises:
            TypeError: If position is not a ColorbarPosition.

        Example:
            >>> import umber as ub
            >>> theme = ub.Theme.PIGMENT.with_colorbar_position(ub.ColorbarPosition.BOTTOM)
            >>> theme == ub.Theme.PIGMENT
            False
        """
        with api_call():
            checked = instance(
                "Theme.with_colorbar_position()",
                "position",
                position,
                ColorbarPosition,
                "use ColorbarPosition.RIGHT, ColorbarPosition.BOTTOM or ColorbarPosition.NONE.",
            )
            return _wrap(self._native.with_colorbar_position(checked.value))

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, Theme):
            return NotImplemented
        return self._native == other._native

    def __repr__(self) -> str:
        for name in ("PIGMENT", "GRAPHITE", "MIDNIGHT"):
            if self == getattr(Theme, name):
                return f"Theme.{name}"
        return "<umber.Theme (customized)>"

    def __copy__(self) -> Self:
        return self

    def __deepcopy__(self, memo: dict[int, object]) -> Self:
        return self

    def __reduce__(self) -> Never:
        with api_call():
            raise new_error(
                TypeError,
                "Theme",
                "a Theme cannot be pickled.",
                parameter=None,
                hint="rebuild it from a preset: Theme.PIGMENT.with_line_width(3.0).",
            )

    def __setattr__(self, name: str, value: object) -> None:
        raise AttributeError("a Theme is immutable")

    def __delattr__(self, name: str) -> None:
        raise AttributeError("a Theme is immutable")


_NATIVE: Final = attrgetter("_native")


def native(theme: Theme) -> _umber.Theme:
    """Return the native theme of ``theme``.

    Args:
        theme: The theme.

    Returns:
        The native theme object.

    Example:
        >>> import umber as ub
        >>> from umber._theme import native
        >>> native(ub.Theme.PIGMENT) == native(ub.Theme.PIGMENT)
        True
    """
    return cast(_umber.Theme, _NATIVE(theme))


def _wrap(value: _umber.Theme) -> Theme:
    """Build a theme around a native one; the class has no public constructor."""
    theme = object.__new__(Theme)
    object.__setattr__(theme, "_native", value)
    return theme


Theme.PIGMENT = _wrap(_umber.Theme.pigment())
Theme.GRAPHITE = _wrap(_umber.Theme.graphite())
Theme.MIDNIGHT = _wrap(_umber.Theme.midnight())
