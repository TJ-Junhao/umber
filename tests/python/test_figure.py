# pyright: reportPrivateUsage=false
import copy
import os
import pickle
import struct
import traceback
import typing
from collections.abc import Callable
from pathlib import Path
from typing import Any, cast

import numpy as np
import pytest

import umber as ub
from umber._errors import details

SIZE_RULE = "it must be finite, above 0 and at most 200 inches."
THEME_HINT = "use Theme.PIGMENT, Theme.GRAPHITE or Theme.MIDNIGHT."
SVG_TEXT_HINT = "use SvgText.OUTLINES or SvgText.EDITABLE."
CATEGORY_Y = "Figure.add_axes(): y is a category scale; the y axis must be linear or log."
CATEGORY_Y_HINT = "pass y=Scale.linear() or y=Scale.log()."
SECOND_AXES = "Figure.add_axes(): row 0, col 0 is taken by the axes added at row 0, col 0."
SECOND_AXES_HINT = "each cell holds one axes; pick an empty cell."


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


def _png_size(png: bytes) -> tuple[int, int]:
    """The pixel width and height in a PNG's IHDR chunk."""
    assert png[:8] == b"\x89PNG\r\n\x1a\n"
    assert png[12:16] == b"IHDR"
    width, height = struct.unpack(">II", png[16:24])
    return width, height


def _line_figure() -> ub.Figure:
    fig = ub.Figure(3, 2)
    fig.add_axes().line([0, 1, 2], [1.0, 3.0, 2.0], label="North").title("Signal")
    return fig


# Construction


def test_defaults() -> None:
    fig = ub.Figure()
    assert _png_size(fig.to_png(dpi=10)) == (60, 40)
    assert fig.to_svg() == ub.Figure(6.0, 4.0, theme=ub.Theme.PIGMENT).to_svg()


def test_size_and_theme_are_used() -> None:
    assert _png_size(ub.Figure(3, 2.5).to_png(dpi=100)) == (300, 250)
    assert _png_size(
        ub.Figure(cast(float, np.float32(3)), cast(float, np.int64(2))).to_png(dpi=100)
    ) == (300, 200)
    pigment = ub.Figure(3, 2).to_svg()
    midnight = ub.Figure(3, 2, theme=ub.Theme.MIDNIGHT).to_svg()
    assert pigment != midnight
    custom = ub.Theme.PIGMENT.with_background(ub.Color.BLUE)
    assert 'fill="#3366B8"' in ub.Figure(3, 2, theme=custom).to_svg()


def test_each_figure_is_new() -> None:
    first, second = ub.Figure(), ub.Figure()
    first.add_axes().title("First")
    assert first is not second
    second.add_axes()  # the other figure's axes are not shared


@pytest.mark.parametrize(
    ("width", "height", "parameter", "shown"),
    [(0, 4, "width", "0"), (201.0, 4, "width", "201"), (4, float("inf"), "height", "inf")],
)
def test_size_out_of_range(width: float, height: float, parameter: str, shown: str) -> None:
    with pytest.raises(ub.StyleError) as caught:
        ub.Figure(width, height)
    assert str(caught.value) == f"Figure(): {parameter} is {shown}; {SIZE_RULE}"
    assert details(caught.value) == (parameter, None)
    assert _frames(caught.value) == ["test_size_out_of_range"]


@pytest.mark.parametrize(
    ("width", "height", "name", "shown"),
    [("6", 4, "width", "str"), (6, None, "height", "None"), (True, 4, "width", "bool")],
)
def test_size_must_be_a_number(width: object, height: object, name: str, shown: str) -> None:
    with pytest.raises(TypeError) as caught:
        ub.Figure(cast(float, width), cast(float, height))
    assert str(caught.value) == f"Figure(): {name} is {shown}; {name} must be a float."
    assert details(caught.value) == (name, "pass a number such as 1.5.")


@pytest.mark.parametrize("theme", ["midnight", None, ub.Color.BLUE])
def test_theme_must_be_a_theme(theme: object) -> None:
    with pytest.raises(TypeError) as caught:
        ub.Figure(theme=cast(ub.Theme, theme))
    shown = "None" if theme is None else type(theme).__name__
    assert str(caught.value) == f"Figure(): theme is {shown}; theme must be of type Theme."
    assert details(caught.value) == ("theme", THEME_HINT)


