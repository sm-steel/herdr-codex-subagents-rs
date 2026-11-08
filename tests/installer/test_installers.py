"""Offline subprocess characterization of the real installers; never calls real Herdr."""
from __future__ import annotations

import hashlib
import io
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest
import zipfile

REPO = Path(__file__).resolve().parents[2]
WINDOWS = os.name == 'nt'
ASSET = 'herdr-codex-subagents-rs-' + ('x86_64-pc-windows-msvc.zip' if WINDOWS else 'x86_64-unknown-linux-musl.tar.gz')


class InstallerTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='installer tests ')
        self.addCleanup(self.temporary.cleanup)
        self.home = Path(self.temporary.name)
        self.root = self.home / 'install root'
        self.log = self.home / 'herdr calls.txt'
        self.env = dict(os.environ, INSTALLER_LOG=self.log.as_posix(), INSTALLER_SERVER='running', INSTALLER_STATUS_EXIT='0')
        fake = self.home / ('fake herdr.cmd' if WINDOWS else 'fake herdr')
        if WINDOWS:
            fake.write_text('@echo off\r\necho %*>>"%INSTALLER_LOG%"\r\nif "%1"=="status" (\r\n echo status: %INSTALLER_SERVER%\r\n exit /b %INSTALLER_STATUS_EXIT%\r\n)\r\nexit /b 0\r\n')
        else:
            fake.write_text('#!/bin/sh\nprintf "%s\\n" "$*" >> "$INSTALLER_LOG"\nif [ "$1" = status ]; then printf "status: %s\\n" "$INSTALLER_SERVER"; exit "$INSTALLER_STATUS_EXIT"; fi\n')
            fake.chmod(0o755)
        self.env['HERDR_BIN_PATH'] = fake.as_posix()
        self.archive = self.home / ASSET
        self.checksums = self.home / 'SHA256SUMS'

    def bundle(self, extra=None, version='0.1.0'):
        binary = 'bin/herdr-codex-subagents-rs' + ('.exe' if WINDOWS else '')
        entries = [(binary, b'synthetic binary'), ('VERSION', version.encode()), ('herdr-plugin.toml', b'id = "fixture"\n'), ('README.md', b'fixture'), ('LICENSE', b'fixture')]
        entries.extend(extra or [])
        if WINDOWS:
            with zipfile.ZipFile(self.archive, 'w') as archive:
                for name, data in entries:
                    archive.writestr(name, data)
        else:
            with tarfile.open(self.archive, 'w:gz') as archive:
                for name, data in entries:
                    entry = tarfile.TarInfo(name)
                    entry.size = len(data)
                    entry.mode = 0o755 if name == binary else 0o644
                    archive.addfile(entry, io.BytesIO(data))
        self.checksum()

    def checksum(self):
        digest = hashlib.sha256(self.archive.read_bytes()).hexdigest()
        self.checksums.write_text(f'{digest}  {ASSET}\n')

    def install(self, *extra):
        if WINDOWS:
            command = ['powershell.exe', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(REPO / 'install.ps1'), '-ArchivePath', str(self.archive), '-ChecksumsPath', str(self.checksums), '-InstallRoot', str(self.root)]
        else:
            command = ['sh', (REPO / 'install.sh').as_posix(), '--archive-path', self.archive.as_posix(), '--checksums-path', self.checksums.as_posix(), '--install-root', self.root.as_posix()]
        return subprocess.run(command + list(extra), env=self.env, capture_output=True, text=True, timeout=30)

    def assert_rejected(self, result):
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn('Install failed:', result.stdout + result.stderr)
        self.assertFalse(self.root.exists(), 'bad bundle mutated install root')
        self.assertFalse(self.log.exists(), 'bad bundle called Herdr')

    def test_install_paths_with_spaces_and_exact_setup_actions(self):
        self.bundle()
        result = self.install()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        installed = list((self.root / 'versions').iterdir())
        self.assertEqual(len(installed), 1)
        self.assertEqual((installed[0] / 'VERSION').read_text(), '0.1.0')
        calls = self.log.read_text().splitlines()
        self.assertIn('plugin link', calls[0])
        self.assertEqual(calls[1:], ['status server', 'plugin action invoke sm-steel.codex-subagents.setup', 'plugin action invoke sm-steel.codex-subagents.sync'])

    def test_no_server_does_not_invoke_actions(self):
        self.bundle()
        self.env['INSTALLER_SERVER'] = 'stopped'
        result = self.install()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(len(self.log.read_text().splitlines()), 2)
        self.assertIn('next startup', result.stdout)

    def test_failed_status_with_running_text_does_not_invoke_actions(self):
        self.bundle()
        self.env['INSTALLER_STATUS_EXIT'] = '1'
        result = self.install()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(len(self.log.read_text().splitlines()), 2)

    def test_checksum_failure_has_no_side_effects(self):
        self.bundle()
        self.checksums.write_text('0' * 64 + f'  {ASSET}\n')
        self.assert_rejected(self.install())

    def test_duplicate_checksum_has_no_side_effects(self):
        self.bundle()
        self.checksums.write_text(self.checksums.read_text() * 2)
        self.assert_rejected(self.install())

    def test_duplicate_archive_entry_has_no_side_effects(self):
        self.bundle([('VERSION', b'0.2.0')])
        self.assert_rejected(self.install())

    def test_absolute_archive_path_has_no_side_effects(self):
        self.bundle([('/absolute-escape', b'bad')])
        self.assert_rejected(self.install())

    def test_traversal_has_no_side_effects(self):
        self.bundle([('../escaped', b'bad')])
        self.assert_rejected(self.install())
        self.assertFalse((self.home / 'escaped').exists())

    def test_unknown_file_has_no_side_effects(self):
        self.bundle([('unexpected', b'bad')])
        self.assert_rejected(self.install())

    def test_invalid_version_has_no_side_effects(self):
        self.bundle(version='../escape')
        self.assert_rejected(self.install())

    def test_links_have_no_side_effects(self):
        self.bundle()
        if WINDOWS:
            with zipfile.ZipFile(self.archive, 'a') as archive:
                entry = zipfile.ZipInfo('LICENSE-link')
                entry.create_system = 3
                entry.external_attr = 0o120777 << 16
                archive.writestr(entry, '../escape')
        else:
            with tarfile.open(self.archive, 'w:gz') as archive:
                entry = tarfile.TarInfo('LICENSE-link')
                entry.type = tarfile.SYMTYPE
                entry.linkname = '../escape'
                archive.addfile(entry)
        self.checksum()
        self.assert_rejected(self.install())

    def test_exact_rerun_reuses_unchanged_install(self):
        self.bundle()
        first = self.install()
        self.assertEqual(first.returncode, 0, first.stdout + first.stderr)
        previous = next((self.root / 'versions').iterdir())
        before = (previous / 'VERSION').stat().st_mtime_ns
        second = self.install()
        self.assertEqual(second.returncode, 0, second.stdout + second.stderr)
        self.assertEqual(list((self.root / 'versions').iterdir()), [previous])
        self.assertEqual((previous / 'VERSION').stat().st_mtime_ns, before)
        calls = self.log.read_text().splitlines()
        self.assertEqual(calls[:4], calls[4:])

    def test_tampered_install_is_preserved_and_not_registered(self):
        self.bundle()
        first = self.install()
        self.assertEqual(first.returncode, 0, first.stdout + first.stderr)
        previous = next((self.root / 'versions').iterdir())
        manifest = previous / 'herdr-plugin.toml'
        manifest.write_bytes(b'tampered')
        calls = self.log.read_bytes()
        second = self.install()
        self.assertNotEqual(second.returncode, 0)
        self.assertIn('differs', second.stdout + second.stderr)
        self.assertEqual(manifest.read_bytes(), b'tampered')
        self.assertEqual(self.log.read_bytes(), calls)
        self.assertEqual(list((self.root / 'versions').iterdir()), [previous])

    def test_extra_installed_file_is_preserved_and_not_registered(self):
        self.bundle()
        first = self.install()
        self.assertEqual(first.returncode, 0, first.stdout + first.stderr)
        previous = next((self.root / 'versions').iterdir())
        unexpected = previous / 'unexpected'
        unexpected.write_bytes(b'preserve')
        calls = self.log.read_bytes()
        second = self.install()
        self.assertNotEqual(second.returncode, 0)
        self.assertEqual(unexpected.read_bytes(), b'preserve')
        self.assertEqual(self.log.read_bytes(), calls)

    def test_installed_directory_link_is_preserved_and_not_registered(self):
        self.bundle()
        first = self.install()
        self.assertEqual(first.returncode, 0, first.stdout + first.stderr)
        previous = next((self.root / 'versions').iterdir())
        outside = self.home / 'outside'
        outside.mkdir()
        marker = outside / 'marker'
        marker.write_bytes(b'preserve')
        link = previous / 'unexpected-link'
        if WINDOWS:
            create_link = self.home / 'junction.ps1'
            create_link.write_text('New-Item -ItemType Junction -Path $env:INSTALLER_LINK -Target $env:INSTALLER_LINK_TARGET | Out-Null')
            env = dict(self.env, INSTALLER_LINK=str(link), INSTALLER_LINK_TARGET=str(outside))
            created = subprocess.run(['powershell.exe', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(create_link)], env=env, capture_output=True, text=True, timeout=30)
            self.assertEqual(created.returncode, 0, created.stdout + created.stderr)
        else:
            link.symlink_to(outside, target_is_directory=True)
        calls = self.log.read_bytes()
        second = self.install()
        self.assertNotEqual(second.returncode, 0)
        self.assertIn('link', second.stdout + second.stderr)
        self.assertTrue(link.exists())
        self.assertEqual(marker.read_bytes(), b'preserve')
        self.assertEqual(self.log.read_bytes(), calls)

    def test_upgrade_preserves_previous_version(self):
        self.bundle()
        result = self.install()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        previous = next((self.root / 'versions').iterdir())
        self.bundle(version='0.2.0')
        result = self.install()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertTrue(previous.is_dir())
        self.assertEqual(len(list((self.root / 'versions').iterdir())), 2)


if __name__ == '__main__':
    unittest.main(verbosity=2)
