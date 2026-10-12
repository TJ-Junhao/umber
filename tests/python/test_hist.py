import traceback
from collections.abc import Callable
from pathlib import Path
from typing import Any, cast

import numpy as np
import numpy.typing as npt
import pandas as pd
import pytest

import umber as ub
from umber._errors import details

COLOR_HINT = 'use Color.hex("#3366B8"), Color.rgb() or a named color such as Color.BLUE.'
TEXT_HINT = "pass a str; convert other values with str()."
TOO_LONG_HINT = "shorten the text; it would not fit the figure."
INTEGER_HINT = "pass a whole number such as 3."
LINEAR_X_HINT = "pass x=Scale.linear() to Figure.add_axes()."
HIST_NAN = "a histogram ignores NaN, so use it to leave a value out."
BINS_RULE = "bins must be between 1 and 10000."

type Numeric = ub.Axes[float, float]


def _notes(err: BaseException) -> list[str]:
    return list(getattr(err, "__notes__", []))


def _marked(line: str, argument: str) -> str:
    """The highlight note for ``argument`` in the source ``line``."""
    return f"  {line}\n{' ' * (2 + line.index(argument))}{'^' * len(argument)}"


def _raised(tmp_path: Path, line: str, setup: str = "") -> BaseException:
    """Run ``line`` as user code in a file, so the error is reported at its call site."""
    # A file per line: ``linecache`` would serve the first content for a reused name.
    path = tmp_path / f"user_code_{abs(hash((setup, line)))}.py"
    path.write_text(f"import umber as ub\n{setup}\n{line}\n")
    try:
        exec(compile(path.read_text(), str(path), "exec"), {})
    except Exception as err:
        return err
    raise AssertionError(f"{line} raised nothing")


def _package_frames(err: BaseException) -> list[str]:
    package = Path(ub.__file__).resolve().parent
    return [
        entry.name
        for entry in traceback.extract_tb(err.__traceback__)
        if Path(entry.filename).resolve().is_relative_to(package)
    ]


def _axes(x: ub.Scale[Any] | None = None) -> Numeric:
    fig = ub.Figure(3, 2)
    return cast(Numeric, fig.add_axes() if x is None else fig.add_axes(x=x))


VALUES = [1.2, 2.5, 2.7, 3.1, 4.8, 5.0]


def test_hist_returns_the_same_axes() -> None:
    ax = _axes()
    assert ax.hist(VALUES) is ax
    assert isinstance(ax.hist(VALUES, bins=3).title("T").xlim(0, 6), ub.Axes)


def test_hist_draws_bars() -> None:
    empty = _axes().figure.to_svg()
    assert _axes().hist(VALUES).figure.to_svg() != empty


def test_hist_accepts_lists_arrays_and_series() -> None:
    outputs: set[str] = set()
    for values in (
        VALUES,
        tuple(VALUES),
        np.array(VALUES),
        pd.Series(VALUES),
    ):
        outputs.add(_axes().hist(values, bins=4).figure.to_svg())
    assert len(outputs) == 1


def test_bins_change_the_output() -> None:
    few = _axes().hist(VALUES, bins=2).figure.to_svg()
    many = _axes().hist(VALUES, bins=5).figure.to_svg()
    assert few != many
    assert _axes().hist(VALUES, bins=cast(int, np.int16(5))).figure.to_svg() == many
    assert _axes().hist(VALUES).figure.to_svg() == _axes().hist(VALUES, bins=30).figure.to_svg()


def test_bins_limits() -> None:
    _axes().hist(VALUES, bins=1)
    _axes().hist(VALUES, bins=ub.MAX_BINS)


def test_style_arguments_reach_the_figure() -> None:
    red = _axes().hist(VALUES, color=ub.Color.RED).figure.to_svg()
    assert 'fill="#D64933"' in red
    labelled = _axes().hist(VALUES, label="Sizes").figure
    assert ">Sizes</text>" in labelled.to_svg(text=ub.SvgText.EDITABLE)


def test_nan_values_are_left_out() -> None:
    with_nan = _axes().hist([*VALUES, np.nan], bins=4).figure.to_svg()
    assert with_nan == _axes().hist(VALUES, bins=4).figure.to_svg()


