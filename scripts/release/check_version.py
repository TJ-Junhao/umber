"""Check that a release tag names the version the wheels are built with.

Run from any directory:

    python scripts/release/check_version.py v0.3.0

The tag must be ``v`` followed by the version of the ``umber-py`` crate, which maturin uses as
the Python package version. ``pyproject.toml`` must declare the version as dynamic, so the
crate stays its single source.
"""

from __future__ import annotations

import json
import subprocess
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def crate_version() -> str:
    """Return the ``umber-py`` version as Cargo resolves it, workspace inheritance included."""
    output = subprocess.run(
        [
            "cargo",
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--manifest-path",
            str(ROOT / "Cargo.toml"),
        ],
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    packages = json.loads(output)["packages"]
    versions = [package["version"] for package in packages if package["name"] == "umber-py"]
    if len(versions) != 1:
        raise SystemExit("error: the workspace must contain exactly one umber-py package")
    return str(versions[0])


def main() -> None:
    """Compare the tag given on the command line with the crate and project versions."""
    if len(sys.argv) != 2:
        raise SystemExit("usage: check_version.py TAG")
    tag = sys.argv[1]

    project = tomllib.loads((ROOT / "pyproject.toml").read_text(encoding="utf-8"))["project"]
    if "version" in project or "version" not in project.get("dynamic", []):
        raise SystemExit(
            "error: pyproject.toml must list version in project.dynamic and not set it;\n"
            "hint: the version comes from crates/umber-py/Cargo.toml"
        )

    version = crate_version()
    if tag != f"v{version}":
        raise SystemExit(
            f"error: tag {tag} does not match the package version {version};\n"
            f"hint: tag the release v{version}, or change the version in Cargo.toml first"
        )
    print(f"tag {tag} matches the package version {version}")


if __name__ == "__main__":
    main()
