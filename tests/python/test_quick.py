import os
import subprocess
import sys
import traceback
from collections.abc import Callable
from pathlib import Path
from typing import cast

import numpy as np
import pytest

import umber as ub
from umber._errors import details

THEME_HINT = "use Theme.PIGMENT, Theme.GRAPHITE or Theme.MIDNIGHT."
COLOR_HINT = 'use Color.hex("#3366B8"), Color.rgb() or a named color such as Color.BLUE.'
DASH_HINT = "use Dash.SOLID, Dash.DASHED or Dash.DOTTED."
LINE_GAP = "use NaN to leave a gap in the line."
CATEGORY_X_HINT = "pass x=Scale.category() to Figure.add_axes()."
QUICK = ["line", "scatter", "bar", "hist"]


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


def _frames(err: BaseException) -> list[str]:
    return [entry.name for entry in traceback.extract_tb(err.__traceback__)]


def _package_frames(err: BaseException) -> list[str]:
    package = Path(ub.__file__).resolve().parent
    return [
        entry.name
        for entry in traceback.extract_tb(err.__traceback__)
        if Path(entry.filename).resolve().is_relative_to(package)
    ]


def _quick(name: str) -> Callable[..., ub.Axes[object, float]]:
    return cast(Callable[..., ub.Axes[object, float]], getattr(ub, name))


# Each quick call, its arguments and the same chart built step by step.
BUILT: dict[str, tuple[tuple[object, ...], Callable[[ub.Theme], ub.Figure]]] = {
    "line": (
        ([0, 1, 2], [1.0, 3.0, 2.0]),
        lambda theme: ub.Figure(theme=theme).add_axes().line([0, 1, 2], [1.0, 3.0, 2.0]).figure,
    ),
    "scatter": (
        ([0, 1, 2], [1.0, 3.0, 2.0]),
        lambda theme: ub.Figure(theme=theme).add_axes().scatter([0, 1, 2], [1.0, 3.0, 2.0]).figure,
    ),
    "bar": (
        (["a", "b"], [3.0, 5.0]),
        lambda theme: (
            ub.Figure(theme=theme)
            .add_axes(x=ub.Scale.category())
            .bar(["a", "b"], [3.0, 5.0])
            .figure
        ),
    ),
    "hist": (
        ([1.0, 2.0, 2.5, 4.0],),
        lambda theme: ub.Figure(theme=theme).add_axes().hist([1.0, 2.0, 2.5, 4.0]).figure,
    ),
}


@pytest.mark.parametrize("name", QUICK)
def test_quick_function_matches_the_step_by_step_chart(name: str) -> None:
    arguments, build = BUILT[name]
    ax = _quick(name)(*arguments)
    assert isinstance(ax, ub.Axes)
    assert isinstance(ax.figure, ub.Figure)
    assert ax.figure.axes is ax
    assert ax.figure.to_svg() == build(ub.Theme.PIGMENT).to_svg()


@pytest.mark.parametrize("name", QUICK)
def test_quick_function_uses_the_theme(name: str) -> None:
    arguments, build = BUILT[name]
    ax = _quick(name)(*arguments, theme=ub.Theme.MIDNIGHT)
    assert ax.figure.to_svg() == build(ub.Theme.MIDNIGHT).to_svg()
    assert ax.figure.to_svg() != build(ub.Theme.PIGMENT).to_svg()


@pytest.mark.parametrize("name", QUICK)
def test_each_call_makes_a_new_figure(name: str) -> None:
    arguments, _ = BUILT[name]
    first = _quick(name)(*arguments)
    second = _quick(name)(*arguments)
    assert first is not second
    assert first.figure is not second.figure
    before = second.figure.to_svg()
    first.title("Only the first")
    assert second.figure.to_svg() == before


