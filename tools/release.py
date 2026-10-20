#!/usr/bin/env python3
"""
release.py - Manage Flux releases end-to-end.

Usage:
    python3 scripts/release.py <version> [--push] [--dry-run] [--force-retag]
                               [--notes-file PATH | --notes STRING]

Prints `git log <last_tag>..HEAD`, then opens $EDITOR (or $VISUAL, or vi)
on an empty temp file for you to write the changelog block. Save and quit;
the script reads the file back, inserts it verbatim into CHANGELOG.md,
bumps Cargo.toml, commits, and tags.

Baseline: v0.2.5. Anything below it is pre-history and is never used as
a range boundary.
"""

from __future__ import annotations

import argparse
import datetime as dt
import os
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
CARGO_TOML = REPO_ROOT / "Cargo.toml"
CARGO_LOCK = REPO_ROOT / "Cargo.lock"
CHANGELOG = REPO_ROOT / "CHANGELOG.md"

MIN_RELEASE_TAG = "v0.2.5"

SEMVER_RE = re.compile(r"^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$")
PACKAGE_VERSION_RE = re.compile(
    r'(^\[package\][^\[]*?^version\s*=\s*")([^"]+)(")',
    re.MULTILINE | re.DOTALL,
)
RELEASE_TAG_RE = re.compile(r"^v?\d+\.\d+\.\d+$")
RELEASE_COMMIT_RE = re.compile(
    r"^(release:\s*v?\d+\.\d+\.\d+|Release \d+\.\d+\.\d+:?|chore\(release\):)",
    re.IGNORECASE,
)
ENTRY_DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")

CHANGELOG_HEADER = (
    "# Changelog\n\n"
    "All notable changes to Flux are documented here.\n"
    "Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),\n"
    "versioning follows [Semantic Versioning](https://semver.org/).\n\n"
)

DRY_RUN = False
FORCE_RETAG = False
_COLOR = sys.stdout.isatty() and "NO_COLOR" not in os.environ


# ── Terminal ──────────────────────────────────────────────────────────────────
def info(msg: str) -> None:
    print(f"\033[1;34m->\033[0m {msg}" if _COLOR else f"-> {msg}", flush=True)


def ok(msg: str) -> None:
    print(f"\033[1;32mok\033[0m {msg}" if _COLOR else f"ok {msg}", flush=True)


def warn(msg: str) -> None:
    print(
        f"\033[1;33m!!\033[0m {msg}" if _COLOR else f"!! {msg}",
        file=sys.stderr,
        flush=True,
    )


def die(msg: str, code: int = 1) -> "NoReturn":  # type: ignore[name-defined]
    print(
        f"\033[1;31merror:\033[0m {msg}" if _COLOR else f"error: {msg}",
        file=sys.stderr,
        flush=True,
    )
    sys.exit(code)


# ── Subprocess ────────────────────────────────────────────────────────────────
def run(
    *cmd, check: bool = True, capture: bool = False, cwd: Path | None = None
) -> subprocess.CompletedProcess:
    args = [str(c) for c in cmd]
    if not capture:
        info("$ " + " ".join(args))
    return subprocess.run(
        args,
        check=check,
        capture_output=capture,
        text=True,
        cwd=str(cwd) if cwd else None,
    )


def output(*cmd, cwd: Path | None = None) -> str:
    return run(*cmd, capture=True, cwd=cwd).stdout.strip()


def which(cmd: str) -> bool:
    return shutil.which(cmd) is not None


# ── Version comparison ──────────────────────────────────────────────────────
def _version_parts(tag: str) -> tuple[int, int, int]:
    nums = [int(x) for x in re.findall(r"\d+", tag)[:3]]
    while len(nums) < 3:
        nums.append(0)
    return nums[0], nums[1], nums[2]


def _tag_lt(a: str, b: str) -> bool:
    return _version_parts(a) < _version_parts(b)