# (call, axes factory, error class, message, parameter, hint)
HIST_ERRORS: list[
    tuple[str, Callable[[], Numeric], Callable[[Numeric], object], type, str, Any, Any]
] = [
    (
        "no bins",
        _axes,
        lambda ax: ax.hist(VALUES, bins=0),
        ub.StyleError,
        f"Axes.hist(): bins is 0; {BINS_RULE}",
        "bins",
        "use at least one bin.",
    ),
    (
        "negative bins",
        _axes,
        lambda ax: ax.hist(VALUES, bins=-3),
        ub.StyleError,
        f"Axes.hist(): bins is -3; {BINS_RULE}",
        "bins",
        "use at least one bin.",
    ),
    (
        "too many bins",
        _axes,
        lambda ax: ax.hist(VALUES, bins=ub.MAX_BINS + 1),
        ub.StyleError,
        f"Axes.hist(): bins is 10001; {BINS_RULE}",
        "bins",
        None,
    ),
    (
        "huge bins",
        _axes,
        lambda ax: ax.hist(VALUES, bins=2**70),
        ub.StyleError,
        f"Axes.hist(): bins is 9223372036854775807; {BINS_RULE}",
        "bins",
        None,
    ),
    (
        "category x",
        lambda: _axes(ub.Scale.category()),
        lambda ax: ax.hist(VALUES),
        ub.DataError,
        "Axes.hist(): values holds numbers but the x axis is a category axis; use a linear x axis.",
        None,
        LINEAR_X_HINT,
    ),
    (
        "log x",
        lambda: _axes(ub.Scale.log()),
        lambda ax: ax.hist(VALUES),
        ub.UsageError,
        "Axes.hist(): the x axis is a log axis, which histograms do not support; "
        "use a linear x axis.",
        None,
        LINEAR_X_HINT,
    ),
    (
        "infinite",
        _axes,
        lambda ax: ax.hist([1.0, np.inf]),
        ub.DataError,
        "Axes.hist(): values[1] is inf; values must be finite.",
        "values",
        HIST_NAN,
    ),
    (
        "magnitude",
        _axes,
        lambda ax: ax.hist([1.0, -1e301]),
        ub.DataError,
        "Axes.hist(): values[1] is -1e301; magnitudes must not exceed 1e300.",
        "values",
        None,
    ),
    (
        "values type",
        _axes,
        lambda ax: ax.hist(cast(list[float], 3.0)),
        TypeError,
        "Axes.hist(): values is float; values must be a sequence or an array.",
        "values",
        "wrap a single value in a list.",
    ),
    (
        "values dtype",
        _axes,
        lambda ax: ax.hist(np.array([True, False])),
        ub.DataError,
        "Axes.hist(): values has dtype bool; values must be numbers.",
        "values",
        "convert with .astype(float) if 0 and 1 are meant.",
    ),
    (
        "bins float",
        _axes,
        lambda ax: ax.hist(VALUES, bins=cast(int, 3.0)),
        TypeError,
        "Axes.hist(): bins is float; bins must be an int.",
        "bins",
        INTEGER_HINT,
    ),
    (
        "bins bool",
        _axes,
        lambda ax: ax.hist(VALUES, bins=True),
        TypeError,
        "Axes.hist(): bins is bool; bins must be an int.",
        "bins",
        INTEGER_HINT,
    ),
    (
        "label type",
        _axes,
        lambda ax: ax.hist(VALUES, label=cast(str, None)),
        TypeError,
        "Axes.hist(): label is None; label must be a str.",
        "label",
        TEXT_HINT,
    ),
    (
        "label length",
        _axes,
        lambda ax: ax.hist(VALUES, label="x" * 1001),
        ub.StyleError,
        "Axes.hist(): label has 1,001 characters; text must be at most 1,000 characters.",
        "label",
        TOO_LONG_HINT,
    ),
    (
        "color type",
        _axes,
        lambda ax: ax.hist(VALUES, color=cast(ub.Color, "red")),
        TypeError,
        "Axes.hist(): color is str; color must be of type Color.",
        "color",
        COLOR_HINT,
    ),
]


