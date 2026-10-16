# pyright: reportPrivateUsage=false
import copy
import pickle
import traceback
import typing
from collections.abc import Callable, Sequence
from pathlib import Path
from typing import cast

import numpy as np
import pytest

import umber as ub
from umber import _umber
from umber._errors import details
from umber._theme import native

BASE = ub.Theme.MIDNIGHT
COLOR_HINT = 'use Color.hex("#3366B8"), Color.rgb() or a named color such as Color.BLUE.'
CONCRETE_HINT = "use Color.hex(), Color.rgb() or a named color."

COLOR_SETTERS = [
    "with_background",
    "with_text_color",
    "with_muted_color",
    "with_grid_color",
    "with_baseline_color",
]
# (setter, enum class name, two members, a member of another enum)
ENUM_SETTERS: list[tuple[str, str, tuple[str, str], str]] = [
    ("with_grid_lines", "GridLines", ("HORIZONTAL", "BOTH"), "Dash.DASHED"),
    ("with_grid_dash", "Dash", ("SOLID", "DOTTED"), "GridLines.BOTH"),
    ("with_title_font", "Font", ("SOURCE_SERIF_4", "INTER"), "Dash.DASHED"),
    ("with_title_align", "Align", ("LEFT", "CENTER"), "LegendPosition.RIGHT"),
    ("with_body_font", "Font", ("IBM_PLEX_SANS", "GEIST"), "Dash.DASHED"),
    ("with_legend_style", "LegendStyle", ("HEADER", "CHIPS"), "Dash.DASHED"),
    ("with_legend_position", "LegendPosition", ("TOP_LEFT", "RIGHT"), "Align.RIGHT"),
    ("with_fill_style", "FillStyle", ("SOLID", "GRADIENT"), "Dash.DASHED"),
]
# (setter, parameter name in the native message, two valid values, an invalid value, requirement)
GREATER_THAN_0 = "a finite number greater than 0"
AT_LEAST_0 = "a finite number of at least 0"
NUMERIC_SETTERS: list[tuple[str, tuple[float, float], str, str]] = [
    ("font_scale", (0.5, 2.0), "-1.0", f"{GREATER_THAN_0} and at most 10"),
    ("line_width", (0.5, 3.0), "-1.0", GREATER_THAN_0),
    ("marker_size", (2.0, 8.0), "0.0", GREATER_THAN_0),
    ("marker_alpha", (0.0, 1.0), "1.5", "a finite number from 0.0 to 1.0"),
    ("baseline_width", (0.0, 2.0), "-0.5", AT_LEAST_0),
    ("bar_corner_radius", (0.0, 4.0), "-1.0", AT_LEAST_0),
    ("hist_gap", (0.0, 3.0), "-1.0", AT_LEAST_0),
]


def _notes(err: BaseException) -> list[str]:
    return list(getattr(err, "__notes__", []))


def _marked(line: str, argument: str) -> str:
    """The highlight note for ``argument`` in the source ``line``."""
    return f"  {line}\n{' ' * (2 + line.index(argument))}{'^' * len(argument)}"


def _raised(tmp_path: Path, line: str) -> BaseException:
    """Run ``line`` as user code in a file, so the error is reported at its call site."""
    # A file per line: ``linecache`` would serve the first content for a reused name.
    path = tmp_path / f"user_code_{abs(hash(line))}.py"
    path.write_text(f"import umber as ub\n{line}\n")
    try:
        exec(compile(path.read_text(), str(path), "exec"), {})
    except Exception as err:
        return err
    raise AssertionError(f"{line} raised nothing")


def _type_name(value: object) -> str:
    return "None" if value is None else type(value).__name__


def _setter(theme: ub.Theme, name: str) -> Callable[[object], ub.Theme]:
    return cast(Callable[[object], ub.Theme], getattr(theme, name))


def test_presets_wrap_the_native_themes() -> None:
    assert native(ub.Theme.PIGMENT) == _umber.Theme.pigment()
    assert native(ub.Theme.GRAPHITE) == _umber.Theme.graphite()
    assert native(ub.Theme.MIDNIGHT) == _umber.Theme.midnight()
    assert ub.Theme.PIGMENT != ub.Theme.GRAPHITE != ub.Theme.MIDNIGHT


def test_every_native_setter_is_bound() -> None:
    native_names = {name for name in vars(_umber.Theme) if name.startswith("with_")}
    bound = {name for name in vars(ub.Theme) if name.startswith("with_")}
    assert bound == native_names
    expected = {"with_palette", *COLOR_SETTERS, *(f"with_{n}" for n, *_ in NUMERIC_SETTERS)}
    expected |= {"with_cmap", "with_diverging_cmap", "with_colorbar_position"}
    expected |= {name for name, *_ in ENUM_SETTERS}
    assert bound == expected


