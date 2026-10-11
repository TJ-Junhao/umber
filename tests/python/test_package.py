import tomllib
from pathlib import Path

import umber

ROOT = Path(__file__).resolve().parents[2]


def test_version_matches_cargo_workspace() -> None:
    cargo = tomllib.loads((ROOT / "Cargo.toml").read_text())
    assert umber.__version__ == cargo["workspace"]["package"]["version"]


def test_extension_is_newer_than_rust_sources() -> None:
    from umber import _umber

    extension = Path(_umber.__file__)
    sources = [ROOT / "Cargo.toml", ROOT / "Cargo.lock"]
    sources += [
        p
        for p in ROOT.glob("crates/*/src/**/*")
        if p.is_file() and not {"tests", "test_support"} & set(p.relative_to(ROOT).parts)
    ]
    sources += list(ROOT.glob("crates/*/Cargo.toml"))
    newest = max(sources, key=lambda p: p.stat().st_mtime)
    assert extension.stat().st_mtime >= newest.stat().st_mtime, (
        f"{newest.relative_to(ROOT)} is newer than the built extension; "
        'run "uv run maturin develop --uv" to rebuild the extension'
    )


def test_extension_is_private_and_present() -> None:
    from umber import _umber

    assert _umber.__version__ == umber.__version__
