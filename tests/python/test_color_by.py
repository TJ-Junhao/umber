# pyright: reportPrivateUsage=false
"""``ColorBy`` conversions, exact errors, highlights, atomicity and the color range."""

import copy
import pickle
from collections.abc import Callable
from pathlib import Path
from typing import Any, cast
from xml.etree import ElementTree

import numpy as np
import pandas as pd
import pytest

import umber as ub
from umber._errors import details

POINT_COLOR_HINT = (
    'use Color.hex("#3366B8"), Color.rgb(), a named color such as Color.BLUE, or ColorBy(values).'
)
SAME_HINT = "use the same cmap, scale and center for every color-mapped mark of an axes."
GRAYS = ub.Cmap.from_colors([ub.Color.BLACK, ub.Color.WHITE])


def _notes(err: BaseException) -> list[str]:
    return list(getattr(err, "__notes__", []))


def _marked(line: str, argument: str) -> str:
    """The highlight note for ``argument`` in the source ``line``."""
    return f"  {line}\n{' ' * (2 + line.index(argument))}{'^' * len(argument)}"


def _raised(tmp_path: Path, line: str) -> BaseException:
    """Run ``line`` as user code in a file, so the error is reported at its call site."""
    path = tmp_path / f"user_code_{abs(hash(line))}.py"
    path.write_text(f"import umber as ub\n{line}\n")
    try:
        exec(compile(path.read_text(), str(path), "exec"), {})
    except Exception as err:
        return err
    raise AssertionError(f"{line} raised nothing")


def axes() -> ub.Axes[float, float]:
    return ub.Figure(6, 4).add_axes()


def call(function: object) -> Callable[..., object]:
    return cast(Callable[..., object], function)


def test_color_by_is_exported_with_keyword_only_options() -> None:
    assert ub.ColorBy.__module__ == "umber"
    assert "ColorBy" in ub.__all__
    by = ub.ColorBy([1.0, 2.0], ub.Cmap.MAGMA, scale=ub.Scale.log(), center=ub.AUTO)
    assert isinstance(by, ub.ColorBy)
    with pytest.raises(TypeError):
        call(ub.ColorBy)([1.0], ub.Cmap.MAGMA, ub.Scale.linear())


def test_lists_arrays_and_series_match() -> None:
    outputs: set[str] = set()
    inputs: list[object] = [
        [0, 5, 10],
        (0.0, 5.0, 10.0),
        np.array([0, 5, 10], dtype=np.int16),
        pd.Series([0.0, 5.0, 10.0]),
    ]
    for values in inputs:
        by = cast(ub.ColorBy, call(ub.ColorBy)(values))
        ax = axes().scatter([1, 2, 3], [1, 2, 3], color=by)
        outputs.add(ax.figure.to_svg())
    assert len(outputs) == 1


def _marker_styles(svg: str) -> list[dict[str, str]]:
    ns = "{http://www.w3.org/2000/svg}"
    return [
        group.attrib
        for group in ElementTree.fromstring(svg).iter(f"{ns}g")
        if group.find(f"{ns}use") is not None
    ]


@pytest.mark.parametrize(
    ("theme", "background", "center_fill", "center_stroke"),
    [
        (ub.Theme.PIGMENT, "#FAF6F0", "#F3EDE4", "#ACA69E"),
        (ub.Theme.GRAPHITE, "#FFFFFF", "#F5F5F5", "#AFAFAF"),
        (ub.Theme.MIDNIGHT, "#121419", "#2A2F3A", "#41454E"),
    ],
)
def test_centered_color_by_outline_in_png_svg_and_pdf(
    theme: ub.Theme, background: str, center_fill: str, center_stroke: str
) -> None:
    fig = ub.Figure(6, 4, theme=theme)
    fig.add_axes().scatter([1, 2, 3], [1, 2, 1], color=ub.ColorBy([-1, 0, 1], center=0))
    assert fig.to_png().startswith(b"\x89PNG\r\n\x1a\n")
    assert fig.to_pdf().startswith(b"%PDF-")
    for text in (ub.SvgText.EDITABLE, ub.SvgText.OUTLINES):
        markers = _marker_styles(fig.to_svg(text=text))
        assert len(markers) == 3
        assert all(marker["stroke-width"] == "0.5" for marker in markers)
        assert markers[1]["fill"] == center_fill
        assert markers[1]["stroke"] == center_stroke != background
        assert markers[0]["stroke"] == markers[2]["stroke"] == background
        assert all(
            "fill-opacity" not in marker and "stroke-opacity" not in marker for marker in markers
        )


