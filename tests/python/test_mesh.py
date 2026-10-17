"""Numeric cells, binned counts, validation and color mapping."""

import datetime as dt
import math
import re
import xml.etree.ElementTree as ET
from collections.abc import Callable
from typing import Any, cast

import numpy as np
import pytest

import umber as ub
from umber._errors import details


def axes() -> ub.Axes[float, float]:
    return ub.Figure(6, 4).add_axes()


def call(function: object) -> Callable[..., Any]:
    return cast(Callable[..., Any], function)


@pytest.mark.parametrize("bad", [True, "bad", None, np.inf, 1e301])
@pytest.mark.parametrize("array", [False, True])
def test_matrix_errors_name_row_and_column(bad: object, array: bool) -> None:
    values = [[1, 2], [3, bad]]
    matrix = np.asarray(values, dtype=object) if array else values
    with pytest.raises((TypeError, ub.DataError)) as caught:
        axes().mesh([0, 1, 2], [0, 1, 2], matrix)
    assert "values[1, 1]" in str(caught.value)
    found = details(caught.value)
    assert found is not None and found[0] == "values" and found[1]


@pytest.mark.parametrize("axis", ["x", "y", "both"])
def test_hist2d_log_bins_match_transformed_edges(axis: str) -> None:
    x = np.array([1.0, 4.0, 10.0, 20.0, 70.0, 100.0])
    y = x[::-1]
    xe = np.geomspace(1, 100, 4) if axis != "y" else np.linspace(1, 100, 4)
    ye = np.geomspace(1, 100, 4) if axis != "x" else np.linspace(1, 100, 4)
    counts, _, _ = np.histogram2d(x, y, bins=(xe, ye))
    values = counts.T.copy()
    values[values == 0] = np.nan
    xscale = ub.Scale.log() if axis != "y" else ub.Scale.linear()
    yscale = ub.Scale.log() if axis != "x" else ub.Scale.linear()
    actual = ub.Figure(6, 4).add_axes(x=xscale, y=yscale).hist2d(x, y, bins=3)
    expected = ub.Figure(6, 4).add_axes(x=xscale, y=yscale).mesh(xe, ye, values)
    assert actual.figure.to_svg() == expected.figure.to_svg()


@pytest.mark.parametrize(
    ("rows", "share_x", "share_y", "gridsize", "different"),
    [
        (1, ub.Share.NONE, ub.Share.NONE, 10, False),
        (1, ub.Share.ALL, ub.Share.NONE, 10, False),
        (1, ub.Share.ALL, ub.Share.NONE, 30, False),
        (2, ub.Share.ALL, ub.Share.NONE, 10, True),
        (2, ub.Share.ALL, ub.Share.ALL, 10, False),
    ],
)
def test_dense_hexbin_grid_geometry(
    rows: int, share_x: ub.Share, share_y: ub.Share, gridsize: int, different: bool
) -> None:
    i = np.arange(2000)
    x = (i * 0.6180339887 % 1) * 4 - 2
    y = (i * 0.4142135623 % 1) * 4 - 2 + 0.5 * x
    fig = ub.Figure(10, 6, rows=rows, cols=2, share_x=share_x, share_y=share_y)
    for row in range(rows):
        for col in range(2):
            offset = (2 * row + col) * 0.7 if different else 0.0
            fig.add_axes(row=row, col=col).hexbin(x + offset, y + offset, gridsize=gridsize)
    assert_svg_hexagons_regular_and_whole(fig.to_svg())


Point = tuple[float, float]

SVG = "{http://www.w3.org/2000/svg}"


def edge_distance(polygon: list[Point], p: Point) -> float:
    """The distance from ``p`` to the nearest edge of ``polygon``."""
    best = math.inf
    for i, a in enumerate(polygon):
        b = polygon[(i + 1) % len(polygon)]
        dx, dy = b[0] - a[0], b[1] - a[1]
        t = min(1.0, max(0.0, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / (dx * dx + dy * dy)))
        best = min(best, math.hypot(p[0] - a[0] - t * dx, p[1] - a[1] - t * dy))
    return best


