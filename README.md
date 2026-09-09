# Mavind

**An ultra-lightweight, bootable desktop operating system that runs supported Windows applications.**

Mavind is a real Linux-based OS — not a simulation, not an HTML mockup. It boots on UEFI
hardware and virtual machines, loads a lightweight Wayland desktop, and runs Windows
`.exe` / `.msi` applications through an integrated Wine compatibility layer.

| Target | Goal |
|---|---|
| 💾 Disk (minimal base) | ~1 GB |
| 🧠 RAM (normal desktop) | ~2 GB or less |
| ⚡ Boot | Fast |
| 🖥️ Hardware | x86_64, UEFI, 2 GB+ RAM, low-end CPUs, integrated graphics |
| 🪟 Windows apps | `.exe`, `.msi`, portable apps via Wine |

> Targets are **targets**. Actual measured size and RAM are reported by
> [`tests/measure-size.sh`](tests/measure-size.sh) / [`tests/measure-ram.sh`](tests/measure-ram.sh)
> and tracked honestly in [`STATUS.md`](STATUS.md). Mavind does not claim a number it has not measured.

---

## Quick start

### Build the ISO

You need a Linux host **or** Docker/Podman (the build cannot run on Windows/macOS directly).

**With Docker / Podman (any OS):**

```bash
./scripts/build.sh --profile compat
# -> build/out/Mavind.iso
```

**On a Debian/Ubuntu host:**

```bash
sudo ./scripts/build-iso.sh --profile compat --out build/out/Mavind.iso
```

Profiles: `core` (no Wine, smallest), `compat` (core + Wine, default), `full` (compat + Mrowser + extras).

### Boot it

```bash
./tests/run-qemu.sh build/out/Mavind.iso        # UEFI via OVMF
./tests/run-virtualbox.sh build/out/Mavind.iso  # creates + starts a VM
# VMware: see tests/vmware.md
```

Default live credentials: user `mavind` / password `mavind`.

---

## What's in the box (minimal install)

| Component | What it is |
|---|---|
| **Mavind Desktop** | labwc (Wayland) + `mavind-shell` panel, launcher, notifications, tray |
| **Mavind Windows Apps** | Install / run / manage Windows `.exe` & `.msi` via Wine prefixes |
| **Minder** | Lightweight file manager |
| **Mavind Settings** | Display, Sound, Network, Bluetooth, Storage, Apps, Windows Apps, Users, Security, Updates, About |
| **System Monitor** | Real RAM / CPU / disk / process data from `/proc` — never faked |
| **Terminal** | `foot` (tiny Wayland terminal) |

Optional (installed later via `mpk`): **Mrowser** (browser), **Mavind Store**, Text Editor, Calculator, extra drivers, games.

---

## Repository layout

```
mavind/
├── boot/            GRUB config, UEFI+BIOS hybrid boot, splash
├── kernel/          Kernel config fragments + module keep-lists
├── system/          os-release, systemd unit tuning, sysctl, skel, package tier lists
├── desktop/         labwc config, session startup, mavind-shell (Rust)
├── apps/
│   ├── minder/            file manager (Rust/GTK4)
│   ├── settings/          Mavind Settings (Rust/GTK4)
│   ├── terminal/          foot config + launcher
│   ├── system-monitor/    System Monitor (Rust/GTK4)
│   └── windows-apps/      Mavind Windows Apps (Rust: CLI core + GTK4 GUI)
├── compatibility/
│   └── wine/         Wine package set, prefix templates, MIME + right-click integration, DXVK helper
├── packages/        mpk — lightweight tier-aware package tool over dpkg/apt
├── installer/       TUI installer (partition, unsquash, GRUB, user)
├── tests/           QEMU/VBox boot, size + RAM measurement, smoke tests
├── scripts/         reproducible build pipeline (stages 00–50)
├── build/           build container + output (git-ignored)
└── docs/            architecture, build, size/RAM budgets, roadmap
```

---

## Documentation

- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — how the system is put together
- [`docs/BUILD.md`](docs/BUILD.md) — build system internals, reproducibility
- [`docs/SIZE-BUDGET.md`](docs/SIZE-BUDGET.md) — where every megabyte goes
- [`docs/RAM-BUDGET.md`](docs/RAM-BUDGET.md) — idle memory accounting
- [`docs/WINDOWS-APPS.md`](docs/WINDOWS-APPS.md) — the Wine compatibility layer
- [`docs/ROADMAP.md`](docs/ROADMAP.md) — phases 1–5
- [`STATUS.md`](STATUS.md) — **what actually works today**

## License

See [`LICENSE`](LICENSE). Mavind bundles third-party components (Linux kernel, Debian
packages, Wine, Mesa, labwc, GTK) under their respective licenses.
