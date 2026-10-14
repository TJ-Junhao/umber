# pyright: reportPrivateUsage=false
import copy
import math
import pickle
import traceback
import typing
from collections.abc import Callable
from typing import cast

import numpy as np
import pytest

import umber as ub
from umber import _umber
from umber._color import native
from umber._errors import details

NAMES = [
    "BLACK",
    "WHITE",
    "GRAY",
    "RED",
    "ORANGE",
    "YELLOW",
    "GREEN",
    "TEAL",
    "BLUE",
    "PURPLE",
    "PINK",
    "BROWN",
]
# The documented constants.
HEX = {
    "BLACK": "#1A1A1A",
    "WHITE": "#FFFFFF",
    "GRAY": "#8C8C8C",
    "RED": "#D64933",
    "ORANGE": "#E8853D",
    "YELLOW": "#E9B949",
    "GREEN": "#4E9A5E",
    "TEAL": "#2E8B8B",
    "BLUE": "#3366B8",
    "PURPLE": "#7D5BA6",
    "PINK": "#D46A92",
    "BROWN": "#7A5236",
}


def _notes(err: BaseException) -> list[str]:
    return list(getattr(err, "__notes__", []))


def _marked(line: str, argument: str) -> str:
    """The highlight note for ``argument`` in the source ``line``."""
    return f"  {line}\n{' ' * (2 + line.index(argument))}{'^' * len(argument)}"


def test_named_colors_match_the_documented_hex_table() -> None:
    assert [name for name, _ in _umber.named_colors()] == NAMES
    for name in NAMES:
        assert getattr(ub.Color, name) == ub.Color.hex(HEX[name])
        assert native(ub.Color.hex(HEX[name])) == (2, int(HEX[name][1:] + "FF", 16), 1.0)


def test_named_colors_are_not_the_auto_color() -> None:
    assert ub.Color.AUTO != ub.Color.BLACK
    assert native(ub.Color.AUTO) == (0, 0, 1.0)


def test_hex_accepts_six_and_eight_digits_in_either_case() -> None:
    assert ub.Color.hex("#3366b8") == ub.Color.BLUE
    assert ub.Color.hex("#3366B8FF") == ub.Color.BLUE
    assert native(ub.Color.hex("#3366B880")) == (2, 0x3366B880, 1.0)


def test_rgb_builds_an_opaque_color() -> None:
    assert ub.Color.rgb(0x33, 0x66, 0xB8) == ub.Color.BLUE
    assert native(ub.Color.rgb(0, 0, 0)) == (2, 0x000000FF, 1.0)


def test_palette_color_wraps_at_resolution_time() -> None:
    assert native(ub.Color.palette(2)) == (1, 2, 1.0)
    assert native(ub.Color.palette(2**63 - 1)) == (1, 2**63 - 1, 1.0)


def test_alpha_replaces_the_alpha_of_a_fixed_color() -> None:
    half = ub.Color.BLUE.alpha(0.5)
    assert native(half) == (2, 0x3366B880, 1.0)
    assert half.alpha(1.0) == ub.Color.BLUE
    assert native(half.alpha(0.0)) == (2, 0x3366B800, 1.0)


def test_alpha_keeps_the_kind_of_a_theme_color() -> None:
    assert native(ub.Color.AUTO.alpha(0.25)) == (0, 0, 0.25)
    assert native(ub.Color.palette(2).alpha(0.5)) == (1, 2, 0.5)
    assert ub.Color.palette(2).alpha(0.5).alpha(1.0) == ub.Color.palette(2)


def test_alpha_returns_a_new_color() -> None:
    original = ub.Color.BLUE
    original.alpha(0.5)
    assert original == ub.Color.BLUE


