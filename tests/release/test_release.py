# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Offline release metadata and archive contract fixtures."""

import hashlib
import importlib.util
import subprocess
import tarfile
import tempfile
import tomllib
import unittest
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def load(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


packaging = load("package_release")
versions = load("set_version")
publication = load("publish_release")


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="hcs-release-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        for name in (
            "Cargo.toml",
            "Cargo.lock",
            "herdr-plugin.toml",
            "README.md",
            "LICENSE-MIT",
            "LICENSE-APACHE",
        ):
            (self.root / name).write_bytes((ROOT / name).read_bytes())
        self.binary = self.root / "fixture-binary"
        self.binary.write_bytes(b"synthetic executable fixture")

    def test_version_sync(self):
        versions.set_version("0.2.3", self.root)
        cargo = tomllib.loads((self.root / "Cargo.toml").read_text(encoding="utf-8"))
        lock = tomllib.loads((self.root / "Cargo.lock").read_text(encoding="utf-8"))
        plugin = tomllib.loads(
            (self.root / "herdr-plugin.toml").read_text(encoding="utf-8")
        )
        self.assertEqual(cargo["package"]["version"], "0.2.3")
        self.assertEqual(plugin["version"], "0.2.3")
        self.assertEqual(
            next(
                item
                for item in lock["package"]
                if item["name"] == "herdr-codex-subagents-rs"
            )["version"],
            "0.2.3",
        )

    def test_invalid_metadata_writes_nothing(self):
        before = (self.root / "Cargo.toml").read_bytes()
        (self.root / "herdr-plugin.toml").write_text("invalid", encoding="utf-8")
        with self.assertRaises(ValueError):
            versions.set_version("0.2.4", self.root)
        self.assertEqual((self.root / "Cargo.toml").read_bytes(), before)

    def test_invalid_version_rejected(self):
        with self.assertRaises(ValueError):
            versions.set_version("../bad", self.root)

    def test_publication_pushes_only_exact_branch_and_tag(self):
        calls = []

        def fake_run(arguments, **options):
            calls.append(arguments[3:])
            command = arguments[3:]
            output = "main" if command == ["branch", "--show-current"] else ""
            if command[:1] == ["rev-parse"]:
                output = "synthetic-head"
            return subprocess.CompletedProcess(arguments, 0, stdout=output)

        publication.publish(self.root, fake_run)
        version = tomllib.loads((self.root / "Cargo.toml").read_text(encoding="utf-8"))[
            "package"
        ]["version"]
        self.assertEqual(
            calls[-1],
            ["push", "--atomic", "origin", "refs/heads/main", f"refs/tags/v{version}"],
        )
        self.assertEqual(sum(command[0] == "push" for command in calls), 1)

    def test_publication_rejects_wrong_branch_without_push(self):
        calls = []

        def fake_run(arguments, **options):
            calls.append(arguments[3:])
            return subprocess.CompletedProcess(arguments, 0, stdout="feat/panel")

        with self.assertRaises(ValueError):
            publication.publish(self.root, fake_run)
        self.assertFalse(any(command[0] == "push" for command in calls))

    def test_archives(self):
        for target in packaging.TARGETS:
            with self.subTest(target=target):
                first = packaging.package(
                    target, self.binary, self.root / target / "one", self.root
                )
                second = packaging.package(
                    target, self.binary, self.root / target / "two", self.root
                )
                self.assertEqual(first.read_bytes(), second.read_bytes())
                if first.suffix == ".zip":
                    with zipfile.ZipFile(first) as bundle:
                        entries = {
                            name: bundle.read(name) for name in bundle.namelist()
                        }
                else:
                    with tarfile.open(first) as bundle:
                        entries = {
                            item.name: bundle.extractfile(item).read()
                            for item in bundle.getmembers()
                        }
                        self.assertEqual(
                            bundle.getmember("bin/herdr-codex-subagents-rs").mode, 0o755
                        )
                binary = "bin/herdr-codex-subagents-rs" + (
                    ".exe" if target.endswith("msvc") else ""
                )
                self.assertEqual(
                    set(entries),
                    {
                        "VERSION",
                        "README.md",
                        "LICENSE-MIT",
                        "LICENSE-APACHE",
                        "herdr-plugin.toml",
                        binary,
                    },
                )
                manifest = tomllib.loads(entries["herdr-plugin.toml"].decode())
                self.assertNotIn("build", manifest)
                self.assertNotIn(b"target/release/", entries["herdr-plugin.toml"])
                self.assertIn(binary.encode(), entries["herdr-plugin.toml"])
                expected = tomllib.loads(
                    (self.root / "Cargo.toml").read_text(encoding="utf-8")
                )["package"]["version"]
                self.assertEqual(entries["VERSION"], f"{expected}\n".encode())
                self.assertEqual(
                    (first.parent / "SHA256SUMS").read_bytes(),
                    f"{hashlib.sha256(first.read_bytes()).hexdigest()}  {first.name}\n".encode(),
                )


if __name__ == "__main__":
    unittest.main()
