//! Colors. Download / upload / status colors switch with light / dark, and the user can customize the download and upload colors (same as macOS).
//! Fixed styles such as backgrounds and cards are in `data/resources/style.css`; colors here are provided to the stylesheet as CSS variables
//! and passed directly to the custom-drawn widgets (gauges, charts).

use gtk::gdk;

use crate::settings::AppSettings;

/// Colors determined by the current light / dark mode and custom colors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    pub download: gdk::RGBA,
    pub upload: gdk::RGBA,
    /// Green for statuses such as "Connected".
    pub accent: gdk::RGBA,
    /// Gauge track ring.
    pub track: gdk::RGBA,
    pub dark: bool,
}

impl Palette {
    /// `download` / `upload` are user-chosen colors (`None` means the theme default).
    pub fn new(dark: bool, download: Option<gdk::RGBA>, upload: Option<gdk::RGBA>) -> Self {
        let (default_download, default_upload, accent, track) = if dark {
            (
                rgb(0x4FD1E8, 1.0),
                rgb(0xE879F9, 1.0),
                rgb(0x8FD94F, 1.0),
                rgb(0xFFFFFF, 0.10),
            )
        } else {
            (
                rgb(0x0E8FB0, 1.0),
                rgb(0xB03CC4, 1.0),
                rgb(0x3E9A1E, 1.0),
                rgb(0x000000, 0.08),
            )
        };
        Palette {
            download: download.unwrap_or(default_download),
            upload: upload.unwrap_or(default_upload),
            accent,
            track,
            dark,
        }
    }

    /// From the custom colors in the settings (invalid values count as unset).
    pub fn from_settings(dark: bool, settings: &AppSettings) -> Self {
        let custom = |color: &Option<String>| color.as_deref().and_then(parse_hex);
        Self::new(
            dark,
            custom(&settings.download_color),
            custom(&settings.upload_color),
        )
    }

    /// CSS variables for the stylesheet (`--anemobit-download` etc.).
    pub fn css(&self) -> String {
        let rgb = |c: &gdk::RGBA| {
            let [r, g, b] = [c.red(), c.green(), c.blue()].map(|v| (v * 255.0).round() as u8);
            format!("rgb({r},{g},{b})")
        };
        format!(
            "window {{ --anemobit-download: {}; --anemobit-upload: {}; --anemobit-accent: {}; }}",
            rgb(&self.download),
            rgb(&self.upload),
            rgb(&self.accent)
        )
    }
}

/// Translucent background for the main window and system tray popup (opacity 0–1). The window itself is transparent, and the content's base color is the system background
/// times the opacity (`--anemobit-surface`, used by `.anemobit-background`); when not fully opaque, text gets an outline (halo)
/// so it stays legible over very transparent backgrounds (same as the Windows floating window: white 90% in light, black 85% in dark).
pub fn translucency_css(dark: bool, main_window_opacity: f64, tray_window_opacity: f64) -> String {
    let percent = |opacity: f64| (opacity.clamp(0.0, 1.0) * 100.0).round() as u32;
    let halo = if dark {
        "rgba(0,0,0,0.85)"
    } else {
        "rgba(255,255,255,0.9)"
    };
    let mut css = String::from(
        "window.anemobit-main-window, window.anemobit-tray-window { background: none; }\n",
    );
    for (class, opacity) in [
        ("anemobit-main-window", main_window_opacity),
        ("anemobit-tray-window", tray_window_opacity),
    ] {
        let percent = percent(opacity);
        css += &format!(
            "window.{class} {{ --anemobit-surface: color-mix(in srgb, var(--window-bg-color) {percent}%, transparent); }}\n"
        );
        if percent < 100 {
            css += &format!(
                "window.{class} label {{ text-shadow: 0 0 1px {halo}, 0 0 2px {halo}, 0 0 3px {halo}; }}\n"
            );
        }
    }
    css
}

/// Parses "#RRGGBB" (surrounding whitespace is ignored); other formats return `None`.
pub fn parse_hex(text: &str) -> Option<gdk::RGBA> {
    let digits = text.trim().strip_prefix('#')?;
    if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(digits, 16)
        .ok()
        .map(|value| rgb(value, 1.0))
}

/// Converts to "#RRGGBB" (ignoring transparency).
pub fn to_hex(color: &gdk::RGBA) -> String {
    let [r, g, b] = [color.red(), color.green(), color.blue()]
        .map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
    format!("#{r:02X}{g:02X}{b:02X}")
}

