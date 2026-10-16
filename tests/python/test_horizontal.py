# pyright: reportPrivateUsage=false
"""Category y axes: horizontal bars, boxes and violins, dot plots and their errors."""

from collections.abc import Callable
from pathlib import Path
from typing import Any, cast

import numpy as np
import pandas as pd
import pytest

import umber as ub
from umber._errors import details

PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"
COUNTRIES = ["United States", "Germany", "Brazil", "India"]
NUMERIC_Y_HINT = "use a linear or log y axis."
NUMERIC_X_HINT = "use a linear or log x axis."

type Horizontal = ub.Axes[float, str]


def _horizontal(x: ub.Scale[float] | None = None) -> Horizontal:
    fig = ub.Figure(4, 3)
    if x is None:
        return fig.add_axes(y=ub.Scale.category())
    return fig.add_axes(x=x, y=ub.Scale.category())


def _groups(shift: float = 0.0) -> list[list[float]]:
    return [[value + shift + i for value in (1.0, 2.0, 3.0, 5.0, 9.0)] for i in range(4)]


def _notes(err: BaseException) -> list[str]:
    return list(getattr(err, "__notes__", []))


def _marked(line: str, argument: str) -> str:
    """The highlight note for ``argument`` in the source ``line``."""
    return f"  {line}\n{' ' * (2 + line.index(argument))}{'^' * len(argument)}"


def _raised(tmp_path: Path, line: str, setup: str = "") -> BaseException:
    """Run ``line`` as user code in a file, so the error is reported at its call site."""
    path = tmp_path / f"user_code_{abs(hash((setup, line)))}.py"
    path.write_text(f"import umber as ub\n{setup}\n{line}\n")
    try:
        exec(compile(path.read_text(), str(path), "exec"), {})
    except Exception as err:
        return err
    raise AssertionError(f"{line} raised nothing")


def test_a_category_y_axis_gives_horizontal_axes() -> None:
    fig = ub.Figure()
    ax = fig.add_axes(y=ub.Scale.category())
    assert fig.axes_at(y=ub.Scale.category()) is ax
    assert ax.bar(COUNTRIES, [23, 54, 89, 21], label="Share") is ax
    assert fig.to_png().startswith(PNG_SIGNATURE)


def test_cell_add_axes_takes_a_category_y() -> None:
    fig = ub.Figure(cols=2)
    cell = next(fig.cells())
    ax = cell.add_axes(y=ub.Scale.category()).bar(["A", "B"], [3, 5])
    assert fig.axes_at(col=0, y=ub.Scale.category()) is ax


def test_every_horizontal_mark_renders() -> None:
    ax = _horizontal()
    distributions = [ub.Distribution("Car", _groups()), ub.Distribution("Transit", _groups(2))]
    series = [ub.Series("2015", [13, 30, 75, 16]), ub.Series("2023", [23, 54, 89, 21])]
    chained = (
        ax.bar(COUNTRIES, [23, 54, 89, 21], label="Share")
        .grouped_bar(COUNTRIES, series)
        .stacked_bar(COUNTRIES, series)
        .box(COUNTRIES, _groups(), label="All")
        .grouped_box(COUNTRIES, distributions)
        .violin(COUNTRIES, _groups(), label="All")
        .grouped_violin(COUNTRIES, distributions)
        .scatter([78.7, 81.0, 75.3, 68.3], COUNTRIES, label="2015")
        .x_error_bars(COUNTRIES, [70.0, 75.0, 70.0, 60.0], [80.0, 85.0, 80.0, 75.0])
        .vline(50.0, label="Target")
        .vspan(40.0, 45.0)
        .xlim(0, 100)
        .title("Renewables")
        .xlabel("Share, %")
    )
    assert chained is ax
    svg = ax.figure.to_svg(text=ub.SvgText.EDITABLE)
    for name in COUNTRIES:
        assert name in svg
    assert ax.figure.to_png().startswith(PNG_SIGNATURE)


def test_a_dot_plot_takes_names_in_any_container() -> None:
    values = [78.7, 81.0, 75.3, 68.3]
    for names in (COUNTRIES, tuple(COUNTRIES), np.array(COUNTRIES), pd.Series(COUNTRIES)):
        ax = _horizontal().scatter(values, names)
        assert ax.figure.to_png().startswith(PNG_SIGNATURE)
    # Empty data adds a mark with nothing to draw.
    assert _horizontal().scatter([], []).figure.to_png().startswith(PNG_SIGNATURE)


def test_numbers_on_a_category_y_are_data_errors() -> None:
    ax = _horizontal()
    before = ax.figure.to_svg()
    with pytest.raises(ub.DataError) as caught:
        ax.scatter([1.0], cast(Any, [2.0]))
    assert "category axis" in str(caught.value)
    with pytest.raises(ub.DataError):
        ax.x_error_bars(cast(Any, [1.0]), [0.0], [2.0])
    assert ax.figure.to_svg() == before


