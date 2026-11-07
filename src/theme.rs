//! P2-12 theme selection (baseline 16.1): light, dark and follow-system.
//! P2-14 backdrop material selection (baseline 16.3).
//!
//! The palette is pure data so the mapping can be unit-tested without a
//! window; `main.rs` converts it into the Slint `ThemeColors` struct.

/// The persisted `settings.theme` value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Light,
    Dark,
    System,
}

/// P2-14 (baseline 16.3): the DWM system backdrop material. `None` is the
/// plain solid color; Mica/Acrylic are Windows 11 system-drawn materials.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backdrop {
    None,
    Mica,
    Acrylic,
}

impl Backdrop {
    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "mica" => Self::Mica,
            "acrylic" => Self::Acrylic,
            _ => Self::None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Mica => "mica",
            Self::Acrylic => "acrylic",
        }
    }
}

/// Invalid/unknown backdrop values fall back to `none`: with
/// `renderer-software` the client paints opaque pixels, so `none` is always
/// a working visual.
pub fn normalize_backdrop(value: &str) -> &'static str {
    Backdrop::parse(value).as_str()
}

impl ThemeMode {
    /// Invalid/unknown values fall back to `system`, matching the baseline
    /// default and keeping future config values forward-compatible.
    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "light" => Self::Light,
            "dark" => Self::Dark,
            _ => Self::System,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
            Self::System => "system",
        }
    }

    pub fn is_dark(self, system_dark: bool) -> bool {
        match self {
            Self::Light => false,
            Self::Dark => true,
            Self::System => system_dark,
        }
    }
}

/// One semantic color as 8-bit RGB. The UI converts these into Slint colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

/// The semantic palette shared by all UI surfaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette {
    pub window: Rgb,
    pub panel: Rgb,
    pub sidebar: Rgb,
    pub border: Rgb,
    pub text: Rgb,
    pub secondary: Rgb,
    pub muted: Rgb,
    pub subtle: Rgb,
    pub accent: Rgb,
    pub accent_soft: Rgb,
    /// 卡片规范 V1.0 §6: the card hover background.
    pub card_hover: Rgb,
    /// 卡片规范 V1.0 §6: the card hover border.
    pub card_hover_border: Rgb,
    /// 卡片规范 V1.0 §6: the card pressed background.
    pub card_pressed: Rgb,
    /// UI 优化（2026-11-06 侧边栏样式）: the hover pill on a category row.
    pub sidebar_hover: Rgb,
    /// UI 优化: the selected/hit-target pill on a category row (a clearly
    /// visible accent blend, unlike the very faint accent_soft).
    pub sidebar_active: Rgb,
    /// UI 体验修复（2026-11-07 标题栏规范）: hover background for the
    /// fixed-size title bar / status bar action buttons.
    pub title_hover: Rgb,
    pub alt_row: Rgb,
    pub danger: Rgb,
    pub warning: Rgb,
}

/// Normalizes `settings.accent_color` to lowercase `#rrggbb`; anything that is
/// not exactly six hex digits falls back to the baseline blue.
pub fn normalize_accent(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() == 7
        && trimmed.starts_with('#')
        && trimmed[1..].chars().all(|c| c.is_ascii_hexdigit())
    {
        trimmed.to_ascii_lowercase()
    } else {
        "#2563eb".to_string()
    }
}

fn parse_rgb(hex: &str) -> Rgb {
    let channel = |offset: usize| u8::from_str_radix(&hex[offset..offset + 2], 16).unwrap_or(0);
    Rgb(channel(1), channel(3), channel(5))
}

/// Blends `foreground` over `background` by `alpha` (0.0..=1.0).
pub fn blend(foreground: Rgb, background: Rgb, alpha: f32) -> Rgb {
    let alpha = alpha.clamp(0.0, 1.0);
    let mix = |f: u8, b: u8| {
        (f as f32 * alpha + b as f32 * (1.0 - alpha))
            .round()
            .clamp(0.0, 255.0) as u8
    };
    Rgb(
        mix(foreground.0, background.0),
        mix(foreground.1, background.1),
        mix(foreground.2, background.2),
    )
}

