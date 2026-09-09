# system/ — files copied verbatim into the image

`scripts/10-configure-system.sh` drops these into the rootfs.

| Path | Installed to | Purpose |
|---|---|---|
| `os-release` | `/usr/lib/os-release` | branding / `ID=mavind` |
| `issue`, `motd` | `/etc/` | console banners |
| `packages/*.list` | `/usr/lib/mavind/packages/` + read by the build | the core/compat/optional tiers |
| `systemd/mask.list` | (applied) | units masked for RAM + boot time |
| `systemd/enable.list` | (applied) | units explicitly enabled |
| `systemd/journald.conf.d/` | `/etc/systemd/journald.conf.d/` | volatile, tiny journal |
| `systemd/logind.conf.d/` | `/etc/systemd/logind.conf.d/` | lid/power behaviour |
| `systemd/getty-autologin.conf` | `getty@tty1` drop-in | **live ISO** autologin (installer removes) |
| `sysctl.d/99-mavind.conf` | `/etc/sysctl.d/` | low-RAM VM tuning |
| `zram/zram-generator.conf` | `/etc/systemd/zram-generator.conf` | compressed RAM swap |
| `modprobe.d/mavind.conf` | `/etc/modprobe.d/` | blacklist pcspkr, watchdogs |
| `greetd/config.toml` | `/etc/greetd/` | login manager for installed systems |
| `firstboot/` | `/usr/lib/mavind/` + a unit | one-shot first-boot setup |
| `skel/` | `/etc/skel/` | per-user defaults (bashrc, foot, mako, performance.conf) |
| `polkit/*.rules` | `/usr/share/polkit-1/rules.d/` | let the desktop user mount media, manage Wi-Fi, suspend |

Nothing here is executable policy on its own — the stage script decides what gets
applied. Keep additions small and justified (`docs/SIZE-BUDGET.md`,
`docs/RAM-BUDGET.md`).
