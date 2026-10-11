"""``Scale``: how data values map to positions along an axis."""

import math
import numbers
from collections.abc import Callable
from typing import Generic, Self, TypeVar, cast, final

from umber import _umber
from umber._errors import new_error
from umber._report import api_call

# An old-style, invariant type variable: a PEP 695 parameter that no method uses would be
# inferred as bivariant, making ``Scale[str]`` assignable to ``Scale[float]``.
T = TypeVar("T")

# The native scale kinds; see ``umber._umber``.
_LINEAR = 0
_LOG = 1
_CATEGORY = 2


@final
class Scale(Generic[T]):  # noqa: UP046
    """An axis scale. The type parameter is the type of the values the axis holds.

    Use ``Scale.linear()`` and ``Scale.log()`` for numeric axes (``Scale[float]``) and
    ``Scale.category()`` for an axis of names (``Scale[str]``). Scales compare equal when
    they are the same kind with the same base.

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
        """Refuse construction; use ``Scale.linear()``, ``Scale.log()`` or ``Scale.category()``."""
        with api_call():
            raise new_error(
                TypeError,
                "Scale()",
                "Scale has no public constructor; use one of its constructors.",
                parameter=None,
                hint="use Scale.linear(), Scale.log() or Scale.category().",
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
            base: The base of the logarithm; finite and greater than 1.

        Returns:
            The scale. Axes using it hold positive values only.

        Raises:
            TypeError: If ``base`` is not a number.
            StyleError: If ``base`` is not finite and greater than 1.

        Example:
            >>> import umber as ub
            >>> ub.Scale.log(2)
            Scale.log(2.0)
        """
        with api_call():
            if isinstance(base, bool) or not isinstance(base, numbers.Real):
                raise new_error(
                    TypeError,
                    "Scale.log()",
                    f"base is {type(base).__name__}; base must be a float.",
                    parameter="base",
                    hint="pass a number greater than 1, such as 10.",
                )
            try:
                value = float(base)
            except OverflowError:
                # An int beyond the float range: the native check rejects the infinity.
                value = math.inf if cast(int, base) > 0 else -math.inf
            _umber.check_log_base(value)
            return cast(Scale[float], _new(_LOG, value))

    @staticmethod
    def category() -> "Scale[str]":
        """Return the scale of named categories, used for bar charts.

        Returns:
            The scale.

        Example:
            >>> import umber as ub
            >>> ub.Scale.category()
            Scale.category()
        """
        with api_call():
            return cast(Scale[str], _new(_CATEGORY, 10.0))

    def _kind_and_base(self) -> tuple[int, float]:
        """Return the native scale argument: ``(kind, base)``; the base matters for logs only."""
        return self._kind, self._base

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


def _new(kind: int, base: float) -> Scale[object]:
    """Build a scale; callers narrow the type argument; the class has no public constructor."""
    scale = cast(Scale[object], object.__new__(Scale))
    object.__setattr__(scale, "_kind", kind)
    object.__setattr__(scale, "_base", base)
    return scale
