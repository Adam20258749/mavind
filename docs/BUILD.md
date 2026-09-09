# Building Mavind

## Requirements

The build runs on **Linux** (or a Linux container). It cannot run natively on Windows or
macOS because it uses `mmdebstrap`, `chroot`, loop mounts, `mksquashfs`, and `xorriso`.

Three supported ways to build:

| Method | Host | Command |
|---|---|---|
| Docker / Podman | any OS with a container engine | `./scripts/build.sh` |
| Native | Debian 12+/Ubuntu 22.04+ | `sudo ./scripts/build-iso.sh` |
| CI | GitHub Actions (`ubuntu-latest`) | automatic on push |

### Native host packages

```bash
sudo apt-get install -y \
  mmdebstrap xorriso squashfs-tools \
  grub-common grub-pc-bin grub-efi-amd64-bin mtools dosfstools \
  cargo rustc pkg-config \
  libgtk-4-dev libglib2.0-dev libgtk4-layer-shell-dev \
  ca-certificates zstd file fakeroot
```

## Pipeline

`scripts/build-iso.sh` is the orchestrator. It runs numbered stages from `scripts/`:

| Stage | Script | Does |
|---|---|---|
| 00 | `00-bootstrap-rootfs.sh` | `mmdebstrap` a Debian `minbase` rootfs; add tier packages; pre-seed `dpkg` no-doc/no-locale excludes |
| 10 | `10-configure-system.sh` | os-release, hostname, users, systemd mask-list, greetd/autologin, skel, sysctl, zram, drop `system/` + `desktop/` files in |
| 20 | `20-build-components.sh` | `cargo build --release` the workspace; strip; install binaries + `.desktop` + icons + polkit into the rootfs; `compatibility/wine/` integration files |
| 30 | `30-strip-and-shrink.sh` | strip ELF, delete `*.a`/headers, purge `doc`/`man`/`info`/`locale`(keep en), firmware whitelist, optional module prune, clean apt/logs |
| 40 | `40-make-squashfs.sh` | build live initramfs; `mksquashfs` with zstd-19; emit `filesystem.squashfs` + size + checksums |
| 50 | `50-make-iso.sh` | assemble `iso/` tree; standalone GRUB for UEFI + BIOS; `xorriso` hybrid image → `Mavind.iso` |

Each stage is idempotent-ish and can be re-run; pass `--from 30` to resume.

### Flags

```
scripts/build-iso.sh
  --profile core|compat|full     (default: compat)
  --arch amd64                    (only amd64 for now)
  --suite trixie                  Debian suite
  --mirror URL                    Debian mirror (default: deb.debian.org)
  --out PATH                      output ISO path (default: build/out/Mavind.iso)
  --from STAGE                    resume from stage number (00,10,20,30,40,50)
  --aggressive                    also prune kernel modules by keep-list (риск)
  --no-cache                      ignore build/cache
  --keep-work                     don't delete build/work on success
```

## Reproducibility

- `SOURCE_DATE_EPOCH` is set from the last git commit and threaded into `mksquashfs`
  (`-mkfs-time`, `-all-time`) and `xorriso` (`--modification-date`).
- The Debian snapshot mirror can be pinned with `--mirror https://snapshot.debian.org/archive/debian/<TS>/`.
- The exact package set that landed in a build is written to
  `build/out/Mavind.iso.manifest` (`dpkg-query -W`).
- `build/out/Mavind.iso.sha256` is emitted next to the image.

Two builds from the same commit + same mirror snapshot should produce byte-identical
squashfs. (Full ISO determinism also needs reproducible GRUB, tracked in ROADMAP.)

## Output

```
build/out/
├── Mavind.iso
├── Mavind.iso.sha256
├── Mavind.iso.manifest      # every package + version in the image
└── Mavind.iso.sizes         # per-directory du of the rootfs before squashing
```

## Troubleshooting

- **`mmdebstrap: E: ... mount ... permission denied`** — run under `sudo`, or add
  `--unshare` support: `sysctl kernel.unprivileged_userns_clone=1`, or use the container.
- **Container build needs privilege** — `docker run --privileged` (or
  `--cap-add SYS_ADMIN --security-opt apparmor=unconfined`). Podman: `--privileged` too,
  rootless works with `newuidmap` installed.
- **`grub-mkstandalone: command not found`** — install `grub-common` + `grub-efi-amd64-bin`
  + `grub-pc-bin`.
- **ISO boots BIOS but not UEFI in VBox** — enable EFI in the VM's System settings, or use
  the QEMU/OVMF runner which is UEFI by default.
