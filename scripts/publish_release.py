# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Explicitly push a reviewed main release commit and its exact version tag."""

from __future__ import annotations

import subprocess
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def publish(root: Path = ROOT, run=subprocess.run) -> None:
    """Validate the local release before one atomic branch-and-tag push."""

    def git(*arguments: str) -> str:
        return run(
            ["git", "-C", str(root), *arguments],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()

    if git("branch", "--show-current") != "main":
        raise ValueError("Release publication requires main")
    if git("status", "--porcelain"):
        raise ValueError("Release publication requires a clean working tree")
    cargo = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    plugin = tomllib.loads((root / "herdr-plugin.toml").read_text(encoding="utf-8"))
    lock = tomllib.loads((root / "Cargo.lock").read_text(encoding="utf-8"))
    version = cargo["package"]["version"]
    locked = next(
        item["version"]
        for item in lock["package"]
        if item["name"] == cargo["package"]["name"]
    )
    if version != plugin["version"] or version != locked:
        raise ValueError("Release versions are not synchronized")
    tag = f"v{version}"
    if git("rev-parse", "HEAD") != git("rev-parse", f"refs/tags/{tag}^{{commit}}"):
        raise ValueError("The current version tag must point at HEAD")
    run(
        [
            "git",
            "-C",
            str(root),
            "push",
            "--atomic",
            "origin",
            "refs/heads/main",
            f"refs/tags/{tag}",
        ],
        check=True,
    )


if __name__ == "__main__":
    publish()
