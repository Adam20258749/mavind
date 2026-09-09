# tests/

| Script | What it does | Needs |
|---|---|---|
| `run-qemu.sh` | interactive UEFI (or `--bios`) boot in QEMU | `qemu-system-x86`, `ovmf` |
| `smoke-boot.sh` | headless serial boot, waits for the ready marker, exit code | `qemu-system-x86` |
| `run-virtualbox.sh` | create + start a VirtualBox VM for the ISO | `VirtualBox` |
| `vmware.md` | manual VMware setup notes | — |
| `measure-size.sh` | real installed size vs the ~1 GB core target | `unsquashfs` / `xorriso` |
| `measure-ram.sh` | boots headless, reads `free -m` over serial vs the RAM budget | `qemu-system-x86` |
| `windows-apps/run-matrix.sh` | Windows-app compat procedure (stub) | — |

## CI

`.github/workflows/build.yml` runs `build-iso.sh` then `smoke-boot.sh` on every
push. Size/RAM measurement jobs attach their numbers to the run summary.

## Quick loop

```bash
./scripts/build.sh --profile core --keep-work
./tests/smoke-boot.sh build/out/Mavind.iso
./tests/measure-size.sh build/out/Mavind.iso
./tests/run-qemu.sh build/out/Mavind.iso      # eyeball the desktop
```