def test_points_take_the_mapped_colors() -> None:
    # Mapped fills are opaque, as explicit colors are under a theme without marker opacity.
    mapped = axes().scatter([1, 2], [1, 2], color=ub.ColorBy([0, 10], GRAYS))
    opaque = ub.Theme.PIGMENT.with_marker_alpha(1.0)
    explicit = (
        ub.Figure(6, 4, theme=opaque)
        .add_axes()
        .scatter([1], [1], color=ub.Color.BLACK)
        .scatter([2], [2], color=ub.Color.WHITE)
    )
    faded = axes().scatter([1], [1], color=ub.Color.BLACK).scatter([2], [2], color=ub.Color.WHITE)
    mapped.colorbar(position=ub.ColorbarPosition.NONE)
    assert mapped.figure.to_svg() != faded.figure.to_svg()
    mapped_styles = _marker_styles(mapped.figure.to_svg())
    explicit_styles = _marker_styles(explicit.figure.to_svg())
    assert [style["fill"] for style in mapped_styles] == [
        style["fill"] for style in explicit_styles
    ]
    assert all("fill-opacity" not in style for style in mapped_styles + explicit_styles)
    assert all(style["stroke-width"] == "0.5" for style in mapped_styles)
    assert all(style["stroke-width"] == "0.8" for style in explicit_styles)
    clamped = axes().clim(0, 10).scatter([1, 2], [1, 2], color=ub.ColorBy([-50, 50], GRAYS))
    clamped.colorbar(position=ub.ColorbarPosition.NONE)
    assert clamped.figure.to_svg() == mapped.figure.to_svg()


def test_a_mapped_scatter_takes_no_palette_color() -> None:
    ax = axes().scatter([1], [1], color=ub.ColorBy([1.0])).scatter([2], [2])
    reference = (
        axes()
        .scatter([1], [1], color=ub.ColorBy([1.0]))
        .scatter([2], [2], color=ub.Color.palette(0))
    )
    assert ax.figure.to_svg() == reference.figure.to_svg()


@pytest.mark.parametrize(
    ("values", "options", "error", "message", "parameter"),
    [
        ([1.0, np.inf], {}, ub.DataError, "values[1] is inf; values must be finite.", "values"),
        (
            [1.0, 2.0, 0.0],
            {"scale": ub.Scale.log()},
            ub.DataError,
            "values[2] is 0; values on a log scale must be positive.",
            "values",
        ),
        (
            [1.0],
            {"center": float("nan")},
            ub.StyleError,
            "center is NaN; it must be finite and at most 1e300 in magnitude.",
            "center",
        ),
        (
            [1.0],
            {"center": 1.0, "scale": ub.Scale.log(2)},
            ub.StyleError,
            "center is 1 with a log scale; a centered color scale is linear.",
            "center",
        ),
    ],
)
def test_invalid_values_raise_at_the_call(
    values: list[float],
    options: dict[str, Any],
    error: type[Exception],
    message: str,
    parameter: str,
) -> None:
    with pytest.raises(error) as caught:
        ub.ColorBy(values, **options)
    assert str(caught.value) == f"ColorBy(): {message}"
    assert details(caught.value) is not None
    assert cast(tuple[str, str], details(caught.value))[0] == parameter


def test_value_errors_highlight_the_values(tmp_path: Path) -> None:
    line = "ub.ColorBy([1.0, 0.0], scale=ub.Scale.log())"
    err = _raised(tmp_path, line)
    assert isinstance(err, ub.DataError)
    assert _notes(err)[0] == _marked(line, "[1.0, 0.0]")


@pytest.mark.parametrize(
    ("arguments", "message", "parameter"),
    [
        ({"values": "abc"}, None, "values"),
        ({"values": [1.0], "cmap": "viridis"}, "cmap is str; cmap must be of type Cmap.", "cmap"),
        (
            {"values": [1.0], "scale": ub.Scale.category()},
            "scale is Scale.category(); scale must be Scale.linear() or Scale.log().",
            "scale",
        ),
        ({"values": [1.0], "scale": "log"}, "scale is str; scale must be of type Scale.", "scale"),
        (
            {"values": [1.0], "center": "0"},
            "center is str; center must be a float or AUTO.",
            "center",
        ),
    ],
)
def test_wrong_types_raise_type_errors(
    arguments: dict[str, object], message: str | None, parameter: str
) -> None:
    with pytest.raises(TypeError) as caught:
        call(ub.ColorBy)(**arguments)
    if message is not None:
        assert str(caught.value) == f"ColorBy(): {message}"
    found = details(caught.value)
    assert found is not None and found[0] == parameter


