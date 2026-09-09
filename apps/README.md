# apps/ — Mavind's own applications

All Rust, all in the workspace `Cargo.toml` at the repo root, all built with
`opt-level = "z"` + LTO + `panic = "abort"` + `strip` (see the root `Cargo.toml`).

| Crate | Binary | State | Notes |
|---|---|---|---|
| `system-monitor` | `mavind-system-monitor` | usable | real `/proc` + `statvfs`; **no synthetic data** |
| `windows-apps` | `mavind-windows-apps` (+ `mavind-wine` symlink) | core logic done | CLI engine + GTK4 GUI; needs Wine present to exercise |
| `minder` | `minder` | usable | navigate, copy/move/rename/delete, search, properties, drives |
| `settings` | `mavind-settings` | usable | About/Storage/Display/Sound/Network/Windows-Apps/Updates/Users wired to live data |
| `terminal` | (ships `foot`) | done | config only — see `apps/terminal/` |

Build just the apps (fast, no ISO):

```bash
cargo build --release
./target/release/mavind-system-monitor      # runs on any Wayland/Linux dev box
```

`scripts/20-build-components.sh` compiles the workspace and installs the stripped
binaries + `.desktop` files + icons into the image rootfs.

## Shared conventions

- GTK4 via the `gtk4` crate (`v4_10` feature). One toolkit for all apps so the
  ~50 MB runtime cost is paid once.
- No `clone!` macro, minimal `unsafe` — code should read the same across
  gtk4-rs versions.
- Every app takes `--version`; GUIs ignore leftover argv (`run_with_args(&[])`).
- Long/blocking Wine actions are launched in a `foot` terminal so output is visible.