def test_style_arguments_are_passed_on() -> None:
    styled = ub.line([0, 1], [0, 1], label="North", color=ub.Color.RED, dash=ub.Dash.DOTTED)
    built = (
        ub.Figure()
        .add_axes()
        .line([0, 1], [0, 1], label="North", color=ub.Color.RED, dash=ub.Dash.DOTTED)
    )
    assert styled.figure.to_svg() == built.figure.to_svg()
    marked = ub.scatter([0, 1], [0, 1], marker=ub.Marker.CROSS, color=ub.Color.TEAL)
    built = (
        ub.Figure().add_axes().scatter([0, 1], [0, 1], marker=ub.Marker.CROSS, color=ub.Color.TEAL)
    )
    assert marked.figure.to_svg() == built.figure.to_svg()
    bars = ub.bar(["a"], [1.0], label="2025", color=ub.Color.BLUE)
    built_bars = (
        ub.Figure()
        .add_axes(x=ub.Scale.category())
        .bar(["a"], [1.0], label="2025", color=ub.Color.BLUE)
    )
    assert bars.figure.to_svg() == built_bars.figure.to_svg()
    hist = ub.hist([1.0, 2.0, 4.0], bins=2, label="h", color=ub.Color.GREEN)
    built = ub.Figure().add_axes().hist([1.0, 2.0, 4.0], bins=2, label="h", color=ub.Color.GREEN)
    assert hist.figure.to_svg() == built.figure.to_svg()


def test_bar_returns_category_axes() -> None:
    ax = ub.bar(["a", "b"], [1.0, 2.0])
    ax.bar(["c"], [3.0])
    with pytest.raises(ub.UsageError):
        cast(ub.Axes[float, float], ax).xlim(0, 1)


def test_quick_functions_report_the_package_as_their_module() -> None:
    for name in QUICK:
        assert _quick(name).__module__ == "umber"


@pytest.mark.parametrize("name", QUICK)
@pytest.mark.parametrize("theme", ["midnight", None, ub.Theme])
def test_theme_must_be_a_theme(name: str, theme: object) -> None:
    arguments, _ = BUILT[name]
    with pytest.raises(TypeError) as caught:
        _quick(name)(*arguments, theme=theme)
    shown = "None" if theme is None else type(theme).__name__
    assert str(caught.value) == f"{name}(): theme is {shown}; theme must be of type Theme."
    assert details(caught.value) == ("theme", THEME_HINT)
    assert _package_frames(caught.value) == []


def test_theme_error_highlights_the_theme(tmp_path: Path) -> None:
    line = "ub.bar(['a'], [1.0], theme='graphite')"
    err = _raised(tmp_path, line)
    assert _notes(err) == [_marked(line, "theme='graphite'"), f"hint: {THEME_HINT}"]


# (quick call, message, parameter, hint)
MARK_ERRORS: list[tuple[Callable[[], object], type, str, str | None, str | None]] = [
    (
        lambda: ub.line([0, 1], [1.0, np.inf]),
        ub.DataError,
        "Axes.line(): y[1] is inf; values must be finite.",
        "y",
        LINE_GAP,
    ),
    (
        lambda: ub.line([0, 1], [1.0], dash=cast(ub.Dash, "--")),
        TypeError,
        "Axes.line(): dash is str; dash must be of type Dash.",
        "dash",
        DASH_HINT,
    ),
    (
        lambda: ub.line(cast(list[float], ["a"]), [1.0]),
        ub.DataError,
        "Axes.line(): x holds category names but the x axis is not a category axis; pass numbers.",
        None,
        CATEGORY_X_HINT,
    ),
    (
        lambda: ub.scatter([0, 1], [1.0], color=cast(ub.Color, "red")),
        TypeError,
        "Axes.scatter(): color is str; color must be of type Color.",
        "color",
        COLOR_HINT,
    ),
    (
        lambda: ub.bar(["a", "a"], [1.0, 2.0]),
        ub.DataError,
        'Axes.bar(): categories[1] repeats "a" from categories[0]; each category must appear once.',
        "categories",
        "combine repeated categories and sum their heights.",
    ),
    (
        lambda: ub.hist([1.0], bins=0),
        ub.StyleError,
        "Axes.hist(): bins is 0; bins must be between 1 and 10000.",
        "bins",
        "use at least one bin.",
    ),
]


