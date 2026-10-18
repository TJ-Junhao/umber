"""Per-call text sizes, sharing, measured layout and argument errors."""

import copy
import inspect
from collections.abc import Callable
from typing import cast
from xml.etree import ElementTree

import pytest

import umber as ub
from umber import _umber
from umber._errors import details


@pytest.mark.parametrize(
    "method",
    [
        "figure_title",
        "figure_subtitle",
        "title",
        "subtitle",
        "xlabel",
        "ylabel",
        "legend",
        "legend_at",
        "figure_colorbar",
        "colorbar",
        "xtick_labels",
        "ytick_labels",
    ],
)
def test_native_text_sizes_are_required(method: str) -> None:
    signature = inspect.signature(getattr(_umber.Figure, method))
    assert signature.parameters["size"].default is inspect.Parameter.empty


def text_sizes(fig: ub.Figure) -> dict[str, float]:
    root = ElementTree.fromstring(fig.to_svg(text=ub.SvgText.EDITABLE))
    return {
        "".join(node.itertext()): float(node.attrib["font-size"])
        for node in root.iter("{http://www.w3.org/2000/svg}text")
    }


def labelled(theme: ub.Theme = ub.Theme.PIGMENT) -> tuple[ub.Figure, ub.Axes[float, float]]:
    fig = ub.Figure(12, 10, theme=theme)
    ax = fig.add_axes().line([0, 1], [0, 1], label="Series")
    ax.xticks_labeled([0, 1], ["X0", "X1"]).yticks_labeled([0, 1], ["Y0", "Y1"])
    return fig, ax


def set_sizes(fig: ub.Figure, ax: ub.Axes[float, float], size: ub.Size) -> None:
    fig.title("Figure title", size=size).subtitle("Figure subtitle", size=size)
    ax.title("Panel title", size=size).subtitle("Panel subtitle", size=size)
    ax.xlabel("Horizontal", size=size).ylabel("Vertical", size=size)
    ax.legend(ub.LegendStyle.HEADER, size=size)
    ax.xtick_labels(size=size).ytick_labels(size=size)


@pytest.mark.parametrize("scale", [0.5, 1, 2])
def test_pt_size_ignores_font_scale(scale: float) -> None:
    fig, ax = labelled(ub.Theme.PIGMENT.with_font_scale(scale))
    set_sizes(fig, ax, ub.Size.pt(12))
    sizes = text_sizes(fig)
    assert len(sizes) == 11
    assert set(sizes.values()) == {12}


@pytest.mark.parametrize("scale", [0.5, 1, 2])
def test_times_size_follows_font_scale(scale: float) -> None:
    base, ax = labelled()
    set_sizes(base, ax, ub.Size.AUTO)
    fig, ax = labelled(ub.Theme.PIGMENT.with_font_scale(scale))
    set_sizes(fig, ax, ub.Size.times(1.5))
    assert text_sizes(fig) == pytest.approx(
        {text: size * scale * 1.5 for text, size in text_sizes(base).items()}
    )


@pytest.mark.parametrize("theme", [ub.Theme.PIGMENT, ub.Theme.GRAPHITE, ub.Theme.MIDNIGHT])
def test_auto_is_unchanged(theme: ub.Theme) -> None:
    fig, ax = labelled(theme)
    fig.title("Figure title").subtitle("Figure subtitle")
    ax.title("Panel title").subtitle("Panel subtitle")
    ax.xlabel("Horizontal").ylabel("Vertical").legend(ub.LegendStyle.HEADER)
    before = (fig.to_svg(), fig.to_png())
    set_sizes(fig, ax, ub.Size.AUTO)
    assert (fig.to_svg(), fig.to_png()) == before
    set_sizes(fig, ax, ub.Size.pt(8))
    fig.title("Figure title").subtitle("Figure subtitle")
    ax.title("Panel title").subtitle("Panel subtitle")
    ax.xlabel("Horizontal").ylabel("Vertical").legend(ub.LegendStyle.HEADER)
    ax.xtick_labels().ytick_labels()
    assert (fig.to_svg(), fig.to_png()) == before


def test_tick_label_sizes_per_axis() -> None:
    fig, ax = labelled()
    base = text_sizes(fig)
    assert ax.xtick_labels(size=ub.Size.pt(9)) is ax
    assert ax.ytick_labels(size=ub.Size.pt(15)) is ax
    sizes = text_sizes(fig)
    assert sizes["X0"] == sizes["X1"] == 9
    assert sizes["Y0"] == sizes["Y1"] == 15
    assert sizes["Series"] == base["Series"]