def test_construction_errors_highlight_the_argument(tmp_path: Path) -> None:
    line = "ub.Figure(6, -4)"
    err = _raised(tmp_path, line)
    assert isinstance(err, ub.StyleError)
    assert _notes(err) == [_marked(line, "-4")]
    line = 'ub.Figure(theme="midnight")'
    err = _raised(tmp_path, line)
    assert isinstance(err, TypeError)
    assert _notes(err) == [_marked(line, 'theme="midnight"'), f"hint: {THEME_HINT}"]


# Axes


def test_add_axes_returns_axes_of_this_figure() -> None:
    fig = ub.Figure()
    ax = fig.add_axes()
    assert isinstance(ax, ub.Axes)
    assert ax.figure is fig
    assert fig._axes is ax


def test_add_axes_passes_the_scales() -> None:
    from umber._axes import scales

    fig = ub.Figure()
    ax = fig.add_axes(x=ub.Scale.log(2), y=ub.Scale.log())
    assert scales(ax) == (ub.Scale.log(2), ub.Scale.log())
    assert fig.to_svg() != ub.Figure().to_svg()
    ax.line([1, 10, 100], [1, 10, 100])  # positive data on log axes is accepted
    with pytest.raises(ub.DataError):
        ax.line([1, 2], [0, 1])


def test_a_second_axes_is_refused() -> None:
    fig = ub.Figure()
    ax = fig.add_axes().title("Kept")
    before = fig.to_svg()
    with pytest.raises(ub.UsageError) as caught:
        fig.add_axes(x=ub.Scale.category())
    assert str(caught.value) == SECOND_AXES
    assert details(caught.value) == ("row", SECOND_AXES_HINT)
    assert fig._axes is ax
    assert fig.to_svg() == before


def test_a_category_y_axis_is_refused() -> None:
    fig = ub.Figure()
    before = fig.to_svg()
    with pytest.raises(ub.UsageError) as caught:
        fig.add_axes(x=ub.Scale.category(), y=cast(ub.Scale[float], ub.Scale.category()))
    assert str(caught.value) == CATEGORY_Y
    assert details(caught.value) == ("y", CATEGORY_Y_HINT)
    assert fig._axes is None
    assert fig.to_svg() == before
    fig.add_axes()  # the failed call added nothing


def test_a_category_y_is_reported_before_a_second_axes() -> None:
    fig = ub.Figure()
    fig.add_axes()
    with pytest.raises(ub.UsageError) as caught:
        fig.add_axes(y=cast(ub.Scale[float], ub.Scale.category()))
    assert str(caught.value) == CATEGORY_Y


def test_category_y_error_highlights_y(tmp_path: Path) -> None:
    line = "ub.Figure().add_axes(y=ub.Scale.category())"
    err = _raised(tmp_path, line)
    assert isinstance(err, ub.UsageError)
    assert _notes(err) == [_marked(line, "y=ub.Scale.category()"), f"hint: {CATEGORY_Y_HINT}"]


@pytest.mark.parametrize(
    ("name", "value", "hint"),
    [
        ("x", "log", "use Scale.linear(), Scale.log() or Scale.category()."),
        ("y", None, "use Scale.linear() or Scale.log()."),
    ],
)
def test_scales_must_be_scales(name: str, value: object, hint: str) -> None:
    fig = ub.Figure()
    with pytest.raises(TypeError) as caught:
        fig.add_axes(**{name: cast(ub.Scale[float], value)})
    shown = "None" if value is None else type(value).__name__
    assert (
        str(caught.value) == f"Figure.add_axes(): {name} is {shown}; {name} must be of type Scale."
    )
    assert details(caught.value) == (name, hint)
    assert fig._axes is None


# The axes property

NO_AXES = "Figure.axes: the figure has no axes; call add_axes() first."
NO_AXES_HINT = "call fig.add_axes() before drawing."


