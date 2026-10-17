"""Per-axes baseline visibility, replacement, errors and rendering invariants."""

import copy
import xml.etree.ElementTree as ET
from collections.abc import Callable
from pathlib import Path
from typing import Any, cast

import pytest

import umber as ub
from baseline_check import assert_only_baseline_added
from umber._errors import details

TYPE_HINT = "pass True or False, or ub.AUTO to follow the marks."
USAGE_HINT = "use shown=False or shown=ub.AUTO, or create axes with a numeric or time value axis."
NS = "{http://www.w3.org/2000/svg}"


def axes(kind: str) -> ub.Axes[Any, float]:
    x: ub.Scale[Any] = ub.Scale.category() if kind == "bar" else ub.Scale.linear()
    ax = ub.Figure(6, 4).add_axes(x=x)
    if kind == "line":
        ax.line([1, 2], [1, 2])
    elif kind == "bar":
        ax.bar(["A", "B"], [1, 2])
    elif kind == "mesh":
        ax.mesh([0, 1, 2], [-1, 0, 1], [[1, 2], [3, 4]])
        ax.colorbar(position=ub.ColorbarPosition.NONE)
    return ax


def baselines(svg: str) -> list[ET.Element]:
    return [
        path
        for path in ET.fromstring(svg).iter(f"{NS}path")
        if path.get("stroke") == "#2B211B" and path.get("stroke-width") == "1.3"
    ]


@pytest.mark.parametrize("kind", ["line", "bar", "mesh", "empty"])
@pytest.mark.parametrize("shown", [True, False, ub.AUTO])
def test_values_replace_and_auto_restores(kind: str, shown: bool | ub.Auto) -> None:
    ax = axes(kind)
    default = ax.figure.to_svg()
    ax.baseline(shown=False)
    hidden = ax.figure.to_svg()
    ax.baseline(shown=True)
    visible = ax.figure.to_svg()
    assert_only_baseline_added(hidden, visible)
    assert ax.baseline(shown=shown) is ax
    expected = kind == "bar" if shown is ub.AUTO else shown
    assert ax.figure.to_svg() == (visible if expected else hidden)
    assert len(baselines(ax.figure.to_svg())) == int(expected)
    ax.baseline(shown=ub.AUTO)
    assert ax.figure.to_svg() == default


@pytest.mark.parametrize("kind", ["line", "bar", "mesh", "empty"])
def test_zero_gridline_follows_visibility(kind: str) -> None:
    ax = axes(kind).ylim(-2, 2).yticks([-2, 0, 2])
    ax.baseline(shown=True)
    visible = ax.figure.to_svg()
    path = baselines(visible)[0].get("d")
    ax.baseline(shown=False)
    hidden = ax.figure.to_svg()
    assert_only_baseline_added(hidden, visible)
    grids = [
        item
        for item in ET.fromstring(hidden).iter(f"{NS}path")
        if item.get("stroke") == "#DDD3C7" and item.get("d") == path
    ]
    assert len(grids) == 1
    assert not any(
        item.get("stroke") == "#DDD3C7" and item.get("d") == path
        for item in ET.fromstring(visible).iter(f"{NS}path")
    )


@pytest.mark.parametrize(("value", "name"), [(None, "None"), (1, "int"), ("yes", "str")])
def test_wrong_types_are_atomic(value: object, name: str) -> None:
    ax = axes("bar").baseline(shown=False)
    before = ax.figure.to_svg()
    call = cast(Callable[..., object], ax.baseline)
    with pytest.raises(TypeError) as caught:
        call(shown=value)
    assert str(caught.value) == (
        f"Axes.baseline(): shown is {name}; shown must be a bool or ub.AUTO."
    )
    assert details(caught.value) == ("shown", TYPE_HINT)
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize("argument", ["shown=None", "shown=1", 'shown="yes"', "shown=True"])
def test_errors_highlight_shown(tmp_path: Path, argument: str) -> None:
    line = f"ax.baseline({argument})"
    path = tmp_path / "baseline_call.py"
    source = (
        "import umber as ub\n"
        "ax = ub.Figure().add_axes(x=ub.Scale.category(), y=ub.Scale.category())\n"
        f"{line}\n"
    )
    path.write_text(source)
    with pytest.raises(ub.UsageError if argument == "shown=True" else TypeError) as caught:
        exec(compile(source, str(path), "exec"), {})
    marked = f"  {line}\n{' ' * (2 + line.index(argument))}{'^' * len(argument)}"
    assert getattr(caught.value, "__notes__", [])[0] == marked


