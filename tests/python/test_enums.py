# pyright: reportPrivateUsage=false
import enum
import re
from collections.abc import Iterable
from typing import cast

import pytest

import umber as ub
from umber import _umber

ENUMS = [
    ub.Dash,
    ub.Marker,
    ub.LegendStyle,
    ub.FillStyle,
    ub.GridLines,
    ub.SvgText,
    ub.Share,
    ub.Legend,
    ub.LegendPosition,
    ub.Align,
    ub.Step,
    ub.Render,
    ub.Hist,
    ub.Stat,
    ub.Contour,
]


def _public_enums() -> list[type[enum.Enum]]:
    found = [getattr(ub, name) for name in ub.__all__]
    return [
        obj
        for obj in found
        if isinstance(obj, type) and issubclass(obj, enum.Enum) and obj is not ub.Auto
    ]


def test_auto_has_one_member_and_no_native_table() -> None:
    assert list(ub.Auto) == [ub.AUTO]
    assert "Auto" not in _umber.enum_members()
    assert repr(ub.AUTO) == "AUTO"


def test_public_enums_are_exactly_the_native_tables() -> None:
    assert {e.__name__ for e in _public_enums()} == set(_umber.enum_members())


@pytest.mark.parametrize("enum_type", _public_enums(), ids=lambda e: e.__name__)
def test_members_and_values_match_the_native_table(enum_type: type[enum.Enum]) -> None:
    table = _umber.enum_members()[enum_type.__name__]
    assert [(m.name, m.value) for m in enum_type] == table


@pytest.mark.parametrize("enum_type", ENUMS, ids=lambda e: e.__name__)
def test_enum_is_a_real_enum_with_integer_values(enum_type: type[enum.Enum]) -> None:
    assert issubclass(enum_type, enum.Enum)
    assert all(type(m.value) is int for m in enum_type)
    assert enum_type.__module__ == "umber"
    assert enum_type.__members__ == {m.name: m for m in enum_type}


def test_members_are_unique() -> None:
    for enum_type in ENUMS:
        values = [m.value for m in enum_type]
        assert len(values) == len(set(values))


def test_members_are_identified_by_name_and_value() -> None:
    assert ub.Dash["DOTTED"] is ub.Dash.DOTTED
    assert ub.Dash(1) is ub.Dash.DASHED
    assert ub.Marker.CROSS.value == 4
    assert {m.value for m in ub.SvgText} == {0, 1}


def test_fill_style_members() -> None:
    assert [(m.name, m.value) for m in ub.FillStyle] == [("SOLID", 0), ("GRADIENT", 1)]
    assert ub.FillStyle(1) is ub.FillStyle.GRADIENT


def test_step_members() -> None:
    assert [(m.name, m.value) for m in ub.Step] == [("POST", 0), ("PRE", 1), ("MID", 2)]
    assert ub.Step(2) is ub.Step.MID


BUNDLED_FAMILIES = {
    "SOURCE_SERIF_4": "'Source Serif 4', serif",
    "IBM_PLEX_SANS": "'IBM Plex Sans', sans-serif",
    "GEIST": "'Geist', sans-serif",
    "INTER": "'Inter', sans-serif",
    "JETBRAINS_MONO": "'JetBrains Mono', monospace",
}


def test_font_is_not_an_enum() -> None:
    assert not isinstance(ub.Font.INTER, enum.Enum)
    assert not issubclass(ub.Font, enum.Enum)
    assert "Font" not in _umber.enum_members()
    with pytest.raises(TypeError):
        iter(cast(Iterable[object], ub.Font))


def test_bundled_constants_round_trip() -> None:
    table = _umber.bundled_fonts()
    assert [name for name, _ in table] == list(BUNDLED_FAMILIES)
    for name, index in table:
        font = getattr(ub.Font, name)
        assert isinstance(font, ub.Font)
        assert font._native == (0, index)
        assert repr(font) == f"Font.{name}"
        figure = ub.line([0.0, 1.0], [0.0, 1.0], theme=ub.Theme.PIGMENT.with_title_font(font))
        svg = figure.title("Revenue").figure.to_svg(text=ub.SvgText.EDITABLE)
        title = re.search(r'<text [^>]*font-family="([^"]*)"[^>]*>Revenue</text>', svg)
        assert title is not None
        assert title.group(1) == BUNDLED_FAMILIES[name]
