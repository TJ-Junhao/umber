"""``Font``: an immutable font family that a theme sets its text in."""

from collections.abc import Callable
from operator import attrgetter
from typing import ClassVar, Final, Self, cast, final

from umber import _umber
from umber._errors import new_error
from umber._report import api_call

# The native font argument kinds; see ``umber._umber``.
_BUNDLED = 0


@final
class Font:
    """An immutable font family for a theme's titles or body text.

    The families bundled with umber are constants of this class. A font compares equal to
    another that names the same family.

    Example:
        >>> import umber as ub
        >>> theme = ub.Theme.PIGMENT.with_title_font(ub.Font.INTER)
        >>> ub.Font.INTER == ub.Font.GEIST
        False
    """

    __slots__ = ("_name", "_native")
    _name: str
    _native: tuple[int, int]

    SOURCE_SERIF_4: ClassVar["Font"]
    """Source Serif 4, a bundled serif face.

    Example:
        >>> import umber as ub
        >>> ub.Font.SOURCE_SERIF_4
        Font.SOURCE_SERIF_4
    """

    IBM_PLEX_SANS: ClassVar["Font"]
    """IBM Plex Sans, a bundled sans-serif face.

    Example:
        >>> import umber as ub
        >>> ub.Font.IBM_PLEX_SANS
        Font.IBM_PLEX_SANS
    """

    GEIST: ClassVar["Font"]
    """Geist, a bundled sans-serif face.

    Example:
        >>> import umber as ub
        >>> ub.Font.GEIST
        Font.GEIST
    """

    INTER: ClassVar["Font"]
    """Inter, a bundled sans-serif face.

    Example:
        >>> import umber as ub
        >>> ub.Font.INTER
        Font.INTER
    """

    JETBRAINS_MONO: ClassVar["Font"]
    """JetBrains Mono, a bundled monospaced face.

    Example:
        >>> import umber as ub
        >>> ub.Font.JETBRAINS_MONO
        Font.JETBRAINS_MONO
    """

    def __init__(self) -> None:
        """Refuse construction; use one of the font constants."""
        with api_call():
            raise new_error(
                TypeError,
                "Font()",
                "Font has no public constructor; use a bundled font.",
                parameter=None,
                hint=FONT_HINT,
            )

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, Font):
            return NotImplemented
        return self._native == other._native

    def __hash__(self) -> int:
        return hash(self._native)

    def __repr__(self) -> str:
        return f"Font.{self._name}"

    def __copy__(self) -> Self:
        return self

    def __deepcopy__(self, memo: dict[int, object]) -> Self:
        return self

    def __reduce__(self) -> tuple[Callable[[int], "Font"], tuple[int]]:
        return _bundled, (self._native[1],)

    def __setattr__(self, name: str, value: object) -> None:
        raise AttributeError("a Font is immutable")

    def __delattr__(self, name: str) -> None:
        raise AttributeError("a Font is immutable")


def _new(name: str, native: tuple[int, int]) -> Font:
    """Build a font; the class has no public constructor."""
    font = object.__new__(Font)
    object.__setattr__(font, "_name", name)
    object.__setattr__(font, "_native", native)
    return font


_NATIVE: Final = attrgetter("_native")


def native(font: Font) -> tuple[int, int]:
    """Return the native font argument of ``font``: ``(kind, payload)``.

    Args:
        font: The font.

    Returns:
        The kind (0 for a bundled font) and the payload (the index of the bundled font).

    Example:
        >>> import umber as ub
        >>> from umber._font import native
        >>> native(ub.Font.INTER)
        (0, 3)
    """
    return cast(tuple[int, int], _NATIVE(font))


_BUNDLED_FONTS: Final[list[Font]] = []
for _name, _index in _umber.bundled_fonts():
    _constant = _new(_name, (_BUNDLED, _index))
    setattr(Font, _name, _constant)
    _BUNDLED_FONTS.append(_constant)


def _bundled(index: int) -> Font:
    """Return the bundled font at ``index``, as pickling stores it."""
    return _BUNDLED_FONTS[index]


FONT_HINT: Final = f"use {', '.join(map(repr, _BUNDLED_FONTS[:-1]))} or {_BUNDLED_FONTS[-1]!r}."
"""The hint of the errors for a value that is not a ``Font``: it lists the bundled fonts."""
