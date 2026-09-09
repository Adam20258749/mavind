# boot/

Boot-time assets baked into the ISO by `scripts/50-make-iso.sh`.

| File | Role |
|---|---|
| `grub/grub.cfg.in` | GRUB menu template (`@VOLID@`, `@PROFILE@` substituted at build) |
| `grub/theme.txt` | optional GRUB theme (used if present) |
| `splash.png` | optional 640×480+ background for the GRUB menu (used if present) |

## How boot works

1. **UEFI**: firmware runs `EFI/BOOT/BOOTX64.EFI` (a `grub-mkstandalone` image with an
   embedded stub that `configfile`s `/boot/grub/grub.cfg`).
2. **BIOS**: El Torito boots `boot/grub/bios.img` (`cdboot.img` + standalone i386-pc GRUB),
   same stub, same `grub.cfg`.
3. `grub.cfg` loads `/live/vmlinuz` + `/live/initrd.img` with `boot=live`.
4. `live-boot`'s initramfs finds the medium by volume id **MAVIND**, mounts
   `/live/filesystem.squashfs`, stacks a tmpfs overlay (or copies to RAM with `toram`),
   and switches root. systemd takes over at `graphical.target`.

## Menu entries (spec §14)

- **Mavind** — normal boot
- **safe graphics** — `nomodeset`, software GL, pixman renderer (for broken/old GPUs)
- **load to RAM** — `toram`, so a USB stick can be removed after boot
- **verbose boot** — no `quiet`/`splash`
- **Tools** — medium check, UEFI firmware setup, reboot, shutdown

## Secure Boot

Not yet. The standalone GRUB is unsigned. Roadmap: ship `shim` + signed GRUB + signed
kernel. For now, disable Secure Boot in firmware or use the QEMU/OVMF (non-SB) runner.
