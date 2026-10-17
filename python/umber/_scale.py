"""``Scale``: how data values map to positions along an axis."""

from collections.abc import Callable
from operator import attrgetter
from typing import Final, Generic, Self, TypeVar, cast, final

from umber import _args, _umber
from umber._errors import new_error
from umber._report import api_call
from umber._types import Time

# An old-style, invariant type variable: a PEP 695 parameter that no method uses would be
# inferred as bivariant, making ``Scale[str]`` assignable to ``Scale[float]``.
T = TypeVar("T")

# The native scale kinds; see ``umber._umber``.
_LINEAR = 0
_LOG = 1
_CATEGORY = 2
_TIME = 3


@final
class Scale(Generic[T]):  # noqa: UP046
    """An axis scale. The type parameter is the type of the values the axis holds.

    Use ``Scale.linear()`` and ``Scale.log()`` for numeric axes (``Scale[float]``),
    ``Scale.category()`` for an axis of names (``Scale[str]``) and ``Scale.time()`` for an axis
    of times (``Scale[Time]``). Scales compare equal when they are the same kind with the same
    base.

    Example:
        >>> import umber as ub
        >>> ub.Scale.log(2) == ub.Scale.log(2.0)
        True
        >>> ub.Scale.category()
        Scale.category()
    """

    __slots__ = ("_base", "_kind")
    _kind: int
    _base: float

    def __init__(self) -> None:
        """Refuse construction; use ``Scale.linear()``, ``Scale.log()`` or another constructor."""
        with api_call():
            raise new_error(
                TypeError,
                "Scale()",
                "Scale has no public constructor; use one of its constructors.",
                parameter=None,
                hint="use Scale.linear(), Scale.log(), Scale.category() or Scale.time().",
            )

    @staticmethod
    def linear() -> "Scale[float]":
        """Return the linear scale, the default for numeric axes.

        Returns:
            The scale.

        Example:
            >>> import umber as ub
            >>> ub.Scale.linear()
            Scale.linear()
        """
        with api_call():
            return cast(Scale[float], _new(_LINEAR, 10.0))

    @staticmethod
    def log(base: float = 10.0) -> "Scale[float]":
        """Return the logarithmic scale with the given base.

        Args:
            base: The base of the logarithm; finite and at least 1.000000000001.
                This lower bound keeps consecutive tick exponents distinct.

        Returns:
            The scale. Axes using it hold positive values only.

        Raises:
            TypeError: If ``base`` is not a number.
            StyleError: If ``base`` is not finite or is less than 1.000000000001.

        Example:
            >>> import umber as ub
            >>> ub.Scale.log(2)
            Scale.log(2.0)
            >>> ub.Scale.log(1.000000000001)
            Scale.log(1.000000000001)
        """
        with api_call():
            value = _args.real(
                "Scale.log()",
                "base",
                base,
                hint="pass a finite number at least 1.000000000001, such as 10.",
            )
            _umber.check_log_base(value)
            return cast(Scale[float], _new(_LOG, value))

    @staticmethod
    def category() -> "Scale[str]":
        """Return the scale of named categories.

        On x it makes vertical bar, box and violin charts. On y, beside a numeric x axis, it
        makes them horizontal, with the categories running top to bottom, and a scatter a
        dot plot. Automatic labels start at category 0 and show every k-th category, using
        the smallest k in 1, 2, 5, 10, 20, 50, ... that keeps their widths (x) or heights (y)
        apart by at least half the tick label font size, as for numeric ticks. Shared axes
        use the same labels. This does not change explicit tick labels or heatmap annotations.

        Returns:
            The scale.

        Example:
            >>> import umber as ub
            >>> ub.Scale.category()
            Scale.category()
        """
        with api_call():
            return cast(Scale[str], _new(_CATEGORY, 10.0))

    @staticmethod
    def time() -> "Scale[Time]":
        """Return the scale of times, for time series.

        A time axis takes ``datetime``, ``date`` and ``numpy.datetime64`` values, NumPy
        ``datetime64`` arrays of any unit, and naive pandas and polars datetime columns. A
        ``date`` is midnight of that day. Times are kept to the microsecond: finer values are
        rounded to the nearest microsecond, halves to even. Times must not carry a time zone,
        and must lie between 0001-01-01 and 9999-12-31; ``NaT`` leaves a gap. The ticks fall on
        calendar steps, such as months or hours, with concise labels.

        Returns:
            The scale.

        Example:
            >>> import umber as ub
            >>> ub.Scale.time()
            Scale.time()
        """
        with api_call():
            return cast(Scale[Time], _new(_TIME, 10.0))

    def _key(self) -> tuple[int, float | None]:
        return self._kind, self._base if self._kind == _LOG else None

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, Scale):
            return NotImplemented
        return self._key() == other._key()

    def __hash__(self) -> int:
        return hash(self._key())

    def __repr__(self) -> str:
        if self._kind == _LINEAR:
            return "Scale.linear()"
        if self._kind == _CATEGORY:
            return "Scale.category()"
        if self._kind == _TIME:
            return "Scale.time()"
        return f"Scale.log({self._base!r})"

    def __copy__(self) -> Self:
        return self

    def __deepcopy__(self, memo: dict[int, object]) -> Self:
        return self

    def __reduce__(self) -> tuple[Callable[[int, float], "Scale[object]"], tuple[int, float]]:
        return _new, (self._kind, self._base)

    def __setattr__(self, name: str, value: object) -> None:
        raise AttributeError("a Scale is immutable")

    def __delattr__(self, name: str) -> None:
        raise AttributeError("a Scale is immutable")


_NATIVE: Final = attrgetter("_kind", "_base")


def native[V](scale: Scale[V]) -> tuple[int, float]:
    """Return the native scale argument of ``scale``: ``(kind, base)``.

    Args:
        scale: The scale.

    Returns:
        The kind (0 for linear, 1 for log, 2 for category, 3 for time) and the base, which
        matters for logarithmic scales only.

    Example:
        >>> import umber as ub
        >>> from umber._scale import native
        >>> native(ub.Scale.log(2))
        (1, 2.0)
    """
    return cast(tuple[int, float], _NATIVE(scale))


def _new(kind: int, base: float) -> Scale[object]:
    """Build a scale; callers narrow the type argument; the class has no public constructor."""
    scale = cast(Scale[object], object.__new__(Scale))
    object.__setattr__(scale, "_kind", kind)
    object.__setattr__(scale, "_base", base)
    return scale
