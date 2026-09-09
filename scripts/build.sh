#!/usr/bin/env bash
# Cross-platform entry point: build Mavind inside a container so it works on
# Windows/macOS/Linux without installing the Debian build toolchain on the host.
#
#   ./scripts/build.sh [--profile core|compat|full] [any build-iso.sh flag]
#
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${REPO_ROOT}"

IMAGE="mavind-build:local"
ENGINE=""
for e in podman docker; do command -v "$e" >/dev/null 2>&1 && { ENGINE="$e"; break; }; done
[ -n "${ENGINE}" ] || { echo "need docker or podman (see docs/BUILD.md)" >&2; exit 1; }
echo "[build.sh] engine: ${ENGINE}"

if ! "${ENGINE}" image inspect "${IMAGE}" >/dev/null 2>&1; then
  echo "[build.sh] building ${IMAGE} ..."
  "${ENGINE}" build -t "${IMAGE}" -f build/Containerfile build/
fi

# mmdebstrap + chroot + loop mounts need elevated caps.
CAPS=(--privileged)
[ "${ENGINE}" = "podman" ] && CAPS=(--privileged --security-opt label=disable)

exec "${ENGINE}" run --rm -it \
  "${CAPS[@]}" \
  -v "${REPO_ROOT}:/work:Z" \
  -w /work \
  -e MAVIND_NO_COLOR="${MAVIND_NO_COLOR:-0}" \
  "${IMAGE}" \
  /work/scripts/build-iso.sh "$@"
