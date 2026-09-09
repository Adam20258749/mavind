# compatibility/wine — Windows application layer

Installed only in the **compat** tier (`--profile compat`, the default). See
[`docs/WINDOWS-APPS.md`](../../docs/WINDOWS-APPS.md) for the full picture.

| File | Installed to | Purpose |
|---|---|---|
| `mime/mavind-windows-apps.xml` | `/usr/share/mime/packages/` | give `.exe`/`.msi` a stable MIME type |
| `mime/mavind-windows-apps.desktop` | `/usr/share/applications/` | the "Open with Mavind Windows Apps" handler + isolated-install action |
| `hints.tsv` | `/usr/lib/mavind/wine/` | local, curated compatibility notes (no network DB) |
| `prefix-baseline.txt` | `/usr/lib/mavind/wine/` | winetricks verbs applied to every new prefix — keep short |
| `dxvk.version` / `vkd3d.version` | `/usr/lib/mavind/wine/` | pinned optional-overlay versions |
| `install-dxvk.sh` / `install-vkd3d.sh` | `/usr/lib/mavind/wine/` | download + inject the DLLs into a prefix, set overrides |

The engine (`mavind-windows-apps` / `mavind-wine`) is Rust — see
[`apps/windows-apps/`](../../apps/windows-apps/). It shells out to `wine`, `wineboot`,
`winetricks` and coreutils only, so it builds and unit-tests without Wine present.

## Prefix layout at runtime

```
~/.local/share/mavind/
├── prefixes/default/       shared prefix
├── prefixes/<appid>/       isolated prefixes
├── windows-apps/apps.json  registry
└── logs/<appid>.log
```

Nothing here needs root. Prefixes are disposable: `mavind-wine repair <app>` rebuilds one,
`mavind-wine uninstall <app> --purge-prefix` deletes an isolated one.
