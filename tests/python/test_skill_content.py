import ast
import doctest
import re
import runpy
import shutil
import subprocess
import sys
from collections.abc import Callable
from pathlib import Path
from typing import cast

import pytest

import umber

ROOT = Path(__file__).resolve().parents[2]
PACKAGE = ROOT / "python/umber"
SKILL = PACKAGE / "_skill/umber"
COPY = ROOT / "skills/umber"


def _generate(root: Path, *arguments: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(root / "scripts/gen_skill.py"), *arguments],
        cwd=root,
        capture_output=True,
        text=True,
        check=False,
    )


def _files(root: Path) -> dict[str, bytes]:
    return {
        path.relative_to(root).as_posix(): path.read_bytes()
        for path in root.rglob("*")
        if path.is_file()
    }


def _copy_sources(destination: Path) -> None:
    shutil.copytree(
        PACKAGE,
        destination / "python/umber",
        ignore=shutil.ignore_patterns("*.so", "*.pyd", "__pycache__"),
    )
    shutil.copytree(COPY, destination / "skills/umber")
    (destination / "scripts").mkdir()
    shutil.copy2(ROOT / "scripts/gen_skill.py", destination / "scripts/gen_skill.py")


def _python_blocks(text: str) -> list[tuple[int, str]]:
    blocks: list[tuple[int, str]] = []
    fence = ""
    language = ""
    first = 0
    lines: list[str] = []
    for number, line in enumerate(text.splitlines(), 1):
        if not fence:
            match = re.fullmatch(r" {0,3}(`{3,}|~{3,})\s*([^\s]*)\s*", line)
            if match:
                fence, language = match.groups()
                first = number + 1
                lines = []
        elif re.fullmatch(rf" {{0,3}}{re.escape(fence[0])}{{{len(fence)},}}\s*", line):
            if language.lower() in {"python", "py", "pycon"}:
                blocks.append((first, "\n".join(lines) + "\n"))
            fence = ""
        else:
            lines.append(line)
    assert not fence, f"Unclosed code fence at line {first - 1}"
    return blocks


def _execute(code: str, filename: str, line: int, namespace: dict[str, object]) -> None:
    if re.search(r"^\s*>>>", code, re.MULTILINE):
        test = doctest.DocTestParser().get_doctest(code, namespace, filename, filename, line - 1)
        output: list[str] = []
        result = doctest.DocTestRunner().run(test, out=output.append, clear_globs=False)
        assert result.failed == 0, "".join(output)
        namespace.update(test.globs)
    else:
        exec(compile("\n" * (line - 1) + code, filename, "exec"), namespace)


def test_generated_reference_is_current() -> None:
    result = _generate(ROOT, "--check")
    assert result.returncode == 0, result.stdout + result.stderr
    assert _files(SKILL) and _files(SKILL) == _files(COPY)


def test_every_public_name_documented() -> None:
    text = (SKILL / "references/api.md").read_text(encoding="utf-8")
    headings = set(re.findall(r"^#{2,3} `([^`]+)`$", text, re.MULTILINE))
    expected = {f"umber.{name}" for name in umber.__all__}
    members = cast(
        Callable[[Path, str], list[tuple[str, ast.stmt, str]]],
        runpy.run_path(str(ROOT / "scripts/gen_skill.py"))["_public_members"],
    )
    for name in umber.__all__:
        if isinstance(getattr(umber, name), type):
            expected.update(f"umber.{name}.{attr}" for attr, _, _ in members(PACKAGE, name))
    assert headings == expected, sorted(headings ^ expected)
    assert "Axes.line(" in text and "Values[X]" in text
    assert "Theme.PIGMENT" in text and "Theme.with_palette(" in text
    assert ">>> import umber as ub" in text
    assert "0x" not in text


