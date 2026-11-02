"""Types of the functions that write several figures as one PDF."""

from pathlib import Path
from typing import assert_type

import umber as ub

first = ub.Figure()
second = ub.line([0, 1], [1, 2])
assert_type(ub.to_pdf([first, second]), bytes)
assert_type(ub.to_pdf((first, second.figure)), bytes)
assert_type(ub.save_pdf("results.pdf", [first, second]), None)
assert_type(ub.save_pdf(Path("results.pdf"), (first,)), None)

ub.to_pdf(3)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ub.to_pdf([3])  # type: ignore[list-item]  # pyright: ignore[reportArgumentType]
ub.save_pdf(b"x.pdf", [first])  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ub.save_pdf("results.pdf")  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