# ── Git helpers ──────────────────────────────────────────────────────────────
def tag_exists_local(tag: str) -> bool:
    return (
        run(
            "git",
            "rev-parse",
            "--verify",
            f"refs/tags/{tag}",
            cwd=REPO_ROOT,
            check=False,
            capture=True,
        ).returncode
        == 0
    )


def tag_exists_remote(tag: str) -> bool:
    r = run(
        "git",
        "ls-remote",
        "--tags",
        "origin",
        f"refs/tags/{tag}",
        cwd=REPO_ROOT,
        check=False,
        capture=True,
    )
    return bool(r.stdout.strip())


def branch_ahead_of_remote(branch: str) -> bool:
    r = run(
        "git",
        "rev-list",
        "--count",
        f"origin/{branch}..{branch}",
        cwd=REPO_ROOT,
        check=False,
        capture=True,
    )
    try:
        return int(r.stdout.strip() or "0") > 0
    except ValueError:
        return True


def _all_release_tags() -> list[str]:
    """
    Release-format tags at or above MIN_RELEASE_TAG, sorted desc by semver.
    Pre-history tags (below MIN_RELEASE_TAG) are never returned.
    """
    r = run(
        "git",
        "tag",
        "--merged",
        "HEAD",
        "--sort=-version:refname",
        cwd=REPO_ROOT,
        check=False,
        capture=True,
    )
    out: list[str] = []
    for line in r.stdout.splitlines():
        tag = line.strip()
        if not RELEASE_TAG_RE.match(tag):
            continue
        if _tag_lt(tag, MIN_RELEASE_TAG):
            continue
        out.append(tag)
    return out


def latest_release_tag(ignore: str | None = None) -> str | None:
    for tag in _all_release_tags():
        if ignore and tag == ignore:
            continue
        return tag
    return None


def print_git_log_since_last_tag(prev: str | None) -> None:
    range_spec = f"{prev}..HEAD" if prev else "HEAD"
    r = run(
        "git",
        "log",
        "--oneline",
        "--no-merges",
        "--decorate=no",
        range_spec,
        cwd=REPO_ROOT,
        check=False,
        capture=True,
    )
    lines = [
        l
        for l in r.stdout.splitlines()
        if l.strip() and not RELEASE_COMMIT_RE.match(l.split(" ", 1)[-1].strip())
    ]

    print()
    if prev:
        print(f"Commits since {prev} ({len(lines)}):")
    else:
        print(f"Commits (no previous release tag at or above {MIN_RELEASE_TAG}):")
    print("┌" + "─" * 72)
    if not lines:
        print("│ (none)")
    else:
        for line in lines:
            print(f"│ {line}")
    print("└" + "─" * 72)
    print()


# ── Cargo.toml ───────────────────────────────────────────────────────────────
def write_cargo_version(new: str) -> bool:
    text = CARGO_TOML.read_text(encoding="utf-8")
    m = PACKAGE_VERSION_RE.search(text)
    if not m:
        die("no [package] version found in Cargo.toml")
    old = m.group(2)
    if old == new:
        info(f"Cargo.toml already at {new}; skipping bump")
        return False
    info(f"Cargo.toml: {old} -> {new}")
    if DRY_RUN:
        info("[dry-run] would rewrite Cargo.toml")
        return True
    new_text = text[: m.start(2)] + new + text[m.end(2) :]
    CARGO_TOML.write_text(new_text, encoding="utf-8")
    ok(f"Cargo.toml bumped to {new}")
    return True


def refresh_cargo_lock() -> None:
    if DRY_RUN:
        info("[dry-run] would run `cargo update -p flux`")
        return
    info("Refreshing Cargo.lock")
    run(
        "cargo",
        "update",
        "--package",
        "flux",
        "--quiet",
        cwd=REPO_ROOT,
        check=False,
        capture=True,
    )


