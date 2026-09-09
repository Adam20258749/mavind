#!/usr/bin/env bash
# Boot a Mavind ISO in QEMU with UEFI (OVMF). Interactive by default.
#
#   tests/run-qemu.sh [build/out/Mavind.iso] [--bios] [--mem 2048] [--headless]
#
set -euo pipefail
ISO="${1:-build/out/Mavind.iso}"
shift || true
[ -f "$ISO" ] || { echo "no ISO at $ISO (run scripts/build.sh first)"; exit 1; }

MEM=2048 CPUS=2 MODE=uefi DISPLAY_ARGS=(-display gtk) EXTRA=()
DISK="${TMPDIR:-/tmp}/mavind-qemu-disk.qcow2"

while [ $# -gt 0 ]; do
  case "$1" in
    --bios) MODE=bios; shift;;
    --mem) MEM="$2"; shift 2;;
    --cpus) CPUS="$2"; shift 2;;
    --headless) DISPLAY_ARGS=(-nographic); shift;;
    --disk) DISK="$2"; shift 2;;
    *) EXTRA+=("$1"); shift;;
  esac
done

command -v qemu-system-x86_64 >/dev/null || { echo "install qemu-system-x86"; exit 1; }
[ -f "$DISK" ] || qemu-img create -f qcow2 "$DISK" 8G >/dev/null

OVMF_CODE=""
for p in /usr/share/OVMF/OVMF_CODE_4M.fd /usr/share/OVMF/OVMF_CODE.fd \
         /usr/share/edk2/x64/OVMF_CODE.4m.fd /usr/share/qemu/OVMF.fd; do
  [ -f "$p" ] && { OVMF_CODE="$p"; break; }
done
VARS="${TMPDIR:-/tmp}/mavind-OVMF_VARS.fd"
for p in /usr/share/OVMF/OVMF_VARS_4M.fd /usr/share/OVMF/OVMF_VARS.fd /usr/share/edk2/x64/OVMF_VARS.4m.fd; do
  [ -f "$p" ] && { [ -f "$VARS" ] || cp "$p" "$VARS"; break; }
done

ARGS=(
  -machine q35,accel=kvm:tcg -cpu max -smp "$CPUS" -m "$MEM"
  -drive file="$DISK",if=virtio,format=qcow2
  -drive file="$ISO",media=cdrom,if=none,id=cd0 -device ide-cd,drive=cd0,bootindex=1
  -boot menu=on
  -netdev user,id=n0 -device virtio-net-pci,netdev=n0
  -device virtio-vga-gl -device qemu-xhci -device usb-tablet
  -rtc base=utc
)
if [ "$MODE" = uefi ] && [ -n "$OVMF_CODE" ]; then
  ARGS+=(-drive if=pflash,format=raw,unit=0,readonly=on,file="$OVMF_CODE"
         -drive if=pflash,format=raw,unit=1,file="$VARS")
  echo "firmware: UEFI ($OVMF_CODE)"
else
  [ "$MODE" = uefi ] && echo "OVMF not found — falling back to BIOS"
  echo "firmware: BIOS (SeaBIOS)"
fi

echo "qemu: ${MEM}MB / ${CPUS} vCPU / disk $DISK"
exec qemu-system-x86_64 "${ARGS[@]}" "${DISPLAY_ARGS[@]}" "${EXTRA[@]}"