def inside(polygon: list[Point], p: Point) -> bool:
    """Whether ``p`` lies inside ``polygon`` by the nonzero winding rule."""
    winding = 0
    for i, a in enumerate(polygon):
        b = polygon[(i + 1) % len(polygon)]
        side = (b[0] - a[0]) * (p[1] - a[1]) - (p[0] - a[0]) * (b[1] - a[1])
        if a[1] <= p[1] < b[1] and side > 0:
            winding += 1
        elif b[1] <= p[1] < a[1] and side < 0:
            winding -= 1
    return winding != 0


def covers(polygon: list[Point], p: Point, tolerance: float) -> bool:
    """Whether ``p`` lies inside ``polygon`` or within ``tolerance`` of its outline."""
    return inside(polygon, p) or edge_distance(polygon, p) <= tolerance


def samples(polygon: list[Point]) -> list[Point]:
    """The corners of ``polygon`` and points every quarter point along its edges."""
    out: list[Point] = []
    for i, a in enumerate(polygon):
        b = polygon[(i + 1) % len(polygon)]
        steps = max(1, math.ceil(math.dist(a, b) / 0.25))
        out += [
            (a[0] + k / steps * (b[0] - a[0]), a[1] + k / steps * (b[1] - a[1]))
            for k in range(steps)
        ]
    return out


def outline_error(
    outline: list[Point], hexagon: list[Point], later: list[list[Point]], tolerance: float = 0.005
) -> str | None:
    """Why ``outline`` is not a valid drawing of the cell ``hexagon``, or None when it is.

    The outline must cover the whole exact hexagon, and may reach past it only under a cell
    drawn after it (``later``, their exact hexagons) across one of its edges, at most 1 pt deep
    or half that cell when it is smaller.
    """
    for p in samples(hexagon):
        if not covers(outline, p, tolerance):
            return f"{p} of {hexagon} is not covered"
    for p in outline:
        if inside(hexagon, p) and edge_distance(hexagon, p) > tolerance:
            return f"corner {p} lies inside {hexagon}"
    center = (sum(p[0] for p in hexagon) / 6, sum(p[1] for p in hexagon) / 6)

    def line_distance(p: Point, a: Point, b: Point) -> float:
        cross = (b[0] - a[0]) * (p[1] - a[1]) - (p[0] - a[0]) * (b[1] - a[1])
        return abs(cross) / math.dist(a, b)

    strips: list[tuple[list[Point], Point, Point, float]] = []
    for i, a in enumerate(hexagon):
        b = hexagon[(i + 1) % 6]
        depth = min(1.0, line_distance(center, a, b))
        for cell in later:
            if any(math.dist(q, a) < 0.01 for q in cell) and any(
                math.dist(q, b) < 0.01 for q in cell
            ):
                strips.append((cell, a, b, depth))
    for p in samples(outline):
        if not covers(hexagon, p, tolerance) and not any(
            covers(cell, p, tolerance) and line_distance(p, a, b) <= depth + tolerance
            for cell, a, b, depth in strips
        ):
            return f"{p} lies past {hexagon} and the reach under later cells"
    return None


def pointy_hexagon(center: Point, side: float) -> list[Point]:
    """The regular pointy-top hexagon of ``side`` around ``center``, in SVG coordinates."""
    (x, y), half = center, side * math.sqrt(3) / 2
    return [
        (x, y - side),
        (x + half, y - side / 2),
        (x + half, y + side / 2),
        (x, y + side),
        (x - half, y + side / 2),
        (x - half, y - side / 2),
    ]


def clip_box(root: ET.Element, group: ET.Element) -> tuple[float, float, float, float]:
    """The left, top, right and bottom of the clip of ``group``."""
    clip_id = group.attrib["clip-path"][len("url(#") : -1]
    clip = next(c for c in root.iter(SVG + "clipPath") if c.attrib.get("id") == clip_id)
    rect = clip.find(SVG + "rect")
    if rect is not None:
        x, y, w, h = (float(rect.attrib.get(k, 0)) for k in ("x", "y", "width", "height"))
        return x, y, x + w, y + h
    path = clip.find(SVG + "path")
    assert path is not None
    points = [
        (float(x), float(y)) for x, y in re.findall(r"[ML]([-\d.e]+) ([-\d.e]+)", path.attrib["d"])
    ]
    xs, ys = zip(*points, strict=True)
    return min(xs), min(ys), max(xs), max(ys)