def test_construction_is_refused_with_a_hint() -> None:
    with pytest.raises(TypeError) as caught:
        ub.Theme()
    assert (
        str(caught.value) == "Theme(): Theme has no public constructor; start from a preset theme."
    )
    assert details(caught.value) == (
        None,
        "start from Theme.PIGMENT, Theme.GRAPHITE or Theme.MIDNIGHT.",
    )
    assert _notes(caught.value) == [
        "hint: start from Theme.PIGMENT, Theme.GRAPHITE or Theme.MIDNIGHT."
    ]
    frames = [entry.name for entry in traceback.extract_tb(caught.value.__traceback__)]
    assert frames == ["test_construction_is_refused_with_a_hint"]


def test_equality_and_hashing() -> None:
    assert ub.Theme.PIGMENT == ub.Theme.PIGMENT
    assert ub.Theme.PIGMENT.with_line_width(3.0) == ub.Theme.PIGMENT.with_line_width(3.0)
    assert ub.Theme.PIGMENT != ub.Theme.PIGMENT.with_line_width(3.0)
    assert ub.Theme.PIGMENT != "pigment"
    assert ub.Theme.__hash__ is None
    with pytest.raises(TypeError):
        hash(ub.Theme.PIGMENT)


def test_themes_are_immutable() -> None:
    theme = ub.Theme.PIGMENT
    with pytest.raises(AttributeError):
        theme._native = _umber.Theme.midnight()
    with pytest.raises(AttributeError):
        theme.extra = 1
    with pytest.raises(AttributeError):
        del theme._native
    assert not hasattr(theme, "__dict__")
    assert native(theme) == _umber.Theme.pigment()


def test_copy_and_deepcopy_return_the_same_theme() -> None:
    theme = ub.Theme.GRAPHITE.with_line_width(3.0)
    assert copy.copy(theme) is theme
    assert copy.deepcopy(theme) is theme


@pytest.mark.parametrize("protocol", range(pickle.HIGHEST_PROTOCOL + 1))
def test_pickling_is_refused(protocol: int) -> None:
    with pytest.raises(TypeError) as caught:
        pickle.dumps(ub.Theme.PIGMENT, protocol)
    assert str(caught.value) == "Theme: a Theme cannot be pickled."
    assert details(caught.value) == (
        None,
        "rebuild it from a preset: Theme.PIGMENT.with_line_width(3.0).",
    )
    assert _notes(caught.value) == [
        "hint: rebuild it from a preset: Theme.PIGMENT.with_line_width(3.0)."
    ]
    frames = [entry.name for entry in traceback.extract_tb(caught.value.__traceback__)]
    assert frames == ["test_pickling_is_refused"]


@pytest.mark.parametrize("name", ["PIGMENT", "GRAPHITE", "MIDNIGHT"])
def test_repr_of_a_preset_names_it(name: str) -> None:
    assert repr(getattr(ub.Theme, name)) == f"Theme.{name}"


def test_repr_of_a_custom_theme() -> None:
    assert repr(ub.Theme.PIGMENT.with_line_width(3.0)) == "<umber.Theme (customized)>"


def test_class_annotations_resolve_with_the_package_as_module() -> None:
    hints = typing.get_type_hints(ub.Theme)
    assert hints["PIGMENT"] == typing.ClassVar[ub.Theme]
    assert typing.get_type_hints(ub.Theme.with_line_width)["return"] is ub.Theme
    assert typing.get_type_hints(ub.Theme.with_palette)["colors"] == Sequence[ub.Color]


# Colors


@pytest.mark.parametrize("name", COLOR_SETTERS)
def test_color_setters_return_a_new_theme(name: str) -> None:
    one = _setter(BASE, name)(ub.Color.RED)
    assert isinstance(one, ub.Theme)
    assert one == _setter(BASE, name)(ub.Color.hex("#D64933"))
    assert one != _setter(BASE, name)(ub.Color.BLUE)
    assert one != BASE
    assert BASE == ub.Theme.MIDNIGHT


@pytest.mark.parametrize("name", COLOR_SETTERS)
def test_color_setters_apply_the_color_natively(name: str) -> None:
    expected = getattr(native(BASE), name)(native_color(ub.Color.RED))
    assert native(_setter(BASE, name)(ub.Color.RED)) == expected


