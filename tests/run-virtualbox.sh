#!/usr/bin/env bash
# Create (or reuse) a VirtualBox VM for a Mavind ISO and start it.
#   tests/run-virtualbox.sh [build/out/Mavind.iso] [--efi] [--bios]
set -euo pipefail
ISO="${1:-build/out/Mavind.iso}"; shift || true
NAME="Mavind-Test"
FW=efi
for a in "$@"; do case "$a" in --bios) FW=bios;; --efi) FW=efi;; esac; done
[ -f "$ISO" ] || { echo "no ISO at $ISO"; exit 1; }
command -v VBoxManage >/dev/null || { echo "install VirtualBox (VBoxManage not found)"; exit 1; }
ISO_ABS="$(cd "$(dirname "$ISO")" && pwd)/$(basename "$ISO")"

if ! VBoxManage showvminfo "$NAME" >/dev/null 2>&1; then
  echo "creating VM $NAME ($FW)"
  VBoxManage createvm --name "$NAME" --ostype Debian_64 --register
  VBoxManage modifyvm "$NAME" \
    --memory 2048 --cpus 2 --vram 64 --graphicscontroller vmsvga \
    --firmware "$FW" --audio-driver none --nic1 nat --boot1 dvd --boot2 disk
  VBoxManage storagectl "$NAME" --name SATA --add sata --controller IntelAhci --portcount 2
  DISK="$(dirname "$ISO_ABS")/${NAME}.vdi"
  [ -f "$DISK" ] || VBoxManage createmedium disk --filename "$DISK" --size 8192 --format VDI
  VBoxManage storageattach "$NAME" --storagectl SATA --port 0 --device 0 --type hdd --medium "$DISK"
  VBoxManage storageattach "$NAME" --storagectl SATA --port 1 --device 0 --type dvddrive --medium "$ISO_ABS"
else
  echo "reusing VM $NAME; swapping ISO"
  VBoxManage storageattach "$NAME" --storagectl SATA --port 1 --device 0 --type dvddrive --medium "$ISO_ABS"
fi

exec VBoxManage startvm "$NAME"
