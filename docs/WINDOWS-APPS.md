# Windows Application Compatibility

Mavind's purpose is to run supported Windows applications on a tiny Linux base. This is
done with **Wine** — no emulation of a full Windows OS, no VM.

> **Not every Windows application works.** Mavind never claims otherwise. Anti-cheat
> games, apps needing specific kernel drivers, and some DRM will not run. Mavind shows the
> best compatibility hint it has before you run something.

## Components (`compat` tier)

| Package | Role | Optional |
|---|---|---|
| `wine` / `wine64` | 64-bit Windows API | no |
| `wine32:i386` | 32-bit Windows apps (still the majority of installers) | no |
| `winetricks` | fetch/install redistributables (vcrun, dotnet, corefonts…) | no |
| `cabextract`, `p7zip` | unpack installers | no |
| `Xwayland` | apps that only speak X11 | no (pulled by compat) |
| **DXVK** | Direct3D 9/10/11 → Vulkan; big speed-up for games | **yes** |
| **VKD3D-Proton** | Direct3D 12 → Vulkan | **yes** |

Installed to `~/.local/share/mavind/` (per user), never system-wide, so prefixes are
disposable and don't need root.

## `mavind-windows-apps`

One binary, two faces:

- **GUI** (`mavind-windows-apps`): GTK4. Lists installed Windows apps, buttons for
  Install / Run / Shortcut / Configure / Repair / Uninstall / Install DXVK.
- **CLI core** (`mavind-windows-apps --cli …`, also symlinked as `mavind-wine`): all the
  logic; scriptable; used by the GUI and by the right-click handler.

### Storage layout

```
~/.local/share/mavind/
├── prefixes/
│   ├── default/            WINEPREFIX for "shared" apps
│   └── <appid>/            per-app prefix (isolation)
├── apps.json               registry: id, name, exe path, prefix, arch, dxvk, added, last_run
└── logs/<appid>.log
~/.local/share/applications/mavind-app-<appid>.desktop   generated launchers
```

### CLI

```
mavind-wine prefix create <name> [--arch win64|win32]
mavind-wine prefix list
mavind-wine prefix rm <name>

mavind-wine install <file.exe|file.msi> [--prefix NAME] [--isolated] [--name "App"]
mavind-wine run <appid|name>
mavind-wine shortcut <appid> [--menu] [--desktop]
mavind-wine configure <appid>          # launches winecfg in that prefix
mavind-wine repair <appid>             # wineboot -u ; re-run winetricks baseline
mavind-wine uninstall <appid> [--purge-prefix]
mavind-wine dxvk install <appid>       # or: vkd3d install
mavind-wine list                       # registry, with compat hint
mavind-wine hint <file-or-name>        # print known compatibility notes
```

### Right-click integration

`compatibility/wine/mime/` ships:

- `mavind-windows-apps.xml` — registers `application/x-ms-dos-executable`,
  `application/x-msi`, `application/vnd.microsoft.portable-executable`.
- `mavind-windows-apps.desktop` with
  `MimeType=application/x-ms-dos-executable;application/x-msi;` and an `Install` +
  `Run` action.

Result: **Right-click `.exe` → Open with Mavind Windows Apps → Install / Run** (spec §4).

### Safety (spec §16)

Before the **first** execution of any `.exe`/`.msi` that Mavind did not install itself,
`mavind-wine` prints/di­splays:

```
⚠  "setup.exe" is an unverified Windows program from:
      /home/mavind/Downloads/setup.exe   (SHA-256 abc123…, 4.2 MB)
   Windows programs run with your user's permissions and can modify your files.
   Run it in an isolated Wine prefix?   [Isolated]  [Shared prefix]  [Cancel]
```

Isolated is the default highlighted choice.

### Compatibility hints

`compatibility/wine/hints.tsv` — a small local table (`name<TAB>verdict<TAB>note`).
No network lookup, no bundled copy of a third-party DB. Example rows:

```
Notepad++       good    Works out of the box.
7-Zip           good    Installer + app both fine.
iTunes          poor    Broken on modern Wine; not recommended.
```

`mavind-wine hint <x>` matches on filename/registry name substring.

## DXVK install flow

`compatibility/wine/install-dxvk.sh <PREFIX>`:

1. Detect Vulkan (`vulkaninfo` / `libvulkan1`); warn if no ICD.
2. Download the pinned DXVK release tarball (version in `dxvk.version`), verify SHA-256.
3. Copy `x64/*.dll` and `x32/*.dll` into the prefix, set DLL overrides
   (`d3d9,d3d10core,d3d11,dxgi = native`).
4. Record `dxvk=<version>` in `apps.json`.

Offline builds skip this; the user runs it later from Settings → Windows Apps.