def native_color(color: ub.Color) -> tuple[int, int, float]:
    from umber._color import native as color_native

    return color_native(color)


@pytest.mark.parametrize("name", COLOR_SETTERS)
@pytest.mark.parametrize(
    ("color", "shown"),
    [
        (ub.Color.AUTO, "Color.AUTO"),
        (ub.Color.palette(3), "Color.palette(3)"),
        (ub.Color.palette(3).alpha(0.5), "Color.palette(3)"),
    ],
)
def test_color_setters_refuse_colors_that_are_not_concrete(
    name: str, color: ub.Color, shown: str
) -> None:
    with pytest.raises(ub.StyleError) as caught:
        _setter(BASE, name)(color)
    assert str(caught.value) == (
        f"Theme.{name}(): value is {shown}; theme colors must be concrete colors."
    )
    assert details(caught.value) == ("value", CONCRETE_HINT)
    assert BASE == ub.Theme.MIDNIGHT


@pytest.mark.parametrize("name", COLOR_SETTERS)
def test_color_setters_reject_a_non_color(name: str) -> None:
    with pytest.raises(TypeError) as caught:
        _setter(BASE, name)(cast(ub.Color, "#3366B8"))
    assert str(caught.value) == f"Theme.{name}(): value is str; value must be of type Color."
    assert details(caught.value) == ("value", COLOR_HINT)


@pytest.mark.parametrize("name", COLOR_SETTERS)
def test_color_setter_errors_highlight_the_value(name: str, tmp_path: Path) -> None:
    line = f"ub.Theme.PIGMENT.{name}(ub.Color.AUTO)"
    err = _raised(tmp_path, line)
    assert isinstance(err, ub.StyleError)
    assert _notes(err) == [_marked(line, "ub.Color.AUTO"), f"hint: {CONCRETE_HINT}"]
    line = f"ub.Theme.PIGMENT.{name}(value='red')"
    err = _raised(tmp_path, line)
    assert isinstance(err, TypeError)
    assert _notes(err) == [_marked(line, "value='red'"), f"hint: {COLOR_HINT}"]


# Palette


def test_with_palette_sets_the_palette() -> None:
    one = BASE.with_palette([ub.Color.RED, ub.Color.BLUE])
    assert one == BASE.with_palette((ub.Color.RED, ub.Color.BLUE))
    assert one != BASE.with_palette([ub.Color.BLUE, ub.Color.RED])
    assert one != BASE
    assert native(one) == native(BASE).with_palette([(2, 0xD64933FF, 1.0), (2, 0x3366B8FF, 1.0)])
    assert BASE == ub.Theme.MIDNIGHT


def test_with_palette_accepts_one_to_sixteen_colors() -> None:
    assert BASE.with_palette([ub.Color.RED] * 16) != BASE.with_palette([ub.Color.RED])


@pytest.mark.parametrize("count", [0, 17])
def test_with_palette_rejects_other_counts(count: int) -> None:
    with pytest.raises(ub.StyleError) as caught:
        BASE.with_palette([ub.Color.RED] * count)
    assert str(caught.value) == (
        f"Theme.with_palette(): palette has {count} colors; a palette needs 1 to 16 colors."
    )
    assert details(caught.value) == ("colors", None)


@pytest.mark.parametrize(
    ("colors", "message"),
    [
        ([ub.Color.RED, ub.Color.BLUE, ub.Color.AUTO], "colors[2] is Color.AUTO"),
        ([ub.Color.palette(4), ub.Color.AUTO], "colors[0] is Color.palette(4)"),
    ],
)
def test_with_palette_names_the_color_that_is_not_concrete(
    colors: list[ub.Color], message: str
) -> None:
    with pytest.raises(ub.StyleError) as caught:
        BASE.with_palette(colors)
    assert str(caught.value) == (
        f"Theme.with_palette(): {message}; theme colors must be concrete colors."
    )
    assert details(caught.value) == ("colors", CONCRETE_HINT)


def test_with_palette_error_highlights_the_colors() -> None:
    colors = [ub.Color.RED, ub.Color.AUTO]
    with pytest.raises(ub.StyleError) as caught:
        ub.Theme.PIGMENT.with_palette(colors)
    assert _notes(caught.value) == [
        _marked("ub.Theme.PIGMENT.with_palette(colors)", "colors"),
        f"hint: {CONCRETE_HINT}",
    ]


