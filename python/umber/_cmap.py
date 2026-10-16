"""``Cmap``: an immutable colormap that maps numbers to colors."""

from collections.abc import Sequence
from operator import attrgetter
from typing import ClassVar, Final, Never, Self, cast, final

from umber import _color, _umber
from umber._color import Color
from umber._errors import new_error
from umber._report import api_call

CMAP_HINT: Final = "use Cmap.VIRIDIS, Cmap.BLUE_RED or Cmap.from_colors([Color.BLUE, Color.RED])."
_BUILTINS: Final = ("VIRIDIS", "MAGMA", "CIVIDIS", "BLUE_RED")


@final
class Cmap:
    """An immutable colormap: colors spaced evenly from 0 to 1.

    Use a built-in colormap, or build one from your own colors with ``Cmap.from_colors()``.
    The colors between two neighbouring stops are mixed in Oklab, so a colormap is
    perceptually even between its stops. Colormaps compare equal when their stops are equal,
    and can be hashed. Unlike a ``Color``, a colormap cannot be pickled.

    Example:
        >>> import umber as ub
        >>> ub.Cmap.VIRIDIS.reversed().reversed() == ub.Cmap.VIRIDIS
        True
        >>> ub.Cmap.from_colors([ub.Color.BLUE, ub.Color.RED])
        Cmap.from_colors([Color.BLUE, Color.RED])
    """

    __slots__ = ("_native",)
    _native: _umber.Cmap

    VIRIDIS: ClassVar["Cmap"]
    """The perceptually uniform sequential colormap from dark purple through teal to yellow.

    Example:
        >>> import umber as ub
        >>> ub.Cmap.VIRIDIS
        Cmap.VIRIDIS
    """

    MAGMA: ClassVar["Cmap"]
    """The sequential colormap from black through purple and orange to pale yellow.

    Example:
        >>> import umber as ub
        >>> ub.Cmap.MAGMA
        Cmap.MAGMA
    """

    CIVIDIS: ClassVar["Cmap"]
    """The sequential colormap from dark blue to yellow, readable with color blindness.

    Example:
        >>> import umber as ub
        >>> ub.Cmap.CIVIDIS
        Cmap.CIVIDIS
    """

    BLUE_RED: ClassVar["Cmap"]
    """The diverging colormap from blue through near-white to red.

    Example:
        >>> import umber as ub
        >>> ub.Cmap.BLUE_RED
        Cmap.BLUE_RED
    """

    def __init__(self) -> None:
        """Refuse construction; use a built-in colormap or ``Cmap.from_colors()``."""
        with api_call():
            raise new_error(
                TypeError,
                "Cmap()",
                "Cmap has no public constructor; use a built-in colormap or Cmap.from_colors().",
                parameter=None,
                hint=CMAP_HINT,
            )

    @staticmethod
    def from_colors(colors: Sequence[Color]) -> "Cmap":
        """Return the colormap through ``colors``, spaced evenly from 0 to 1.

        Args:
            colors: Two to 16 concrete, opaque colors, in a list or tuple.

        Returns:
            The colormap.

        Raises:
            TypeError: If ``colors`` is not a sequence of ``Color``.
            StyleError: If a color is ``Color.AUTO``, ``Color.palette()`` or not opaque, or the
                number of colors is not between 2 and 16.

        Example:
            >>> import umber as ub
            >>> cmap = ub.Cmap.from_colors([ub.Color.WHITE, ub.Color.hex("#3E5B8C")])
            >>> cmap == ub.Cmap.VIRIDIS
            False
        """
        with api_call():
            arguments = _color.native_sequence("Cmap.from_colors()", colors)
            return _wrap(_umber.Cmap.from_colors(arguments))

    def reversed(self) -> "Cmap":
        """Return this colormap read backwards.

        Returns:
            A new colormap; this one is unchanged.

        Example:
            >>> import umber as ub
            >>> ub.Cmap.MAGMA.reversed()
            Cmap.MAGMA.reversed()
        """
        return _wrap(self._native.reversed())

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, Cmap):
            return NotImplemented
        return self._native == other._native

    def __hash__(self) -> int:
        return hash(tuple(self._native.colors()))

    def __repr__(self) -> str:
        for name in _BUILTINS:
            builtin = cast(Cmap, getattr(Cmap, name))
            if self == builtin:
                return f"Cmap.{name}"
            if self == builtin.reversed():
                return f"Cmap.{name}.reversed()"
        stops = (Color.hex(f"#{packed >> 8:06X}") for packed in self._native.colors())
        return f"Cmap.from_colors([{', '.join(map(repr, stops))}])"

    def __copy__(self) -> Self:
        return self

    def __deepcopy__(self, memo: dict[int, object]) -> Self:
        return self

    def __reduce__(self) -> Never:
        with api_call():
            raise new_error(
                TypeError,
                "Cmap",
                "a Cmap cannot be pickled.",
                parameter=None,
                hint="rebuild it with Cmap.from_colors([Color.BLUE, Color.RED]).",
            )

    def __setattr__(self, name: str, value: object) -> None:
        raise AttributeError("a Cmap is immutable")

    def __delattr__(self, name: str) -> None:
        raise AttributeError("a Cmap is immutable")


_NATIVE: Final = attrgetter("_native")


def native(cmap: Cmap) -> _umber.Cmap:
    """Return the native colormap of ``cmap``.

    Args:
        cmap: The colormap.

    Returns:
        The native colormap object.

    Example:
        >>> import umber as ub
        >>> from umber._cmap import native
        >>> native(ub.Cmap.VIRIDIS) == native(ub.Cmap.VIRIDIS)
        True
    """
    return cast(_umber.Cmap, _NATIVE(cmap))


def _wrap(value: _umber.Cmap) -> Cmap:
    """Build a colormap around a native one; the class has no public constructor."""
    cmap = object.__new__(Cmap)
    object.__setattr__(cmap, "_native", value)
    return cmap


Cmap.VIRIDIS = _wrap(_umber.Cmap.viridis())
Cmap.MAGMA = _wrap(_umber.Cmap.magma())
Cmap.CIVIDIS = _wrap(_umber.Cmap.cividis())
Cmap.BLUE_RED = _wrap(_umber.Cmap.blue_red())
