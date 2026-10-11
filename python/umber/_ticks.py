"""``TickFormat``: how the numbers in tick labels are written."""

from collections.abc import Callable
from operator import attrgetter
from typing import ClassVar, Final, Self, cast, final

from umber import _args, _umber
from umber._errors import new_error
from umber._report import api_call

# The native tick format kinds; see ``umber._umber``.
_AUTO = 0
_FIXED = 1
_PERCENT = 2
_SCIENTIFIC = 3


@final
class TickFormat:
    """An immutable format for the tick labels of a linear or log axis.

    ``TickFormat.AUTO`` chooses the decimals from the tick step. ``TickFormat.fixed()``,
    ``TickFormat.percent()`` and ``TickFormat.scientific()`` write every label the same way,
    and ``affix()`` adds text before and after each label. Negative numbers use the minus sign
    U+2212 and zero is never signed. Only ``AUTO`` adds decimals when two ticks would share a
    label; with another format, that is your choice. Formats compare equal when they write
    labels the same way.

    Example:
        >>> import umber as ub
        >>> ub.TickFormat.percent(1).affix(suffix=" pts")
        TickFormat.percent(1).affix(suffix=' pts')
        >>> ub.TickFormat.fixed(2) == ub.TickFormat.fixed(2)
        True
    """

    __slots__ = ("_decimals", "_kind", "_prefix", "_suffix")
    _kind: int
    _decimals: int
    _prefix: str
    _suffix: str

    AUTO: ClassVar["TickFormat"]
    """The automatic labels: the decimals follow from the tick step, and a label is written with
    more precision when two ticks would otherwise share it.

    Example:
        >>> import umber as ub
        >>> ub.TickFormat.AUTO
        TickFormat.AUTO
    """

    def __init__(self) -> None:
        """Refuse construction; use ``TickFormat.AUTO`` or one of its constructors."""
        with api_call():
            raise new_error(
                TypeError,
                "TickFormat()",
                "TickFormat has no public constructor; use TickFormat.AUTO or one of its "
                "constructors.",
                parameter=None,
                hint="use TickFormat.AUTO, TickFormat.fixed(), TickFormat.percent() or "
                "TickFormat.scientific().",
            )

    @staticmethod
    def fixed(decimals: int) -> "TickFormat":
        """Return a format that writes every label with ``decimals`` decimals, such as ``2.50``.

        Args:
            decimals: The number of decimals, from 0 to 12.

        Returns:
            The format.

        Raises:
            TypeError: If ``decimals`` is not an ``int``.
            StyleError: If ``decimals`` is not between 0 and 12.

        Example:
            >>> import umber as ub
            >>> ub.TickFormat.fixed(2)
            TickFormat.fixed(2)
        """
        with api_call():
            places = _args.integer("TickFormat.fixed()", "decimals", decimals)
            return _checked(_FIXED, places, "", "")

    @staticmethod
    def percent(decimals: int = 0) -> "TickFormat":
        """Return a format that writes each value times 100 followed by ``%``, such as ``12.5%``.

        Args:
            decimals: The number of decimals of the percentage, from 0 to 12.

        Returns:
            The format. A value of 0.125 is written ``12.5%`` with one decimal.

        Raises:
            TypeError: If ``decimals`` is not an ``int``.
            StyleError: If ``decimals`` is not between 0 and 12.

        Example:
            >>> import umber as ub
            >>> ub.TickFormat.percent(1)
            TickFormat.percent(1)
        """
        with api_call():
            places = _args.integer("TickFormat.percent()", "decimals", decimals)
            return _checked(_PERCENT, places, "", "")

    @staticmethod
    def scientific() -> "TickFormat":
        """Return a format that always writes labels in scientific form, such as ``2.5e6``.

        Returns:
            The format.

        Example:
            >>> import umber as ub
            >>> ub.TickFormat.scientific()
            TickFormat.scientific()
        """
        return _new(_SCIENTIFIC, 0, "", "")

    def affix(self, prefix: str = "", suffix: str = "") -> "TickFormat":
        """Return this format with text before and after every label.

        The text replaces any prefix and suffix this format had. A percent label puts its
        ``%`` before the suffix. A format with a prefix or a suffix cannot be combined with
        labels given by ``Axes.xticks_labeled()`` or ``Axes.yticks_labeled()``.

        Args:
            prefix: The text before each label, at most 1,000 characters.
            suffix: The text after each label, at most 1,000 characters.

        Returns:
            The new format; this one is unchanged.

        Raises:
            TypeError: If ``prefix`` or ``suffix`` is not a ``str``.
            StyleError: If ``prefix`` or ``suffix`` is too long.

        Example:
            >>> import umber as ub
            >>> ub.TickFormat.fixed(2).affix("$")
            TickFormat.fixed(2).affix(prefix='$')
        """
        with api_call():
            before = _args.text("TickFormat.affix()", "prefix", prefix)
            after = _args.text("TickFormat.affix()", "suffix", suffix)
            return _checked(self._kind, self._decimals, before, after)

    def _key(self) -> tuple[int, int, str, str]:
        return self._kind, self._decimals, self._prefix, self._suffix

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, TickFormat):
            return NotImplemented
        return self._key() == other._key()

    def __hash__(self) -> int:
        return hash(self._key())

    def __repr__(self) -> str:
        if self._kind == _AUTO:
            text = "TickFormat.AUTO"
        elif self._kind == _SCIENTIFIC:
            text = "TickFormat.scientific()"
        else:
            name = "fixed" if self._kind == _FIXED else "percent"
            text = f"TickFormat.{name}({self._decimals})"
        affixes = [
            f"{name}={value!r}"
            for name, value in (("prefix", self._prefix), ("suffix", self._suffix))
            if value
        ]
        return f"{text}.affix({', '.join(affixes)})" if affixes else text

    def __copy__(self) -> Self:
        return self

    def __deepcopy__(self, memo: dict[int, object]) -> Self:
        return self

    def __reduce__(
        self,
    ) -> tuple[Callable[[int, int, str, str], "TickFormat"], tuple[int, int, str, str]]:
        return _new, self._key()

    def __setattr__(self, name: str, value: object) -> None:
        raise AttributeError("a TickFormat is immutable")

    def __delattr__(self, name: str) -> None:
        raise AttributeError("a TickFormat is immutable")