@pytest.mark.parametrize("colors", ["red", b"red", 5, None, {ub.Color.RED}, np.array([1, 2])])
def test_with_palette_rejects_a_non_sequence(colors: object) -> None:
    name = _type_name(colors)
    with pytest.raises(TypeError) as caught:
        BASE.with_palette(cast(Sequence[ub.Color], colors))
    assert str(caught.value) == (
        f"Theme.with_palette(): colors is {name}; colors must be a sequence of Color."
    )
    assert details(caught.value) == (
        "colors",
        "pass a list of colors, e.g. [Color.RED, Color.BLUE].",
    )


def test_with_palette_names_the_element_that_is_not_a_color() -> None:
    with pytest.raises(TypeError) as caught:
        BASE.with_palette(cast(list[ub.Color], [ub.Color.RED, "blue", 3]))
    assert str(caught.value) == (
        "Theme.with_palette(): colors[1] is str; colors[1] must be of type Color."
    )
    assert details(caught.value) == ("colors", COLOR_HINT)


def test_with_palette_type_errors_come_before_native_errors() -> None:
    with pytest.raises(TypeError) as caught:
        BASE.with_palette(cast(list[ub.Color], [ub.Color.AUTO, 3]))
    assert "colors[1] is int" in str(caught.value)


def test_with_palette_type_error_highlights_the_colors(tmp_path: Path) -> None:
    line = "ub.Theme.PIGMENT.with_palette([ub.Color.RED, 7])"
    err = _raised(tmp_path, line)
    assert isinstance(err, TypeError)
    assert _notes(err) == [
        _marked(line, "[ub.Color.RED, 7]"),
        'hint: use Color.hex("#3366B8"), Color.rgb() or a named color such as Color.BLUE.',
    ]


def test_with_palette_concrete_error_highlights_the_colors_in_a_literal(tmp_path: Path) -> None:
    line = "ub.Theme.PIGMENT.with_palette([ub.Color.RED, ub.Color.AUTO])"
    err = _raised(tmp_path, line)
    assert isinstance(err, ub.StyleError)
    assert str(err).startswith("Theme.with_palette(): colors[1] is Color.AUTO;")
    assert _notes(err)[0] == _marked(line, "[ub.Color.RED, ub.Color.AUTO]")


# Enums


@pytest.mark.parametrize(("name", "cls", "members", "other"), ENUM_SETTERS)
def test_enum_setters_return_a_new_theme(
    name: str, cls: str, members: tuple[str, str], other: str
) -> None:
    enum = getattr(ub, cls)
    first, second = (getattr(enum, member) for member in members)
    one = _setter(BASE, name)(first)
    assert isinstance(one, ub.Theme)
    assert one == _setter(BASE, name)(first)
    assert one != _setter(BASE, name)(second)
    assert BASE == ub.Theme.MIDNIGHT


@pytest.mark.parametrize(("name", "cls", "members", "other"), ENUM_SETTERS)
def test_enum_setters_pass_the_enum_value(
    name: str, cls: str, members: tuple[str, str], other: str
) -> None:
    enum = getattr(ub, cls)
    for member in members:
        value = getattr(enum, member)
        assert native(_setter(BASE, name)(value)) == getattr(native(BASE), name)(value.value)


@pytest.mark.parametrize(("name", "cls", "members", "other"), ENUM_SETTERS)
def test_enum_setters_reject_other_types(
    name: str, cls: str, members: tuple[str, str], other: str
) -> None:
    enum = getattr(ub, cls)
    names = [f"{cls}.{member.name}" for member in enum]
    hint = f"use {', '.join(names[:-1])} or {names[-1]}."
    other_enum, other_name = other.split(".")
    other_member = getattr(getattr(ub, other_enum), other_name)
    for value in ("SOLID", 0, None, other_member):
        with pytest.raises(TypeError) as caught:
            _setter(BASE, name)(value)
        assert str(caught.value) == (
            f"Theme.{name}(): value is {_type_name(value)}; value must be of type {cls}."
        )
        assert details(caught.value) == ("value", hint)


def test_enum_type_error_highlights_the_value(tmp_path: Path) -> None:
    line = "ub.Theme.PIGMENT.with_grid_dash(ub.GridLines.BOTH)"
    err = _raised(tmp_path, line)
    assert isinstance(err, TypeError)
    assert _notes(err) == [
        _marked(line, "ub.GridLines.BOTH"),
        "hint: use Dash.SOLID, Dash.DASHED or Dash.DOTTED.",
    ]


def test_font_hint_lists_every_font() -> None:
    with pytest.raises(TypeError) as caught:
        BASE.with_title_font(cast(ub.Font, "Inter"))
    assert details(caught.value) == (
        "value",
        "use Font.SOURCE_SERIF_4, Font.IBM_PLEX_SANS, Font.GEIST, Font.INTER "
        "or Font.JETBRAINS_MONO.",
    )


