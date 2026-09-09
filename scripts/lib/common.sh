# shellcheck shell=bash
# Common helpers for the Mavind build pipeline. Sourced by every stage script.

set -euo pipefail

# --- paths -------------------------------------------------------------------
# REPO_ROOT is the directory containing this repo (one level above scripts/).
SCRIPTS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="$(cd "${SCRIPTS_DIR}/.." && pwd)"
export SCRIPTS_DIR REPO_ROOT

BUILD_DIR="${REPO_ROOT}/build"
WORK_DIR="${BUILD_DIR}/work"
CACHE_DIR="${BUILD_DIR}/cache"
OUT_DIR="${BUILD_DIR}/out"
ROOTFS="${WORK_DIR}/rootfs"
ISO_TREE="${WORK_DIR}/iso"
export BUILD_DIR WORK_DIR CACHE_DIR OUT_DIR ROOTFS ISO_TREE

# --- build parameters (overridable by env / build-iso.sh) ------------------
: "${MAVIND_PROFILE:=compat}"          # core | compat | full
: "${MAVIND_ARCH:=amd64}"
: "${MAVIND_SUITE:=trixie}"
: "${MAVIND_MIRROR:=http://deb.debian.org/debian}"
: "${MAVIND_VOLID:=MAVIND}"
: "${MAVIND_AGGRESSIVE:=0}"            # 1 = prune kernel modules by keep-list
: "${MAVIND_KEEP_WORK:=0}"
export MAVIND_PROFILE MAVIND_ARCH MAVIND_SUITE MAVIND_MIRROR MAVIND_VOLID MAVIND_AGGRESSIVE

# Reproducibility: pin timestamps to the last commit if we're in a git tree.
if [ -z "${SOURCE_DATE_EPOCH:-}" ]; then
  if git -C "${REPO_ROOT}" rev-parse --git-dir >/dev/null 2>&1; then
    SOURCE_DATE_EPOCH="$(git -C "${REPO_ROOT}" log -1 --pretty=%ct 2>/dev/null || date -u +%s)"
  else
    SOURCE_DATE_EPOCH="$(date -u +%s)"
  fi
fi
export SOURCE_DATE_EPOCH

# --- logging ---------------------------------------------------------------
_c() { case "${MAVIND_NO_COLOR:-0}" in 1) printf '';; *) printf '\033[%sm' "$1";; esac; }
log()   { printf '%s[mavind]%s %s\n'  "$(_c '1;36')" "$(_c 0)" "$*"; }
step()  { printf '\n%s==>%s %s%s%s\n' "$(_c '1;35')" "$(_c 0)" "$(_c 1)" "$*" "$(_c 0)"; }
warn()  { printf '%s[warn]%s %s\n'    "$(_c '1;33')" "$(_c 0)" "$*" >&2; }
die()   { printf '%s[fail]%s %s\n'    "$(_c '1;31')" "$(_c 0)" "$*" >&2; exit 1; }

# --- guards --------------------------------------------------------------
need_root() {
  [ "$(id -u)" -eq 0 ] || die "stage $(basename "$0") must run as root (use sudo, or scripts/build.sh for the container)"
}

need_cmd() {
  local missing=0 c
  for c in "$@"; do
    command -v "$c" >/dev/null 2>&1 || { warn "missing command: $c"; missing=1; }
  done
  [ "$missing" -eq 0 ] || die "install the missing build dependencies (see docs/BUILD.md)"
}

need_linux() {
  [ "$(uname -s)" = "Linux" ] || die "the Mavind build must run on Linux or in a Linux container (see docs/BUILD.md)"
}

# --- chroot helpers ----------------------------------------------------
CHROOT_MOUNTS=()
chroot_mount() {
  local d="$1"
  mount --bind /dev      "${d}/dev"
  mount --bind /dev/pts  "${d}/dev/pts"
  mount -t proc  proc    "${d}/proc"
  mount -t sysfs sys     "${d}/sys"
  mount -t tmpfs tmpfs   "${d}/run"
  CHROOT_MOUNTS=("${d}/run" "${d}/sys" "${d}/proc" "${d}/dev/pts" "${d}/dev")
  # resolv.conf for apt inside the chroot
  cp -f /etc/resolv.conf "${d}/etc/resolv.conf" 2>/dev/null || true
}
chroot_umount() {
  local m
  for m in "${CHROOT_MOUNTS[@]:-}"; do
    mountpoint -q "$m" && umount -l "$m" || true
  done
  CHROOT_MOUNTS=()
}
in_chroot() {
  local d="$1"; shift
  chroot "$d" /usr/bin/env -i \
    HOME=/root PATH=/usr/sbin:/usr/bin:/sbin:/bin TERM="${TERM:-xterm}" \
    DEBIAN_FRONTEND=noninteractive LC_ALL=C LANG=C \
    SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH}" \
    "$@"
}

# Run apt inside the chroot with all the size-saving flags.
chroot_apt() {
  local d="$1"; shift
  in_chroot "$d" apt-get -y \
    -o Acquire::Retries=3 \
    -o APT::Install-Recommends=false \
    -o APT::Install-Suggests=false \
    -o Dpkg::Use-Pty=0 \
    "$@"
}

# --- misc ------------------------------------------------------------
read_list() {
  # print a package list file with comments and blank lines stripped
  local f="$1"
  [ -f "$f" ] || die "package list not found: $f"
  sed -e 's/#.*$//' -e '/^[[:space:]]*$/d' -e 's/[[:space:]]//g' "$f"
}

human() { numfmt --to=iec --suffix=B "${1:-0}" 2>/dev/null || echo "${1:-0}B"; }

trap_cleanup() {
  # every stage that mounts should `trap trap_cleanup EXIT`
  chroot_umount "${ROOTFS}" 2>/dev/null || true
}
