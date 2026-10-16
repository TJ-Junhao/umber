"""``ColorBy``: one value per point, mapped through a colormap to the point's color."""

from operator import attrgetter
from typing import Final, Never, Self, cast, final

from umber import _cmap, _color, _scale, _umber
from umber._args import instance, real, type_name
from umber._cmap import CMAP_HINT, Cmap
from umber._color import Color
from umber._enums import AUTO, Auto
from umber._errors import new_error
from umber._report import api_call
from umber._scale import Scale
from umber._types import Values
from umber._values import check_floats_type, floats

_SCALE_HINT: Final = "use Scale.linear() or Scale.log()."
_CENTER_HINT: Final = "pass a number such as 0, or AUTO for a sequential mapping."
_POINT_COLOR_HINT: Final = (
    'use Color.hex("#3366B8"), Color.rgb(), a named color such as Color.BLUE, or ColorBy(values).'
)


@final
class ColorBy:
    """Values mapped through a colormap: pass it as the ``color`` of a scatter or bubbles.

    Each point takes the color its value maps to; a NaN value leaves its point undrawn, and its
    position still counts in the axis limits. All color-mapped marks of an axes share one
    color scale: the same ``cmap``, ``scale`` and ``center``, and one color range, from the
    smallest to the largest finite value unless ``Axes.clim()`` sets it. Values outside the
    range take the colormap's end colors.

    With a ``center`` the mapping is diverging and symmetric: the color range is
    ``center ± d``, where ``d`` is the larger distance from ``center`` to either end of the
    values (or of the range set by ``Axes.clim()``), so ``center`` maps to the middle of the
    colormap and the side with less extent does not reach its end color.

    Args:
        values: One number per point: finite, or NaN to leave the point undrawn.
        cmap: The colormap. ``AUTO`` uses the theme's diverging colormap when ``center`` is
            given and its sequential colormap otherwise.
        scale: ``Scale.linear()``, or ``Scale.log()`` for values that are all positive.
        center: The value at the middle of a diverging colormap; ``AUTO`` for a sequential
            mapping. A centered mapping is linear.

    Raises:
        TypeError: If an argument has the wrong type, or ``scale`` is ``Scale.category()``.
        DataError: If a value is infinite or beyond 1e300 in magnitude, or not positive on a
            log scale.
        StyleError: If ``center`` is not finite, or is given with a log scale.

    Example:
        >>> import umber as ub
        >>> ax = ub.Figure().add_axes()
        >>> temperature = ub.ColorBy([12.0, 18.5, 24.0], cmap=ub.Cmap.MAGMA)
        >>> ax = ax.scatter([1, 2, 3], [4.0, 2.0, 5.0], color=temperature)
    """

    __slots__ = ("_native",)
    _native: _umber.ColorBy

    def __init__(
        self,
        values: Values[float],
        cmap: Cmap | Auto = AUTO,
        *,
        scale: Scale[float] = Scale.linear(),  # noqa: B008
        center: float | Auto = AUTO,
    ) -> None:
        with api_call():
            context = "ColorBy()"
            check_floats_type(context, "values", values)
            colormap = None if cmap is AUTO else instance(context, "cmap", cmap, Cmap, CMAP_HINT)
            instance(context, "scale", scale, Scale, _SCALE_HINT)
            if scale == Scale.category():
                raise new_error(
                    TypeError,
                    context,
                    "scale is Scale.category(); scale must be Scale.linear() or Scale.log().",
                    parameter="scale",
                    hint=_SCALE_HINT,
                )
            middle = (
                None
                if center is AUTO
                else real(context, "center", center, expected="a float or AUTO", hint=_CENTER_HINT)
            )
            data = floats(context, "values", values)
            native_cmap = None if colormap is None else _cmap.native(colormap)
            native = _umber.ColorBy(data, native_cmap, *_scale.native(scale), middle)
            object.__setattr__(self, "_native", native)

    def __copy__(self) -> Self:
        return self

    def __deepcopy__(self, memo: dict[int, object]) -> Self:
        return self

    def __reduce__(self) -> Never:
        with api_call():
            raise new_error(
                TypeError,
                "ColorBy",
                "a ColorBy cannot be pickled.",
                parameter=None,
                hint="build it again from its values: ColorBy(values).",
            )

    def __setattr__(self, name: str, value: object) -> None:
        raise AttributeError("a ColorBy is immutable")

    def __delattr__(self, name: str) -> None:
        raise AttributeError("a ColorBy is immutable")


_NATIVE: Final = attrgetter("_native")


def native_point_color(context: str, value: object) -> tuple[int, int, float] | _umber.ColorBy:
    """Check the ``color`` argument of a scatter or bubbles and return its native form.

    Args:
        context: The API call, such as ``"Axes.scatter()"``.
        value: The value passed.

    Returns:
        The native color argument of a ``Color``, or the native mapping of a ``ColorBy``.

    Raises:
        TypeError: If the value is neither a ``Color`` nor a ``ColorBy``.

    Example:
        >>> import umber as ub
        >>> from umber._color_by import native_point_color
        >>> native_point_color("Axes.scatter()", ub.Color.AUTO)
        (0, 0, 1.0)
    """
    if isinstance(value, ColorBy):
        return cast(_umber.ColorBy, _NATIVE(value))
    if isinstance(value, Color):
        return _color.native(value)
    message = f"color is {type_name(value)}; color must be of type Color or ColorBy."
    raise new_error(TypeError, context, message, parameter="color", hint=_POINT_COLOR_HINT)