@pytest.mark.parametrize("style", list(ub.LegendStyle))
def test_legend_sizes_in_each_style(style: ub.LegendStyle) -> None:
    fig, ax = labelled()
    ax.legend(style, size=ub.Size.pt(17))
    sizes = text_sizes(fig)
    if style is ub.LegendStyle.NONE:
        assert "Series" not in sizes
    else:
        assert sizes["Series"] == 17


@pytest.mark.parametrize("position", list(ub.LegendPosition))
def test_legend_sizes_in_each_position(position: ub.LegendPosition) -> None:
    fig, ax = labelled()
    ax.legend(ub.LegendStyle.CHIPS, position=position, size=ub.Size.pt(17))
    assert text_sizes(fig)["Series"] == 17


def test_merged_legend_size_conflict_rejected_atomically() -> None:
    fig = ub.Figure(cols=2)
    left = fig.add_axes(col=0).line([0, 1], [0, 1], label="Left")
    right = fig.add_axes(col=1).line([0, 1], [1, 0], label="Right")
    left.legend(ub.LegendStyle.HEADER, size=ub.Size.pt(8))
    assert text_sizes(fig)["Left"] == text_sizes(fig)["Right"] == 8
    before = fig.to_svg()
    with pytest.raises(ub.UsageError) as caught:
        right.legend(ub.LegendStyle.CHIPS, position=ub.LegendPosition.BOTTOM, size=ub.Size.pt(9))
    assert str(caught.value) == (
        "Axes.legend(): size is Size.pt(9); row 0, col 0 uses Size.pt(8), "
        "and the figure shows one legend."
    )
    assert details(caught.value) == ("size", "set the same size on every panel.")
    assert "size=ub.Size.pt(9)" in caught.value.__notes__[0]
    assert fig.to_svg() == before
    right.legend(ub.LegendStyle.HEADER, size=ub.Size.pt(8))
    left.legend(ub.LegendStyle.HEADER)
    assert fig.to_svg() == before
    right.legend(ub.LegendStyle.HEADER)
    base, _ = labelled()
    assert text_sizes(fig)["Left"] == text_sizes(base)["Series"]


def test_per_panel_legends_can_use_different_sizes() -> None:
    fig = ub.Figure(cols=2, legend=ub.Legend.PER_PANEL)
    fig.add_axes(col=0).line([0, 1], [0, 1], label="Left").legend(
        ub.LegendStyle.HEADER, size=ub.Size.pt(8)
    )
    fig.add_axes(col=1).line([0, 1], [0, 1], label="Right").legend(
        ub.LegendStyle.HEADER, size=ub.Size.pt(14)
    )
    assert text_sizes(fig)["Left"] == 8
    assert text_sizes(fig)["Right"] == 14


@pytest.mark.parametrize("axis", ["x", "y"])
@pytest.mark.parametrize("share", [ub.Share.ROW, ub.Share.COL, ub.Share.ALL])
def test_shared_tick_size_conflict_rejected(axis: str, share: ub.Share) -> None:
    fig = ub.Figure(
        rows=2,
        cols=2,
        share_x=share if axis == "x" else ub.Share.NONE,
        share_y=share if axis == "y" else ub.Share.NONE,
    )
    first = fig.add_axes(row=0, col=0)
    first.xticks_labeled([0], ["X"]).yticks_labeled([0], ["Y"])
    setter = cast(Callable[..., object], getattr(first, f"{axis}tick_labels"))
    setter(size=ub.Size.pt(8))
    # Panels added later inherit the shared size without recording an explicit call.
    second = fig.add_axes(
        row=1 if share is ub.Share.COL else 0, col=0 if share is ub.Share.COL else 1
    )
    other = cast(Callable[..., object], getattr(second, f"{axis}tick_labels"))
    before = fig.to_svg()
    with pytest.raises(ub.UsageError) as caught:
        if axis == "x":
            second.xtick_labels(size=ub.Size.pt(9))
        else:
            second.ytick_labels(size=ub.Size.pt(9))
    assert details(caught.value) == ("size", "set the same size on every panel.")
    assert str(caught.value) == (
        f"Axes.{axis}tick_labels(): size is Size.pt(9); row 0, col 0 uses Size.pt(8), "
        f"and the {axis} axis is shared."
    )
    assert "size=ub.Size.pt(9)" in caught.value.__notes__[0]
    assert "^" in caught.value.__notes__[0]
    assert fig.to_svg() == before
    other(size=ub.Size.pt(8))
    setter(size=ub.Size.AUTO)
    assert fig.to_svg() == before
    other(size=ub.Size.AUTO)
    base, _ = labelled()
    assert text_sizes(fig)[axis.upper()] == text_sizes(base)["X0"]


