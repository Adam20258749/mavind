#!/usr/bin/env bash
# Mavind ISO build orchestrator.
# Runs stages 00..50. See docs/BUILD.md.
#
#   sudo ./scripts/build-iso.sh [--profile core|compat|full] [--from 30] [--aggressive] ...
#
set -euo pipefail
# shellcheck source=scripts/lib/common.sh
source "$(dirname "$0")/lib/common.sh"

FROM_STAGE="00"
OUT_ISO="${OUT_DIR}/Mavind.iso"
NO_CACHE=0

usage() {
  sed -n '2,12p' "$0" | sed 's/^# \{0,1\}//'
  cat <<EOF

Options:
  --profile core|compat|full   package tier to include   (default: ${MAVIND_PROFILE})
  --arch amd64                  target architecture       (default: ${MAVIND_ARCH})
  --suite NAME                  Debian suite              (default: ${MAVIND_SUITE})
  --mirror URL                  Debian mirror             (default: ${MAVIND_MIRROR})
  --out PATH                    output ISO                (default: ${OUT_ISO})
  --from STAGE                  resume from 00|10|20|30|40|50
  --aggressive                  prune kernel modules by keep-list
  --no-cache                    ignore build/cache
  --keep-work                   keep build/work on success
  -h, --help
EOF
}

while [ $# -gt 0 ]; do
  case "$1" in
    --profile)     MAVIND_PROFILE="$2"; shift 2;;
    --arch)        MAVIND_ARCH="$2"; shift 2;;
    --suite)       MAVIND_SUITE="$2"; shift 2;;
    --mirror)      MAVIND_MIRROR="$2"; shift 2;;
    --out)         OUT_ISO="$2"; shift 2;;
    --from)        FROM_STAGE="$2"; shift 2;;
    --aggressive)  MAVIND_AGGRESSIVE=1; shift;;
    --no-cache)    NO_CACHE=1; shift;;
    --keep-work)   MAVIND_KEEP_WORK=1; shift;;
    -h|--help)     usage; exit 0;;
    *) die "unknown option: $1 (try --help)";;
  esac
done
export MAVIND_PROFILE MAVIND_ARCH MAVIND_SUITE MAVIND_MIRROR MAVIND_AGGRESSIVE MAVIND_KEEP_WORK

case "${MAVIND_PROFILE}" in core|compat|full) ;; *) die "bad --profile: ${MAVIND_PROFILE}";; esac
case "${MAVIND_ARCH}"    in amd64) ;; *) die "only amd64 is supported right now";; esac

need_linux
need_root
need_cmd mmdebstrap chroot mksquashfs xorriso mtools grub-mkstandalone \
         cargo rustc pkg-config numfmt file zstd

# --- release version, stamped from git (changes on every commit/update) -----
_gd="$(git -C "${REPO_ROOT}" describe --tags --always --dirty 2>/dev/null || true)"
_gc="$(git -C "${REPO_ROOT}" rev-parse --short HEAD 2>/dev/null || echo unknown)"
_bd="$(date -u -d "@${SOURCE_DATE_EPOCH}" +%Y%m%d 2>/dev/null || date -u +%Y%m%d)"
: "${MAVIND_BASE_VERSION:=0.1.0}"
if [ -n "${_gd}" ] && printf '%s' "${_gd}" | grep -q '^v\?[0-9]'; then
  MAVIND_VERSION_ID="${_gd#v}"
else
  MAVIND_VERSION_ID="${MAVIND_BASE_VERSION}+${_bd}.g${_gc}"
fi
MAVIND_VERSION="${MAVIND_VERSION_ID} (${MAVIND_PROFILE}, build ${_bd})"
MAVIND_BUILD_ID="${_bd}.g${_gc}"
export MAVIND_VERSION_ID MAVIND_VERSION MAVIND_BUILD_ID

step "Mavind build  profile=${MAVIND_PROFILE} arch=${MAVIND_ARCH} suite=${MAVIND_SUITE}"
log "repo:        ${REPO_ROOT}"
log "version:     ${MAVIND_VERSION}"
log "output:      ${OUT_ISO}"
log "SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH} ($(date -u -d "@${SOURCE_DATE_EPOCH}" 2>/dev/null || true))"
[ "${NO_CACHE}" -eq 1 ] && { warn "clearing build cache"; rm -rf "${CACHE_DIR}"; }

mkdir -p "${WORK_DIR}" "${CACHE_DIR}" "${OUT_DIR}" "$(dirname "${OUT_ISO}")"

run_stage() {
  local num="$1" script="${SCRIPTS_DIR}/$2"
  if [ "$((10#${num}))" -lt "$((10#${FROM_STAGE}))" ]; then
    log "skip stage ${num} (${2}) — resuming from ${FROM_STAGE}"
    return 0
  fi
  [ -x "${script}" ] || chmod +x "${script}"
  step "stage ${num}: ${2}"
  local t0; t0="$(date +%s)"
  MAVIND_OUT_ISO="${OUT_ISO}" bash "${script}"
  log "stage ${num} done in $(( $(date +%s) - t0 ))s"
}

trap 'chroot_umount "${ROOTFS}" 2>/dev/null || true' EXIT

run_stage 00 00-bootstrap-rootfs.sh
run_stage 10 10-configure-system.sh
run_stage 20 20-build-components.sh
run_stage 30 30-strip-and-shrink.sh
run_stage 40 40-make-squashfs.sh
run_stage 50 50-make-iso.sh

step "Build complete"
if [ -f "${OUT_ISO}" ]; then
  sz=$(stat -c%s "${OUT_ISO}")
  ( cd "$(dirname "${OUT_ISO}")" && sha256sum "$(basename "${OUT_ISO}")" > "$(basename "${OUT_ISO}").sha256" )
  log "ISO:    ${OUT_ISO}  ($(human "${sz}"))"
  log "sha256: $(cut -d' ' -f1 "${OUT_ISO}.sha256")"
  [ -f "${OUT_ISO}.manifest" ] && log "manifest: ${OUT_ISO}.manifest ($(wc -l < "${OUT_ISO}.manifest") packages)"
  log "next:   ./tests/run-qemu.sh ${OUT_ISO}"
else
  die "expected ${OUT_ISO} but it is not there"
fi

if [ "${MAVIND_KEEP_WORK}" -eq 0 ]; then
  log "cleaning build/work (pass --keep-work to keep it)"
  rm -rf "${WORK_DIR}"
fi
