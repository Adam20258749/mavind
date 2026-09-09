# Size Budget

**Target: ~1 GB installed for the `core` tier.** This document is the plan for hitting it.
Real numbers replace the estimates once `tests/measure-size.sh` runs against a build.

All figures are *uncompressed installed* size unless noted. The ISO squashfs compresses
roughly 2.5–3× with zstd-19.

## Core tier estimate

| Group | Estimate | Notes / how it's kept down |
|---|--:|---|
| Debian `minbase` (glibc, apt, dpkg, coreutils, systemd, udev) | 300 MB | `--variant=minbase`, no-recommends, no-doc, no-man |
| Kernel `linux-image-amd64` + `initramfs-tools` | 90 MB | one flavour; consider `linux-image-amd64` → custom later |
| Kernel modules | 250 MB → **90 MB** | `--aggressive` prune to keep-list (`kernel/modules-keep.list`) |
| Firmware | 250 MB → **60 MB** | whitelist only (`kernel/firmware-keep.list`): common wifi/gpu + `regulatory.db` |
| Mesa + libdrm + Wayland + seatd | 120 MB | `libgl1-mesa-dri` for common GPUs; drop `nouveau`/`r300` etc. optionally |
| GTK4 + pango + cairo + gdk-pixbuf + graphene | 55 MB | shared by all Mavind apps; single toolkit |
| labwc + wlroots + wofi + mako | 12 MB | |
| NetworkManager + iwd + wpa fallback | 22 MB | NM only; no networkd |
| PipeWire + WirePlumber | 15 MB | replaces PulseAudio + ALSA plugins |
| bluez | 10 MB | socket-activated |
| fonts (1 UI + 1 mono) | 8 MB | `fonts-dejavu-core` only (or Inter subset) |
| Mavind apps (5 Rust binaries, `opt-level=z`, stripped) | 25 MB | static-ish; share GTK4 |
| `foot` terminal | 2 MB | |
| zram-tools, sudo, polkit, greetd | 10 MB | |
| **Core total (with `--aggressive`)** | **≈ 900 MB – 1.05 GB** | **on target** |
| Core total (without module/firmware prune) | ≈ 1.35 GB | fallback if prune breaks hardware |

## compat tier (added by default profile)

| Group | Estimate |
|---|--:|
| `wine` + `wine64` + `wine32:i386` + libs | 350 MB |
| i386 glibc + X libs pulled by wine32 | 120 MB |
| `winetricks`, `cabextract`, `p7zip` | 8 MB |
| Xwayland | 25 MB |
| `fonts-liberation`, core fonts | 15 MB |
| **compat adds** | **≈ 500–520 MB** |

→ **core + compat ≈ 1.4–1.55 GB installed**, **ISO ≈ 600–750 MB**.
This is expected and why compat is a separate tier (spec §3, §10).

## optional tier (never in core)

| Package | Installed |
|---|--:|
| Mrowser (WebKitGTK or Chromium-based) | 250–450 MB |
| Mavind Store | 10 MB |
| Text editor / calculator | 5 MB each |
| Full `firmware-linux` | +200 MB |
| Games | varies |

## Levers (in priority order)

1. **Kernel module prune** — biggest single win (~160 MB). Needs a curated keep-list and
   hardware testing. `scripts/30` `--aggressive`.
2. **Firmware whitelist** — ~190 MB. Risk: missing Wi-Fi on untested chips.
3. **Locale/doc/man purge** — ~120 MB. Safe. Always on.
4. **Strip all ELF** — ~40–70 MB. Safe. Always on.
5. **Drop unused Mesa DRI drivers** — ~30–50 MB. Keep i965/iris/radeonsi/swrast.
6. **`opt-level="z"` + LTO + `panic=abort`** on Rust apps — ~40% off each binary.
7. **squashfs zstd-19, 1 MiB blocks** — image-only, no install-size effect.
8. **`toram` boot option** — lets low-RAM-but-slow-disk machines run fully from RAM.

## Measurement

`tests/measure-size.sh` mounts the squashfs (or reads `Mavind.iso.sizes`) and prints:

```
core        <measured>   (target 1024 MiB)   PASS/FAIL
core+compat <measured>
top 30 dirs by du
```

`Mavind Settings → Storage` shows the same live number from the running system.
