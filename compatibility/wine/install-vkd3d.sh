#!/bin/sh
# Install (or remove) VKD3D-Proton (Direct3D 12 -> Vulkan) into a Wine prefix.
#   install-vkd3d.sh <WINEPREFIX> [--uninstall]
set -eu

PREFIX="${1:?usage: install-vkd3d.sh <WINEPREFIX> [--uninstall]}"
MODE="${2:-install}"
VER="$(cat /usr/lib/mavind/wine/vkd3d.version 2>/dev/null || echo 2.14.1)"
CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/mavind/vkd3d"
BASE="https://github.com/HansKristian-Work/vkd3d-proton/releases/download"
TARBALL="vkd3d-proton-${VER}.tar.zst"

export WINEPREFIX="$PREFIX"
[ -d "$PREFIX" ] || { echo "no such prefix: $PREFIX" >&2; exit 1; }
dlls="d3d12 d3d12core"

if [ "$MODE" = "--uninstall" ] || [ "$MODE" = "uninstall" ]; then
    for d in $dlls; do
        rm -f "$PREFIX/drive_c/windows/system32/$d.dll" "$PREFIX/drive_c/windows/syswow64/$d.dll" 2>/dev/null || true
        wine reg delete "HKCU\\Software\\Wine\\DllOverrides" /v "$d" /f >/dev/null 2>&1 || true
    done
    echo "VKD3D-Proton removed."
    exit 0
fi

mkdir -p "$CACHE"
[ -f "$CACHE/$TARBALL" ] || { echo "Downloading VKD3D-Proton $VER…"; curl -fL --retry 3 -o "$CACHE/$TARBALL" "$BASE/v${VER}/${TARBALL}"; }

TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT
zstd -dc "$CACHE/$TARBALL" | tar -C "$TMP" -xf -
SRC="$TMP/vkd3d-proton-${VER}"

for pair in "x64 system32" "x86 syswow64"; do
    set -- $pair
    [ -d "$SRC/$1" ] || continue
    mkdir -p "$PREFIX/drive_c/windows/$2"
    for d in $dlls; do
        [ -f "$SRC/$1/$d.dll" ] && cp -f "$SRC/$1/$d.dll" "$PREFIX/drive_c/windows/$2/$d.dll"
    done
done
for d in $dlls; do
    wine reg add "HKCU\\Software\\Wine\\DllOverrides" /v "$d" /d native /f >/dev/null 2>&1
done
echo "VKD3D-Proton $VER installed into $PREFIX."