def test_equality_and_hash() -> None:
    assert ub.Color.hex("#3366B8") == ub.Color.BLUE
    assert hash(ub.Color.hex("#3366B8")) == hash(ub.Color.BLUE)
    assert ub.Color.palette(1) != ub.Color.palette(2)
    assert ub.Color.palette(1) != ub.Color.AUTO
    assert ub.Color.palette(1) != ub.Color.palette(1).alpha(0.5)
    assert ub.Color.BLUE != "blue"
    assert len({ub.Color.BLUE, ub.Color.hex("#3366B8"), ub.Color.RED}) == 2


@pytest.mark.parametrize(
    ("color", "text"),
    [
        (ub.Color.AUTO, "Color.AUTO"),
        (ub.Color.BLUE, "Color.BLUE"),
        (ub.Color.hex("#112233"), 'Color.hex("#112233")'),
        (ub.Color.hex("#3366B880"), 'Color.hex("#3366B880")'),
        (ub.Color.BLUE.alpha(0.5), 'Color.hex("#3366B880")'),
        (ub.Color.rgb(1, 2, 3), 'Color.hex("#010203")'),
        (ub.Color.palette(2), "Color.palette(2)"),
        (ub.Color.palette(2).alpha(0.5), "Color.palette(2).alpha(0.5)"),
        (ub.Color.AUTO.alpha(0.25), "Color.AUTO.alpha(0.25)"),
    ],
)
def test_repr(color: ub.Color, text: str) -> None:
    assert repr(color) == text


def test_color_is_immutable_and_has_no_public_constructor() -> None:
    color = ub.Color.BLUE
    with pytest.raises(AttributeError):
        color._kind = 0
    with pytest.raises(AttributeError):
        del color._kind
    with pytest.raises(AttributeError):
        color.extra = 1
    with pytest.raises(TypeError) as caught:
        ub.Color()
    assert str(caught.value) == (
        "Color(): Color has no public constructor; use a named color or a Color constructor."
    )
    assert details(caught.value) == (
        None,
        'use Color.hex("#3366B8"), Color.rgb(), Color.palette() or a named color.',
    )


def test_hex_error_highlights_the_argument() -> None:
    with pytest.raises(ub.StyleError) as caught:
        ub.Color.hex("#12")
    assert str(caught.value) == 'Color.hex(): "#12" is not a hex color; use #RRGGBB or #RRGGBBAA.'
    assert details(caught.value) == ("code", None)
    assert _notes(caught.value) == [_marked('ub.Color.hex("#12")', '"#12"')]


def test_rgb_error_highlights_the_positional_argument() -> None:
    with pytest.raises(ub.StyleError) as caught:
        ub.Color.rgb(0, 300, 0)
    assert str(caught.value) == ("Color.rgb(): g is 300; each channel must be between 0 and 255.")
    assert details(caught.value) == ("g", None)
    assert _notes(caught.value) == [_marked("ub.Color.rgb(0, 300, 0)", "300")]


def test_rgb_error_highlights_a_keyword_argument() -> None:
    with pytest.raises(ub.StyleError) as caught:
        ub.Color.rgb(0, 0, b=256)
    assert details(caught.value) == ("b", None)
    assert _notes(caught.value) == [_marked("ub.Color.rgb(0, 0, b=256)", "b=256")]


def test_palette_rejects_a_negative_index() -> None:
    with pytest.raises(ub.StyleError) as caught:
        ub.Color.palette(-1)
    assert str(caught.value) == (
        "Color.palette(): index is -1; a palette index must be 0 or greater."
    )
    assert details(caught.value) == ("index", "indexes start at 0 and wrap around the palette.")
    assert _notes(caught.value) == [
        _marked("ub.Color.palette(-1)", "-1"),
        "hint: indexes start at 0 and wrap around the palette.",
    ]


def test_palette_rejects_a_hugely_negative_index() -> None:
    with pytest.raises(ub.StyleError):
        ub.Color.palette(-(10**30))


