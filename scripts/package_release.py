# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Build deterministic runtime-only release archives with the Python standard library."""

from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import os
import re
import tarfile
import tomllib
import zipfile
from pathlib import Path

TARGETS = ("x86_64-unknown-linux-musl", "x86_64-pc-windows-msvc")
ROOT = Path(__file__).resolve().parent.parent


def package(
    target: str, binary_path: Path, output_directory: Path, root: Path = ROOT
) -> Path:
    """Package existing binary bytes; no build or host configuration changes."""
    if target not in TARGETS:
        raise ValueError(f"Unsupported target: {target}")
    version = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))[
        "package"
    ]["version"]
    release_tag = os.environ.get("RELEASE_TAG", "")
    if release_tag and release_tag != f"v{version}":
        raise ValueError("Release tag must match Cargo package version")
    extension = ".exe" if target.endswith("windows-msvc") else ""
    manifest = (root / "herdr-plugin.toml").read_text(encoding="utf-8")
    manifest = re.sub(r"(?ms)^\[\[build\]\]\n.*?(?=^\[\[|\Z)", "", manifest)
    manifest = manifest.replace(
        "target/release/herdr-codex-subagents-rs",
        f"bin/herdr-codex-subagents-rs{extension}",
    )
    if "[[build]]" in manifest or "target/release/" in manifest:
        raise ValueError("Source-only manifest content remains")
    entries = {
        name: (root / name).read_bytes()
        for name in ("README.md", "LICENSE-MIT", "LICENSE-APACHE")
    }
    binary_name = f"bin/herdr-codex-subagents-rs{extension}"
    entries.update(
        {
            "VERSION": f"{version}\n".encode(),
            "herdr-plugin.toml": manifest.encode(),
            binary_name: binary_path.read_bytes(),
        }
    )
    output_directory.mkdir(parents=True, exist_ok=True)
    suffix = ".zip" if extension else ".tar.gz"
    archive = output_directory / f"herdr-codex-subagents-rs-{target}{suffix}"
    if extension:
        with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as bundle:
            for name, content in sorted(entries.items()):
                info = zipfile.ZipInfo(name, date_time=(2000, 1, 1, 0, 0, 0))
                info.create_system = 3
                info.external_attr = (
                    0o100755 if name == binary_name else 0o100644
                ) << 16
                info.compress_type = zipfile.ZIP_DEFLATED
                bundle.writestr(info, content)
    else:
        with (
            archive.open("wb") as raw,
            gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as compressed,
            tarfile.open(
                fileobj=compressed, mode="w", format=tarfile.USTAR_FORMAT
            ) as bundle,
        ):
            for name, content in sorted(entries.items()):
                info = tarfile.TarInfo(name)
                info.size = len(content)
                info.mode = 0o755 if name == binary_name else 0o644
                info.mtime = 946684800
                bundle.addfile(info, io.BytesIO(content))
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    (output_directory / "SHA256SUMS").write_text(
        f"{digest}  {archive.name}\n", encoding="utf-8", newline="\n"
    )
    return archive


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=TARGETS, required=True)
    parser.add_argument("--binary-path", type=Path, required=True)
    parser.add_argument("--output-directory", type=Path, default=Path("dist"))
    arguments = parser.parse_args()
    print(package(arguments.target, arguments.binary_path, arguments.output_directory))
