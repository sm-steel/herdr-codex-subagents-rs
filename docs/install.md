# User installation

Install [Herdr 0.9.5 or newer](https://herdr.dev/) first. Published bundles support
Linux x86_64 (including Ubuntu and Arch) and Windows x86_64. The default install
uses a prebuilt binary; Rust, Git, mise, and administrator access are unnecessary.
Linux needs curl, tar, and standard coreutils. Windows needs PowerShell 5.1+.

```sh
curl -fsSL https://github.com/sm-steel/herdr-codex-subagents-rs/releases/latest/download/install.sh | sh
```

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/sm-steel/herdr-codex-subagents-rs/releases/latest/download/install.ps1 | iex"
```

The installer verifies the release SHA256, rejects archive links and unsafe paths,
then atomically installs a version under `$XDG_DATA_HOME/herdr-codex-subagents`
(default `~/.local/share/herdr-codex-subagents`) or
`%LOCALAPPDATA%\herdr-codex-subagents`. It preserves older versions and running
binaries. Repeating the installer reuses an identical installation and repeats
registration/setup/sync. Changed files, extra entries, and links in an existing
installation cause a safe failure; existing files remain untouched. It registers that directory with `herdr plugin link`. If Herdr reports a
running server, it invokes the setup and sync plugin actions; otherwise the
manifest runs them at the next server startup. Review the Codex hook normally
with `/hooks` when prompted. Installation does not bypass trust review.

To inspect the script or choose a release, download it before running:

```sh
curl -fsSL https://github.com/sm-steel/herdr-codex-subagents-rs/releases/latest/download/install.sh -o install.sh
sh install.sh --version v0.1.0
# Optional source build: pinned Rust toolchain and native linker required.
sh install.sh --build --version v0.1.0
```

```powershell
irm https://github.com/sm-steel/herdr-codex-subagents-rs/releases/latest/download/install.ps1 -OutFile install.ps1
powershell -ExecutionPolicy Bypass -File .\install.ps1 -Version v0.1.0
# Optional source build: pinned Rust toolchain and native linker required.
powershell -ExecutionPolicy Bypass -File .\install.ps1 -Build -Version v0.1.0
```

Source mode downloads tagged source without Git and runs `cargo build --release
--locked`. It does not install a linker or change global toolchain settings.
Use the default prebuilt install if source compilation prerequisites are missing.

`--install-root` / `-InstallRoot` selects an absolute installation directory.
`--archive-path` and `--checksums-path` (PowerShell: `-ArchivePath` and
`-ChecksumsPath`) use local release assets with the same validation. These are
also used by offline tests; normal downloads always use HTTPS.

If registration or setup fails, the verified installation remains available.
The error explains which link or plugin action to retry. Keep older installation
directories while their binaries may still be running. To switch back, run
`herdr plugin link <older-installation-directory>`.

Installer verification uses synthetic archives and a fake Herdr executable:

```sh
mise run test:installers
```

See [usage](usage.md) for plugin behavior and [compatibility](compatibility.md)
for the actual platform verification status.
