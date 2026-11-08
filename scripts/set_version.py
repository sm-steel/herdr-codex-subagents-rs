# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Synchronize package metadata for Cocogitto without building or accessing the network."""

from __future__ import annotations

import argparse
import re
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SEMVER = re.compile(
    r"(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?"
)
PATTERNS = {
    "Cargo.toml": r'(?ms)(^\[package\]\n(?:(?!^\[).)*?^version = ")[^"]+(")',
    "Cargo.lock": r'(?ms)(^\[\[package\]\]\nname = "herdr-codex-subagents-rs"\nversion = ")[^"]+(")',
    "herdr-plugin.toml": r'(?m)(^version = ")[^"]+(")',
}


def set_version(version: str, root: Path = ROOT) -> None:
    """Validate every input before writing the three synchronized versions."""
    if not SEMVER.fullmatch(version):
        raise ValueError(f"Invalid semantic version: {version}")
    updates = {}
    for name, pattern in PATTERNS.items():
        path = root / name
        content = path.read_text(encoding="utf-8")
        tomllib.loads(content)
        regex = re.compile(pattern)
        if len(regex.findall(content)) != 1:
            raise ValueError(f"Expected exactly one package version in {name}")
        updated = regex.sub(lambda match: match[1] + version + match[2], content)
        tomllib.loads(updated)
        updates[path] = updated
    for path, content in updates.items():
        path.write_text(content, encoding="utf-8", newline="\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("version")
    parser.add_argument("--root", type=Path, default=ROOT)
    arguments = parser.parse_args()
    set_version(arguments.version, arguments.root)