def assert_svg_hexagons_regular_and_whole(svg: str) -> None:
    """Checks every hexbin of ``svg`` against a regular lattice computed from its own cells.

    In each marks group, the lattice is that of regular pointy-top hexagons whose side is half
    the height of the shortest cell outline, through the center of that outline (a cell with no
    later neighbour is drawn exactly), refined over the outlines that reach no further than it
    does. Each outline must snap to its own lattice cell, lie whole inside the plot, and draw
    that cell as ``outline_error`` requires.
    """
    count = 0
    root = ET.fromstring(svg)
    for group in root.iter(SVG + "g"):
        if "clip-path" not in group.attrib:
            continue
        outlines: list[list[Point]] = []
        for path in group.findall(SVG + "path"):
            data = path.attrib.get("d", "")
            if "C" in data or path.attrib.get("fill") in (None, "none"):
                continue
            outlines.append(
                [(float(x), float(y)) for x, y in re.findall(r"[ML]([-\d.e]+) ([-\d.e]+)", data)]
            )
        if not outlines:
            continue
        boxes = [
            (min(p[0] for p in o), min(p[1] for p in o), max(p[0] for p in o), max(p[1] for p in o))
            for o in outlines
        ]
        heights = [b[3] - b[1] for b in boxes]
        widths = [b[2] - b[0] for b in boxes]
        centers = [((b[0] + b[2]) / 2, (b[1] + b[3]) / 2) for b in boxes]
        short = [k for k in range(len(boxes)) if heights[k] < min(heights) + 0.003]
        narrow = [k for k in range(len(boxes)) if widths[k] < min(widths) + 0.003]
        exact = [k for k in short if k in narrow]
        assert exact, "no outline is as short as the shortest and as narrow as the narrowest"
        reference = exact[0]
        side = heights[reference] / 2
        width = side * math.sqrt(3)
        assert abs(widths[reference] - width) < 0.01, (boxes[reference], side)
        cells: list[tuple[int, int]] = []
        for x, y in centers:
            row = round((y - centers[reference][1]) / (1.5 * side))
            col = round((x - centers[reference][0] - row * width / 2) / width)
            cells.append((row, col))
        assert len(set(cells)) == len(cells), "two outlines draw one cell"
        # Refine the lattice from the outlines that reach neither up nor down (as short as the
        # shortest), whose centers are their cells' centers, and then the columns from those
        # that reach neither left nor right.
        rows = [cells[k][0] for k in short]
        if len(set(rows)) > 1:
            step, top_row = np.polyfit(rows, [centers[k][1] for k in short], 1)
            side = float(step) / 1.5
        else:
            top_row = float(np.mean([centers[k][1] - 1.5 * side * cells[k][0] for k in short]))
        width = side * math.sqrt(3)
        left_col = float(
            np.mean([centers[k][0] - (cells[k][1] + cells[k][0] / 2) * width for k in narrow])
        )
        hexagons = [
            pointy_hexagon(
                (left_col + (col + row / 2) * width, float(top_row) + row * 1.5 * side), side
            )
            for row, col in cells
        ]
        left, top, right, bottom = clip_box(root, group)
        for i, (outline, hexagon) in enumerate(zip(outlines, hexagons, strict=True)):
            assert all(
                left - 0.005 <= x <= right + 0.005 and top - 0.005 <= y <= bottom + 0.005
                for x, y in hexagon
            ), f"clipped hexagon {hexagon}"
            error = outline_error(outline, hexagon, hexagons[i + 1 :])
            assert error is None, error
            count += 1
    assert count >= 10


def test_outline_check_rejects_notches_and_deep_reaches() -> None:
    side = 10.0
    half = side * math.sqrt(3) / 2
    hexagon = pointy_hexagon((0.0, 0.0), side)
    assert outline_error(hexagon, hexagon, []) is None
    # A corner halfway along the left edge, half a point in or out.
    for offset in (0.5, -0.5):
        notched = [*hexagon[:5], (-half + offset, 0.0), hexagon[5]]
        assert outline_error(notched, hexagon, []) is not None
    # Under the cell to the right, drawn later, the right edge may reach 1 pt, not more.
    right = pointy_hexagon((2 * half, 0.0), side)
    for reach, ok in ((1.0, True), (1.5, False)):
        lift = reach / math.sqrt(3)
        pushed = [
            hexagon[0],
            hexagon[1],
            (half + reach, -side / 2 - lift),
            (half + reach, side / 2 + lift),
            *hexagon[2:],
        ]
        assert (outline_error(pushed, hexagon, [right]) is None) == ok
        assert outline_error(pushed, hexagon, []) is not None