@pytest.mark.parametrize("a", [1.5, -0.1, math.inf, math.nan])
def test_alpha_rejects_values_outside_the_unit_interval(a: float) -> None:
    for color in (ub.Color.BLUE, ub.Color.AUTO, ub.Color.palette(1)):
        with pytest.raises(ub.StyleError) as caught:
            color.alpha(a)
        assert str(caught.value).startswith("Color.alpha(): alpha is ")
        assert str(caught.value).endswith("alpha must be between 0.0 and 1.0.")
        assert details(caught.value) == ("a", None)


def test_alpha_error_highlights_the_argument() -> None:
    with pytest.raises(ub.StyleError) as caught:
        ub.Color.BLUE.alpha(1.5)
    assert str(caught.value) == "Color.alpha(): alpha is 1.5; alpha must be between 0.0 and 1.0."
    assert _notes(caught.value) == [_marked("ub.Color.BLUE.alpha(1.5)", "1.5")]


@pytest.mark.parametrize("a", [10**400, -(10**400)])
def test_alpha_rejects_an_int_beyond_the_float_range(a: int) -> None:
    with pytest.raises(ub.StyleError) as caught:
        ub.Color.BLUE.alpha(a)
    assert details(caught.value) == ("a", None)


def test_hex_rejects_a_non_string() -> None:
    with pytest.raises(TypeError) as caught:
        ub.Color.hex(cast(str, 123))
    assert str(caught.value) == "Color.hex(): code is int; code must be a str."
    assert details(caught.value) == ("code", 'pass a string such as "#3366B8".')
    assert _notes(caught.value) == [
        _marked("ub.Color.hex(cast(str, 123))", "cast(str, 123)"),
        'hint: pass a string such as "#3366B8".',
    ]


def test_alpha_rejects_a_string() -> None:
    with pytest.raises(TypeError) as caught:
        ub.Color.BLUE.alpha(cast(float, "0.5"))
    assert str(caught.value) == "Color.alpha(): a is str; a must be a float."
    assert details(caught.value) == ("a", "pass a number from 0.0 to 1.0.")
    assert _notes(caught.value)[0].startswith('  ub.Color.BLUE.alpha(cast(float, "0.5"))\n')
    assert _notes(caught.value)[1] == "hint: pass a number from 0.0 to 1.0."


@pytest.mark.parametrize("value", [1.5, "1", None, True])
def test_rgb_and_palette_reject_non_integers(value: object) -> None:
    name = "None" if value is None else type(value).__name__
    number = cast(int, value)
    calls: list[tuple[Callable[[], object], str]] = [
        (lambda: ub.Color.rgb(number, 0, 0), "r"),
        (lambda: ub.Color.rgb(0, number, 0), "g"),
        (lambda: ub.Color.rgb(0, 0, number), "b"),
        (lambda: ub.Color.palette(number), "index"),
    ]
    for call, parameter in calls:
        with pytest.raises(TypeError) as caught:
            call()
        context = "Color.palette()" if parameter == "index" else "Color.rgb()"
        assert str(caught.value) == (
            f"{context}: {parameter} is {name}; {parameter} must be an int."
        )
        assert details(caught.value) == (parameter, "pass a whole number such as 3.")


def test_type_checks_run_in_parameter_order_before_the_native_check() -> None:
    with pytest.raises(TypeError) as caught:
        ub.Color.rgb(300, cast(int, "1"), 0)
    found = details(caught.value)
    assert found is not None
    assert found[0] == "g"


def test_numpy_numbers_are_accepted() -> None:
    assert (
        ub.Color.rgb(cast(int, np.int64(51)), cast(int, np.uint8(102)), cast(int, np.int32(184)))
        == ub.Color.BLUE
    )
    assert ub.Color.palette(cast(int, np.int64(3))) == ub.Color.palette(3)
    assert ub.Color.BLUE.alpha(cast(float, np.float32(0.5))) == ub.Color.BLUE.alpha(0.5)
    assert ub.Color.BLUE.alpha(cast(float, np.int64(1))) == ub.Color.BLUE


