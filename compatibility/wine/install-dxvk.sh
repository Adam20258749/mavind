#!/bin/sh
# Install (or remove) DXVK into a Wine prefix.
#   install-dxvk.sh <WINEPREFIX> [--uninstall]
# Version is pinned in /usr/lib/mavind/wine/dxvk.version.
set -eu

PREFIX="${1:?usage: install-dxvk.sh <WINEPREFIX> [--uninstall]}"
MODE="${2:-install}"
VER="$(cat /usr/lib/mavind/wine/dxvk.version 2>/dev/null || echo 2.4.1)"
CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/mavind/dxvk"
BASE="https://github.com/doitsujin/dxvk/releases/download"
TARBALL="dxvk-${VER}.tar.gz"

export WINEPREFIX="$PREFIX"
[ -d "$PREFIX" ] || { echo "no such prefix: $PREFIX" >&2; exit 1; }

dlls="d3d9 d3d10core d3d11 dxgi"

if [ "$MODE" = "--uninstall" ] || [ "$MODE" = "uninstall" ]; then
    echo "Removing DXVK from $PREFIX (restoring WineD3D)…"
    for d in $dlls; do
        for sub in system32 syswow64; do
            f="$PREFIX/drive_c/windows/$sub/$d.dll"
            [ -e "$f" ] && rm -f "$f" || true
        done
        wine reg delete "HKCU\\Software\\Wine\\DllOverrides" /v "$d" /f >/dev/null 2>&1 || true
    done
    wineboot -u >/dev/null 2>&1 || true
    echo "done."
    exit 0
fi

command -v vulkaninfo >/dev/null 2>&1 && vulkaninfo >/dev/null 2>&1 || \
    echo "warning: no working Vulkan ICD detected — DXVK will fall back to software or fail."

mkdir -p "$CACHE"
if [ ! -f "$CACHE/$TARBALL" ]; then
    echo "Downloading DXVK $VER…"
    curl -fL --retry 3 -o "$CACHE/$TARBALL" "$BASE/v${VER}/${TARBALL}"
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
tar -C "$TMP" -xf "$CACHE/$TARBALL"
SRC="$TMP/dxvk-${VER}"

copy() { # <arch-dir> <win-dir>
    [ -d "$SRC/$1" ] || return 0
    mkdir -p "$PREFIX/drive_c/windows/$2"
    for d in $dlls; do
        [ -f "$SRC/$1/$d.dll" ] && cp -f "$SRC/$1/$d.dll" "$PREFIX/drive_c/windows/$2/$d.dll"
    done
}
copy x64 system32
copy x32 syswow64

for d in $dlls; do
    wine reg add "HKCU\\Software\\Wine\\DllOverrides" /v "$d" /d native /f >/dev/null 2>&1
done

echo "DXVK $VER installed into $PREFIX."