/// 0xRRGGBB and opacity.
fn rgb(hex: u32, alpha: f32) -> gdk::RGBA {
    gdk::RGBA::new(
        ((hex >> 16) & 0xFF) as f32 / 255.0,
        ((hex >> 8) & 0xFF) as f32 / 255.0,
        (hex & 0xFF) as f32 / 255.0,
        alpha,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_colors_match_macos() {
        let light = Palette::new(false, None, None);
        assert_eq!(to_hex(&light.download), "#0E8FB0");
        assert_eq!(to_hex(&light.upload), "#B03CC4");
        assert_eq!(to_hex(&light.accent), "#3E9A1E");
        assert!(!light.dark);
        let dark = Palette::new(true, None, None);
        assert_eq!(to_hex(&dark.download), "#4FD1E8");
        assert_eq!(to_hex(&dark.upload), "#E879F9");
        assert_eq!(to_hex(&dark.accent), "#8FD94F");
        assert!(dark.dark);
        assert_eq!(light.track, rgb(0x000000, 0.08));
        assert_eq!(dark.track, rgb(0xFFFFFF, 0.10));
    }

    #[test]
    fn custom_colors_override_both_modes() {
        let custom = parse_hex("#123456");
        for dark in [false, true] {
            let palette = Palette::new(dark, custom, None);
            assert_eq!(to_hex(&palette.download), "#123456");
            assert_eq!(palette.upload, Palette::new(dark, None, None).upload);
        }
    }

    #[test]
    fn colors_from_settings() {
        let settings = AppSettings {
            download_color: Some("#102030".into()),
            upload_color: Some("not a color".into()),
            ..Default::default()
        };
        let palette = Palette::from_settings(true, &settings);
        assert_eq!(to_hex(&palette.download), "#102030");
        assert_eq!(palette.upload, Palette::new(true, None, None).upload);
    }

    #[test]
    fn translucent_windows_mix_the_background_color() {
        let css = translucency_css(false, 0.7, 0.25);
        assert!(
            css.contains(
                "window.anemobit-main-window, window.anemobit-tray-window { background: none; }"
            ),
            "{css}"
        );
        assert!(
            css.contains("window.anemobit-main-window { --anemobit-surface: color-mix(in srgb, var(--window-bg-color) 70%, transparent); }"),
            "{css}"
        );
        assert!(
            css.contains("window.anemobit-tray-window { --anemobit-surface: color-mix(in srgb, var(--window-bg-color) 25%, transparent); }"),
            "{css}"
        );
        // The outline is white in light mode and black in dark mode.
        assert!(
            css.contains("window.anemobit-main-window label { text-shadow:"),
            "{css}"
        );
        assert!(css.contains("rgba(255,255,255,0.9)"), "{css}");
        assert!(translucency_css(true, 0.7, 0.7).contains("rgba(0,0,0,0.85)"));
    }

    #[test]
    fn opaque_windows_have_no_text_outline() {
        let css = translucency_css(false, 1.0, 0.5);
        assert!(css.contains("var(--window-bg-color) 100%"), "{css}");
        assert!(!css.contains("window.anemobit-main-window label"), "{css}");
        assert!(css.contains("window.anemobit-tray-window label"), "{css}");
    }

    #[test]
    fn opacity_is_clamped() {
        let css = translucency_css(false, 1.5, -1.0);
        assert!(css.contains("var(--window-bg-color) 100%"), "{css}");
        assert!(css.contains("var(--window-bg-color) 0%"), "{css}");
    }

    #[test]
    fn hex_parsing_is_strict() {
        assert_eq!(
            parse_hex("#1E88E5").map(|c| to_hex(&c)),
            Some("#1E88E5".into())
        );
        assert_eq!(
            parse_hex(" #1e88e5 ").map(|c| to_hex(&c)),
            Some("#1E88E5".into())
        );
        for invalid in [
            "",
            "1E88E5",
            "#1E88E",
            "#1E88E5FF",
            "#GGGGGG",
            "red",
            "rgb(1,2,3)",
        ] {
            assert_eq!(parse_hex(invalid), None, "{invalid:?}");
        }
    }

    #[test]
    fn css_defines_variables() {
        let css = Palette::new(false, parse_hex("#102030"), None).css();
        assert!(css.contains("--anemobit-download: rgb(16,32,48);"), "{css}");
        assert!(css.contains("--anemobit-upload: rgb(176,60,196);"), "{css}");
        assert!(css.contains("--anemobit-accent: rgb(62,154,30);"), "{css}");
    }
}