def test_horizontal_values_are_checked_on_the_x_axis() -> None:
    ax = _horizontal(ub.Scale.log())
    before = ax.figure.to_svg()
    with pytest.raises(ub.DataError) as caught:
        ax.bar(["A", "B"], [3.0, -1.0])
    assert str(caught.value) == (
        "Axes.bar(): heights[1] is -1 on a log axis; values must be positive."
    )
    assert ax.figure.to_svg() == before
    ax.bar(["A", "B"], [3.0, 10.0])


@pytest.mark.parametrize(
    ("name", "arguments"),
    [
        ("line", ([0.0, 1.0], [1.0, 2.0])),
        ("area", ([0.0, 1.0], [1.0, 2.0])),
        ("step", ([0.0, 1.0], [1.0, 2.0])),
        ("stem", ([0.0, 1.0], [1.0, 2.0])),
        ("hist", ([1.0, 2.0],)),
        ("hline", (1.0,)),
        ("hspan", (0.0, 1.0)),
        ("y_error_bars", ([0.0], [0.0], [1.0])),
    ],
)
def test_marks_without_a_horizontal_form_refuse_a_category_y(
    name: str, arguments: tuple[object, ...]
) -> None:
    ax = _horizontal()
    before = ax.figure.to_svg()
    method: Callable[..., object] = getattr(ax, name)
    with pytest.raises(ub.UsageError) as caught:
        method(*arguments)
    context = f"Axes.{name}()"
    assert str(caught.value) == (
        f"{context}: y is a category scale; {context} needs a numeric y axis."
    )
    assert details(caught.value) == ("y", NUMERIC_Y_HINT)
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize(
    ("name", "arguments"),
    [
        ("bar", (["A"], [1.0])),
        ("grouped_bar", (["A"], [ub.Series("S", [1.0])])),
        ("stacked_bar", (["A"], [ub.Series("S", [1.0])])),
        ("box", (["A"], [[1.0, 2.0]])),
        ("grouped_box", (["A"], [ub.Distribution("D", [[1.0]])])),
        ("violin", (["A"], [[1.0, 2.0]])),
        ("grouped_violin", (["A"], [ub.Distribution("D", [[1.0]])])),
        ("scatter", (["A"], ["B"])),
        ("x_error_bars", (["A"], [0.0], [1.0])),
        ("vline", ("A",)),
        ("vspan", (0.0, 1.0)),
    ],
)
def test_two_category_axes_take_no_mark_yet(name: str, arguments: tuple[object, ...]) -> None:
    ax = ub.Figure(4, 3).add_axes(x=ub.Scale.category(), y=ub.Scale.category())
    before = ax.figure.to_svg()
    method: Callable[..., object] = getattr(ax, name)
    with pytest.raises(ub.UsageError) as caught:
        method(*arguments)
    context = f"Axes.{name}()"
    assert str(caught.value) == (
        f"{context}: x is a category scale; {context} needs a numeric x axis."
    )
    assert details(caught.value) == ("x", NUMERIC_X_HINT)
    assert ax.figure.to_svg() == before


def test_bars_on_two_numeric_axes_keep_their_error() -> None:
    ax = cast(ub.Axes[str, float], ub.Figure(4, 3).add_axes())
    with pytest.raises(ub.DataError) as caught:
        ax.bar(["A"], [1.0])
    assert str(caught.value) == (
        "Axes.bar(): categories holds names but the x axis is not a category axis; "
        "use a category x axis."
    )


def test_category_y_limits_stay_refused(tmp_path: Path) -> None:
    err = _raised(tmp_path, "ax.ylim(0, 1)", "ax = ub.Figure().add_axes(y=ub.Scale.category())")
    assert isinstance(err, ub.UsageError)
    assert str(err) == (
        "Axes.ylim(): this is a category axis; limits apply to linear and log axes only."
    )


def test_a_dot_plot_value_error_highlights_its_argument(tmp_path: Path) -> None:
    line = "ax.scatter([1.0, float('inf')], ['A', 'B'])"
    err = _raised(tmp_path, line, "ax = ub.Figure().add_axes(y=ub.Scale.category())")
    assert isinstance(err, ub.DataError)
    assert str(err) == "Axes.scatter(): x[1] is inf; values must be finite."
    assert _notes(err)[0] == _marked(line, "[1.0, float('inf')]")


def test_many_long_categories_fit_or_name_the_label() -> None:
    names = [f"{i:03} " + ("category name " * 5)[:56] for i in range(100)]
    names[3] = "003 " + "類別名稱" * 14
    heights = [float(i % 9 + 1) for i in range(100)]
    wide = ub.Figure(16, 30).add_axes(y=ub.Scale.category()).bar(names, heights)
    assert wide.figure.to_png(dpi=72).startswith(PNG_SIGNATURE)
    narrow = ub.Figure(4, 30).add_axes(y=ub.Scale.category()).bar(names, heights)
    with pytest.raises(ub.StyleError) as caught:
        narrow.figure.to_svg()
    # The message quotes the widest label, here the CJK one, shortened with an ellipsis.
    message = str(caught.value)
    prefix = 'Figure.to_svg(): y tick label "'
    assert message.startswith(prefix), message
    quoted = message.removeprefix(prefix).split("…")[0]
    assert len(quoted) > 20
    assert names[3].startswith(quoted), message
    assert details(caught.value) == (None, "shorten the y tick labels or widen the figure.")