/// Builds the light or dark palette for the given accent.
pub fn palette(dark: bool, accent: &str) -> Palette {
    let accent = parse_rgb(&normalize_accent(accent));
    if dark {
        let panel = Rgb(26, 33, 43);
        Palette {
            window: Rgb(16, 21, 29),
            panel,
            sidebar: Rgb(21, 27, 37),
            border: Rgb(43, 53, 67),
            text: Rgb(230, 237, 247),
            secondary: Rgb(180, 192, 211),
            muted: Rgb(139, 153, 173),
            subtle: Rgb(111, 125, 145),
            accent,
            accent_soft: blend(accent, panel, 0.22),
            card_hover: blend(accent, panel, 0.12),
            card_hover_border: blend(accent, panel, 0.45),
            card_pressed: blend(accent, panel, 0.22),
            sidebar_hover: blend(accent, panel, 0.16),
            sidebar_active: blend(accent, panel, 0.32),
            title_hover: blend(accent, panel, 0.16),
            alt_row: Rgb(22, 29, 39),
            danger: Rgb(248, 113, 113),
            warning: Rgb(251, 191, 36),
        }
    } else {
        let panel = Rgb(255, 255, 255);
        Palette {
            window: Rgb(244, 246, 250),
            panel,
            sidebar: Rgb(238, 242, 248),
            border: Rgb(219, 226, 237),
            text: Rgb(23, 36, 59),
            secondary: Rgb(74, 88, 110),
            muted: Rgb(99, 113, 138),
            subtle: Rgb(154, 166, 184),
            accent,
            accent_soft: blend(accent, panel, 0.09),
            card_hover: Rgb(247, 249, 255),
            card_hover_border: Rgb(184, 204, 244),
            card_pressed: Rgb(238, 243, 255),
            sidebar_hover: blend(accent, panel, 0.08),
            sidebar_active: blend(accent, panel, 0.16),
            title_hover: blend(accent, panel, 0.08),
            alt_row: Rgb(247, 249, 252),
            danger: Rgb(185, 28, 28),
            warning: Rgb(180, 83, 9),
        }
    }
}

/// Reads the current Windows app theme; non-Windows builds are always light.
#[cfg(windows)]
pub fn system_theme_is_dark() -> bool {
    crate::infrastructure::windows::theme::system_uses_dark_theme()
}