def test_error_text_corpus() -> None:
    ax = ub.Figure().add_axes(x=ub.Scale.category(), y=ub.Scale.category())
    call = cast(Callable[..., object], ax.baseline)
    cases: list[tuple[str, object]] = [
        ("shown=None", None),
        ("shown=1", 1),
        ('shown="yes"', "yes"),
        ("shown=True on two category axes", True),
    ]
    records: list[str] = []
    for label, shown in cases:
        with pytest.raises((TypeError, ub.UsageError)) as caught:
            call(shown=shown)
        err = caught.value
        found = details(err)
        assert found is not None
        parameter, hint = found
        records.append(
            f"{label}\n{type(err).__name__}: {err}\nparameter={parameter}\nhint={hint}\n"
        )
    expected = Path(__file__).parent / "fixtures" / "errors" / "baseline.txt"
    assert "\n".join(records) == expected.read_text()


def test_shown_is_required_and_keyword_only() -> None:
    call = cast(Callable[..., object], axes("empty").baseline)
    with pytest.raises(TypeError, match="required keyword-only argument: 'shown'"):
        call()
    with pytest.raises(TypeError, match="positional argument"):
        call(True)


def test_two_category_axes_reject_shown_at_call_time() -> None:
    ax = ub.Figure().add_axes(x=ub.Scale.category(), y=ub.Scale.category())
    for shown in (False, ub.AUTO):
        assert ax.baseline(shown=shown) is ax
        before = ax.figure.to_svg()
        with pytest.raises(ub.UsageError) as caught:
            ax.baseline(shown=True)
        assert str(caught.value) == (
            "Axes.baseline(): these axes have two category axes and no value axis; "
            "a baseline cannot be shown."
        )
        assert details(caught.value) == ("shown", USAGE_HINT)
        assert ax.figure.to_svg() == before
        assert not baselines(before)


@pytest.mark.parametrize("shown", [True, False, ub.AUTO])
def test_copy_keeps_setting(shown: bool | ub.Auto) -> None:
    fig = axes("bar").baseline(shown=shown).figure
    before = fig.to_svg()
    duplicate = copy.copy(fig)
    assert duplicate.to_svg() == before
    duplicate.axes_at(x=ub.Scale.category()).baseline(shown=False if shown is not False else True)
    assert duplicate.to_svg() != before
    assert fig.to_svg() == before


@pytest.mark.parametrize("horizontal", [False, True])
@pytest.mark.parametrize("log", [False, True])
def test_bar_bases_and_reference_clipping_stay_unchanged(horizontal: bool, log: bool) -> None:
    value_scale = ub.Scale.log() if log else ub.Scale.linear()
    fig = ub.Figure(6, 4)
    ax: ub.Axes[Any, Any]
    if horizontal:
        ax = fig.add_axes(x=value_scale, y=ub.Scale.category()).bar(["A", "B"], [2, 8])
        ax.xlim(1, 5).vline(8)
    else:
        ax = fig.add_axes(x=ub.Scale.category(), y=value_scale).bar(["A", "B"], [2, 8])
        ax.ylim(1, 5).hline(8)
    ax.baseline(shown=False)
    hidden = fig.to_svg()
    ax.baseline(shown=True)
    assert_only_baseline_added(hidden, fig.to_svg())


def test_area_fills_and_reference_clipping_stay_unchanged() -> None:
    ax = ub.Figure(6, 4).add_axes().area([0, 1, 2], [1, 8, 2]).ylim(1, 5).hline(8)
    ax.baseline(shown=False)
    hidden = ax.figure.to_svg()
    ax.baseline(shown=True)
    assert_only_baseline_added(hidden, ax.figure.to_svg())


def test_shared_panels_do_not_inherit_setting() -> None:
    fig = ub.Figure(10, 4, cols=2, share_x=ub.Share.ALL, share_y=ub.Share.ALL)
    left = fig.add_axes(col=0).line([1, 2], [1, 2]).baseline(shown=True)
    right = fig.add_axes(col=1).line([1, 2], [1, 2])
    assert len(baselines(fig.to_svg())) == 1
    right.baseline(shown=True)
    assert len(baselines(fig.to_svg())) == 2
    left.baseline(shown=False)
    assert len(baselines(fig.to_svg())) == 1
    right.baseline(shown=ub.AUTO)
    assert not baselines(fig.to_svg())