def test_size_changes_and_figure_copies_are_independent() -> None:
    fig = ub.Figure(cols=2, share_x=ub.Share.ALL)
    ax = fig.add_axes(col=0).xticks_labeled([0], ["X"])
    ax.xtick_labels(size=ub.Size.pt(8)).legend(ub.LegendStyle.HEADER, size=ub.Size.pt(8))
    copied = copy.copy(fig)
    before = copied.to_svg()
    ax.xtick_labels(size=ub.Size.pt(12)).legend(ub.LegendStyle.HEADER, size=ub.Size.pt(12))
    fig.add_axes(col=1)
    assert text_sizes(fig)["X"] == 12
    assert copied.to_svg() == before


@pytest.mark.parametrize("shared", [False, True])
@pytest.mark.parametrize("position", [ub.ColorbarPosition.RIGHT, ub.ColorbarPosition.BOTTOM])
def test_colorbar_size_changes_only_its_title(shared: bool, position: ub.ColorbarPosition) -> None:
    fig = ub.Figure(10, 8, share_color=ub.Share.ALL if shared else ub.Share.NONE)
    ax = fig.add_axes().scatter([0, 1], [0, 1], color=ub.ColorBy([0, 1]))
    call = fig.colorbar if shared else ax.colorbar
    call("Intensity", position=position, format=ub.TickFormat.fixed(1).affix(suffix=" units"))
    before = text_sizes(fig)
    call(
        "Intensity",
        position=position,
        format=ub.TickFormat.fixed(1).affix(suffix=" units"),
        size=ub.Size.pt(20),
    )
    after = text_sizes(fig)
    assert after.pop("Intensity") == 20
    before.pop("Intensity")
    assert after == before


@pytest.mark.parametrize("first_auto", [False, True])
@pytest.mark.parametrize("style", list(ub.LegendStyle))
def test_merged_legend_auto_and_none_do_not_set_size(
    first_auto: bool, style: ub.LegendStyle
) -> None:
    fig = ub.Figure(cols=2)
    left = fig.add_axes(col=0).line([0, 1], [0, 1], label="Left")
    right = fig.add_axes(col=1).line([0, 1], [1, 0], label="Right")
    if first_auto:
        left.legend(style)
    right.legend(ub.LegendStyle.HEADER, size=ub.Size.pt(12))
    left.legend(style)
    assert text_sizes(fig)["Right"] == 12
    left.legend(ub.LegendStyle.NONE, size=ub.Size.pt(30))
    assert text_sizes(fig)["Right"] == 12
    right.legend(ub.LegendStyle.HEADER)
    base, _ = labelled()
    assert text_sizes(fig)["Right"] == text_sizes(base)["Series"]
    left.legend(ub.LegendStyle.HEADER)
    assert text_sizes(fig)["Left"] == text_sizes(base)["Series"]


@pytest.mark.parametrize("axis", ["x", "y"])
def test_shared_tick_auto_does_not_set_size(axis: str) -> None:
    fig = ub.Figure(cols=2, share_x=ub.Share.ALL, share_y=ub.Share.ALL)
    left = fig.add_axes(col=0).xticks_labeled([0], ["X"]).yticks_labeled([0], ["Y"])
    right = fig.add_axes(col=1)
    first = cast(Callable[..., object], getattr(left, f"{axis}tick_labels"))
    second = cast(Callable[..., object], getattr(right, f"{axis}tick_labels"))
    first()
    second(size=ub.Size.pt(30))
    first()
    assert text_sizes(fig)[axis.upper()] == 30
    second()
    base, _ = labelled()
    assert text_sizes(fig)[axis.upper()] == text_sizes(base)["X0"]
    first(size=ub.Size.times(4))
    second()
    assert text_sizes(fig)[axis.upper()] == 4 * text_sizes(base)["X0"]


@pytest.mark.parametrize(
    ("style", "position", "size", "parameter"),
    [
        (None, None, None, "style"),
        (ub.LegendStyle.HEADER, None, None, "position"),
        (ub.LegendStyle.HEADER, ub.AUTO, None, "size"),
    ],
)
def test_legend_checks_types_in_signature_order(
    style: object, position: object, size: object, parameter: str
) -> None:
    fig, ax = labelled()
    before = fig.to_svg()
    call = cast(Callable[..., object], ax.legend)
    with pytest.raises(TypeError) as caught:
        call(style, position=position, size=size)
    found = details(caught.value)
    assert found is not None and found[0] == parameter
    assert fig.to_svg() == before


