"""Several figures in one PDF through the Python API."""

import re
import subprocess
from collections.abc import Sequence
from pathlib import Path
from shutil import which
from typing import Any, cast

import pytest

import umber as ub
from umber._errors import details


def _chart(width: float, title: str) -> ub.Figure:
    fig = ub.Figure(width, 4)
    fig.add_axes().line([0, 1, 2], [1, 3, 2]).title(title)
    return fig


def _failing() -> ub.Figure:
    fig = ub.Figure(0.5, 0.5)
    fig.add_axes().title("A title that does not fit")
    return fig


def _media_boxes(pdf: bytes) -> list[str]:
    return [box.decode() for box in re.findall(rb"/MediaBox \[([^\]]*)\]", pdf)]


def test_pages_follow_the_list_and_keep_their_sizes() -> None:
    first, second = _chart(6, "First"), _chart(3, "Second")
    pdf = ub.to_pdf([first, second])
    assert pdf.startswith(b"%PDF-")
    assert _media_boxes(pdf) == ["0 0 432 288", "0 0 216 288"]
    assert _media_boxes(ub.to_pdf((second, first, second))) == [
        "0 0 216 288",
        "0 0 432 288",
        "0 0 216 288",
    ]


def test_one_figure_is_the_single_page_document() -> None:
    fig = _chart(6, "Alone")
    assert ub.to_pdf([fig]) == fig.to_pdf()


def test_the_same_figure_twice_gives_two_identical_pages() -> None:
    fig = _chart(6, "Twice")
    pdf = ub.to_pdf([fig, fig])
    assert _media_boxes(pdf) == ["0 0 432 288"] * 2
    assert pdf == ub.to_pdf([fig, fig])


def test_axes_stand_for_their_figure() -> None:
    fig = _chart(6, "Line")
    pie = ub.Figure(2, 2)
    panel = pie.add_pie_axes()
    panel.pie(["a", "b", "c"], [1, 2, 3])
    expected = ub.to_pdf([fig, pie])
    assert ub.to_pdf([fig.axes_at(), panel]) == expected


def test_save_pdf_writes_the_document(tmp_path: Path) -> None:
    figures = [_chart(6, "A"), _chart(4, "B")]
    ub.save_pdf(tmp_path / "pages.pdf", figures)
    assert (tmp_path / "pages.pdf").read_bytes() == ub.to_pdf(figures)
    ub.save_pdf(str(tmp_path / "string.pdf"), figures)
    assert (tmp_path / "string.pdf").read_bytes() == ub.to_pdf(figures)


def test_a_failing_figure_writes_nothing(tmp_path: Path) -> None:
    figures = [_chart(6, "Fine"), _failing()]
    with pytest.raises(ub.StyleError) as rendered:
        ub.to_pdf(figures)
    assert str(rendered.value).startswith("to_pdf(): figures[1]: ")
    new = tmp_path / "new.pdf"
    with pytest.raises(ub.StyleError) as saved:
        ub.save_pdf(new, figures)
    assert str(saved.value).startswith("save_pdf(): figures[1]: ")
    assert not new.exists()
    assert details(saved.value) == details(rendered.value)
    saved_details = details(saved.value)
    assert saved_details is not None and saved_details[0] == "figures"
    existing = tmp_path / "existing.pdf"
    existing.write_bytes(b"unchanged")
    with pytest.raises(ub.StyleError):
        ub.save_pdf(existing, figures)
    assert existing.read_bytes() == b"unchanged"


def test_the_error_class_is_the_figures_own() -> None:
    with pytest.raises(ub.StyleError):
        ub.to_pdf([_failing()])


def test_non_figure_items_name_their_index() -> None:
    fig = _chart(6, "Fine")
    cases: list[tuple[list[object], int]] = [
        ([fig, 3], 1),
        (["chart"], 0),
        ([None, fig], 0),
        ([fig, fig, ub.Theme.PIGMENT], 2),
    ]
    for items, index in cases:
        with pytest.raises(TypeError) as caught:
            ub.to_pdf(cast(Sequence[ub.Figure], items))
        assert str(caught.value).startswith(f"to_pdf(): figures[{index}] is ")
        detail = details(caught.value)
        assert detail is not None and detail[0] == "figures"


def test_an_empty_sequence_is_a_data_error(tmp_path: Path) -> None:
    empties: list[Sequence[ub.Figure]] = [[], ()]
    for empty in empties:
        with pytest.raises(ub.DataError) as caught:
            ub.to_pdf(empty)
        assert str(caught.value).startswith("to_pdf(): figures is empty")
    with pytest.raises(ub.DataError):
        ub.save_pdf(tmp_path / "none.pdf", [])
    assert not (tmp_path / "none.pdf").exists()


def test_a_single_figure_is_refused_with_a_hint(tmp_path: Path) -> None:
    fig = _chart(6, "One")
    calls = (
        lambda: ub.to_pdf(cast(Any, fig)),
        lambda: ub.save_pdf(tmp_path / "f.pdf", cast(Any, fig)),
    )
    for call in calls:
        with pytest.raises(TypeError) as caught:
            call()
        detail = details(caught.value)
        assert detail is not None
        assert detail[1] is not None and "fig.save_pdf(path)" in detail[1]
        assert "wrap it in a list" in detail[1]
    assert not (tmp_path / "f.pdf").exists()


@pytest.mark.parametrize("value", ["fig", b"fig", 3, None, iter([])])
def test_other_containers_are_refused(value: object) -> None:
    with pytest.raises(TypeError) as caught:
        ub.to_pdf(cast(Any, value))
    assert str(caught.value).startswith("to_pdf(): figures is ")


def test_a_bad_path_is_refused_before_rendering() -> None:
    with pytest.raises(TypeError) as caught:
        ub.save_pdf(cast(Any, 3), [_failing()])
    assert str(caught.value).startswith("save_pdf(): path is ")


def test_a_font_used_on_two_pages_is_embedded_once() -> None:
    first, second = _chart(6, "Alpha"), _chart(5, "Omega")
    single = first.to_pdf()
    both = ub.to_pdf([first, second])
    for key in (b"/FontFile2", b"/FontDescriptor", b"/ToUnicode"):
        assert both.count(key) == single.count(key) >= 1


@pytest.mark.skipif(which("pdftoppm") is None, reason="needs pdftoppm")
def test_pages_rasterize_like_single_pages(tmp_path: Path) -> None:
    figures = [_chart(6, "Alpha"), _chart(3, "Omega")]
    combined = tmp_path / "combined.pdf"
    ub.save_pdf(combined, figures)
    for number, fig in enumerate(figures, start=1):
        fig.save_pdf(tmp_path / f"single{number}.pdf")
        page = str(number)
        subprocess.run(
            ["pdftoppm", "-r", "72", "-f", page, "-l", page, "-singlefile", str(combined)]
            + [str(tmp_path / f"page{number}")],
            check=True,
            capture_output=True,
        )
        subprocess.run(
            ["pdftoppm", "-r", "72", "-singlefile", str(tmp_path / f"single{number}.pdf")]
            + [str(tmp_path / f"alone{number}")],
            check=True,
            capture_output=True,
        )
        left = (tmp_path / f"page{number}.ppm").read_bytes()
        assert left == (tmp_path / f"alone{number}.ppm").read_bytes()