# Numbers


@pytest.mark.parametrize(("name", "values", "bad", "required"), NUMERIC_SETTERS)
def test_numeric_setters_return_a_new_theme(
    name: str, values: tuple[float, float], bad: str, required: str
) -> None:
    setter = _setter(BASE, f"with_{name}")
    one = setter(values[0])
    assert isinstance(one, ub.Theme)
    assert one == setter(values[0])
    assert one != setter(values[1])
    assert native(one) == getattr(native(BASE), f"with_{name}")(values[0])
    assert BASE == ub.Theme.MIDNIGHT


@pytest.mark.parametrize(("name", "values", "bad", "required"), NUMERIC_SETTERS)
def test_numeric_setters_accept_ints_and_numpy_numbers(
    name: str, values: tuple[float, float], bad: str, required: str
) -> None:
    setter = _setter(BASE, f"with_{name}")
    value = int(values[1])
    assert setter(value) == setter(float(value))
    assert setter(np.int64(value)) == setter(float(value))
    assert setter(np.float32(values[1])) == setter(values[1])


@pytest.mark.parametrize(("name", "values", "bad", "required"), NUMERIC_SETTERS)
def test_numeric_setters_report_a_value_that_is_out_of_range(
    name: str, values: tuple[float, float], bad: str, required: str
) -> None:
    value = float(bad)
    with pytest.raises(ub.StyleError) as caught:
        _setter(BASE, f"with_{name}")(value)
    shown = f"{value:g}"
    assert str(caught.value) == (
        f"Theme.with_{name}(): {name} is {shown}; {name} must be {required}."
    )
    assert details(caught.value) == ("value", None)
    assert BASE == ub.Theme.MIDNIGHT


@pytest.mark.parametrize(("name", "values", "bad", "required"), NUMERIC_SETTERS)
@pytest.mark.parametrize("value", ["1.5", None, True, np.True_, [1.0]])
def test_numeric_setters_reject_a_non_number(
    name: str, values: tuple[float, float], bad: str, required: str, value: object
) -> None:
    with pytest.raises(TypeError) as caught:
        _setter(BASE, f"with_{name}")(value)
    type_name = _type_name(value)
    assert str(caught.value) == (
        f"Theme.with_{name}(): value is {type_name}; value must be a float."
    )
    assert details(caught.value) == ("value", "pass a number such as 1.5.")


@pytest.mark.parametrize(("name", "values", "bad", "required"), NUMERIC_SETTERS)
def test_numeric_setters_reject_non_finite_numbers(
    name: str, values: tuple[float, float], bad: str, required: str
) -> None:
    for value in (float("nan"), float("inf"), 10**400):
        with pytest.raises(ub.StyleError) as caught:
            _setter(BASE, f"with_{name}")(value)
        assert details(caught.value) == ("value", None)


@pytest.mark.parametrize(("name", "values", "bad", "required"), NUMERIC_SETTERS)
def test_numeric_setter_errors_highlight_the_value(
    name: str, values: tuple[float, float], bad: str, required: str, tmp_path: Path
) -> None:
    line = f"ub.Theme.PIGMENT.with_{name}({bad})"
    err = _raised(tmp_path, line)
    assert isinstance(err, ub.StyleError)
    assert _notes(err) == [_marked(line, bad)]
    line = f"ub.Theme.PIGMENT.with_{name}(value='x')"
    err = _raised(tmp_path, line)
    assert isinstance(err, TypeError)
    assert _notes(err)[0] == _marked(line, "value='x'")


def test_a_failed_call_leaves_the_theme_unchanged() -> None:
    theme = ub.Theme.GRAPHITE.with_line_width(2.5)
    before = native(theme)
    with pytest.raises(ub.StyleError):
        theme.with_line_width(-1.0)
    with pytest.raises(TypeError):
        theme.with_palette(cast(list[ub.Color], [3]))
    assert native(theme) == before
    assert theme == ub.Theme.GRAPHITE.with_line_width(2.5)


def test_setters_chain() -> None:
    theme = (
        ub.Theme.PIGMENT.with_background(ub.Color.WHITE)
        .with_title_font(ub.Font.INTER)
        .with_line_width(3.0)
    )
    assert theme == ub.Theme.PIGMENT.with_line_width(3.0).with_title_font(
        ub.Font.INTER
    ).with_background(ub.Color.WHITE)