@pytest.mark.parametrize(("call", "cls", "message", "parameter", "hint"), MARK_ERRORS)
def test_mark_errors_keep_the_mark_context(
    call: Callable[[], object],
    cls: type[BaseException],
    message: str,
    parameter: str | None,
    hint: str | None,
) -> None:
    with pytest.raises(cls) as caught:
        call()
    assert type(caught.value) is cls
    assert str(caught.value) == message
    assert details(caught.value) == (parameter, hint)
    assert _package_frames(caught.value) == []


@pytest.mark.parametrize(
    ("call", "argument"),
    [
        ("ub.line(t, y, label='North')", "y"),
        ("ub.line(x=t, y=y)", "y=y"),
        ("ub.scatter(y, t)", "y"),
        ("ub.line(t, t, dash='--')", "dash='--'"),
        ("ub.bar(['a', 'a'], [1, 2])", "['a', 'a']"),
        ("ub.bar(['a', 'b'], heights=y[:2])", "heights=y[:2]"),
        ("ub.hist(y, bins=4)", "y"),
        ("ub.hist(t, bins=0)", "bins=0"),
        ("ub.line(t,\n    y).title('Signal')", "y"),
    ],
)
def test_mark_errors_highlight_the_quick_call(tmp_path: Path, call: str, argument: str) -> None:
    setup = "t = [0.0, 1.0, 2.0]\ny = [1.0, float('inf'), 2.0]"
    err = _raised(tmp_path, call, setup)
    shown = call.splitlines()[-1].strip() if "\n" in call else call
    assert _notes(err)[0] == _marked(shown, argument)
    assert len(_notes(err)) == 2


def test_full_error_output_points_at_the_quick_call(tmp_path: Path) -> None:
    script = tmp_path / "plot.py"
    script.write_text(
        "import numpy as np\n"
        "import umber as ub\n"
        "\n"
        "t = np.linspace(0.0, 1.0, 20)\n"
        "y = np.sin(t)\n"
        "y[17] = np.inf\n"
        'ub.line(t, y, label="North")\n'
    )
    environment = {
        key: value
        for key, value in os.environ.items()
        if key not in {"FORCE_COLOR", "PYTHON_COLORS"}
    }
    environment["NO_COLOR"] = "1"
    finished = subprocess.run(
        [sys.executable, "plot.py"],
        cwd=tmp_path,
        env=environment,
        capture_output=True,
        text=True,
        check=False,
    )
    assert finished.returncode == 1
    shown = finished.stderr.replace(str(script.resolve()), "plot.py").replace(
        str(script), "plot.py"
    )
    # From Python 3.13 the interpreter underlines the failing call itself.
    underline = "    ~~~~~~~^^^^^^^^^^^^^^^^^^^^^\n" if sys.version_info >= (3, 13) else ""
    assert shown == (
        "Traceback (most recent call last):\n"
        '  File "plot.py", line 7, in <module>\n'
        '    ub.line(t, y, label="North")\n'
        f"{underline}"
        "umber.DataError: Axes.line(): y[17] is inf; values must be finite.\n"
        '  ub.line(t, y, label="North")\n'
        "             ^\n"
        "hint: use NaN to leave a gap in the line.\n"
    )


def test_the_quick_function_example_works(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.chdir(tmp_path)
    t = np.linspace(0.0, 10.0, 200)
    y = np.sin(t)
    ub.line(t, y).title("Signal").figure.save_svg("signal.svg")
    svg = (tmp_path / "signal.svg").read_text(encoding="utf-8")
    assert svg.startswith("<svg")
    expected = ub.Figure().add_axes().line(t, y).title("Signal").figure.to_svg()
    assert svg == expected


def test_frames_of_a_failed_quick_call_are_the_users() -> None:
    with pytest.raises(ub.DataError) as caught:
        ub.hist([np.inf])
    assert _frames(caught.value) == ["test_frames_of_a_failed_quick_call_are_the_users"]