def test_axes_property_returns_the_axes() -> None:
    fig = ub.Figure()
    ax = fig.add_axes(x=ub.Scale.category())
    assert fig.axes is ax
    assert fig.axes.figure is fig


def test_axes_property_without_axes_is_a_usage_error() -> None:
    with pytest.raises(ub.UsageError) as caught:
        _ = ub.Figure().axes
    assert str(caught.value) == NO_AXES
    assert details(caught.value) == (None, NO_AXES_HINT)
    assert _notes(caught.value) == [f"hint: {NO_AXES_HINT}"]
    assert _frames(caught.value) == ["test_axes_property_without_axes_is_a_usage_error"]


def test_axes_property_error_from_user_code(tmp_path: Path) -> None:
    err = _raised(tmp_path, "ub.Figure().axes.title('T')")
    assert isinstance(err, ub.UsageError)
    assert _notes(err) == [f"hint: {NO_AXES_HINT}"]


def test_axes_property_is_read_only() -> None:
    fig = ub.Figure()
    ax = fig.add_axes()
    with pytest.raises(AttributeError):
        fig.axes = ax  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue]
    with pytest.raises(AttributeError):
        del fig.axes  # pyright: ignore[reportAttributeAccessIssue]
    assert fig.axes is ax


def test_axes_of_a_copy_draw_on_the_copy_only() -> None:
    fig = ub.Figure(3, 2)
    fig.add_axes().line([0, 1, 2], [1, 3, 2])
    before = fig.to_svg()
    variant = copy.copy(fig)
    variant.axes.line([0, 1, 2], [2, 1, 3], color=ub.Color.RED)
    assert variant.axes is not fig.axes
    assert variant.axes.figure is variant
    assert fig.to_svg() == before
    assert variant.to_svg() != before
    deep = copy.deepcopy(fig)
    deep.axes.title("Deep")
    assert fig.to_svg() == before


def test_axes_property_type_hints_resolve() -> None:
    getter = cast(property, vars(ub.Figure)["axes"]).fget
    assert getter is not None
    assert getter.__module__ == "umber"
    assert typing.get_type_hints(getter)["return"] == ub.Axes[typing.Any, typing.Any]


# Rendering


def test_png_size_follows_dpi() -> None:
    fig = _line_figure()
    assert _png_size(fig.to_png(dpi=50)) == (150, 100)
    assert _png_size(fig.to_png()) == (600, 400)
    assert _png_size(fig.to_png(dpi=cast(int, np.int32(30)))) == (90, 60)


def test_png_dpi_out_of_range() -> None:
    fig = _line_figure()
    with pytest.raises(ub.StyleError) as caught:
        fig.to_png(dpi=5)
    assert str(caught.value) == (
        "Figure.to_png(): dpi 5 is out of range; use a value from 10 to 2400."
    )
    assert details(caught.value) == ("dpi", None)


@pytest.mark.parametrize("dpi", [100.0, True, "100", None])
def test_png_dpi_must_be_an_int(dpi: object) -> None:
    with pytest.raises(TypeError) as caught:
        ub.Figure().to_png(dpi=cast(int, dpi))
    shown = "None" if dpi is None else type(dpi).__name__
    assert str(caught.value) == f"Figure.to_png(): dpi is {shown}; dpi must be an int."
    assert details(caught.value) == ("dpi", "pass a whole number such as 3.")


def test_png_dpi_error_highlights_dpi(tmp_path: Path) -> None:
    line = "ub.Figure().to_png(dpi=1)"
    err = _raised(tmp_path, line)
    assert isinstance(err, ub.StyleError)
    assert _notes(err) == [_marked(line, "dpi=1")]


def test_svg_text_modes() -> None:
    fig = _line_figure()
    assert fig.to_svg() == fig.to_svg(text=ub.SvgText.OUTLINES)
    assert ">Signal</text>" in fig.to_svg(text=ub.SvgText.EDITABLE)
    assert ">Signal</text>" not in fig.to_svg()
    assert fig.to_svg().startswith("<svg")


