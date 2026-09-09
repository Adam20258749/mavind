# packages/ — the Mavind package system

Mavind does **not** invent a package format. The base is Debian's `dpkg`/`apt`
(proven, huge repo, security updates). `mpk` is a thin, opinionated front-end:

- always `--no-install-recommends` (keeps installs minimal)
- knows the **core / compat / optional** tiers (`/usr/lib/mavind/packages/*.list`)
- `mpk size` shows the **real** on-disk breakdown
- `mpk install-tier compat` pulls the whole Wine set in one go

```
mpk install <pkg>...      mpk remove <pkg>...     mpk search <q>
mpk update                mpk upgrade             mpk list [--tier]
mpk install-tier compat|optional                 mpk size
```

Anything `mpk` doesn't cover, plain `apt`/`dpkg` still work.

## Why not a custom package manager?

A new format means a new resolver, a new repo, new signing, new tooling, and years
before it's trustworthy — all cost, little benefit for a small OS. The spec's goal
("small base → user installs only what they need") is met by **tiers + a small ISO
+ apt**, not by a bespoke manager. See `docs/ARCHITECTURE.md`.

## Future (post-0.1, `docs/ROADMAP.md`)

- a signed Mavind apt repo for `mavind-shell`, `minder`, etc. as real `.deb`s
- `mpk` learns `.deb` build-from-source for the Mavind components
- optional Flatpak tier for sandboxed desktop apps
