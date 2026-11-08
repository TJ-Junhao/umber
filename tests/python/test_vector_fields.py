import datetime as dt
from collections.abc import Callable
from typing import Any, cast
from xml.etree import ElementTree

import numpy as np
import pandas as pd
import pytest

import umber as ub
from umber._errors import details

SVG = "{http://www.w3.org/2000/svg}"
X = [0.0, 1.0, 2.0]
Y = [0.0, 1.0]
U = [[1.0, 2.0, 3.0], [0.0, 1.0, 2.0]]
V = [[1.0, 0.0, 1.0], [2.0, 1.0, 0.0]]
HOURS = [dt.datetime(2026, 1, 1, hour) for hour in (0, 6, 12)]


def axes() -> ub.Axes[float, float]:
    return ub.Figure(6, 4).add_axes()


def call(function: object) -> Callable[..., object]:
    return cast(Callable[..., object], function)


def parameter_of(error: BaseException) -> str | None:
    found = details(error)
    assert found is not None
    return found[0]


def hint_of(error: BaseException) -> str:
    found = details(error)
    assert found is not None and found[1] is not None
    return found[1]


def paths(ax: ub.Axes[Any, Any]) -> list[ElementTree.Element]:
    root = ElementTree.fromstring(ax.figure.to_svg(text=ub.SvgText.EDITABLE))
    return list(root.iter(f"{SVG}path"))


def test_quiver_returns_the_axes_in_both_forms() -> None:
    ax = axes()
    assert ax.quiver(X, Y, U, V) is ax
    assert ax.quiver(X, X, X, X) is ax
    assert ax.barbs(X, Y, np.array(U), np.array(V)) is ax
    assert ax.barbs(X, X, X, X) is ax
    ax.figure.to_svg()


def test_lists_arrays_and_frames_match() -> None:
    inputs: list[tuple[object, object]] = [
        (U, V),
        ([tuple(row) for row in U], [tuple(row) for row in V]),
        (np.array(U), np.array(V)),
        (np.array(U, dtype=np.float32), np.array(V, dtype=np.float32)),
        (pd.DataFrame(U), pd.DataFrame(V)),
    ]
    outputs = {axes().quiver(X, Y, u, v).figure.to_svg() for u, v in inputs}
    assert len(outputs) == 1


def test_arrays_of_meshgrid_are_refused_with_a_hint() -> None:
    grid_x, grid_y = np.meshgrid(X, Y)
    for name, x, y in (("x", grid_x, Y), ("y", X, grid_y)):
        with pytest.raises(ub.DataError, match="coordinates must be one-dimensional") as raised:
            axes().quiver(x, y, U, V)
        assert parameter_of(raised.value) == name
        assert "np.meshgrid()" in hint_of(raised.value)
    with pytest.raises(ub.DataError, match="coordinates must be one-dimensional"):
        call(axes().barbs)([[0.0, 1.0, 2.0]], Y, U, V)


def test_the_shape_of_u_decides_and_wrong_shapes_are_refused() -> None:
    with pytest.raises(ub.DataError, match="u has 2 dimensions and v has 1") as raised:
        axes().quiver(X, Y, U, [1.0, 2.0, 3.0])
    assert parameter_of(raised.value) == "u"
    with pytest.raises(ub.DataError, match="u has 3 dimensions"):
        axes().quiver(X, Y, np.ones((2, 3, 1)), np.ones((2, 3, 1)))
    with pytest.raises(ub.DataError, match=r"\(len\(y\), len\(x\)\)") as raised:
        axes().quiver(X, Y, np.ones((3, 2)), np.ones((3, 2)))
    assert parameter_of(raised.value) == "u"
    with pytest.raises(ub.DataError, match="the same length"):
        axes().quiver(X, Y, [1.0, 2.0, 3.0], [1.0, 2.0, 3.0])
    with pytest.raises(ub.DataError, match="strictly increasing") as raised:
        axes().quiver([0.0, 2.0, 1.0], Y, U, V)
    assert parameter_of(raised.value) == "x"


