"""Tests of the native extension's value validators, enum tables and error mapping."""

import importlib
from collections.abc import Callable

import pytest

import umber as ub
from umber import _umber


def details(err: BaseException) -> tuple[object, object]:
    """The parameter and hint the native layer stored on an exception it raised."""
    attributes = vars(err)
    return attributes["_umber_parameter"], attributes["_umber_hint"]


def test_constants() -> None:
    assert _umber.MAX_BINS == 10_000
    assert isinstance(_umber.__version__, str)


def test_enum_members_lists_every_enum_with_values_in_declaration_order() -> None:
    assert _umber.enum_members() == {
        "Dash": [("SOLID", 0), ("DASHED", 1), ("DOTTED", 2)],
        "Marker": [("CIRCLE", 0), ("SQUARE", 1), ("TRIANGLE", 2), ("DIAMOND", 3), ("CROSS", 4)],
        "LegendStyle": [("HEADER", 0), ("DIRECT", 1), ("CHIPS", 2), ("NONE", 3)],
        "FillStyle": [("SOLID", 0), ("GRADIENT", 1)],
        "GridLines": [("HORIZONTAL", 0), ("VERTICAL", 1), ("BOTH", 2), ("NONE", 3)],
        "SvgText": [("OUTLINES", 0), ("EDITABLE", 1)],
        "Share": [("NONE", 0), ("ROW", 1), ("COL", 2), ("ALL", 3)],
        "Legend": [("MERGED", 0), ("PER_PANEL", 1)],
        "LegendPosition": [
            ("TOP_LEFT", 0),
            ("TOP_RIGHT", 1),
            ("RIGHT", 2),
            ("BOTTOM", 3),
            ("BEST", 4),
        ],
        "Align": [("LEFT", 0), ("CENTER", 1), ("RIGHT", 2)],
        "Step": [("POST", 0), ("PRE", 1), ("MID", 2)],
        "ColorbarPosition": [("RIGHT", 0), ("BOTTOM", 1), ("NONE", 2)],
        "Render": [("VECTOR", 0), ("RASTER", 1)],
    }


def test_bundled_fonts_lists_every_bundled_font_in_order() -> None:
    assert _umber.bundled_fonts() == [
        ("SOURCE_SERIF_4", 0),
        ("IBM_PLEX_SANS", 1),
        ("GEIST", 2),
        ("INTER", 3),
        ("JETBRAINS_MONO", 4),
    ]


def test_enum_members_keys_are_the_bound_enum_names() -> None:
    assert set(_umber.enum_members()) == {
        "Dash",
        "Marker",
        "LegendStyle",
        "FillStyle",
        "GridLines",
        "SvgText",
        "Share",
        "Legend",
        "LegendPosition",
        "Align",
        "Step",
        "ColorbarPosition",
        "Render",
    }


def test_named_colors_match_the_documented_hex_values() -> None:
    assert _umber.named_colors() == [
        ("BLACK", 0x1A1A1AFF),
        ("WHITE", 0xFFFFFFFF),
        ("GRAY", 0x8C8C8CFF),
        ("RED", 0xD64933FF),
        ("ORANGE", 0xE8853DFF),
        ("YELLOW", 0xE9B949FF),
        ("GREEN", 0x4E9A5EFF),
        ("TEAL", 0x2E8B8BFF),
        ("BLUE", 0x3366B8FF),
        ("PURPLE", 0x7D5BA6FF),
        ("PINK", 0xD46A92FF),
        ("BROWN", 0x7A5236FF),
    ]


@pytest.mark.parametrize(
    ("code", "packed"),
    [
        ("#3366B8", 0x3366B8FF),
        ("#3366b8", 0x3366B8FF),
        ("#3366B880", 0x3366B880),
        ("#00000000", 0x00000000),
    ],
)
def test_color_hex_packs_rgba(code: str, packed: int) -> None:
    assert _umber.color_hex(code) == packed


@pytest.mark.parametrize("code", ["#12", "3366B8", "#3366B", "#GG0000", "#3366B8801", ""])
def test_color_hex_rejects_malformed_codes(code: str) -> None:
    with pytest.raises(ub.StyleError) as caught:
        _umber.color_hex(code)
    assert type(caught.value) is ub.StyleError
    assert (
        str(caught.value) == f'Color.hex(): "{code}" is not a hex color; use #RRGGBB or #RRGGBBAA.'
    )
    assert details(caught.value) == ("code", None)


def test_color_rgb_packs_an_opaque_color() -> None:
    assert _umber.color_rgb(0x33, 0x66, 0xB8) == 0x3366B8FF
    assert _umber.color_rgb(0, 0, 0) == 0x000000FF
    assert _umber.color_rgb(255, 255, 255) == 0xFFFFFFFF


@pytest.mark.parametrize(
    ("channels", "name", "value"),
    [((-1, 0, 0), "r", -1), ((0, 300, 0), "g", 300), ((0, 0, 256), "b", 256)],
)
def test_color_rgb_rejects_channels_out_of_range(
    channels: tuple[int, int, int], name: str, value: int
) -> None:
    with pytest.raises(ub.StyleError) as caught:
        _umber.color_rgb(*channels)
    assert str(caught.value) == (
        f"Color.rgb(): {name} is {value}; each channel must be between 0 and 255."
    )
    assert details(caught.value) == (name, None)


