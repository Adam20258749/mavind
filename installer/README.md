# installer/

`mavind-install` — a small script that installs the running live system to disk.

## What it does

1. GPT partitions the target: 512 MB EFI System Partition + one ext4 root.
2. `unsquashfs` the live `filesystem.squashfs` onto root.
3. Writes `/etc/fstab` from partition UUIDs.
4. In a chroot: sets hostname, creates your user, **removes** the live
   passwordless-sudo and tty1 autologin, enables `greetd`, blanks `machine-id`.
5. Installs GRUB — `grub-efi-amd64` for UEFI or `grub-pc` for BIOS — and runs
   `update-grub`.

## Usage

```bash
sudo mavind-install                       # interactive (uses `dialog` if present)
sudo mavind-install --unattended --disk /dev/sda --user me --hostname mavind-pc
```

## Deliberate limits (Phase 1)

- No LVM, no LUKS, no Btrfs subvolumes, no swap partition (Mavind uses zram).
- No dual-boot menu wizard — GRUB's `os-prober` (optional tier) finds other OSes
  if installed.
- No manual partition editor — it takes the whole disk. Pre-partition and mount
  by hand, then `unsquashfs` yourself for custom layouts.

The installer is meant to be readable in one sitting. Extend it in `installer/`.
