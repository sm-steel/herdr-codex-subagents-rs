# User installation and README implementation plan

> Use the executing-plans or subagent-driven-development workflow. Installer I/O tests do not change approved reducer contracts.

**Goal:** A minimal README matching nani-pix-bot, a real synthetic-session feature screenshot and preview, and one-line installation for users on Windows/Linux with optional source compilation.

**Architecture:** Platform scripts download a checksummed release bundle containing bin/, a runtime-only Herdr manifest, README/licenses and VERSION. They register the bundle and invoke setup/sync when a server is available. Source mode downloads the tagged source and builds the same runtime bundle. Release CI builds Windows MSVC and Linux musl, smoke-checks Ubuntu/Arch, and publishes tag releases. Existing Rust product behavior remains unchanged.

**Tech:** PowerShell 5.1+, POSIX shell, existing Cargo/Rust 1.99 build, GitHub Actions, Herdr 0.9.5+, Edge headless for documentation capture only.

- [ ] Installers: install.sh / install.ps1, prebuilt default, --build / -Build, safe extraction/checksum/versioned staging, Herdr prerequisites and normal hook trust, local fake I/O tests.
- [ ] Packaging: release Windows x64 zip and Linux x64 musl tar.gz, root bin/ and runtime manifest, SHA256SUMS and VERSION; build/smoke/test CI and tag release publishing.
- [ ] Documentation: centered title/tagline, small badges/nav, how it works, features, user installation, development, license; move details to docs/usage.md.
- [ ] Capture: isolated Herdr session with synthetic data, expanded/folded states; no private sessions. Generate feature PNG and local rendered README preview.
- [ ] Verify installer tests, workflow checks, image/link rendering, release artifacts and Linux/Windows CI. Show the preview and explain Ubuntu/Arch/mise availability honestly.