@pytest.mark.parametrize(
    ("make", "call", "cls", "message", "parameter", "hint"),
    [case[1:] for case in HIST_ERRORS],
    ids=[case[0] for case in HIST_ERRORS],
)
def test_hist_errors_leave_the_figure_unchanged(
    make: Callable[[], Numeric],
    call: Callable[[Numeric], object],
    cls: type[BaseException],
    message: str,
    parameter: object,
    hint: object,
) -> None:
    ax = make().title("Kept")
    before = ax.figure.to_svg(text=ub.SvgText.EDITABLE)
    with pytest.raises(cls) as caught:
        call(ax)
    assert type(caught.value) is cls
    assert str(caught.value) == message
    assert details(caught.value) == (parameter, hint)
    assert _package_frames(caught.value) == []
    assert ax.figure.to_svg(text=ub.SvgText.EDITABLE) == before


class _RecordsConversion:
    """A data object that notes whether it was converted to an array."""

    def __init__(self) -> None:
        self.converted = False

    def __array__(self, dtype: object = None, copy: object = None) -> npt.NDArray[np.float64]:
        self.converted = True
        return np.array([1.0, 2.0])


def _parameter(err: BaseException) -> object:
    found = details(err)
    assert found is not None
    return found[0]


def test_type_checks_follow_parameter_order() -> None:
    hist = cast(Callable[..., object], _axes().hist)
    with pytest.raises(TypeError) as caught:
        hist("abc", bins="3", label=1, color="red")
    assert _parameter(caught.value) == "values"
    with pytest.raises(TypeError) as caught:
        hist([1.0], bins="3", label=1, color="red")
    assert details(caught.value) == ("bins", INTEGER_HINT)
    with pytest.raises(TypeError) as caught:
        hist([1.0], label=1, color="red")
    assert details(caught.value) == ("label", TEXT_HINT)
    with pytest.raises(TypeError) as caught:
        hist([1.0], color="red")
    assert details(caught.value) == ("color", COLOR_HINT)


def test_types_are_checked_before_any_conversion() -> None:
    hist = cast(Callable[..., object], _axes().hist)
    with pytest.raises(TypeError) as caught:
        hist([0, "bad"], bins="3")
    assert _parameter(caught.value) == "bins"
    with pytest.raises(TypeError) as caught:
        hist([0, "bad"], label="x" * 1001, color="red")
    assert _parameter(caught.value) == "color"
    recorder = _RecordsConversion()
    with pytest.raises(TypeError) as caught:
        hist(recorder, bins="3")
    assert _parameter(caught.value) == "bins"
    assert not recorder.converted
    with pytest.raises(ub.DataError) as data:
        hist([0, "bad"])
    assert _parameter(data.value) == "values"


@pytest.mark.parametrize(
    ("call", "argument"),
    [
        ("ax.hist([1.0, inf])", "[1.0, inf]"),
        ("ax.hist(values=[1.0, inf], bins=3)", "values=[1.0, inf]"),
        ("ax.hist([1.0], bins=0)", "bins=0"),
        ("ax.hist([1.0], bins=2.5)", "bins=2.5"),
        ("ax.hist([1.0], label='h', color='red')", "color='red'"),
        ("ax.title('T').hist(\n    [inf])", "[inf]"),
    ],
)
def test_hist_errors_highlight_the_argument(tmp_path: Path, call: str, argument: str) -> None:
    setup = "inf = float('inf')\nax = ub.Figure().add_axes()"
    err = _raised(tmp_path, call, setup)
    shown = call.splitlines()[-1].strip() if "\n" in call else call
    assert _notes(err)[0] == _marked(shown, argument)


def test_axis_errors_have_a_hint_and_no_highlight(tmp_path: Path) -> None:
    err = _raised(tmp_path, "ub.Figure().add_axes(x=ub.Scale.log()).hist([1.0])")
    assert isinstance(err, ub.UsageError)
    assert _notes(err) == [f"hint: {LINEAR_X_HINT}"]


def test_max_bins_is_exported() -> None:
    assert ub.MAX_BINS == 10_000
    assert "MAX_BINS" in ub.__all__
    with pytest.raises(ub.StyleError) as caught:
        _axes().hist(VALUES, bins=ub.MAX_BINS + 1)
    assert str(ub.MAX_BINS) in str(caught.value)


def test_masked_array_is_a_value_error_after_the_types() -> None:
    masked = np.ma.array([1.0, 2.0])
    with pytest.raises(TypeError) as caught:
        cast(Any, _axes()).hist(masked, bins="bad")
    assert _parameter(caught.value) == "bins"
    with pytest.raises(ub.DataError) as data:
        _axes().hist(masked)
    assert _parameter(data.value) == "values"