def test_color_rgb_names_the_first_bad_channel() -> None:
    with pytest.raises(ub.StyleError) as caught:
        _umber.color_rgb(0, -5, 999)
    assert details(caught.value) == ("g", None)


@pytest.mark.parametrize(
    ("rgba", "a", "packed"),
    [
        (0x3366B8FF, 0.5, 0x3366B880),
        (0x3366B880, 1.0, 0x3366B8FF),
        (0xFFFFFFFF, 0.0, 0xFFFFFF00),
    ],
)
def test_color_with_alpha_replaces_the_alpha(rgba: int, a: float, packed: int) -> None:
    assert _umber.color_with_alpha(rgba, a) == packed


@pytest.mark.parametrize(("a", "shown"), [(1.5, "1.5"), (-0.1, "-0.1"), (float("nan"), "NaN")])
def test_color_with_alpha_rejects_alpha_out_of_range(a: float, shown: str) -> None:
    with pytest.raises(ub.StyleError) as caught:
        _umber.color_with_alpha(0x3366B8FF, a)
    assert (
        str(caught.value) == f"Color.alpha(): alpha is {shown}; alpha must be between 0.0 and 1.0."
    )
    assert details(caught.value) == ("a", None)


@pytest.mark.parametrize("rgba", [-1, 0x1_0000_0000])
def test_color_with_alpha_rejects_unpacked_values_as_an_internal_error(rgba: int) -> None:
    with pytest.raises(ValueError) as caught:
        _umber.color_with_alpha(rgba, 0.5)
    assert type(caught.value) is ValueError
    assert str(caught.value) == f"internal error: {rgba} is not a color packed as 0xRRGGBBAA"
    assert "_umber_parameter" not in vars(caught.value)


@pytest.mark.parametrize("a", [0.0, 0.25, 1.0])
def test_check_alpha_accepts_the_closed_unit_interval(a: float) -> None:
    _umber.check_alpha(a)  # no exception


@pytest.mark.parametrize(("a", "shown"), [(1.5, "1.5"), (-1.0, "-1"), (float("inf"), "inf")])
def test_check_alpha_rejects_values_outside_the_unit_interval(a: float, shown: str) -> None:
    with pytest.raises(ub.StyleError) as caught:
        _umber.check_alpha(a)
    assert (
        str(caught.value) == f"Color.alpha(): alpha is {shown}; alpha must be between 0.0 and 1.0."
    )
    assert details(caught.value) == ("a", None)


@pytest.mark.parametrize("index", [0, 1, 7, 2**63 - 1])
def test_check_palette_index_accepts_non_negative_indexes(index: int) -> None:
    _umber.check_palette_index(index)  # no exception


@pytest.mark.parametrize("index", [-1, -(2**63)])
def test_check_palette_index_rejects_negative_indexes(index: int) -> None:
    with pytest.raises(ub.StyleError) as caught:
        _umber.check_palette_index(index)
    assert str(caught.value) == (
        f"Color.palette(): index is {index}; a palette index must be 0 or greater."
    )
    assert details(caught.value) == ("index", "indexes start at 0 and wrap around the palette.")


@pytest.mark.parametrize("base", [2.0, 10.0, 1.000001, 1e300])
def test_check_log_base_accepts_finite_bases_above_one(base: float) -> None:
    _umber.check_log_base(base)  # no exception


@pytest.mark.parametrize(
    ("base", "shown"),
    [(1.0, "1"), (0.5, "0.5"), (-2.0, "-2"), (float("inf"), "inf"), (float("nan"), "NaN")],
)
def test_check_log_base_rejects_other_bases(base: float, shown: str) -> None:
    with pytest.raises(ub.StyleError) as caught:
        _umber.check_log_base(base)
    assert str(caught.value) == (
        f"Scale.log(): base is {shown}; a log base must be finite and greater than 1."
    )
    assert details(caught.value) == ("base", None)


@pytest.mark.parametrize(
    "call",
    [
        lambda: _umber.color_hex("#12"),
        lambda: _umber.color_rgb(0, 300, 0),
        lambda: _umber.color_with_alpha(0, 2.0),
        lambda: _umber.check_alpha(2.0),
        lambda: _umber.check_palette_index(-1),
        lambda: _umber.check_log_base(0.0),
    ],
)
def test_reported_errors_are_style_errors_and_value_errors(call: Callable[[], object]) -> None:
    with pytest.raises(ub.StyleError) as caught:
        call()
    assert isinstance(caught.value, ValueError)
    assert isinstance(caught.value, ub.UmberError)
    assert set(vars(caught.value)) == {"_umber_parameter", "_umber_hint"}


def test_errors_take_the_classes_of_a_reloaded_errors_module(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    from umber import _errors

    _umber.check_alpha(0.5)
    with pytest.raises(ub.StyleError):
        _umber.check_alpha(2.0)
    original = _errors.StyleError
    # Restores the original classes afterwards, so later tests see the classes `umber` exports.
    for name in ("UmberError", "DataError", "StyleError", "UsageError"):
        monkeypatch.setattr(_errors, name, getattr(_errors, name))
    importlib.reload(_errors)
    assert _errors.StyleError is not original

    with pytest.raises(_errors.StyleError) as caught:
        _umber.check_alpha(2.0)
    assert type(caught.value) is _errors.StyleError
    assert isinstance(caught.value, _errors.UmberError)
    assert not isinstance(caught.value, original)
    assert details(caught.value) == ("a", None)
