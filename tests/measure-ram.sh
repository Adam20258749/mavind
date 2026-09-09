#!/usr/bin/env bash
# Boot the ISO headless, let the desktop settle, then pull real memory numbers
# over the serial console. Compares idle "Used" to the budget in docs/RAM-BUDGET.md.
#
#   tests/measure-ram.sh [build/out/Mavind.iso] [settle_seconds]
#
set -euo pipefail
ISO="${1:-build/out/Mavind.iso}"
SETTLE="${2:-45}"
BUDGET_IDLE_MIB=700
[ -f "$ISO" ] || { echo "no ISO at $ISO"; exit 1; }
command -v qemu-system-x86_64 >/dev/null || { echo "need qemu-system-x86"; exit 2; }

OUT="$(mktemp)"; trap 'rm -f "$OUT"' EXIT

# Autologin lands us on tty1 which starts the desktop; we also get a serial
# getty. We send `free -m` etc. over serial after SETTLE seconds.
(
  sleep "$SETTLE"
  printf '\n'
  printf 'root\n'                       # (serial getty; live has passwordless? no) -> may fail, fine
  printf 'free -m; echo __MEM__; cat /proc/loadavg; echo __LOAD__; system-analyze 2>/dev/null; echo __DONE__\n'
  sleep 8
) | timeout 240 qemu-system-x86_64 \
      -machine q35,accel=kvm:tcg -cpu max -smp 2 -m 2048 \
      -drive file="$ISO",media=cdrom,readonly=on -boot d \
      -netdev user,id=n0 -device virtio-net-pci,netdev=n0 \
      -nographic -serial mon:stdio -append "console=ttyS0" \
      2>&1 | tee "$OUT" | sed 's/^/  vm| /' || true

echo
echo "== RAM report =="
if grep -q "__MEM__" "$OUT"; then
  used=$(awk '/^Mem:/ {print $3}' "$OUT" | tail -1)
  total=$(awk '/^Mem:/ {print $2}' "$OUT" | tail -1)
  echo "MemTotal:  ${total:-?} MiB"
  echo "Used:      ${used:-?} MiB   (idle desktop)"
  if [ -n "${used:-}" ] && [ "${used:-99999}" -le "$BUDGET_IDLE_MIB" ]; then
    echo "Budget:    ${BUDGET_IDLE_MIB} MiB idle  -> PASS"
  else
    echo "Budget:    ${BUDGET_IDLE_MIB} MiB idle  -> OVER / not captured (see docs/RAM-BUDGET.md)"
  fi
else
  echo "Could not read memory over serial (serial login may be disabled in this build)."
  echo "Use the on-screen System Monitor instead, or enable a serial getty for testing."
  exit 1
fi