def test_a_grid_refuses_a_nan_coordinate_as_a_contour_does() -> None:
    for method in (axes().quiver, axes().barbs):
        with pytest.raises(ub.DataError, match="coordinates must be finite") as raised:
            method([0.0, np.nan, 2.0], Y, U, V)
        assert parameter_of(raised.value) == "x"
    with pytest.raises(ub.DataError, match="coordinates must be finite") as contour:
        axes().contour([0.0, np.nan, 2.0], Y, U)
    assert hint_of(contour.value) == "pass finite coordinates within the supported numeric range."


def test_a_speed_beyond_every_flag_is_refused() -> None:
    axes().barbs([0.0], [0.0], [0.0], [-3247.4])
    with pytest.raises(ub.DataError, match="barbs show speeds up to 3245") as raised:
        axes().barbs([0.0], [0.0], [0.0], [-3250.0])
    assert parameter_of(raised.value) == "u"
    assert "3250" in str(raised.value)


def test_data_arrows_follow_the_final_axes() -> None:
    ax = axes().quiver([0, 1], [0, 1], [1, 1], [-1, 1], direction=ub.Direction.DATA)
    ax.figure.to_svg()
    wide = ub.Figure(10, 2).add_axes()
    wide.quiver([0, 1], [0, 100], [1, 1], [1, 1], direction=ub.Direction.DATA, pt_per_unit=200)
    wide.figure.to_svg()


def test_tiny_vectors_keep_their_length() -> None:
    tiny = axes().quiver([0, 1], [0, 1], [1e-307, 1e-307], [0, 0]).figure.to_svg()
    assert tiny == axes().quiver([0, 1], [0, 1], [1.0, 1.0], [0, 0]).figure.to_svg()


def test_points_may_come_in_any_order_but_a_grid_may_not() -> None:
    axes().quiver([2.0, 0.0, 1.0], [0.0, 0.0, 1.0], [1.0, 1.0, 1.0], [0.0, 0.0, 0.0])


def test_wrong_argument_types_are_refused() -> None:
    ax = axes()
    with pytest.raises(TypeError, match="pivot"):
        call(ax.quiver)(X, Y, U, V, pivot="tail")
    with pytest.raises(TypeError, match="direction"):
        call(ax.quiver)(X, Y, U, V, direction="data")
    with pytest.raises(TypeError, match="pt_per_unit"):
        call(ax.quiver)(X, Y, U, V, pt_per_unit="3")
    with pytest.raises(TypeError, match="key"):
        call(ax.quiver)(X, Y, U, V, key=10)
    with pytest.raises(TypeError, match="key"):
        call(ax.quiver)(X, Y, U, V, key=(10, 20))
    with pytest.raises(TypeError, match="key"):
        call(ax.quiver)(X, Y, U, V, key=("10", "10 m/s"))
    with pytest.raises(TypeError, match="color"):
        call(ax.quiver)(X, Y, U, V, color="red")
    with pytest.raises(TypeError, match="flip"):
        call(ax.barbs)(X, Y, U, V, flip="yes")
    with pytest.raises(TypeError, match="label"):
        call(ax.barbs)(X, Y, U, V, label=3)
    with pytest.raises(TypeError, match="size"):
        call(ax.barbs)(X, Y, U, V, size=24)
    with pytest.raises(TypeError, match="u"):
        call(ax.quiver)(X, Y, "u", V)


def test_invalid_values_are_refused_at_the_call() -> None:
    with pytest.raises(ub.StyleError, match="greater than 0") as raised:
        axes().quiver(X, Y, U, V, pt_per_unit=0)
    assert parameter_of(raised.value) == "pt_per_unit"
    with pytest.raises(ub.StyleError, match="at most 14400 pt long"):
        axes().quiver(X, Y, U, V, pt_per_unit=1e9)
    with pytest.raises(ub.StyleError, match="greater than 0") as raised:
        axes().quiver(X, Y, U, V, key=(0, "none"))
    assert parameter_of(raised.value) == "key"
    with pytest.raises(ub.StyleError, match="the key text is empty"):
        axes().quiver(X, Y, U, V, key=(1, ""))
    with pytest.raises(ub.StyleError, match="one line"):
        axes().quiver(X, Y, U, V, key=(1, "two\nlines"))
    with pytest.raises(ub.DataError, match="values must be finite") as infinite:
        axes().quiver(X, Y, [[1.0, 2.0, 3.0], [0.0, np.inf, 2.0]], V)
    assert parameter_of(infinite.value) == "u"
    assert "u[1][1]" in str(infinite.value)


