# Contributing to Mavind

## The one rule

> Every component must justify its RAM, storage, CPU, and maintenance cost.

A PR that adds size or a background process must say, in the description, **how much** and
**why it's worth it**. "It would be nice" is not a reason.

## Dev environment (kept separate from the OS — spec §17)

Your dev machine needs: Linux or Docker, `rustc`/`cargo`, `libgtk-4-dev`,
`libgtk4-layer-shell-dev`, `pkg-config`, plus the build packages in `docs/BUILD.md`.
**None of these tools go into the Mavind image.**

```bash
# lint + format + test the components
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --workspace

# build just the components (fast, no ISO)
cargo build --release

# full ISO
./scripts/build.sh --profile compat
```

## Code layout

| Path | Language | Notes |
|---|---|---|
| `desktop/mavind-shell`, `apps/*` | Rust 2021, `opt-level="z"` | GTK4 via `gtk4` crate; layer-shell via `gtk4-layer-shell` |
| `scripts/*.sh` | Bash, `set -euo pipefail` | POSIX-ish; run through `shellcheck` |
| `packages/mpk` | Rust | thin wrapper over `apt`/`dpkg` |
| `installer/` | Bash + `dialog` | keep dependency-free where possible |
| `system/`, `desktop/labwc/`, `boot/` | config files | copied verbatim into the rootfs |

## Style

- Rust: `cargo fmt`, no `unwrap()` in non-test code (use `anyhow`), no `unsafe` without a
  `// SAFETY:` comment.
- Bash: `set -euo pipefail`, quote everything, `shellcheck` clean, functions over copy-paste.
- Commits: imperative mood, reference the phase (`phase2: minder USB automount`).
- No new runtime dependency without a line in `docs/SIZE-BUDGET.md`.

## Testing a change to the image

1. `./scripts/build.sh --profile core --keep-work`
2. `./tests/run-qemu.sh build/out/Mavind.iso`
3. `./tests/measure-size.sh build/out/Mavind.iso`
4. `./tests/measure-ram.sh build/out/Mavind.iso`
5. Paste the size/RAM deltas into your PR.

## Filing issues

Include: what tier (`core`/`compat`/`optional`), the ISO `sha256` from
`build/out/Mavind.iso.sha256`, `journalctl -b` if it booted, and the VM/hardware.
