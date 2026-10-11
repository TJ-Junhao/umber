"""``Color``: an immutable color, either fixed or resolved from the theme when drawn."""

import math
import numbers
from collections.abc import Callable
from typing import ClassVar, Self, cast, final

from umber import _umber
from umber._errors import StyleError, new_error
from umber._report import api_call

_INT64_MIN = -(2**63)
_INT64_MAX = 2**63 - 1

# The native color argument kinds; see ``umber._umber``.
_AUTO = 0
_PALETTE = 1
_FIXED = 2


def _clamp(value: int) -> int:
    """Limit ``value`` to the native integer range."""
    return min(max(value, _INT64_MIN), _INT64_MAX)


def _type_error(context: str, name: str, value: object, expected: str, hint: str) -> TypeError:
    """Build the ``TypeError`` for a parameter of the wrong type."""
    return new_error(
        TypeError,
        context,
        f"{name} is {type(value).__name__}; {name} must be a {expected}.",
        parameter=name,
        hint=hint,
    )


def _check_text(context: str, name: str, value: object, hint: str) -> str:
    if not isinstance(value, str):
        raise _type_error(context, name, value, "str", hint)
    return value


def _check_int(context: str, name: str, value: object) -> int:
    if isinstance(value, bool) or not isinstance(value, numbers.Integral):
        raise _type_error(context, name, value, "int", "pass a whole number such as 3.")
    return int(value)


def _check_float(context: str, name: str, value: object, hint: str) -> float:
    if isinstance(value, bool) or not isinstance(value, numbers.Real):
        raise _type_error(context, name, value, "float", hint)
    try:
        return float(value)
    except OverflowError:
        # An int beyond the float range: the native check rejects the infinity.
        return math.inf if cast(int, value) > 0 else -math.inf


