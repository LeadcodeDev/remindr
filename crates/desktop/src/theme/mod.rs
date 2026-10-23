use gpui_kit::component::Theme;
use gpui_kit::{App, Hsla, Window, rgb};

pub struct Palette {
    pub background: u32,
    pub foreground: u32,
    pub muted_foreground: u32,
    pub border: u32,
    pub sidebar: u32,
    pub sidebar_foreground: u32,
    pub sidebar_accent: u32,
    pub accent: u32,
}

pub const LIGHT: Palette = Palette {
    background: 0xFFFFFF,
    foreground: 0x37352F,
    muted_foreground: 0x787774,
    border: 0xE9E9E7,
    sidebar: 0xF7F7F5,
    sidebar_foreground: 0x5F5E5B,
    sidebar_accent: 0xEFEFED,
    accent: 0xEFEFED,
};

pub const DARK: Palette = Palette {
    background: 0x191919,
    foreground: 0xD4D4D4,
    muted_foreground: 0x9B9B9B,
    border: 0x2F2F2F,
    sidebar: 0x202020,
    sidebar_foreground: 0x9B9B9B,
    sidebar_accent: 0x2C2C2C,
    accent: 0x2C2C2C,
};

fn color(hex: u32) -> Hsla {
    rgb(hex).into()
}

#[must_use]
pub fn palette(dark: bool) -> &'static Palette {
    if dark { &DARK } else { &LIGHT }
}

pub fn apply(cx: &mut App) {
    Theme::update(cx, |theme| {
        let palette = palette(theme.is_dark());
        let colors = &mut theme.colors;
        colors.background = color(palette.background);
        colors.foreground = color(palette.foreground);
        colors.muted_foreground = color(palette.muted_foreground);
        colors.border = color(palette.border);
        colors.sidebar = color(palette.sidebar);
        colors.sidebar_foreground = color(palette.sidebar_foreground);
        colors.sidebar_accent = color(palette.sidebar_accent);
        colors.sidebar_accent_foreground = color(palette.foreground);
        colors.sidebar_border = color(palette.border);
        colors.accent = color(palette.accent);
        colors.title_bar = color(palette.background);
        colors.title_bar_border = color(palette.border);
    });
}

pub fn sync(window: &mut Window, cx: &mut App) {
    Theme::sync_system_appearance(Some(window), cx);
    apply(cx);
}

pub fn install(window: &mut Window, cx: &mut App) {
    sync(window, cx);
    window.observe_window_appearance(sync).detach();
}

#[cfg(test)]
mod tests {
    use gpui_kit::component::{ActiveTheme as _, Theme, ThemeMode};
    use gpui_kit::{Hsla, TestAppContext, rgb};

    use super::{DARK, LIGHT, apply, palette};

    fn hsla(hex: u32) -> Hsla {
        rgb(hex).into()
    }

    #[test]
    fn palette_follows_the_mode() {
        assert_eq!(palette(false).background, LIGHT.background);
        assert_eq!(palette(true).background, DARK.background);
    }

    #[test]
    fn palettes_use_the_notion_values() {
        assert_eq!(LIGHT.background, 0xFFFFFF);
        assert_eq!(LIGHT.foreground, 0x37352F);
        assert_eq!(LIGHT.sidebar, 0xF7F7F5);
        assert_eq!(DARK.background, 0x191919);
        assert_eq!(DARK.border, 0x2F2F2F);
        assert_eq!(DARK.sidebar_accent, 0x2C2C2C);
    }

    #[gpui_kit::test]
    fn applying_the_palette_overrides_the_light_theme(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            Theme::change(ThemeMode::Light, None, cx);
            apply(cx);
            let colors = &cx.theme().colors;
            assert_eq!(colors.background, hsla(LIGHT.background));
            assert_eq!(colors.sidebar, hsla(LIGHT.sidebar));
            assert_eq!(colors.border, hsla(LIGHT.border));
            assert_eq!(colors.foreground, hsla(LIGHT.foreground));
            assert_eq!(colors.accent, hsla(LIGHT.accent));
        });
    }

    #[gpui_kit::test]
    fn applying_the_palette_overrides_the_dark_theme(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            Theme::change(ThemeMode::Dark, None, cx);
            apply(cx);
            let colors = &cx.theme().colors;
            assert_eq!(colors.background, hsla(DARK.background));
            assert_eq!(colors.sidebar, hsla(DARK.sidebar));
            assert_eq!(colors.muted_foreground, hsla(DARK.muted_foreground));
            assert_eq!(colors.sidebar_accent, hsla(DARK.sidebar_accent));
        });
    }
}