# ── Changelog notes: editor-driven ───────────────────────────────────────────
def _editor_command() -> list[str]:
    """
    Resolve the editor command: $VISUAL, then $EDITOR, then `nano`, then `vi`.
    Splits on shlex so `EDITOR="code -w"` works.
    """
    for var in ("VISUAL", "EDITOR"):
        raw = os.environ.get(var, "").strip()
        if raw:
            parts = shlex.split(raw)
            if parts and which(parts[0]):
                return parts
    for fallback in ("nano", "vi", "vim"):
        if which(fallback):
            return [fallback]
    die("no editor found (set $EDITOR or $VISUAL, or install nano/vi/vim)")


def _open_editor_for_notes(version: str) -> str:
    """
    Open an EMPTY temp file in the editor. The user writes the changelog block
    freely. Save and quit. If the file is still empty (or only whitespace),
    the release is aborted before anything is mutated.
    """
    editor = _editor_command()

    fd, tmp_path_str = tempfile.mkstemp(prefix=f"flux-{version}-notes-", suffix=".md")
    tmp_path = Path(tmp_path_str)
    try:
        # Close the fd we got from mkstemp, the editor opens the path itself.
        # Leave the file empty so the editor starts on a blank buffer.
        os.close(fd)

        print()
        info(f"Opening editor: {' '.join(editor)}")
        info(f"Notes file: {tmp_path}")
        print()

        try:
            rc = subprocess.call(editor + [str(tmp_path)])
        except FileNotFoundError:
            die(f"editor not found: {editor[0]}")

        if rc != 0:
            die(f"editor exited with status {rc}")

        raw = tmp_path.read_text(encoding="utf-8")
    finally:
        try:
            tmp_path.unlink()
        except OSError:
            pass

    # Collapse trailing blank lines, require at least one non-blank line.
    notes = raw.rstrip() + "\n"
    if not notes.strip():
        die("notes file is empty; aborting release (nothing was written)")

    return notes


def read_notes_from_file(path: Path) -> str:
    if not path.is_file():
        die(f"notes file not found: {path}")
    text = path.read_text(encoding="utf-8").rstrip() + "\n"
    if not text.strip():
        die(f"notes file is empty: {path}")
    return text


# ── Changelog insertion ──────────────────────────────────────────────────────
def confirm_replace(version: str) -> None:
    if not CHANGELOG.exists():
        return
    existing = CHANGELOG.read_text(encoding="utf-8")
    heading = f"## [{version}]"
    if heading not in existing:
        return
    print()
    warn(f"CHANGELOG.md already contains a section for {version}.")
    try:
        answer = input("Replace it? [y/N] ").strip().lower()
    except EOFError:
        die("aborted (no input)")
    if answer not in ("y", "yes"):
        die("aborted")


def _existing_entry_date(version: str, existing: str) -> str | None:
    """
    Return the date already recorded for `version` in `existing`, if any.
    Used so re-runs don't overwrite the changelog date with today's date.
    """
    heading = f"## [{version}] - "
    i = existing.find(heading)
    if i == -1:
        return None
    j = existing.find("\n", i + len(heading))
    if j == -1:
        return None
    date = existing[i + len(heading) : j].strip()
    return date if ENTRY_DATE_RE.match(date) else None


