# Roadmap

Mirrors spec §19. Each phase ends with a measurable, testable milestone.

## Phase 1 — Core  ← current

**Goal:** a Debian-minbase image that boots UEFI in QEMU to a login shell.

- [x] Repo + build pipeline skeleton
- [x] Build container + CI
- [ ] `mmdebstrap` core rootfs builds clean
- [ ] systemd config + user + os-release
- [ ] GRUB UEFI+BIOS hybrid ISO via `live-boot`
- [ ] `foot` / VT login works
- [ ] **Milestone:** `tests/smoke-boot.sh` gets `mavind: reached multi-user` on serial in QEMU

## Phase 2 — Desktop

**Goal:** log in to a usable Wayland desktop.

- [ ] labwc session via greetd/autologin
- [ ] `mavind-shell`: panel, clock, launcher, power menu, **taskbar (wlr-foreign-toplevel)**
- [ ] mako notifications, tray (SNI)
- [ ] Minder: full copy/move/rename/delete/search/properties, USB automount
- [ ] Mavind Settings: all panels functional (Display, Sound, Network, Bluetooth, Storage,
      Applications, Windows Apps, Users, Security, Updates, About)
- [ ] Performance Mode end-to-end
- [ ] **Milestone:** screenshot test + `measure-ram.sh` idle ≤ 700 MiB

## Phase 3 — Windows Apps

**Goal:** install and run a real `.exe` from Minder.

- [ ] `compat` tier builds (multiarch Wine)
- [ ] `mavind-wine` CLI: prefix/install/run/shortcut/repair/uninstall verified against Wine
- [ ] GUI wired to CLI core
- [ ] Right-click `.exe` → Open with Mavind Windows Apps
- [ ] Unknown-exe warning
- [ ] DXVK optional install
- [ ] **Milestone:** CI installs Notepad++ portable in the booted VM and launches it headless

## Phase 4 — Optimization

**Goal:** hit the size/RAM targets or document why not.

- [ ] Kernel module keep-list curated + `--aggressive` verified on QEMU virtio + common laptop chipsets
- [ ] Firmware whitelist verified (Intel/Realtek/Atheros/MediaTek Wi-Fi)
- [ ] Boot time ≤ 12 s to desktop on a 2-core VM (`systemd-analyze`)
- [ ] `measure-size.sh`: core ≤ 1024 MiB **or** ROADMAP updated with the real floor
- [ ] squashfs reproducible build
- [ ] **Milestone:** published size + RAM numbers in `STATUS.md`, matching CI artifacts

## Phase 5 — Testing & hardware

- [ ] QEMU (UEFI + BIOS), VirtualBox, VMware all boot to desktop — documented
- [ ] Real-hardware test log (≥ 3 machines: old ThinkPad, old Dell, mini-PC)
- [ ] `memtest`-style stability soak (24 h desktop idle, no OOM, no leak)
- [ ] Windows-app compatibility matrix: top 20 common apps with verdicts
- [ ] Installer tested: fresh disk, dual-boot alongside Windows (shared ESP)
- [ ] **Milestone:** `v1.0.2` tagged, ISO + checksums + manifest released

## Post-0.1 backlog

- runit/dinit "ultra" init profile
- Custom minimal initramfs (drop `live-boot`)
- `mpk` repo hosting + signed metadata
- ARM64 / older 32-bit x86
- Secure Boot (signed shim + kernel)
- Mavind Store GUI
- Wayland-native Mrowser packaging
- Flatpak as an optional tier
