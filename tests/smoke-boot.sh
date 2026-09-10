#!/usr/bin/env bash
# Headless boot smoke test: boot the ISO in QEMU and wait for a marker that
# proves the base system came up. Exits 0 on success, non-zero on timeout/panic.
# Used by CI (.github/workflows/build.yml).
#
#   tests/smoke-boot.sh build/out/Mavind.iso [timeout_seconds]
#
# The ISO's GRUB entry already puts the kernel console on ttyS0, so we do NOT
# pass -append here (that flag is only valid with -kernel and makes QEMU refuse
# to start when booting from a CD).
set -euo pipefail
ISO="${1:?usage: smoke-boot.sh <iso> [timeout]}"
TIMEOUT="${2:-300}"
LOG="$(mktemp)"
trap 'rm -f "$LOG"' EXIT

command -v qemu-system-x86_64 >/dev/null || { echo "need qemu-system-x86"; exit 2; }

# Prefer UEFI if OVMF is around; the CI image installs `ovmf`.
PFLASH=()
FIRMWARE="BIOS"
for c in /usr/share/OVMF/OVMF_CODE_4M.fd /usr/share/OVMF/OVMF_CODE.fd /usr/share/ovmf/OVMF.fd; do
  if [ -f "$c" ]; then
    v="$(mktemp)"
    for vv in /usr/share/OVMF/OVMF_VARS_4M.fd /usr/share/OVMF/OVMF_VARS.fd; do
      [ -f "$vv" ] && { cp "$vv" "$v"; break; }
    done
    [ -s "$v" ] || cp "$c" "$v"   # last resort
    PFLASH=(-drive if=pflash,format=raw,unit=0,readonly=on,file="$c"
            -drive if=pflash,format=raw,unit=1,file="$v")
    FIRMWARE="UEFI ($c)"
    break
  fi
done

echo "smoke-boot: booting $ISO  firmware=$FIRMWARE  timeout=${TIMEOUT}s"
set +e
timeout --foreground "$TIMEOUT" qemu-system-x86_64 \
  -machine q35,accel=kvm:tcg -cpu max -smp 2 -m 2048 \
  "${PFLASH[@]}" \
  -drive file="$ISO",media=cdrom,readonly=on,id=cd0 \
  -boot d \
  -netdev user,id=n0 -device virtio-net-pci,netdev=n0 \
  -display none -serial stdio -no-reboot \
  2>&1 | tee "$LOG" | sed 's/^/  vm| /' &
QEMU_PID=$!

# Any of these means the ISO booted a working Linux userspace.
READY='mavind-session: starting labwc|Reached target Graphical Interface|Reached target .*Multi-User|Reached target Basic System|mavind login:|Mavind .* tty|mavind: first boot setup'
FATAL='Kernel panic|end Kernel panic|Oops:|BUG: unable to handle|Boot failed:|No bootable device|SeaBIOS.*cannot'

RESULT=2
for _ in $(seq 1 "$TIMEOUT"); do
  if grep -qE "$READY" "$LOG"; then RESULT=0; break; fi
  if grep -qE "$FATAL" "$LOG"; then RESULT=1; break; fi
  kill -0 "$QEMU_PID" 2>/dev/null || { RESULT=1; break; }
  sleep 1
done
kill "$QEMU_PID" 2>/dev/null || true
wait "$QEMU_PID" 2>/dev/null || true
set -e

echo "---- last 40 lines of serial ----"
tail -40 "$LOG" | sed 's/^/  vm| /'
echo "---------------------------------"
case "$RESULT" in
  0) echo "smoke-boot: PASS — Mavind booted to a working userspace";;
  1) echo "smoke-boot: FAIL — panic / no bootable device / early exit";;
  *) echo "smoke-boot: FAIL — no ready marker within ${TIMEOUT}s";;
esac
exit "$RESULT"