def write_changelog_entry(version: str, notes: str) -> bool:
    """
    Insert or replace the `## [version]` section.

    Returns True iff the on-disk content actually changed.
    Skips the write and returns False when the file is already up to date,
    so re-runs are idempotent and don't touch the date.
    """
    heading = f"## [{version}]"

    if not CHANGELOG.exists():
        entry = f"{heading} - {dt.date.today().isoformat()}\n\n{notes}\n"
        new_text = CHANGELOG_HEADER + f"## [Unreleased]\n\n{entry}"
        info(f"Would create {CHANGELOG.name}")
        if DRY_RUN:
            return True
        CHANGELOG.write_text(new_text, encoding="utf-8")
        ok(f"created {CHANGELOG.name}")
        return True

    existing = CHANGELOG.read_text(encoding="utf-8")
    entry_date = _existing_entry_date(version, existing) or dt.date.today().isoformat()
    entry = f"{heading} - {entry_date}\n\n{notes}\n"

    idx = existing.find(heading)
    if idx != -1:
        next_idx = existing.find("\n## [", idx + len(heading))
        tail = existing[next_idx + 1 :] if next_idx != -1 else ""
        new_text = existing[:idx] + entry + ("\n" + tail if tail else "")
        if new_text == existing:
            info(f"{CHANGELOG.name} already up to date for {version}; skipping")
            return False
        info(f"Would replace existing {version} entry in {CHANGELOG.name}")
        if DRY_RUN:
            return True
        CHANGELOG.write_text(new_text, encoding="utf-8")
        ok(f"replaced {version} entry")
        return True

    marker = "## [Unreleased]"
    midx = existing.find(marker)
    if midx != -1:
        nl = existing.find("\n", midx)
        if nl == -1:
            new_text = existing.rstrip() + "\n\n" + entry
        else:
            end = existing.find("\n## [", nl)
            if end == -1:
                new_text = existing.rstrip() + "\n\n" + entry
            else:
                new_text = existing[: end + 1] + entry + "\n" + existing[end + 1 :]
    else:
        first = existing.find("\n## [")
        new_text = (
            (existing.rstrip() + "\n\n" + entry)
            if first == -1
            else (existing[: first + 1] + entry + "\n" + existing[first + 1 :])
        )

    if new_text == existing:
        info(f"{CHANGELOG.name} already up to date for {version}; skipping")
        return False
    info(f"Would insert new {version} entry into {CHANGELOG.name}")
    if DRY_RUN:
        return True
    CHANGELOG.write_text(new_text, encoding="utf-8")
    ok(f"inserted {version} entry")
    return True


# ── Commit + tag ─────────────────────────────────────────────────────────────
def has_uncommitted_release_changes() -> bool:
    r = output(
        "git",
        "status",
        "--porcelain",
        "--",
        "Cargo.toml",
        "Cargo.lock",
        "CHANGELOG.md",
        cwd=REPO_ROOT,
    )
    return bool(r)


def commit_and_tag(version: str, notes: str) -> tuple[bool, bool]:
    """
    Commit release files if dirty, then create the annotated tag if it doesn't
    exist (or --force-retag). Returns (committed, tagged).
    """
    tag = f"v{version}"

    committed = False
    if has_uncommitted_release_changes():
        if DRY_RUN:
            info("[dry-run] would commit: Cargo.toml Cargo.lock CHANGELOG.md")
            committed = True
        else:
            run(
                "git",
                "add",
                "--",
                "Cargo.toml",
                "Cargo.lock",
                "CHANGELOG.md",
                cwd=REPO_ROOT,
            )
            run("git", "commit", "-m", f"chore(release): {tag}", cwd=REPO_ROOT)
            ok(f"committed {tag}")
            committed = True
    else:
        info("No uncommitted release changes; skipping commit")

    if tag_exists_local(tag):
        if FORCE_RETAG:
            info(f"Deleting existing tag {tag} (--force-retag)")
            if not DRY_RUN:
                run("git", "tag", "-d", tag, cwd=REPO_ROOT)
        else:
            info(f"Tag {tag} already exists locally; skipping tag creation")
            return committed, False

    if DRY_RUN:
        info(f"[dry-run] would create annotated tag {tag}")
        return committed, True
    run("git", "tag", "-a", tag, "-m", f"Release {tag}\n\n{notes}", cwd=REPO_ROOT)
    ok(f"created tag {tag}")
    return committed, True


# ── Push ─────────────────────────────────────────────────────────────────────
def push_everything(version: str, branch: str) -> None:
    tag = f"v{version}"

    if DRY_RUN:
        info("[dry-run] push plan:")
        info(
            f"  branch {branch}: "
            f"{'would push' if branch_ahead_of_remote(branch) else 'already up to date'}"
        )
        info(
            f"  tag {tag}: "
            f"{'would push' if not tag_exists_remote(tag) else 'already on origin'}"
        )
        return

    if branch_ahead_of_remote(branch):
        info(f"Pushing {branch}")
        run("git", "push", "origin", branch, cwd=REPO_ROOT)
        ok(f"pushed {branch}")
    else:
        info(f"{branch} already up to date on origin; skipping")

    if not tag_exists_remote(tag):
        info(f"Pushing tag {tag}")
        run("git", "push", "origin", tag, cwd=REPO_ROOT)
        ok(f"pushed {tag}")
    else:
        info(f"tag {tag} already on origin; skipping")


