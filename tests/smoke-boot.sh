#!/usr/bin/env bash
# Headless boot smoke test: boot the ISO in QEMU with a serial console and wait
# for Mavind's "reached" marker. Exits 0 on success, non-zero on timeout/panic.
# Used by CI (.github/workflows/build.yml).
#
#   tests/smoke-boot.sh build/out/Mavind.iso [timeout_seconds]
#
set -euo pipefail
ISO="${1:?usage: smoke-boot.sh <iso> [timeout]}"
TIMEOUT="${2:-180}"
LOG="$(mktemp)"
trap 'rm -f "$LOG"' EXIT

command -v qemu-system-x86_64 >/dev/null || { echo "need qemu-system-x86"; exit 2; }

# Prefer UEFI if OVMF is around; the CI image installs `ovmf`.
PFLASH=()
for c in /usr/share/OVMF/OVMF_CODE_4M.fd /usr/share/OVMF/OVMF_CODE.fd; do
  if [ -f "$c" ]; then
    v="$(mktemp)"
    for vv in /usr/share/OVMF/OVMF_VARS_4M.fd /usr/share/OVMF/OVMF_VARS.fd; do
      [ -f "$vv" ] && { cp "$vv" "$v"; break; }
    done
    PFLASH=(-drive if=pflash,format=raw,unit=0,readonly=on,file="$c"
            -drive if=pflash,format=raw,unit=1,file="$v")
    break
  fi
done

echo "smoke-boot: booting $ISO (timeout ${TIMEOUT}s)…"
set +e
timeout --foreground "$TIMEOUT" qemu-system-x86_64 \
  -machine q35,accel=kvm:tcg -cpu max -smp 2 -m 2048 \
  "${PFLASH[@]}" \
  -drive file="$ISO",media=cdrom,readonly=on \
  -boot d \
  -netdev user,id=n0 -device virtio-net-pci,netdev=n0 \
  -nographic -serial mon:stdio \
  -append "console=ttyS0" \
  2>&1 | tee "$LOG" | sed 's/^/  qemu| /' &
QEMU_PID=$!

# Watch the log for success / failure markers.
RESULT=2
for _ in $(seq 1 "$TIMEOUT"); do
  if grep -qE "mavind-session: starting labwc|Reached target .*Graphical Interface|mavind: first boot setup complete" "$LOG"; then
    RESULT=0; break
  fi
  if grep -qE "Kernel panic|-- BEGIN Kernel|Oops:|Segmentation fault in PID 1" "$LOG"; then
    RESULT=1; break
  fi
  kill -0 "$QEMU_PID" 2>/dev/null || { RESULT=1; break; }
  sleep 1
done
kill "$QEMU_PID" 2>/dev/null || true
wait "$QEMU_PID" 2>/dev/null || true
set -e

case "$RESULT" in
  0) echo "smoke-boot: PASS — Mavind reached the graphical session";;
  1) echo "smoke-boot: FAIL — panic or early exit (see log above)";;
  *) echo "smoke-boot: FAIL — timed out after ${TIMEOUT}s without the ready marker";;
esac
exit "$RESULT"