def test_mismatched_cmap_is_rejected_atomically(tmp_path: Path) -> None:
    ax = axes().scatter([1, 2], [1, 2], color=ub.ColorBy([0, 1], ub.Cmap.VIRIDIS))
    before = ax.figure.to_svg()
    with pytest.raises(ub.UsageError) as caught:
        ax.scatter([3], [3], color=ub.ColorBy([2], ub.Cmap.MAGMA))
    assert str(caught.value) == (
        "Axes.scatter(): color uses Cmap.MAGMA; this axes already maps colors with Cmap.VIRIDIS."
    )
    assert details(caught.value) == ("color", SAME_HINT)
    with pytest.raises(ub.UsageError) as caught:
        ax.bubbles([3], [3], [1], color=ub.ColorBy([2], ub.Cmap.VIRIDIS, scale=ub.Scale.log()))
    assert str(caught.value) == (
        "Axes.bubbles(): color uses Scale.log(10); this axes already maps colors with "
        "Scale.linear()."
    )
    assert ax.figure.to_svg() == before
    line = (
        "ub.Figure().add_axes().scatter([1], [1], color=ub.ColorBy([1])).scatter("
        "[2], [2], color=ub.ColorBy([2], center=0))"
    )
    err = _raised(tmp_path, line)
    assert isinstance(err, ub.UsageError)
    assert _notes(err) == [
        _marked(line, "color=ub.ColorBy([2], center=0)"),
        f"hint: {SAME_HINT}",
    ]


def test_values_must_match_the_points() -> None:
    ax = axes()
    before = ax.figure.to_svg()
    with pytest.raises(ub.DataError) as caught:
        ax.scatter([1, 2], [1, 2], color=ub.ColorBy([1]))
    assert str(caught.value) == (
        "Axes.scatter(): x has 2 values but color has 1; x and color must have the same length."
    )
    assert details(caught.value) == ("color", "pass one color value per point.")
    assert ax.figure.to_svg() == before


def test_a_mapped_scatter_takes_no_label() -> None:
    ax = axes()
    with pytest.raises(ub.UsageError) as caught:
        ax.scatter([1], [1], label="a", color=ub.ColorBy([1]))
    assert str(caught.value) == (
        'Axes.scatter(): label is "a"; a color-mapped scatter has no legend entry.'
    )
    assert details(caught.value) == ("label", "remove the label; the colorbar explains the colors.")
    ax.bubbles([1], [1], [5], label="Population", color=ub.ColorBy([1]))


def test_wrong_color_types_name_both_accepted_types() -> None:
    for method in (axes().scatter, axes().bubbles):
        arguments = ([1], [1], [1]) if method.__name__ == "bubbles" else ([1], [1])
        with pytest.raises(TypeError) as caught:
            call(method)(*arguments, color="red")
        assert str(caught.value).endswith("color is str; color must be of type Color or ColorBy.")
        assert details(caught.value) == (
            "color",
            POINT_COLOR_HINT,
        )
    with pytest.raises(TypeError):
        call(axes().line)([1], [1], color=ub.ColorBy([1]))


def test_clim_validates_and_is_atomic() -> None:
    ax = axes()
    assert ax.clim(0, 10) is ax
    with pytest.raises(ub.StyleError) as caught:
        ax.clim(5, 2)
    assert str(caught.value) == "Axes.clim(): low is 5, high is 2; low must be below high."
    assert details(caught.value) == ("low", "swap the two values.")
    with pytest.raises(TypeError):
        call(ax.clim)("0", 1)
    ax.scatter([1], [1], color=ub.ColorBy([1], center=5))
    before = ax.figure.to_svg()
    with pytest.raises(ub.StyleError) as caught:
        ax.clim(6, 10)
    assert str(caught.value) == (
        "Axes.clim(): center is 5; it must lie inside the color range 6 to 10."
    )
    assert ax.figure.to_svg() == before
    log = axes().scatter([1], [1], color=ub.ColorBy([1], scale=ub.Scale.log()))
    with pytest.raises(ub.StyleError) as caught:
        log.clim(0, 10)
    assert str(caught.value) == (
        "Axes.clim(): the color limits are 0 and 10; a log color scale needs positive limits."
    )


def test_color_by_is_immutable_and_not_picklable() -> None:
    by = ub.ColorBy([1.0])
    assert copy.copy(by) is by
    assert copy.deepcopy(by) is by
    with pytest.raises(AttributeError):
        by.values = [2.0]
    with pytest.raises(AttributeError):
        del by._native
    assert not hasattr(by, "__dict__")
    with pytest.raises(TypeError) as caught:
        pickle.dumps(by)
    assert str(caught.value) == "ColorBy: a ColorBy cannot be pickled."


def test_quick_scatter_takes_a_color_mapping() -> None:
    by = ub.ColorBy([3.0, 1.0, 2.0], ub.Cmap.CIVIDIS)
    quick = ub.scatter([1, 2, 3], [1, 2, 3], color=by)
    step_by_step = ub.Figure().add_axes().scatter([1, 2, 3], [1, 2, 3], color=by)
    assert quick.figure.to_svg() == step_by_step.figure.to_svg()
    with pytest.raises(ub.UsageError) as caught:
        ub.scatter([1], [1], label="a", color=ub.ColorBy([1]))
    assert str(caught.value).startswith('Axes.scatter(): label is "a"')
