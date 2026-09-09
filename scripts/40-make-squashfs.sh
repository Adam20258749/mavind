#!/usr/bin/env bash
# Stage 40 — build the live initramfs and compress the rootfs into a squashfs.
set -euo pipefail
# shellcheck source=scripts/lib/common.sh
source "$(dirname "$0")/lib/common.sh"
need_root
need_cmd mksquashfs
[ -d "${ROOTFS}" ] || die "no rootfs"

LIVE="${WORK_DIR}/live"
mkdir -p "${LIVE}"

# ---------------------------------------------------------------------------
step "regenerate initramfs with live-boot hooks"
trap 'chroot_umount "${ROOTFS}"' EXIT
chroot_mount "${ROOTFS}"

# live-boot must be present (it's in core.list). It adds the initramfs hook that
# finds our ISO by volume id and sets up the overlay.
in_chroot "${ROOTFS}" bash -c '
  set -e
  export LIVE_BOOT_MOUNT_POINTS=""
  update-initramfs -u -k all 2>&1 | tail -5 || update-initramfs -c -k all
'

KVER="$(basename "$(find "${ROOTFS}/usr/lib/modules" -maxdepth 1 -mindepth 1 -type d | sort | tail -1)")"
[ -n "${KVER}" ] || KVER="$(basename "$(find "${ROOTFS}/lib/modules" -maxdepth 1 -mindepth 1 -type d | sort | tail -1)")"
log "kernel version: ${KVER}"

cp "${ROOTFS}/boot/vmlinuz-${KVER}"    "${LIVE}/vmlinuz"
cp "${ROOTFS}/boot/initrd.img-${KVER}" "${LIVE}/initrd.img"
echo "${KVER}" > "${LIVE}/kernel.version"

chroot_umount "${ROOTFS}"
trap - EXIT

# ---------------------------------------------------------------------------
step "mksquashfs (zstd, level 19, 1 MiB blocks)"
SQUASH="${LIVE}/filesystem.squashfs"
rm -f "${SQUASH}"

# Exclude /boot (kernel+initrd live outside the squashfs) and volatile dirs.
mksquashfs "${ROOTFS}" "${SQUASH}" \
  -comp zstd -Xcompression-level 19 \
  -b 1M -noappend -no-recovery -no-exports \
  -wildcards \
  -e "boot/*" \
  -e "var/cache/apt/archives/*.deb" \
  -e "var/lib/apt/lists/*" \
  -e "tmp/*" -e "var/tmp/*" -e "var/log/*" \
  -e "root/.cache" -e "home/*/.cache" \
  ${SOURCE_DATE_EPOCH:+-mkfs-time "${SOURCE_DATE_EPOCH}" -all-time "${SOURCE_DATE_EPOCH}"}

printf '%s' "$(stat -c%s "${SQUASH}")" > "${LIVE}/filesystem.size"
( cd "${LIVE}" && sha256sum filesystem.squashfs > filesystem.squashfs.sha256 )

rootfs_b="$(du -sb "${ROOTFS}" | cut -f1)"
squash_b="$(stat -c%s "${SQUASH}")"
log "rootfs (installed): $(human "${rootfs_b}")"
log "squashfs:           $(human "${squash_b}")  (ratio $(awk "BEGIN{printf \"%.2fx\", ${rootfs_b}/${squash_b}}"))"

# Quick reality check vs the ~1 GB target (installed size, core tier).
if [ "${MAVIND_PROFILE}" = "core" ]; then
  target=$((1024*1024*1024))
  if [ "${rootfs_b}" -le "${target}" ]; then
    log "core installed size WITHIN 1 GiB target ✔"
  else
    warn "core installed size $(human "${rootfs_b}") EXCEEDS 1 GiB target — see docs/SIZE-BUDGET.md levers"
  fi
fi

log "stage 40 complete"
