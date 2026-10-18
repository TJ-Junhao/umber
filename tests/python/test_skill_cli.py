import os
import subprocess
import sys
from pathlib import Path

import pytest

import umber

SKILL = Path(umber.__file__).resolve().parent / "_skill" / "umber"
AGENTS = ("claude-code", "codex", "cursor", "pi")


def _run(directory: Path, home: Path, *args: str) -> subprocess.CompletedProcess[str]:
    environment = {**os.environ, "HOME": str(home), "USERPROFILE": str(home)}
    return subprocess.run(
        [sys.executable, "-m", "umber", *args],
        cwd=directory,
        env=environment,
        text=True,
        capture_output=True,
        check=False,
    )


def _files(directory: Path) -> dict[str, bytes]:
    return {
        str(path.relative_to(directory)): path.read_bytes()
        for path in directory.rglob("*")
        if path.is_file()
    }


def test_install_writes_two_dirs_for_four_agents(tmp_path: Path) -> None:
    arguments = [arg for agent in AGENTS for arg in ("--agent", agent)]
    result = _run(tmp_path, tmp_path / "home", "skill", "install", *arguments)
    assert result.returncode == 0, result.stderr
    destinations = [tmp_path / folder / "skills/umber" for folder in (".claude", ".agents")]
    assert result.stdout.splitlines() == [str(path) for path in destinations]
    assert _files(SKILL)
    for target in destinations:
        assert _files(target) == _files(SKILL)


@pytest.mark.parametrize("agent", AGENTS)
def test_install_user_writes_home(tmp_path: Path, agent: str) -> None:
    project = tmp_path / "project"
    project.mkdir()
    home = tmp_path / "home"
    home.mkdir()
    result = _run(project, home, "skill", "install", "--agent", agent, "--user")
    assert result.returncode == 0, result.stderr
    folder = ".claude" if agent == "claude-code" else ".agents"
    target = home / folder / "skills/umber"
    assert result.stdout.splitlines() == [str(target)]
    assert _files(target) == _files(SKILL)
    assert list(project.iterdir()) == []


@pytest.mark.parametrize("command", ("install", "uninstall"))
def test_install_requires_agent(tmp_path: Path, command: str) -> None:
    result = _run(tmp_path, tmp_path / "home", "skill", command)
    assert result.returncode == 2
    assert "--agent" in result.stderr
    assert list(tmp_path.iterdir()) == []


@pytest.mark.parametrize(
    "arguments",
    [(), ("skill",), ("skill", "install", "--agent", "unknown"), ("unknown",)],
)
def test_invalid_arguments_write_nothing(tmp_path: Path, arguments: tuple[str, ...]) -> None:
    result = _run(tmp_path, tmp_path / "home", *arguments)
    assert result.returncode == 2
    assert "usage:" in result.stderr
    assert list(tmp_path.iterdir()) == []


def test_install_overwrites_and_deduplicates(tmp_path: Path) -> None:
    target = tmp_path / ".agents/skills/umber"
    target.mkdir(parents=True)
    (target / "SKILL.md").write_text("Old content", encoding="utf-8")
    (target / "obsolete.md").write_text("Old reference", encoding="utf-8")
    result = _run(tmp_path, tmp_path / "home", "skill", "install", "--agent", "pi", "--agent", "pi")
    assert result.returncode == 0, result.stderr
    assert result.stdout.splitlines() == [str(target)]
    assert _files(target) == _files(SKILL)


def test_uninstall_removes_only_installed(tmp_path: Path) -> None:
    arguments = [arg for agent in AGENTS for arg in ("--agent", agent)]
    assert _run(tmp_path, tmp_path / "home", "skill", "install", *arguments).returncode == 0
    sibling = tmp_path / ".agents/skills/other/SKILL.md"
    sibling.parent.mkdir()
    sibling.write_text("Keep this skill", encoding="utf-8")
    config = tmp_path / ".agents/config.json"
    config.write_text("{}", encoding="utf-8")
    result = _run(tmp_path, tmp_path / "home", "skill", "uninstall", "--agent", "codex")
    assert result.returncode == 0, result.stderr
    assert result.stdout.splitlines() == [str(tmp_path / ".agents/skills/umber")]
    assert not (tmp_path / ".agents/skills/umber").exists()
    assert _files(tmp_path / ".claude/skills/umber") == _files(SKILL)
    assert sibling.read_text(encoding="utf-8") == "Keep this skill"
    assert config.read_text(encoding="utf-8") == "{}"
    result = _run(tmp_path, tmp_path / "home", "skill", "uninstall", *arguments)
    assert result.returncode == 0, result.stderr
    assert result.stdout.splitlines() == [str(tmp_path / ".claude/skills/umber")]


def test_uninstall_user_leaves_project_copy(tmp_path: Path) -> None:
    home = tmp_path / "home"
    home.mkdir()
    for scope in ((), ("--user",)):
        result = _run(tmp_path, home, "skill", "install", "--agent", "pi", *scope)
        assert result.returncode == 0, result.stderr
    result = _run(tmp_path, home, "skill", "uninstall", "--agent", "pi", "--user")
    assert result.returncode == 0, result.stderr
    assert not (home / ".agents/skills/umber").exists()
    assert _files(tmp_path / ".agents/skills/umber") == _files(SKILL)


@pytest.mark.parametrize("command", ("install", "uninstall"))
def test_missing_uninstall_is_harmless_and_help_writes_nothing(
    tmp_path: Path, command: str
) -> None:
    result = _run(tmp_path, tmp_path / "home", "skill", command, "--help")
    assert result.returncode == 0, result.stderr
    assert "--agent" in result.stdout and "--user" in result.stdout
    result = _run(tmp_path, tmp_path / "home", "skill", "uninstall", "--agent", "pi")
    assert result.returncode == 0, result.stderr
    assert result.stdout == ""
    assert list(tmp_path.iterdir()) == []


@pytest.mark.parametrize("command", ("install", "uninstall"))
def test_non_directory_target_fails_before_writing(tmp_path: Path, command: str) -> None:
    blocked = tmp_path / ".agents"
    blocked.write_text("Keep this file", encoding="utf-8")
    result = _run(
        tmp_path,
        tmp_path / "home",
        "skill",
        command,
        "--agent",
        "claude-code",
        "--agent",
        "pi",
    )
    assert result.returncode == 1
    assert str(blocked) in result.stderr
    assert "hint:" in result.stderr and "Traceback" not in result.stderr
    assert not (tmp_path / ".claude").exists()
    assert blocked.read_text(encoding="utf-8") == "Keep this file"


@pytest.mark.skipif(os.name == "nt", reason="Directory symlinks need elevated privileges.")
@pytest.mark.parametrize("command", ("install", "uninstall"))
@pytest.mark.parametrize("relative", (".agents", ".agents/skills", ".agents/skills/umber"))
def test_symbolic_link_targets_are_not_followed(
    tmp_path: Path, command: str, relative: str
) -> None:
    external = tmp_path / "external"
    external.mkdir()
    marker = external / "keep.txt"
    marker.write_text("Keep this file", encoding="utf-8")
    link = tmp_path / relative
    link.parent.mkdir(parents=True, exist_ok=True)
    link.symlink_to(external, target_is_directory=True)
    result = _run(tmp_path, tmp_path / "home", "skill", command, "--agent", "pi")
    assert result.returncode == 1
    assert str(link) in result.stderr
    assert "hint:" in result.stderr and "Traceback" not in result.stderr
    assert link.is_symlink()
    assert _files(external) == {"keep.txt": b"Keep this file"}
