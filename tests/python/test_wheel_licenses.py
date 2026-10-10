from importlib.metadata import distribution
from pathlib import Path

FAMILIES = ["Geist", "IBMPlexSans", "Inter", "JetBrainsMono", "SourceSerif4"]


def test_distribution_ships_every_font_license() -> None:
    dist = distribution("umber")
    shipped = {
        path.name: Path(str(dist.locate_file(path)))
        for path in dist.files or []
        if "licenses" in path.parts and path.name.endswith("-OFL.txt")
    }
    for family in FAMILIES:
        name = f"{family}-OFL.txt"
        assert name in shipped, f"{name} is missing from the installed distribution metadata"
        text = shipped[name].read_text(encoding="utf-8")
        assert "SIL OPEN FONT LICENSE" in text, name