def test_frontmatter_version_matches() -> None:
    for directory in (SKILL, COPY):
        text = (directory / "SKILL.md").read_text(encoding="utf-8")
        assert text.startswith("---\n")
        frontmatter, content = text[4:].split("\n---\n", 1)
        assert re.search(r"^name: umber$", frontmatter, re.MULTILINE)
        description = re.search(r"^description: (.+)$", frontmatter, re.MULTILINE)
        assert description is not None
        assert description[1] == (
            '"Plot data in Python with umber: line, scatter, bar, histogram, box, violin, '
            "heatmap and other charts, saved as PNG or SVG. Use when writing or changing "
            "code that makes charts with umber, or when turning matplotlib code into "
            'umber code."'
        )
        assert re.search(
            rf"^metadata:\n  umber-version: ['\"]?{re.escape(umber.__version__)}['\"]?$",
            frontmatter,
            re.MULTILINE,
        )
        first = content.strip().split("\n\n")[0]
        assert first.startswith(
            "Before using this skill, compare `metadata.umber-version` above with "
            "`umber.__version__` in the project's Python environment."
        )
        assert "python -m umber skill install" in first
        assert "--agent" in first
        assert "references/api.md" in content


def test_skill_code_blocks_run(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    files = sorted(SKILL.rglob("*.md"))
    assert files, "The packaged skill must contain Markdown files."
    monkeypatch.delenv("FORCE_COLOR", raising=False)
    monkeypatch.delenv("PYTHON_COLORS", raising=False)
    monkeypatch.setenv("NO_COLOR", "1")
    home = tmp_path / "home"
    home.mkdir()
    monkeypatch.setenv("HOME", str(home))
    monkeypatch.setenv("USERPROFILE", str(home))
    total = 0
    for index, file in enumerate(files):
        directory = tmp_path / str(index)
        directory.mkdir()
        monkeypatch.chdir(directory)
        namespace: dict[str, object] = {"__name__": "__main__"}
        for line, code in _python_blocks(file.read_text(encoding="utf-8")):
            _execute(code, str(file), line, namespace)
            total += 1
    assert total > 0, "The generated reference must contain runnable examples."


def test_check_prints_diff_without_writing(tmp_path: Path) -> None:
    _copy_sources(tmp_path)
    reference = tmp_path / "python/umber/_skill/umber/references/api.md"
    reference.write_text("Stale reference\n", encoding="utf-8")
    (tmp_path / "skills/umber/z-obsolete.md").write_text("Obsolete", encoding="utf-8")
    before = _files(tmp_path)
    result = _generate(tmp_path, "--check")
    assert result.returncode == 1
    assert "--- " in result.stdout and "+++ " in result.stdout
    assert "-Stale reference" in result.stdout and "+# umber API" in result.stdout
    assert result.stdout.splitlines()[-1] == (
        'The skill reference is out of date; run "uv run python scripts/gen_skill.py" '
        "and commit python/umber/_skill and skills/umber."
    )
    assert _files(tmp_path) == before
    assert _generate(tmp_path).returncode == 0
    assert _generate(tmp_path, "--check").returncode == 0


def test_new_hand_written_pages_are_copied_without_tool_changes(tmp_path: Path) -> None:
    _copy_sources(tmp_path)
    page = tmp_path / "python/umber/_skill/umber/references/usage.md"
    text = "# Usage\n\n```python\nassert 2 + 2 == 4\n```\n"
    page.write_text(text, encoding="utf-8")
    result = _generate(tmp_path, "--check")
    assert result.returncode == 1 and "usage.md" in result.stdout
    assert _generate(tmp_path).returncode == 0
    assert page.read_text(encoding="utf-8") == text
    assert (tmp_path / "skills/umber/references/usage.md").read_text(encoding="utf-8") == text
    assert _generate(tmp_path, "--check").returncode == 0
    page.unlink()
    assert _generate(tmp_path, "--check").returncode == 1
    assert _generate(tmp_path).returncode == 0
    assert not (tmp_path / "skills/umber/references/usage.md").exists()


def test_source_signature_changes_make_reference_stale(tmp_path: Path) -> None:
    _copy_sources(tmp_path)
    source = tmp_path / "python/umber/_axes.py"
    text = source.read_text(encoding="utf-8")
    assert "def line(" in text
    source.write_text(text.replace("def line(", "def connected_line(", 1), encoding="utf-8")
    result = _generate(tmp_path, "--check")
    assert result.returncode == 1
    assert "connected_line" in result.stdout


def test_public_members_include_inherited_members_in_sorted_order(tmp_path: Path) -> None:
    _copy_sources(tmp_path)
    source = tmp_path / "python/umber/_theme.py"
    source.write_text(
        "class _Base:\n"
        "    def zebra(self):\n"
        '        """Base method."""\n'
        "    def shared(self):\n"
        '        """Root method."""\n'
        "    @property\n"
        "    def alpha(self):\n"
        '        """Inherited property."""\n'
        "    DEFAULT = 1\n"
        '    """Inherited constant."""\n'
        "class _Left(_Base):\n"
        "    pass\n"
        "class _Right(_Base):\n"
        "    def shared(self):\n"
        '        """Right method."""\n'
        "class Theme(_Left, _Right):\n"
        '    """A theme."""\n'
        "    def middle(self):\n"
        '        """Own method."""\n'
        "    def zebra(self):\n"
        '        """Overridden method."""\n',
        encoding="utf-8",
    )
    result = _generate(tmp_path)
    assert result.returncode == 0, result.stdout + result.stderr
    text = (tmp_path / "skills/umber/references/api.md").read_text(encoding="utf-8")
    assert re.findall(r"^### `umber.Theme.([^`]+)`$", text, re.MULTILINE) == [
        "DEFAULT",
        "alpha",
        "middle",
        "shared",
        "zebra",
    ]
    assert "Inherited property." in text and "Inherited constant." in text
    assert "Overridden method." in text and "Base method." not in text
    assert "Right method." in text and "Root method." not in text
    assert _generate(tmp_path, "--check").returncode == 0


@pytest.mark.parametrize("inherited", (False, True))
@pytest.mark.parametrize("member", ("def missing(self): pass", "missing = 1"))
def test_public_members_without_docstrings_fail(
    tmp_path: Path, inherited: bool, member: str
) -> None:
    _copy_sources(tmp_path)
    source = tmp_path / "python/umber/_theme.py"
    text = f'class Theme:\n    """A theme."""\n    {member}\n'
    if inherited:
        text = f'class _Base:\n    {member}\nclass Theme(_Base):\n    """A theme."""\n'
    source.write_text(text, encoding="utf-8")
    before = _files(tmp_path)
    result = _generate(tmp_path)
    assert result.returncode == 1
    assert "Theme.missing has no docstring; every public member needs one." in result.stderr
    assert _files(tmp_path) == before


@pytest.mark.parametrize("language", ("python", "py", "pycon"))
@pytest.mark.parametrize("fence", ("```", "~~~~"))
def test_code_block_runner_checks_python_and_output(language: str, fence: str) -> None:
    text = f"{fence}{language}\n>>> 1 + 1\n2\n{fence}\n"
    [(line, code)] = _python_blocks(text)
    _execute(code, "example.md", line, {})
    with pytest.raises(AssertionError, match="Expected"):
        _execute(code.replace("\n2\n", "\n3\n"), "example.md", line, {})
    with pytest.raises(AssertionError):
        _execute("assert False\n", "example.md", line, {})


def test_code_block_runner_handles_plain_python_and_rejects_unclosed_fences() -> None:
    assert _python_blocks("```text\n```python\n```\n") == []
    _execute("value = 2\nassert value == 2\n", "example.md", 3, {})
    with pytest.raises(AssertionError, match="Unclosed"):
        _python_blocks("```python\npass\n")