def test_nan_leaves_arrows_out() -> None:
    whole = axes().quiver([0.0, 1.0], [0.0, 1.0], [1.0, 1.0], [0.0, 0.0], pt_per_unit=9)
    holed = axes().quiver(
        [0.0, 1.0, 50.0, np.nan],
        [0.0, 1.0, 50.0, 3.0],
        [1.0, 1.0, np.nan, 1.0],
        [0.0, 0.0, 1.0, 0.0],
        pt_per_unit=9,
    )
    assert whole.figure.to_svg() == holed.figure.to_svg()


def test_one_item_per_color() -> None:
    ax = axes().xlim(-1, 3).ylim(-1, 2).quiver(X, Y, U, V)
    fills = [path for path in paths(ax) if path.get("fill-rule") is None]
    assert len(fills) >= 1
    mapped = axes().quiver(X, Y, U, V, color=ub.ColorBy(np.array(U)))
    assert len(paths(mapped)) >= len(fills)


def test_a_color_mapping_takes_the_shape_of_u() -> None:
    speed = np.hypot(U, V)
    ax = axes().quiver(X, Y, U, V, color=ub.ColorBy(speed)).colorbar("Speed")
    assert "linearGradient" in ax.figure.to_svg()
    axes().barbs(X, Y, U, V, color=ub.ColorBy([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]))
    axes().quiver(X, X, X, X, color=ub.ColorBy(X))
    with pytest.raises(ub.DataError, match=r"color has shape \(6,\)") as raised:
        axes().quiver(X, Y, U, V, color=ub.ColorBy(speed.ravel()))
    assert parameter_of(raised.value) == "color"
    with pytest.raises(ub.DataError, match=r"color has shape \(2, 3\) but u has shape \(3,\)"):
        axes().quiver(X, X, X, X, color=ub.ColorBy(speed))
    with pytest.raises(ub.DataError, match="must be one-dimensional"):
        axes().scatter(X, X, color=ub.ColorBy(speed))
    with pytest.raises(ub.DataError, match="must be one-dimensional"):
        axes().bubbles(X, X, X, color=ub.ColorBy(speed))


def test_a_ragged_color_mapping_is_refused() -> None:
    with pytest.raises(ub.DataError):
        ub.ColorBy([[1.0, 2.0], [3.0]])


def test_directions_and_pivots_change_the_drawing() -> None:
    wide = {
        direction: ub.Figure(8, 3).add_axes().quiver(X, Y, U, V, direction=direction)
        for direction in ub.Direction
    }
    assert wide[ub.Direction.SCREEN].figure.to_svg() != wide[ub.Direction.DATA].figure.to_svg()
    pivots = {axes().quiver(X, Y, U, V, pivot=pivot).figure.to_svg() for pivot in ub.Pivot}
    assert len(pivots) == 3


def test_both_directions_agree_on_equal_aspect() -> None:
    drawn = {
        ub.Figure(8, 3)
        .add_axes()
        .equal_aspect()
        .quiver(X, Y, U, V, direction=direction)
        .figure.to_svg()
        for direction in ub.Direction
    }
    assert len(drawn) == 1


def test_the_data_direction_needs_linear_axes() -> None:
    log = ub.Figure().add_axes(y=ub.Scale.log())
    with pytest.raises(ub.UsageError, match="a log or time axis") as raised:
        log.quiver(X, [1.0, 10.0], U, V, direction=ub.Direction.DATA)
    assert parameter_of(raised.value) == "direction"
    timed = ub.Figure().add_axes(x=ub.Scale.time())
    with pytest.raises(ub.UsageError, match="a log or time axis"):
        timed.quiver(HOURS, Y, U, V, direction=ub.Direction.DATA)


