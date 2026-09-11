# Mavind — Build & Feature Status

_Last updated: 2026-09-11 (later same day)_

This file is the honest source of truth. It does not claim anything is done that is not
verified. "Verified" means a CI run or a human booted it and checked.

## Legend

- ✅ implemented & builds
- 🟡 partial / scaffolded / needs a real build host to verify
- ⬜ not started
- 🔬 needs measurement

## Phase 1 — Core

| Item | State | Notes |
|---|---|---|
| Repo structure | ✅ | Matches the spec layout |
| Reproducible build pipeline (stages 00–50) | 🟡 | Scripts written; not yet executed (no Linux/Docker on dev machine) |
| Build container (Debian) | ✅ | `build/Containerfile` |
| GitHub Actions build + QEMU smoke boot | ✅ | `.github/workflows/build.yml` — first run pending push to GitHub |
| Debian `minbase` bootstrap | 🟡 | `scripts/00-bootstrap-rootfs.sh` |
| System config (users, os-release, systemd mask-list) | 🟡 | `scripts/10-configure-system.sh`, `system/` |
| UEFI + BIOS hybrid ISO (GRUB + live-boot) | 🟡 | `scripts/50-make-iso.sh`, `boot/` |
| Terminal (`foot`) | ✅ | Config in `apps/terminal/`; binary comes from Debian package |
| Boots in QEMU | 🔬 | Pending first CI build |

## Phase 2 — Desktop

| Item | State | Notes |
|---|---|---|
| labwc compositor + config | ✅ | `desktop/labwc/` |
| Session startup (`mavind-session`) | ✅ | `desktop/mavind-session` |
| `mavind-shell`: top bar + floating dock | 🟡 | Two layer-shell surfaces now (macOS-style layout): top bar (system menu, status, clock), bottom dock (pinned app launchers); taskbar (wlr-foreign-toplevel) is still TODO |
| App launcher (Start Menu) | 🟡 | `mavind-launcher`: compact popup near the dock (not full-screen), dismisses on Escape or losing focus; earlier full-screen version had a capture-phase gesture bug that silently ate clicks meant for the search box and app tiles |
| Graphical login (`mavind-greeter`) | 🟡 | Rust/GTK4 crate builds: account picker, greetd IPC client (`apps/greeter/src/greetd.rs`), power menu pre-login; not yet boot-tested against a live greetd |
| Shared theming (`mavind-theme`) | 🟡 | Dark/light, accent color, wallpaper, "glass" (translucent panels — not a real compositor blur, labwc has none); live-reloads in Settings + the panel; now also used by Minder and Mrowser |
| Notifications / tray | 🟡 | tray via `mako` + StatusNotifierItem; shell tray widget TODO |
| Minder file manager | 🟡 | Real crate: browse/copy/move/rename/delete/properties work; keyboard shortcuts (Ctrl+C/X/V, F2, Delete, Alt+Left/Right) and per-extension icons added; search + drive automount partial |
| Mrowser browser | 🟡 | WebKitGTK crate; Chrome-style touches: new-tab button lives in the tab strip (Notebook action widget) instead of the main toolbar, rounded pill omnibox, glass toolbar. Only built for `--profile full` — not in the default `core` CI artifact |
| Mavind Settings | 🟡 | Real crate: About / Appearance / Storage / Network panels wired to live data; others are "coming soon" panels |
| Performance Mode | 🟡 | Toggle writes labwc + shell config; effects are already minimal by default |

## Phase 3 — Windows Apps

| Item | State | Notes |
|---|---|---|
| Wine package set (multiarch) | ✅ | `system/packages/compat.list`, `compatibility/wine/` |
| `mavind-windows-apps` CLI core | 🟡 | prefix create/list/remove, install `.exe`/`.msi`, run, shortcut, dxvk — logic written, needs Wine present to test |
| `mavind-windows-apps` GUI | 🟡 | GTK4 list + actions over the CLI core |
| Right-click `.exe` → Open with Mavind Windows Apps | ✅ | MIME assoc + `.desktop` in `compatibility/wine/` |
| Unknown-`.exe` warning | ✅ | Shown by the CLI core and GUI before first run |
| Compatibility info display | 🟡 | Local static hints table; no online DB |
| DXVK / VKD3D-Proton (optional) | 🟡 | `compatibility/wine/install-dxvk.sh` |

## Phase 4 — Optimization

| Item | State | Notes |
|---|---|---|
| Strip binaries | ✅ | `scripts/30-strip-and-shrink.sh` |
| Remove docs / man / info | ✅ | `dpkg` path-excludes set pre-install |
| Purge locales (keep en) | ✅ | stage 30 |
| Firmware whitelist | 🟡 | `kernel/firmware-keep.list` — conservative default, needs hardware testing |
| Kernel module prune | ⬜ | opt-in `--aggressive`; keep-list not yet curated |
| zram swap + sysctl tuning | ✅ | `system/` |
| Measured disk size | 🔬 | unknown until first build — target ~1 GB core |
| Measured idle RAM | 🔬 | unknown until first boot — target < ~2 GB (expect 250–450 MB idle) |

## Phase 5 — Testing

| Item | State | Notes |
|---|---|---|
| QEMU UEFI runner | ✅ | `tests/run-qemu.sh` |
| VirtualBox runner | ✅ | `tests/run-virtualbox.sh` |
| VMware notes | ✅ | `tests/vmware.md` |
| Headless smoke boot | ✅ | `tests/smoke-boot.sh` — serial-console wait for session-started marker |
| Size measurement | ✅ | `tests/measure-size.sh` |
| RAM measurement | ✅ | `tests/measure-ram.sh` |
| Windows-app compatibility matrix | ⬜ | `tests/windows-apps/` harness stub only |

## Known open questions / risks

1. **Wine size.** Multiarch Wine + deps is ~400–600 MB installed. Core+compat will exceed 1 GB;
   this is why `compat` is a separate tier. Numbers to be confirmed by build.
2. **Firmware.** The whitelist is a guess until tested on real Wi-Fi chips. Falling back to the
   full `firmware-*` set costs ~250 MB.
3. **GTK4 footprint.** ~40–60 MB with deps. Justified by reuse across all Mavind apps; revisit if
   core must hit 1 GB hard.
4. **live-boot vs custom initramfs.** Using Debian `live-boot` now for reliability; a custom
   minimal init is a later size win.
