# Release packaging

CI builds Rust 1.99.0 with the committed lockfile for Linux x86-64 musl
and Windows x86-64 MSVC with the C runtime linked statically. Prebuilt Windows
bundles require neither Visual Studio nor a separate Visual C++ runtime install.
The release assets are:

- herdr-codex-subagents-rs-x86_64-unknown-linux-musl.tar.gz
- herdr-codex-subagents-rs-x86_64-pc-windows-msvc.zip
- install.sh, install.ps1, and SHA256SUMS

Each archive has root-level VERSION, README.md, LICENSE-MIT, LICENSE-APACHE,
herdr-plugin.toml, and bin/ with the platform binary. The runtime manifest has
no build steps and all commands reference bin/. VERSION comes from Cargo.toml;
a release tag must equal v<VERSION>. SHA256SUMS uses standard lowercase SHA-256,
two spaces, then the asset filename; published checksums include the installers.

Pushes build and upload review artifacts, including feature branches. Manual
dispatch also builds artifacts once GitHub recognizes the workflow on the default
branch. A v* tag publishes only after both platform jobs succeed. No tag or release
is created by ordinary branch pushes. Publishing has write permission; build jobs
have read-only repository permission. A failed or existing release requires
maintainer review; the workflow does not overwrite published assets.

The Linux bundle is extracted and run in Ubuntu 22.04, Ubuntu 24.04 and Arch containers with
synthetic HOME/CODEX_HOME directories and networking disabled. Smoke checks run
--version and doctor; they do not call models, link a live plugin, or change user
configuration. Windows extracts the zip and runs --version natively.
These checks establish binary startup compatibility, not a full desktop test.

For local packaging, run mise run build:linux then mise run package:linux on
Linux, or mise run build:windows then mise run package:windows on Windows.
Build targets and native toolchains must already be installed. Packaging tasks
write dist/linux and dist/windows; source-only builds use mise run build:release.
Python 3.11+ and uv are required for maintainer scripts; mise pins uv.
Python's standard library creates both archive formats without PowerShell,
GNU tar or chmod. Archives normalize metadata, entry order and gzip headers for
identical input bytes; this does not claim reproducible Rust compilation.

See [installation](install.md) for user installers. Ubuntu and Arch use the same
musl bundle. Native .deb and AUR packages have not been published. After a GitHub
release exists, mise's GitHub backend may select these assets; mise installs alone
do not perform Herdr registration or setup. Mise support has not been verified.

## Version management

Cocogitto 7.0.0, pinned in mise.toml, is the sole version/release preparation
tool. From a clean, reviewed main branch after CI passes, maintainers run
mise run version:next to inspect the calculated version, then
mise run release:prepare to create the local version commit and v-prefixed
tag. Only main is allowed. The initial feature history calculates v0.1.0.
Install the pinned tool first with mise install github:cocogitto/cocogitto.

The pre-bump Python hook synchronizes Cargo.toml, this package's Cargo.lock
entry, and herdr-plugin.toml without network access or rebuilding the executable.
Cocogitto writes CHANGELOG.md and includes the metadata in its version commit.
Maintainer scripts use Python 3.11+ through uv on both operating systems.
Release fixtures run in CI and locally with
mise run test:release.

Review the resulting commit, changelog and tag, then explicitly run
mise run release:publish. Publication requires clean main, synchronized versions,
and its exact version tag at HEAD. It pushes only main and that tag atomically
to origin; it never pushes all tags or forces updates. No hook pushes, creates a GitHub release, or runs cargo publish.
The tag workflow builds/tests artifacts and publishes GitHub release notes
extracted from the matching Cocogitto-generated CHANGELOG.md section. A missing
section fails publication. Ordinary branch CI neither bumps versions nor tags.

For routine development use mise run fmt, fmt:check, lint, test, test:installers,
test:release and deny. mise run check combines all verification tasks.

For local Herdr development, use `mise run plugin:link`, then `plugin:setup` and
`plugin:sync`. These explicit tasks affect your Herdr installation; tests use fakes.
Python helper formatting, lint and type checks run through `mise run fmt`, `lint` and
`typecheck`. `mise run check` includes all test suites and the dependency audit.

Run `mise tasks ls` to list every command. Optional `test:nextest` uses the
pinned nextest runner, `lint:workflows` audits workflow files, `lint:typos` checks
spelling, `lint:secrets` scans committed history, and `lint:commits` runs Cocogitto.
`hooks:install` explicitly installs the checkout's local Git hooks.
