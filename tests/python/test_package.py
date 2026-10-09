import tomllib
from pathlib import Path

import umber

ROOT = Path(__file__).resolve().parents[2]


def test_version_matches_cargo_workspace() -> None:
    cargo = tomllib.loads((ROOT / "Cargo.toml").read_text())
    assert umber.__version__ == cargo["workspace"]["package"]["version"]


def test_extension_is_private_and_present() -> None:
    from umber import _umber

    assert _umber.__version__ == umber.__version__


def test_public_names() -> None:
    assert umber.__all__ == ["DataError", "StyleError", "UmberError", "UsageError", "__version__"]