@final
class Color:
    """An immutable color for series, themes and other drawn elements.

    ``Color.AUTO`` takes the next color of the theme palette. The named constants and the
    colors made by ``hex()`` and ``rgb()`` are fixed. ``palette()`` picks a color of the
    theme. ``alpha()`` returns the same color with another opacity. A color compares equal
    to another with the same value.

    Example:
        >>> import umber as ub
        >>> ub.Color.hex("#3366B8") == ub.Color.BLUE
        True
        >>> ub.Color.palette(2).alpha(0.5)
        Color.palette(2).alpha(0.5)
    """

    __slots__ = ("_alpha", "_kind", "_payload")
    _kind: int
    _payload: int
    _alpha: float

    AUTO: ClassVar["Color"]
    """The next color of the theme palette, in series order within the axes.

    Example:
        >>> import umber as ub
        >>> ub.Color.AUTO
        Color.AUTO
    """

    BLACK: ClassVar["Color"]
    """The fixed color ``#1A1A1A``; it does not depend on the theme.

    Example:
        >>> import umber as ub
        >>> ub.Color.BLACK
        Color.BLACK
    """

    WHITE: ClassVar["Color"]
    """The fixed color ``#FFFFFF``; it does not depend on the theme.

    Example:
        >>> import umber as ub
        >>> ub.Color.WHITE
        Color.WHITE
    """

    GRAY: ClassVar["Color"]
    """The fixed color ``#8C8C8C``; it does not depend on the theme.

    Example:
        >>> import umber as ub
        >>> ub.Color.GRAY
        Color.GRAY
    """

    RED: ClassVar["Color"]
    """The fixed color ``#D64933``; it does not depend on the theme.

    Example:
        >>> import umber as ub
        >>> ub.Color.RED
        Color.RED
    """

    ORANGE: ClassVar["Color"]
    """The fixed color ``#E8853D``; it does not depend on the theme.

    Example:
        >>> import umber as ub
        >>> ub.Color.ORANGE
        Color.ORANGE
    """

    YELLOW: ClassVar["Color"]
    """The fixed color ``#E9B949``; it does not depend on the theme.

    Example:
        >>> import umber as ub
        >>> ub.Color.YELLOW
        Color.YELLOW
    """

    GREEN: ClassVar["Color"]
    """The fixed color ``#4E9A5E``; it does not depend on the theme.

    Example:
        >>> import umber as ub
        >>> ub.Color.GREEN
        Color.GREEN
    """

    TEAL: ClassVar["Color"]
    """The fixed color ``#2E8B8B``; it does not depend on the theme.

    Example:
        >>> import umber as ub
        >>> ub.Color.TEAL
        Color.TEAL
    """

    BLUE: ClassVar["Color"]
    """The fixed color ``#3366B8``; it does not depend on the theme.

    Example:
        >>> import umber as ub
        >>> ub.Color.BLUE
        Color.BLUE
    """

    PURPLE: ClassVar["Color"]
    """The fixed color ``#7D5BA6``; it does not depend on the theme.

    Example:
        >>> import umber as ub
        >>> ub.Color.PURPLE
        Color.PURPLE
    """

    PINK: ClassVar["Color"]
    """The fixed color ``#D46A92``; it does not depend on the theme.

    Example:
        >>> import umber as ub
        >>> ub.Color.PINK
        Color.PINK
    """

    BROWN: ClassVar["Color"]
    """The fixed color ``#7A5236``; it does not depend on the theme.

    Example:
        >>> import umber as ub
        >>> ub.Color.BROWN
        Color.BROWN
    """

    def __init__(self) -> None:
        """Refuse construction; use a constant or one of the constructors."""
        with api_call():
            raise new_error(
                TypeError,
                "Color()",
                "Color has no public constructor; use a named color or a Color constructor.",
                parameter=None,
                hint='use Color.hex("#3366B8"), Color.rgb(), Color.palette() or a named color.',
            )

    @staticmethod
    def hex(code: str) -> "Color":
        """Return the fixed color written as ``#RRGGBB`` or ``#RRGGBBAA``.

        Args:
            code: The color code; ``AA`` is the opacity.

        Returns:
            The color.

        Raises:
            TypeError: If ``code`` is not a ``str``.
            StyleError: If ``code`` is not a valid hex color.

        Example:
            >>> import umber as ub
            >>> ub.Color.hex("#3366B880")
            Color.hex("#3366B880")
        """
        with api_call():
            text = _check_text("Color.hex()", "code", code, 'pass a string such as "#3366B8".')
            return _new(_FIXED, _umber.color_hex(text), 1.0)

    @staticmethod
    def rgb(r: int, g: int, b: int) -> "Color":
        """Return the opaque fixed color with the given channels.

        Args:
            r: The red channel, 0 to 255.
            g: The green channel, 0 to 255.
            b: The blue channel, 0 to 255.

        Returns:
            The color.

        Raises:
            TypeError: If a channel is not an ``int``.
            StyleError: If a channel is outside 0 to 255.

        Example:
            >>> import umber as ub
            >>> ub.Color.rgb(51, 102, 184) == ub.Color.BLUE
            True
        """
        with api_call():
            red = _check_int("Color.rgb()", "r", r)
            green = _check_int("Color.rgb()", "g", g)
            blue = _check_int("Color.rgb()", "b", b)
            # Channels beyond the native integer range are clamped; the native check rejects
            # them with its own message.
            return _new(_FIXED, _umber.color_rgb(_clamp(red), _clamp(green), _clamp(blue)), 1.0)

    @staticmethod
    def palette(index: int) -> "Color":
        """Return the ``index``-th color of the figure's theme palette.

        Indexes past the end of the palette wrap around. The largest index is
        9223372036854775807.

        Args:
            index: The palette position, 0 or greater.

        Returns:
            The color; it is resolved when the figure is drawn.

        Raises:
            TypeError: If ``index`` is not an ``int``.
            StyleError: If ``index`` is negative or above 9223372036854775807.

        Example:
            >>> import umber as ub
            >>> ub.Color.palette(0)
            Color.palette(0)
        """
        with api_call():
            position = _check_int("Color.palette()", "index", index)
            if position > _INT64_MAX:
                raise new_error(
                    StyleError,
                    "Color.palette()",
                    f"index is {position}; a palette index must be at most {_INT64_MAX}.",
                    parameter="index",
                    hint="indexes wrap around the palette, so a smaller one picks the same color.",
                )
            position = max(position, _INT64_MIN)
            _umber.check_palette_index(position)
            return _new(_PALETTE, position, 1.0)

    def alpha(self, a: float) -> "Color":
        """Return this color with its opacity replaced by ``a``.

        Args:
            a: The opacity, 0.0 (transparent) to 1.0 (opaque).

        Returns:
            A new color; this one is unchanged.

        Raises:
            TypeError: If ``a`` is not a number.
            StyleError: If ``a`` is outside 0.0 to 1.0.

        Example:
            >>> import umber as ub
            >>> ub.Color.BLUE.alpha(0.5)
            Color.hex("#3366B880")
        """
        with api_call():
            opacity = _check_float("Color.alpha()", "a", a, "pass a number from 0.0 to 1.0.")
            if self._kind == _FIXED:
                return _new(_FIXED, _umber.color_with_alpha(self._payload, opacity), 1.0)
            _umber.check_alpha(opacity)
            return _new(self._kind, self._payload, opacity)

    def _argument(self) -> tuple[int, int, float]:
        """Return the native color argument: ``(kind, payload, alpha)``."""
        return self._kind, self._payload, self._alpha

    def _key(self) -> tuple[int, int, float]:
        return self._kind, self._payload, self._alpha

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, Color):
            return NotImplemented
        return self._key() == other._key()

    def __hash__(self) -> int:
        return hash(self._key())

    def __repr__(self) -> str:
        if self._kind == _FIXED:
            name = _NAMES.get(self._payload)
            if name is not None:
                return f"Color.{name}"
            digits = f"{self._payload:08X}"
            return f'Color.hex("#{digits[:6] if self._payload & 0xFF == 0xFF else digits}")'
        base = "Color.AUTO" if self._kind == _AUTO else f"Color.palette({self._payload})"
        return base if self._alpha == 1.0 else f"{base}.alpha({self._alpha!r})"

    def __copy__(self) -> Self:
        return self

    def __deepcopy__(self, memo: dict[int, object]) -> Self:
        return self

    def __reduce__(self) -> tuple[Callable[[int, int, float], "Color"], tuple[int, int, float]]:
        return _new, self._key()

    def __setattr__(self, name: str, value: object) -> None:
        raise AttributeError("a Color is immutable")

    def __delattr__(self, name: str) -> None:
        raise AttributeError("a Color is immutable")


def _new(kind: int, payload: int, alpha: float) -> "Color":
    """Build a color; the class has no public constructor."""
    color = object.__new__(Color)
    object.__setattr__(color, "_kind", kind)
    object.__setattr__(color, "_payload", payload)
    object.__setattr__(color, "_alpha", alpha)
    return color


_NAMES: dict[int, str] = {}
for _name, _packed in _umber.named_colors():
    setattr(Color, _name, _new(_FIXED, _packed, 1.0))
    _NAMES[_packed] = _name
Color.AUTO = _new(_AUTO, 0, 1.0)
