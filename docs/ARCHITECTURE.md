# Mavind Architecture

## Design principle

> Every component must justify its RAM, storage, CPU, and maintenance cost.

Mavind is a **Debian-derived live/installable Linux distribution** with a custom Wayland
desktop and a first-class Windows-application compatibility layer. It is assembled, not
compiled from scratch: the kernel and userspace come from Debian `trixie`; Mavind adds a
thin custom layer on top and removes aggressively.

## Layering

```
┌──────────────────────────────────────────────────────────────┐
│  Mavind apps      minder · settings · system-monitor ·        │  Rust + GTK4
│                   windows-apps · mavind-shell                 │
├──────────────────────────────────────────────────────────────┤
│  Desktop          labwc (Wayland compositor, stacking)        │  wlroots
│                   mako (notifications) · wofi (launcher)      │
├──────────────────────────────────────────────────────────────┤
│  Compatibility    Wine (multiarch) · winetricks · DXVK*       │  optional tier
│                   mavind-wine prefix manager                  │
├──────────────────────────────────────────────────────────────┤
│  Graphics         Mesa · libdrm · Wayland · seatd/libseat     │
├──────────────────────────────────────────────────────────────┤
│  Session/system   systemd (masked down) · udev · NetworkManager│
│                   + iwd · bluez · pipewire · wireplumber       │
├──────────────────────────────────────────────────────────────┤
│  Base             Debian trixie minbase · glibc · busybox*    │
├──────────────────────────────────────────────────────────────┤
│  Kernel           Debian linux-image-amd64 (or custom config) │
└──────────────────────────────────────────────────────────────┘
        * optional / where present
```

## Package tiers

Defined by three lists in [`system/packages/`](../system/packages/):

| Tier | List | Contents | Rough installed size |
|---|---|---|---|
| **core** | `core.list` | kernel, systemd, udev, Mesa, Wayland, labwc, NetworkManager, pipewire, GTK4, the Mavind apps, `foot` | target ~1 GB |
| **compat** | `compat.list` | `wine`, `wine64`, `wine32:i386`, `winetricks`, fonts, `cabextract` | +400–600 MB |
| **optional** | `optional.list` | `mrowser` (browser), `mavind-store`, text editor, calculator, extra firmware, games | user-chosen |

The ISO always contains **core**. `--profile compat` (default) adds **compat** to the
squashfs. `--profile full` adds selected **optional** packages. Anything not in the image
is installed post-boot with `mpk` (see [`packages/`](../packages/)).

## Boot flow

1. **Firmware (UEFI)** loads `EFI/BOOT/BOOTX64.EFI` — a standalone GRUB built by
   `grub-mkstandalone` with an embedded prefix config.
2. **GRUB** reads `boot/grub/grub.cfg` from the ISO, shows the Mavind menu, loads
   `vmlinuz` + `initrd.img` with `boot=live` kernel cmdline.
3. **initramfs (`live-boot`)** finds the ISO by volume id `MAVIND`, mounts
   `live/filesystem.squashfs` read-only, stacks a `tmpfs` overlay (or `toram`), pivots root.
4. **systemd** starts `graphical.target` with almost everything masked (see RAM budget).
5. **greetd** (installed) or **autologin** (live ISO) starts a session:
   `mavind-session` → `dbus-run-session labwc`.
6. **labwc** autostart runs `mavind-shell` (panel), `mako`, `nm-applet` equivalent, and
   applies the wallpaper. Desktop is up.

BIOS boot uses the same GRUB via El Torito + isohybrid MBR, so the ISO also works on
machines without UEFI and on VirtualBox's default (BIOS) firmware.

## The Windows-apps path

- `.exe` / `.msi` files get MIME type `application/x-ms-dos-executable` /
  `application/x-msi`, associated with `mavind-windows-apps.desktop`.
- File managers (Minder, and any XDG-compliant one) show
  **Open with Mavind Windows Apps**.
- `mavind-windows-apps` (GUI) calls the `mavind-wine` core (same binary, `--cli`), which:
  - creates a per-app or shared **Wine prefix** under `~/.local/share/mavind/prefixes/`,
  - runs the installer or the portable exe,
  - records the app in `apps.json`,
  - writes a `.desktop` launcher into `~/.local/share/applications/`,
  - optionally installs **DXVK** into the prefix.
- First run of any unsigned `.exe` shows a **warning dialog** (spec §16).

See [`WINDOWS-APPS.md`](WINDOWS-APPS.md).

## What Mavind deliberately does *not* have

- No display manager theming engine, no compositor eye-candy pipeline
- No systemd-networkd + NetworkManager both — NM only
- No X11 (`Xwayland` is pulled only when Wine/legacy apps need it; it's in `compat`)
- No documentation, man pages, locales except `en`, or dev headers in the image
- No snap/flatpak/appimage runtime in core (optional tier only)
- No telemetry, no analytics, no background updater daemon (updates are user-initiated)
