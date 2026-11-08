# PowerShell 5.1+; the default install uses a ready-to-run release binary.
[CmdletBinding()]
param(
    [switch]$Build,
    [string]$Version,
    [string]$ArchivePath,
    [string]$ChecksumsPath,
    [string]$InstallRoot
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0
$base = 'https://github.com/sm-steel/herdr-codex-subagents-rs'
$asset = 'herdr-codex-subagents-rs-x86_64-pc-windows-msvc.zip'
$work = $null
$stage = $null
function Assert-PlainDirectory([string]$Path) {
    if (Test-Path -LiteralPath $Path) {
        $item = Get-Item -LiteralPath $Path -Force
        if (-not $item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
            throw 'Installation directories must be ordinary directories, not links.'
        }
    }
}
function Get-Sha256([string]$Path) {
    $stream = [IO.File]::OpenRead($Path)
    $algorithm = [Security.Cryptography.SHA256]::Create()
    try { return [BitConverter]::ToString($algorithm.ComputeHash($stream)).Replace('-', '').ToLowerInvariant() }
    finally { $stream.Dispose(); $algorithm.Dispose() }
}
function Get-InstallInventory([string]$Path) {
    Assert-PlainDirectory $Path
    $inventory = @{}
    $pending = New-Object 'System.Collections.Generic.Stack[string]'
    $pending.Push($Path)
    while ($pending.Count -gt 0) {
        $directory = $pending.Pop()
        foreach ($item in Get-ChildItem -LiteralPath $directory -Force) {
            if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
                throw 'Existing installation differs from the verified bundle (link found); it was preserved.'
            }
            $relative = $item.FullName.Substring($Path.TrimEnd('\').Length + 1)
            if ($item.PSIsContainer) {
                $inventory[$relative] = 'directory'
                $pending.Push($item.FullName)
            } else {
                $inventory[$relative] = Get-Sha256 $item.FullName
            }
        }
    }
    return $inventory
}
function Assert-SameInstall([string]$Verified, [string]$Installed) {
    $expected = Get-InstallInventory $Verified
    $existing = Get-InstallInventory $Installed
    if ($expected.Count -ne $existing.Count) {
        throw 'Existing installation differs from the verified bundle; it was preserved. Choose a different -InstallRoot or inspect the existing installation.'
    }
    foreach ($name in $expected.Keys) {
        if (-not $existing.ContainsKey($name) -or $existing[$name] -ne $expected[$name]) {
            throw 'Existing installation differs from the verified bundle; it was preserved. Choose a different -InstallRoot or inspect the existing installation.'
        }
    }
}
function Get-Download([string]$Url, [string]$Destination) {
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
    try { Invoke-WebRequest -Uri $Url -OutFile $Destination -UseBasicParsing } catch {
        throw 'Download failed. Check network access and the release version.'
    }
}
function Expand-SafeZip([string]$Path, [string]$Destination, [bool]$Bundle) {
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip = [IO.Compression.ZipFile]::OpenRead($Path)
    try {
        $seen = @{}
        foreach ($entry in $zip.Entries) {
            $name = $entry.FullName
            if ($name -match '(^/|\\|:|(^|/)\.\.?(/|$)|[\x00-\x1f])' -or $name -match '[. ](/|$)' -or $name -match '(^|/)(CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])(\.|/|$)') { throw 'Unsafe archive path.' }
            if ($seen.ContainsKey($name)) { throw 'Duplicate archive entry.' }
            $seen[$name] = $true
            $mode = ($entry.ExternalAttributes -shr 16) -band 61440
            if ($mode -ne 0 -and $mode -ne 32768 -and $mode -ne 16384) { throw 'Archive links and special files are forbidden.' }
            if ($Bundle -and $name -notmatch '^(bin/?|bin/herdr-codex-subagents-rs\.exe|herdr-plugin\.toml|VERSION|README\.md|LICENSE[A-Za-z0-9._-]*)$') { throw 'Unexpected bundle entry.' }
            $target = [IO.Path]::GetFullPath((Join-Path $Destination $name))
            if (-not $target.StartsWith($Destination.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Unsafe archive path.' }
        }
        # Validate every entry before creating any extracted files.
        [IO.Directory]::CreateDirectory($Destination) | Out-Null
        foreach ($entry in $zip.Entries) {
            $target = Join-Path $Destination $entry.FullName
            if ($entry.FullName.EndsWith('/')) { [IO.Directory]::CreateDirectory($target) | Out-Null; continue }
            [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($target)) | Out-Null
            [IO.Compression.ZipFileExtensions]::ExtractToFile($entry, $target, $false)
        }
    } finally { $zip.Dispose() }
}
try {
    if ($env:OS -ne 'Windows_NT') { throw 'Use install.sh on Linux.' }
    $architecture = $env:PROCESSOR_ARCHITEW6432
    if (-not $architecture) { $architecture = $env:PROCESSOR_ARCHITECTURE }
    if ($architecture -ne 'AMD64') { throw 'Only Windows x86_64 is supported by published bundles.' }
    $herdrName = 'herdr'
    if ($env:HERDR_BIN_PATH) { $herdrName = $env:HERDR_BIN_PATH }
    $herdrCommand = Get-Command $herdrName -ErrorAction SilentlyContinue
    if (-not $herdrCommand) { throw 'Install Herdr 0.9.5 or newer first, then retry.' }
    $herdr = $herdrCommand.Source
    if (-not $InstallRoot) {
        if (-not $env:LOCALAPPDATA) { throw 'LOCALAPPDATA is unavailable; provide -InstallRoot with an absolute path.' }
        $InstallRoot = Join-Path $env:LOCALAPPDATA 'herdr-codex-subagents'
    }
    if (-not [IO.Path]::IsPathRooted($InstallRoot)) { throw '-InstallRoot must be absolute.' }
    $InstallRoot = [IO.Path]::GetFullPath($InstallRoot)
    Assert-PlainDirectory $InstallRoot
    if ($Version -and $Version -notmatch '^v?[0-9][A-Za-z0-9._-]*$') { throw 'Use a numeric release tag, for example v0.1.0.' }
    $work = Join-Path ([IO.Path]::GetTempPath()) ('herdr-install-' + [guid]::NewGuid().ToString('N'))
    [IO.Directory]::CreateDirectory($work) | Out-Null
    $release = "$base/releases/latest/download"
    if ($Version) { $release = "$base/releases/download/$Version" }
    if ($Build) {
        if ($ArchivePath -or $ChecksumsPath) { throw '-Build cannot use prebuilt fixture options.' }
        if (-not $Version) { throw 'Source builds require -Version vX.Y.Z (a published tag).' }
        if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { throw 'Source builds need the pinned Rust toolchain and a native linker. Retry without -Build for a ready-to-run binary.' }
        $sourceZip = Join-Path $work 'source.zip'
        Get-Download "$base/archive/refs/tags/$Version.zip" $sourceZip
        $sourceRoot = Join-Path $work 'source'
        Expand-SafeZip $sourceZip $sourceRoot $false
        $sources = @(Get-ChildItem -LiteralPath $sourceRoot -Directory)
        if ($sources.Count -ne 1) { throw 'Source archive must have one root directory.' }
        $source = $sources[0].FullName
        Push-Location $source
        $previousAutoInstall = $env:RUSTUP_AUTO_INSTALL
        try {
            $env:RUSTUP_AUTO_INSTALL = '0'
            & cargo build --release --locked
            if ($LASTEXITCODE -ne 0) { throw 'Source build failed. Install the pinned Rust toolchain and native linker, or retry without -Build.' }
        } finally {
            $env:RUSTUP_AUTO_INSTALL = $previousAutoInstall
            Pop-Location
        }
        $bundle = Join-Path $work 'bundle'
        [IO.Directory]::CreateDirectory((Join-Path $bundle 'bin')) | Out-Null
        Copy-Item -LiteralPath (Join-Path $source 'target/release/herdr-codex-subagents-rs.exe') -Destination (Join-Path $bundle 'bin')
        $manifest = Get-Content -LiteralPath (Join-Path $source 'herdr-plugin.toml') -Raw
        $manifest = [regex]::Replace($manifest, '(?ms)^\[\[build\]\].*?(?=^\[|\z)', '')
        $manifest = $manifest.Replace('target/release/herdr-codex-subagents-rs', 'bin/herdr-codex-subagents-rs.exe')
        [IO.File]::WriteAllText((Join-Path $bundle 'herdr-plugin.toml'), $manifest)
        [IO.File]::WriteAllText((Join-Path $bundle 'VERSION'), $Version.TrimStart('v') + "`n")
        Copy-Item -LiteralPath (Join-Path $source 'README.md') -Destination $bundle
        Get-ChildItem -Path (Join-Path $source 'LICENSE*') -File | Copy-Item -Destination $bundle
        $ArchivePath = Join-Path $work $asset
        [IO.Compression.ZipFile]::CreateFromDirectory($bundle, $ArchivePath)
        $ChecksumsPath = Join-Path $work 'SHA256SUMS'
        [IO.File]::WriteAllText($ChecksumsPath, (Get-Sha256 $ArchivePath) + '  ' + $asset)
    } else {
        if (-not $ArchivePath) { $ArchivePath = Join-Path $work $asset; Get-Download "$release/$asset" $ArchivePath }
        if (-not $ChecksumsPath) { $ChecksumsPath = Join-Path $work 'SHA256SUMS'; Get-Download "$release/SHA256SUMS" $ChecksumsPath }
    }
    $hashes = @(Get-Content -LiteralPath $ChecksumsPath | ForEach-Object {
        if ($_ -match ('^([a-fA-F0-9]{64})\s+\*?' + [regex]::Escape($asset) + '$')) { $Matches[1] }
    })
    if ($hashes.Count -ne 1) { throw 'Checksum list must contain exactly one SHA256 for the platform asset.' }
    $actual = (Get-Sha256 $ArchivePath)
    if ($hashes[0] -ne $actual) { throw 'SHA256 mismatch; no files were installed. Download the release again.' }
    $verified = Join-Path $work 'verified'
    Expand-SafeZip $ArchivePath $verified $true
    foreach ($required in @('bin/herdr-codex-subagents-rs.exe', 'herdr-plugin.toml', 'VERSION')) {
        if (-not (Test-Path -LiteralPath (Join-Path $verified $required) -PathType Leaf)) { throw "Bundle is missing $required" }
    }
    $bundleVersion = (Get-Content -LiteralPath (Join-Path $verified 'VERSION') -Raw).Trim()
    if ($bundleVersion -notmatch '^[A-Za-z0-9][A-Za-z0-9._-]*$') { throw 'Invalid bundle VERSION.' }
    $versions = Join-Path $InstallRoot 'versions'
    Assert-PlainDirectory $versions
    $destination = Join-Path $versions ($bundleVersion + '-' + $actual.Substring(0,16))
    if (Test-Path -LiteralPath $destination) {
        Assert-SameInstall $verified $destination
    } else {
        [IO.Directory]::CreateDirectory($versions) | Out-Null
        $stage = Join-Path $versions ('.staging-' + [guid]::NewGuid().ToString('N'))
        [IO.Directory]::CreateDirectory($stage) | Out-Null
        Copy-Item -Path (Join-Path $verified '*') -Destination $stage -Recurse
        [IO.Directory]::Move($stage, $destination)
        $stage = $null
    }
    & $herdr plugin link $destination
    if ($LASTEXITCODE -ne 0) { throw "Installed at $destination but registration failed; run herdr plugin link with that directory." }
    $status = & $herdr status server 2>$null
    if ($LASTEXITCODE -eq 0 -and (($status -join "`n") -match '(?m)^status:\s+running\s*$')) {
        & $herdr plugin action invoke sm-steel.codex-subagents.setup
        if ($LASTEXITCODE -ne 0) { throw 'Registered; setup failed. Retry the setup plugin action inside your Herdr server.' }
        & $herdr plugin action invoke sm-steel.codex-subagents.sync
        if ($LASTEXITCODE -ne 0) { throw 'Registered; sync failed. Retry the sync plugin action inside your Herdr server.' }
    } else { Write-Host 'No running Herdr server found; setup and sync will run at next startup.' }
    Write-Host "Installed $bundleVersion at $destination"
    Write-Host 'Review the Codex hook normally with /hooks. Existing installations were preserved.'
} catch {
    throw ('Install failed: ' + $_.Exception.Message)
} finally {
    foreach ($temporary in @($stage, $work)) {
        if ($temporary -and (Test-Path -LiteralPath $temporary)) {
            $resolved = [IO.Path]::GetFullPath($temporary)
            $tempPrefix = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
            $stagePrefix = [IO.Path]::GetFullPath((Join-Path $InstallRoot 'versions')).TrimEnd('\') + '\'
            if (-not ($resolved.StartsWith($tempPrefix, [StringComparison]::OrdinalIgnoreCase) -or
                $resolved.StartsWith($stagePrefix, [StringComparison]::OrdinalIgnoreCase))) {
                throw 'Refusing cleanup outside the temporary or staging directory.'
            }
            Remove-Item -LiteralPath $resolved -Recurse -Force
        }
    }
}
