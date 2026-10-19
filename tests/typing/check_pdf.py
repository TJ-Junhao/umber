"""PDF return types, quick-function access and accepted file paths."""

import os
from pathlib import Path
from typing import assert_type

import umber as ub
from umber import _umber

fig = ub.Figure()
assert_type(fig.to_pdf(), bytes)
assert_type(fig.save_pdf("chart.pdf"), None)
assert_type(fig.save_pdf(Path("chart.pdf")), None)
assert_type(ub.line([0, 1], [1, 2]).figure.to_pdf(), bytes)


def save_to_pathlike(path: os.PathLike[str]) -> None:
    fig.save_pdf(path)


def native_pdf(native: _umber.Figure) -> None:
    assert_type(native.to_pdf(), bytes)
    assert_type(native.pdf_for_file(), bytes)


fig.save_pdf(b"chart.pdf")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
fig.save_pdf(3)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
fig.save_pdf(None)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
fig.to_pdf(dpi=200)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