_NATIVE: Final = attrgetter("_kind", "_decimals", "_prefix", "_suffix")


def native(format: TickFormat) -> tuple[int, int, str, str]:
    """Return the native tick format argument of ``format``.

    Args:
        format: The tick format.

    Returns:
        The kind (0 for ``TickFormat.AUTO``, 1 for ``fixed()``, 2 for ``percent()``, 3 for
        ``scientific()``), the decimals (0 for the kinds without), the prefix and the suffix.

    Example:
        >>> import umber as ub
        >>> from umber._ticks import native
        >>> native(ub.TickFormat.fixed(2).affix("$"))
        (1, 2, '$', '')
    """
    return cast(tuple[int, int, str, str], _NATIVE(format))


def _checked(kind: int, decimals: int, prefix: str, suffix: str) -> TickFormat:
    """Build a format after the native layer has checked its decimals and texts."""
    _umber.check_tick_format((kind, decimals, prefix, suffix))
    return _new(kind, decimals, prefix, suffix)


def _new(kind: int, decimals: int, prefix: str, suffix: str) -> TickFormat:
    """Build a format; the class has no public constructor."""
    format = object.__new__(TickFormat)
    object.__setattr__(format, "_kind", kind)
    object.__setattr__(format, "_decimals", decimals)
    object.__setattr__(format, "_prefix", prefix)
    object.__setattr__(format, "_suffix", suffix)
    return format


TickFormat.AUTO = _new(_AUTO, 0, "", "")
