#!/usr/bin/env bash
# Stage 00 — bootstrap a minimal Debian rootfs and install the package tiers.
set -euo pipefail
# shellcheck source=scripts/lib/common.sh
source "$(dirname "$0")/lib/common.sh"
need_root

PKG_DIR="${REPO_ROOT}/system/packages"
OUT_ISO="${MAVIND_OUT_ISO:-${OUT_DIR}/Mavind.iso}"

INCLUDE="$(read_list "${PKG_DIR}/core.list" | paste -sd, -)"
COMPONENTS="main contrib non-free-firmware"

step "mmdebstrap ${MAVIND_SUITE} (${MAVIND_ARCH}) -> ${ROOTFS}"
log "components:    ${COMPONENTS}"
log "core packages: $(read_list "${PKG_DIR}/core.list" | wc -l)"

rm -rf "${ROOTFS}"
mkdir -p "${ROOTFS}" "${CACHE_DIR}/apt"

# A real hook script (NOT an exported shell function — mmdebstrap runs hooks via
# /bin/sh). It drops Mavind's permanent apt/dpkg trimming config into the target
# early, so it also governs anything installed later in this stage.
HOOK="${WORK_DIR}/setup-hook.sh"
mkdir -p "${WORK_DIR}"
cat > "${HOOK}" <<'HOOK_EOF'
#!/bin/sh
# $1 = target rootfs
set -eu
t="$1"
install -Dm644 /dev/stdin "$t/etc/dpkg/dpkg.cfg.d/01-mavind-trim" <<'CFG'
path-exclude /usr/share/doc/*
path-include /usr/share/doc/*/copyright
path-exclude /usr/share/man/*
path-exclude /usr/share/info/*
path-exclude /usr/share/groff/*
path-exclude /usr/share/lintian/*
path-exclude /usr/share/help/*
path-exclude /usr/share/locale/*
path-include /usr/share/locale/en*
path-include /usr/share/locale/locale.alias
CFG
install -Dm644 /dev/stdin "$t/etc/apt/apt.conf.d/99-mavind" <<'CFG'
APT::Install-Recommends "false";
APT::Install-Suggests "false";
APT::AutoRemove::RecommendsImportant "false";
Acquire::Languages "none";
CFG
HOOK_EOF
chmod +x "${HOOK}"

mmdebstrap \
  --arch="${MAVIND_ARCH}" \
  --variant=minbase \
  --components="${COMPONENTS}" \
  --include="${INCLUDE}" \
  --aptopt='Acquire::Retries "3"' \
  --dpkgopt='path-exclude=/usr/share/doc/*' \
  --dpkgopt='path-include=/usr/share/doc/*/copyright' \
  --dpkgopt='path-exclude=/usr/share/man/*' \
  --dpkgopt='path-exclude=/usr/share/info/*' \
  --dpkgopt='path-exclude=/usr/share/locale/*' \
  --dpkgopt='path-include=/usr/share/locale/en*' \
  --setup-hook="${HOOK}" \
  --customize-hook='chroot "$1" update-alternatives --set editor /bin/nano 2>/dev/null || true' \
  --skip=cleanup/apt \
  --format=directory \
  "${MAVIND_SUITE}" "${ROOTFS}" "${MAVIND_MIRROR}"

log "core rootfs: $(du -sh --apparent-size "${ROOTFS}" | cut -f1)"

# ---------------------------------------------------------------------------
# Always give the image a sane sources.list (mmdebstrap's may be minimal).
# ---------------------------------------------------------------------------
cat > "${ROOTFS}/etc/apt/sources.list" <<EOF
deb ${MAVIND_MIRROR} ${MAVIND_SUITE} ${COMPONENTS}
deb ${MAVIND_MIRROR} ${MAVIND_SUITE}-updates ${COMPONENTS}
deb http://security.debian.org/debian-security ${MAVIND_SUITE}-security ${COMPONENTS}
EOF

# ---------------------------------------------------------------------------
# Tier packages need apt INSIDE the chroot (i386 multiarch for Wine, etc.).
# `core` installs nothing extra here, so it never touches chroot networking.
# ---------------------------------------------------------------------------
if [ "${MAVIND_PROFILE}" != "core" ]; then
  trap 'chroot_umount "${ROOTFS}"' EXIT
  chroot_mount "${ROOTFS}"

  step "compat tier: i386 multiarch + Wine set"
  in_chroot "${ROOTFS}" dpkg --add-architecture i386
  chroot_apt "${ROOTFS}" update
  mapfile -t COMPAT < <(read_list "${PKG_DIR}/compat.list")
  log "compat packages: ${#COMPAT[@]}"
  # `wine` pulls the correct i386 deps itself; don't hand-list every :i386 lib.
  chroot_apt "${ROOTFS}" install "${COMPAT[@]}"
fi

if [ "${MAVIND_PROFILE}" = "full" ]; then
  step "full profile: optional packages"
  mapfile -t OPT < <(read_list "${PKG_DIR}/optional.list")
  if [ "${#OPT[@]}" -gt 0 ]; then
    log "optional packages: ${#OPT[@]}"
    chroot_apt "${ROOTFS}" install "${OPT[@]}" || warn "some optional packages failed; continuing"
  fi
fi

# Stash the apt cache on the host for next time, then clear it in the image
# (host-side rm — no chroot needed, so the core path stays offline-safe).
cp -a "${ROOTFS}/var/cache/apt/archives/." "${CACHE_DIR}/apt/" 2>/dev/null || true
rm -rf "${ROOTFS}/var/cache/apt/archives/"*.deb \
       "${ROOTFS}/var/cache/apt/archives/partial/"* \
       "${ROOTFS}/var/lib/apt/lists/"*

if [ "${MAVIND_PROFILE}" != "core" ]; then
  chroot_umount "${ROOTFS}"
  trap - EXIT
fi

mkdir -p "$(dirname "${OUT_ISO}")"
chroot "${ROOTFS}" dpkg-query -W -f='${Package}\t${Version}\t${Architecture}\n' \
  | sort > "${OUT_ISO}.manifest"

log "stage 00 complete — $(wc -l < "${OUT_ISO}.manifest") packages, rootfs $(du -sh "${ROOTFS}" | cut -f1)"
