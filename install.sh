#!/bin/sh
# Install the published runtime bundle. No Rust toolchain is needed by default.
set -eu
fail() { printf '%s\n' "Install failed: $*" >&2; exit 1; }
base=https://github.com/sm-steel/herdr-codex-subagents-rs
archive= checksums= root= build=0 tag=
while [ "$#" -gt 0 ]; do
    case "$1" in
        --build) build=1; shift ;;
        --archive-path|--checksums-path|--install-root|--version)
            [ "$#" -ge 2 ] || fail "Missing value for $1"
            case "$1" in --archive-path) archive=$2;; --checksums-path) checksums=$2;; --install-root) root=$2;; --version) tag=$2;; esac
            shift 2 ;;
        *) fail "Unknown option: $1" ;;
    esac
done
[ "$(uname -s)" = Linux ] || fail 'Only Linux is supported by install.sh; use install.ps1 on Windows.'
case "$(uname -m)" in x86_64|amd64) ;; *) fail 'Only x86_64 is supported by published bundles.';; esac
herdr=${HERDR_BIN_PATH:-herdr}
command -v "$herdr" >/dev/null 2>&1 || fail 'Install Herdr 0.9.5 or newer first, then retry.'
for tool in tar awk sha256sum mktemp find diff; do command -v "$tool" >/dev/null 2>&1 || fail "Required command missing: $tool"; done
[ -n "$root" ] || root=${XDG_DATA_HOME:-"$HOME/.local/share"}/herdr-codex-subagents
case "$root" in /*) ;; *) fail '--install-root must be absolute.';; esac
[ ! -L "$root" ] || fail 'Installation root must not be a symlink.'
work=$(mktemp -d)
stage=
cleanup() { [ -z "$stage" ] || rm -rf -- "$stage"; rm -rf -- "$work"; }
trap cleanup EXIT HUP INT TERM
download() { command -v curl >/dev/null 2>&1 || fail 'curl is required for downloads.'; curl --proto '=https' --tlsv1.2 -fsSL "$1" -o "$2" || fail 'Download failed; check network access and the release version.'; }
asset=herdr-codex-subagents-rs-x86_64-unknown-linux-musl.tar.gz
if [ -n "$tag" ]; then case "$tag" in v[0-9]*|[0-9]*) ;; *) fail 'Use a numeric release tag, for example v0.1.0.';; esac; case "$tag" in *[!a-zA-Z0-9._-]*) fail 'Invalid release tag.';; esac; fi
release=$base/releases/latest/download
[ -z "$tag" ] || release=$base/releases/download/$tag
validate_tar() {
    tar -tzf "$1" > "$work/names" || fail 'Invalid tar archive.'
    awk 'BEGIN {bad=0} /^\// || /\\/ || /(^|\/)\.\.?(\/|$)/ {bad=1} END {exit bad}' "$work/names" || fail 'Unsafe archive path.'
    tar -tvzf "$1" > "$work/types" || fail 'Invalid tar metadata.'
    awk 'substr($0,1,1)!="-" && substr($0,1,1)!="d" {bad=1} END {exit bad}' "$work/types" || fail 'Archive links and special files are forbidden.'
}
if [ "$build" -eq 1 ]; then
    [ -z "$archive$checksums" ] || fail '--build cannot use prebuilt fixture options.'
    command -v cargo >/dev/null 2>&1 || fail 'Source builds need the pinned Rust toolchain and a native linker. Retry without --build for a ready-to-run binary.'
    [ -n "$tag" ] || fail 'Source builds require --version vX.Y.Z (a published tag).'
    download "$base/archive/refs/tags/$tag.tar.gz" "$work/source.tar.gz"
    validate_tar "$work/source.tar.gz"
    mkdir "$work/source"
    tar -xzf "$work/source.tar.gz" --strip-components=1 -C "$work/source"
    (cd "$work/source" && RUSTUP_AUTO_INSTALL=0 cargo build --release --locked) || fail 'Source build failed. Install the pinned Rust toolchain and native linker, or retry without --build.'
    mkdir -p "$work/bundle/bin"
    cp "$work/source/target/release/herdr-codex-subagents-rs" "$work/bundle/bin/"
    awk '/^\[\[build\]\]/{skip=1;next} /^\[/{skip=0} !skip {gsub("target/release/","bin/");print}' "$work/source/herdr-plugin.toml" > "$work/bundle/herdr-plugin.toml"
    cp "$work/source/README.md" "$work/source"/LICENSE* "$work/bundle/"
    printf '%s\n' "${tag#v}" > "$work/bundle/VERSION"
    # Release archives deliberately have no ./ prefix.
    tar -czf "$work/$asset" -C "$work/bundle" bin herdr-plugin.toml VERSION README.md $(cd "$work/bundle" && printf '%s ' LICENSE*)
    archive=$work/$asset
    sha256sum "$archive" | awk -v n="$asset" '{print $1 "  " n}' > "$work/SHA256SUMS"
    checksums=$work/SHA256SUMS
else
    if [ -z "$archive" ]; then download "$release/$asset" "$work/$asset"; archive=$work/$asset; fi
    if [ -z "$checksums" ]; then download "$release/SHA256SUMS" "$work/SHA256SUMS"; checksums=$work/SHA256SUMS; fi
fi
expected=$(awk -v n="$asset" '{sub(/\r$/, "")} $2==n || $2=="*" n {print $1}' "$checksums")
[ "${#expected}" -eq 64 ] || fail 'Checksum list must contain exactly one SHA256 for the platform asset.'
case "$expected" in *[!a-fA-F0-9]*) fail 'Invalid SHA256 checksum.';; esac
actual=$(sha256sum "$archive" | awk '{print $1}')
[ "$(printf '%s' "$expected" | tr A-F a-f)" = "$actual" ] || fail 'SHA256 mismatch; no files were installed. Download the release again.'
validate_tar "$archive"
awk '!/^(bin\/?|bin\/herdr-codex-subagents-rs|herdr-plugin\.toml|VERSION|README\.md|LICENSE[A-Za-z0-9._-]*)$/ {bad=1} END {exit bad}' "$work/names" || fail 'Unexpected bundle entry.'
[ "$(sort "$work/names" | uniq -d | wc -l)" -eq 0 ] || fail 'Duplicate archive entries.'
mkdir "$work/verified"
tar -xzf "$archive" -C "$work/verified"
for file in bin/herdr-codex-subagents-rs herdr-plugin.toml VERSION; do [ -f "$work/verified/$file" ] || fail "Bundle is missing $file"; done
version=$(awk '{sub(/\r$/, ""); print}' "$work/verified/VERSION")
case "$version" in ''|*[!a-zA-Z0-9._-]*|.*) fail 'Invalid bundle VERSION.';; esac
[ -x "$work/verified/bin/herdr-codex-subagents-rs" ] || fail 'Bundle binary is not executable.'
mkdir -p "$root/versions"
[ ! -L "$root/versions" ] || fail 'versions directory must not be a symlink.'
dest=$root/versions/$version-$(printf '%.16s' "$actual")
if [ -e "$dest" ] || [ -L "$dest" ]; then
    [ -d "$dest" ] && [ ! -L "$dest" ] || fail 'Existing installation differs from the verified bundle; it was preserved.'
    unsafe=$(find "$dest" ! -type f ! -type d -print -quit)
    [ -z "$unsafe" ] || fail 'Existing installation differs from the verified bundle (link or special file); it was preserved.'
    diff -r "$work/verified" "$dest" >/dev/null 2>&1 || fail 'Existing installation differs from the verified bundle; it was preserved. Choose another --install-root or inspect the existing installation.'
    [ -x "$dest/bin/herdr-codex-subagents-rs" ] || fail 'Existing installation differs: binary is not executable; it was preserved.'
else
    stage=$(mktemp -d "$root/versions/.staging.XXXXXX")
    cp -R "$work/verified/." "$stage/"
    mv -T "$stage" "$dest"
    stage=
fi
"$herdr" plugin link "$dest" || fail "Installed at $dest but registration failed; run herdr plugin link with that directory."
status_ok=0
status=$("$herdr" status server 2>/dev/null) && status_ok=1
if [ "$status_ok" -eq 1 ] && printf '%s\n' "$status" | awk '$1=="status:" && $2=="running" {found=1} END {exit !found}'; then
    "$herdr" plugin action invoke sm-steel.codex-subagents.setup || fail 'Registered; setup failed. Retry the setup plugin action inside your Herdr server.'
    "$herdr" plugin action invoke sm-steel.codex-subagents.sync || fail 'Registered; sync failed. Retry the sync plugin action inside your Herdr server.'
else
    printf '%s\n' 'No running Herdr server found; setup and sync will run at next startup.'
fi
printf '%s\n' "Installed $version at $dest" 'Review the Codex hook normally with /hooks. Existing installations were preserved.'