@pytest.mark.parametrize("shape", [(12, 2.5), (3, 10)])
def test_dense_hexbin_ylim_geometry(shape: tuple[float, float]) -> None:
    rng = np.random.default_rng(194)
    x, y = rng.normal(size=(2, 2000))
    ax = ub.Figure(*shape).add_axes().ylim(-10, 10)
    ax.hexbin(x, y, gridsize=30)
    assert_svg_hexagons_regular_and_whole(ax.figure.to_svg())


def test_dense_hexbin_scatter_geometry() -> None:
    rng = np.random.default_rng(194)
    x, y = rng.normal(size=(2, 2000))
    ax = ub.Figure(12, 2.5).add_axes().hexbin(x, y, gridsize=30)
    ax.scatter([-1, 1], [-1, 1])
    assert_svg_hexagons_regular_and_whole(ax.figure.to_svg())


def test_mesh_cells_span_edges() -> None:
    ax = axes()
    assert ax.mesh([0, 1, 4], [2, 3, 7], [[1, 2], [3, 4]]) is ax
    svg = ax.figure.to_svg()
    assert "linearGradient" in svg
    assert svg == ax.figure.to_svg()


def test_mesh_edges_set_domain() -> None:
    automatic = axes().mesh([-2, 1, 7], [3, 4, 9], [[1, 2], [3, 4]])
    explicit = axes().xlim(-2, 7).ylim(3, 9).mesh([-2, 1, 7], [3, 4, 9], [[1, 2], [3, 4]])
    assert automatic.figure.to_svg() == explicit.figure.to_svg()


@pytest.mark.parametrize("bins", [1, 3, (2, 5), (7, 2)])
def test_hist2d_counts_match_numpy(bins: int | tuple[int, int]) -> None:
    x = np.array([-3, -2, -1, 0, 1, 2, 3, 3, 0.5, np.nan, 100])
    y = np.array([4, 2, 1, 0, -1, -2, -4, -4, 0.5, 100, np.nan])
    valid = np.isfinite(x) & np.isfinite(y)
    counts, xe, ye = np.histogram2d(x[valid], y[valid], bins=bins)
    values = counts.T.copy()
    values[values == 0] = np.nan
    actual = axes().hist2d(x, y, bins=bins)
    expected = axes().mesh(xe, ye, values)
    assert actual.figure.to_svg() == expected.figure.to_svg()


def test_hist2d_log_skips_empty_cells() -> None:
    actual = axes().hist2d([0, 0, 2], [0, 0, 2], bins=2, scale=ub.Scale.log())
    expected = axes().mesh([0, 1, 2], [0, 1, 2], [[2, np.nan], [np.nan, 1]], scale=ub.Scale.log())
    assert actual.figure.to_svg() == expected.figure.to_svg()


@pytest.mark.parametrize("method", ["hist2d", "hexbin"])
def test_empty_counts_draw_no_colorbar(method: str) -> None:
    ax = axes()
    call(getattr(ax, method))([np.nan, 1], [1, np.nan], scale=ub.Scale.log())
    assert "linearGradient" not in ax.figure.to_svg()


def test_mesh_all_nan_draws_no_colorbar() -> None:
    ax = axes().mesh([0, 1], [0, 1], [[np.nan]])
    assert "linearGradient" not in ax.figure.to_svg()