@pytest.mark.parametrize("text", [1, "EDITABLE", ub.Dash.SOLID])
def test_svg_text_must_be_an_svg_text(text: object) -> None:
    with pytest.raises(TypeError) as caught:
        ub.Figure().to_svg(text=cast(ub.SvgText, text))
    assert str(caught.value) == (
        f"Figure.to_svg(): text is {type(text).__name__}; text must be of type SvgText."
    )
    assert details(caught.value) == ("text", SVG_TEXT_HINT)


def test_figure_without_axes_renders_the_background() -> None:
    svg = ub.Figure(3, 2, theme=ub.Theme.MIDNIGHT).to_svg()
    assert 'fill="#121419"' in svg
    assert _png_size(ub.Figure(3, 2).to_png(dpi=10)) == (30, 20)


# Saving


def test_save_png_writes_the_rendered_image(tmp_path: Path) -> None:
    fig = _line_figure()
    fig.save_png(tmp_path / "chart.png", dpi=50)
    assert (tmp_path / "chart.png").read_bytes() == fig.to_png(dpi=50)
    fig.save_png(str(tmp_path / "default.png"))
    assert (tmp_path / "default.png").read_bytes() == fig.to_png()


def test_save_svg_writes_utf8(tmp_path: Path) -> None:
    fig = ub.Figure(3, 2)
    fig.add_axes().title("Température °C")
    fig.save_svg(tmp_path / "chart.svg", text=ub.SvgText.EDITABLE)
    written = (tmp_path / "chart.svg").read_bytes()
    assert written == fig.to_svg(text=ub.SvgText.EDITABLE).encode("utf-8")
    assert "Température °C".encode() in written
    fig.save_svg(os.fspath(tmp_path / "outlines.svg"))
    assert (tmp_path / "outlines.svg").read_text(encoding="utf-8") == fig.to_svg()


def test_save_to_a_missing_directory_raises_the_os_error(tmp_path: Path) -> None:
    target = tmp_path / "missing" / "chart.png"
    for save in (ub.Figure().save_png, ub.Figure().save_svg):
        with pytest.raises(FileNotFoundError) as caught:
            save(target)
        assert caught.value.filename == str(target)
        assert details(caught.value) is None
        assert _notes(caught.value) == []
        # Not an error umber raised on purpose: the full traceback stays.
        assert len(_frames(caught.value)) > 1


@pytest.mark.parametrize("name", ["save_png", "save_svg"])
@pytest.mark.parametrize(
    ("path", "message", "hint"),
    [
        (b"chart.png", "path is bytes; path must be a str or a path-like object.",
         "decode a bytes path with os.fsdecode()."),
        (3, "path is int; path must be a str or a path-like object.",
         "pass a str or a pathlib.Path."),
    ],
)  # fmt: skip
def test_save_path_must_be_a_str_path(name: str, path: object, message: str, hint: str) -> None:
    with pytest.raises(TypeError) as caught:
        getattr(ub.Figure(), name)(path)
    assert str(caught.value) == f"Figure.{name}(): {message}"
    assert details(caught.value) == ("path", hint)


def test_save_png_reports_render_errors_in_its_own_context(tmp_path: Path) -> None:
    with pytest.raises(ub.StyleError) as caught:
        ub.Figure().save_png(tmp_path / "chart.png", dpi=9999)
    assert str(caught.value) == (
        "Figure.save_png(): dpi 9999 is out of range; use a value from 10 to 2400."
    )
    assert details(caught.value) == ("dpi", None)
    assert not (tmp_path / "chart.png").exists()


def test_save_type_checks_follow_parameter_order(tmp_path: Path) -> None:
    with pytest.raises(TypeError) as caught:
        ub.Figure().save_png(cast(str, 3), dpi=cast(int, "x"))
    assert details(caught.value) == ("path", "pass a str or a pathlib.Path.")
    with pytest.raises(TypeError) as caught:
        ub.Figure().save_svg(tmp_path / "chart.svg", text=cast(ub.SvgText, 1))
    assert details(caught.value) == ("text", SVG_TEXT_HINT)
    assert not (tmp_path / "chart.svg").exists()


def test_save_error_highlights_the_path(tmp_path: Path) -> None:
    line = 'ub.Figure().save_png(b"chart.png")'
    err = _raised(tmp_path, line)
    assert isinstance(err, TypeError)
    assert _notes(err) == [
        _marked(line, 'b"chart.png"'),
        "hint: decode a bytes path with os.fsdecode().",
    ]


