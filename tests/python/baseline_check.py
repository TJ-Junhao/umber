"""Checks for SVG changes caused only by adding a baseline."""

import xml.etree.ElementTree as ET


def assert_only_baseline_added(before: str, after: str) -> None:
    """Require one new baseline, at most one replaced gridline, and no other changes."""
    old, new = ET.fromstring(before), ET.fromstring(after)
    ns = "{http://www.w3.org/2000/svg}"
    old_groups, new_groups = old.findall(f"{ns}g"), new.findall(f"{ns}g")
    assert len(new_groups) == len(old_groups) + 1
    baseline = new_groups[-1]
    assert baseline.attrib == {} and len(baseline) == 1
    path = baseline[0]
    assert path.tag == f"{ns}path"
    assert path.get("fill") == "none"
    assert path.get("stroke") == "#2B211B"
    assert path.get("stroke-width") == "1.3"
    assert path.get("d") is not None
    replaced = [
        item for item in old_groups[0] if item.tag == f"{ns}path" and item.get("d") == path.get("d")
    ]
    assert len(replaced) <= 1
    for item in replaced:
        assert item.get("stroke") == "#DDD3C7"
        assert item.get("stroke-width") == "0.75"
        old_groups[0].remove(item)
    new.remove(baseline)
    assert ET.tostring(new) == ET.tostring(old)