# ── Preflight ────────────────────────────────────────────────────────────────
def preflight() -> str:
    info("Preflight checks")
    if not CARGO_TOML.exists():
        die(f"{CARGO_TOML} not found")
    if not which("git"):
        die("git is not installed")

    status_lines = output("git", "status", "--porcelain", cwd=REPO_ROOT).splitlines()
    dirty = [l for l in status_lines if not l.startswith("??")]
    if dirty:
        die(
            "uncommitted changes to tracked files; commit or stash first\n"
            + "\n".join(dirty)
        )

    branch = output("git", "rev-parse", "--abbrev-ref", "HEAD", cwd=REPO_ROOT)
    if branch not in ("main", "master") and not branch.startswith("release/"):
        warn(f"on branch '{branch}'")

    return branch


# ── Main ─────────────────────────────────────────────────────────────────────
def parse_args(argv: list[str]) -> argparse.Namespace:
    p = argparse.ArgumentParser(prog="release.py")
    p.add_argument("version", help="semver, e.g. 3.0.0")
    p.add_argument(
        "--push", action="store_true", help="push branch + tag to trigger CI"
    )
    p.add_argument(
        "--dry-run", action="store_true", help="print what would happen; no mutations"
    )
    p.add_argument(
        "--force-retag",
        action="store_true",
        help="delete and recreate local tag if it already exists",
    )
    g = p.add_mutually_exclusive_group()
    g.add_argument(
        "--notes",
        metavar="STRING",
        help="release notes body (verbatim, no editor, no git log)",
    )
    g.add_argument(
        "--notes-file",
        metavar="PATH",
        type=Path,
        help="read release notes body from PATH (verbatim, no editor, no git log)",
    )
    return p.parse_args(argv)


def main(argv: list[str]) -> int:
    global DRY_RUN, FORCE_RETAG
    args = parse_args(argv)
    if not SEMVER_RE.match(args.version):
        die(f"invalid semver: {args.version}")

    DRY_RUN = args.dry_run
    FORCE_RETAG = args.force_retag
    if DRY_RUN:
        warn("dry-run mode")

    os.chdir(REPO_ROOT)
    branch = preflight()

    # Resolve notes before touching anything else, so aborting the editor
    # leaves the working tree exactly as we found it.
    if args.notes is not None:
        notes = args.notes.rstrip() + "\n"
        if not notes.strip():
            die("--notes is empty")
    elif args.notes_file is not None:
        notes = read_notes_from_file(args.notes_file)
    else:
        prev = latest_release_tag(ignore=f"v{args.version}")
        print_git_log_since_last_tag(prev)
        notes = _open_editor_for_notes(args.version)

    confirm_replace(args.version)

    bumped = write_cargo_version(args.version)
    if bumped:
        refresh_cargo_lock()

    changed = write_changelog_entry(args.version, notes)
    committed, tagged = commit_and_tag(args.version, notes)

    if args.push:
        push_everything(args.version, branch)
    elif not DRY_RUN:
        if not bumped and not changed and not committed and not tagged:
            print()
            print(f"Nothing to do: v{args.version} is already released locally.")
            if not tag_exists_remote(f"v{args.version}"):
                print("The tag exists locally but not on origin:")
                print(f"  git push origin v{args.version}")
            return 0
        print()
        print("Next steps:")
        print(f"  git push origin {branch}")
        print(f"  git push origin v{args.version}")
        print()
        print("Or re-run with --push.")

    return 0


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv[1:]))
    except subprocess.CalledProcessError as e:
        die(f"command failed (exit {e.returncode}): {' '.join(map(str, e.cmd))}")
    except KeyboardInterrupt:
        die("interrupted", code=130)