# Copying and pickling


@pytest.mark.parametrize("duplicate", [copy.copy, copy.deepcopy])
def test_copies_are_independent(duplicate: Callable[[ub.Figure], ub.Figure]) -> None:
    fig = ub.Figure(3, 2)
    ax = fig.add_axes(x=ub.Scale.log()).line([1, 10], [1, 2]).title("Before")
    before = fig.to_svg()
    clone = duplicate(fig)
    assert isinstance(clone, ub.Figure)
    assert clone is not fig
    assert clone.to_svg() == before
    assert clone.to_png(dpi=20) == fig.to_png(dpi=20)

    ax.title("After").scatter([1, 100], [2, 3])
    assert fig.to_svg() != before
    assert clone.to_svg() == before

    clone_axes = clone._axes
    assert clone_axes is not None
    assert clone_axes is not ax
    assert clone_axes.figure is clone
    clone_axes.title("Clone")
    assert fig.to_svg(text=ub.SvgText.EDITABLE).count(">After</text>") == 1
    assert ">Clone</text>" in clone.to_svg(text=ub.SvgText.EDITABLE)

    from umber._axes import scales

    assert scales(clone_axes) == scales(ax)
    with pytest.raises(ub.UsageError):
        clone.add_axes()


@pytest.mark.parametrize("duplicate", [copy.copy, copy.deepcopy])
def test_a_copy_of_a_figure_without_axes_gets_its_own(
    duplicate: Callable[[ub.Figure], ub.Figure],
) -> None:
    fig = ub.Figure(3, 2)
    clone = duplicate(fig)
    assert clone._axes is None
    clone.add_axes().title("Clone")
    assert fig._axes is None
    fig.add_axes()  # the original can still get axes of its own


def test_deepcopy_inside_a_container_keeps_sharing() -> None:
    fig = ub.Figure(3, 2)
    pair = copy.deepcopy([fig, fig])
    assert pair[0] is pair[1]
    assert pair[0] is not fig


@pytest.mark.parametrize("protocol", range(pickle.HIGHEST_PROTOCOL + 1))
def test_pickling_is_refused(protocol: int) -> None:
    fig = _line_figure()
    with pytest.raises(TypeError) as caught:
        pickle.dumps(fig, protocol)
    assert str(caught.value) == "Figure: a Figure cannot be pickled."
    hint = "save it with save_png() or save_svg(), or build it again."
    assert details(caught.value) == (None, hint)
    assert _notes(caught.value) == [f"hint: {hint}"]
    assert _frames(caught.value) == ["test_pickling_is_refused"]


# Structure


def test_figures_have_no_instance_dictionary() -> None:
    fig = ub.Figure()
    assert not hasattr(fig, "__dict__")
    with pytest.raises(AttributeError):
        fig.extra = 1  # type: ignore[attr-defined]  # pyright: ignore[reportAttributeAccessIssue]
    with pytest.raises(AttributeError):
        fig.add_axes = fig.add_axes  # type: ignore[method-assign]


def test_figure_reports_the_package_as_its_module() -> None:
    assert ub.Figure.__module__ == "umber"
    for name in ("add_axes", "to_png", "to_svg", "save_png", "save_svg"):
        assert getattr(ub.Figure, name).__module__ == "umber"


class _RecordsFspath:
    """A path-like object that notes whether its path was read."""

    def __init__(self) -> None:
        self.read = False

    def __fspath__(self) -> str:
        self.read = True
        return "chart.out"


def test_save_checks_other_types_before_reading_the_path() -> None:
    recorder = _RecordsFspath()
    with pytest.raises(TypeError) as caught:
        cast(Any, ub.Figure()).save_png(recorder, dpi="bad")
    found = details(caught.value)
    assert found is not None
    assert found[0] == "dpi"
    with pytest.raises(TypeError) as caught:
        cast(Any, ub.Figure()).save_svg(recorder, text="bad")
    found = details(caught.value)
    assert found is not None
    assert found[0] == "text"
    assert not recorder.read
