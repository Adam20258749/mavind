# scripts/ — the build pipeline

`build-iso.sh` orchestrates numbered stages. Run it on Linux as root, or use
`build.sh` (container) from any OS. Full details: [`docs/BUILD.md`](../docs/BUILD.md).

```
build.sh / build.ps1     container wrappers (docker/podman)
build-iso.sh             orchestrator: parses flags, runs stages 00..50
lib/common.sh            shared helpers (paths, logging, chroot mount, apt)
lib/prune-modules.sh     dependency-aware kernel-module trimming (--aggressive)

00-bootstrap-rootfs.sh   mmdebstrap minbase + tier packages
10-configure-system.sh   branding, users, systemd trim, session, sysctl, zram, skel
20-build-components.sh   cargo build the workspace -> install into rootfs; Wine glue
30-strip-and-shrink.sh   strip ELF, purge doc/man/locale, firmware whitelist, clean
40-make-squashfs.sh      live initramfs + mksquashfs (zstd-19)
50-make-iso.sh           GRUB (UEFI+BIOS) + xorriso hybrid ISO
```

Resume a failed build: `sudo ./scripts/build-iso.sh --from 30 --keep-work`.

All scripts are `set -euo pipefail` and meant to be `shellcheck`-clean.
