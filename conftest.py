"""Root pytest configuration shared by the Python tests and the package doctests."""

from pathlib import Path
from typing import cast

import pytest


@pytest.fixture(autouse=True)
def _isolated_doctests(
    request: pytest.FixtureRequest, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Run each doctest in a temporary directory with colour turned off."""
    node = cast(object, request.node)  # pyright: ignore[reportUnknownMemberType]
    if type(node).__name__ != "DoctestItem":
        return
    monkeypatch.chdir(tmp_path)
    monkeypatch.delenv("FORCE_COLOR", raising=False)
    monkeypatch.delenv("PYTHON_COLORS", raising=False)
    monkeypatch.setenv("NO_COLOR", "1")
