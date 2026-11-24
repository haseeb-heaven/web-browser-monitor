#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
    Classic,
}

impl ThemeMode {
    pub fn next(&self) -> Self {
        match self {
            ThemeMode::Dark => ThemeMode::Light,
            ThemeMode::Light => ThemeMode::Classic,
            ThemeMode::Classic => ThemeMode::Dark,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            ThemeMode::Dark => "Dark",
            ThemeMode::Light => "Light",
            ThemeMode::Classic => "Classic",
        }
    }

    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "dark" => Some(ThemeMode::Dark),
            "light" => Some(ThemeMode::Light),
            "classic" | "retro" | "win95" | "95" => Some(ThemeMode::Classic),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ThemePalette {
    pub bg_app: u32,
    pub bg_panel: u32,
    pub bg_header: u32,
    pub bg_row_alt: u32,
    pub bg_button: u32,
    pub bg_button_active: u32,
    pub border: u32,
    pub border_strong: u32,
    pub text_primary: u32,
    pub text_header: u32,
    pub text_muted: u32,
    pub accent_cyan: u32,
    pub accent_green: u32,
    pub accent_amber: u32,
    pub accent_purple: u32,
    pub accent_blue: u32,
    pub accent_red: u32,
}

impl ThemePalette {
    pub fn for_mode(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Dark => Self {
                bg_app: 0x0f1117,
                bg_panel: 0x161a23,
                bg_header: 0x161a23,
                bg_row_alt: 0x1b202b,
                bg_button: 0x222836,
                bg_button_active: 0x164e63,
                border: 0x262c3a,
                border_strong: 0x3b4459,
                text_primary: 0xf1f5f9,
                text_header: 0xf1f5f9,
                text_muted: 0x94a3b8,
                accent_cyan: 0x22d3ee,
                accent_green: 0x34d399,
                accent_amber: 0xfbbf24,
                accent_purple: 0xc084fc,
                accent_blue: 0x60a5fa,
                accent_red: 0xf87171,
            },
            ThemeMode::Light => Self {
                bg_app: 0xf1f5f9,
                bg_panel: 0xffffff,
                bg_header: 0xffffff,
                bg_row_alt: 0xf8fafc,
                bg_button: 0xe2e8f0,
                bg_button_active: 0xbae6fd,
                border: 0xcbd5e1,
                border_strong: 0x94a3b8,
                text_primary: 0x0f172a,
                text_header: 0x0f172a,
                text_muted: 0x475569,
                accent_cyan: 0x0284c7,
                accent_green: 0x059669,
                accent_amber: 0xd97706,
                accent_purple: 0x7c3aed,
                accent_blue: 0x2563eb,
                accent_red: 0xdc2626,
            },
            ThemeMode::Classic => Self {
                bg_app: 0xd4d0c8,
                bg_panel: 0xffffff,
                bg_header: 0x000080,
                bg_row_alt: 0xf0eee9,
                bg_button: 0xd4d0c8,
                bg_button_active: 0x0a246a,
                border: 0x808080,
                border_strong: 0x404040,
                text_primary: 0x000000,
                text_header: 0xffffff,
                text_muted: 0x404040,
                accent_cyan: 0x008080,
                accent_green: 0x008000,
                accent_amber: 0xb45309,
                accent_purple: 0x800080,
                accent_blue: 0x000080,
                accent_red: 0xc00000,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn themes_cycle_and_parse_cleanly() {
        assert_eq!(ThemeMode::Dark.next(), ThemeMode::Light);
        assert_eq!(ThemeMode::Light.next(), ThemeMode::Classic);
        assert_eq!(ThemeMode::Classic.next(), ThemeMode::Dark);

        assert_eq!(ThemeMode::from_str_loose("DARK"), Some(ThemeMode::Dark));
        assert_eq!(ThemeMode::from_str_loose("light"), Some(ThemeMode::Light));
        assert_eq!(ThemeMode::from_str_loose("classic"), Some(ThemeMode::Classic));
        assert_eq!(ThemeMode::from_str_loose("unknown"), None);
    }

    #[test]
    fn palettes_have_distinct_bg_and_text_contrast() {
        for mode in [ThemeMode::Dark, ThemeMode::Light, ThemeMode::Classic] {
            let p = ThemePalette::for_mode(mode);
            assert_ne!(p.bg_app, p.text_primary);
            assert_ne!(p.bg_panel, p.text_primary);
            assert_ne!(p.bg_header, p.text_header);
        }
        let dark = ThemePalette::for_mode(ThemeMode::Dark);
        let light = ThemePalette::for_mode(ThemeMode::Light);
        let classic = ThemePalette::for_mode(ThemeMode::Classic);
        assert_ne!(dark.bg_app, light.bg_app);
        assert_ne!(light.bg_app, classic.bg_app);
        assert_eq!(classic.bg_header, 0x000080);
    }
}
