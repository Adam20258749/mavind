#!/usr/bin/env bash
# Report the REAL installed size of a Mavind build and compare to the ~1 GB
# core target. Reads the squashfs directly (needs root or `unsquashfs`).
#
#   tests/measure-size.sh [build/out/Mavind.iso]
#
set -euo pipefail
ISO="${1:-build/out/Mavind.iso}"
TARGET_MIB=1024

sizes_file="${ISO}.sizes"
manifest="${ISO}.manifest"

echo "== Mavind size report =="
[ -f "$ISO" ] && echo "ISO:            $(du -h "$ISO" | cut -f1)   ($ISO)"

# 1) installed size — from the squashfs if we can mount/extract it
SQ=""
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
if command -v xorriso >/dev/null 2>&1 && [ -f "$ISO" ]; then
  xorriso -osirrox on -indev "$ISO" -extract /live/filesystem.squashfs "$work/fs.squashfs" >/dev/null 2>&1 || true
  [ -f "$work/fs.squashfs" ] && SQ="$work/fs.squashfs"
fi
[ -z "$SQ" ] && [ -f build/work/live/filesystem.squashfs ] && SQ=build/work/live/filesystem.squashfs

if [ -n "$SQ" ] && command -v unsquashfs >/dev/null 2>&1; then
  bytes=$(unsquashfs -s "$SQ" | awk -F': *' '/Filesystem size/ {print $2}' | awk '{print $1*1024}')
  # that is the compressed size; get uncompressed via listing
  unsquashfs -lls "$SQ" >/dev/null 2>&1 || true
  unc=$(unsquashfs -n -d "$work/root" "$SQ" >/dev/null 2>&1 && du -sb "$work/root" | cut -f1 || echo 0)
  if [ "${unc:-0}" -gt 0 ]; then
    mib=$(( unc / 1024 / 1024 ))
    echo "Installed size:  ${mib} MiB   (uncompressed rootfs)"
    if [ "$mib" -le "$TARGET_MIB" ]; then
      echo "Target (core):   ${TARGET_MIB} MiB   -> PASS"
    else
      echo "Target (core):   ${TARGET_MIB} MiB   -> OVER by $((mib-TARGET_MIB)) MiB (see docs/SIZE-BUDGET.md)"
    fi
    echo
    echo "Top directories:"
    du -x -d2 -h "$work/root" 2>/dev/null | sort -rh | head -25
  fi
elif [ -f "$sizes_file" ]; then
  echo "(using $sizes_file from the build)"
  cat "$sizes_file"
else
  echo "Cannot measure: no squashfs and no ${sizes_file}. Run a build first."
  exit 1
fi

[ -f "$manifest" ] && echo && echo "Packages in image: $(wc -l < "$manifest")"
