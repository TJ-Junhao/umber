"""Root pytest configuration shared by the Python tests and the package doctests."""

from pathlib import Path
from typing import cast

import pytest


@pytest.fixture
def isolated_environment(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """Work in a temporary directory with colour turned off, as every doctest does."""
    monkeypatch.chdir(tmp_path)
    monkeypatch.delenv("FORCE_COLOR", raising=False)
    monkeypatch.delenv("PYTHON_COLORS", raising=False)
    monkeypatch.setenv("NO_COLOR", "1")


@pytest.fixture(autouse=True)
def _isolated_doctests(request: pytest.FixtureRequest) -> None:
    """Run each doctest in the isolated environment."""
    node = cast(object, request.node)  # pyright: ignore[reportUnknownMemberType]
    if type(node).__name__ == "DoctestItem":
        request.getfixturevalue("isolated_environment")