@pytest.mark.parametrize("index", [2**63, 10**30, 2**64 - 1])
def test_palette_rejects_an_index_above_the_largest(index: int) -> None:
    with pytest.raises(ub.StyleError) as caught:
        ub.Color.palette(index)
    assert str(caught.value) == (
        f"Color.palette(): index is {index}; a palette index must be at most 9223372036854775807."
    )
    assert details(caught.value) == (
        "index",
        "indexes wrap around the palette, so a smaller one picks the same color.",
    )
    assert _notes(caught.value)[0] == _marked("ub.Color.palette(index)", "index")


@pytest.mark.parametrize(
    ("channels", "parameter", "shown"),
    [
        ((2**63, 0, 0), "r", 2**63),
        ((0, 2**64, 0), "g", 2**64),
        ((0, 0, -(2**63) - 1), "b", -(2**63) - 1),
        ((0, 0, -(10**30)), "b", -(10**30)),
        ((2**63, 2**64, 0), "r", 2**63),
    ],
)
def test_rgb_channels_beyond_the_native_range_are_style_errors(
    channels: tuple[int, int, int], parameter: str, shown: int
) -> None:
    with pytest.raises(ub.StyleError) as caught:
        ub.Color.rgb(*channels)
    assert details(caught.value) == (parameter, None)
    assert str(caught.value).startswith(f"Color.rgb(): {parameter} is ")
    assert str(caught.value).endswith("each channel must be between 0 and 255.")


def test_rgb_accepts_the_largest_numpy_integer_as_a_range_error() -> None:
    with pytest.raises(ub.StyleError) as caught:
        ub.Color.rgb(0, cast(int, np.uint64(2**64 - 1)), 0)
    assert details(caught.value) == ("g", None)


def test_rgb_overflow_highlights_the_argument() -> None:
    with pytest.raises(ub.StyleError) as caught:
        ub.Color.rgb(0, 2**63, 0)
    assert _notes(caught.value) == [_marked("ub.Color.rgb(0, 2**63, 0)", "2**63")]


def test_direct_construction_is_reported_with_a_hint_and_no_package_frame() -> None:
    with pytest.raises(TypeError) as caught:
        ub.Color()
    assert _notes(caught.value) == [
        'hint: use Color.hex("#3366B8"), Color.rgb(), Color.palette() or a named color.'
    ]
    frames = [entry.name for entry in traceback.extract_tb(caught.value.__traceback__)]
    assert frames == ["test_direct_construction_is_reported_with_a_hint_and_no_package_frame"]


COLORS = [
    ub.Color.AUTO,
    ub.Color.BLUE,
    ub.Color.hex("#11223344"),
    ub.Color.palette(3),
    ub.Color.palette(3).alpha(0.5),
    ub.Color.AUTO.alpha(0.25),
]


@pytest.mark.parametrize("color", COLORS, ids=repr)
def test_copy_and_deepcopy_return_the_same_value(color: ub.Color) -> None:
    assert copy.copy(color) is color
    assert copy.deepcopy(color) is color


@pytest.mark.parametrize("color", COLORS, ids=repr)
def test_pickle_round_trip_gives_an_equal_color(color: ub.Color) -> None:
    for protocol in range(pickle.HIGHEST_PROTOCOL + 1):
        restored = pickle.loads(pickle.dumps(color, protocol))
        assert isinstance(restored, ub.Color)
        assert restored == color
        assert repr(restored) == repr(color)


def test_class_annotations_resolve_with_the_package_as_module() -> None:
    hints = typing.get_type_hints(ub.Color)
    assert hints["AUTO"] == typing.ClassVar[ub.Color]
    assert typing.get_type_hints(ub.Color.hex)["return"] is ub.Color
    assert typing.get_type_hints(ub.Color.alpha)["return"] is ub.Color
