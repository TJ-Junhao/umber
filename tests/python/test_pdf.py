"""PDF bytes, file writing and errors through the Python API."""

import os
from pathlib import Path
from typing import cast

import pytest

import umber as ub
from umber import _umber
from umber._errors import details


def test_to_pdf_returns_pdf_bytes() -> None:
    fig = ub.Figure(3, 2)
    fig.add_axes().line([0, 1, 2], [1, 3, 2]).title("Signal")
    pdf = fig.to_pdf()
    assert isinstance(pdf, bytes)
    assert pdf.startswith(b"%PDF-")
    assert pdf.rstrip().endswith(b"%%EOF")
    assert pdf == fig.to_pdf()


def test_save_pdf_writes_file(tmp_path: Path) -> None:
    fig = ub.Figure(3, 2)
    for target in (tmp_path / "chart.pdf", str(tmp_path / "string.pdf")):
        fig.save_pdf(target)
        assert Path(target).read_bytes() == fig.to_pdf()
    fig.add_axes().line([0, 1], [1, 2])
    fig.save_pdf(tmp_path / "chart.pdf")
    assert (tmp_path / "chart.pdf").read_bytes() == fig.to_pdf()


@pytest.mark.parametrize("destination", ["missing", "directory", "readonly"])
def test_save_pdf_unwritable_path_error(tmp_path: Path, destination: str) -> None:
    target = tmp_path / "chart.pdf"
    if destination == "missing":
        target = tmp_path / "missing" / "chart.pdf"
    elif destination == "directory":
        target.mkdir()
    else:
        target.write_bytes(b"unchanged")
        target.chmod(0o444)
        if os.access(target, os.W_OK):
            pytest.skip("this account can write read-only files")
    fig = ub.Figure(3, 2)
    try:
        errors: list[OSError] = []
        for save in (fig.save_png, fig.save_svg, fig.save_pdf):
            with pytest.raises(OSError) as caught:
                save(target)
            errors.append(caught.value)
            assert caught.value.filename == str(target)
            assert details(caught.value) is None
            assert not getattr(caught.value, "__notes__", [])
        assert all(type(err) is type(errors[0]) for err in errors)
        assert all(err.errno == errors[0].errno for err in errors)
        if destination == "readonly":
            assert target.read_bytes() == b"unchanged"
    finally:
        if destination == "readonly":
            target.chmod(0o644)


def test_quick_function_figure_to_pdf() -> None:
    result = ub.line([0, 1, 2], [1, 3, 2])
    pdf = result.figure.to_pdf()
    assert pdf.startswith(b"%PDF-")
    assert pdf.rstrip().endswith(b"%%EOF")


@pytest.mark.parametrize("path", [b"chart.pdf", 3])
def test_save_pdf_rejects_non_string_paths(path: object) -> None:
    with pytest.raises(TypeError) as caught:
        ub.Figure().save_pdf(cast(str, path))
    assert str(caught.value).startswith("Figure.save_pdf(): path is ")
    detail = details(caught.value)
    assert detail is not None and detail[0] == "path"


class BytesPath:
    def __fspath__(self) -> bytes:
        return b"chart.pdf"


def test_save_pdf_rejects_bytes_pathlike() -> None:
    with pytest.raises(TypeError) as caught:
        ub.Figure().save_pdf(cast(os.PathLike[str], BytesPath()))
    assert str(caught.value).startswith("Figure.save_pdf(): ")
    detail = details(caught.value)
    assert detail is not None and detail[0] == "path"


def test_pdf_render_errors_use_the_call_context(tmp_path: Path) -> None:
    fig = ub.Figure(0.5, 0.5)
    fig.add_axes().title("A title that does not fit")
    with pytest.raises(ub.StyleError) as rendered:
        fig.to_pdf()
    assert str(rendered.value).startswith("Figure.to_pdf(): ")
    target = tmp_path / "chart.pdf"
    target.write_bytes(b"unchanged")
    with pytest.raises(ub.StyleError) as saved:
        fig.save_pdf(target)
    assert str(saved.value).startswith("Figure.save_pdf(): ")
    assert details(saved.value) == details(rendered.value)
    assert target.read_bytes() == b"unchanged"


def test_pdf_bytes_match_the_native_figure() -> None:
    native = _umber.Figure(3.0, 2.0, _umber.Theme.pigment(), 1, 1, 0, 0, 0, 0)
    pdf = ub.Figure(3, 2).to_pdf()
    assert native.to_pdf() == pdf
    assert native.pdf_for_file() == pdf
