"""The release tag check accepts only the tag of a released version."""

import importlib.util
from pathlib import Path
from types import ModuleType

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "release" / "check_version.py"


def load_script() -> ModuleType:
    spec = importlib.util.spec_from_file_location("check_version", SCRIPT)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_tag_equal_to_release_version_passes() -> None:
    load_script().check_tag("v0.5.0", "0.5.0")


def test_tag_differing_from_version_fails() -> None:
    with pytest.raises(SystemExit, match="does not match"):
        load_script().check_tag("v0.5.1", "0.5.0")


def test_dev_version_cannot_be_released_even_with_a_matching_tag() -> None:
    with pytest.raises(SystemExit, match="development version"):
        load_script().check_tag("v0.5.0-dev.0", "0.5.0-dev.0")
