"""Copy the packaged skill to explicitly selected agent directories."""

import argparse
import shutil
import sys
from contextlib import ExitStack
from importlib import resources
from pathlib import Path
from tempfile import TemporaryDirectory
from typing import Final, cast

_AGENT_DIRS: Final = {
    "claude-code": ".claude",
    "codex": ".agents",
    "cursor": ".agents",
    "pi": ".agents",
}


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="python -m umber")
    commands = parser.add_subparsers(dest="command", required=True)
    skill = commands.add_parser("skill", help="manage the bundled umber skill")
    actions = skill.add_subparsers(dest="action", required=True)
    for action in ("install", "uninstall"):
        command = actions.add_parser(action, help=f"{action} the bundled umber skill")
        command.add_argument(
            "--agent",
            action="append",
            choices=tuple(_AGENT_DIRS),
            required=True,
            help="target agent; repeat to select more than one",
        )
        command.add_argument(
            "--user",
            action="store_true",
            help="use the home directory instead of the current directory",
        )
    return parser


def _targets(base: Path, agents: list[str]) -> list[Path]:
    return list(dict.fromkeys(base / _AGENT_DIRS[agent] / "skills/umber" for agent in agents))


def _check_targets(base: Path, targets: list[Path], *, contained: bool) -> None:
    # A project directory may come from someone else's repository, so links there must not lead
    # outside it; the home directory is the user's own and may link agent folders elsewhere.
    for target in targets:
        if contained and not target.parent.resolve().is_relative_to(base.resolve()):
            raise OSError(
                f"{target.parent}: a symbolic link leads outside the current directory; "
                "a project skill directory must be inside it."
            )
        path = base
        for part in target.relative_to(base).parts:
            path /= part
            if path == target and path.is_symlink():
                raise OSError(f"{path}: symbolic links are not supported; use a real directory.")
            if path.exists() and not path.is_dir():
                raise OSError(f"{path}: not a directory; a directory is required.")


def _make_parents(path: Path, created: list[Path]) -> None:
    if path.exists():
        return
    _make_parents(path.parent, created)
    path.mkdir()
    created.append(path)


def _apply(targets: list[Path], *, install: bool) -> list[Path]:
    """Stage complete copies before replacing any destination; undo failed replacements."""
    changed: list[Path] = []
    created: list[Path] = []
    saved: list[tuple[Path, Path]] = []
    destination: Path | None = None
    try:
        with ExitStack() as stack:
            staged: list[tuple[Path, Path]] = []
            for target in targets:
                destination = target
                if not install and not target.exists():
                    continue
                _make_parents(target.parent, created)
                directory = Path(
                    stack.enter_context(TemporaryDirectory(prefix=".umber-", dir=target.parent))
                )
                if install:
                    with resources.as_file(
                        resources.files("umber").joinpath("_skill", "umber")
                    ) as source:
                        shutil.copytree(source, directory / "new")
                staged.append((target, directory))
            try:
                for target, directory in staged:
                    destination = target
                    if target.exists():
                        previous = directory / "previous"
                        target.rename(previous)
                        saved.append((target, previous))
                    if install:
                        (directory / "new").rename(target)
                    changed.append(target)
            except OSError:
                for target in reversed(changed):
                    if install:
                        shutil.rmtree(target)
                for target, previous in reversed(saved):
                    previous.rename(target)
                raise
    except OSError as error:
        for path in reversed(created):
            path.rmdir()
        if destination is not None:
            error.filename = str(
                destination.parent if isinstance(error, PermissionError) else destination
            )
            error.filename2 = None
        raise
    return changed


def main(arguments: list[str]) -> int:
    """Install or uninstall the skill for explicitly named agents.

    Args:
        arguments: Command-line arguments without the executable name.

    Returns:
        Zero on success, or one for a file-system error. Argument errors exit with two.

    Example:
        >>> from umber._skill_cli import _parser
        >>> options = _parser().parse_args(["skill", "uninstall", "--agent", "pi"])
        >>> (options.action, options.agent, options.user)
        ('uninstall', ['pi'], False)
    """
    options = _parser().parse_args(arguments)
    action = cast(str, options.action)
    agents = cast(list[str], options.agent)
    base = Path.home() if options.user else Path.cwd()
    try:
        targets = _targets(base, agents)
        _check_targets(base, targets, contained=not options.user)
        paths = _apply(targets, install=action == "install")
    except PermissionError as error:
        print(
            f"umber skill {action}: {error.filename} is not writable; "
            "the skill directory must be writable.",
            file=sys.stderr,
        )
        alternative = f"{action} without --user." if options.user else "use --user."
        print(f"hint: check the directory's permissions, or {alternative}", file=sys.stderr)
        return 1
    except OSError as error:
        message = f"{error.filename}: {error.strerror}" if error.filename else str(error)
        print(f"umber skill {action}: {message}", file=sys.stderr)
        print("hint: check the target directory, then try again.", file=sys.stderr)
        return 1
    for path in targets:
        print(path if path in paths else f"nothing to remove at {path}")
    return 0
