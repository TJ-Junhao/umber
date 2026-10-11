"""Type stub for the private native extension ``umber._umber``.

Errors that umber reports on purpose are ``DataError``, ``StyleError`` or ``UsageError`` with the
instance attributes ``_umber_parameter`` and ``_umber_hint``. A ``TypeError`` or ``ValueError``
whose message starts with "internal error:" means an argument the Python layer should never
pass; it carries neither attribute.

Float data arguments must be one-dimensional, aligned (start and strides multiples of 8 bytes),
C-contiguous or strided float64 buffers whose format is "d", "@d" or "=d"; an explicit byte order
such as "<d" is refused. ``np.require(values, dtype="=f8", requirements=["A"]).view(np.float64)``
produces such a buffer. Name arguments must be a ``list`` of ``str`` without lone surrogates.
"""

__all__ = [
    "MAX_BINS",
    "__version__",
    "check_alpha",
    "check_log_base",
    "check_palette_index",
    "color_hex",
    "color_rgb",
    "color_with_alpha",
    "enum_members",
    "named_colors",
]

__version__: str
MAX_BINS: int

# A color argument: (kind, payload, alpha). Kind 0 is Color.AUTO (payload unused, 0), 1 is
# Color.palette() (payload: the index), 2 is a fixed color (payload: 0xRRGGBBAA; alpha unused,
# since the opacity is part of the payload).
type _ColorArg = tuple[int, int, float]

def enum_members() -> dict[str, list[tuple[str, int]]]: ...

# Named colors as (name, 0xRRGGBBAA), in the order BLACK, WHITE, GRAY, RED, ORANGE, YELLOW,
# GREEN, TEAL, BLUE, PURPLE, PINK, BROWN.
def named_colors() -> list[tuple[str, int]]: ...

# Concrete colors are returned as 0xRRGGBBAA.
def color_hex(code: str) -> int: ...
def color_rgb(r: int, g: int, b: int) -> int: ...
def color_with_alpha(rgba: int, a: float) -> int: ...
def check_alpha(a: float) -> None: ...
def check_palette_index(index: int) -> None: ...

# A scale argument is (kind, base): kind 0 is linear, 1 is log, 2 is category; the base is
# used by kind 1 only.
def check_log_base(base: float) -> None: ...
