# Mavind Installer & OOBE

A graphical, Windows-11-Setup-style install experience. No terminal, no scripts
in the user's face.

## Two programs

| Program | Runs | Language | Job |
|---|---|---|---|
| **`mavind-installer`** | on the live ISO desktop | Rust + GTK4 | pick options → write Mavind to a disk |
| **`mavind-oobe`** | first boot of the *installed* system, before login | Rust + GTK4 | create your account, finish setup |
| `mavind-install` | invoked by the installer via polkit | Bash | the privileged backend (partition, unpack, GRUB) |

The GUI never runs as root. It writes an install **plan** (JSON) and calls
`pkexec mavind-install --plan <file>`; the backend streams `PROGRESS: <pct> <msg>`
lines back, which the GUI shows as a progress bar + log.

## Installer flow (`mavind-installer`)

1. **Language** — UI language for the rest of setup (and the default system locale).
2. **Keyboard layout** — layout + variant; a test field to try it.
3. **What do you want to do?** — one card for now: **Install Mavind**. (Room for
   "Repair", "Try the live desktop" later.)
4. **Choose version** — lists entries from `/usr/lib/mavind/releases.json`
   (falls back to the running ISO's `/usr/lib/mavind/release.json`). The version
   string is stamped at build time from git, so it changes on every update.
5. **System check** — RAM, disk space, firmware (UEFI), CPU arch/cores, network.
   Each row is ✅ pass / ⚠️ warning / ❌ blocker. Blockers disable **Next**.
6. **Choose disk** — `lsblk` list (model, size, bus). Whole-disk install; a big
   "everything on this disk will be erased" warning. (No partition editor yet.)
7. **Summary** — every choice on one screen. **Install** button.
8. **Installing** — progress bar + collapsible log, driven by the backend.
9. **Done** — "Remove the installation media and restart." Restart button.

State is a single `InstallPlan` struct serialised to
`$XDG_RUNTIME_DIR/mavind-install-plan.json`.

## OOBE flow (`mavind-oobe`)

Runs once, fullscreen, as the `mavind-oobe.service` on `oobe.target` (greetd is
masked until OOBE completes).

1. **Welcome** — hi, language confirm.
2. **Region & time** — country → timezone; NTP on/off.
3. **Keyboard** — confirm/adjust (prefilled from the installer choice).
4. **Network** — optional: pick a Wi-Fi network, or **Skip**.
5. **Your account** — full name, username, password (+ confirm), hostname.
   "Log in automatically" checkbox.
6. **Privacy** — one screen: *Mavind collects no telemetry.* Nothing to opt out of.
7. **All set** — creates the user, applies settings, enables greetd, then starts
   the session.

`mavind-oobe --done` writes `/var/lib/mavind/oobe-complete`, unmasks + enables
`greetd`, and `systemctl isolate graphical.target`.

## Backend contract (`mavind-install --plan FILE`)

`FILE` is JSON:

```json
{
  "disk": "/dev/sda",
  "firmware": "uefi",
  "locale": "en_US.UTF-8",
  "keymap": "us",
  "timezone": "UTC",
  "hostname": "mavind",
  "version_id": "0.1.0-12-gabc1234",
  "create_user": false
}
```

`create_user` is `false` from the graphical installer (OOBE makes the account);
`true` is available for unattended installs. The backend prints:

```
PROGRESS: 0   Preparing
PROGRESS: 5   Partitioning /dev/sda
PROGRESS: 20  Formatting
PROGRESS: 30  Copying system  (unsquashfs)
PROGRESS: 80  Installing bootloader (uefi)
PROGRESS: 95  Writing configuration
PROGRESS: 100 Done
```

Anything on stderr is surfaced in the log pane. Non-zero exit → the GUI shows the
last error and offers **Retry** / **View log**.

## Live-ISO integration

- `labwc` autostart launches `mavind-installer` maximised **only** when booted
  live (`/run/live/medium` exists). Close it to explore the live desktop.
- Desktop + launcher also have an **Install Mavind** entry.

## Versioning

`scripts/build-iso.sh` stamps `VERSION_ID` / `BUILD_ID` into `os-release` and
`/usr/lib/mavind/release.json` from `git describe --tags --always`. Multi-version
media can ship `/usr/lib/mavind/releases.json` (an array) and the installer's
step 4 lists them all.
