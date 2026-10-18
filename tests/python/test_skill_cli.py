import doctest
import errno
import os
import shutil
import subprocess
import sys
from pathlib import Path

import pytest

import umber
from umber import _skill_cli

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
    assert result.stdout.splitlines() == [
        str(tmp_path / ".claude/skills/umber"),
        f"nothing to remove at {tmp_path / '.agents/skills/umber'}",
    ]


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
    assert result.stdout.splitlines() == [
        f"nothing to remove at {tmp_path / '.agents/skills/umber'}"
    ]
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
@pytest.mark.parametrize("agent, folder", (("claude-code", ".claude"), ("pi", ".agents")))
@pytest.mark.parametrize("user", (False, True))
@pytest.mark.parametrize("dangling", (False, True))
def test_symbolic_link_targets_are_not_followed(
    tmp_path: Path, command: str, agent: str, folder: str, user: bool, dangling: bool
) -> None:
    external = tmp_path / "external"
    if not dangling:
        external.mkdir()
        (external / "keep.txt").write_text("Keep this file", encoding="utf-8")
    home = tmp_path / "home"
    base = home if user else tmp_path
    link = base / folder / "skills/umber"
    link.parent.mkdir(parents=True)
    link.symlink_to(external, target_is_directory=True)
    scope = ["--user"] if user else []
    result = _run(tmp_path, home, "skill", command, "--agent", agent, *scope)
    assert result.returncode == 1
    assert f"umber skill {command}: {link}: symbolic links are not supported;" in result.stderr
    assert "hint:" in result.stderr and "Traceback" not in result.stderr
    assert link.is_symlink()
    assert _files(external) == ({} if dangling else {"keep.txt": b"Keep this file"})


@pytest.mark.skipif(os.name == "nt", reason="Directory symlinks need elevated privileges.")
@pytest.mark.parametrize("agent, folder", (("claude-code", ".claude"), ("pi", ".agents")))
@pytest.mark.parametrize("user", (False, True))
@pytest.mark.parametrize("skills_link", (False, True))
def test_symbolic_link_ancestors_are_followed(
    tmp_path: Path, agent: str, folder: str, user: bool, skills_link: bool
) -> None:
    home = tmp_path / "home"
    base = home if user else tmp_path
    external = tmp_path / "external"
    external.mkdir()
    (external / "keep.txt").write_text("Keep this file", encoding="utf-8")
    link = base / folder
    if skills_link:
        link /= "skills"
    link.parent.mkdir(parents=True, exist_ok=True)
    link.symlink_to(external, target_is_directory=True)
    target = base / folder / "skills/umber"
    scope = ["--user"] if user else []
    for command in ("install", "uninstall"):
        result = _run(tmp_path, home, "skill", command, "--agent", agent, *scope)
        assert result.returncode == 0, result.stderr
        assert result.stdout.splitlines() == [str(target)]
        assert link.is_symlink()
        if command == "install":
            assert _files(target) == _files(SKILL)
        else:
            assert not target.exists()
        assert (external / "keep.txt").read_text(encoding="utf-8") == "Keep this file"


@pytest.mark.skipif(os.name == "nt", reason="Directory symlinks need elevated privileges.")
@pytest.mark.parametrize("command", ("install", "uninstall"))
@pytest.mark.parametrize("agent, folder", (("claude-code", ".claude"), ("pi", ".agents")))
def test_project_links_must_stay_inside_the_project(
    tmp_path: Path, command: str, agent: str, folder: str
) -> None:
    project = tmp_path / "project"
    outside = tmp_path / "outside"
    (outside / "skills/umber").mkdir(parents=True)
    (outside / "skills/umber/keep.txt").write_text("Keep this file", encoding="utf-8")
    project.mkdir()
    (project / folder).symlink_to(outside, target_is_directory=True)
    result = _run(project, tmp_path / "home", "skill", command, "--agent", agent)
    assert result.returncode == 1
    assert (
        f"umber skill {command}: {project / folder / 'skills'}: a symbolic link leads outside "
        "the current directory; a project skill directory must be inside it."
    ) in result.stderr
    assert _files(outside / "skills/umber") == {"keep.txt": b"Keep this file"}


def test_cli_examples_leave_existing_skills_untouched(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    home = tmp_path / "home"
    for base in (tmp_path, home):
        for folder in (".claude", ".agents"):
            path = base / folder / "skills/umber/SKILL.md"
            path.parent.mkdir(parents=True)
            path.write_text("Keep this skill", encoding="utf-8")
    before = _files(tmp_path)
    monkeypatch.chdir(tmp_path)
    monkeypatch.setenv("HOME", str(home))
    monkeypatch.setenv("USERPROFILE", str(home))
    result = doctest.testmod(_skill_cli)
    assert _files(tmp_path) == before
    assert result.failed == 0 and result.attempted > 0


@pytest.mark.skipif(os.name == "nt", reason="Windows does not enforce POSIX directory modes.")
@pytest.mark.parametrize("command", ("install", "uninstall"))
@pytest.mark.parametrize("user", (False, True))
def test_permission_error_names_target_directory(tmp_path: Path, command: str, user: bool) -> None:
    home = tmp_path / "home"
    base = home if user else tmp_path
    blocked = base / ".agents/skills"
    target = blocked / "umber"
    target.mkdir(parents=True)
    (target / "SKILL.md").write_text("Keep this skill", encoding="utf-8")
    blocked.chmod(0o555)
    try:
        if os.access(blocked, os.W_OK):
            pytest.skip("This process can write to read-only directories.")
        scope = ["--user"] if user else []
        result = _run(
            tmp_path, home, "skill", command, "--agent", "claude-code", "--agent", "pi", *scope
        )
        assert result.returncode == 1
        hint = f"{command} without --user." if user else "use --user."
        assert result.stderr.splitlines() == [
            f"umber skill {command}: {blocked} is not writable; "
            "the skill directory must be writable.",
            f"hint: check the directory's permissions, or {hint}",
        ]
        assert _files(target) == {"SKILL.md": b"Keep this skill"}
        assert list(blocked.iterdir()) == [target]
        assert not (base / ".claude").exists()
    finally:
        blocked.chmod(0o755)


def test_other_filesystem_error_names_target(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    target = tmp_path / ".agents/skills/umber"
    target.mkdir(parents=True)
    (target / "SKILL.md").write_text("Keep this skill", encoding="utf-8")

    def fail_copy(source: object, destination: Path) -> None:
        raise OSError(errno.ENOSPC, "No space left on device", str(destination))

    monkeypatch.chdir(tmp_path)
    monkeypatch.setattr(shutil, "copytree", fail_copy)
    assert _skill_cli.main(["skill", "install", "--agent", "pi"]) == 1
    error = capsys.readouterr().err
    assert f"umber skill install: {target}" in error
    assert "No space left on device" in error
    assert ".umber-" not in error and "Traceback" not in error
    assert _files(target) == {"SKILL.md": b"Keep this skill"}
