#!/usr/bin/env bash
# Stage 30 — the aggressive size pass (spec §10).
set -euo pipefail
# shellcheck source=scripts/lib/common.sh
source "$(dirname "$0")/lib/common.sh"
need_root
[ -d "${ROOTFS}" ] || die "no rootfs"

before="$(du -sb "${ROOTFS}" | cut -f1)"
log "rootfs before shrink: $(human "${before}")"

# ---------------------------------------------------------------------------
step "remove docs / man / info / examples / headers / static libs"
rm -rf \
  "${ROOTFS}/usr/share/doc"/* \
  "${ROOTFS}/usr/share/man"/* \
  "${ROOTFS}/usr/share/info"/* \
  "${ROOTFS}/usr/share/gtk-doc" \
  "${ROOTFS}/usr/share/help"/* \
  "${ROOTFS}/usr/share/lintian" \
  "${ROOTFS}/usr/share/bug" \
  "${ROOTFS}/usr/include"/* \
  "${ROOTFS}/usr/share/common-licenses"/* 2>/dev/null || true
find "${ROOTFS}/usr" -name '*.a' -delete 2>/dev/null || true
find "${ROOTFS}/usr/lib" -name '*.la' -delete 2>/dev/null || true
# keep one copyright per package for licence compliance
find "${ROOTFS}/usr/share/doc" -type f ! -name copyright -delete 2>/dev/null || true

# ---------------------------------------------------------------------------
step "purge locales (keep C + en)"
if [ -d "${ROOTFS}/usr/share/locale" ]; then
  find "${ROOTFS}/usr/share/locale" -mindepth 1 -maxdepth 1 -type d \
    ! -name 'en' ! -name 'en_US' ! -name 'en_GB' ! -name 'C' -exec rm -rf {} + 2>/dev/null || true
fi
if [ -d "${ROOTFS}/usr/share/i18n/locales" ]; then
  find "${ROOTFS}/usr/share/i18n/locales" -type f \
    ! -name 'C' ! -name 'en_US' ! -name 'en_GB' ! -name 'i18n*' ! -name 'iso14651_t1*' \
    ! -name 'translit*' -delete 2>/dev/null || true
fi
rm -rf "${ROOTFS}/usr/share/X11/locale" 2>/dev/null || true

# ---------------------------------------------------------------------------
step "strip ELF binaries and shared objects"
stripped=0
while IFS= read -r -d '' f; do
  case "$(file -b --mime-type "$f" 2>/dev/null)" in
    application/x-executable|application/x-pie-executable|application/x-sharedlib)
      strip --strip-unneeded "$f" 2>/dev/null && stripped=$((stripped+1)) || true
      ;;
  esac
done < <(find "${ROOTFS}/usr" "${ROOTFS}/lib" "${ROOTFS}/sbin" "${ROOTFS}/bin" -type f -print0 2>/dev/null)
log "stripped ${stripped} ELF files"

# ---------------------------------------------------------------------------
step "trim firmware to whitelist"
FW="${ROOTFS}/usr/lib/firmware"
KEEP="${REPO_ROOT}/kernel/firmware-keep.list"
if [ -d "${FW}" ] && [ -f "${KEEP}" ]; then
  keepdir="$(mktemp -d)"
  while read -r pat; do
    [ -n "${pat}" ] || continue
    ( cd "${FW}" && find . -path "./${pat}" -print 2>/dev/null ) >> "${keepdir}/list" || true
  done < <(read_list "${KEEP}")
  if [ -s "${keepdir}/list" ]; then
    mkdir -p "${keepdir}/fw"
    rsync -a --files-from="${keepdir}/list" "${FW}/" "${keepdir}/fw/" 2>/dev/null || \
      tar -C "${FW}" -cf - -T "${keepdir}/list" | tar -C "${keepdir}/fw" -xf -
    rm -rf "${FW:?}"/*
    cp -a "${keepdir}/fw/." "${FW}/"
    # regulatory.db is essential for Wi-Fi channels
    log "firmware kept: $(find "${FW}" -type f | wc -l) files, $(du -sh "${FW}" | cut -f1)"
  else
    warn "firmware whitelist matched nothing — keeping full firmware set"
  fi
  rm -rf "${keepdir}"
fi

# ---------------------------------------------------------------------------
step "kernel modules"
MODROOT="${ROOTFS}/usr/lib/modules"
[ -d "${MODROOT}" ] || MODROOT="${ROOTFS}/lib/modules"
if [ "${MAVIND_AGGRESSIVE}" = "1" ] && [ -f "${REPO_ROOT}/kernel/modules-keep.list" ]; then
  warn "--aggressive: pruning kernel modules by keep-list (test hardware after!)"
  for kv in "${MODROOT}"/*; do
    [ -d "${kv}" ] || continue
    "${SCRIPTS_DIR}/lib/prune-modules.sh" "${kv}" "${REPO_ROOT}/kernel/modules-keep.list"
  done
else
  log "keeping all kernel modules (pass --aggressive to prune)"
fi
# always recompress + rebuild dep info in chroot
trap 'chroot_umount "${ROOTFS}"' EXIT
chroot_mount "${ROOTFS}"
for kv in "${MODROOT}"/*; do
  [ -d "${kv}" ] || continue
  in_chroot "${ROOTFS}" depmod "$(basename "${kv}")" 2>/dev/null || true
done

# ---------------------------------------------------------------------------
# NOTE: Mesa DRI-driver pruning is DISABLED for now. Getting the keep-list wrong
# breaks graphics on real GPUs and VMs (e.g. dropping vmwgfx_dri.so kills VMSVGA
# in VirtualBox/VMware). Re-enable with a per-GPU tested list once Phase 4
# hardware testing is done. Savings would be ~30-50 MB.
if [ "${MAVIND_PRUNE_DRI:-0}" = "1" ]; then
  step "drop unused Mesa DRI drivers (MAVIND_PRUNE_DRI=1)"
  DRI="${ROOTFS}/usr/lib/${MAVIND_ARCH}-linux-gnu/dri"
  [ -d "${DRI}" ] || DRI="${ROOTFS}/usr/lib/x86_64-linux-gnu/dri"
  if [ -d "${DRI}" ]; then
    find "${DRI}" -maxdepth 1 -type f -name '*_dri.so' \
      ! -name 'iris_dri.so' ! -name 'crocus_dri.so' ! -name 'i965_dri.so' \
      ! -name 'radeonsi_dri.so' ! -name 'r600_dri.so' ! -name 'nouveau_dri.so' \
      ! -name 'swrast_dri.so' ! -name 'kms_swrast_dri.so' \
      ! -name 'virtio_gpu_dri.so' ! -name 'vmwgfx_dri.so' ! -name 'zink_dri.so' \
      -delete 2>/dev/null || true
    log "DRI drivers kept: $(find "${DRI}" -name '*_dri.so' | wc -l)"
  fi
fi

# ---------------------------------------------------------------------------
step "clean caches, logs, apt state"
in_chroot "${ROOTFS}" bash -c '
  apt-get clean
  dpkg --clear-avail || true
  rm -rf /var/lib/apt/lists/* /var/cache/* /var/tmp/* /tmp/* /var/log/* \
         /root/.cache /root/.bash_history /usr/share/mime/application \
         2>/dev/null || true
  find /var/log -type f -exec truncate -s0 {} + 2>/dev/null || true
  ldconfig
'
# python bytecode, if any python slipped in
find "${ROOTFS}" -name '__pycache__' -type d -prune -exec rm -rf {} + 2>/dev/null || true
find "${ROOTFS}" -name '*.pyc' -delete 2>/dev/null || true

chroot_umount "${ROOTFS}"
trap - EXIT

after="$(du -sb "${ROOTFS}" | cut -f1)"
log "rootfs after shrink:  $(human "${after}")  (saved $(human $((before - after))))"

# write per-dir sizes for the measurement tools / STATUS
du -x -d3 -b "${ROOTFS}" 2>/dev/null | sort -rn | head -60 \
  | awk -v h="$(command -v numfmt)" '{ "numfmt --to=iec --suffix=B " $1 | getline s; print s "\t" $2 }' \
  > "${MAVIND_OUT_ISO:-${OUT_DIR}/Mavind.iso}.sizes" || \
  du -x -d3 -h "${ROOTFS}" | sort -rh | head -60 > "${MAVIND_OUT_ISO:-${OUT_DIR}/Mavind.iso}.sizes"

log "stage 30 complete"
