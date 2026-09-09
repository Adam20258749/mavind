# Running Mavind in VMware (Workstation / Player / Fusion / ESXi)

Mavind ships no VMware-specific driver in `core` beyond what the mainline kernel
already has (`vmwgfx`, `vmxnet3`, `vmw_pvscsi`, `vmw_balloon`), so it boots as a
generic Linux guest.

## New VM

| Setting | Value |
|---|---|
| Guest OS | Linux → *Debian 12/13 64-bit* |
| Firmware | **UEFI** (Options → Advanced → Firmware type). BIOS also works. |
| Memory | 2048 MB (minimum 1536) |
| Processors | 2 |
| Disk | 8 GB, SCSI or NVMe, single file |
| Display | Accelerate 3D graphics: optional; **Graphics memory ≥ 64 MB** |
| CD/DVD | Use ISO image → `build/out/Mavind.iso`, connected at power-on |
| Network | NAT |

## Notes

- If the screen is black after the GRUB menu, pick **"Mavind — safe graphics"**.
  `vmwgfx` + Wayland occasionally needs `nomodeset` on old VMware versions.
- Clipboard/drag-drop needs `open-vm-tools`; it is **not** in `core`. Install it
  later: `mpk install open-vm-tools` (adds ~15 MB).
- ESXi: upload the ISO to a datastore, attach as CD/DVD, set firmware to EFI.
- Measured size/RAM: run `System Monitor` inside the guest, or
  `tests/measure-size.sh` / `tests/measure-ram.sh` on the build host with QEMU.