@pytest.mark.parametrize(
    ("case", "message", "hint"),
    [
        (
            "grid-title",
            "row 0, col 0: the title is 530 pt wide; the panel has 108 pt.",
            "shorten the title or widen the figure.",
        ),
        (
            "xlabel",
            'the x axis label "Horizontal" needs 1109 pt vertically; only 91 pt is available.',
            "make the figure taller, or use a smaller text size.",
        ),
        (
            "rounded",
            'the x axis label "Day" needs 22 pt vertically; only 0 pt is available.',
            "make the figure taller, or use a smaller text size.",
        ),
        (
            "title",
            'the title "Quarterly revenue" needs 75 pt vertically; only 69 pt is available.',
            "make the figure taller, or use a smaller text size.",
        ),
        (
            "small-xlabel",
            'the title "Title" needs 19 pt vertically; only 0 pt is available.',
            "make the figure taller, or use a smaller text size.",
        ),
        (
            "grid-height",
            "the figure is 72 pt tall; at least 288 pt is needed for 2 rows of plots.",
            "make the figure taller, use fewer rows, use smaller text, "
            "or remove titles or legends.",
        ),
        (
            "grid-width",
            "the figure is 57 pt wide; at least 131 pt is needed for 3 columns of plots.",
            "widen the figure, use fewer columns, use smaller text, or remove titles or legends.",
        ),
    ],
)
def test_large_text_room_errors(case: str, message: str, hint: str) -> None:
    if case == "grid-title":
        fig = ub.Figure(6, 4, rows=2, cols=3)
        for cell in fig.cells():
            cell.add_axes().title("Large panel title", size=ub.Size.pt(72))
    elif case == "xlabel":
        fig = ub.Figure(6, 4, theme=ub.Theme.PIGMENT.with_font_scale(10))
        fig.add_axes().xlabel("Horizontal", size=ub.Size.times(10))
    elif case == "rounded":
        fig = ub.Figure(6, 0.6)
        fig.add_axes().xlabel("Day", size=ub.Size.pt(13.3))
    elif case == "title":
        fig = ub.Figure(6, 1.9)
        fig.add_axes().title("Quarterly revenue", size=ub.Size.pt(60)).ylabel(
            "y", size=ub.Size.pt(9)
        )
    elif case == "small-xlabel":
        fig = ub.Figure(6, 0.75)
        fig.add_axes().title("Title").xlabel("x", size=ub.Size.pt(6))
    elif case == "grid-height":
        fig = ub.Figure(6, 1, rows=2, cols=3)
        for cell in fig.cells():
            cell.add_axes().title("T", size=ub.Size.pt(72))
    else:
        fig = ub.Figure(0.8, 4, cols=3)
        for cell in fig.cells():
            cell.add_axes()
    with pytest.raises(ub.StyleError) as caught:
        fig.to_svg()
    assert str(caught.value) == "Figure.to_svg(): " + message
    assert details(caught.value) == (None, hint)


@pytest.mark.parametrize("xlabel_order", ["auto", "before-title", "after-title"])
def test_figure_title_room_error_is_independent_of_xlabel_size(xlabel_order: str) -> None:
    fig = ub.Figure(6, 0.6)
    ax = fig.add_axes().xlabel("x")
    if xlabel_order == "before-title":
        ax.xlabel("x", size=ub.Size.pt(8.5))
    fig.title("Title", size=ub.Size.pt(72))
    if xlabel_order == "after-title":
        ax.xlabel("x", size=ub.Size.pt(8.5))

    with pytest.raises(ub.StyleError) as caught:
        fig.to_svg()
    assert str(caught.value) == (
        "Figure.to_svg(): the figure header needs 98 pt vertically; only 0 pt is available."
    )
    assert details(caught.value) == (
        None,
        "make the figure taller, or use a smaller text size.",
    )


@pytest.mark.parametrize(
    "method",
    [
        "title",
        "subtitle",
        "xlabel",
        "ylabel",
        "legend",
        "xtick_labels",
        "ytick_labels",
        "colorbar",
        "figure_title",
        "figure_subtitle",
        "figure_colorbar",
    ],
)
@pytest.mark.parametrize("value", [None, 12, True, "large", ub.AUTO])
def test_size_type_errors_are_atomic(method: str, value: object) -> None:
    fig = ub.Figure(12, 10, share_color=ub.Share.ALL)
    ax = fig.add_axes()
    target: object = fig if method.startswith("figure_") else ax
    name = method.removeprefix("figure_")
    call = cast(Callable[..., object], getattr(target, name))
    args: tuple[object, ...] = (
        (ub.LegendStyle.HEADER,)
        if name == "legend"
        else ()
        if name in ("xtick_labels", "ytick_labels", "colorbar")
        else ("Changed",)
    )
    before = fig.to_svg()
    with pytest.raises(TypeError) as caught:
        call(*args, size=value)
    found = details(caught.value)
    assert found == (
        "size",
        "use Size.times(1.5) for a multiple of the theme's size, "
        "Size.pt(12) for points, or Size.AUTO.",
    )
    assert fig.to_svg() == before
