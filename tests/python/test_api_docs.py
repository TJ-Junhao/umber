import ast
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def test_distribution_docs_name_both_grouped_charts() -> None:
    module = ast.parse((ROOT / "python/umber/_series.py").read_text(encoding="utf-8"))
    distribution = next(
        node
        for node in module.body
        if isinstance(node, ast.ClassDef) and node.name == "Distribution"
    )
    methods = ("Axes.grouped_box()", "Axes.grouped_violin()")
    missing = {
        name: [method for method in methods if method not in (ast.get_docstring(node) or "")]
        for name, node in (("module", module), ("Distribution", distribution))
    }
    assert not any(missing.values()), missing


def test_core_api_overview_lists_figure_and_theme_exports() -> None:
    source = (ROOT / "crates/umber-core/src/lib.rs").read_text(encoding="utf-8")
    overview = source.split("//! # API surface", 1)[1].split("#![deny", 1)[0]
    exports = re.findall(r"pub use (?:figure|theme)::(?:\{([^}]+)\}|(\w+));", source)
    assert exports
    names = {
        name.strip()
        for group, single in exports
        for name in (group or single).split(",")
        if name.strip()
    }
    missing = sorted(name for name in names if f"[`{name}`]" not in overview)
    assert not missing, missing


def test_core_readme_lists_the_same_charts_as_the_main_readme() -> None:
    main = (ROOT / "README.md").read_text(encoding="utf-8")
    core = (ROOT / "crates/umber-core/README.md").read_text(encoding="utf-8")
    chart_list = re.search(r"It draws (.+?) charts", main)
    assert chart_list is not None
    charts = chart_list.group(1).replace(" and ", ", ").split(", ")
    missing = [
        chart for chart in charts if not re.search(rf"\b{re.escape(chart)}(?:s|es)?\b", core)
    ]
    assert not missing, missing


def test_core_readme_describes_grids_and_shared_axes() -> None:
    core = (ROOT / "crates/umber-core/README.md").read_text(encoding="utf-8")
    assert "grid" in core and "shared axes" in core
