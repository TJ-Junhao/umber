"""Fail if a wheel exceeds the size budget, and report each wheel's size and font share.

Run from any directory:

    python scripts/release/check_wheel_sizes.py dist/*.whl

The fonts are embedded in the extension module, so their share of a wheel is estimated by
deflating the bundled font files the way the wheel's zip archive does. The report is printed
and, on GitHub Actions, appended to the job summary.
"""

from __future__ import annotations

import os
import sys
import zipfile
import zlib
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FONTS = ROOT / "crates" / "umber-core" / "fonts"
MB = 1_000_000

# Largest wheel allowed, compressed, as uploaded to the package index.
WHEEL_BUDGET = 8 * MB
# The bundled fonts may add at most this much to a wheel.
FONT_BUDGET = 3 * MB


@dataclass(frozen=True)
class WheelSize:
    """Sizes of one wheel and of the extension module inside it."""

    name: str
    compressed: int
    extension_installed: int
    extension_compressed: int


def measure(path: Path) -> WheelSize:
    """Read the wheel's size and its extension module's installed and compressed sizes."""
    with zipfile.ZipFile(path) as archive:
        extensions = [
            info
            for info in archive.infolist()
            if Path(info.filename).name.startswith("_umber.")
            and Path(info.filename).suffix in {".so", ".pyd"}
        ]
    if len(extensions) != 1:
        raise SystemExit(f"error: {path.name} must contain exactly one _umber extension module")
    extension = extensions[0]
    return WheelSize(
        name=path.name,
        compressed=path.stat().st_size,
        extension_installed=extension.file_size,
        extension_compressed=extension.compress_size,
    )


def font_sizes() -> tuple[int, int]:
    """Return the bundled fonts' total size, raw and deflated."""
    fonts = sorted(FONTS.glob("*.ttf"))
    if not fonts:
        raise SystemExit(f"error: no fonts found in {FONTS}")
    data = [font.read_bytes() for font in fonts]
    return sum(len(d) for d in data), sum(len(zlib.compress(d, 6)) for d in data)


def mb(size: int) -> str:
    """Format a byte count in megabytes."""
    return f"{size / MB:.2f} MB"


def main() -> None:
    """Measure every wheel given on the command line, report, and enforce the budgets."""
    paths = [Path(arg) for arg in sys.argv[1:]]
    if not paths:
        raise SystemExit("usage: check_wheel_sizes.py WHEEL...")
    wheels = [measure(path) for path in paths]
    font_raw, font_deflated = font_sizes()

    lines = [
        "## Wheel sizes",
        "",
        f"Budget: {mb(WHEEL_BUDGET)} per wheel. Bundled fonts: {mb(font_raw)} raw, "
        f"about {mb(font_deflated)} deflated (budget {mb(FONT_BUDGET)}).",
        "",
        "| Wheel | Size | Extension (installed) | Fonts' share of the wheel |",
        "| --- | ---: | ---: | ---: |",
    ]
    for wheel in sorted(wheels, key=lambda w: w.name):
        share = font_deflated / wheel.compressed
        lines.append(
            f"| `{wheel.name}` | {mb(wheel.compressed)} | {mb(wheel.extension_installed)} "
            f"| {share:.0%} |"
        )

    errors = [
        f"error: {wheel.name} is {mb(wheel.compressed)}, over the {mb(WHEEL_BUDGET)} budget"
        for wheel in wheels
        if wheel.compressed > WHEEL_BUDGET
    ]
    if font_raw > FONT_BUDGET:
        errors.append(f"error: the bundled fonts are {mb(font_raw)}, over {mb(FONT_BUDGET)}")

    report = "\n".join(lines) + "\n"
    print(report)
    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary:
        with open(summary, "a", encoding="utf-8") as file:
            file.write(report)
    if errors:
        raise SystemExit("\n".join(errors))


if __name__ == "__main__":
    main()
