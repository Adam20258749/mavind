# build/

Build environment and output. **Everything under `out/`, `work/`, `cache/`, `rootfs/` is
git-ignored.**

| Path | What |
|---|---|
| `Containerfile` | Debian build image used by `scripts/build.sh` and CI |
| `out/` | `Mavind.iso`, `.sha256`, `.manifest`, `.sizes` (created by a build) |
| `work/` | scratch: `rootfs/`, `live/`, `iso/` (deleted on success unless `--keep-work`) |
| `cache/` | apt archives + cargo target/registry, reused across builds |

## Build

```bash
../scripts/build.sh --profile compat      # docker/podman, any OS
# or on Debian/Ubuntu:
sudo ../scripts/build-iso.sh --profile compat
```

## Note on OneDrive

If this repo lives inside a OneDrive-synced folder, exclude `build/` from sync (right-click
`build` → *Always keep on this device* off, or *Free up space*), or move the repo out of
OneDrive for builds. A build writes tens of thousands of files; syncing them mid-build
causes file-lock failures and pointless upload traffic.
