# Mrowser

Mavind's web browser. A thin **GTK4** shell over **WebKitGTK 6** — no custom
engine (spec §8). Optional: not in `core`/`compat`, only `--profile full` or
installed later.

## Features

| | |
|---|---|
| Tabs | new (`Ctrl+T` / +), close (`Ctrl+W` / ×), reorderable |
| Address bar | `Ctrl+L`; URL if it looks like one, else DuckDuckGo search |
| Navigation | Back/Forward/Reload buttons, `Alt+←/→`, `Ctrl+R` / `F5` |
| Bookmarks | ⭐ adds the current page → `~/.config/mrowser/bookmarks.json`; menu lists them |
| History | recorded on load → `~/.local/share/mrowser/history.json` (cap 800); menu shows recent |
| Downloads | auto-saved to `~/Downloads`, `notify-send` on finish |
| Private window | `mrowser --private` — ephemeral `NetworkSession`, nothing written to disk |

## Why WebKitGTK, not Chromium/Firefox

- Fits the GTK4 stack Mavind already ships; one toolkit.
- ~90 MB installed vs 250–450 MB. Still too big for the ~1 GB base, hence optional.
- `libwebkitgtk-6.0-4` is the only runtime dependency.

## Build / install

- In the tree it's a workspace member but **not** a default member, so
  `cargo build --release` (core/compat) skips it and its WebKit build-dep.
- `--profile full` builds it (`cargo build -p mrowser`) and bakes in
  `libwebkitgtk-6.0-4`.
- Later, on an installed system: `mpk install libwebkitgtk-6.0-4` then drop in
  the `mrowser` binary (a signed Mavind repo for the Mavind-authored optional
  apps is on the roadmap).

## Not yet

- No sync, no extensions, no per-site permissions UI, no reader mode.
- Downloads use the URL basename (no `Content-Disposition` parsing).
- One process per window; tabs share it.