@pytest.mark.parametrize(
    ("xe", "ye", "values", "message", "parameter"),
    [
        (
            [0, 1, 2, 3, 4],
            [0, 1],
            [[1, 2, 3, 4, 5]],
            "x_edges has 5 values; values has 5 columns, so x_edges needs 6.",
            "x_edges",
        ),
        (
            [0, 1],
            [0, 1],
            [[1], [2]],
            "y_edges has 2 values; values has 2 rows, so y_edges needs 3.",
            "y_edges",
        ),
        (
            [0, 1, 1],
            [0, 1],
            [[1, 2]],
            "x_edges[2] is 1; edges must be strictly increasing.",
            "x_edges",
        ),
    ],
)
def test_mesh_errors_are_atomic(
    xe: list[float], ye: list[float], values: object, message: str, parameter: str
) -> None:
    ax = axes().mesh([0, 1], [0, 1], [[1]])
    before = ax.figure.to_svg()
    with pytest.raises(ub.DataError) as caught:
        ax.mesh(xe, ye, values)
    assert str(caught.value) == f"Axes.mesh(): {message}"
    found = details(caught.value)
    assert found is not None and found[0] == parameter and found[1]
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize("values", [[1, 2], [[[1]]], [[1], [2, 3]], [[True]], [["1"]]])
def test_mesh_rejects_invalid_matrices(values: object) -> None:
    with pytest.raises((ub.DataError, TypeError)):
        axes().mesh([0, 1], [0, 1], values)


@pytest.mark.parametrize("method, option", [("hist2d", "bins"), ("hexbin", "gridsize")])
@pytest.mark.parametrize("value", [0, -1, 10001])
def test_invalid_bin_counts(method: str, option: str, value: int) -> None:
    ax = axes()
    before = ax.figure.to_svg()
    with pytest.raises(ub.StyleError) as caught:
        call(getattr(ax, method))([0, 1], [0, 1], **{option: value})
    found = details(caught.value)
    assert found is not None and found[0] == option and found[1]
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize("method", ["mesh", "hist2d", "hexbin"])
def test_category_axes_rejected(method: str) -> None:
    for ax in [
        ub.Figure().add_axes(x=ub.Scale.category()),
        ub.Figure().add_axes(y=ub.Scale.category()),
    ]:
        args = ([0, 1], [0, 1], [[1]]) if method == "mesh" else ([0, 1], [0, 1])
        with pytest.raises(ub.UsageError):
            call(getattr(ax, method))(*args)


@pytest.mark.parametrize("method", ["hist2d", "hexbin"])
@pytest.mark.parametrize("axis", ["x", "y"])
@pytest.mark.parametrize("time_values", [False, True])
def test_time_axes_rejected_before_cell_conversion(
    method: str, axis: str, time_values: bool
) -> None:
    ax = (
        ub.Figure().add_axes(x=ub.Scale.time())
        if axis == "x"
        else ub.Figure().add_axes(y=ub.Scale.time())
    )
    before = ax.figure.to_svg()
    times = [dt.datetime(2026, 1, 1), dt.datetime(2026, 1, 2)]
    x = times if time_values and axis == "x" else [0, 1]
    y = times if time_values and axis == "y" else [0, 1]
    with pytest.raises(ub.UsageError) as caught:
        call(getattr(ax, method))(x, y)
    assert str(caught.value) == f"Axes.{method}(): {axis} must be a numeric axis."
    assert details(caught.value) == (axis, "use Scale.linear() or Scale.log() for both axes.")
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize("axis", ["x", "y"])
def test_mesh_category_axes_name_the_axis(axis: str) -> None:
    ax = (
        ub.Figure().add_axes(x=ub.Scale.category())
        if axis == "x"
        else ub.Figure().add_axes(y=ub.Scale.category())
    )
    before = ax.figure.to_svg()
    with pytest.raises(ub.UsageError) as caught:
        call(cast(Any, ax).mesh)([0, 1], [0, 1], [[1]])
    assert str(caught.value) == f"Axes.mesh(): {axis} must be a numeric or time axis."
    assert details(caught.value) == (
        axis,
        "use Scale.linear(), Scale.log() or Scale.time() for both axes.",
    )
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize("method", ["mesh", "hist2d", "hexbin"])
@pytest.mark.parametrize("scale", [ub.Scale.category(), ub.Scale.time()])
def test_cell_color_scale_must_be_numeric(method: str, scale: ub.Scale[Any]) -> None:
    ax = axes().scatter([0, 1], [0, 1])
    before = ax.figure.to_svg()
    args = ([0, 1], [0, 1], [[1]]) if method == "mesh" else ([0, 1], [0, 1])
    with pytest.raises(TypeError) as caught:
        call(getattr(ax, method))(*args, scale=scale)
    assert str(caught.value) == (
        f"Axes.{method}(): scale is {scale!r}; scale must be Scale.linear() or Scale.log()."
    )
    assert details(caught.value) == ("scale", "use Scale.linear() or Scale.log().")
    assert ax.figure.to_svg() == before


