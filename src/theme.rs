use ratatui::style::Color;
use serde::{Deserialize, Serialize};

/// Glass surfaces from the frontend design system, baked into solid terminal colors.
/// `forest` / `dawn` names stay in the config file so existing setups keep loading.
/// Each surface is one flat color: a per-row gradient turns into stripes in a terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    /// Dark frosted glass. Config key: `forest`.
    #[default]
    Forest,
    /// Light frosted glass on a cool slate canvas. Config key: `dawn`.
    Dawn,
}

impl Theme {
    pub fn next(self) -> Self {
        match self {
            Self::Forest => Self::Dawn,
            Self::Dawn => Self::Forest,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Forest => "glass",
            Self::Dawn => "light",
        }
    }

    pub fn palette(self) -> Palette {
        match self {
            Self::Forest => Palette::glass_dark(),
            Self::Dawn => Palette::glass_light(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    /// Field behind the panels.
    pub canvas: Color,
    /// Panel fill. The same color on every row.
    pub panel: Color,
    pub border: Color,
    pub accent: Color,
    pub accent_bright: Color,
    pub muted: Color,
    pub text: Color,
    /// Artist names. A hue of their own, separate from the title color.
    pub artist: Color,
    pub yellow: Color,
    pub red: Color,
    pub sel_bg: Color,
    pub sel_fg: Color,
    pub gauge_track: Color,
    pub badge_fg: Color,
    /// Hairline that stays visible against the panel.
    pub edge: Color,
}

impl Palette {
    /// Adwaita dark neutrals and the system green accent.
    fn glass_dark() -> Self {
        Self {
            canvas: Color::Rgb(0x1d, 0x1d, 0x20),
            panel: Color::Rgb(0x2e, 0x2e, 0x32),
            border: Color::Rgb(0x8d, 0x93, 0x8d),
            accent: Color::Rgb(0x26, 0xa2, 0x69),
            accent_bright: Color::Rgb(0x8f, 0xf0, 0xa4),
            muted: Color::Rgb(0xb8, 0xb8, 0xbc),
            text: Color::Rgb(0xff, 0xff, 0xff),
            artist: Color::Rgb(0x57, 0xe3, 0x89),
            yellow: Color::Rgb(0xe5, 0xa5, 0x0a),
            red: Color::Rgb(0xe0, 0x1b, 0x24),
            sel_bg: Color::Rgb(0x1a, 0x5c, 0x40),
            sel_fg: Color::Rgb(0xff, 0xff, 0xff),
            gauge_track: Color::Rgb(0x5c, 0x61, 0x5c),
            badge_fg: Color::Rgb(0xff, 0xff, 0xff),
            edge: Color::Rgb(0x7a, 0x80, 0x7a),
        }
    }

    fn glass_light() -> Self {
        Self {
            canvas: Color::Rgb(0xd4, 0xdb, 0xe6),
            panel: Color::Rgb(0xf3, 0xf6, 0xfa),
            border: Color::Rgb(0x9a, 0xa8, 0xbc),
            accent: Color::Rgb(0x1b, 0x8b, 0xa7),
            accent_bright: Color::Rgb(0x0f, 0x6e, 0x86),
            muted: Color::Rgb(0x3a, 0x43, 0x52),
            text: Color::Rgb(0x1a, 0x1f, 0x2a),
            artist: Color::Rgb(0x0b, 0x5c, 0x72),
            yellow: Color::Rgb(0x8a, 0x58, 0x10),
            red: Color::Rgb(0xb4, 0x3b, 0x32),
            sel_bg: Color::Rgb(0xd5, 0xeb, 0xef),
            sel_fg: Color::Rgb(0x1a, 0x1f, 0x2a),
            gauge_track: Color::Rgb(0x8e, 0x9a, 0xae),
            badge_fg: Color::Rgb(0xf7, 0xf8, 0xfb),
            edge: Color::Rgb(0x5c, 0x6a, 0x80),
        }
    }
}
