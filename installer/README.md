# installer/

The privileged **backend** for the graphical installer. Not meant to be used by
hand except for debugging.

| File | What |
|---|---|
| `mavind-install` | Bash. `--plan FILE` (non-interactive, used by `mavind-installer` via `pkexec`), `--probe` (print machine facts as JSON, no root), or bare (tiny `dialog` fallback). Streams `PROGRESS: <pct> <msg>` on stdout. |

The user-facing pieces live elsewhere:

- **`apps/installer/`** — `mavind-installer`, the GTK4 wizard (language → keyboard →
  action → version → system check → disk → summary → install). Runs on the live
  desktop; autostarts when booted from the ISO.
- **`apps/oobe/`** + **`system/oobe/`** — `mavind-oobe`, the first-boot experience
  on an installed system (welcome → region → account → privacy → apply). Creates
  your user account, then hands off to the login screen.

Flow and the `--plan` JSON contract: [`docs/INSTALLER.md`](../docs/INSTALLER.md).

## Debug the backend directly

```bash
mavind-install --probe                     # what the wizard sees
sudo mavind-install --plan /tmp/plan.json   # run an install from a hand-written plan
```

Log: `/var/log/mavind-install.log`.

## Deliberate limits (Phase 3)

- Whole-disk install only (GPT: 512 MB ESP + ext4 root). No partition editor,
  LVM, LUKS, or dual-boot wizard yet.
- Account creation is the OOBE's job, not the installer's (like Windows Setup).
