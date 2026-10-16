"""Smoke test for an installed umber wheel: import it, render a PNG and save an SVG.

Run with the interpreter of an environment that has only the wheel and its dependencies
installed (``test-wheel.sh`` sets one up). The script fails if ``umber`` is imported from
anywhere other than that environment's ``site-packages``.
"""

from __future__ import annotations

import sys
import sysconfig
import tempfile
from pathlib import Path

import umber as ub


def main() -> None:
    """Check where umber is imported from, then render one figure to PNG and to an SVG file."""
    location = Path(ub.__file__).resolve()
    site_packages = Path(sysconfig.get_paths()["platlib"]).resolve()
    if site_packages not in location.parents:
        raise SystemExit(f"error: umber was imported from {location}, not from {site_packages}")

    png = ub.line([0, 1], [0, 1]).figure.to_png()
    if not png.startswith(b"\x89PNG\r\n\x1a\n"):
        raise SystemExit("error: to_png() did not return a PNG")

    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / "smoke.svg"
        ub.line([0, 1], [0, 1]).figure.save_svg(path)
        svg = path.read_text(encoding="utf-8")
        if "<svg" not in svg:
            raise SystemExit("error: save_svg() did not write an SVG document")

    if sysconfig.get_config_var("Py_GIL_DISABLED"):
        gil_enabled = sys._is_gil_enabled()
        if gil_enabled:
            raise SystemExit("error: importing and rendering with umber enabled the GIL")
        print(f"sys._is_gil_enabled() = {gil_enabled}")

    print(f"umber from {location}: PNG {len(png)} bytes, SVG {len(svg)} characters")


if __name__ == "__main__":
    main()