#[cfg(not(windows))]
pub fn system_theme_is_dark() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_mode_parses_and_falls_back_to_system() {
        assert_eq!(ThemeMode::parse("light"), ThemeMode::Light);
        assert_eq!(ThemeMode::parse("DARK"), ThemeMode::Dark);
        assert_eq!(ThemeMode::parse("system"), ThemeMode::System);
        assert_eq!(ThemeMode::parse("  dark  "), ThemeMode::Dark);
        assert_eq!(ThemeMode::parse("sepia"), ThemeMode::System);
        assert_eq!(ThemeMode::parse(""), ThemeMode::System);
        assert_eq!(ThemeMode::Light.as_str(), "light");
        assert_eq!(ThemeMode::Dark.as_str(), "dark");
        assert_eq!(ThemeMode::System.as_str(), "system");
    }

    #[test]
    fn follow_system_resolves_from_the_observed_preference() {
        assert!(!ThemeMode::Light.is_dark(true));
        assert!(ThemeMode::Dark.is_dark(false));
        assert!(ThemeMode::System.is_dark(true));
        assert!(!ThemeMode::System.is_dark(false));
    }

    #[test]
    fn accent_normalization_rejects_invalid_values() {
        assert_eq!(normalize_accent("#2563EB"), "#2563eb");
        assert_eq!(normalize_accent("  #16A34A "), "#16a34a");
        for invalid in ["", "#12345", "#1234567", "2563eb", "#xyzxyz", "red"] {
            assert_eq!(normalize_accent(invalid), "#2563eb", "{invalid}");
        }
    }

    #[test]
    fn backdrop_parsing_falls_back_to_none() {
        assert_eq!(Backdrop::parse("mica"), Backdrop::Mica);
        assert_eq!(Backdrop::parse("ACRYLIC"), Backdrop::Acrylic);
        assert_eq!(Backdrop::parse("none"), Backdrop::None);
        for invalid in ["", "blur", "MicaAlt", "wallpaper"] {
            assert_eq!(Backdrop::parse(invalid), Backdrop::None, "{invalid}");
        }
        assert_eq!(normalize_backdrop("Mica"), "mica");
        assert_eq!(normalize_backdrop("nope"), "none");
        assert_eq!(Backdrop::None.as_str(), "none");
    }

    #[test]
    fn light_and_dark_palettes_are_distinct_and_use_the_accent() {
        let light = palette(false, "#16a34a");
        let dark = palette(true, "#16a34a");
        assert_eq!(light.panel, Rgb(255, 255, 255));
        assert_eq!(dark.panel, Rgb(26, 33, 43));
        assert_ne!(light.window, dark.window);
        assert_ne!(light.text, dark.text);
        assert_eq!(light.accent, Rgb(22, 163, 74));
        assert_eq!(dark.accent, Rgb(22, 163, 74));
        assert_ne!(light.accent_soft, dark.accent_soft);
        // The soft tint keeps the accent's hue but is much closer to the panel.
        assert!(light.accent_soft.1 > light.accent.1);
        assert!(dark.accent_soft.1 < dark.text.1);
    }

    #[test]
    fn blending_is_bounded_and_midpoint_correct() {
        assert_eq!(
            blend(Rgb(0, 0, 0), Rgb(255, 255, 255), 0.0),
            Rgb(255, 255, 255)
        );
        assert_eq!(blend(Rgb(0, 0, 0), Rgb(255, 255, 255), 1.0), Rgb(0, 0, 0));
        assert_eq!(
            blend(Rgb(0, 0, 0), Rgb(255, 255, 255), 0.5),
            Rgb(128, 128, 128)
        );
        assert_eq!(blend(Rgb(10, 20, 30), Rgb(0, 0, 0), 2.0), Rgb(10, 20, 30));
    }
}

#[cfg(test)]
mod sidebar_palette_tests {
    use super::*;

    // UI 优化（侧边栏样式）: the selected pill must be clearly distinguishable
    // from the sidebar base in both modes, and the hover pill sits between
    // the base and the selected pill.
    #[test]
    fn sidebar_states_are_visually_separated() {
        for dark in [false, true] {
            let palette = palette(dark, "#2563eb");
            let distance = |left: Rgb, right: Rgb| -> i32 {
                (left.0 as i32 - right.0 as i32).abs()
                    + (left.1 as i32 - right.1 as i32).abs()
                    + (left.2 as i32 - right.2 as i32).abs()
            };
            let base_to_hover = distance(palette.sidebar, palette.sidebar_hover);
            let base_to_active = distance(palette.sidebar, palette.sidebar_active);
            assert!(
                base_to_active >= 24,
                "dark={dark} selected pill too close to the sidebar base: {base_to_active}"
            );
            assert!(
                base_to_hover > 0 && base_to_hover < base_to_active,
                "dark={dark} hover/active ordering wrong: {base_to_hover} vs {base_to_active}"
            );
            assert_ne!(palette.sidebar_active, palette.accent_soft);
        }
    }
}

#[cfg(test)]
mod card_palette_tests {
    use super::*;

    // 卡片规范 V1.0 §6: the light mode values are pinned exactly; the dark
    // mode must stay distinguishable from the panel background.
    #[test]
    fn card_hover_pressed_states_match_the_spec() {
        let light = palette(false, "#2563eb");
        assert_eq!(light.card_hover, Rgb(247, 249, 255));
        assert_eq!(light.card_hover_border, Rgb(184, 204, 244));
        assert_eq!(light.card_pressed, Rgb(238, 243, 255));
        let dark = palette(true, "#2563eb");
        let distance = |left: Rgb, right: Rgb| -> i32 {
            (left.0 as i32 - right.0 as i32).abs()
                + (left.1 as i32 - right.1 as i32).abs()
                + (left.2 as i32 - right.2 as i32).abs()
        };
        assert!(
            distance(dark.card_hover, dark.panel) >= 12,
            "the dark hover state is invisible against the panel"
        );
        assert_ne!(dark.card_hover, dark.card_pressed);
    }
}
