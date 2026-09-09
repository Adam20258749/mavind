# kernel/

Mavind uses Debian's `linux-image-amd64` by default — building a kernel from source is a
Phase 4+ optimisation, not a Phase 1 requirement. This directory holds the data the build
uses to *trim* the stock kernel, plus a config fragment for an eventual custom kernel.

| File | Used by | Purpose |
|---|---|---|
| `firmware-keep.list` | `scripts/30` | glob whitelist of `/usr/lib/firmware` paths to keep (saves ~190 MB) |
| `modules-keep.list` | `scripts/30 --aggressive` | module names to keep; deps resolved automatically (saves ~160 MB) |
| `config-fragments/mavind.config` | future custom kernel | `merge_config.sh` fragment: no debug, no exotic drivers, small |

## Trimming risk

Both lists are **conservative guesses** until tested on real hardware (STATUS.md /
ROADMAP Phase 4). If Wi-Fi or a GPU breaks after `--aggressive`:

1. `journalctl -b | grep -i firmware` — note the missing file, add its glob to
   `firmware-keep.list`.
2. `lsmod` on a working system, `modinfo <mod>` — add missing modules to
   `modules-keep.list`.
3. Rebuild without `--aggressive` to confirm it's a trimming problem, not a real bug.

## Custom kernel (later)

`config-fragments/mavind.config` targets: `CONFIG_KERNEL_ZSTD`, no `CONFIG_DEBUG_INFO`,
`CONFIG_SLUB_TINY` where viable, only `virtio`/`ahci`/`nvme`/`xhci` builtin, everything
else modular, no sound cards builtin (snd-* modular), `CONFIG_ZRAM=y`,
`CONFIG_ZSWAP=y`. Expected vmlinuz + modules footprint: ~45 MB vs ~110 MB stock.
