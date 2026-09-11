//! Shared appearance state (theme / accent / wallpaper) and CSS loading, used
//! by every Mavind GTK4 app so a change in **Settings → Appearance** shows up
//! consistently everywhere instead of each app hard-coding its own colours.
//!
//! Honesty note: `glass` styling here means alpha-transparent panels with a
//! tint + highlight border, not a live Gaussian blur of what's behind them —
//! labwc/wlroots has no background-blur protocol for layer-shell surfaces to
//! render against. See docs/APPEARANCE.md.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub const DEFAULT_ACCENT: &str = "#8a5cf6";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Appearance {
    #[serde(default = "default_theme")]
    pub theme: String, // "dark" | "light"
    #[serde(default = "default_accent")]
    pub accent: String, // "#rrggbb"
    #[serde(default)]
    pub wallpaper: Option<String>, // source path the user picked (display only)
    #[serde(default = "default_true")]
    pub glass: bool, // translucent panel/menu styling on/off
}
fn default_theme() -> String {
    "dark".into()
}
fn default_accent() -> String {
    DEFAULT_ACCENT.into()
}
fn default_true() -> bool {
    true
}

impl Default for Appearance {
    fn default() -> Self {
        Appearance {
            theme: default_theme(),
            accent: default_accent(),
            wallpaper: None,
            glass: true,
        }
    }
}

impl Appearance {
    pub fn load() -> Appearance {
        fs::read_to_string(config_path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Persist the JSON state and regenerate the shared theme.css that every
    /// app layers on top of its own stylesheet.
    pub fn save(&self) -> std::io::Result<()> {
        let p = config_path();
        if let Some(d) = p.parent() {
            fs::create_dir_all(d)?;
        }
        fs::write(p, serde_json::to_vec_pretty(self).unwrap_or_default())?;
        fs::write(css_path(), self.render_css())?;
        Ok(())
    }

    pub fn render_css(&self) -> String {
        let (r, g, b) = hex_to_rgb(&self.accent).unwrap_or((138, 92, 246));
        let accent = format!("#{r:02x}{g:02x}{b:02x}");
        let accent_soft = format!("rgba({r},{g},{b},0.20)");
        let (bg, fg, dim, glass_panel, card) = if self.theme == "light" {
            ("#f2f2f6", "#1c1c22", "#6b6b76", "rgba(255,255,255,0.70)", "#ffffff")
        } else {
            ("#1c1c22", "#e6e6ec", "#9a9aa6", "rgba(24,24,30,0.58)", "#23232b")
        };
        // glass off: fully opaque panels (same solid tone as cards), no highlight border.
        let panel = if self.glass { glass_panel } else { card };
        let panel_border = if self.glass {
            "rgba(255,255,255,0.10)"
        } else {
            "transparent"
        };
        format!(
            "@define-color accent_color {accent};\n\
             @define-color accent_soft {accent_soft};\n\
             @define-color mavind_bg {bg};\n\
             @define-color mavind_fg {fg};\n\
             @define-color mavind_dim {dim};\n\
             @define-color mavind_panel {panel};\n\
             @define-color mavind_card {card};\n\
             @define-color mavind_panel_border {panel_border};\n\
             window {{ background-color: @mavind_bg; color: @mavind_fg; }}\n\
             .dim {{ color: @mavind_dim; }}\n\
             .mavind-accent {{ color: @accent_color; }}\n\
             button.suggested-action {{ background-color: @accent_color; color: #ffffff; }}\n\
             levelbar > trough > block.filled {{ background-color: @accent_color; }}\n\
             progressbar > trough > progress {{ background-color: @accent_color; }}\n\
             popover {{ background: none; box-shadow: none; }}\n\
             popover > contents {{\n\
                 background-color: @mavind_panel;\n\
                 border: 1px solid @mavind_panel_border;\n\
                 border-radius: 12px;\n\
                 padding: 4px;\n\
             }}\n"
        )
    }
}

pub fn config_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
    std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|p| p.starts_with('/'))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(home).join(".config"))
        .join("mavind")
}
fn config_path() -> PathBuf {
    config_dir().join("appearance.json")
}
fn css_path() -> PathBuf {
    config_dir().join("theme.css")
}

/// The wallpaper file every app/session should treat as "the current one".
/// Settings always writes/converts into this exact path so `swaybg` and the
/// login/OOBE backgrounds don't need to parse JSON.
pub fn wallpaper_path() -> PathBuf {
    config_dir().join("wallpaper.png")
}

/// Load an app's own stylesheet, then layer the shared theme tokens (accent
/// colour, light/dark) on top. Call once, right after building the GTK
/// `Application`. Cheap: two small `CssProvider`s.
pub fn load_app_css(app_css: &str) {
    let Some(display) = gtk4::gdk::Display::default() else {
        return;
    };
    let base = gtk4::CssProvider::new();
    base.load_from_string(app_css);
    gtk4::style_context_add_provider_for_display(&display, &base, gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION);
    reload_theme_css();
}

/// Re-read the shared theme.css from disk and layer a fresh provider on top
/// of whatever `load_app_css` already installed. Call this again after an
/// Appearance change to preview it live, without re-loading the app's own
/// (static) stylesheet.
///
/// A long-running app (the panel) can poll `theme_css_mtime()` on a timer it
/// already has and call this only when the file actually changed, instead of
/// standing up a dedicated file-watcher just for this.
pub fn reload_theme_css() {
    let Some(display) = gtk4::gdk::Display::default() else {
        return;
    };
    let theme_css =
        fs::read_to_string(css_path()).unwrap_or_else(|_| Appearance::default().render_css());
    let theme = gtk4::CssProvider::new();
    theme.load_from_string(&theme_css);
    gtk4::style_context_add_provider_for_display(
        &display,
        &theme,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
    );
}

/// Modification time of the shared theme.css, e.g. for a long-running app
/// that wants to notice live Appearance changes (see `reload_theme_css`).
pub fn theme_css_mtime() -> Option<std::time::SystemTime> {
    fs::metadata(css_path()).ok()?.modified().ok()
}

pub fn hex_to_rgb(s: &str) -> Option<(u8, u8, u8)> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some((r, g, b))
}

/// A small curated palette for the Appearance panel's swatches.
pub const ACCENT_SWATCHES: &[(&str, &str)] = &[
    ("Violet", "#8a5cf6"),
    ("Blue", "#4f8cff"),
    ("Teal", "#22c3a6"),
    ("Green", "#57d98a"),
    ("Amber", "#e0a83f"),
    ("Coral", "#ff6b6b"),
    ("Pink", "#ec5fb0"),
    ("Graphite", "#8a8f98"),
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hex_parse() {
        assert_eq!(hex_to_rgb("#8a5cf6"), Some((0x8a, 0x5c, 0xf6)));
        assert_eq!(hex_to_rgb("4f8cff"), Some((0x4f, 0x8c, 0xff)));
        assert_eq!(hex_to_rgb("nope"), None);
    }
}
