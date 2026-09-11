# Appearance

Mavind's look — theme, accent color, wallpaper, and "glass" panels — is controlled from
**Settings → Appearance** and shared by every Mavind app through the `mavind-theme` crate
(`libs/mavind-theme`). This doc is the honest description of what that system actually does.

## State

One JSON file, `~/.config/mavind/appearance.json`:

```json
{ "theme": "dark", "accent": "#8a5cf6", "wallpaper": null, "glass": true }
```

- `theme` — `"dark"` or `"light"`.
- `accent` — a hex color. Settings offers a curated palette
  (`mavind_theme::ACCENT_SWATCHES`); any valid `#rrggbb` works if you edit the file by hand.
- `wallpaper` — the source path the user last picked, kept for display only. The desktop
  background is always read from `~/.config/mavind/wallpaper.png` (see below), not this field.
- `glass` — on/off switch for translucent panel styling.

Saving (`Appearance::save`) writes that JSON and regenerates
`~/.config/mavind/theme.css` — a small stylesheet of GTK `@define-color` tokens
(`accent_color`, `accent_soft`, `mavind_bg`, `mavind_fg`, `mavind_dim`, `mavind_panel`,
`mavind_card`, `mavind_panel_border`) plus a handful of cross-cutting rules (suggested-action
buttons, progress/level bars, popover backgrounds).

## How apps pick it up

Every Mavind GTK4 app calls `mavind_theme::load_app_css(include_str!("style.css"))` once at
startup: it loads the app's own stylesheet, then layers `theme.css` on top at a higher CSS
priority, so accent/theme tokens always win over an app's hard-coded fallback colors. An app's
own `style.css` is free to reference the shared tokens directly (`background-color:
@mavind_panel;` etc.) — that's how the launcher, greeter, and shell panel get their "glass"
look without duplicating color logic.

Changing Appearance in Settings takes effect immediately in Settings itself, and in any
long-running app that polls for it — currently `mavind-shell` (the panel), which checks
`mavind_theme::theme_css_mtime()` on the 1-second timer it already runs for the clock, and
calls `mavind_theme::reload_theme_css()` when the file has changed. Apps that don't hold such a
timer (the launcher) simply pick up the new theme the next time they start, since they read
`theme.css` fresh on every launch. There is no cross-process push/broadcast — this is
deliberately the simplest thing that works, not a general live-config-sync service.

`mavind-greeter` is the one exception, and deliberately so: it runs as the unprivileged
`greeter` system account (see `system/greetd/config.toml`), which has its own empty
`~/.config/mavind/`, not any real user's. So the login screen always renders
`Appearance::default()` — dark, the default violet accent, system wallpaper — never whatever a
particular user picked. That's the same boundary every mainstream display manager (GDM, SDDM,
...) keeps: a pre-auth screen has no business reading a not-yet-authenticated user's files.

## Wallpaper

`mavind_theme::wallpaper_path()` — `~/.config/mavind/wallpaper.png` — is the one file the
desktop background (`swaybg`, launched from `desktop/labwc/autostart`) treats as "the current
wallpaper." Settings' wallpaper picker copies whatever image file you choose to that exact path
(image loaders sniff file contents, not the extension, so this works regardless of the source
format). If that file doesn't exist, the desktop falls back to
`/usr/share/backgrounds/mavind/wallpaper.png` (the system default), and then to a solid color.
The login screen only ever sees the system default or a solid color, for the same
not-your-files-before-you've-authenticated reason as the theme/accent (previous section).

## "Glass" — what it actually is

**There is no live background blur.** labwc (via wlroots) has no compositor-side blur protocol
for layer-shell or regular surfaces to render against, and Mavind doesn't ship a bespoke one.
"Glass" here means:

- semi-transparent panel/card backgrounds (`mavind_panel`: `rgba(24,24,30,0.58)` in dark,
  `rgba(255,255,255,0.70)` in light),
- a faint 1px highlight border (`mavind_panel_border`),
- a soft drop shadow.

What's actually behind a "glass" panel shows through as flat alpha blending, not a blurred
sample of it. Turning `glass` off makes `mavind_panel` fully opaque (the same solid tone as
`mavind_card`) and drops the highlight border — a normal flat panel, no transparency at all.

## Adding a new app to the theme system

1. Add `mavind-theme.workspace = true` to the app's `Cargo.toml`.
2. Write the app's own `src/style.css` — reference `@accent_color`, `@mavind_panel`, etc. where
   you want theme-aware colors; anything else is the app's own fixed styling.
3. Call `mavind_theme::load_app_css(include_str!("style.css"));` once, right after building the
   `gtk4::Application`, before any window is constructed.