def test_cell_color_conflicts_are_atomic() -> None:
    ax = axes().mesh([0, 1], [0, 1], [[1]], cmap=ub.Cmap.VIRIDIS)
    before = ax.figure.to_svg()
    for method in ["mesh", "hist2d", "hexbin"]:
        args = ([0, 1], [0, 1], [[1]]) if method == "mesh" else ([0, 1], [0, 1])
        with pytest.raises(ub.UsageError):
            call(getattr(ax, method))(*args, cmap=ub.Cmap.MAGMA)
        assert ax.figure.to_svg() == before


@pytest.mark.parametrize("method", ["hist2d", "hexbin"])
@pytest.mark.parametrize("points", [([], []), ([1], [2]), ([1, 1], [2, 2])])
def test_count_degenerate_ranges(method: str, points: tuple[list[float], list[float]]) -> None:
    ax = axes()
    call(getattr(ax, method))(*points)
    assert ax.figure.to_svg() == ax.figure.to_svg()


@pytest.mark.parametrize("bins", [True, 2.5, (2,), (2, 3, 4), (2, False), (2, 0), 2**100])
def test_hist2d_bad_bins_are_atomic(bins: object) -> None:
    ax = axes()
    before = ax.figure.to_svg()
    with pytest.raises((TypeError, ub.StyleError)) as caught:
        call(ax.hist2d)([0, 1], [0, 1], bins=bins)
    found = details(caught.value)
    assert found is not None and found[0] == "bins" and found[1]
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize("method", ["mesh", "hist2d", "hexbin"])
def test_cells_on_log_axes(method: str) -> None:
    ax = ub.Figure().add_axes(x=ub.Scale.log(), y=ub.Scale.log())
    args = ([1, 10, 100], [1, 10], [[1, 2]]) if method == "mesh" else ([1, 10, 100], [1, 10, 100])
    call(getattr(ax, method))(*args)
    assert "linearGradient" in ax.figure.to_svg()


@pytest.mark.parametrize("share", [ub.Share.NONE, ub.Share.ALL])
def test_hexbin_shared_axes(share: ub.Share) -> None:
    fig = ub.Figure(12, 5, cols=2, share_x=share, share_y=share)
    fig.add_axes(col=0).hexbin([0, 1, 2], [0, 2, 1], gridsize=5)
    fig.add_axes(col=1).hexbin([-1, 1, 3], [-3, 4, 2], gridsize=7)
    assert fig.to_svg() == fig.to_svg()


def test_empty_counts_do_not_change_other_marks() -> None:
    ax = axes().scatter([0, 1], [0, 2])
    before = ax.figure.to_svg()
    ax.hist2d([], []).hexbin([], [])
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize("edges", [[1, 1 + 1e-12], [0, 1e-300]])
def test_mesh_rejects_unresolvable_edge_range(edges: list[float]) -> None:
    ax = axes()
    before = ax.figure.to_svg()
    with pytest.raises(ub.DataError) as caught:
        ax.mesh(edges, [0, 1], [[1]])
    found = details(caught.value)
    assert found is not None and found[0] == "x_edges" and found[1]
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize("count", [1, 2, 30, ub.MAX_BINS])
def test_hexbin_one_point_is_renderable(count: int) -> None:
    ax = axes().hexbin([1], [2], gridsize=count)
    assert "linearGradient" in ax.figure.to_svg()


def test_hexbin_explicit_limits_clip() -> None:
    ax = axes().xlim(0.2, 0.8).ylim(0.2, 0.8)
    ax.hexbin([0, 0.5, 1], [0, 0.5, 1], gridsize=3)
    assert ax.figure.to_svg() == ax.figure.to_svg()


def test_hexbin_renders_counts_with_log_color() -> None:
    ax = axes().hexbin([0, 0, 1, 2, np.nan], [0, 0, 1, 2, 1], gridsize=5, scale=ub.Scale.log())
    assert "linearGradient" in ax.figure.to_svg()
    assert ax.figure.to_svg() == ax.figure.to_svg()
