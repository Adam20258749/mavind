# desktop/ — the Mavind desktop

| Path | Role |
|---|---|
| `mavind-session` | session entry point (greetd / live tty1 → sets env → `exec labwc`) |
| `mavind-session.desktop` | `/usr/share/wayland-sessions/mavind.desktop` |
| `labwc/rc.xml` | compositor config — stacking, SSD, no animations, keybinds |
| `labwc/menu.xml` | right-click root menu (apps, Performance Mode, power) |
| `labwc/autostart` | wallpaper (`swaybg`), `mako`, `mavind-shell`, `swayidle` |
| `labwc/environment` | cursor theme + SSD hint |
| `mavind-shell/` | **Rust/GTK4 panel** — launcher, clock, volume/net/battery, power menu |
| `bin/mavind-launcher` | wraps `wofi --show drun` |
| `bin/mavind-screenshot` | `grim` / `grim -g "$(slurp)"` + notify + clipboard |
| `applications/*.desktop` | launcher entries for Minder, Settings, System Monitor, Terminal |
| `assets/` | wallpaper + icons (drop `wallpaper.png` here; shell falls back to solid colour) |

## Why labwc

Windows apps under Wine expect a **stacking / floating** WM with server-side title
bars they can drag. labwc (wlroots, Openbox-style) is ~2 MB and does exactly that.
Tiling compositors (sway) fight Wine's window management.

## mavind-shell scope

Now: launcher button, live clock, status readouts, power menu — all real data.
Next (ROADMAP Phase 2): a taskbar via `wlr-foreign-toplevel-management`, and an
SNI system tray. Kept out of scope until they're done properly rather than faked.