def test_log_and_time_axes_take_arrows() -> None:
    log = ub.Figure().add_axes(x=ub.Scale.log()).quiver([1.0, 10.0, 100.0], Y, U, V)
    log.figure.to_svg()
    timed = ub.Figure().add_axes(x=ub.Scale.time()).quiver(HOURS, Y, U, V, key=(1, "1 m/s"))
    timed.figure.to_svg()


def test_a_category_axis_is_refused() -> None:
    ax = cast(Any, ub.Figure().add_axes(x=ub.Scale.category()))
    for method in (ax.quiver, ax.barbs):
        with pytest.raises(ub.UsageError, match="must be a numeric or time axis"):
            call(method)(X, Y, U, V)


def test_a_key_is_a_legend_entry_in_every_position() -> None:
    positions = (
        ub.LegendPosition.TOP_LEFT,
        ub.LegendPosition.TOP_RIGHT,
        ub.LegendPosition.RIGHT,
        ub.LegendPosition.BOTTOM,
        ub.LegendPosition.BEST,
    )
    for position in positions:
        ax = axes().quiver(X, Y, U, V, key=(2, "2 m/s"))
        ax = ax.legend(ub.LegendStyle.HEADER, position=position)
        assert "2 m/s" in ax.figure.to_svg(text=ub.SvgText.EDITABLE)
    direct = axes().quiver(X, Y, U, V, key=(2, "2 m/s")).legend(ub.LegendStyle.DIRECT)
    assert "2 m/s" in direct.figure.to_svg(text=ub.SvgText.EDITABLE)


def test_a_long_key_and_a_key_without_a_scale_are_refused_when_rendering() -> None:
    long = axes().quiver(X, Y, U, V, pt_per_unit=2, key=(1000, "1000 m/s"))
    with pytest.raises(ub.StyleError, match="the shorter side of the plot") as raised:
        long.figure.to_svg()
    assert "pass a smaller value to key" in hint_of(raised.value)
    still = axes().quiver(X, Y, [[0.0] * 3] * 2, [[0.0] * 3] * 2, key=(1, "1 m/s"))
    with pytest.raises(ub.StyleError, match="has no scale"):
        still.figure.to_svg()


def test_arrows_too_long_for_automatic_limits_are_refused_when_rendering() -> None:
    ax = axes().quiver([0.0, 1.0], [0.0, 1.0], [0.0, 0.0], [10.0, 10.0], pt_per_unit=40)
    with pytest.raises(ub.StyleError, match="the arrows need a figure at least"):
        ax.figure.to_svg()
    clipped = axes().xlim(0, 1).ylim(0, 1)
    clipped.quiver([0.0, 1.0], [0.0, 1.0], [0.0, 0.0], [10.0, 10.0], pt_per_unit=40)
    clipped.figure.to_svg()


def test_barbs_on_a_time_height_section_in_every_format() -> None:
    heights = [500.0, 1000.0, 2000.0]
    speed = np.linspace(0.0, 80.0, 9).reshape(3, 3)
    ax = ub.Figure(6, 4).add_axes(x=ub.Scale.time())
    ax = ax.barbs(HOURS, heights, speed, -0.5 * speed, label="Wind (kn)")
    assert "Wind (kn)" in ax.figure.to_svg(text=ub.SvgText.EDITABLE)
    assert ax.figure.to_png(dpi=72).startswith(b"\x89PNG")
    assert ax.figure.to_pdf().startswith(b"%PDF")


def test_barb_options_change_the_drawing() -> None:
    wind = [0.0, 25.0, 65.0]
    plain = axes().barbs(X, X, wind, wind).figure.to_svg()
    assert axes().barbs(X, X, wind, wind, flip=True).figure.to_svg() != plain
    assert axes().barbs(X, X, wind, wind, size=ub.Size.pt(30)).figure.to_svg() != plain
    assert axes().barbs(X, X, wind, wind, size=ub.Size.times(1.0)).figure.to_svg() == plain


def test_the_same_figure_draws_the_same_bytes() -> None:
    def drawn() -> bytes:
        return axes().quiver(X, Y, U, V, key=(2, "2 m/s")).figure.to_png(dpi=72)

    assert drawn() == drawn()
