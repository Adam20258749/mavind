# apps/terminal

Mavind's terminal is **`foot`** — a Wayland-native terminal that is ~1 MB, has no
toolkit dependency, starts instantly, and supports a server mode
(`footclient`) to share one process across windows.

Writing a terminal emulator from scratch would add code to maintain for zero user
benefit, so Mavind doesn't. `foot` comes from the Debian `foot` package (in
`system/packages/core.list`).

| File | Where it ends up |
|---|---|
| `../../system/skel/.config/foot/foot.ini` | `~/.config/foot/foot.ini` (via `/etc/skel`) — Mavind colours + padding |
| `../../desktop/applications/mavind-terminal.desktop` | `/usr/share/applications/` — "Terminal" launcher (`Exec=foot`) |

Keybind: **Super+Return** (set in `desktop/labwc/rc.xml`).

To use the server mode later: run `foot --server` from the labwc autostart and
make the launcher `footclient`.
