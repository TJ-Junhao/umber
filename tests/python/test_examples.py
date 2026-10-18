"""Every script in ``examples/`` runs and writes valid PNG files."""

import shutil
import subprocess
import sys
from pathlib import Path

import pytest

EXAMPLES = Path(__file__).resolve().parents[2] / "examples"
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"
SCRIPTS = sorted(EXAMPLES.glob("*.py"))


def test_examples_are_present() -> None:
    assert len(SCRIPTS) >= 7


@pytest.mark.parametrize("script", SCRIPTS, ids=lambda path: path.name)
def test_example_writes_png(script: Path, tmp_path: Path) -> None:
    copy = tmp_path / script.name
    shutil.copy(script, copy)
    result = subprocess.run(
        [sys.executable, str(copy)], cwd=tmp_path, capture_output=True, text=True, check=False
    )
    assert result.returncode == 0, result.stderr
    images = sorted(tmp_path.glob("*.png"))
    assert images, "the example wrote no PNG"
    for image in images:
        assert image.read_bytes().startswith(PNG_SIGNATURE), image.name
    if script.name == "pdf.py":
        pdf = (tmp_path / "pdf.pdf").read_bytes()
        assert pdf.startswith(b"%PDF-")
        assert pdf.rstrip().endswith(b"%%EOF")
