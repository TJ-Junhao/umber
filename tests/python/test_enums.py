import enum

import pytest

import umber as ub
from umber import _umber

ENUMS = [
    ub.Dash,
    ub.Marker,
    ub.Font,
    ub.LegendStyle,
    ub.FillStyle,
    ub.GridLines,
    ub.SvgText,
    ub.Step,
]


def _public_enums() -> list[type[enum.Enum]]:
    found = [getattr(ub, name) for name in ub.__all__]
    return [obj for obj in found if isinstance(obj, type) and issubclass(obj, enum.Enum)]


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
    assert ub.Font.JETBRAINS_MONO.name == "JETBRAINS_MONO"
    assert {m.value for m in ub.SvgText} == {0, 1}


def test_fill_style_members() -> None:
    assert [(m.name, m.value) for m in ub.FillStyle] == [("SOLID", 0), ("GRADIENT", 1)]
    assert ub.FillStyle(1) is ub.FillStyle.GRADIENT


def test_step_members() -> None:
    assert [(m.name, m.value) for m in ub.Step] == [("POST", 0), ("PRE", 1), ("MID", 2)]
    assert ub.Step(2) is ub.Step.MID
