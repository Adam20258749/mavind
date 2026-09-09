# RAM Budget

**Target: ~2 GB or less during normal desktop operation** (spec §11). "Normal" = desktop
up, panel, file manager open, one small app. Running heavy Windows apps is separate and
depends on the app.

Idle expectation on Mavind `core`: **250–450 MB** used (excluding caches/buffers).

## Idle accounting (estimate, `Used` from `free -m` semantics)

| Process / area | Estimate |
|---|--:|
| kernel + slab + page tables | 80–120 MB |
| systemd (pid 1) + journald (volatile) + udevd + logind | 25 MB |
| seatd / dbus-broker | 6 MB |
| NetworkManager + iwd | 20 MB |
| PipeWire + WirePlumber | 15 MB |
| bluez (`bluetoothd`, if enabled) | 5 MB |
| labwc + wlroots | 30–45 MB |
| mavind-shell (GTK4) | 35–55 MB |
| mako | 6 MB |
| polkitd | 8 MB |
| greetd (after login: gone) | 0 |
| **Idle total** | **≈ 260–360 MB** |

Add ~60–90 MB for Minder open, ~50–80 MB for Settings, ~30 MB for `foot`.

## What keeps it low

- **systemd mask-list** (`system/systemd/mask.list`): `ModemManager`,
  `systemd-networkd*`, `NetworkManager-wait-online`, `systemd-timesyncd` (→ we keep it, it's
  tiny; masked: `*-wait-online`), `apt-daily*`, `man-db.timer`, `e2scrub*`,
  `systemd-oomd` (kept! see below), `packagekit`, `fwupd`, `avahi-daemon`, `cups*`,
  `plocate`, `unattended-upgrades`.
- **journald** `Storage=volatile`, `RuntimeMaxUse=16M`, `MaxLevelStore=notice`.
- **No tracker/indexer** — no `plocate`, no file-content indexing.
- **zram swap** (`system/zram/`) sized to 50% RAM, `zstd`, `vm.swappiness=180`,
  `vm.page-cluster=0`. On a 2 GB machine this roughly doubles effective memory for cold
  pages without touching disk.
- **`systemd-oomd`** *is* enabled with a per-user-slice pressure limit so a runaway Wine
  app gets killed instead of thrashing the whole machine.
- **PipeWire** instead of PulseAudio+Alsa daemons.
- **dbus-broker** instead of `dbus-daemon` (lower latency + slightly lower RSS).
- **GTK4** apps use `GSK_RENDERER=gl` (or `ngl`); on weak integrated GPUs the shell sets
  `GSK_RENDERER=cairo` automatically (Performance Mode forces it).

## Performance Mode (spec §6)

Toggled in `Mavind Settings → Display` or `mavind-shell` menu. Writes
`~/.config/mavind/performance.conf` and signals the shell. Effects:

| Setting | Normal | Performance Mode |
|---|---|---|
| labwc animations | off (already) | off |
| Server-side shadows | on | off |
| GTK renderer | gl | cairo |
| Shell update tick | 1 s | 2 s |
| Wallpaper | image | solid colour |
| `vm.swappiness` | 180 | 180 |
| Thumbnails in Minder | on | off |

## Measurement

`tests/measure-ram.sh <iso>` boots headless, waits for the desktop, then over the serial
console runs `free -m`, `ps_mem`-style RSS accounting, and dumps `systemd-cgtop -b -n1`.
It prints:

```
MemTotal      <n> MiB
Used (idle)   <n> MiB     target <= 700 MiB idle    PASS/FAIL
Used (+Minder,+Settings)  <n> MiB
per-process RSS table
```

`System Monitor` shows the same live data on the running system — read straight from
`/proc/meminfo`, `/proc/*/smaps_rollup`, `/proc/stat`. **No value in System Monitor is
synthetic** (spec §11).
