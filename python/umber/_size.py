"""``Size``: the width of a line or the size of a marker, relative to the theme or in points."""

from collections.abc import Callable
from operator import attrgetter
from typing import ClassVar, Final, Self, cast, final

from umber import _args, _umber
from umber._errors import new_error
from umber._report import api_call

# The native size kinds; see ``umber._umber``.
_AUTO = 0
_TIMES = 1
_POINTS = 2


@final
class Size:
    """An immutable size for a line width or a marker size.

    ``Size.AUTO`` is the size of the theme. ``Size.times()`` is a multiple of it, so an
    emphasized series stays emphasized under any theme. ``Size.pt()`` is a fixed size in
    points that ignores the theme. Sizes compare equal when they are the same kind with the
    same value.

    Example:
        >>> import umber as ub
        >>> ub.Size.times(2) == ub.Size.times(2.0)
        True
        >>> ub.Size.pt(0.75)
        Size.pt(0.75)
    """

    __slots__ = ("_kind", "_value")
    _kind: int
    _value: float

    AUTO: ClassVar["Size"]
    """The size of the theme: its line width or its marker size.

    Example:
        >>> import umber as ub
        >>> ub.Size.AUTO
        Size.AUTO
    """

    def __init__(self) -> None:
        """Refuse construction; use ``Size.AUTO``, ``Size.times()`` or ``Size.pt()``."""
        with api_call():
            raise new_error(
                TypeError,
                "Size()",
                "Size has no public constructor; use Size.AUTO or one of its constructors.",
                parameter=None,
                hint="use Size.AUTO, Size.times() or Size.pt().",
            )

    @staticmethod
    def times(factor: float) -> "Size":
        """Return a size that is ``factor`` times the size of the theme.

        Args:
            factor: The multiple; greater than 0 and at most 10.

        Returns:
            The size; it is resolved against the theme when the figure is drawn.

        Raises:
            TypeError: If ``factor`` is not a number.
            StyleError: If ``factor`` is not greater than 0 and at most 10.

        Example:
            >>> import umber as ub
            >>> ub.Size.times(1.5)
            Size.times(1.5)
        """
        with api_call():
            value = _args.real("Size.times()", "factor", factor)
            _umber.check_size_times(value)
            return _new(_TIMES, value)

    @staticmethod
    def pt(points: float) -> "Size":
        """Return a fixed size in points, whatever the theme.

        Args:
            points: The size in points; greater than 0 and at most 72 (one inch).

        Returns:
            The size.

        Raises:
            TypeError: If ``points`` is not a number.
            StyleError: If ``points`` is not greater than 0 and at most 72.

        Example:
            >>> import umber as ub
            >>> ub.Size.pt(2)
            Size.pt(2)
        """
        with api_call():
            value = _args.real("Size.pt()", "points", points)
            _umber.check_size_points(value)
            return _new(_POINTS, value)

    def _key(self) -> tuple[int, float]:
        return self._kind, self._value

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, Size):
            return NotImplemented
        return self._key() == other._key()

    def __hash__(self) -> int:
        return hash(self._key())

    def __repr__(self) -> str:
        if self._kind == _AUTO:
            return "Size.AUTO"
        number = repr(self._value).removesuffix(".0")
        return f"Size.{'times' if self._kind == _TIMES else 'pt'}({number})"

    def __copy__(self) -> Self:
        return self

    def __deepcopy__(self, memo: dict[int, object]) -> Self:
        return self

    def __reduce__(self) -> tuple[Callable[[int, float], "Size"], tuple[int, float]]:
        return _new, self._key()

    def __setattr__(self, name: str, value: object) -> None:
        raise AttributeError("a Size is immutable")

    def __delattr__(self, name: str) -> None:
        raise AttributeError("a Size is immutable")


_NATIVE: Final = attrgetter("_kind", "_value")


def native(size: Size) -> tuple[int, float]:
    """Return the native size argument of ``size``: ``(kind, value)``.

    Args:
        size: The size.

    Returns:
        The kind (0 for ``Size.AUTO``, 1 for ``Size.times()``, 2 for ``Size.pt()``) and the
        factor or the points; the value is 0.0 for ``Size.AUTO``.

    Example:
        >>> import umber as ub
        >>> from umber._size import native
        >>> native(ub.Size.times(2))
        (1, 2.0)
    """
    return cast(tuple[int, float], _NATIVE(size))


def _new(kind: int, value: float) -> "Size":
    """Build a size; the class has no public constructor."""
    size = object.__new__(Size)
    object.__setattr__(size, "_kind", kind)
    object.__setattr__(size, "_value", value)
    return size


Size.AUTO = _new(_AUTO, 0.0)
