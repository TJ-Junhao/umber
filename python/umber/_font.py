"""``Font``: an immutable font family that a theme sets its text in."""

import os
from collections.abc import Callable
from operator import attrgetter
from pathlib import Path
from typing import ClassVar, Final, Self, cast, final

from umber import _args, _umber
from umber._enums import AUTO, Auto
from umber._errors import DataError, UsageError, details, new_error
from umber._report import api_call

# The native font argument kinds; see ``umber._umber``.
_BUNDLED = 0
_FILE = 1
_SYSTEM = 2

type _Native = tuple[int, int | _umber.FontValue]
type _Source = int | str | tuple[bytes, bytes | None, str]


@final
class Font:
    """An immutable font family for a theme's titles or body text.

    The bundled families are constants. ``from_file()`` reads regular and optional bold
    files; ``system()`` finds an installed family. Fonts compare equal by bundled constant,
    file bytes' SHA-256 digests, or installed family, never by a file's path.

    Variable fonts use their default instance. A font without bold is allowed until bold text
    is drawn; no synthetic bold is made. Font licensing is your responsibility. Editable SVG
    viewers need the font installed; outlined SVG and PNG do not.

    Example:
        >>> import umber as ub
        >>> theme = ub.Theme.PIGMENT.with_title_font(ub.Font.INTER)
        >>> ub.Font.INTER == ub.Font.GEIST
        False
    """

    __slots__ = ("_name", "_native", "_source")
    _name: str
    _native: _Native
    _source: _Source

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

    @staticmethod
    def from_file(
        regular: str | os.PathLike[str], *, bold: str | os.PathLike[str] | Auto = AUTO
    ) -> "Font":
        """Read a TrueType or OpenType regular file and an optional bold file.

        Bytes are kept in memory, so changing or removing the files does not change the font.
        The family name comes from the regular file. Collections (``.ttc``) are not accepted;
        use ``Font.system()`` to choose a family from an installed collection.

        Args:
            regular: Path to the regular ``.ttf`` or ``.otf`` file.
            bold: Path to the bold face, used for SemiBold text, or ``AUTO`` for no bold face.

        Returns:
            An immutable font for ``Theme.with_title_font()`` or ``Theme.with_body_font()``.

        Raises:
            TypeError: If a path is not a string or string-valued path-like object.
            UsageError: If a file cannot be read.
            DataError: If either file is damaged, is not a font, or is a collection.

        Example:
            >>> import umber as ub
            >>> from pathlib import Path
            >>> regular = Path("Example-Regular.ttf")
            >>> bold = Path("Example-Bold.ttf")
            >>> if regular.is_file() and bold.is_file():
            ...     font = ub.Font.from_file(regular, bold=bold)
            ...     theme = ub.Theme.PIGMENT.with_title_font(font)
        """
        with api_call():
            context = "Font.from_file()"
            _args.require_path(context, "regular", regular)
            if not isinstance(bold, Auto):
                _args.require_path(context, "bold", bold)
            regular_path = _args.path(context, "regular", regular)
            bold_path = AUTO if isinstance(bold, Auto) else _args.path(context, "bold", bold)
            regular_bytes = _read(regular_path, "regular")
            bold_bytes = None if isinstance(bold_path, Auto) else _read(bold_path, "bold")
            try:
                value = _umber.font_from_files(regular_bytes, bold_bytes, Path(regular_path).stem)
            except DataError as err:
                detail = details(err)
                if detail is None:
                    raise
                parameter, hint = detail
                path = bold_path if parameter == "bold" else regular_path
                raise new_error(
                    DataError,
                    context,
                    f"{parameter} is {path!r}, which is not a valid font; "
                    "it must be a TrueType or OpenType font.",
                    parameter=parameter,
                    hint=hint,
                ) from None
            name = f"from_file({regular_path!r}"
            if not isinstance(bold_path, Auto):
                name += f", bold={bold_path!r}"
            source = (regular_bytes, bold_bytes, Path(regular_path).stem)
            return _new(name + ")", (_FILE, value), source)

    @staticmethod
    def system(family: str) -> "Font":
        """Find an installed font family, matching its name case-insensitively.

        The regular face is closest to weight 400, with upright faces preferred. The bold face
        is upright and closest to weight 600 among weights at least 600. Selected bytes are
        kept in memory. A family without bold fails only when bold text is drawn.

        Args:
            family: An installed family name, not a font file path.

        Returns:
            An immutable font for a theme.

        Raises:
            TypeError: If ``family`` is not a string.
            UsageError: If the family is not installed or its selected files cannot be read.
                An unknown family reports the closest installed names.

        Example:
            >>> import umber as ub
            >>> try:
            ...     font = ub.Font.system("DejaVu Sans")
            ... except ub.UsageError:
            ...     font = ub.Font.INTER
            >>> isinstance(font, ub.Font)
            True
        """
        with api_call():
            family = _args.text("Font.system()", "family", family)
            value = _umber.font_system(family)
            name = value.family_name()
            return _new(f"system({name!r})", (_SYSTEM, value), name)

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

    def __reduce__(self) -> tuple[Callable[[_Source], "Font"], tuple[_Source]]:
        return _restore, (self._source,)

    def __setattr__(self, name: str, value: object) -> None:
        raise AttributeError("a Font is immutable")

    def __delattr__(self, name: str) -> None:
        raise AttributeError("a Font is immutable")


def _new(name: str, native: _Native, source: _Source) -> Font:
    """Build a font; the class has no public constructor."""
    font = object.__new__(Font)
    object.__setattr__(font, "_name", name)
    object.__setattr__(font, "_native", native)
    object.__setattr__(font, "_source", source)
    return font


_NATIVE: Final = attrgetter("_native")


def native(font: Font) -> _Native:
    """Return the native font argument of ``font``: ``(kind, payload)``.

    Args:
        font: The font.

    Returns:
        The kind (0 bundled, 1 file, 2 system) and a bundled index or parsed native font.

    Example:
        >>> import umber as ub
        >>> from umber._font import native
        >>> native(ub.Font.INTER)
        (0, 3)
    """
    return cast(_Native, _NATIVE(font))


_BUNDLED_FONTS: Final[list[Font]] = []
for _name, _index in _umber.bundled_fonts():
    _constant = _new(_name, (_BUNDLED, _index), _index)
    setattr(Font, _name, _constant)
    _BUNDLED_FONTS.append(_constant)


def _restore(source: _Source) -> Font:
    """Restore a value; file fonts use saved bytes, never the original paths."""
    if isinstance(source, int):
        return _BUNDLED_FONTS[source]
    if isinstance(source, str):
        return Font.system(source)
    regular, bold, name = source
    value = _umber.font_from_files(regular, bold, name)
    return _new(f"from_file({name!r})", (_FILE, value), source)


def _read(path: str, parameter: str) -> bytes:
    """Read bytes and attach the public path error to the argument that failed."""
    try:
        return Path(path).read_bytes()
    except (OSError, ValueError):
        raise new_error(
            UsageError,
            "Font.from_file()",
            f"{parameter} is {path!r}, which cannot be read; it must be a readable font file.",
            parameter=parameter,
            hint="check the path.",
        ) from None


FONT_HINT: Final = f"use {', '.join(map(repr, _BUNDLED_FONTS[:-1]))} or {_BUNDLED_FONTS[-1]!r}."
"""The hint of the errors for a value that is not a ``Font``: it lists the bundled fonts."""
